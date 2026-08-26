#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, vec},
	imports::std::send_partial_result,
	prelude::*,
};
mod helpers;
mod models;
use helpers::{
	body_to_text, fetch_chapters, has_next, list_url, manga_from_detail, manga_from_list, request,
};
use models::{ChapterBody, ListResponse};
pub const BASE_URL: &str = "https://chikari.moe";
pub const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";
const SORTS: &[&str] = &[
	"popular",
	"trending",
	"top_rated",
	"updated",
	"added",
	"most_bookmarked",
];
struct Chikari;

fn page_list(sort: &str, page: i32, filters: &[FilterValue]) -> Result<MangaPageResult> {
	let limit = 60u32;
	let offset = (page.max(1) as u32 - 1) * limit;
	let data: ListResponse = request(&list_url(sort, limit, offset, None, filters))?;
	let response_offset = if data.offset == 0 {
		offset
	} else {
		data.offset
	};
	let has_next_page = has_next(
		data.items.len(),
		data.total,
		response_offset,
		data.limit.max(limit),
	);
	Ok(MangaPageResult {
		entries: data.items.into_iter().map(manga_from_list).collect(),
		has_next_page,
	})
}
impl Source for Chikari {
	fn new() -> Self {
		Self
	}
	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let limit = 36u32;
		let offset = (page.max(1) as u32 - 1) * limit;
		let sort = filters
			.iter()
			.find_map(|filter| match filter {
				FilterValue::Sort { index, .. } => SORTS.get(*index as usize).copied(),
				_ => None,
			})
			.unwrap_or("popular");
		let data: ListResponse =
			request(&list_url(sort, limit, offset, query.as_deref(), &filters))?;
		let response_offset = if data.offset == 0 {
			offset
		} else {
			data.offset
		};
		let has_next_page = has_next(
			data.items.len(),
			data.total,
			response_offset,
			data.limit.max(limit),
		);
		Ok(MangaPageResult {
			entries: data.items.into_iter().map(manga_from_list).collect(),
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
			manga.copy_from(manga_from_detail(request(&format!(
				"{BASE_URL}/api/novels/{}",
				manga.key
			))?));
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			manga.chapters = Some(fetch_chapters(&manga.key)?);
		}
		Ok(manga)
	}
	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let data: ChapterBody = request(&format!(
			"{BASE_URL}/api/novels/{}/chapters/{}/read",
			manga.key, chapter.key
		))?;
		Ok(vec![Page {
			content: PageContent::text(body_to_text(data.body)?),
			..Default::default()
		}])
	}
}
impl ListingProvider for Chikari {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = SORTS
			.iter()
			.find(|s| **s == listing.id)
			.ok_or_else(|| error!("Unknown listing: {}", listing.id))?;
		page_list(sort, page, &[])
	}
}
impl DeepLinkHandler for Chikari {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.split(['?', '#']).next().unwrap_or("");
		let Some(path) = path.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
		if parts.len() == 2 && parts[0] == "novels" && valid_slug(parts[1]) {
			return Ok(Some(DeepLinkResult::Manga {
				key: parts[1].into(),
			}));
		}
		if parts.len() == 3
			&& parts[0] == "novels"
			&& valid_slug(parts[1])
			&& valid_number(parts[2])
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: parts[1].into(),
				key: parts[2].into(),
			}));
		}
		if parts.len() == 6
			&& parts[0] == "api"
			&& parts[1] == "novels"
			&& parts[3] == "chapters"
			&& parts[5] == "read"
			&& valid_slug(parts[2])
			&& valid_number(parts[4])
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: parts[2].into(),
				key: parts[4].into(),
			}));
		}
		if parts.len() == 4
			&& parts[0] == "novels"
			&& parts[2] == "chapters"
			&& valid_slug(parts[1])
			&& valid_number(parts[3])
		{
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: parts[1].into(),
				key: parts[3].into(),
			}));
		}
		Ok(None)
	}
}
fn valid_slug(value: &str) -> bool {
	!value.is_empty()
		&& value
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
fn valid_number(value: &str) -> bool {
	let mut has_dot = false;
	let mut digits = 0;
	let mut fractional_digits = 0;
	for byte in value.bytes() {
		match byte {
			b'0'..=b'9' => {
				digits += 1;
				if has_dot {
					fractional_digits += 1;
				}
			}
			b'.' if !has_dot && digits > 0 => has_dot = true,
			_ => return false,
		}
	}
	if digits == 0 || (has_dot && fractional_digits == 0) {
		return false;
	}
	match value.parse::<f32>() {
		Ok(number) => number.is_finite() && number > 0.0,
		Err(_) => false,
	}
}
register_source!(Chikari, ListingProvider, DeepLinkHandler);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::helpers::chapter_key;
	use aidoku_test::aidoku_test;
	#[aidoku_test]
	fn search_returns_shadow_slave() {
		let result = Chikari
			.get_search_manga_list(Some("shadow slave".into()), 1, Vec::new())
			.expect("search failed");
		assert!(
			result
				.entries
				.iter()
				.any(|m| m.title.to_ascii_lowercase().contains("shadow slave"))
		);
	}
	#[aidoku_test]
	fn details_fetch_all_chapters() {
		let manga = Chikari
			.get_manga_update(
				Manga {
					key: "shadow-slave".into(),
					..Default::default()
				},
				true,
				true,
			)
			.expect("update failed");
		assert_eq!(manga.title, "Shadow Slave");
		assert!(manga.description.is_some());
		let chapters = manga.chapters.expect("chapters missing");
		assert!(chapters.len() > 3000);
		assert!(
			chapters
				.iter()
				.any(|chapter| chapter.date_uploaded.is_some())
		);
		assert!(
			chapters.first().and_then(|chapter| chapter.chapter_number)
				> chapters.last().and_then(|chapter| chapter.chapter_number)
		);
	}
	#[aidoku_test]
	fn chapter_one_is_text() {
		let pages = Chikari
			.get_page_list(
				Manga {
					key: "shadow-slave".into(),
					..Default::default()
				},
				Chapter {
					key: chapter_key(1.0),
					..Default::default()
				},
			)
			.expect("chapter failed");
		assert_eq!(pages.len(), 1);
		match &pages[0].content {
			PageContent::Text(text) => assert!(!text.is_empty()),
			_ => panic!("expected text"),
		}
	}
	#[aidoku_test]
	fn deep_links_resolve() {
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/novels/shadow-slave".into()).unwrap(), Some(DeepLinkResult::Manga { key }) if key == "shadow-slave")
		);
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/api/novels/shadow-slave/chapters/1/read".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "shadow-slave" && key == "1")
		);
		assert!(
			matches!(Chikari.handle_deep_link("https://chikari.moe/novels/shadow-slave/1".into()).unwrap(), Some(DeepLinkResult::Chapter { manga_key, key }) if manga_key == "shadow-slave" && key == "1")
		);
	}
}
