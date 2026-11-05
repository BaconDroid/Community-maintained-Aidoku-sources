use aidoku::{
	alloc::{String, Vec, string::ToString},
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{settings, watermark};

fn escape_markdown_text(text: &str) -> String {
	let mut output = String::default();
	for (index, line) in text.lines().enumerate() {
		if index > 0 {
			output.push_str("  \n");
		}
		output.push_str(&escape_line(line));
	}
	output
}

fn escape_line(line: &str) -> String {
	let trimmed = line.trim_start();
	let indent = &line[..line.len() - trimmed.len()];
	let block_marker = trimmed.starts_with(['#', '-', '+', '|']);
	let mut escaped = String::default();
	escaped.push_str(indent);
	for (index, character) in trimmed.char_indices() {
		if (index == 0 && block_marker) || matches!(character, '<' | '>') {
			escaped.push('\\');
		}
		escaped.push(character);
	}
	escaped
}

fn escape_paragraph(block: &str) -> String {
	let mut escaped = String::default();
	for character in block.chars() {
		match character {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			_ => escaped.push(character),
		}
	}
	escaped
}

fn assemble_paragraphs(input: &str) -> String {
	let mut paragraphs = String::default();
	let mut current = String::default();
	for line in input.lines() {
		if line.trim().is_empty() {
			if !current.is_empty() {
				push_paragraph(&mut paragraphs, &current);
				current.clear();
			}
		} else {
			if !current.is_empty() {
				current.push('\n');
			}
			current.push_str(line);
		}
	}
	if !current.is_empty() {
		push_paragraph(&mut paragraphs, &current);
	}
	["<div id=\"novelarchive-root\">", &paragraphs, "</div>"].concat()
}

fn push_paragraph(paragraphs: &mut String, block: &str) {
	paragraphs.push_str("<p>");
	paragraphs.push_str(&escape_paragraph(block));
	paragraphs.push_str("</p>");
}

fn longest_backtick_run(text: &str) -> usize {
	let mut longest = 0;
	let mut current = 0;
	for ch in text.chars() {
		if ch == '`' {
			current += 1;
			longest = longest.max(current);
		} else {
			current = 0;
		}
	}
	longest
}

fn append_raw_text(element: &Element, output: &mut String) {
	if let Some(text) = element.text() {
		output.push_str(&text);
	}
}

// Element-kind nodes pair with the next children entry.
fn convert_children_to_markdown(element: &Element, output: &mut String) {
	let mut elements = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					output.push_str(&escape_markdown_text(&text));
				}
			}
			Kind::Element => {
				if let Some(child) = elements.next() {
					convert_element_to_markdown(&child, output);
				}
			}
			_ => {}
		}
	}
}

fn break_before_block(output: &mut String) {
	if output.is_empty() {
		return;
	}
	while !output.ends_with("\n\n") {
		output.push('\n');
	}
}

fn convert_heading(element: &Element, output: &mut String) {
	break_before_block(output);
	let tag = element.tag_name().unwrap_or_default();
	let level = tag.as_bytes()[1] - b'0';
	for _ in 0..level {
		output.push('#');
	}
	output.push(' ');
	convert_children_to_markdown(element, output);
	output.push_str("\n\n");
}

fn convert_emphasis(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children_to_markdown(element, &mut inner);
	let trimmed = inner.trim();
	if trimmed.is_empty() {
		return;
	}
	// CommonMark has no strikethrough or underline: emit the content unmarked.
	let marker = match element.tag_name().as_deref() {
		Some("strong" | "b") => "**",
		Some("em" | "i") => "*",
		_ => "",
	};
	output.push_str(marker);
	output.push_str(trimmed);
	output.push_str(marker);
}

fn convert_inline_code(element: &Element, output: &mut String) {
	let mut raw = String::default();
	append_raw_text(element, &mut raw);
	let ticks = longest_backtick_run(&raw) + 1;
	for _ in 0..ticks {
		output.push('`');
	}
	// CommonMark strips one padding space: pad content touching the fence.
	if raw.starts_with('`') || raw.ends_with('`') {
		output.push(' ');
		output.push_str(&raw);
		output.push(' ');
	} else {
		output.push_str(&raw);
	}
	for _ in 0..ticks {
		output.push('`');
	}
}

fn convert_fenced_code(element: &Element, output: &mut String) {
	break_before_block(output);
	let mut raw = String::default();
	append_raw_text(element, &mut raw);
	let fence = "`".repeat(3.max(longest_backtick_run(&raw) + 1));
	output.push_str(&fence);
	output.push('\n');
	output.push_str(&raw);
	if !raw.ends_with('\n') {
		output.push('\n');
	}
	output.push_str(&fence);
	output.push_str("\n\n");
}

