use aidoku::{
	alloc::{String, Vec, string::ToString},
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{settings, watermark};

const VOID_TAGS: &[&str] = &["<br", "<img", "<hr"];

fn has_markup(input: &str) -> bool {
	input.contains("</") || VOID_TAGS.iter().any(|tag| input.contains(tag))
}

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
	let marker = ordered_marker_escape_index(trimmed);
	let mut escaped = String::default();
	escaped.push_str(indent);
	for (index, character) in trimmed.char_indices() {
		if Some(index) == marker || character == '<' {
			escaped.push('\\');
		}
		escaped.push(character);
	}
	escaped
}

fn ordered_marker_escape_index(line: &str) -> Option<usize> {
	let digits = line.chars().take_while(char::is_ascii_digit).count();
	if digits > 0 && line[digits..].starts_with(['.', ')']) {
		return Some(digits);
	}
	None
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

fn push_paragraph(paragraphs: &mut String, block: &str) {
	paragraphs.push_str("<p>");
	paragraphs.push_str(&escape_paragraph(block));
	paragraphs.push_str("</p>");
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
	paragraphs
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

fn convert_quoted(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children_to_markdown(element, &mut inner);
	let trimmed = inner.trim();
	if trimmed.is_empty() {
		return;
	}
	output.push('\u{201C}');
	output.push_str(trimmed);
	output.push('\u{201D}');
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
		"strong" | "b" | "em" | "i" | "u" | "s" | "del" => convert_emphasis(element, output),
		"code" => convert_inline_code(element, output),
		"q" => convert_quoted(element, output),
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
		| "hgroup" | "details" | "dl" | "address" | "nav" | "dialog" | "search" | "body"
		| "html" | "form" | "fieldset" => convert_block_container(element, output),
		"span" | "li" | "sup" | "sub" | "ruby" | "rt" | "rp" | "small" | "mark" | "abbr"
		| "cite" | "time" | "kbd" | "samp" | "var" | "ins" | "bdi" | "bdo" | "label" | "table"
		| "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" | "colgroup" | "col"
		| "wbr" | "picture" | "dfn" | "data" | "output" | "legend" | "slot" | "meter"
		| "progress" => convert_children_to_markdown(element, output),
		_ => {}
	}
}

fn convert_list_to_markdown(element: &Element, tag: &str, output: &mut String) {
	break_before_block(output);
	let ordered = tag == "ol";
	let items: Vec<_> = element
		.children()
		.filter(|child| child.tag_name().as_deref() == Some("li"))
		.collect();
	for (index, item) in items.iter().enumerate() {
		let mut body = String::default();
		convert_children_to_markdown(item, &mut body);
		let mut marker = String::default();
		if ordered {
			let _ = write!(marker, "{}. ", index + 1);
		} else {
			marker.push_str("- ");
		}
		let mut indent = String::default();
		for _ in 0..marker.len() {
			indent.push(' ');
		}
		output.push_str(&marker);
		for (line_index, line) in body.trim().lines().enumerate() {
			if line_index > 0 {
				output.push('\n');
			}
			if line.is_empty() {
				continue;
			}
			if line_index > 0 {
				output.push_str(&indent);
			}
			output.push_str(line);
		}
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

fn without_empty_emphasis(text: &str) -> String {
	let chars: Vec<char> = text.chars().collect();
	let mut output = String::default();
	let mut index = 0;
	while index < chars.len() {
		if chars[index] == '`' {
			let mut close = index;
			while close < chars.len() && chars[close] == '`' {
				close += 1;
			}
			let width = close - index;
			let mut after = close;
			let mut fence = None;
			while after < chars.len() {
				if chars[after] == '`' {
					let mut run = after;
					while run < chars.len() && chars[run] == '`' {
						run += 1;
					}
					let matches = if width >= 3 {
						run - after >= width
					} else {
						run - after == width
					};
					if matches {
						fence = Some(run);
						break;
					}
					after = run;
				} else {
					after += 1;
				}
			}
			let stop = fence.unwrap_or(close);
			output.push_str(&chars[index..stop].iter().collect::<String>());
			index = stop;
			continue;
		}
		let mut end = index;
		while end < chars.len() && chars[end] == '*' {
			end += 1;
		}
		let bounded = (index == 0 || chars[index - 1].is_whitespace())
			&& (end == chars.len() || chars[end].is_whitespace());
		let emptied = (end - index) % 2 == 0 && end > index;
		if emptied && bounded {
			if output.ends_with(' ') && chars.get(end) == Some(&' ') {
				output.pop();
			}
			index = end;
			continue;
		}
		output.push(chars[index]);
		index += 1;
	}
	output
}

pub fn to_markdown(input: &str) -> String {
	let body = if has_markup(input) {
		input.to_string()
	} else {
		assemble_paragraphs(input)
	};
	let wrapped = ["<div id=\"md-root\">", body.as_str(), "</div>"].concat();
	let Ok(doc) = Html::parse_fragment(wrapped) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#md-root") {
		convert_children_to_markdown(&root, &mut output);
	}
	let text = if settings::hide_watermark() {
		watermark::strip(output.trim())
	} else {
		output.trim().to_string()
	};
	without_empty_emphasis(&text)
}
