//! IndoPak view rules (PARITY B6; port of `web/src/lib/quran/view/indopak.ts`):
//! split an IndoPak verse into Quran.com-style word boxes, its final word,
//! and the end sign + annotations that ride with the verse number. Every
//! piece is a verbatim slice: concatenating them gives the row back.
//!
//! Offsets here are UTF-8 byte offsets into the verse (the web's are UTF-16
//! indexes; the partition is the same).

use std::sync::OnceLock;

use regex::Regex;

/// JS `\s`, spelled out so Rust matches the web byte for byte.
const JS_SPACE: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";
/// End signs the verse may close with.
const END_SIGNS: &str = r"\x{E01A}-\x{E01C}\x{E01E}\x{E01F}\x{E022}";
/// Inline annotation marks a cluster starts with (no ruku sign).
const BODY_SIGNS: &str =
    r"\x{E01A}-\x{E01C}\x{E01E}\x{E01F}\x{E021}\x{0614}\x{0615}\x{06D6}-\x{06DC}";
/// Annotation marks of an end suffix (ruku sign included).
const ANNOTATION_SIGNS: &str =
    r"\x{E01A}-\x{E01C}\x{E01E}\x{E01F}\x{E021}\x{E022}\x{0614}\x{0615}\x{06D6}-\x{06DC}";
const STOP_SIGNS: &str = r"\x{0615}\x{0617}\x{06D6}-\x{06DC}\x{06E2}\x{06E5}\x{06EA}\x{06EB}\x{E01A}-\x{E01C}\x{E01E}\x{E01F}\x{E021}\x{E022}";
const RUKU_SIGN: char = '\u{E022}';

struct Patterns {
    ending: Regex,
    sign: Regex,
    annotation: Regex,
    cluster: Regex,
    stop: Regex,
    trailing_space: Regex,
    trailing_mark_space: Regex,
    token: Regex,
    zwsp_tail: Regex,
    lead: Regex,
    letter: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let re = |p: String| Regex::new(&p).expect("indopak pattern");
        Patterns {
            ending: re(format!(
                r"([{END_SIGNS}][{END_SIGNS}\p{{M}}\p{{Cf}}{JS_SPACE}]*)$"
            )),
            sign: re(format!(r"^[{END_SIGNS}]\p{{Cf}}*")),
            annotation: re(format!(r"[{ANNOTATION_SIGNS}][^{ANNOTATION_SIGNS}]*")),
            cluster: re(format!(
                r"[{BODY_SIGNS}](?:[\p{{M}}\p{{Cf}}{JS_SPACE}]*[{BODY_SIGNS}])+[\p{{M}}\p{{Cf}}]*"
            )),
            stop: re(format!(r"[{STOP_SIGNS}]")),
            trailing_space: re(format!(r"[{JS_SPACE}]*$")),
            trailing_mark_space: re(format!(r"[{JS_SPACE}][{JS_SPACE}\p{{Cf}}]*$")),
            token: re(format!(r"[{JS_SPACE}]+|[^{JS_SPACE}]+")),
            zwsp_tail: re(r"\x{200B}[^\p{L}]*".to_owned()),
            lead: re(r"^[^\p{L}]*".to_owned()),
            letter: re(r"\p{L}".to_owned()),
        }
    })
}

/// One annotation mark in an end suffix or inline cluster.
#[derive(Debug, Clone, PartialEq)]
pub struct EndAnnotation {
    pub text: String,
    pub offset: usize,
    /// `text` without its trailing source whitespace.
    pub mark: String,
    /// That whitespace: kept, drawn zero-width so stacked signs stay together.
    pub space: String,
    /// Ink box of a zero-advance mark, in em; `None` for spacing glyphs.
    pub width_em: Option<f64>,
    /// Shift putting the mark's ink at its box start, in em.
    pub indent_em: Option<f64>,
}

/// A run of body text, with annotations when it is a sign cluster.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPart {
    pub text: String,
    pub offset: usize,
    pub annotations: Vec<EndAnnotation>,
}

