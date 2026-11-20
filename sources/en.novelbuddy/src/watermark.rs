//! Match characters rather than wording: the injected credit rotates through Unicode look-alikes.

use aidoku::alloc::{String, Vec};

const PROSE_BLOCKS: &[(u32, u32)] = &[
	(0x0000, 0x007F), // Basic Latin
	(0x0080, 0x00FF), // Latin-1 Supplement
	(0x2000, 0x206F), // General Punctuation
];

const PAIRS: &[(char, char)] = &[
	('(', ')'),
	('[', ']'),
	('{', '}'),
	('<', '>'),
	('\u{AB}', '\u{BB}'),
	('\u{201A}', '\u{2018}'),
	('\u{201E}', '\u{201C}'),
	('\u{2018}', '\u{2019}'),
	('\u{2045}', '\u{2046}'),
	('\u{201C}', '\u{201D}'),
	('\u{2039}', '\u{203A}'),
];

const SPELLED_MIN_LETTERS: usize = 5;

const SENTENCE_ENDS: &[char] = &[
	'.', '!', '?', '\u{2026}', '\u{203C}', '\u{203D}', '\u{2047}', '\u{2048}', '\u{2049}',
];

fn is_prose_char(ch: char) -> bool {
	let code = u32::from(ch);
	ch.is_whitespace()
		|| PROSE_BLOCKS
			.iter()
			.any(|(start, end)| (*start..=*end).contains(&code))
}

fn is_leading_mark(ch: char) -> bool {
	!is_prose_char(ch) && !ch.is_alphanumeric()
}

fn is_noisy(word: &str) -> bool {
	word.chars().any(|ch| !is_prose_char(ch))
}

fn is_separator(ch: char) -> bool {
	!ch.is_alphanumeric() && ch != '\n' && ch != '\r'
}

fn pair_close(open: char) -> Option<char> {
	PAIRS.iter().find(|pair| pair.0 == open).map(|pair| pair.1)
}

fn matching_opener(close: char) -> Option<char> {
	PAIRS.iter().find(|pair| pair.1 == close).map(|pair| pair.0)
}

fn spelled_run(text: &[char], start: usize) -> Option<usize> {
	if !text.get(start)?.is_alphabetic() {
		return None;
	}
	if start > 0 && text[start - 1].is_alphanumeric() {
		return None;
	}
	if !is_separator(*text.get(start + 1)?) {
		return None;
	}
	let mut end = start + 1;
	let mut letters = 1;
	while text.get(end).is_some_and(|ch| is_separator(*ch)) {
		let Some(letter) = text.get(end + 1) else {
			break;
		};
		if !letter.is_alphabetic() {
			break;
		}
		if text.get(end + 2).is_some_and(|next| next.is_alphanumeric()) {
			break;
		}
		letters += 1;
		end += 2;
	}
	(letters >= SPELLED_MIN_LETTERS).then_some(end)
}

fn strip_injected(markdown: &str) -> String {
	let text: Vec<char> = markdown.chars().collect();
	let mut output = String::default();
	let mut index = 0;
	while index < text.len() {
		let Some(span) = spelled_run(&text, index) else {
			output.push(text[index]);
			index += 1;
			continue;
		};
		let mut left = 0;
		for ch in output.chars().rev() {
			if ch.is_whitespace() || ch.is_alphanumeric() {
				break;
			}
			left += 1;
		}
		let mut right = 0;
		for ch in text[span..].iter() {
			if ch.is_whitespace() || ch.is_alphanumeric() {
				break;
			}
			right += 1;
		}
		let marks = left.min(right);
		for _ in 0..marks {
			output.pop();
		}
		let end = span + marks;
		let at_line_start = output.is_empty() || output.ends_with('\n');
		index = if text.get(end) == Some(&' ') && (at_line_start || output.ends_with(' ')) {
			end + 1
		} else {
			end
		};
	}
	output
}

fn unbalanced(text: &str) -> Vec<char> {
	let mut stack: Vec<char> = Vec::new();
	for ch in text.chars() {
		if pair_close(ch).is_some() {
			stack.push(ch);
		} else if let Some(open) = matching_opener(ch)
			&& stack.last() == Some(&open)
		{
			stack.pop();
		}
	}
	stack
}

fn sentences(text: &str) -> Vec<String> {
	let chars: Vec<char> = text.chars().collect();
	let mut out: Vec<String> = Vec::new();
	let mut start = 0;
	for (index, ch) in chars.iter().enumerate() {
		if !SENTENCE_ENDS.contains(ch) {
			continue;
		}
		let mut next = index + 1;
		while next < chars.len() && chars[next].is_whitespace() {
			next += 1;
		}
		if next > index + 1 {
			out.push(chars[start..index + 1].iter().collect());
			start = next;
		}
	}
	if start < chars.len() {
		out.push(chars[start..].iter().collect());
	}
	out
}

