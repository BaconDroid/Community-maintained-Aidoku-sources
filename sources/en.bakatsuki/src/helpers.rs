use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::{QueryParameters, decode_uri},
	imports::{
		defaults::{defaults_get, defaults_set_data},
		html::Html,
		net::Request,
		std::current_date,
	},
	prelude::*,
};
use serde::{Deserialize, Serialize};

use crate::models::{CatalogueEntry, MWResponse};

pub const BASE_URL: &str = "https://www.baka-tsuki.org/project";
pub const API_URL: &str = "https://www.baka-tsuki.org/project/api.php";
pub const ORIGIN: &str = "https://www.baka-tsuki.org";

// ── MediaWiki API ─────────────────────────────────────────────────

pub fn mw_query(params: &[(&str, &str)]) -> Result<MWResponse> {
	let mut query = QueryParameters::new();
	for (key, value) in params {
		query.push(key, Some(value));
	}
	let url = format!("{API_URL}?{query}");

	let resp: MWResponse = Request::get(&url)?
		.header("User-Agent", "Mozilla/5.0 (compatible; Aidoku)")
		.json_owned()?;

	if let Some(err) = &resp.error {
		let info = err.info.as_deref().unwrap_or("unknown");
		bail!("MediaWiki API error: {info}");
	}

	Ok(resp)
}

// ── URL helpers ───────────────────────────────────────────────────

pub fn absolute_url(path: &str) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		path.into()
	} else if path.starts_with("//") {
		format!("https:{path}")
	} else if path.starts_with('/') {
		format!("{ORIGIN}{path}")
	} else {
		format!("{BASE_URL}/{path}")
	}
}

// ── HTML → text ───────────────────────────────────────────────────

/// Extract paragraph text from MediaWiki HTML (strips all tags).
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

// ── Catalogue ─────────────────────────────────────────────────────

/// Build the full English LN catalogue by paginating categorymembers for the
/// catalogue and each status category.
pub fn build_catalogue() -> Result<Vec<CatalogueEntry>> {
	const CACHE_KEY: &str = "en.bakatsuki.catalogue.v1";
	const CACHE_TTL: i64 = 60 * 60;

	let now = current_date();
	if let Some(cache) = defaults_get::<CatalogueCache>(CACHE_KEY)
		&& now >= cache.fetched_at
		&& now - cache.fetched_at < CACHE_TTL
	{
		return Ok(cache.entries);
	}

	// 1. Fetch all page titles in "Category:Light novel (English)"
	let mut titles: Vec<String> = Vec::new();
	let mut cmcontinue: Option<String> = None;

	loop {
		let mut params: Vec<(&str, &str)> = vec![
			("action", "query"),
			("list", "categorymembers"),
			("cmtitle", "Category:Light novel (English)"),
			("cmnamespace", "0"),
			("cmtype", "page"),
			("cmlimit", "500"),
			("format", "json"),
			("formatversion", "2"),
		];
		if let Some(ref tok) = cmcontinue {
			params.push(("cmcontinue", tok));
		}
		let resp = mw_query(&params)?;

		if let Some(q) = &resp.query
			&& let Some(members) = &q.categorymembers
		{
			for m in members {
				// Filter out chapter sub-pages (contain ":")
				if !m.title.contains(':') {
					titles.push(m.title.clone());
				}
			}
		}

		cmcontinue = resp.r#continue.as_ref().and_then(|c| c.cmcontinue.clone());
		if cmcontinue.is_none() {
			break;
		}
	}

	// 2. Fetch status categories
	let status_cats: &[(&str, &str)] = &[
		("Category:Active Projects", "ongoing"),
		("Category:Completed Project", "completed"),
		("Category:Hosted Projects", "ongoing"),
		("Category:Stalled Projects", "hiatus"),
		("Category:Inactive Projects", "cancelled"),
	];

	// status_entries: Vec of (title, status) — linear scan is fine (< 2000 entries)
	let mut status_entries: Vec<(String, String)> = Vec::new();

	for (cat, status) in status_cats {
		let mut cat_cont: Option<String> = None;
		loop {
			let mut params: Vec<(&str, &str)> = vec![
				("action", "query"),
				("list", "categorymembers"),
				("cmtitle", cat),
				("cmnamespace", "0"),
				("cmtype", "page"),
				("cmlimit", "500"),
				("format", "json"),
				("formatversion", "2"),
			];
			if let Some(ref tok) = cat_cont {
				params.push(("cmcontinue", tok));
			}
			let resp = mw_query(&params)?;

			if let Some(q) = &resp.query
				&& let Some(members) = &q.categorymembers
			{
				for m in members {
					status_entries.push((m.title.clone(), status.to_string()));
				}
			}

			cat_cont = resp.r#continue.as_ref().and_then(|c| c.cmcontinue.clone());
			if cat_cont.is_none() {
				break;
			}
		}
	}

	let catalogue = titles
		.into_iter()
		.map(|title| {
			let status = status_entries
				.iter()
				.find(|(t, _)| t == &title)
				.map(|(_, s)| s.clone())
				.unwrap_or_default();
			CatalogueEntry { title, status }
		})
		.collect();

	let cache = CatalogueCache {
		fetched_at: current_date(),
		entries: catalogue,
	};
	defaults_set_data(CACHE_KEY, &cache);
	Ok(cache.entries)
}

