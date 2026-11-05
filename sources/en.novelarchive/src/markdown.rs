//! The chapter endpoint returns plain text, already using markdown for emphasis
//! (`*bzz*`) and scene breaks (`***`). Reproduce that text as markdown instead
//! of escaping it, so the site's emphasis survives; only angle brackets are
//! escaped, because a reader would treat `<...>` as markup.

use aidoku::alloc::String;

use crate::{settings, watermark};

fn escape_angle_brackets(line: &str) -> String {
	if !line.contains(['<', '>']) {
		return line.into();
	}
	let mut escaped = String::default();
	for character in line.chars() {
		if matches!(character, '<' | '>') {
			escaped.push('\\');
		}
		escaped.push(character);
	}
	escaped
}

pub fn text_to_markdown(text: &str) -> String {
	let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
	let mut output = String::default();
	let mut after_blank_line = false;
	for raw in normalized.lines() {
		let line = raw.trim_end();
		if line.trim().is_empty() {
			after_blank_line = !output.is_empty();
			continue;
		}
		if after_blank_line {
			output.push_str("\n\n");
			after_blank_line = false;
		} else if !output.is_empty() {
			output.push('\n');
		}
		output.push_str(&escape_angle_brackets(line));
	}
	if settings::hide_watermark() {
		watermark::strip(&output)
	} else {
		output
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn keeps_paragraphs_and_emphasis() {
		let out = text_to_markdown("First *bzz* line.\n\nSecond **bold** line.");
		assert_eq!(out, "First *bzz* line.\n\nSecond **bold** line.");
	}

	#[aidoku_test]
	fn keeps_scene_breaks_and_collapses_blank_lines() {
		let out = text_to_markdown("One.\n\n\n\n***\n\nTwo.\r\n");
		assert_eq!(out, "One.\n\n***\n\nTwo.");
	}

	#[aidoku_test]
	fn escapes_only_angle_brackets() {
		let out = text_to_markdown("A <Bond> of shadow, 5 < 10, and *emphasis*.");
		assert_eq!(out, "A \\<Bond\\> of shadow, 5 \\< 10, and *emphasis*.");
	}

	#[aidoku_test]
	fn keeps_single_line_breaks() {
		let out = text_to_markdown("Line one\nLine two");
		assert_eq!(out, "Line one\nLine two");
	}
}
