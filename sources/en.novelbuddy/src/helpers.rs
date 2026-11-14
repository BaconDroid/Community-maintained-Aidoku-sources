use aidoku::{
	AidokuError, Chapter, ContentRating, FilterValue, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{html::Html, net::Request, std::parse_date},
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::{
	ApiResponse, BySlugData, ChapterListData, ChapterListItem, ListData, TitleDetail, TitleListItem,
};
use crate::{API_URL, BASE_URL, USER_AGENT};

// Length separates the id from the slug in `/titles/<id>-<slug>`; relaxing it
// parses `/titles/shadow-slave` as the id `shadow`.
const CANONICAL_ID_LEN: usize = 8;

const ALLOWED_HOSTS: &[&str] = &["novelbuddy.me", "www.novelbuddy.me"];
const ALLOWED_SCHEMES: &[&str] = &["http", "https"];

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	let response = Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json, text/plain, */*")
		.header("Referer", "https://novelbuddy.me/")
		.header("Origin", BASE_URL)
		.json_owned::<ApiResponse<T>>()?;
	if !response.success {
		let msg = response
			.message
			.unwrap_or_else(|| "API request failed".into());
		bail!("{msg}");
	}
	response.data.ok_or_else(|| error!("API returned no data"))
}

pub fn fetch_chapter_list(title_id: &str) -> Result<Vec<Chapter>> {
	let url = format!("{API_URL}/titles/{title_id}/chapters");
	let data: ChapterListData = request(&url)?;
	Ok(data.chapters.into_iter().map(Chapter::from).collect())
}

pub fn resolve_slug(slug: &str) -> Result<String> {
	let url = format!("{API_URL}/titles/by-slug/{slug}");
	let data: BySlugData = request(&url)?;
	parse_id_from_canonical(&data.new_url)
		.ok_or_else(|| error!("Could not parse id from {}", data.new_url))
}

impl From<TitleListItem> for Manga {
	fn from(item: TitleListItem) -> Self {
		let slug = item.slug.as_deref().unwrap_or("");
		let url = if slug.is_empty() {
			None
		} else {
			Some(format!("{BASE_URL}/{slug}"))
		};
		let mut genre_names: Vec<String> = item.genres.into_iter().map(|g| g.name).collect();
		if let Some(slugs) = item.genre_slugs {
			for slug in slugs.split(',').map(str::trim).filter(|s| !s.is_empty()) {
				genre_names.push(slug.into());
			}
		}
		let content_rating = content_rating(item.is_adult, &genre_names);
		Manga {
			key: item.id,
			title: item.name,
			cover: item.cover,
			url,
			content_rating,
			..Default::default()
		}
	}
}

impl From<TitleDetail> for Manga {
	fn from(detail: TitleDetail) -> Self {
		let url = detail.slug.as_deref().map(|s| format!("{BASE_URL}/{s}"));
		let description = detail
			.summary
			.as_deref()
			.map(html_to_text)
			.filter(|t| !t.is_empty());
		let status = detail
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or(MangaStatus::Unknown);
		let authors: Vec<String> = detail.authors.into_iter().map(|a| a.name).collect();
		let artists: Vec<String> = detail.artists.into_iter().map(|a| a.name).collect();
		let mut tags: Vec<String> = detail.genres.into_iter().map(|g| g.name).collect();
		for tag in detail.tags.into_iter().map(|t| t.name) {
			if !tags.iter().any(|t| t == &tag) {
				tags.push(tag);
			}
		}
		let rating = content_rating(detail.is_adult, &tags);
		Manga {
			key: detail.id,
			title: detail.name,
			cover: detail.cover,
			url,
			description,
			authors: (!authors.is_empty()).then_some(authors),
			artists: (!artists.is_empty()).then_some(artists),
			tags: (!tags.is_empty()).then_some(tags),
			status,
			content_rating: rating,
			..Default::default()
		}
	}
}

impl From<ChapterListItem> for Chapter {
	fn from(item: ChapterListItem) -> Self {
		let chapter_number = parse_chapter_number(&item.name);
		let date_uploaded = item.updated_at.as_deref().and_then(parse_iso_date);
		let url = item.url.as_deref().map(absolute_url);
		Chapter {
			key: item.id,
			title: Some(item.name),
			chapter_number,
			date_uploaded,
			url,
			..Default::default()
		}
	}
}

fn content_rating(is_adult: bool, tags: &[String]) -> ContentRating {
	if is_adult {
		return ContentRating::NSFW;
	}
	if tags.iter().any(|tag| is_suggestive_signal(tag)) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn is_suggestive_signal(value: &str) -> bool {
	const SUGGESTIVE_TAGS: &[&str] = &["adult", "bl", "ecchi", "smut"];
	SUGGESTIVE_TAGS
		.iter()
		.any(|tag| value.trim().eq_ignore_ascii_case(tag))
}

pub fn parse_status(value: &str) -> MangaStatus {
	match value.to_ascii_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "canceled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn absolute_url(path_or_url: &str) -> String {
	if path_or_url.starts_with("http") {
		path_or_url.into()
	} else if path_or_url.starts_with('/') {
		format!("{BASE_URL}{path_or_url}")
	} else {
		format!("{BASE_URL}/{path_or_url}")
	}
}

pub fn series_path(url: &str) -> Option<&str> {
	let without_fragment = url.split(['?', '#']).next()?;
	let (scheme, authority_and_path) = without_fragment.split_once("://")?;
	if !ALLOWED_SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
		return None;
	}
	let (authority, path) = authority_and_path
		.split_once('/')
		.unwrap_or((authority_and_path, ""));
	if authority.contains('\\') {
		return None;
	}
	let host = authority
		.rsplit('@')
		.next()
		.unwrap_or(authority)
		.split(':')
		.next()
		.unwrap_or(authority);
	let host = host.to_ascii_lowercase();
	ALLOWED_HOSTS.contains(&host.as_str()).then_some(path)
}

pub fn parse_iso_date(value: &str) -> Option<i64> {
	// Fallback: strip fractional seconds and zone, then parse naively as UTC.
	parse_date(value, "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")
		.or_else(|| parse_date(value, "yyyy-MM-dd'T'HH:mm:ss'Z'"))
		.or_else(|| {
			let naive = value.split('.').next()?.trim_end_matches('Z');
			parse_date(naive, "yyyy-MM-ddTHH:mm:ss")
		})
}

pub fn parse_chapter_number(name: &str) -> Option<f32> {
	// Number is the first numeric run; keep a single '.' for decimal chapters.
	let mut num = String::default();
	let mut seen_dot = false;
	for ch in name.chars() {
		if ch.is_ascii_digit() {
			num.push(ch);
		} else if ch == '.' && !seen_dot && !num.is_empty() {
			seen_dot = true;
			num.push(ch);
		} else if !num.is_empty() {
			break;
		}
	}
	num.parse().ok()
}

pub fn parse_id_from_canonical(new_url: &str) -> Option<String> {
	let trimmed = new_url.trim_start_matches("/titles/");
	let id = trimmed.split('-').next()?;
	if id.len() == CANONICAL_ID_LEN && id.chars().all(|c| c.is_ascii_alphanumeric()) {
		Some(id.into())
	} else {
		None
	}
}

pub fn html_to_text(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(html) else {
		return String::default();
	};
	let paragraphs = doc
		.select("p")
		.map(|els| {
			els.filter_map(|el| el.text())
				.map(|text| text.trim().to_string())
				.filter(|trimmed| !trimmed.is_empty())
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();
	if !paragraphs.is_empty() {
		return paragraphs.join("\n\n");
	}
	// `select_first`, not `select`: a nested wrapper would concatenate its text
	// twice. `*` takes the outermost wrapper, whatever the tag is.
	doc.select_first("*")
		.and_then(|el| el.text())
		.map(|text| text.trim().to_string())
		.filter(|text| !text.is_empty())
		.unwrap_or_default()
}

pub const CONTENT_RATING_IDS: &[&str] = &["safe", "suggestive", "nsfw"];

const SORT_IDS: &[&str] = &[
	"popular",
	"latest",
	"newest",
	"rating",
	"views",
	"bookmarks",
];

const STATUS_IDS: &[&str] = &["ongoing", "completed", "hiatus", "cancelled"];

const SERVER_STATUS_IDS: &[&str] = &["ongoing", "completed"];

pub struct SearchContext {
	pub qs: QueryParameters,
	pub status_filter: Option<Vec<String>>,
	pub content_rating_filter: Option<Vec<String>>,
}

pub fn resolve_multi_select(
	all_options: &[&'static str],
	included: Vec<String>,
	excluded: Vec<String>,
) -> Vec<String> {
	let lower_excluded: Vec<String> = excluded.iter().map(|e| e.to_lowercase()).collect();
	let base: Vec<String> = if included.is_empty() {
		all_options.iter().map(|s| (*s).into()).collect()
	} else {
		included
	};
	base.into_iter()
		.filter(|s| !lower_excluded.contains(&s.to_lowercase()))
		.collect()
}

pub fn parse_filters(filters: Vec<FilterValue>) -> SearchContext {
	let mut qs = QueryParameters::new();
	let mut has_sort = false;
	let mut excluded_genres = crate::settings::hidden_genres();
	let mut status_filter: Option<Vec<String>> = None;
	let mut content_rating_filter: Option<Vec<String>> = None;

	for filter in filters {
		match filter {
			FilterValue::Sort { index, .. } => {
				if let Some(s) = SORT_IDS.get(index as usize) {
					qs.push("sort", Some(*s));
					has_sort = true;
				}
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "status" => {
				status_filter = Some(resolve_multi_select(STATUS_IDS, included, excluded));
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "contentRating" => {
				content_rating_filter =
					Some(resolve_multi_select(CONTENT_RATING_IDS, included, excluded));
			}
			FilterValue::MultiSelect {
				id,
				included,
				excluded,
			} if id == "genres" => {
				for genre in &included {
					if let Some(pos) = excluded_genres.iter().position(|g| g == genre) {
						excluded_genres.swap_remove(pos);
					}
				}
				if !included.is_empty() {
					qs.push("genres", Some(&included.join(",")));
				}
				for genre in excluded {
					if !excluded_genres.contains(&genre) {
						excluded_genres.push(genre);
					}
				}
			}
			_ => {}
		}
	}

	if !has_sort {
		qs.push("sort", Some("popular"));
	}
	if !excluded_genres.is_empty() {
		qs.push("exclude", Some(&excluded_genres.join(",")));
	}

	SearchContext {
		qs,
		status_filter,
		content_rating_filter,
	}
}

pub fn build_search_url(
	ctx: &SearchContext,
	query: Option<&str>,
	page: i32,
	status: Option<&str>,
) -> String {
	let mut qs = ctx.qs.clone();
	if let Some(q) = query {
		qs.push("q", Some(q));
	}
	if let Some(s) = status {
		qs.push("status", Some(s));
	}
	format!("{API_URL}/titles/search?{qs}&page={}", page.max(1))
}

pub fn merge_with_persistent(runtime: Option<Vec<String>>, persistent: Vec<String>) -> Vec<String> {
	match runtime {
		None => persistent,
		Some(mut values) => {
			values.sort();
			values.dedup();
			values
		}
	}
}

pub fn is_server_status(value: &str) -> bool {
	SERVER_STATUS_IDS
		.iter()
		.any(|supported| value.eq_ignore_ascii_case(supported))
}

fn rating_matches(rating: &str, content_rating: aidoku::ContentRating) -> bool {
	matches!(
		(rating, content_rating),
		("safe", aidoku::ContentRating::Safe)
			| ("suggestive", aidoku::ContentRating::Suggestive)
			| ("nsfw", aidoku::ContentRating::NSFW)
	)
}

pub fn filter_by_rating(entries: Vec<Manga>, ratings: &[String]) -> Vec<Manga> {
	if ratings.is_empty() {
		return entries;
	}
	entries
		.into_iter()
		.filter(|m| ratings.iter().any(|r| rating_matches(r, m.content_rating)))
		.collect()
}

pub fn fetch_multi_status(
	ctx: &SearchContext,
	query: Option<&str>,
	page: i32,
	all_statuses: &[String],
	all_ratings: &[String],
	hide: bool,
) -> Result<MangaPageResult> {
	let mut all_entries: Vec<Manga> = Vec::new();
	let mut any_has_next = false;
	let mut first_error: Option<AidokuError> = None;
	let mut success_count = 0usize;

	for status in all_statuses {
		let url = build_search_url(ctx, query, page, Some(status));
		match request::<ListData>(&url) {
			Ok(list_data) => {
				let mut filtered: Vec<Manga> = list_data
					.items
					.into_iter()
					.filter(|item| {
						item.status
							.as_deref()
							.is_some_and(|s| s.eq_ignore_ascii_case(status))
					})
					.map(Manga::from)
					.collect();
				if list_data.pagination.has_next {
					any_has_next = true;
				}
				all_entries.append(&mut filtered);
				success_count += 1;
			}
			Err(e) => {
				if first_error.is_none() {
					first_error = Some(e);
				}
			}
		}
	}

	if success_count == 0 {
		if let Some(e) = first_error {
			return Err(e);
		}
		bail!("all status requests failed");
	}

	if hide {
		crate::mtl::retain_manga(&mut all_entries);
	}
	let mut entries = filter_by_rating(all_entries, all_ratings);
	let mut seen: Vec<String> = Vec::new();
	entries.retain(|m| {
		if seen.contains(&m.key) {
			false
		} else {
			seen.push(m.key.clone());
			true
		}
	});
	Ok(MangaPageResult {
		entries,
		has_next_page: any_has_next,
	})
}

pub fn deep_link_slugs(path: &str) -> (&str, &str) {
	let mut segments = path.split('/').filter(|s| !s.is_empty());
	let series = segments.next().unwrap_or("");
	let chapter = segments.next().unwrap_or("");
	(series, chapter)
}

pub fn chapter_url_matches(url: &str, chapter_slug: &str) -> bool {
	!chapter_slug.is_empty() && url.trim_end_matches('/').rsplit('/').next() == Some(chapter_slug)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::string::ToString;
	use aidoku::{ContentRating, alloc::vec};
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn classifies_list_items_from_adult_flag_and_genre_slugs() {
		let item = |is_adult: bool, genre_slugs: Option<&str>| TitleListItem {
			id: "x".into(),
			name: "Title".into(),
			slug: None,
			cover: None,
			status: None,
			is_mtl: false,
			is_adult,
			genres: Vec::new(),
			genre_slugs: genre_slugs.map(Into::into),
		};
		assert_eq!(
			Manga::from(item(true, None)).content_rating,
			ContentRating::NSFW
		);
		assert_eq!(
			Manga::from(item(false, Some("fantasy,action"))).content_rating,
			ContentRating::Safe
		);
		assert_eq!(
			Manga::from(item(false, Some("yaoi,smut,romance"))).content_rating,
			ContentRating::Suggestive
		);
	}

	#[aidoku_test]
	fn extracts_plain_text_paragraphs() {
		let html = "\n  <div><p> Hello world.</p><p>Second paragraph.</p>\n<div style=\"text-align:center\"><div></div></div></div>";
		assert_eq!(html_to_text(html), "Hello world.\n\nSecond paragraph.");
		assert_eq!(
			html_to_text("<p>Tom &amp; Jerry &mdash; together</p>"),
			"Tom & Jerry — together"
		);
		assert_eq!(
			html_to_text("<p>A <strong>bold</strong> description</p>"),
			"A bold description"
		);
	}

	#[aidoku_test]
	fn falls_back_to_text_without_paragraphs() {
		assert_eq!(
			html_to_text("<div>A summary wrapped in a div.</div>"),
			"A summary wrapped in a div."
		);
		assert_eq!(
			html_to_text("<div><span>Nested without p.</span></div>"),
			"Nested without p."
		);
		assert_eq!(
			html_to_text("<section>A summary wrapped in a section.</section>"),
			"A summary wrapped in a section."
		);
		assert_eq!(html_to_text(""), "");
		// The fallback must not swallow the ad spacer div the site injects.
		assert_eq!(
			html_to_text("<p>Kept.</p><div style=\"text-align:center\"><div></div></div>"),
			"Kept."
		);
	}

	#[aidoku_test]
	fn parses_canonical_id() {
		assert_eq!(
			parse_id_from_canonical("/titles/VYPGVZ8z-shadow-slave"),
			Some("VYPGVZ8z".to_string())
		);
		assert_eq!(parse_id_from_canonical("/titles/garbage"), None);
		assert_eq!(parse_id_from_canonical("/titles/shadow-slave"), None);
		assert_eq!(parse_id_from_canonical("/titles/ab-1234567"), None);
	}

	#[aidoku_test]
	fn parses_chapter_number() {
		assert_eq!(
			parse_chapter_number("Chapter 2995 Time to Return"),
			Some(2995.0)
		);
		assert_eq!(parse_chapter_number("Chapter 12.5: Bonus"), Some(12.5));
		assert_eq!(parse_chapter_number("Chapter 374.5"), Some(374.5));
		assert_eq!(parse_chapter_number("Prologue"), None);
		assert_eq!(
			parse_chapter_number("Chapter: 2234 Darkness Falls"),
			Some(2234.0)
		);
		assert_eq!(
			parse_chapter_number("Chapter '2362 Hunter and Prey"),
			Some(2362.0)
		);
		assert_eq!(parse_chapter_number("Chapter One"), None);
	}

	#[aidoku_test]
	fn parses_timestamps_without_zone_via_fallback() {
		assert!(parse_iso_date("2024-01-02T03:04:05.000Z").is_some());
		assert!(parse_iso_date("2024-01-02T03:04:05").is_some());
	}

	#[aidoku_test]
	fn series_path_accepts_only_catalog_hosts() {
		assert_eq!(
			series_path("https://novelbuddy.me/shadow-slave?x=1"),
			Some("shadow-slave")
		);
		assert_eq!(
			series_path("http://www.novelbuddy.me/shadow-slave/#frag"),
			Some("shadow-slave/")
		);
		assert_eq!(
			series_path("https://NOVELBUDDY.ME:443/shadow-slave"),
			Some("shadow-slave")
		);
		assert_eq!(
			series_path("https://evil.com/novelbuddy.me/shadow-slave"),
			None
		);
		assert_eq!(
			series_path("https://novelbuddy.me.evil.com/shadow-slave"),
			None
		);
		assert_eq!(series_path("novelbuddy.me/shadow-slave"), None);
		assert_eq!(series_path("javascript:novelbuddy.me/shadow-slave"), None);
		assert_eq!(
			series_path(r"https://evil.com\@novelbuddy.me/shadow-slave"),
			None
		);
	}

	#[aidoku_test]
	fn rating_matches_the_matching_tier_only() {
		assert!(rating_matches("safe", ContentRating::Safe));
		assert!(rating_matches("suggestive", ContentRating::Suggestive));
		assert!(rating_matches("nsfw", ContentRating::NSFW));
		assert!(!rating_matches("safe", ContentRating::Suggestive));
		assert!(!rating_matches("nsfw", ContentRating::Safe));
		assert!(!rating_matches("suggestive", ContentRating::NSFW));
	}

	#[aidoku_test]
	fn filter_by_rating_empty_selection_returns_all() {
		let entries = vec![
			Manga {
				key: "1".into(),
				content_rating: ContentRating::Safe,
				..Default::default()
			},
			Manga {
				key: "2".into(),
				content_rating: ContentRating::NSFW,
				..Default::default()
			},
		];
		assert_eq!(filter_by_rating(entries, &[]).len(), 2);
	}

	#[aidoku_test]
	fn filter_by_rating_single_selection() {
		let entries = vec![
			Manga {
				key: "1".into(),
				content_rating: ContentRating::Safe,
				..Default::default()
			},
			Manga {
				key: "2".into(),
				content_rating: ContentRating::Suggestive,
				..Default::default()
			},
		];
		let filtered = filter_by_rating(entries, &["suggestive".into()]);
		assert_eq!(filtered.len(), 1);
		assert_eq!(filtered[0].key, "2");
	}

	#[aidoku_test]
	fn filter_by_rating_suggestive_and_nsfw_selection() {
		let entries = vec![
			Manga {
				key: "1".into(),
				content_rating: ContentRating::Safe,
				..Default::default()
			},
			Manga {
				key: "2".into(),
				content_rating: ContentRating::Suggestive,
				..Default::default()
			},
			Manga {
				key: "3".into(),
				content_rating: ContentRating::NSFW,
				..Default::default()
			},
		];
		let filtered = filter_by_rating(entries, &["suggestive".into(), "nsfw".into()]);
		assert_eq!(filtered.len(), 2);
	}

	#[aidoku_test]
	fn resolve_multi_select_resolves_inclusions_and_exclusions() {
		const ALL: &[&str] = &["safe", "suggestive", "nsfw"];
		assert_eq!(
			resolve_multi_select(ALL, Vec::new(), Vec::new()),
			vec!["safe", "suggestive", "nsfw"]
		);
		assert_eq!(
			resolve_multi_select(ALL, vec!["nsfw".into()], Vec::new()),
			vec!["nsfw"]
		);
		assert_eq!(
			resolve_multi_select(ALL, vec!["safe".into(), "nsfw".into()], vec!["NSFW".into()]),
			vec!["safe"]
		);
		assert_eq!(
			resolve_multi_select(ALL, Vec::new(), ALL.iter().map(|s| (*s).into()).collect()),
			Vec::<String>::new()
		);
	}

	#[aidoku_test]
	fn merge_with_persistent_prefers_the_runtime_selection() {
		assert_eq!(
			merge_with_persistent(None, vec!["safe".into()]),
			vec!["safe"]
		);
		assert_eq!(
			merge_with_persistent(Some(vec!["nsfw".into()]), vec!["safe".into()]),
			vec!["nsfw"]
		);
		assert_eq!(
			merge_with_persistent(Some(Vec::new()), vec!["safe".into()]),
			Vec::<String>::new()
		);
	}

	#[aidoku_test]
	fn server_status_rejects_unsupported_values() {
		assert!(is_server_status("ongoing"));
		assert!(is_server_status("Completed"));
		assert!(!is_server_status("hiatus"));
		assert!(!is_server_status("cancelled"));
	}

	#[aidoku_test]
	fn deep_link_slugs_take_first_two_segments() {
		assert_eq!(
			deep_link_slugs("shadow-slave/chapter-1/extra"),
			("shadow-slave", "chapter-1")
		);
		assert_eq!(deep_link_slugs("shadow-slave"), ("shadow-slave", ""));
		assert_eq!(deep_link_slugs(""), ("", ""));
	}

	#[aidoku_test]
	fn chapter_url_matches_final_segment_only() {
		assert!(chapter_url_matches(
			"https://novelbuddy.me/shadow-slave/chapter-1",
			"chapter-1"
		));
		assert!(chapter_url_matches(
			"https://novelbuddy.me/shadow-slave/chapter-1/",
			"chapter-1"
		));
		assert!(!chapter_url_matches(
			"https://novelbuddy.me/shadow-slave/chapter-10",
			"chapter-1"
		));
		assert!(!chapter_url_matches(
			"https://novelbuddy.me/shadow-slave",
			""
		));
	}
}
