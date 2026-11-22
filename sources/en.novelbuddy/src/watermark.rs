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

const QUOTES: &[char] = &[
	'"', '\'', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '\u{AB}', '\u{BB}', '\u{2039}',
	'\u{203A}',
];

const SENTENCE_ENDS: &[char] = &[
	'.', '!', '?', '\u{2026}', '\u{203C}', '\u{203D}', '\u{2047}', '\u{2048}', '\u{2049}',
];

/// Prose is the three blocks, and whitespace from anywhere: a gap between two
/// words is not a mark the credit left.
fn is_prose_char(ch: char) -> bool {
	let code = u32::from(ch);
	ch.is_whitespace()
		|| PROSE_BLOCKS
			.iter()
			.any(|(start, end)| (*start..=*end).contains(&code))
}

/// A sign from another script: a bracket, a dash, a bullet. It is not prose,
/// and it is not a letter either.
fn is_sign(ch: char) -> bool {
	!is_prose_char(ch) && !ch.is_alphanumeric()
}

/// A letter or digit from another script, which is what the credit spells
/// itself with.
fn is_foreign(ch: char) -> bool {
	!is_prose_char(ch) && ch.is_alphanumeric()
}

/// True when the word carries anything the prose blocks do not hold: a sign or
/// a letter from another script.
fn is_noisy(word: &str) -> bool {
	word.chars().any(|ch| !is_prose_char(ch))
}

fn is_separator(ch: char) -> bool {
	!ch.is_alphanumeric() && ch != '\n' && ch != '\r'
}

fn is_quote(ch: char) -> bool {
	QUOTES.contains(&ch)
}

fn pair_close(open: char) -> Option<char> {
	PAIRS.iter().find(|pair| pair.0 == open).map(|pair| pair.1)
}

fn matching_opener(close: char) -> Option<char> {
	PAIRS.iter().find(|pair| pair.1 == close).map(|pair| pair.0)
}

/// The closing character a quote expects: an ASCII quote closes on itself,
/// every other quote takes its counterpart from `PAIRS`.
fn quote_close(open: char) -> Option<char> {
	match open {
		'"' | '\'' => Some(open),
		_ => pair_close(open),
	}
}

/// How many bytes the leading run of signs takes.
fn sign_run(text: &str) -> usize {
	text.char_indices()
		.take_while(|(_, ch)| is_sign(*ch))
		.map(|(index, ch)| index + ch.len_utf8())
		.last()
		.unwrap_or(0)
}

/// Where the trailing run of signs starts.
fn trailing_sign_start(text: &str) -> usize {
	text.char_indices()
		.rev()
		.find(|(_, ch)| !is_sign(*ch))
		.map_or(0, |(index, ch)| index + ch.len_utf8())
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
		let mut after = 0;
		while text.get(span + marks + after) == Some(&' ') {
			after += 1;
		}
		let trailing = output.chars().rev().take_while(|ch| *ch == ' ').count();
		let gap = trailing + after;
		// Snap the gap around an injection down to its nearest bound: one space,
		// then steps of four, so indentation and alignment keep their rung.
		let bound = if gap < 4 { 1 } else { gap / 4 * 4 };
		while output.ends_with(' ') {
			output.pop();
		}
		if text
			.get(span + marks + after)
			.is_some_and(|ch| !ch.is_whitespace())
		{
			for _ in 0..bound {
				output.push(' ');
			}
		}
		index = span + marks + after;
	}
	output
}

