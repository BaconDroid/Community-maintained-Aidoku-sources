use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::net::Request,
	prelude::*,
};
use serde::de::DeserializeOwned;

use crate::models::Novel;
use crate::{BASE_URL, USER_AGENT};

pub fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "en-US,en;q=0.9")
		.json_owned::<T>()
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

pub fn split_genres(genres: &str) -> Vec<String> {
	let mut tags: Vec<String> = Vec::new();
	for genre in genres.split(',') {
		let genre = genre.trim();
		if !genre.is_empty() && !tags.iter().any(|tag| tag.eq_ignore_ascii_case(genre)) {
			tags.push(genre.into());
		}
	}
	tags
}

pub fn parse_status(value: &str) -> MangaStatus {
	match value.trim().to_ascii_lowercase().as_str() {
		"ongoing" | "releasing" => MangaStatus::Ongoing,
		"completed" | "complete" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "canceled" | "dropped" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn content_rating(tags: &[String]) -> ContentRating {
	if tags.iter().any(|tag| is_explicit_signal(tag)) {
		ContentRating::NSFW
	} else if tags.iter().any(|tag| is_suggestive_signal(tag)) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn is_explicit_signal(value: &str) -> bool {
	let value = value.trim();
	value.eq_ignore_ascii_case("adult")
		|| value.eq_ignore_ascii_case("mature")
		|| value.eq_ignore_ascii_case("smut")
		|| value.eq_ignore_ascii_case("erotica")
		|| value.eq_ignore_ascii_case("explicit")
		|| value.eq_ignore_ascii_case("pornographic")
		|| value.eq_ignore_ascii_case("r-18")
		|| value.eq_ignore_ascii_case("r18")
}

fn is_suggestive_signal(value: &str) -> bool {
	let value = value.trim();
	value.eq_ignore_ascii_case("ecchi")
		|| value.eq_ignore_ascii_case("mature themes")
		|| value.eq_ignore_ascii_case("suggestive")
}

impl From<Novel> for Manga {
	fn from(novel: Novel) -> Self {
		let url = Some(format!("{BASE_URL}/novel?id={}", novel.id));
		let tags = split_genres(novel.genres.as_deref().unwrap_or(""));
		let status = parse_status(
			novel
				.release_status
				.as_deref()
				.or(novel.ongoing.as_deref())
				.unwrap_or(""),
		);
		let cover = novel
			.cover_url
			.or(novel.image_url)
			.or(novel.novel_image)
			.map(|path| absolute_url(&path));
		let author = novel.author.filter(|author| !author.trim().is_empty());
		let description = novel
			.description
			.filter(|description| !description.trim().is_empty());
		let rating = content_rating(&tags);
		Manga {
			key: novel.id,
			title: novel.title,
			cover,
			authors: author.map(|author| vec![author]),
			description,
			url,
			tags: (!tags.is_empty()).then_some(tags),
			status,
			content_rating: rating,
			..Default::default()
		}
	}
}

pub fn build_chapters(novel_id: &str, names: Vec<String>) -> Vec<Chapter> {
	names
		.into_iter()
		.enumerate()
		.map(|(index, name)| {
			let number = index + 1;
			Chapter {
				key: number.to_string(),
				title: Some(name),
				chapter_number: Some(number as f32),
				url: Some(format!(
					"{BASE_URL}/reader?novel={novel_id}&chapter={number}"
				)),
				..Default::default()
			}
		})
		.collect()
}

pub fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
	query.split('&').find_map(|pair| {
		let (name, value) = pair.split_once('=')?;
		(name == key).then_some(value)
	})
}

#[cfg(test)]
mod tests {
	use aidoku::alloc::string::ToString;
	use aidoku_test::aidoku_test;

	use super::*;

	#[aidoku_test]
	fn splits_comma_separated_genres() {
		assert_eq!(
			split_genres("Action, Fantasy, Mystery"),
			vec![
				"Action".to_string(),
				"Fantasy".to_string(),
				"Mystery".to_string()
			]
		);
		assert_eq!(split_genres(""), Vec::<String>::new());
		assert_eq!(split_genres("Action, , Action"), vec!["Action".to_string()]);
	}

	#[aidoku_test]
	fn parses_status_values() {
		assert!(matches!(parse_status("ongoing"), MangaStatus::Ongoing));
		assert!(matches!(parse_status("Completed"), MangaStatus::Completed));
		assert!(matches!(parse_status("hiatus"), MangaStatus::Hiatus));
		assert!(matches!(parse_status("dropped"), MangaStatus::Cancelled));
		assert!(matches!(parse_status("unknown"), MangaStatus::Unknown));
	}

	#[aidoku_test]
	fn extracts_query_parameters() {
		assert_eq!(query_param("novel=abc&chapter=12", "chapter"), Some("12"));
		assert_eq!(query_param("novel=abc", "chapter"), None);
	}

	#[aidoku_test]
	fn marks_mature_genres_by_strength() {
		assert!(matches!(
			content_rating(&["Action".to_string(), "Adult".to_string()]),
			ContentRating::NSFW
		));
		assert!(matches!(
			content_rating(&["Mature".to_string()]),
			ContentRating::NSFW
		));
		assert!(matches!(
			content_rating(&["Ecchi".to_string()]),
			ContentRating::Suggestive
		));
		assert!(matches!(
			content_rating(&["Action".to_string()]),
			ContentRating::Safe
		));
	}
}