fn trim_noisy_walls(sentence: &str) -> Vec<&str> {
	let words: Vec<&str> = sentence.split_whitespace().collect();
	let mut start = 0;
	let mut end = words.len();
	while start < end && is_noisy(words[start]) {
		start += 1;
	}
	while end > start && is_noisy(words[end - 1]) {
		end -= 1;
	}
	words[start..end].to_vec()
}

fn without_dead_pairs(line: &str) -> String {
	let mut text: Vec<char> = line.chars().collect();
	let mut index = 0;
	while index < text.len() {
		let opener = text[index];
		let Some(close) = pair_close(opener) else {
			index += 1;
			continue;
		};
		let mut end = index + 1;
		let mut depth = 0;
		while let Some(&ch) = text.get(end) {
			if ch == close && depth == 0 {
				break;
			}
			if ch == close {
				depth -= 1;
			} else if ch == opener {
				depth += 1;
			}
			end += 1;
		}
		if text.get(end) != Some(&close) {
			index += 1;
			continue;
		}
		let content = &text[index + 1..end];
		let blank = content.iter().all(|ch| ch.is_whitespace());
		let foreign = content.iter().any(|ch| !is_prose_char(*ch));
		if !(blank || foreign) {
			index += 1;
			continue;
		}
		if blank && text.get(end + 1) == Some(&'(') {
			index += 1;
			continue;
		}
		if foreign {
			let inner: String = text[index + 1..end].iter().collect();
			let mut kept_sentences: Vec<String> = Vec::new();
			for sentence in sentences(&inner) {
				let words = trim_noisy_walls(&sentence);
				if words.is_empty() || words.iter().any(|word| is_noisy(word)) {
					continue;
				}
				kept_sentences.push(words.join(" "));
			}
			if !kept_sentences.is_empty() {
				text.splice(index + 1..end, kept_sentences.join(" ").chars());
				index = 0;
				continue;
			}
		}
		end += 1;
		if text.get(end) == Some(&' ') && (index == 0 || text[index - 1] == ' ') {
			end += 1;
		}
		text.drain(index..end);
		index = 0;
	}
	text.into_iter().collect()
}

fn last_phrase_end(text: &str) -> Option<usize> {
	let mut found = None;
	for (index, ch) in text.char_indices() {
		if !SENTENCE_ENDS.contains(&ch) {
			continue;
		}
		let after = index + ch.len_utf8();
		if text[after..]
			.chars()
			.next()
			.is_none_or(|next| next.is_whitespace())
		{
			found = Some(after);
		}
	}
	found
}

fn without_single_foreign_token(line: &str) -> Option<String> {
	let mut hit: Option<(usize, usize)> = None;
	let mut word = None;
	for (index, ch) in line.char_indices() {
		if ch.is_whitespace() {
			if let Some(start) = word.take()
				&& !note_foreign(&mut hit, line, start, index)
			{
				return None;
			}
		} else if word.is_none() {
			word = Some(index);
		}
	}
	if let Some(start) = word
		&& !note_foreign(&mut hit, line, start, line.len())
	{
		return None;
	}
	let (start, end) = hit?;
	let mut cleaned = String::from(line[..start].trim_end());
	let rest = line[end..].trim_start();
	if !cleaned.is_empty() && !rest.is_empty() {
		cleaned.push(' ');
	}
	cleaned.push_str(rest);
	Some(cleaned)
}

fn note_foreign(hit: &mut Option<(usize, usize)>, line: &str, start: usize, end: usize) -> bool {
	if !is_noisy(&line[start..end]) {
		return true;
	}
	if hit.is_some() {
		return false;
	}
	*hit = Some((start, end));
	true
}

fn dialogue_line(line: &str) -> String {
	let mut mark_end = 0;
	for (index, ch) in line.char_indices() {
		if !is_leading_mark(ch) {
			break;
		}
		mark_end = index + ch.len_utf8();
	}
	if mark_end == 0 {
		return line.into();
	}
	if let Some(inner) = wrapped_text(line, mark_end) {
		return inner;
	}
	let mut start = mark_end;
	loop {
		let rest = &line[start..];
		let trimmed = rest.trim_start();
		let spaces = rest.len() - trimmed.len();
		let mut end = 0;
		for (index, ch) in trimmed.char_indices() {
			if !is_leading_mark(ch) {
				break;
			}
			end = index + ch.len_utf8();
		}
		if end == 0 {
			start += spaces;
			break;
		}
		start += spaces + end;
	}
	let rest = &line[start..];
	let Some(first) = rest.chars().next() else {
		return line.into();
	};
	if !is_prose_char(first) {
		return line.into();
	}
	let mut out = String::from("\u{2014}");
	if line[..start]
		.chars()
		.next_back()
		.is_some_and(|ch| ch.is_whitespace())
	{
		out.push(' ');
	}
	out.push_str(rest);
	out
}