fn sentences(text: &str) -> Vec<String> {
	let chars: Vec<char> = text.chars().collect();
	let mut out: Vec<String> = Vec::new();
	let mut start = 0;
	for (index, ch) in chars.iter().enumerate() {
		if !SENTENCE_ENDS.contains(ch) {
			continue;
		}
		let mut end = index + 1;
		while end < chars.len() && matching_opener(chars[end]).is_some() {
			end += 1;
		}
		let mut next = end;
		while next < chars.len() && chars[next].is_whitespace() {
			next += 1;
		}
		if next > end {
			out.push(chars[start..end].iter().collect());
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

/// Joins the parts that carry text, one space between them.
fn join_words(parts: &[&str]) -> String {
	let mut out = String::default();
	for part in parts.iter().filter(|part| !part.is_empty()) {
		if !out.is_empty() {
			out.push(' ');
		}
		out.push_str(part);
	}
	out
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
	// Symbols around a word are decoration and can go; a look-alike letter is
	// the credit's signature, and the whole token goes with it.
	let token = &line[start..end];
	let noisy_letters = token.chars().any(is_foreign);
	if !noisy_letters {
		// A mark that sat between two letters becomes a space, so the words it
		// separated stay two words.
		let mut core = String::default();
		let chars: Vec<char> = token.chars().collect();
		for (index, ch) in chars.iter().enumerate() {
			if is_prose_char(*ch) {
				core.push(*ch);
				continue;
			}
			let before = index
				.checked_sub(1)
				.and_then(|at| chars.get(at))
				.is_some_and(|prev| prev.is_alphanumeric());
			let after = chars
				.get(index + 1)
				.is_some_and(|next| next.is_alphanumeric());
			if before && after {
				core.push(' ');
			}
		}
		let core = core.trim();
		if holds_word(core) {
			return Some(join_words(&[
				line[..start].trim_end(),
				core,
				line[end..].trim_start(),
			]));
		}
	}
	Some(join_words(&[
		line[..start].trim_end(),
		line[end..].trim_start(),
	]))
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

/// A quote may front the marks of a dialogue line: it is set aside so the marks
/// are still read as the opener, then put back in front of the result.
fn dialogue_line(line: &str) -> String {
	let lead_end = line
		.char_indices()
		.take_while(|(_, ch)| is_sign(*ch) || is_quote(*ch) || *ch == ' ')
		.map(|(index, ch)| index + ch.len_utf8())
		.last()
		.unwrap_or(0);
	let lead = &line[..lead_end];
	if !lead.chars().any(is_quote) || !lead.chars().any(is_sign) {
		return dialogue_marks(line);
	}
	let rest = &line[lead_end..];
	let quote = lead.chars().find(|ch| is_quote(*ch)).unwrap_or('"');
	let closes = line
		.trim_end()
		.chars()
		.next_back()
		.is_some_and(|ch| quote_close(quote) == Some(ch));
	// The closing marker names the family to keep, and it has to be the
	// counterpart of the opener: a half-quoted line is not a quotation.
	if !closes {
		let mut out = String::from("\u{2014}");
		if lead.ends_with(' ') {
			out.push(' ');
		}
		out.push_str(rest);
		return out;
	}
	let mut out = String::from(quote);
	out.push_str(rest);
	out
}

fn dialogue_marks(line: &str) -> String {
	let mark_end = sign_run(line);
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
		let end = sign_run(trimmed);
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
	if !is_sign(last) {
		return None;
	}
	let close_start = trailing_sign_start(trimmed);
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

/// True when the sentence carries a domain-shaped token.
fn has_domain(sentence: &str) -> bool {
	let chars: Vec<char> = sentence.chars().collect();
	(0..chars.len()).any(|at| {
		chars[at] == '.'
			&& at > 0 && is_word_char(chars[at - 1])
			&& matches!(tail_len(&chars, at), 2 | 3)
	})
}

/// How many lowercase letters sit right after the dot at `dot`, capped at 4 so
/// a long word is not mistaken for a domain tail.
fn tail_len(chars: &[char], dot: usize) -> usize {
	chars
		.iter()
		.skip(dot + 1)
		.take_while(|ch| ch.is_ascii_lowercase())
		.take(5)
		.count()
}

/// True when the text still holds a letter or a digit worth keeping.
fn holds_word(text: &str) -> bool {
	text.chars().any(|ch| ch.is_alphanumeric())
}

/// A domain label may carry a hyphen, so the token run includes it.
fn is_word_char(ch: char) -> bool {
	(ch.is_alphanumeric() || ch == '-') && is_prose_char(ch)
}

fn clean_credit_sentence(text: &str) -> Option<String> {
	let index = text.find(|ch| !is_prose_char(ch))?;
	if let Some(cleaned) = without_single_foreign_token(text)
		&& holds_word(&cleaned)
	{
		return Some(cleaned);
	}
	// The sentence opens on marks, so keep whatever follows the last of them.
	tail_after_last_noisy_token(text, index)
}

fn clean_line(line: &str) -> Option<String> {
	let dialogue = dialogue_line(line);
	let cleaned = without_dead_pairs(&dialogue);
	let line = cleaned.as_str();
	if line.trim().is_empty() {
		return None;
	}
	if line.find(|ch| !is_prose_char(ch)).is_none() && !has_domain(line) {
		return Some(line.into());
	}
	let mut parts: Vec<String> = Vec::new();
	for sentence in sentences(line) {
		let sentence = sentence.trim();
		if has_domain(sentence) {
			continue;
		}
		if sentence.find(|ch| !is_prose_char(ch)).is_none() {
			parts.push(sentence.into());
		} else if let Some(cleaned) = clean_credit_sentence(sentence) {
			parts.push(cleaned);
		} else {
			// A sentence the credit patterns do not recognise is prose wearing
			// marks: drop the marks, keep the words, and keep the sentence.
			let kept: String = sentence.chars().filter(|ch| is_prose_char(*ch)).collect();
			let kept = kept.trim();
			if holds_word(kept) {
				parts.push(kept.into());
			}
		}
	}
	(!parts.is_empty()).then(|| parts.join(" "))
}

/// Takes one block, not a document: the caller already split the text.
pub(crate) fn strip(block: &str) -> String {
	let mut kept: Vec<String> = Vec::new();
	for line in block.trim().lines() {
		if let Some(cleaned) = clean_line(&strip_injected(line)) {
			kept.push(cleaned);
		}
	}
	kept.join("\n").trim_end().into()
}
