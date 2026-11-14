#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::send_partial_result,
	prelude::*,
};

mod helpers;
mod markdown;
mod models;
mod mtl;
mod settings;
mod watermark;

use helpers::{
	build_search_url, chapter_url_matches, deep_link_slugs, fetch_chapter_list, fetch_multi_status,
	filter_by_rating, is_server_status, merge_with_persistent, parse_filters, request,
	resolve_slug, series_path,
};
use markdown::to_markdown;
use models::{ChapterDetailData, ListData, TitleDetailData, TrendingData};
use settings::{
	content_rating_filter as persistent_content_rating, status_filter as persistent_status,
};

pub const BASE_URL: &str = "https://novelbuddy.me";
pub const API_URL: &str = "https://api.novelbuddy.me";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 \
	 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";

struct NovelBuddy;

impl Source for NovelBuddy {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let ctx = parse_filters(filters);
		let hide = settings::hide_mtl() && !query.as_deref().is_some_and(mtl::mentions_mtl);

		let all_ratings = merge_with_persistent(
			ctx.content_rating_filter.clone(),
			persistent_content_rating(),
		);
		let all_statuses = merge_with_persistent(ctx.status_filter.clone(), persistent_status());

		let match_nothing = ctx
			.content_rating_filter
			.as_ref()
			.is_some_and(|ratings| ratings.is_empty())
			|| ctx
				.status_filter
				.as_ref()
				.is_some_and(|statuses| statuses.is_empty());
		if match_nothing {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}

		if all_statuses.iter().any(|s| !is_server_status(s)) {
			let url = build_search_url(&ctx, query.as_deref(), page, None);
			let data: ListData = request(&url)?;
			let mut entries: Vec<Manga> = data
				.items
				.into_iter()
				.filter(|item| {
					item.status.as_deref().is_some_and(|status| {
						all_statuses
							.iter()
							.any(|selected| selected.eq_ignore_ascii_case(status))
					})
				})
				.map(Manga::from)
				.collect();
			if hide {
				mtl::retain_manga(&mut entries);
			}
			entries = filter_by_rating(entries, &all_ratings);
			return Ok(MangaPageResult {
				entries,
				has_next_page: data.pagination.has_next,
			});
		}

		if all_statuses.len() > 1 {
			return fetch_multi_status(
				&ctx,
				query.as_deref(),
				page,
				&all_statuses,
				&all_ratings,
				hide,
			);
		}

		let status = all_statuses.first().map(|s| s.as_str());
		let url = build_search_url(&ctx, query.as_deref(), page, status);
		let data: ListData = request(&url)?;
		let mut entries: Vec<Manga> = data.items.into_iter().map(Manga::from).collect();
		if hide {
			mtl::retain_manga(&mut entries);
		}
		entries = filter_by_rating(entries, &all_ratings);
		Ok(MangaPageResult {
			entries,
			has_next_page: data.pagination.has_next,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let url = format!("{API_URL}/titles/{}", manga.key);
			let data: TitleDetailData = request(&url)?;
			manga.copy_from(data.title.into());
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			let mut chapters = fetch_chapter_list(&manga.key)?;
			if settings::hide_mtl() {
				mtl::retain_chapters(&mut chapters);
			}
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/titles/{}/chapters/{}", manga.key, chapter.key);
		let data: ChapterDetailData = request(&url)?;
		let body = data
			.chapter
			.content
			.as_deref()
			.map(to_markdown)
			.unwrap_or_default();
		let text = if body.is_empty() {
			"(empty chapter)".to_string()
		} else {
			body
		};
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for NovelBuddy {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"trending-week" | "trending-today" => {
				let window = if listing.id == "trending-week" {
					"week"
				} else {
					"today"
				};
				let url = format!("{API_URL}/trending/titles?window={window}");
				let data: TrendingData = request(&url)?;
				let mut entries: Vec<Manga> = data.items.into_iter().map(Manga::from).collect();
				if settings::hide_mtl() {
					mtl::retain_manga(&mut entries);
				}
				entries = filter_by_rating(entries, &persistent_content_rating());
				Ok(MangaPageResult {
					entries,
					has_next_page: false,
				})
			}
			"latest" => {
				let url = format!("{API_URL}/titles/search?sort=latest&page={}", page.max(1));
				let data: ListData = request(&url)?;
				let mut entries: Vec<Manga> = data.items.into_iter().map(Manga::from).collect();
				if settings::hide_mtl() {
					mtl::retain_manga(&mut entries);
				}
				entries = filter_by_rating(entries, &persistent_content_rating());
				Ok(MangaPageResult {
					entries,
					has_next_page: data.pagination.has_next,
				})
			}
			_ => {
				bail!("Unknown listing: {}", listing.id);
			}
		}
	}
}

impl DeepLinkHandler for NovelBuddy {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = series_path(&url) else {
			return Ok(None);
		};
		let path = path.trim_start_matches('/').trim_end_matches('/');
		if path.is_empty() {
			return Ok(None);
		}
		let (series_slug, chapter_slug) = deep_link_slugs(path);
		if series_slug.is_empty() {
			return Ok(None);
		}

