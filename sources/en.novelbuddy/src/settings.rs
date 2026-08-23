use aidoku::{
	alloc::{String, Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const HIDDEN_GENRES_KEY: &str = "hiddenGenres";
const DEFAULT_SORT_KEY: &str = "defaultSort";
const DEFAULT_STATUS_KEY: &str = "defaultStatus";

pub fn hidden_genres() -> Vec<String> {
	defaults_get::<Vec<String>>(HIDDEN_GENRES_KEY).unwrap_or_default()
}

pub fn reset_hidden_genres() {
	defaults_set(HIDDEN_GENRES_KEY, DefaultValue::Null);
}

/// Default sort order used when the search sort filter is left at its own
/// default. Mirrors the `sort` options in `filters.json`.
pub fn default_sort() -> String {
	defaults_get::<String>(DEFAULT_SORT_KEY).unwrap_or_else(|| "popular".into())
}

/// Default status filter used when the search status filter is left at "All".
/// Mirrors the `status` options in `filters.json`.
pub fn default_status() -> String {
	defaults_get::<String>(DEFAULT_STATUS_KEY).unwrap_or_else(|| "all".into())
}
