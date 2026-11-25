//! Match characters rather than wording: the injected credit rotates through Unicode look-alikes.
//!
//! `markdown.rs` calls `strip` once per paragraph when the `hide_watermark`
//! setting is on. Three definitions carry every decision below: prose is the
//! blocks `PROSE_BLOCKS` names plus the whitespace of any script; a sign is
//! neither prose nor alphanumeric; a foreign character is outside prose and
//! alphanumeric, which is how the credit spells itself.
//!
//! The rules run in this order, one block at a time:
//!
//! 1. `strip_injected` removes a credit spelled out letter by letter, keeping
//!    the prose around it and snapping the gap it leaves to one space or a
//!    step of four, so indentation keeps its rung;
//! 2. `dialogue_line` keeps the quote a dialogue line opens with, and leaves an
//!    em dash where the marks answer nothing;
//! 3. `without_dead_pairs` removes the bracket groups that hold no prose, and
//!    keeps the sentences inside one that holds a credit;
//! 4. `sentences` cuts what is left into sentences, so a credit can go without
//!    the sentence beside it;
//! 5. `clean_line` decides each sentence on its own: a site name takes the
//!    sentence, plain prose stays, and a credit is cleaned word by word;
//! 6. `strip` glues the kept sentences back together.
//!
//! Two constraints come from the corpus and look like bugs otherwise. A domain
//! names its extension in full, so `focusboost.sh:` in a script log stays a
//! line of the story; and the extension of a real credit is styled like the
//! rest of it, so a look-alike there still counts as a letter.
//!
//! This file is copied byte for byte into the other source: change one copy,
//! copy it over, and check the two still match.

use aidoku::alloc::{String, Vec};

