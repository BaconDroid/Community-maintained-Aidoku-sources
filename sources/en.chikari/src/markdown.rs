use aidoku::{
	alloc::{String, Vec},
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{BASE_URL, settings, watermark};

fn join_lines(text: &str) -> String {
	text.lines()
		.filter(|line| !line.trim().is_empty())
		.collect::<Vec<&str>>()
		.join("  \n")
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
					output.push_str(&join_lines(&text));
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

fn anchored(inner: &str, open: &str, close: &str) -> String {
	let start = inner.len() - inner.trim_start().len();
	let end = inner.trim_end().len();
	if start >= end {
		return inner.into();
	}
	let mut out = String::from(&inner[..start]);
	out.push_str(open);
	out.push_str(&inner[start..end]);
	out.push_str(close);
	out.push_str(&inner[end..]);
	out
}

fn convert_emphasis(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children_to_markdown(element, &mut inner);
	let marker = match element.tag_name().as_deref() {
		Some("strong" | "b") => "**",
		_ => "*",
	};
	output.push_str(&anchored(&inner, marker, marker));
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
	if let Some(src) = element.attr("abs:src") {
		let alt = element.attr("alt").unwrap_or_default();
		let _ = writeln!(output, "![{alt}]({src})");
		output.push('\n');
	}
}

fn convert_link(element: &Element, output: &mut String) {
	if let Some(href) = element.attr("abs:href") {
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
	output.push_str(&anchored(&inner, "\u{201C}", "\u{201D}"));
}

// Elements come from the WHATWG HTML index,
// https://html.spec.whatwg.org/multipage/indices.html#elements-3 ; the obsolete
// ones that still carry prose come from
// https://html.spec.whatwg.org/multipage/obsolete.html#non-conforming-features .
// A tag outside both lists is dropped with its content.
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
		"strong" | "b" | "em" | "i" => convert_emphasis(element, output),
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
		| "html" | "form" | "fieldset" | "center" | "dir" => convert_block_container(element, output),
		"span" | "li" | "sup" | "sub" | "ruby" | "rt" | "rp" | "small" | "mark" | "abbr"
		| "cite" | "time" | "kbd" | "samp" | "var" | "ins" | "bdi" | "bdo" | "label" | "table"
		| "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" | "colgroup" | "col"
		| "wbr" | "picture" | "dfn" | "data" | "output" | "legend" | "slot" | "meter"
		| "progress" | "u" | "s" | "del" | "font" | "big" | "tt" | "strike" | "nobr"
		| "acronym" | "rb" | "rtc" => convert_children_to_markdown(element, output),
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
		let indent = " ".repeat(marker.len());
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

fn html_to_markdown(input: &str) -> String {
	let wrapped = ["<div id=\"md-root\">", input, "</div>"].concat();
	let Ok(doc) = Html::parse_fragment_with_url(wrapped, BASE_URL) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#md-root") {
		convert_children_to_markdown(&root, &mut output);
	}
	output
}

pub fn to_markdown(input: &str) -> String {
	let text = watermark::blocks(input)
		.into_iter()
		.map(|block| {
			if block.contains('<') {
				html_to_markdown(&block).trim().into()
			} else {
				block
			}
		})
		.collect::<Vec<String>>()
		.join("\n\n");
	if settings::hide_watermark() {
		watermark::strip(text.trim())
	} else {
		text.trim().into()
	}
}