#[derive(Deserialize, Serialize)]
struct CatalogueCache {
	fetched_at: i64,
	entries: Vec<CatalogueEntry>,
}

// ── Novel details ─────────────────────────────────────────────────

pub struct NovelDetail {
	pub title: String,
	pub cover: Option<String>,
	pub description: Option<String>,
	pub authors: Option<Vec<String>>,
	pub tags: Option<Vec<String>>,
	pub status: MangaStatus,
	pub content_rating: ContentRating,
}

pub fn fetch_novel_details(title: &str) -> Result<NovelDetail> {
	let params = [
		("action", "query"),
		("titles", title),
		("prop", "extracts|pageimages|categories"),
		("explaintext", "1"),
		("exsectionformat", "raw"),
		("piprop", "thumbnail"),
		("pithumbsize", "400"),
		("cllimit", "500"),
		("clshow", "!hidden"),
		("redirects", "1"),
		("format", "json"),
		("formatversion", "2"),
	];
	let resp = mw_query(&params)?;

	let page = resp
		.query
		.as_ref()
		.and_then(|q| q.pages.as_ref())
		.and_then(|ps| ps.first())
		.ok_or_else(|| error!("No page found"))?;

	if page.missing.is_some() {
		bail!("Page not found: {title}");
	}

	let display_title = page.title.clone().unwrap_or_else(|| title.into());
	let cover = page.thumbnail.as_ref().and_then(|t| t.source.clone());

	// Summary: use the extract
	let description = page.extract.as_deref().and_then(extract_summary);

	// Genres, author, status from categories
	let categories: Vec<String> = page
		.categories
		.as_ref()
		.map(|cs| cs.iter().filter_map(|c| c.title.clone()).collect())
		.unwrap_or_default();

	let genres: Vec<String> = categories.iter().filter_map(|c| parse_genre(c)).collect();
	let author = parse_author(&categories);
	let status = parse_status_from_categories(&categories);

	// Content rating: check for adult/mature genres
	let content_rating = if genres.iter().any(|g| {
		let l = g.to_ascii_lowercase();
		l == "adult" || l == "smut" || l == "mature" || l == "ecchi"
	}) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	};

	Ok(NovelDetail {
		title: display_title,
		cover,
		description,
		authors: author.map(|a| vec![a]),
		tags: if genres.is_empty() {
			None
		} else {
			Some(genres)
		},
		status,
		content_rating,
	})
}

// ── Chapter list ──────────────────────────────────────────────────