fn wrapped_text(line: &str, mark_end: usize) -> Option<String> {
	let trimmed = line.trim_end();
	let last = trimmed.chars().next_back()?;
	if !is_leading_mark(last) {
		return None;
	}
	let close_start = trimmed
		.char_indices()
		.rev()
		.find(|(_, ch)| !is_leading_mark(*ch))
		.map_or(0, |(index, ch)| index + ch.len_utf8());
	if close_start <= mark_end {
		return None;
	}
	let inner = line[mark_end..close_start].trim();
	(!inner.is_empty()).then(|| String::from(inner))
}

fn tail_after_last_noisy_token(line: &str, from: usize) -> Option<String> {
	let mut last_end = from;
	let mut start = None;
	for (index, ch) in line.char_indices() {
		if ch.is_whitespace() {
			if let Some(word) = start.take()
				&& is_noisy(&line[word..index])
			{
				last_end = index;
			}
		} else if start.is_none() {
			start = Some(index);
		}
	}
	if let Some(word) = start
		&& is_noisy(&line[word..])
	{
		last_end = line.len();
	}
	let mut word = None;
	for (index, ch) in line
		.char_indices()
		.skip_while(|(index, _)| *index < last_end)
	{
		if ch.is_whitespace() {
			if let Some(start) = word.take()
				&& line[start..index].starts_with(char::is_uppercase)
			{
				return Some(line[start..].trim_end().into());
			}
		} else if word.is_none() {
			word = Some(index);
		}
	}
	if let Some(start) = word
		&& line[start..].starts_with(char::is_uppercase)
	{
		return Some(line[start..].trim_end().into());
	}
	None
}

fn clean_credit_sentence(text: &str) -> Option<String> {
	let index = text.find(|ch| !is_prose_char(ch))?;
	if let Some(cleaned) = without_single_foreign_token(text)
		&& cleaned.chars().any(|ch| ch.is_alphanumeric())
	{
		return Some(cleaned);
	}
	let head = &text[..index];
	let Some(end) = last_phrase_end(head) else {
		return tail_after_last_noisy_token(text, index);
	};
	let mut fixed = String::from(&head[..end]);
	if let Some(tail) = tail_after_last_noisy_token(text, index) {
		fixed.push(' ');
		fixed.push_str(&tail);
	}
	for opener in unbalanced(&fixed).into_iter().rev() {
		let Some(close) = pair_close(opener) else {
			break;
		};
		fixed.push(close);
	}
	Some(fixed)
}

fn append_block(block: &str) -> String {
	let mut kept: Vec<String> = Vec::new();
	for raw_line in block.trim().lines() {
		let cleaned = without_dead_pairs(raw_line);
		let dialogue = dialogue_line(&cleaned);
		let line = dialogue.as_str();
		if line.trim().is_empty() {
			continue;
		}
		if line.find(|ch| !is_prose_char(ch)).is_none() {
			kept.push(line.into());
			continue;
		}
		let mut parts: Vec<String> = Vec::new();
		for sentence in sentences(line) {
			if sentence.find(|ch| !is_prose_char(ch)).is_none() {
				parts.push(sentence.trim().into());
			} else if let Some(cleaned) = clean_credit_sentence(&sentence) {
				parts.push(cleaned);
			}
		}
		if !parts.is_empty() {
			kept.push(parts.join(" "));
		}
	}
	kept.join("\n").trim_end().into()
}

/// Cleans a document: blocks are cleaned on their own and rejoined by a blank
/// line, and a block that keeps nothing leaves no gap.
pub fn strip(text: &str) -> String {
	let mut blocks: Vec<String> = Vec::new();
	let mut block = String::default();
	for line in text.lines() {
		if line.trim().is_empty() {
			if !block.is_empty() {
				blocks.push(core::mem::take(&mut block));
			}
		} else {
			block.push_str(if block.is_empty() { "" } else { "\n" });
			block.push_str(line);
		}
	}
	if !block.is_empty() {
		blocks.push(block);
	}
	blocks
		.iter()
		.map(|block| append_block(&strip_injected(block)))
		.filter(|block| !block.is_empty())
		.collect::<Vec<String>>()
		.join("\n\n")
}