/// The blocks a book is written in: the credit styles its words with letters
/// from other scripts, but the site censors its own prose with Latin
/// look-alikes, and those belong to the text.
const PROSE_BLOCKS: &[(char, char)] = &[
	('\u{0000}', '\u{007F}'), // Basic Latin
	('\u{0080}', '\u{00FF}'), // Latin-1 Supplement
	('\u{0100}', '\u{024F}'), // Latin Extended-A and B
	('\u{2000}', '\u{206F}'), // General Punctuation
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

/// The punctuation a sentence keeps between its ends: a pause, not a stop. The
/// credit reads it as prose, so it comes back with the sentence instead of
/// leaving with the marks.
const SENTENCE_PAUSES: &[char] = &[',', ';', ':'];

/// A line break ends a line, so it never separates two words.
const LINE_BREAKS: &[char] = &['\n', '\r'];

/// Prose is the blocks of `PROSE_BLOCKS`, and whitespace from anywhere: a gap
/// between two words is not a mark the credit left.
fn is_prose_char(ch: char) -> bool {
	ch.is_whitespace()
		|| PROSE_BLOCKS
			.iter()
			.any(|(start, end)| (*start..=*end).contains(&ch))
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
fn is_noisy(text: impl IntoIterator<Item = char>) -> bool {
	text.into_iter().any(|ch| !is_prose_char(ch))
}

fn is_separator(ch: char) -> bool {
	!ch.is_alphanumeric() && !LINE_BREAKS.contains(&ch)
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

/// The other half of a quotation mark: an ASCII quote closes on itself, every
/// other quote takes its counterpart from `PAIRS`. The direction matters: an
/// opener is paired with its closer, a closer with its opener, and the two
/// answers differ for `\u{201C}` or `\u{AB}`.
fn quote_counterpart(quote: char, from_opener: bool) -> Option<char> {
	if !is_quote(quote) {
		return None;
	}
	if quote.is_ascii() {
		return Some(quote);
	}
	if from_opener {
		pair_close(quote)
	} else {
		matching_opener(quote)
	}
}

/// How many bytes the leading run of signs takes.
fn leading_sign_len(text: &str) -> usize {
	text.len() - text.trim_start_matches(is_sign).len()
}

/// Where the trailing run of signs starts.
fn trailing_sign_start(text: &str) -> usize {
	text.trim_end_matches(is_sign).len()
}

fn spelled_run(text: &[char], start: usize) -> Option<usize> {
	if !text.get(start).is_some_and(|ch| ch.is_alphabetic()) {
		return None;
	}
	if start > 0 && text[start - 1].is_alphanumeric() {
		return None;
	}
	// An apostrophe belongs to the word it follows, so the run must not start
	// inside a contraction.
	if start > 1 && is_quote(text[start - 1]) && text[start - 2].is_alphabetic() {
		return None;
	}
	if !is_separator(*text.get(start + 1)?) {
		return None;
	}
	let mut at = start + 1;
	let mut letters = 1;
	let mut end = at;
	let mut spaced: Option<bool> = None;
	loop {
		let before = at;
		let mut blanks = 0;
		while text
			.get(at)
			.is_some_and(|ch| is_separator(*ch) && pair_close(*ch).is_none())
		{
			if text[at].is_whitespace() {
				blanks += 1;
			}
			at += 1;
		}
		if at == before {
			break;
		}
		let spaced_here = blanks > 0;
		match spaced {
			None => spaced = Some(spaced_here),
			Some(first) if first != spaced_here => break,
			_ => {}
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
fn count_marks(side: impl Iterator<Item = char>) -> usize {
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
		let left = count_marks(output.chars().rev());
		let right = count_marks(text[span..].iter().copied());
		// A credit wears marks on both sides of the letters it spells. Without a
		// mark on each side, the run is prose that happens to hold separators.
		if left == 0 || right == 0 {
			output.push(text[index]);
			index += 1;
			continue;
		}
		for _ in 0..left {
			output.pop();
		}
		let mut after = 0;
		while text.get(span + right + after) == Some(&' ') {
			after += 1;
		}
		let resume = span + right + after;
		let trailing = output.chars().rev().take_while(|ch| *ch == ' ').count();
		let gap = trailing + after;
		// Snap the gap around an injection down to its nearest bound: one space,
		// then steps of four, so indentation and alignment keep their rung.
		let bound = if gap < 4 { 1 } else { gap / 4 * 4 };
		while output.ends_with(' ') {
			output.pop();
		}
		if text.get(resume).is_some_and(|ch| !ch.is_whitespace()) {
			for _ in 0..bound {
				output.push(' ');
			}
		}
		index = resume;
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
	while start < end && is_noisy(words[start].chars()) {
		start += 1;
	}
	while end > start && is_noisy(words[end - 1].chars()) {
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
			if bracket_close(*ch).is_some() {
				stack.push(at);
				continue;
			}
			if let Some(&open) = stack.last()
				&& bracket_close(text[open]) == Some(*ch)
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
			let foreign = is_noisy(content.iter().copied());
			// An empty pair is prose the site left behind, as in `Your
			// Contribution: <>`: no letter of a credit is in it.
			if content.len() <= 1 || !foreign {
				continue;
			}
			let inner: String = content.iter().collect();
			let mut kept: Vec<String> = Vec::new();
			for sentence in sentences(&inner) {
				let words = trim_noisy_walls(&sentence);
				if words.is_empty() || words.iter().any(|word| is_noisy(word.chars())) {
					continue;
				}
				kept.push(words.join(" "));
			}
			if !kept.is_empty() {
				text.splice(open + 1..close_at, kept.join(" ").chars());
				changed = true;
				break;
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
fn dialogue_line(line: &str) -> String {
	let lead_end = line.len()
		- line
			.trim_start_matches(|ch| is_sign(ch) || is_quote(ch) || ch == ' ')
			.len();
	let lead = &line[..lead_end];
	let Some(quote) = lead.chars().find(|ch| is_quote(*ch)) else {
		return dialogue_marks(line);
	};
	if !lead.chars().any(is_sign) {
		return dialogue_marks(line);
	}
	let rest = &line[lead_end..];
	// The closer need not be the last character: a line may close its speech
	// and keep talking.
	let closes = quote_counterpart(quote, true).is_some_and(|close| rest.contains(close));
	// The closing marker names the family to keep, and it has to be the
	// counterpart of the opener: a half-quoted line is not a quotation.
	if !closes {
		// The line closes on some other quote: keep a quote, the one that expects it.
		if let Some(open) = line
			.trim_end()
			.chars()
			.next_back()
			.and_then(|close| quote_counterpart(close, false))
		{
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
				kept.strip_suffix(ch).unwrap_or(kept)
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
	let mark_end = leading_sign_len(line);
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
		let end = leading_sign_len(trimmed);
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

/// True when the sentence carries a domain-shaped token: a dot, a letter, a
/// digit or a hyphen before it, and at least two lowercase letters after it,
/// fewer than the label carries before the dot.
fn has_domain(sentence: &str) -> bool {
	let chars: Vec<char> = sentence.chars().collect();
	(0..chars.len()).any(|at| {
		chars[at] == '.'
			&& at > 0
			&& is_prose_char(chars[at - 1])
			&& is_domain_char(chars[at - 1])
			&& {
				let tail = chars
					.iter()
					.skip(at + 1)
					.take_while(|ch| ch.is_ascii_lowercase())
					.count();
				let head = chars[..at]
					.iter()
					.rev()
					.take_while(|ch| is_domain_char(**ch))
					.count();
				// The tail carries the extension. Three letters make it one, and a
				// shorter one only counts when it ends the sentence there: a file
				// name like `focusboost.sh:` is followed by more of the sentence.
				// The credit styles its own letters, so a look-alike after the
				// extension does not reopen the sentence.
				tail > 1
					&& tail < head
					&& (tail > 2
						|| chars[at + 1 + tail..]
							.iter()
							.all(|ch| SENTENCE_ENDS.contains(ch) || is_foreign(*ch)))
			}
	})
}

/// A label of a domain: letters, digits and hyphens, as in `novel-fire`.
fn is_domain_char(ch: char) -> bool {
	ch.is_alphanumeric() || ch == '-'
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
		// The scan stops at the end of the line: a bracket that is never closed
		// answers "no pair" instead of running past the slice.
		if let Some(open) = chars.get(probe).copied()
			&& let Some(close) = bracket_close(open)
			&& let Some(end) = matching_pair_index(chars, probe..chars.len(), open, close)
		{
			return Some((probe, end + 1));
		}
		let mut back = start;
		while back > 0 && (chars[back - 1].is_whitespace() || is_sign(chars[back - 1])) {
			back -= 1;
		}
		if back == 0 || !chars[back - 1].is_whitespace() {
			continue;
		}
		let closer = chars[back - 1];
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
	chars.iter().enumerate().any(|(at, ch)| {
		bracket_close(*ch).is_some_and(|close| {
			chars
				.get(at + 1)
				.is_some_and(|inner| !is_prose_char(*inner))
				&& chars.get(at + 2) == Some(&close)
		})
	})
}

/// A credit wears marks on both sides of its word. With a mark on each side they
/// are its own and leave with it; with one side bare, the mark belongs to the
/// text and stays. The counts need not agree: `##` before and `#` after is still
/// one credit.
fn strip_marked_word(words: &[&str]) -> Vec<String> {
	let mut kept: Vec<String> = Vec::new();
	let mut at = 0;
	while at < words.len() {
		let word = words[at];
		at += 1;
		if word.chars().any(is_foreign) {
			let left = kept
				.iter()
				.rev()
				.take_while(|mark| is_mark_token(mark))
				.count();
			let right = words[at..]
				.iter()
				.take_while(|mark| is_mark_token(mark))
				.count();
			if left > 0 && right > 0 {
				kept.truncate(kept.len() - left);
				// A mark token can carry the sentence's own punctuation, as `#.`
				// does. It is not a mark and comes back against the last word.
				let kept_punctuation: String = words[at..at + right]
					.iter()
					.flat_map(|mark| mark.chars())
					.filter(|ch| !is_credit_mark(*ch))
					.collect();
				at += right;
				if let Some(last) = kept.last_mut() {
					last.push_str(&kept_punctuation);
				}
				continue;
			}
		}
		kept.push(word.into());
	}
	kept
}

/// A mark the credit puts around its word: neither a letter nor a digit, nor
/// the punctuation a sentence keeps.
fn is_credit_mark(ch: char) -> bool {
	!ch.is_alphanumeric() && !is_sentence_punctuation(ch)
}

/// What a sentence keeps: the marks that stop it and the marks that pause it.
fn is_sentence_punctuation(ch: char) -> bool {
	SENTENCE_ENDS.contains(&ch) || SENTENCE_PAUSES.contains(&ch)
}

/// A token that carries at least one credit mark and no letter or digit: `#`,
/// `***`, or a mark still holding the sentence's punctuation, as `#.` does.
/// A bare `,` or `.` is the sentence's, not the credit's.
fn is_mark_token(word: &str) -> bool {
	word.chars().all(|ch| !ch.is_alphanumeric()) && word.chars().any(is_credit_mark)
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
	let words = strip_marked_word(&text.split_whitespace().collect::<Vec<&str>>());
	let mut kept: Vec<String> = Vec::new();
	for word in &words {
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
			continue;
		}
		kept.push(clean);
	}
	(!kept.is_empty()).then(|| kept.join(" "))
}

/// A letter wrapped in parentheses inside a word is a site name broken up to
/// dodge filters: `fr(e)webnovel` spells a domain, it is not prose.
/// A bracket that opens or closes a word: every neighbour that exists is a letter,
/// and at least one neighbour exists. A digit or a space beside it is a stat line
/// or a detached parenthesis, not a domain broken up to dodge filters.
fn bracket_inside_word(chars: &[char], at: usize, close: char) -> bool {
	if chars.get(at + 2) != Some(&close) {
		return false;
	}
	let left = at.checked_sub(1).and_then(|index| chars.get(index));
	let right = chars.get(at + 3);
	(left.is_some() || right.is_some())
		&& left.is_none_or(|ch| ch.is_alphabetic())
		&& right.is_none_or(|ch| ch.is_alphabetic())
		&& !chars.iter().any(|ch| ch.is_numeric())
}

fn has_broken_word(sentence: &str) -> bool {
	sentence.split_whitespace().any(|word| {
		let chars: Vec<char> = word.chars().collect();
		chars.iter().enumerate().any(|(at, ch)| {
			bracket_close(*ch).is_some_and(|close| bracket_inside_word(&chars, at, close))
		})
	})
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
		if has_domain(sentence) || has_broken_word(sentence) {
			continue;
		}
		if !is_noisy(sentence.chars()) {
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

#[cfg(test)]
mod tests {
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn an_unclosed_bracket_after_a_credit_does_not_panic() {
		let cleaned = super::strip("\u{444} (");
		assert!(!cleaned.contains('\u{444}'));
	}

	/// A file name is not a domain: `focusboost.sh: 600 points shifted` is a
	/// line of a script log, and the sentence around it is the story.
	#[aidoku_test]
	fn a_file_name_is_not_a_domain() {
		let cleaned = super::strip("focusboost.sh: 600 points shifted to Focus");
		assert_eq!(cleaned, "focusboost.sh: 600 points shifted to Focus");
	}

	/// The apostrophe of a contraction belongs to the word: the spelled credit
	/// that follows it must not swallow it.
	#[aidoku_test]
	fn a_contraction_keeps_its_apostrophe() {
		let cleaned = super::strip("it wasn't \u{b0}* N  v  l  i  g  h  t *\u{b0} normal");
		assert_eq!(cleaned, "it wasn't normal");
	}

	/// The credit styles the letters of its own extension, so a two-letter
	/// extension still names a domain when a look-alike follows it.
	#[aidoku_test]
	fn a_styled_extension_still_names_a_domain() {
		let cleaned = super::strip("The source of this c\u{1d4f8}ntent is freweovel.co\u{271}.");
		assert_eq!(cleaned, "");
	}
}