pub fn fetch_chapter_list(novel_title: &str) -> Result<Vec<Chapter>> {
	let params = [
		("action", "parse"),
		("page", novel_title),
		("prop", "text"),
		("disableeditsection", "1"),
		("disabletoc", "1"),
		("redirects", "1"),
		("format", "json"),
		("formatversion", "2"),
	];
	let resp = mw_query(&params)?;

	let html = resp
		.parse
		.as_ref()
		.and_then(|p| p.text.as_deref())
		.unwrap_or("");

	let mut chapters = parse_chapter_links(html, novel_title);

	// Batch-fetch release dates for all chapters
	let page_titles: Vec<String> = chapters.iter().map(|ch| ch.key.clone()).collect();
	let dates = fetch_chapter_dates(&page_titles);
	for ch in &mut chapters {
		if let Some((_, ts)) = dates.iter().find(|(t, _)| t == &ch.key) {
			ch.date_uploaded = Some(*ts);
		}
	}

	Ok(chapters)
}

/// Batch-fetch revision timestamps for chapter pages (up to 50 per call).
fn fetch_chapter_dates(titles: &[String]) -> Vec<(String, i64)> {
	let mut dates: Vec<(String, i64)> = Vec::new();
	// MediaWiki allows up to 50 titles per query
	for chunk in titles.chunks(50) {
		let joined = chunk
			.iter()
			.map(|s| s.as_str())
			.collect::<Vec<_>>()
			.join("|");
		let params = [
			("action", "query"),
			("prop", "revisions"),
			("titles", &joined),
			("rvprop", "timestamp"),
			("format", "json"),
			("formatversion", "2"),
		];
		let Ok(resp) = mw_query(&params) else {
			continue;
		};
		if let Some(q) = &resp.query
			&& let Some(pages) = &q.pages
		{
			for page in pages {
				let ts = page
					.revisions
					.as_ref()
					.and_then(|rv| rv.first())
					.and_then(|r| r.timestamp.as_deref())
					.and_then(parse_mw_timestamp);
				if let (Some(title), Some(ts)) = (&page.title, ts) {
					dates.push((title.clone(), ts));
				}
			}
		}
	}
	dates
}

/// Normalize a MediaWiki title from either a URL or API link.
pub fn normalize_title(value: &str) -> String {
	decode_uri(value).replace('_', " ")
}

/// Parse MediaWiki timestamp "2024-01-15T12:34:56Z" to Unix seconds.
fn parse_mw_timestamp(ts: &str) -> Option<i64> {
	// Simple parse: "YYYY-MM-DDTHH:MM:SSZ"
	let b = ts.as_bytes();
	if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[19] != b'Z' {
		return None;
	}
	let year: i64 = ts[0..4].parse().ok()?;
	let month: i64 = ts[5..7].parse().ok()?;
	let day: i64 = ts[8..10].parse().ok()?;
	let hour: i64 = ts[11..13].parse().ok()?;
	let min: i64 = ts[14..16].parse().ok()?;
	let sec: i64 = ts[17..19].parse().ok()?;
	// Count leap years using the proleptic Gregorian calendar, including the
	// century exception and years before the Unix epoch.
	let leap_years_before = |y: i64| {
		let y = y - 1;
		y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400)
	};
	let mut days = (year - 1970) * 365 + leap_years_before(year) - leap_years_before(1970);
	let month_days: &[i64] = &[0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
	if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 59 {
		return None;
	}
	days += month_days[(month - 1) as usize];
	if month > 2 && (year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)) {
		days += 1;
	}
	days += day - 1;
	Some((days * 86400) + hour * 3600 + min * 60 + sec)
}

