#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	prelude::*,
};

mod helpers;
mod models;

use helpers::*;
use models::*;

struct BakaTsuki;

const PAGE_SIZE: i32 = 40;

// ── Source ────────────────────────────────────────────────────────

impl Source for BakaTsuki {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(q) = query.as_deref() {
			if q.is_empty() {
				return Ok(MangaPageResult {
					entries: vec![],
					has_next_page: false,
				});
			}
			return search_remote(q, page);
		}

		// Library listing: fetch catalogue and paginate client-side
		let catalogue = build_catalogue()?;

		let mut status_filter: Option<&str> = None;
		let mut reverse = false;

		for filter in &filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if *index == 1 {
						reverse = true;
					}
				}
				FilterValue::Select { id, value } if id == "status" && value != "all" => {
					status_filter = Some(value);
				}
				_ => {}
			}
		}

		let mut entries: Vec<CatalogueEntry> = catalogue
			.into_iter()
			.filter(|e| {
				if let Some(status) = status_filter {
					e.status == status
				} else {
					true
				}
			})
			.collect();

		if reverse {
			entries.reverse();
		}

		let start = ((page - 1) * PAGE_SIZE) as usize;
		if start >= entries.len() {
			return Ok(MangaPageResult {
				entries: vec![],
				has_next_page: false,
			});
		}

		let end = core::cmp::min(start + PAGE_SIZE as usize, entries.len());
		let slice = &entries[start..end];
		let has_next_page = end < entries.len();

		let entries = slice
			.iter()
			.map(|e| Manga {
				key: e.title.clone(),
				title: e.title.clone(),
				url: Some(format!(
					"{}/index.php?title={}",
					BASE_URL,
					e.title.replace(' ', "%20")
				)),
				..Default::default()
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let detail = fetch_novel_details(&manga.key)?;
			manga.title = detail.title;
			manga.cover = detail.cover;
			manga.description = detail.description;
			manga.authors = detail.authors;
			manga.tags = detail.tags;
			manga.status = detail.status;
			manga.content_rating = detail.content_rating;
			manga.url = Some(format!(
				"{}/index.php?title={}",
				BASE_URL,
				manga.key.replace(' ', "%20")
			));
		}
		if needs_chapters {
			let chapters = fetch_chapter_list(&manga.key)?;
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let page_title = chapter.title.as_deref().unwrap_or(&chapter.key);
		let html = fetch_chapter_content(page_title)?;

		let text = if html.is_empty() {
			"(empty chapter)".into()
		} else {
			html
		};

		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

// ── ListingProvider ───────────────────────────────────────────────

impl ListingProvider for BakaTsuki {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"library" => self.get_search_manga_list(None, page, vec![]),
			"latest" => {
				let results = fetch_recent_changes()?;
				let start = ((page - 1) * PAGE_SIZE) as usize;
				let has_next_page = (start + PAGE_SIZE as usize) < results.len();
				let slice = if start >= results.len() {
					vec![]
				} else {
					let end = core::cmp::min(start + PAGE_SIZE as usize, results.len());
					results[start..end].to_vec()
				};
				Ok(MangaPageResult {
					entries: slice,
					has_next_page,
				})
			}
			_ => bail!("Unknown listing: {}", listing.id),
		}
	}
}

// ── DeepLinkHandler ───────────────────────────────────────────────

impl DeepLinkHandler for BakaTsuki {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split(['?', '#'])
			.next()
			.unwrap_or(&url)
			.trim_start_matches("https://www.baka-tsuki.org/project/")
			.trim_start_matches("index.php?title=");

		if path.is_empty() {
			return Ok(None);
		}

		// Check for chapter (contains ":")
		if let Some(idx) = path.find(':') {
			let novel_title = path[..idx].replace("%20", " ");
			let chapter_path = path[idx..].replace("%20", " ");
			let chapters = fetch_chapter_list(&novel_title)?;
			for ch in &chapters {
				if let Some(ref ch_url) = ch.url
					&& ch_url.ends_with(&chapter_path)
				{
					return Ok(Some(DeepLinkResult::Chapter {
						manga_key: novel_title,
						key: ch.key.clone(),
					}));
				}
			}
		}

		let novel_title = path.replace("%20", " ");
		Ok(Some(DeepLinkResult::Manga { key: novel_title }))
	}
}

// ── Search implementation ─────────────────────────────────────────

