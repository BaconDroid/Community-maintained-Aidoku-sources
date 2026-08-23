use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString},
	imports::{
		html::{Element, Html, Kind},
		net::Request,
		std::parse_date,
	},
	prelude::*,
};
use core::fmt::Write as _;
use serde::de::DeserializeOwned;

use crate::models::{
	ApiResponse, BySlugData, ChapterListData, ChapterListItem, TitleDetail, TitleListItem,
};
use crate::{API_URL, BASE_URL, USER_AGENT};

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
		Manga {
			key: item.id,
			title: item.name,
			cover: item.cover,
			url,
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

pub fn parse_status(value: &str) -> MangaStatus {
	match value.to_ascii_lowercase().as_str() {
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		"cancelled" | "canceled" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn content_rating(is_adult: bool, tags: &[String]) -> ContentRating {
	if is_adult {
		return ContentRating::NSFW;
	}
	for tag in tags {
		match tag.as_str() {
			"Adult" | "Smut" | "Mature" | "Ecchi" | "Lolicon" | "Yaoi" | "Yuri" => {
				return ContentRating::Suggestive;
			}
			_ => {}
		}
	}
	ContentRating::Safe
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

pub fn parse_iso_date(value: &str) -> Option<i64> {
	parse_date(value, "yyyy-MM-dd'T'HH:mm:ss.SSSXXX")
}

pub fn parse_chapter_number(name: &str) -> Option<f32> {
	// Chapter-name formats vary across titles ("Chapter 5", "Chapter: 5",
	// "Chapter ’5", "Chapter 12.5"), but the number is always the first numeric
	// run. Keep a single '.' for decimal (bonus) chapters.
	let mut num = String::new();
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
	if id.len() == 8 && id.chars().all(|c| c.is_ascii_alphanumeric()) {
		Some(id.into())
	} else {
		None
	}
}

/// Whether a class or id value indicates an ad placement. Matching is scoped
/// to whole tokens (`ad`, `ads`) and specific substrings (`ad-`, `-ad`,
/// `sponsor`, `promo`, `placement`, `advert`, `banner`) so ordinary classes
/// such as `read` or `breadcrumb` are never mistaken for ads.
fn is_ad_marker(value: &str) -> bool {
	let value = value.to_ascii_lowercase();
	for token in value.split(|c: char| c.is_whitespace() || c == '-') {
		if token == "ad" || token == "ads" {
			return true;
		}
	}
	value.contains("ad-")
		|| value.contains("-ad")
		|| value.contains("sponsor")
		|| value.contains("promo")
		|| value.contains("placement")
		|| value.contains("advert")
		|| value.contains("banner")
}

/// Remove ad-placement elements before conversion so they do not leak into
/// the chapter body.
fn remove_ad_nodes(doc: &aidoku::imports::html::Document) {
	let mut to_remove = Vec::new();

	if let Some(elements) = doc.select("*") {
		for element in elements {
			let matches = element
				.class_name()
				.as_deref()
				.map(is_ad_marker)
				.unwrap_or(false)
				|| element.id().as_deref().map(is_ad_marker).unwrap_or(false);
			if matches {
				to_remove.push(element);
			}
		}
	}

	for el in to_remove {
		el.remove();
	}
}

/// Convert a description to plain text. Descriptions should not contain
/// Markdown markers, since they are displayed as metadata rather than as a
/// `PageContent` body.
pub fn html_to_text(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(html) else {
		return String::new();
	};
	doc.select("p")
		.map(|els| {
			els.filter_map(|el| {
				let text = el.text()?;
				let trimmed = text.trim();
				(!trimmed.is_empty()).then(|| trimmed.to_string())
			})
			.collect::<Vec<_>>()
			.join("\n\n")
		})
		.unwrap_or_default()
}

fn escape_markdown(text: &str, output: &mut String) {
	for ch in text.chars() {
		match ch {
			'\\' | '`' | '*' | '_' | '~' | '{' | '}' | '[' | ']' | '<' | '>' | '(' | ')' | '#'
			| '+' | '-' | '.' | '!' | '|' => {
				output.push('\\');
				output.push(ch);
			}
			_ => output.push(ch),
		}
	}
}

fn convert_element_to_markdown(element: &Element, output: &mut String) {
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					escape_markdown(&text, output);
				}
			}
			Kind::Element => {
				if let Ok(element) = Element::try_from(node) {
					convert_tag_to_markdown(&element, output);
				}
			}
			_ => {}
		}
	}
}