fn parse_chapter_links(html: &str, novel_title: &str) -> Vec<Chapter> {
	let Ok(doc) = Html::parse_fragment(html) else {
		return vec![];
	};

	let title_lower = novel_title.to_ascii_lowercase();
	// Vec-based dedup — chapter lists are < 200 entries
	let mut seen: Vec<String> = Vec::new();
	let mut chapters = Vec::new();

	if let Some(links) = doc.select("a") {
		for a in links {
			let href = match a.attr("href") {
				Some(h) => h,
				None => continue,
			};

			// Skip external links, file downloads, edit links
			if href.starts_with("http://") || href.starts_with("https://") {
				continue;
			}
			if href.contains("redlink=1") {
				continue;
			}
			if href.ends_with(".pdf") || href.ends_with(".epub") || href.ends_with(".zip") {
				continue;
			}

			// Must start with the novel title (case-insensitive)
			// Hrefs look like "/project/index.php?title=Apocalypse_Witch:Volume1_Chapter1"
			// Extract the "title" param value, then normalize underscores to spaces
			let page_title = href
				.split("title=")
				.nth(1)
				.and_then(|s| s.split('&').next())
				.unwrap_or("");
			let decoded = normalize_title(page_title);
			let decoded_lower = decoded.to_ascii_lowercase();
			if !decoded_lower.starts_with(&title_lower) {
				continue;
			}

			// Must contain ":" after the novel title (chapter delimiter)
			let suffix = &decoded[novel_title.len()..];
			if !suffix.starts_with(':') {
				continue;
			}

			// Skip scaffolding pages
			let suffix_lower = suffix[1..].to_ascii_lowercase();
			if suffix_lower.contains("registration")
				|| suffix_lower.contains("staff")
				|| suffix_lower.contains("guidelines")
			{
				continue;
			}

			let ch_title = decoded;
			if seen.contains(&ch_title) {
				continue;
			}
			seen.push(ch_title.clone());

			let chapter_number = parse_chapter_number(&ch_title);
			let volume_number = parse_volume_number(&ch_title);
			let url = absolute_url(&href);

			chapters.push(Chapter {
				key: ch_title.clone(),
				title: Some(ch_title),
				chapter_number,
				volume_number,
				url: Some(url),
				..Default::default()
			});
		}
	}

	// Sort by volume, then by chapter number, preserving document order on ties
	let mut indexed: Vec<(usize, Chapter)> = chapters.into_iter().enumerate().collect();
	indexed.sort_by(|(idx_a, a), (idx_b, b)| {
		let vol_a = a.volume_number.unwrap_or(f32::MAX);
		let vol_b = b.volume_number.unwrap_or(f32::MAX);
		vol_a.total_cmp(&vol_b).then_with(|| {
			let ch_a = a.chapter_number.unwrap_or(0.0);
			let ch_b = b.chapter_number.unwrap_or(0.0);
			ch_a.partial_cmp(&ch_b)
				.unwrap_or(core::cmp::Ordering::Equal)
				.then_with(|| idx_a.cmp(idx_b))
		})
	});
	let chapters: Vec<Chapter> = indexed.into_iter().map(|(_, c)| c).collect();

	chapters
}

// ── Chapter content ───────────────────────────────────────────────

pub fn fetch_chapter_content(page_title: &str) -> Result<String> {
	let params = [
		("action", "parse"),
		("page", page_title),
		("prop", "text"),
		("disableeditsection", "1"),
		("disabletoc", "1"),
		("redirects", "1"),
		("format", "json"),
		("formatversion", "2"),
	];
	let resp = mw_query(&params)?;

	let html = resp
		.parse
		.as_ref()
		.and_then(|p| p.text.as_deref())
		.unwrap_or("");

	let text = html_to_text(html);
	if text.is_empty() {
		Ok("(This chapter appears to be empty.)".into())
	} else {
		Ok(text)
	}
}

// ── Recent changes ────────────────────────────────────────────────