		let manga_key = resolve_slug(series_slug)?;

		if !chapter_slug.is_empty() {
			let chapters = fetch_chapter_list(&manga_key)?;
			let chapter_key = chapters.into_iter().find_map(|c| {
				let url = c.url.as_deref().unwrap_or("");
				if chapter_url_matches(url, chapter_slug) {
					Some(c.key)
				} else {
					None
				}
			});
			if let Some(key) = chapter_key {
				return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
			}
		}
		Ok(Some(DeepLinkResult::Manga { key: manga_key }))
	}
}

register_source!(NovelBuddy, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::FilterValue;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_returns_results() {
		let source = NovelBuddy;
		let result = source
			.get_search_manga_list(Some("shadow slave".into()), 1, Vec::new())
			.expect("search failed");
		assert!(!result.entries.is_empty(), "expected at least one result");
		assert!(
			result
				.entries
				.iter()
				.any(|m| m.title.to_lowercase().contains("shadow slave")),
			"expected 'Shadow Slave' in results"
		);
	}

	#[aidoku_test]
	fn series_detail_has_many_chapters() {
		let source = NovelBuddy;
		let manga = Manga {
			key: "VYPGVZ8z".into(),
			..Default::default()
		};
		let manga = source
			.get_manga_update(manga, true, true)
			.expect("get_manga_update failed");
		assert_eq!(manga.title, "Shadow Slave");
		assert!(manga.description.is_some());
		assert!(manga.authors.is_some());
		let chapters = manga.chapters.expect("no chapters returned");
		assert!(
			chapters.len() > 100,
			"expected >100 chapters, got {}",
			chapters.len()
		);
		// The ISO-8601 format string must keep matching the API's timestamps;
		// a silent mismatch would leave every chapter without an upload date.
		assert!(
			chapters.iter().any(|c| c.date_uploaded.is_some()),
			"expected at least one parsed upload date"
		);
	}

	#[aidoku_test]
	fn page_list_returns_text_page() {
		let source = NovelBuddy;
		let manga = Manga {
			key: "VYPGVZ8z".into(),
			..Default::default()
		};
		// Resolve a chapter key from the live list rather than hardcoding
		// one: the site occasionally removes or re-publishes chapters.
		let chapters = fetch_chapter_list(&manga.key).expect("chapter list failed");
		let key = chapters
			.last()
			.map(|c| c.key.clone())
			.expect("no chapters returned");
		let chapter = Chapter {
			key,
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

	#[aidoku_test]
	fn deep_link_resolves_series() {
		let source = NovelBuddy;
		let result = source
			.handle_deep_link("https://novelbuddy.me/shadow-slave".into())
			.expect("deep link failed")
			.expect("expected Some(DeepLinkResult)");
		match result {
			DeepLinkResult::Manga { key } => assert_eq!(key, "VYPGVZ8z"),
			_ => panic!("expected Manga deep link"),
		}
	}

	#[aidoku_test]
	fn status_multi_select_finds_results() {
		use aidoku::alloc::vec;
		let filters = vec![FilterValue::MultiSelect {
			id: "status".into(),
			included: vec!["ongoing".into(), "completed".into()],
			excluded: Vec::new(),
		}];
		let result = NovelBuddy
			.get_search_manga_list(Some("shadow slave".into()), 1, filters)
			.expect("search with status filter failed");
		assert!(
			!result.entries.is_empty(),
			"status multi-select should return results, got {}",
			result.entries.len()
		);
	}
}
