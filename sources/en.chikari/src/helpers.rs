use crate::models::{ChapterItem, ChapterListResponse, NovelDetail, NovelListItem};
use crate::{BASE_URL, USER_AGENT};
use aidoku::{
	Chapter, ContentRating, FilterValue, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::parse_date},
	prelude::*,
};
use serde::de::DeserializeOwned;

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", "application/json")
		.header("Referer", BASE_URL)
		.header("Origin", BASE_URL)
		.json_owned::<T>()
}

pub fn list_url(
	sort: &str,
	limit: u32,
	offset: u32,
	query: Option<&str>,
	filters: &[FilterValue],
) -> String {
	let mut qs = QueryParameters::new();
	qs.push("sort", Some(sort));
	qs.push("limit", Some(&limit.to_string()));
	qs.push("offset", Some(&offset.to_string()));
	if let Some(query) = query {
		qs.push("q", Some(query));
	}
	for filter in filters {
		if let FilterValue::MultiSelect {
			id,
			included,
			excluded,
		} = filter && id == "genres"
		{
			for genre in included {
				qs.push("genre", Some(genre));
			}
			for genre in excluded {
				qs.push("genre_exclude", Some(genre));
			}
		}
	}
	format!("{BASE_URL}/api/novels?{qs}")
}

pub fn has_next(len: usize, total: u32, offset: u32, limit: u32) -> bool {
	if total > 0 {
		offset.saturating_add(len as u32) < total
	} else {
		len >= limit as usize
	}
}

pub fn manga_from_list(item: NovelListItem) -> Manga {
	let slug = item.slug;
	let url = Some(format!("{BASE_URL}/series/{slug}"));
	Manga {
		key: slug,
		title: item.title,
		cover: item.cover_url,
		url,
		content_rating: if item.is_nsfw {
			ContentRating::NSFW
		} else {
			ContentRating::Unknown
		},
		status: item
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or(MangaStatus::Unknown),
		..Default::default()
	}
}

pub fn manga_from_detail(detail: NovelDetail) -> Manga {
	let slug = detail.slug;
	let url = Some(format!("{BASE_URL}/series/{slug}"));
	let content_rating = content_rating(detail.is_nsfw, &detail.genres, &detail.tags);
	let mut tags = Vec::new();
	for name in detail.genres.into_iter().map(|g| g.name).chain(
		detail
			.tags
			.into_iter()
			.filter(|t| !t.is_spoiler)
			.map(|t| t.name),
	) {
		if !tags.iter().any(|tag| tag == &name) {
			tags.push(name);
		}
	}
	let authors: Vec<String> = detail.authors.into_iter().map(|a| a.name).collect();
	Manga {
		key: slug,
		title: detail.title,
		cover: detail.cover_url,
		url,
		description: detail.description.filter(|s| !s.trim().is_empty()),
		authors: (!authors.is_empty()).then_some(authors),
		tags: (!tags.is_empty()).then_some(tags),
		status: detail
			.status
			.as_deref()
			.map(parse_status)
			.unwrap_or(MangaStatus::Unknown),
		content_rating,
		..Default::default()
	}
}

pub fn content_rating(
	is_nsfw: bool,
	genres: &[crate::models::Named],
	tags: &[crate::models::Tag],
) -> ContentRating {
	if is_nsfw {
		return ContentRating::NSFW;
	}
	let is_mature = genres.iter().any(|genre| is_mature_signal(&genre.name))
		|| tags.iter().any(|tag| is_mature_signal(&tag.name));
	if is_mature {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn is_mature_signal(value: &str) -> bool {
	value.eq_ignore_ascii_case("mature")
		|| value.eq_ignore_ascii_case("r-18")
		|| value.eq_ignore_ascii_case("r18")
		|| value.eq_ignore_ascii_case("adult")
		|| value.eq_ignore_ascii_case("ecchi")
		|| value.eq_ignore_ascii_case("smut")
		|| value.eq_ignore_ascii_case("yaoi")
		|| value.eq_ignore_ascii_case("yuri")
}

pub fn parse_status(status: &str) -> MangaStatus {
	match status.to_ascii_lowercase().as_str() {
		"releasing" | "ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "canceled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}
pub fn chapter_key(number: f32) -> String {
	number.to_string()
}
pub fn chapter_from_item(item: ChapterItem) -> Chapter {
	let key = chapter_key(item.number);
	Chapter {
		key,
		title: Some(
			item.title
				.filter(|s| !s.trim().is_empty())
				.unwrap_or_else(|| format!("Chapter {}", item.number)),
		),
		chapter_number: Some(item.number),
		date_uploaded: item.created_at.as_deref().and_then(parse_iso_date),
		..Default::default()
	}
}
pub fn fetch_chapters(slug: &str) -> Result<Vec<Chapter>> {
	let mut result = Vec::new();
	let mut offset = 0u32;
	let limit = 500u32;
	loop {
		let data: ChapterListResponse = request(&format!(
			"{BASE_URL}/api/novels/{slug}/chapters?order=asc&limit={limit}&offset={offset}"
		))?;
		let count = data.items.len();
		if count == 0 {
			break;
		}
		let total = data.total;
		let response_offset = if data.offset == 0 {
			offset
		} else {
			data.offset
		};
		result.extend(data.items.into_iter().map(chapter_from_item));
		if !has_next(count, total, response_offset, data.limit.max(limit)) {
			break;
		}
		let next_offset = response_offset.saturating_add(count as u32);
		if next_offset <= offset {
			break;
		}
		offset = next_offset;
	}
	result.reverse();
	Ok(result)
}
pub fn parse_iso_date(value: &str) -> Option<i64> {
	parse_date(value.get(..19)?, "yyyy-MM-ddTHH:mm:ss")
		.or_else(|| parse_date(value.get(..10)?, "yyyy-MM-dd"))
}
pub fn body_to_text(body: String) -> Result<String> {
	let text = body
		.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty())
		.collect::<Vec<_>>()
		.join("\n\n");
	if text.is_empty() {
		bail!("Chikari returned an empty chapter")
	}
	Ok(text)
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::vec;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn mature_rating_signals_are_suggestive() {
		let genres = vec![
			crate::models::Named {
				name: "Mature".into(),
			},
			crate::models::Named {
				name: "R-18".into(),
			},
		];
		assert_eq!(
			content_rating(false, &genres, &[]),
			ContentRating::Suggestive
		);
		assert_eq!(content_rating(true, &[], &[]), ContentRating::NSFW);
	}

	#[aidoku_test]
	fn parses_chikari_timestamp() {
		assert!(parse_iso_date("2026-02-21T22:08:14.600092+00:00").is_some());
	}

	#[aidoku_test]
	fn list_url_retains_repeated_filters() {
		let filters = [FilterValue::MultiSelect {
			id: "genres".into(),
			included: vec!["action".into(), "fantasy".into()],
			excluded: vec!["mature".into(), "yuri".into()],
		}];
		let url = list_url("popular", 36, 0, Some("shadow slave"), &filters);
		assert!(url.contains("q=shadow"));
		assert_eq!(url.matches("genre=").count(), 2);
		assert_eq!(url.matches("genre_exclude=").count(), 2);
		assert!(url.contains("genre=action"));
		assert!(url.contains("genre=fantasy"));
		assert!(url.contains("genre_exclude=mature"));
		assert!(url.contains("genre_exclude=yuri"));
	}
}