pub fn fetch_recent_changes() -> Result<Vec<Manga>> {
	let params = [
		("action", "query"),
		("list", "recentchanges"),
		("rcnamespace", "0"),
		("rclimit", "500"),
		("rctype", "edit|new"),
		("rcprop", "title"),
		("format", "json"),
		("formatversion", "2"),
	];
	let resp = mw_query(&params)?;

	// Vec-based dedup
	let mut seen: Vec<String> = Vec::new();
	let mut projects = Vec::new();

	if let Some(q) = &resp.query
		&& let Some(changes) = &q.recentchanges
	{
		for rc in changes {
			// Chapter titles are "Novel Title:Volume X Chapter Y"
			// The project is everything before the first ":"
			let project = rc.title.split(':').next().unwrap_or(&rc.title).trim();
			if !project.is_empty() && !seen.contains(&project.to_string()) {
				seen.push(project.to_string());
				projects.push(project.to_string());
			}
		}
	}

	Ok(projects
		.into_iter()
		.map(|title| Manga {
			key: title.clone(),
			title,
			..Default::default()
		})
		.collect())
}

// ── Genre / Author / Status parsing ───────────────────────────────

const GENRE_PREFIX: &str = "Category:Genre - ";

pub fn parse_genre(category_title: &str) -> Option<String> {
	category_title
		.strip_prefix(GENRE_PREFIX)
		.map(|g| g.trim().to_string())
}

const STATUS_CATEGORIES: &[(&str, MangaStatus)] = &[
	("Category:Active Projects", MangaStatus::Ongoing),
	("Category:Completed Project", MangaStatus::Completed),
	("Category:Hosted Projects", MangaStatus::Ongoing),
	("Category:Stalled Projects", MangaStatus::Hiatus),
	("Category:Inactive Projects", MangaStatus::Cancelled),
];

pub fn parse_status_from_categories(categories: &[String]) -> MangaStatus {
	for cat in categories {
		for (prefix, status) in STATUS_CATEGORIES {
			if cat == *prefix {
				return *status;
			}
		}
	}
	MangaStatus::Unknown
}

pub fn parse_author(categories: &[String]) -> Option<String> {
	for cat in categories {
		let title = cat.strip_prefix("Category:").unwrap_or(cat);
		if title.starts_with("Genre - ") || title.starts_with("Genre-") {
			continue;
		}
		if is_structural_category(title) {
			continue;
		}
		if title.chars().any(|c| c.is_ascii_digit()) {
			continue;
		}
		if title.split_whitespace().count() >= 2 {
			return Some(title.to_string());
		}
	}
	None
}

fn is_structural_category(title: &str) -> bool {
	let lower = title.to_ascii_lowercase();
	let prefixes = [
		"light novel",
		"active project",
		"completed project",
		"stalled project",
		"inactive project",
		"hosted project",
		"all pages needing",
		"pages with",
		"articles with",
		"commons category",
		"project",
		"stub",
		"navbox",
		"template",
		"maintenance",
		"wikipedia",
		"wikidata",
		"good article",
		"featured article",
		"orphan",
		"unassessed",
		"underlinked",
		"cleanup",
	];
	// Publisher imprints appear anywhere in the category name (e.g. "MF Bunko J")
	let is_imprint = lower.contains("bunko") || lower.contains("shobo");
	prefixes.iter().any(|p| lower.starts_with(p)) || is_imprint
}

// ── Summary extraction ────────────────────────────────────────────

pub fn extract_summary(text: &str) -> Option<String> {
	let lines: Vec<&str> = text.lines().collect();
	let mut in_synopsis = false;
	let mut result = Vec::new();

	for line in &lines {
		let trimmed = line.trim();

		if trimmed.starts_with('=') && trimmed.ends_with('=') {
			let heading = trimmed
				.trim_start_matches('=')
				.trim_end_matches('=')
				.trim()
				.to_ascii_lowercase();
			if heading.contains("synopsis")
				|| heading.contains("summary")
				|| heading.contains("plot")
				|| heading.contains("description")
			{
				in_synopsis = true;
				continue;
			} else if in_synopsis {
				break;
			}
		}

		if in_synopsis && !trimmed.is_empty() {
			result.push(trimmed);
		}
	}

	// Fallback: lead section before first heading
	if result.is_empty() {
		for line in &lines {
			let trimmed = line.trim();
			if trimmed.starts_with('=') {
				break;
			}
			if !trimmed.is_empty() && trimmed.len() > 40 {
				result.push(trimmed);
			}
		}
	}

	let summary = result.join(" ");
	if summary.chars().count() > 1500 {
		let truncated: String = summary.chars().take(1500).collect();
		Some(format!("{truncated}..."))
	} else if summary.is_empty() {
		None
	} else {
		Some(summary)
	}
}