/// One atomic word box: lines break only between words; pause signs
/// written after a word stay inside its box. `gap` is the whitespace after.
#[derive(Debug, Clone, PartialEq)]
pub struct IndoPakWord {
    pub offset: usize,
    pub parts: Vec<BodyPart>,
    pub gap: String,
    /// Holds a stop sign.
    pub stop: bool,
}

impl IndoPakWord {
    /// The word's text (parts joined; the gap excluded).
    pub fn text(&self) -> String {
        self.parts.iter().map(|p| p.text.as_str()).collect()
    }
}

/// An IndoPak verse split for rendering (`IndoPakEnding`).
/// `body + last_word + sign + following == text`.
#[derive(Debug, Clone, PartialEq)]
pub struct IndoPakEnding {
    pub body: String,
    pub last_word: String,
    pub last_word_text: String,
    pub last_word_space: String,
    pub sign: String,
    pub following: String,
    /// The verse ends a ruku.
    pub ruku: bool,
    pub annotations: Vec<EndAnnotation>,
    pub body_parts: Vec<BodyPart>,
    pub words: Vec<IndoPakWord>,
}

fn ink_em(first: char) -> Option<(f64, f64)> {
    match first as u32 {
        0x0614 => Some((-0.31, 0.258)),
        0x0615 => Some((0.247, 0.516)),
        0x06d6 => Some((-0.212, 0.255)),
        0x06d7 => Some((-0.18, 0.18)),
        0x06d8 => Some((-0.129, 0.085)),
        0x06d9 => Some((-0.157, 0.113)),
        0x06da => Some((-0.146, 0.138)),
        0x06db => Some((-0.054, 0.052)),
        0x06dc => Some((-0.222, 0.217)),
        0xe021 => Some((-0.394, -0.199)),
        0xe022 => Some((-0.077, 0.172)),
        _ => None,
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn split_at_match<'a>(text: &'a str, re: &Regex) -> (&'a str, &'a str) {
    let at = re.find(text).map_or(text.len(), |m| m.start());
    text.split_at(at)
}

fn annotation(text: &str, offset: usize) -> EndAnnotation {
    let (mark, space) = split_at_match(text, &patterns().trailing_mark_space);
    let ink = text.chars().next().and_then(ink_em);
    EndAnnotation {
        text: text.to_owned(),
        offset,
        mark: mark.to_owned(),
        space: space.to_owned(),
        width_em: ink.map(|(s, e)| round3(e - s)),
        indent_em: ink.map(|(s, _)| round3(-s)),
    }
}

fn end_annotations(suffix: &str, base: usize) -> Vec<EndAnnotation> {
    patterns()
        .annotation
        .find_iter(suffix)
        .map(|m| annotation(m.as_str(), base + m.start()))
        .collect()
}

fn body_parts(text: &str) -> Vec<BodyPart> {
    let mut parts = Vec::new();
    let mut offset = 0;
    for m in patterns().cluster.find_iter(text) {
        if m.start() > offset {
            parts.push(BodyPart {
                text: text[offset..m.start()].to_owned(),
                offset,
                annotations: Vec::new(),
            });
        }
        parts.push(BodyPart {
            text: m.as_str().to_owned(),
            offset: m.start(),
            annotations: end_annotations(m.as_str(), 0),
        });
        offset = m.end();
    }
    if offset < text.len() {
        parts.push(BodyPart {
            text: text[offset..].to_owned(),
            offset,
            annotations: Vec::new(),
        });
    }
    parts
}

/// Splits after a U+200B-joined sign tail so the next word gets its own box
/// (`​[^\p{L}]*(?=\p{L})`).
fn joined_segments(piece: &str, offset: usize) -> Vec<(String, usize)> {
    let mut segments = Vec::new();
    let mut start = 0;
    for m in patterns().zwsp_tail.find_iter(piece) {
        // The lookahead: a letter must follow; a greedy non-letter run that
        // reaches the end has none.
        if m.end() >= piece.len() {
            continue;
        }
        if m.start() <= start {
            continue;
        }
        segments.push((piece[start..m.end()].to_owned(), offset + start));
        start = m.end();
    }
    segments.push((piece[start..].to_owned(), offset + start));
    segments
}

struct WordDraft {
    offset: usize,
    parts: Vec<BodyPart>,
    gap: String,
}

fn append_to(word: &mut WordDraft, part: BodyPart) {
    let mut pending = Vec::with_capacity(2);
    if !word.gap.is_empty() {
        let gap = std::mem::take(&mut word.gap);
        pending.push(BodyPart {
            offset: part.offset - gap.len(),
            text: gap,
            annotations: Vec::new(),
        });
    }
    pending.push(part);
    for item in pending {
        match word.parts.last_mut() {
            Some(last) if last.annotations.is_empty() && item.annotations.is_empty() => {
                last.text.push_str(&item.text);
            }
            _ => word.parts.push(item),
        }
    }
}

fn attach(words: &mut Vec<WordDraft>, part: BodyPart) {
    match words.last_mut() {
        Some(current) => append_to(current, part),
        None => words.push(WordDraft {
            offset: part.offset,
            parts: vec![part],
            gap: String::new(),
        }),
    }
}

fn piece(words: &mut Vec<WordDraft>, segment: &str, offset: usize) {
    let lead = patterns().lead.find(segment).map_or("", |m| m.as_str());
    if lead.len() == segment.len() || words.is_empty() {
        attach(
            words,
            BodyPart {
                text: segment.to_owned(),
                offset,
                annotations: Vec::new(),
            },
        );
        return;
    }
    if !lead.is_empty() {
        attach(
            words,
            BodyPart {
                text: lead.to_owned(),
                offset,
                annotations: Vec::new(),
            },
        );
    }
    let start = offset + lead.len();
    words.push(WordDraft {
        offset: start,
        parts: vec![BodyPart {
            text: segment[lead.len()..].to_owned(),
            offset: start,
            annotations: Vec::new(),
        }],
        gap: String::new(),
    });
}

fn is_js_space_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

fn segment_words(parts: Vec<BodyPart>) -> Vec<WordDraft> {
    let mut words: Vec<WordDraft> = Vec::new();
    for part in parts {
        if !part.annotations.is_empty() {
            attach(&mut words, part);
            continue;
        }
        for m in patterns().token.find_iter(&part.text) {
            let offset = part.offset + m.start();
            let starts_space = m.as_str().starts_with(is_js_space_char);
            if starts_space && let Some(current) = words.last_mut() {
                current.gap.push_str(m.as_str());
                continue;
            }
            for (segment, seg_offset) in joined_segments(m.as_str(), offset) {
                piece(&mut words, &segment, seg_offset);
            }
        }
    }
    words
}

fn to_word(word: WordDraft) -> IndoPakWord {
    let stop = word.parts.iter().any(|p| patterns().stop.is_match(&p.text));
    IndoPakWord {
        offset: word.offset,
        parts: word.parts,
        gap: word.gap,
        stop,
    }
}

/// Splits an IndoPak verse (`indopakEnding`).
pub fn indopak_ending(text: &str) -> IndoPakEnding {
    let p = patterns();
    let ending = p.ending.find(text);
    let suffix = ending.map_or("", |m| m.as_str());
    let before_suffix = &text[..ending.map_or(text.len(), |m| m.start())];
    let mut drafts = segment_words(body_parts(before_suffix));
    let final_is_word = drafts.last().is_some_and(|last| {
        let joined: String = last.parts.iter().map(|x| x.text.as_str()).collect();
        p.letter.is_match(&joined)
    });
    let final_start = match (final_is_word, drafts.last()) {
        (true, Some(last)) => last.offset,
        _ => before_suffix.len(),
    };
    if final_is_word {
        drafts.pop();
    }
    let body = &before_suffix[..final_start];
    let last_word = &before_suffix[final_start..];
    let (last_word_text, last_word_space) = split_at_match(last_word, &p.trailing_space);
    let sign = p.sign.find(suffix).map_or("", |m| m.as_str());
    IndoPakEnding {
        body: body.to_owned(),
        last_word: last_word.to_owned(),
        last_word_text: last_word_text.to_owned(),
        last_word_space: last_word_space.to_owned(),
        sign: sign.to_owned(),
        following: suffix[sign.len()..].to_owned(),
        ruku: suffix.starts_with(RUKU_SIGN),
        annotations: end_annotations(suffix, 0),
        body_parts: body_parts(body),
        words: drafts.into_iter().map(to_word).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The web's fixtures (indopak-ayah.test.ts), as escapes.
    const CASES: [&str; 13] = [
        "\u{064a}\u{064e}\u{0639}\u{06e1}\u{0644}\u{064e}\u{0645}\u{064f}\u{0648}\u{06e1}\u{0646}\u{064e}\u{E01A}\u{200F}",
        "\u{0627}\u{0644}\u{0636}\u{0651}\u{064e}\u{0627}\u{0653}\u{0644}\u{0651}\u{0650}\u{064a}\u{06e1}\u{0646}\u{064e}\u{E022}",
        "\u{0645}\u{064f}\u{0624}\u{06e1}\u{0645}\u{0650}\u{0646}\u{0650}\u{064a}\u{06e1}\u{0646}\u{064e}\u{E022}\u{0615}",
        "\u{0639}\u{064e}\u{0644}\u{064e}\u{064a}\u{06e1}\u{0647}\u{0650}\u{0645}\u{06e1} \u{E021}\u{06d9} \u{063a}\u{064e}\u{064a}\u{06e1}\u{0631}\u{0650}",
        "\u{0644}\u{0650}\u{064a}\u{064e}\u{0633}\u{064f}\u{0640}\u{0648}\u{0652}\u{06e4}\u{0621}\u{E004}\u{0627}",
        "\u{0639}\u{064e}\u{0638}\u{0650}\u{064a}\u{06e1}\u{0645}\u{064c} \u{E022}",
        "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0627}\u{0644}\u{0644}\u{0651}\u{0670}\u{0647}\u{0650}",
        "\u{062a}\u{064e}\u{0633}\u{06e1}\u{0631}\u{064e}\u{062d}\u{064f}\u{0648}\u{06e1}\u{0646}\u{064e}\u{E01B}",
        "\u{0634}\u{0650}\u{064a}\u{06e1}\u{0628}\u{064e}\u{0627}\u{E01C} \u{2002}\u{06d6}",
        "\u{0628}\u{0650}\u{0645}\u{064e}\u{0644}\u{064f}\u{0648}\u{06e1}\u{0645}\u{064d}\u{E01C}\u{E01A}",
        "\u{0627}\u{0644}\u{06e1}\u{062e}\u{064e}\u{0633}\u{0650}\u{0631}\u{0650}\u{064a}\u{06e1}\u{0646}\u{064e} \u{E01A}\u{06d9}",
        "\u{0631}\u{0651}\u{064e}\u{062d}\u{0650}\u{064a}\u{06e1}\u{0645}\u{064c} \u{06d9}",
        "\u{0633}\u{064e}\u{0631}\u{0650}\u{064a}\u{06e1}\u{0639}\u{064f} \u{0627}\u{0644}\u{06e1}\u{0639}\u{0650}\u{0642}\u{064e}\u{0627}\u{0628}\u{0650}\u{E01A} \u{06d6} \u{0648}\u{064e}\u{0627}\u{0650}\u{0646}\u{0651}\u{064e}\u{0647}\u{0657} \u{0644}\u{064e}\u{0640}\u{063a}\u{064e}\u{0641}\u{064f}\u{0648}\u{06e1}\u{0631}\u{064c} \u{0631}\u{0651}\u{064e}\u{062d}\u{0650}\u{064a}\u{06e1}\u{0645}\u{064c}\u{E022}",
    ];

    fn joined_body(e: &IndoPakEnding) -> String {
        e.body_parts.iter().map(|p| p.text.as_str()).collect()
    }

    #[test]
    fn preserves_codepoints_and_separates_only_the_end() {
        let letter = Regex::new(r"\p{L}").unwrap();
        for text in CASES {
            let e = indopak_ending(text);
            assert_eq!(
                format!("{}{}{}{}", e.body, e.last_word, e.sign, e.following),
                text
            );
            assert!(letter.is_match(&e.last_word), "{text}");
            let ann: String = e.annotations.iter().map(|a| a.text.as_str()).collect();
            assert_eq!(ann, format!("{}{}", e.sign, e.following));
            assert_eq!(joined_body(&e), e.body);
        }
        let e0 = indopak_ending(CASES[0]);
        assert_eq!(
            e0.last_word,
            "\u{064a}\u{064e}\u{0639}\u{06e1}\u{0644}\u{064e}\u{0645}\u{064f}\u{0648}\u{06e1}\u{0646}\u{064e}"
        );
        assert_eq!(e0.sign, "\u{E01A}\u{200F}");
        let e2 = indopak_ending(CASES[2]);
        assert_eq!(
            (e2.sign.as_str(), e2.following.as_str()),
            ("\u{E022}", "\u{0615}")
        );
        assert!(e2.ruku);
        assert_eq!(indopak_ending(CASES[3]).sign, "");
        assert_eq!(indopak_ending(CASES[9]).annotations.len(), 2);
        let paired = indopak_ending(CASES[12]);
        let inline = paired
            .body_parts
            .iter()
            .find(|p| !p.annotations.is_empty())
            .unwrap();
        assert_eq!(inline.text, "\u{E01A} \u{06d6}");
        let last = inline.annotations.last().unwrap();
        assert_eq!((last.width_em, last.indent_em), (Some(0.467), Some(0.212)));
    }

    #[test]
    fn isolates_every_mark_in_inline_pairs() {
        for pair in [
            "\u{06D9}\u{E01C}",
            "\u{06DA}\u{E01A}",
            "\u{E021}\u{06D9}",
            "\u{E01E}\u{0614}",
        ] {
            let text = format!(
                "\u{0642}\u{064e}\u{0627}\u{0644}\u{064e}{pair} \u{062b}\u{064f}\u{0645}\u{0651}\u{064e} \u{0642}\u{064e}\u{0627}\u{0644}\u{064e}"
            );
            let e = indopak_ending(&text);
            let cluster = e.words[0]
                .parts
                .iter()
                .find(|p| !p.annotations.is_empty())
                .unwrap();
            let marks: Vec<String> = cluster.annotations.iter().map(|a| a.mark.clone()).collect();
            let expected: Vec<String> = pair.chars().map(String::from).collect();
            assert_eq!(marks, expected);
            assert_eq!(format!("{}{}", joined_body(&e), e.last_word), text);
        }
    }

    #[test]
    fn word_boxes_rebuild_the_body() {
        for text in CASES {
            let e = indopak_ending(text);
            let words: String = e
                .words
                .iter()
                .map(|w| format!("{}{}", w.text(), w.gap))
                .collect();
            assert_eq!(words, e.body);
            assert_eq!(
                format!("{}{}", e.last_word_text, e.last_word_space),
                e.last_word
            );
            assert!(e.words.iter().all(|w| !w.parts.is_empty()));
        }
        let paired = indopak_ending(CASES[12]);
        let texts: Vec<String> = paired.words.iter().map(IndoPakWord::text).collect();
        let tokens: Vec<&str> = CASES[12].split(' ').collect();
        assert_eq!(
            texts,
            vec![
                tokens[0].to_owned(),
                format!("{} {}", tokens[1], tokens[2]),
                tokens[3].to_owned(),
                tokens[4].to_owned()
            ]
        );
        let stops: Vec<bool> = paired.words.iter().map(|w| w.stop).collect();
        assert_eq!(stops, vec![false, true, false, false]);
    }

    #[test]
    fn zwsp_joined_signs_stay_with_the_preceding_word() {
        let standalone = indopak_ending(
            "\u{0627}\u{0644}\u{0633}\u{064e}\u{0651}\u{0645}\u{064e}\u{0622}\u{0621}\u{064f}\u{200B} \u{0615} \u{0628}\u{064e}\u{0646}\u{0670}\u{0647}\u{064e}\u{0627}\u{E01F}",
        );
        let texts: Vec<String> = standalone.words.iter().map(IndoPakWord::text).collect();
        assert_eq!(
            texts,
            vec![
                "\u{0627}\u{0644}\u{0633}\u{064e}\u{0651}\u{0645}\u{064e}\u{0622}\u{0621}\u{064f}\u{200B} \u{0615}"
            ]
        );
        assert!(standalone.words[0].stop);
        let joined = indopak_ending(
            "\u{0644}\u{0650}\u{0627}\u{064e}\u{0646}\u{06e1}\u{0641}\u{064f}\u{0633}\u{0650}\u{0643}\u{064f}\u{0645}\u{06e1}\u{200B}\u{E01E}\u{0648}\u{064e}\u{0627}\u{0650}\u{0646}\u{06e1} \u{0627}\u{064e}\u{0633}\u{064e}\u{0627}\u{06e1}\u{062a}\u{064f}\u{0645}\u{06e1} \u{0641}\u{064e}\u{0644}\u{064e}\u{0647}\u{064e}\u{0627}",
        );
        let texts: Vec<String> = joined.words.iter().map(IndoPakWord::text).collect();
        assert_eq!(
            texts,
            vec![
                "\u{0644}\u{0650}\u{0627}\u{064e}\u{0646}\u{06e1}\u{0641}\u{064f}\u{0633}\u{0650}\u{0643}\u{064f}\u{0645}\u{06e1}\u{200B}\u{E01E}",
                "\u{0648}\u{064e}\u{0627}\u{0650}\u{0646}\u{06e1}",
                "\u{0627}\u{064e}\u{0633}\u{064e}\u{0627}\u{06e1}\u{062a}\u{064f}\u{0645}\u{06e1}",
            ]
        );
        let last = indopak_ending(
            "\u{0643}\u{064f}\u{0645}\u{06e1}\u{200B}\u{E01E}\u{0648}\u{064e}\u{0627}\u{0650}\u{0646}\u{06e1}",
        );
        assert_eq!(
            last.last_word,
            "\u{0648}\u{064e}\u{0627}\u{0650}\u{0646}\u{06e1}"
        );
        assert_eq!(
            last.body,
            "\u{0643}\u{064f}\u{0645}\u{06e1}\u{200B}\u{E01E}"
        );
    }

    #[test]
    fn end_cluster_whitespace_is_kept() {
        let e = indopak_ending(
            "\u{0627}\u{0644}\u{06e1}\u{0645}\u{064f}\u{0637}\u{06e1}\u{0645}\u{064e}\u{0626}\u{0650}\u{0646}\u{064e}\u{0651}\u{0629}\u{064f}\u{2003}\u{E01C}\u{2003}\u{06D6}",
        );
        assert_eq!(e.last_word_space, "\u{2003}");
        let pairs: Vec<(String, String)> = e
            .annotations
            .iter()
            .map(|a| (a.mark.clone(), a.space.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("\u{E01C}".to_owned(), "\u{2003}".to_owned()),
                ("\u{06D6}".to_owned(), String::new())
            ]
        );
    }

    #[test]
    fn glued_signs_keep_letter_led_boxes() {
        let qala = "\u{0642}\u{064E}\u{0627}\u{0644}\u{064E}";
        let araayta =
            "\u{0627}\u{064E}\u{0631}\u{064E}\u{0621}\u{064E}\u{064A}\u{06E1}\u{062A}\u{064E}";
        let idh = "\u{0627}\u{0650}\u{0630}\u{06E1}";
        let cases: Vec<(String, Vec<String>, &str)> = vec![
            (
                format!("{qala}\u{200B}{araayta} {idh}"),
                vec![format!("{qala}\u{200B}"), araayta.into()],
                idh,
            ),
            (
                format!("{qala} \u{0615}{araayta} {idh}"),
                vec![format!("{qala} \u{0615}"), araayta.into()],
                idh,
            ),
            (
                format!("{qala} \u{200B}\u{06DA}{araayta} {idh}"),
                vec![format!("{qala} \u{200B}\u{06DA}"), araayta.into()],
                idh,
            ),
            (
                format!("{qala} \u{E01B}{araayta} {idh}"),
                vec![format!("{qala} \u{E01B}"), araayta.into()],
                idh,
            ),
            (
                format!("{qala} \u{E01B}\u{E01E}{araayta} {idh}"),
                vec![format!("{qala} \u{E01B}\u{E01E}"), araayta.into()],
                idh,
            ),
            (
                format!("{qala}\u{E021} \u{E01B}\u{2003} \u{06D9}{araayta} {idh}"),
                vec![
                    format!("{qala}\u{E021} \u{E01B}\u{2003} \u{06D9}"),
                    araayta.into(),
                ],
                idh,
            ),
            (
                format!("{qala} \u{06E8}{araayta} {idh}"),
                vec![format!("{qala} \u{06E8}"), araayta.into()],
                idh,
            ),
            (
                format!("{qala} \u{06E8}{araayta}"),
                vec![format!("{qala} \u{06E8}")],
                araayta,
            ),
            (
                format!("{qala} \u{E01C}{araayta}"),
                vec![format!("{qala} \u{E01C}")],
                araayta,
            ),
            (
                format!("\u{200B} \u{200B} {qala} {idh}"),
                vec!["\u{200B} \u{200B}".into(), qala.into()],
                idh,
            ),
        ];
        let letter = Regex::new(r"^\p{L}").unwrap();
        for (text, words, last) in cases {
            let e = indopak_ending(&text);
            let boxes: Vec<String> = e.words.iter().map(IndoPakWord::text).collect();
            assert_eq!(boxes, words, "{text:?}");
            assert_eq!(e.last_word_text, last);
            assert_eq!(
                format!("{}{}{}{}", e.body, e.last_word, e.sign, e.following),
                text
            );
            for b in boxes.iter().skip(1) {
                assert!(letter.is_match(b));
            }
        }
        let stops = |t: String| {
            indopak_ending(&t)
                .words
                .iter()
                .map(|w| w.stop)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            stops(format!("{qala} \u{0615}{araayta} {idh}")),
            vec![true, false]
        );
        assert_eq!(
            stops(format!("{qala} \u{E01B}\u{E01E}{araayta} {idh}")),
            vec![true, false]
        );
        assert_eq!(
            stops(format!("{qala} \u{06E8}{araayta} {idh}")),
            vec![false, false]
        );
        let fin = indopak_ending(&format!("{qala} {araayta} \u{200B}\u{06DA}"));
        assert_eq!(fin.last_word_text, format!("{araayta} \u{200B}\u{06DA}"));
        assert_eq!(fin.last_word_space, "");
        let inline = indopak_ending(&format!(
            "{qala}\u{200B}\u{E01E} \u{200B}\u{06DA} {araayta} {idh}"
        ));
        let cluster = inline.words[0]
            .parts
            .iter()
            .find(|p| !p.annotations.is_empty())
            .unwrap();
        assert_eq!(cluster.text, "\u{E01E} \u{200B}\u{06DA}");
        let pairs: Vec<(String, String)> = cluster
            .annotations
            .iter()
            .map(|a| (a.mark.clone(), a.space.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("\u{E01E}".to_owned(), " \u{200B}".to_owned()),
                ("\u{06DA}".to_owned(), String::new())
            ]
        );
        let a = &indopak_ending("\u{0628}\u{0650}\u{0645}\u{064E}\u{0644}\u{064F}\u{0648}\u{06E1}\u{0645}\u{064D}\u{E01A}\u{200F}").annotations[0];
        assert_eq!(
            (a.mark.as_str(), a.space.as_str()),
            ("\u{E01A}\u{200F}", "")
        );
    }
}
