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

const SPELLED_MIN_LETTERS: usize = 4;

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
	let mut at = start + 1;
	let mut letters = 1;
	let mut end = at;
	loop {
		let before = at;
		while text
			.get(at)
			.is_some_and(|ch| is_separator(*ch) && pair_close(*ch).is_none())
		{
			at += 1;
		}
		if at == before {
			break;
		}
		let Some(letter) = text.get(at) else {
			break;
		};
		if !letter.is_alphabetic() {
			break;
		}
		if text.get(at + 1).is_some_and(|next| next.is_alphanumeric()) {
			break;
		}
		letters += 1;
		at += 1;
		end = at;
	}
	(letters >= SPELLED_MIN_LETTERS).then_some(end)
}

/// The marks a credit left on one side, past the spaces that touch it.
fn marks(side: impl Iterator<Item = char>) -> usize {
	let mut count = 0;
	let mut seen = false;
	for ch in side {
		if ch == ' ' && !seen {
			count += 1;
			continue;
		}
		if ch.is_whitespace() || ch.is_alphanumeric() {
			break;
		}
		seen = true;
		count += 1;
	}
	count
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
		let left = marks(output.chars().rev());
		let right = marks(text[span..].iter().copied());
		for _ in 0..left {
			output.pop();
		}
		let mut after = 0;
		while text.get(span + right + after) == Some(&' ') {
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
			.get(span + right + after)
			.is_some_and(|ch| !ch.is_whitespace())
		{
			for _ in 0..bound {
				output.push(' ');
			}
		}
		index = span + right + after;
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
	loop {
		// Every open pair, matched by a stack so mixed and nested brackets line
		// up with their own closer.
		let mut stack: Vec<usize> = Vec::new();
		let mut pairs: Vec<(usize, usize)> = Vec::new();
		for (at, ch) in text.iter().enumerate() {
			if is_quote(*ch) {
				continue;
			}
			if pair_close(*ch).is_some() {
				stack.push(at);
				continue;
			}
			if let Some(&open) = stack.last()
				&& pair_close(text[open]) == Some(*ch)
			{
				stack.pop();
				pairs.push((open, at));
			}
		}
		// A pair is recorded when its closer is reached, so the list already runs
		// from the innermost pair outwards, which is the order they are decided in.
		let mut changed = false;
		for (open, close_at) in pairs {
			let content = &text[open + 1..close_at];
			let blank = content.iter().all(|ch| ch.is_whitespace());
			let foreign = content.iter().any(|ch| !is_prose_char(*ch));
			if !(blank || foreign) {
				continue;
			}
			if foreign {
				if content.len() <= 1 {
					continue;
				}
				let inner: String = content.iter().collect();
				let mut kept: Vec<String> = Vec::new();
				for sentence in sentences(&inner) {
					let words = trim_noisy_walls(&sentence);
					if words.is_empty() || words.iter().any(|word| is_noisy(word)) {
						continue;
					}
					kept.push(words.join(" "));
				}
				if !kept.is_empty() {
					text.splice(open + 1..close_at, kept.join(" ").chars());
					changed = true;
					break;
				}
			}
			let mut close_end = close_at + 1;
			if text.get(close_end) == Some(&' ') && (open == 0 || text[open - 1] == ' ') {
				close_end += 1;
			}
			text.drain(open..close_end);
			changed = true;
			break;
		}
		if !changed {
			break;
		}
	}
	text.into_iter().collect()
}

/// A quote may front the marks of a dialogue line: it is set aside so the marks
/// are still read as the opener, then put back in front of the result.
/// The opening quote that expects `close`, and only a quote: a bracket keeps
/// the em dash, because a bracket is not speech.
fn quote_open(close: char) -> Option<char> {
	if !is_quote(close) {
		return None;
	}
	match close {
		'"' | '\'' => Some(close),
		_ => matching_opener(close),
	}
}

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
	// The closer need not be the last character: a line may close its speech
	// and keep talking.
	let closes = quote_close(quote).is_some_and(|close| rest.contains(close));
	// The closing marker names the family to keep, and it has to be the
	// counterpart of the opener: a half-quoted line is not a quotation.
	if !closes {
		// The line closes on some other quote: keep a quote, the one that expects it.
		if let Some(open) = line.trim_end().chars().next_back().and_then(quote_open) {
			let mut out = String::from(open);
			if lead.ends_with(' ') {
				out.push(' ');
			}
			out.push_str(rest);
			return out;
		}
		// The closer answered the quote we just threw away, so it would hang.
		let kept = rest.trim_end();
		let kept = match kept.chars().next_back() {
			Some(ch) if !is_quote(ch) && matching_opener(ch).is_some() => {
				&kept[..kept.len() - ch.len_utf8()]
			}
			_ => kept,
		};
		let mut out = String::from("\u{2014}");
		if lead.ends_with(' ') {
			out.push(' ');
		}
		out.push_str(kept);
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
		.take(4)
		.count()
}

/// A domain label may carry a hyphen, so the token run includes it.
fn is_word_char(ch: char) -> bool {
	(ch.is_alphanumeric() || ch == '-') && is_prose_char(ch)
}

/// The closer of a bracket, never of a quote: a quoted line is speech to keep,
/// and the dead-pair rule reads the same distinction from the same list.
fn bracket_close(open: char) -> Option<char> {
	if is_quote(open) {
		return None;
	}
	pair_close(open)
}
fn matching_pair_index(
	chars: &[char],
	indices: impl Iterator<Item = usize>,
	first: char,
	second: char,
) -> Option<usize> {
	let mut depth = 0usize;
	for index in indices {
		if chars[index] == first {
			depth += 1;
		} else if chars[index] == second {
			depth = depth.saturating_sub(1);
			if depth == 0 {
				return Some(index);
			}
		}
	}
	None
}
/// The bracket group that touches a token holding a letter from another script:
/// after it, before it, or glued to it. A credit often names itself again in
/// that bracket, and the words there are prose, so the word-by-word pass would
/// keep them and leave a fragment behind.
fn touching_bracket(chars: &[char]) -> Option<(usize, usize)> {
	let mut at = 0;
	while at < chars.len() {
		if chars[at].is_whitespace() {
			at += 1;
			continue;
		}
		let start = at;
		while at < chars.len() && !chars[at].is_whitespace() && bracket_close(chars[at]).is_none() {
			at += 1;
		}
		if at == start {
			at += 1;
			continue;
		}
		if !chars[start..at].iter().copied().any(is_foreign) {
			continue;
		}
		let mut probe = at;
		while probe < chars.len() && (chars[probe].is_whitespace() || is_sign(chars[probe])) {
			probe += 1;
		}
		if let Some(open) = chars.get(probe).copied()
			&& let Some(close) = bracket_close(open)
			&& let Some(end) = matching_pair_index(chars, probe.., open, close)
		{
			return Some((probe, end + 1));
		}
		let mut back = start;
		while back > 0 && (chars[back - 1].is_whitespace() || is_sign(chars[back - 1])) {
			back -= 1;
		}
		if back == 0 || is_quote(chars[back - 1]) {
			continue;
		}
		let closer = chars[back - 1];
		if !chars[back - 1].is_ascii() || chars[start..at].iter().any(|ch| ch.is_ascii_digit()) {
			continue;
		}
		let Some(open) = matching_opener(closer) else {
			continue;
		};
		if let Some(start) = matching_pair_index(chars, (0..back).rev(), closer, open) {
			return Some((start, back));
		}
	}
	None
}

/// A single glyph between parentheses is the text quoting a character, not a
/// credit spelling itself, so the word keeps its shape.
fn holds_quoted_symbol(word: &str) -> bool {
	let chars: Vec<char> = word.chars().collect();
	let mut at = 0;
	while at < chars.len() {
		if chars[at] == '('
			&& chars.get(at + 1).is_some_and(|ch| !is_prose_char(*ch))
			&& chars.get(at + 2) == Some(&')')
		{
			return true;
		}
		at += 1;
	}
	false
}

fn clean_credit_sentence(text: &str) -> Option<String> {
	// A sign is decoration: it goes and the word stays. A run of signs between
	// two letters becomes one space, so the words it separated stay two words.
	// A letter from another script is the credit's own spelling: the word goes.
	// A credit often names itself again in a bracket right after its own
	// spelling. The words there are prose, so they have to go before the
	// word-by-word pass decides to keep them.
	let mut text: Vec<char> = text.chars().collect();
	while let Some((start, end)) = touching_bracket(&text) {
		text.drain(start..end);
	}
	let text: String = text.into_iter().collect();
	let words: Vec<&str> = text.split_whitespace().collect();
	let internal_dot = words.iter().enumerate().any(|(index, word)| {
		let dots = word.chars().filter(|ch| *ch == '.').count();
		word.contains('.') && !(index + 1 == words.len() && word.ends_with('.') && dots == 1)
	});
	// A stray foreign letter in a sentence of prose is a decoration; several of
	// them in the same sentence are the credit spelling itself.
	let scattered =
		text.split_whitespace()
			.filter(|word| word.chars().any(is_foreign))
			.count() == 1;
	let mut kept: Vec<String> = Vec::new();
	for word in text.split_whitespace() {
		if holds_quoted_symbol(word) {
			kept.push(word.into());
			continue;
		}
		let chars: Vec<char> = word.chars().collect();
		let mut clean = String::default();
		let mut at = 0;
		while at < chars.len() {
			if !is_sign(chars[at]) {
				clean.push(chars[at]);
				at += 1;
				continue;
			}
			let from = at;
			while at < chars.len() && is_sign(chars[at]) {
				at += 1;
			}
			let before = from
				.checked_sub(1)
				.and_then(|i| chars.get(i))
				.is_some_and(|prev| prev.is_alphanumeric());
			let after = chars.get(at).is_some_and(|next| next.is_alphanumeric());
			if before && after {
				clean.push(' ');
			}
		}
		if clean.is_empty() {
			continue;
		}
		if clean.chars().any(is_foreign) {
			let foreign_count = clean.chars().filter(|ch| is_foreign(*ch)).count();
			if foreign_count == 1 && !internal_dot && scattered {
				let prose: String = clean.chars().filter(|ch| is_prose_char(*ch)).collect();
				if !prose.is_empty() {
					kept.push(prose);
				}
			}
			continue;
		}
		kept.push(clean);
	}
	(!kept.is_empty()).then(|| kept.join(" "))
}

fn clean_line(line: &str) -> Option<String> {
	let dialogue = dialogue_line(line);
	let cleaned = without_dead_pairs(&dialogue);
	let line = cleaned.as_str();
	if line.trim().is_empty() {
		return None;
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