// ── Chapter / volume number parsing ───────────────────────────────

pub fn parse_chapter_number(name: &str) -> Option<f32> {
	let lower = name.to_ascii_lowercase();
	// Look for "chapter" or "ch." or "ch " followed by digits
	let idx = lower.find("chapter").or_else(|| lower.find("ch."))?;
	let after = &lower[idx..];
	let skip = if after.starts_with("chapter") { 7 } else { 3 };
	let suffix = &lower[idx + skip..];
	let num_str: String = suffix
		.trim_start()
		.chars()
		.take_while(|c| c.is_ascii_digit() || *c == '.')
		.collect();
	num_str.parse().ok()
}

pub fn parse_volume_number(name: &str) -> Option<f32> {
	let lower = name.to_ascii_lowercase();
	let (idx, prefix_len) = lower
		.find("volume")
		.map(|idx| (idx, 6))
		.or_else(|| lower.find("vol.").map(|idx| (idx, 4)))?;
	let after = lower[idx + prefix_len..].trim_start();
	let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
	let val: i32 = num_str.parse().ok()?;
	Some(val as f32)
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[test]
	fn test_absolute_url() {
		assert_eq!(
			absolute_url("/images/test.png"),
			"https://www.baka-tsuki.org/images/test.png"
		);
		assert_eq!(
			absolute_url("//cdn.example.com/img.png"),
			"https://cdn.example.com/img.png"
		);
		assert_eq!(
			absolute_url("https://example.com/img.png"),
			"https://example.com/img.png"
		);
	}

	#[test]
	fn test_parse_genre() {
		assert_eq!(
			parse_genre("Category:Genre - Action"),
			Some("Action".into())
		);
		assert_eq!(parse_genre("Category:Other"), None);
	}

	#[test]
	fn test_parse_chapter_number() {
		assert_eq!(parse_chapter_number("Chapter 5"), Some(5.0));
		assert_eq!(parse_chapter_number("Chapter 12.5 Bonus"), Some(12.5));
		assert_eq!(parse_chapter_number("Prologue"), None);
	}

	#[test]
	fn test_parse_volume_number() {
		assert_eq!(parse_volume_number("Volume 3 Chapter 1"), Some(3.0));
		assert_eq!(parse_volume_number("Vol.5 Extra"), Some(5.0));
		assert_eq!(parse_volume_number("Vol. 5 Extra"), Some(5.0));
		assert_eq!(parse_volume_number("Prologue"), None);
	}

	#[aidoku_test]
	fn test_parse_mw_timestamp_returns_seconds() {
		assert_eq!(parse_mw_timestamp("1970-01-01T00:00:00Z"), Some(0));
		assert_eq!(parse_mw_timestamp("1970-01-02T00:00:00Z"), Some(86_400));
	}

	#[test]
	fn test_normalize_title() {
		assert_eq!(
			normalize_title("Foo%3AVolume_1_Chapter_1"),
			"Foo:Volume 1 Chapter 1"
		);
	}

	#[test]
	fn test_extract_summary() {
		let text = "= Synopsis =\nThis is the story of a hero.\nHe goes on adventures.\n\n== Chapter 1 ==\nMore stuff.";
		let summary = extract_summary(text);
		assert_eq!(
			summary,
			Some("This is the story of a hero. He goes on adventures.".into())
		);
	}

	#[aidoku_test]
	fn test_html_to_text() {
		let html = "<p>Hello world.</p><p>Second paragraph.</p>";
		let out = html_to_text(html);
		assert_eq!(out, "Hello world.\n\nSecond paragraph.");
	}
}