fn convert_tag_to_markdown(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();
	match tag.as_str() {
		"p" => {
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"br" => output.push_str("  \n"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
			let level = tag.as_bytes()[1] - b'0';
			for _ in 0..level {
				output.push('#');
			}
			output.push(' ');
			convert_element_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"strong" | "b" => {
			let mut inner = String::new();
			convert_element_to_markdown(element, &mut inner);
			let trimmed = inner.trim();
			if !trimmed.is_empty() {
				output.push_str("**");
				output.push_str(trimmed);
				output.push_str("**");
			}
		}
		"em" | "i" => {
			let mut inner = String::new();
			convert_element_to_markdown(element, &mut inner);
			let trimmed = inner.trim();
			if !trimmed.is_empty() {
				output.push('*');
				output.push_str(trimmed);
				output.push('*');
			}
		}
		"u" => {
			let mut inner = String::new();
			convert_element_to_markdown(element, &mut inner);
			let trimmed = inner.trim();
			if !trimmed.is_empty() {
				output.push_str("__");
				output.push_str(trimmed);
				output.push_str("__");
			}
		}
		"s" | "strike" | "del" => {
			let mut inner = String::new();
			convert_element_to_markdown(element, &mut inner);
			let trimmed = inner.trim();
			if !trimmed.is_empty() {
				output.push_str("~~");
				output.push_str(trimmed);
				output.push_str("~~");
			}
		}
		"ul" => {
			for node in element.child_nodes() {
				if let Ok(li) = Element::try_from(node)
					&& li.tag_name().as_deref() == Some("li")
				{
					output.push_str("- ");
					convert_element_to_markdown(&li, output);
					output.push('\n');
				}
			}
			output.push('\n');
		}
		"ol" => {
			let mut index = 1;
			for node in element.child_nodes() {
				if let Ok(li) = Element::try_from(node)
					&& li.tag_name().as_deref() == Some("li")
				{
					let _ = write!(output, "{index}. ");
					convert_element_to_markdown(&li, output);
					output.push('\n');
					index += 1;
				}
			}
			output.push('\n');
		}
		"blockquote" => {
			let mut inner = String::new();
			convert_element_to_markdown(element, &mut inner);
			for line in inner.lines() {
				output.push_str("> ");
				output.push_str(line);
				output.push('\n');
			}
			output.push('\n');
		}
		"div" | "section" | "article" | "span" | "li" => {
			convert_element_to_markdown(element, output);
		}
		_ => convert_element_to_markdown(element, output),
	}
}

/// Convert chapter HTML to Aidoku Markdown while excluding ad placements.
pub fn html_to_markdown(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(html) else {
		return String::new();
	};

	// Remove ad placements before conversion
	remove_ad_nodes(&doc);

	// Convert to markdown by traversing all children of the fragment root,
	// so that lists, blockquotes, and prose in generic containers are
	// preserved rather than silently dropped.
	let mut output = String::new();
	let root = Element::from(doc);
	convert_element_to_markdown(&root, &mut output);
	output.trim().to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::alloc::string::ToString;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn extracts_paragraphs() {
		let html = "\n  <div><p> Hello world.</p><p>Second paragraph.</p>\n<div style=\"text-align:center\"><div></div></div></div>";
		let out = html_to_text(html);
		assert_eq!(out, "Hello world.\n\nSecond paragraph.");
	}

	#[aidoku_test]
	fn decodes_entities() {
		let html = "<p>Tom &amp; Jerry &mdash; together</p>";
		let out = html_to_text(html);
		assert_eq!(out, "Tom & Jerry — together");
	}

	#[aidoku_test]
	fn preserves_inline_markdown_without_tags() {
		let html = "<p>A <strong>bold</strong>, <em>italic</em>, <u>underlined</u>, and <del>gone</del>.</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "A **bold**, *italic*, __underlined__, and ~~gone~~\\.");
	}

	#[aidoku_test]
	fn preserves_breaks_and_headings() {
		let html = "<h2>Chapter 1</h2><p>First<br>second</p><div class=\"ad-placement\"></div><p>Third</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "## Chapter 1\n\nFirst  \nsecond\n\nThird");
	}

	#[aidoku_test]
	fn escapes_literal_markdown_and_decodes_entities() {
		let html = "<p>Tom &amp; Jerry &mdash; use *literal*, _text_, and ~~this~~</p>";
		let out = html_to_markdown(html);
		assert_eq!(
			out,
			"Tom & Jerry — use \\*literal\\*, \\_text\\_, and \\~\\~this\\~\\~"
		);
	}

	#[aidoku_test]
	fn descriptions_remain_plain_text() {
		let html = "<p>A <strong>bold</strong> description</p>";
		assert_eq!(html_to_text(html), "A bold description");
	}

	#[aidoku_test]
	fn parses_canonical_id() {
		assert_eq!(
			parse_id_from_canonical("/titles/VYPGVZ8z-shadow-slave"),
			Some("VYPGVZ8z".to_string())
		);
		assert_eq!(parse_id_from_canonical("/titles/garbage"), None);
	}

	#[aidoku_test]
	fn parses_chapter_number() {
		assert_eq!(
			parse_chapter_number("Chapter 2995 Time to Return"),
			Some(2995.0)
		);
		assert_eq!(parse_chapter_number("Chapter 12.5: Bonus"), Some(12.5));
		// Verified real on the live API (rare but present) — do not drop decimal
		// handling: the slug for this is `chapter-374-5` (ambiguous), the name is not.
		assert_eq!(parse_chapter_number("Chapter 374.5"), Some(374.5));
		assert_eq!(parse_chapter_number("Prologue"), None);
		// Live API also returns these formats (verified on the Shadow Slave list):
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
	fn excludes_ad_placements() {
		let html = "<div class=\"ad-placement\"><p>Sponsored text</p></div><p>Real content</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "Real content");
	}

	#[aidoku_test]
	fn renders_text_in_div() {
		let html = "<div>text in div</div>";
		let out = html_to_markdown(html);
		assert_eq!(out, "text in div");
	}

	#[aidoku_test]
	fn renders_unordered_list() {
		let html = "<ul><li>a</li><li>b</li></ul>";
		let out = html_to_markdown(html);
		assert_eq!(out, "- a\n- b");
	}

	#[aidoku_test]
	fn renders_blockquote() {
		let html = "<blockquote>cite</blockquote>";
		let out = html_to_markdown(html);
		assert_eq!(out, "> cite");
	}

	#[aidoku_test]
	fn trims_inline_whitespace() {
		let html = "<p><strong> bold </strong> and <em> italic </em></p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "**bold** and *italic*");
	}
}
