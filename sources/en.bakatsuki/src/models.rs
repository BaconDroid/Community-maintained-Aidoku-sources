use aidoku::alloc::{String, Vec};
use serde::Deserialize;

/// Top-level MediaWiki API response.
#[derive(Deserialize)]
pub struct MWResponse {
	#[serde(default)]
	pub query: Option<MWQuery>,
	#[serde(default)]
	pub parse: Option<MWParse>,
	#[serde(default, rename = "continue")]
	pub r#continue: Option<MWContinue>,
	#[serde(default)]
	pub error: Option<MWError>,
}

#[derive(Deserialize)]
pub struct MWError {
	#[serde(default)]
	#[allow(dead_code)]
	pub code: Option<String>,
	#[serde(default)]
	pub info: Option<String>,
}

/// Continue token — only `cmcontinue` for categorymembers pagination.
#[derive(Deserialize)]
pub struct MWContinue {
	#[serde(default)]
	pub cmcontinue: Option<String>,
}

/// Content of `query`.
#[derive(Deserialize)]
pub struct MWQuery {
	#[serde(default)]
	pub pages: Option<Vec<MWPage>>,
	#[serde(default)]
	pub categorymembers: Option<Vec<MWCategoryMember>>,
	#[serde(default)]
	pub search: Option<Vec<MWTitle>>,
	#[serde(default)]
	pub prefixsearch: Option<Vec<MWTitle>>,
	#[serde(default)]
	pub recentchanges: Option<Vec<MWTitle>>,
}

/// A page with optional metadata props.
#[derive(Deserialize)]
pub struct MWPage {
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub thumbnail: Option<MWThumbnail>,
	#[serde(default)]
	pub extract: Option<String>,
	#[serde(default)]
	pub categories: Option<Vec<MWCategory>>,
	#[serde(default)]
	pub missing: Option<String>,
	#[serde(default)]
	pub revisions: Option<Vec<MWRevision>>,
}

#[derive(Deserialize)]
pub struct MWRevision {
	#[serde(default)]
	pub timestamp: Option<String>,
}

#[derive(Deserialize)]
pub struct MWThumbnail {
	#[serde(default)]
	pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct MWCategory {
	#[serde(default)]
	pub title: Option<String>,
}

/// A category member, a search result, or a recent-change entry.
#[derive(Deserialize)]
pub struct MWCategoryMember {
	pub title: String,
}

#[derive(Deserialize)]
pub struct MWTitle {
	pub title: String,
}

/// Content of `parse`.
#[derive(Deserialize)]
pub struct MWParse {
	#[serde(default)]
	#[allow(dead_code)]
	pub title: Option<String>,
	#[serde(default)]
	pub text: Option<String>,
}

// ── Non-API structs ────────────────────────────────────────────────

/// In-memory novel catalogue entry (built from categorymembers).
#[derive(Clone)]
pub struct CatalogueEntry {
	pub title: String,
	pub status: String,
}