fn search_remote(search_term: &str, page: i32) -> Result<MangaPageResult> {
	if page == 1 {
		// Hybrid local + remote search on page 1
		let catalogue = build_catalogue()?;
		let query_lower = search_term.to_ascii_lowercase();

		// Local scoring
		let mut local: Vec<(i32, String)> = catalogue
			.iter()
			.map(|e| (score(&e.title, &query_lower), e.title.clone()))
			.filter(|(s, _)| *s > 0)
			.collect();
		local.sort_by_key(|a| core::cmp::Reverse(a.0));
		local.truncate(PAGE_SIZE as usize);

		// Remote prefixsearch
		let params = vec![
			("action", "query"),
			("list", "prefixsearch"),
			("pssearch", search_term),
			("psnamespace", "0"),
			("pslimit", "50"),
			("format", "json"),
			("formatversion", "2"),
		];
		let resp = mw_query(&params)?;
		let mut remote_titles: Vec<String> = resp
			.query
			.iter()
			.flat_map(|q| q.prefixsearch.iter().flat_map(|v| v.iter()))
			.map(|t| t.title.clone())
			.collect();

		// Remote search
		let params = vec![
			("action", "query"),
			("list", "search"),
			("srsearch", search_term),
			("srnamespace", "0"),
			("srlimit", "50"),
			("format", "json"),
			("formatversion", "2"),
		];
		let resp = mw_query(&params)?;
		for t in resp
			.query
			.iter()
			.flat_map(|q| q.search.iter().flat_map(|v| v.iter()))
		{
			if !remote_titles.contains(&t.title) {
				remote_titles.push(t.title.clone());
			}
		}

		// Score remote results lower than local
		let mut combined: Vec<(i32, String)> = local;
		for title in &remote_titles {
			if !combined.iter().any(|(_, t)| t == title) {
				let s = score(title, &query_lower) - 50;
				if s > -50 {
					combined.push((s, title.clone()));
				}
			}
		}
		combined.sort_by_key(|a| core::cmp::Reverse(a.0));
		combined.truncate(PAGE_SIZE as usize);

		let has_next_page = combined.len() == PAGE_SIZE as usize;
		let entries = combined
			.into_iter()
			.map(|(_, title)| Manga {
				key: title.clone(),
				title: title.clone(),
				url: Some(format!(
					"{}/index.php?title={}",
					BASE_URL,
					title.replace(' ', "%20")
				)),
				..Default::default()
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	} else {
		// Page 2+: pure API search
		let offset = ((page - 1) * PAGE_SIZE).to_string();
		let params = vec![
			("action", "query"),
			("list", "search"),
			("srsearch", search_term),
			("srnamespace", "0"),
			("srlimit", "40"),
			("sroffset", &offset),
			("format", "json"),
			("formatversion", "2"),
		];
		let resp = mw_query(&params)?;
		let results: Vec<String> = resp
			.query
			.iter()
			.flat_map(|q| q.search.iter().flat_map(|v| v.iter()))
			.map(|t| t.title.clone())
			.collect();

		let has_next_page = results.len() == 40;
		let entries = results
			.into_iter()
			.map(|title| Manga {
				key: title.clone(),
				title: title.clone(),
				url: Some(format!(
					"{}/index.php?title={}",
					BASE_URL,
					title.replace(' ', "%20")
				)),
				..Default::default()
			})
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

/// Simple relevance scoring: exact substring match > word match > partial.
fn score(title: &str, query: &str) -> i32 {
	let lower = title.to_ascii_lowercase();
	if lower == query {
		return 100;
	}
	if lower.contains(query) {
		return 80;
	}
	let query_words: Vec<&str> = query.split_whitespace().collect();
	let title_words: Vec<&str> = lower.split_whitespace().collect();
	let matching = query_words
		.iter()
		.filter(|qw| title_words.iter().any(|tw| tw.contains(*qw)))
		.count();
	if matching > 0 {
		return (matching as i32) * 20;
	}
	0
}

register_source!(BakaTsuki, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_returns_results() {
		let source = BakaTsuki;
		let result = source
			.get_search_manga_list(Some("Apocalypse Witch".into()), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
	}

	#[aidoku_test]
	fn library_has_entries() {
		let source = BakaTsuki;
		let result = source
			.get_search_manga_list(None, 1, Vec::new())
			.expect("library failed");
		assert!(!result.entries.is_empty(), "expected at least one entry");
	}

	#[aidoku_test]
	fn details_has_chapters() {
		let source = BakaTsuki;
		let manga = Manga {
			key: "Apocalypse Witch".into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		assert!(!manga.title.is_empty());
		let chapters = manga.chapters.expect("no chapters");
		assert!(!chapters.is_empty(), "expected at least one chapter");
	}

	#[aidoku_test]
	fn page_list_returns_content() {
		let source = BakaTsuki;
		let manga = Manga {
			key: "Apocalypse Witch".into(),
			..Default::default()
		};
		let chapter = Chapter {
			key: "Apocalypse Witch:Volume1 Chapter1".into(),
			title: Some("Apocalypse Witch:Volume1 Chapter1".into()),
			..Default::default()
		};
		let pages = source
			.get_page_list(manga, chapter)
			.expect("get_page_list failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => {
				assert!(!text.is_empty(), "expected non-empty chapter text");
			}
			_ => panic!("expected PageContent::Text"),
		}
	}
}