fn convert_image(element: &Element, output: &mut String) {
	break_before_block(output);
	if let Some(src) = element.attr("src") {
		let alt = element.attr("alt").unwrap_or_default();
		let _ = writeln!(output, "![{alt}]({src})");
		output.push('\n');
	}
}

fn convert_link(element: &Element, output: &mut String) {
	if let Some(href) = element.attr("href") {
		output.push('[');
		convert_children_to_markdown(element, output);
		let _ = write!(output, "]({href})");
	} else {
		convert_children_to_markdown(element, output);
	}
}

fn convert_block_container(element: &Element, output: &mut String) {
	break_before_block(output);
	convert_children_to_markdown(element, output);
	if !output.ends_with("\n\n") && !output.ends_with('\n') {
		output.push('\n');
	}
}

fn convert_element_to_markdown(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();
	match tag.as_str() {
		"p" | "figcaption" | "dt" | "dd" | "summary" => {
			break_before_block(output);
			convert_children_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"br" => output.push_str("  \n"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => convert_heading(element, output),
		"strong" | "b" | "em" | "i" | "u" | "s" | "strike" | "del" => {
			convert_emphasis(element, output)
		}
		"code" => convert_inline_code(element, output),
		"pre" => convert_fenced_code(element, output),
		"img" => convert_image(element, output),
		"a" => convert_link(element, output),
		"hr" => {
			break_before_block(output);
			output.push_str("---\n\n")
		}
		"ul" | "ol" | "menu" => convert_list_to_markdown(element, &tag, output),
		"blockquote" => convert_blockquote_to_markdown(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" | "figure"
		| "hgroup" | "details" | "dl" => convert_block_container(element, output),
		"span" | "li" => convert_children_to_markdown(element, output),
		_ => convert_children_to_markdown(element, output),
	}
}

fn convert_list_to_markdown(element: &Element, tag: &str, output: &mut String) {
	break_before_block(output);
	let items: Vec<_> = element
		.children()
		.filter(|child| child.tag_name().as_deref() == Some("li"))
		.collect();
	for (index, item) in items.iter().enumerate() {
		if tag == "ol" {
			let _ = write!(output, "{}. ", index + 1);
		} else {
			output.push_str("- ");
		}
		convert_children_to_markdown(item, output);
		output.push('\n');
	}
	output.push('\n');
}

fn convert_blockquote_to_markdown(element: &Element, output: &mut String) {
	break_before_block(output);
	let mut quoted = String::default();
	convert_children_to_markdown(element, &mut quoted);
	for (index, line) in quoted.trim_end().lines().enumerate() {
		if index > 0 {
			output.push('\n');
		}
		output.push_str("> ");
		output.push_str(line);
	}
	output.push_str("\n\n");
}

pub fn html_to_markdown(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(assemble_paragraphs(html)) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#novelarchive-root") {
		convert_children_to_markdown(&root, &mut output);
	}
	if settings::hide_watermark() {
		watermark::strip(output.trim())
	} else {
		output.trim().to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn keeps_paragraphs_and_inline_markup() {
		let out = html_to_markdown("First *bzz* line.\n\nSecond **bold** line.");
		assert_eq!(out, "First *bzz* line.\n\nSecond **bold** line.");
		let out = html_to_markdown("One.\n\n***\n\nTwo.");
		assert_eq!(out, "One.\n\n***\n\nTwo.");
	}

	#[aidoku_test]
	fn keeps_inline_markdown_symbols() {
		let out = html_to_markdown("Use `code`, ~~gone~~, [site](https://example.com) and |a|b|.");
		assert_eq!(
			out,
			"Use `code`, ~~gone~~, [site](https://example.com) and |a|b|."
		);
	}

	#[aidoku_test]
	fn keeps_ordered_lists_as_prose() {
		let out = html_to_markdown("1. The Beginning\n2) Next");
		assert_eq!(out, "1. The Beginning  \n2) Next");
	}

	#[aidoku_test]
	fn escapes_block_markers_at_line_start() {
		let out = html_to_markdown("- item\n+ item\n# head\n| cell");
		assert_eq!(out, "\\- item  \n\\+ item  \n\\# head  \n\\| cell");
	}

	#[aidoku_test]
	fn escapes_angle_brackets() {
		let out = html_to_markdown("<Imperial Rieta Orphanage>\n\nHe shrugged.");
		assert_eq!(out, "\\<Imperial Rieta Orphanage\\>\n\nHe shrugged.");
		let out = html_to_markdown("Tom & Jerry, 5 < 10 and a > b");
		assert_eq!(out, "Tom & Jerry, 5 \\< 10 and a \\> b");
	}
}
