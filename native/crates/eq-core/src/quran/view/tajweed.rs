//! Tajweed markup view (PARITY B6, C5; faithful port of
//! `web/src/lib/quran/view/tajweed.ts`). The tajweed DB carries inline rule
//! segments in `text`, byte for byte: `[h:1468[ٱ]` = `[` + rule letter +
//! optional `:<id>` + `[` + coloured run + `]`, nested at most one level.
//! Parsing happens here only; stored text, wire text and every other source
//! stay untouched. Colours are the UI's: map [`TajweedRule`] to theme tokens
//! (the web's palette is in `tajweed.ts` `RULE_COLORS`).

use crate::quran::types::QuranScript;

/// A tajweed rule letter (the Dar Al-Islam edition's markup keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TajweedRule {
    /// `h` hamzat al-wasl.
    HamzatWasl,
    /// `s` silent letter.
    Silent,
    /// `l` lam shamsiyyah.
    LamShamsiyyah,
    /// `n` madda normal (2).
    MaddaNormal,
    /// `p` madda permissible (2/4/6).
    MaddaPermissible,
    /// `m` madda necessary (6).
    MaddaNecessary,
    /// `q` qalqalah.
    Qalqalah,
    /// `o` madda obligatory (4-5).
    MaddaObligatory,
    /// `c` ikhfa shafawi.
    IkhfaShafawi,
    /// `f` ikhfa.
    Ikhfa,
    /// `w` idgham shafawi.
    IdghamShafawi,
    /// `i` iqlab.
    Iqlab,
    /// `a` idgham with ghunnah.
    IdghamGhunnah,
    /// `u` idgham without ghunnah.
    IdghamNoGhunnah,
    /// `d` idgham mutajanisayn/mutamathilayn.
    IdghamMutajanisayn,
    /// `b` idgham mutaqaribayn.
    IdghamMutaqaribayn,
    /// `g` ghunnah.
    Ghunnah,
}

impl TajweedRule {
    /// Every rule.
    pub const ALL: [TajweedRule; 17] = [
        TajweedRule::HamzatWasl,
        TajweedRule::Silent,
        TajweedRule::LamShamsiyyah,
        TajweedRule::MaddaNormal,
        TajweedRule::MaddaPermissible,
        TajweedRule::MaddaNecessary,
        TajweedRule::Qalqalah,
        TajweedRule::MaddaObligatory,
        TajweedRule::IkhfaShafawi,
        TajweedRule::Ikhfa,
        TajweedRule::IdghamShafawi,
        TajweedRule::Iqlab,
        TajweedRule::IdghamGhunnah,
        TajweedRule::IdghamNoGhunnah,
        TajweedRule::IdghamMutajanisayn,
        TajweedRule::IdghamMutaqaribayn,
        TajweedRule::Ghunnah,
    ];

    /// The markup letter.
    pub fn letter(self) -> char {
        match self {
            TajweedRule::HamzatWasl => 'h',
            TajweedRule::Silent => 's',
            TajweedRule::LamShamsiyyah => 'l',
            TajweedRule::MaddaNormal => 'n',
            TajweedRule::MaddaPermissible => 'p',
            TajweedRule::MaddaNecessary => 'm',
            TajweedRule::Qalqalah => 'q',
            TajweedRule::MaddaObligatory => 'o',
            TajweedRule::IkhfaShafawi => 'c',
            TajweedRule::Ikhfa => 'f',
            TajweedRule::IdghamShafawi => 'w',
            TajweedRule::Iqlab => 'i',
            TajweedRule::IdghamGhunnah => 'a',
            TajweedRule::IdghamNoGhunnah => 'u',
            TajweedRule::IdghamMutajanisayn => 'd',
            TajweedRule::IdghamMutaqaribayn => 'b',
            TajweedRule::Ghunnah => 'g',
        }
    }

    /// The rule a markup letter names.
    pub fn from_letter(letter: u8) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|r| r.letter() as u32 == u32::from(letter))
    }
}

/// One run of verse text: verbatim, with its rule or `None` for plain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TajweedSegment {
    pub text: String,
    pub rule: Option<TajweedRule>,
}

/// Reads `[x[` or `[x:digits[` at `at` (a `[`). Returns the rule and the
/// body's start, or `None` when the bracket is literal text.
fn take_opener(text: &[u8], at: usize) -> Option<(TajweedRule, usize)> {
    let rule = TajweedRule::from_letter(*text.get(at + 1)?)?;
    let mut body = at + 2;
    match text.get(body) {
        Some(b':') => {
            let digits = body + 1;
            let mut end = digits;
            while end < text.len() && text[end].is_ascii_digit() {
                end += 1;
            }
            if end == digits || text.get(end) != Some(&b'[') {
                return None;
            }
            body = end + 1;
        }
        Some(b'[') => body += 1,
        _ => return None,
    }
    Some((rule, body))
}

fn is_bare_arabic(c: char) -> bool {
    matches!(c,
        '\u{0600}'..='\u{06FF}'
        | '\u{0750}'..='\u{077F}'
        | '\u{08A0}'..='\u{08FF}'
        | '\u{200C}'
        | '\u{200D}'
        | '\u{FD70}'..='\u{FDFF}'
        | '\u{FE70}'..='\u{FEFF}')
}

/// `[content]` at `open` with no rule letter and Arabic-only content (the
/// corpus has one: 32:3 `[ٮٰ]`).
fn bare_arabic_group(text: &str, open: usize, close: Option<usize>) -> Option<usize> {
    let close = close?;
    if close == open + 1 {
        return None;
    }
    let body = &text[open + 1..close];
    (!body.contains('[') && body.chars().all(is_bare_arabic)).then_some(close)
}

fn parse_body(
    text: &str,
    cursor: &mut usize,
    segments: &mut Vec<TajweedSegment>,
    group: Option<TajweedRule>,
) {
    let bytes = text.as_bytes();
    let mut plain = String::new();
    let flush = |plain: &mut String, segments: &mut Vec<TajweedSegment>| {
        if !plain.is_empty() {
            segments.push(TajweedSegment {
                text: std::mem::take(plain),
                rule: group,
            });
        }
    };
    while *cursor < bytes.len() {
        let b = bytes[*cursor];
        if b != b'[' && b != b']' {
            let ch = text[*cursor..].chars().next().expect("char boundary");
            plain.push(ch);
            *cursor += ch.len_utf8();
            continue;
        }
        if b == b']' {
            if group.is_none() {
                plain.push(']');
                *cursor += 1;
                continue;
            }
            flush(&mut plain, segments);
            *cursor += 1;
            return;
        }
        let Some((rule, body_start)) = take_opener(bytes, *cursor) else {
            let close = text[*cursor + 1..].find(']').map(|i| i + *cursor + 1);
            if let Some(close) = bare_arabic_group(text, *cursor, close) {
                plain.push_str(&text[*cursor + 1..close]);
                *cursor = close + 1;
                continue;
            }
            plain.push('[');
            *cursor += 1;
            continue;
        };
        if body_start >= bytes.len() {
            plain.push_str(&text[*cursor..body_start.min(bytes.len())]);
            *cursor = body_start;
            continue;
        }
        flush(&mut plain, segments);
        *cursor = body_start;
        parse_body(text, cursor, segments, Some(rule));
    }
    flush(&mut plain, segments);
}

fn coalesce_plain(segments: Vec<TajweedSegment>) -> Vec<TajweedSegment> {
    let mut out: Vec<TajweedSegment> = Vec::with_capacity(segments.len());
    for segment in segments {
        match out.last_mut() {
            Some(last) if last.rule.is_none() && segment.rule.is_none() => {
                last.text.push_str(&segment.text);
            }
            _ => out.push(segment),
        }
    }
    out
}

/// Parses tajweed markup into runs (`parseTajweedSegments`); text with no
/// `[` is one plain run.
pub fn parse_tajweed_segments(text: &str) -> Vec<TajweedSegment> {
    if !text.contains('[') {
        return vec![TajweedSegment {
            text: text.to_owned(),
            rule: None,
        }];
    }
    let mut segments = Vec::new();
    let mut cursor = 0;
    parse_body(text, &mut cursor, &mut segments, None);
    coalesce_plain(segments)
}

/// Markup stripped, for plain-text views only (copy, share, previews,
/// titles); never for stored, wire or displayed text
/// (`stripTajweedMarkup`).
pub fn strip_tajweed_markup(text: &str) -> String {
    if !text.contains('[') {
        return text.to_owned();
    }
    parse_tajweed_segments(text)
        .into_iter()
        .map(|s| s.text)
        .collect()
}

/// Whether a verse renders through the tajweed segments.
pub fn is_tajweed_script(script: QuranScript) -> bool {
    script == QuranScript::Tajweed
}

#[cfg(test)]
mod tests {
    use super::*;
    use TajweedRule as R;

    // Real bytes from db/quran/arabic/quran-tajweed.sqlite, the web test's
    // fixtures (tajweed.test.ts), byte for byte.
    const FATIHAH_1: &str = "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} [h:1[\u{0671}]\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{0650} [h:2[\u{0671}][l[\u{0644}]\u{0631}\u{0651}\u{064E}\u{062D}\u{0652}\u{0645}\u{064E}[n[\u{0640}\u{0670}]\u{0646}\u{0650} [h:3[\u{0671}][l[\u{0644}]\u{0631}\u{0651}\u{064E}\u{062D}[p[\u{0650}\u{064A}]\u{0645}\u{0650}";
    const AYAT_AL_KURSI_EXCERPT: &str = "\u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0644}[o[\u{064E}\u{0627}\u{0653}] \u{0625}\u{0650}\u{0644}\u{064E}[n[\u{0640}\u{0670}]\u{0647}\u{064E} \u{0625}\u{0650}\u{0644}\u{0651}\u{064E}\u{0627} \u{0647}\u{064F}\u{0648}\u{064E} [h:1468[\u{0671}]\u{0644}\u{0652}\u{062D}\u{064E}\u{0649}\u{0651}\u{064F}";
    const NISA_4_158: &str = "\u{0628}\u{064E}[b:3279[\u{0644}] \u{0631}\u{0651}\u{064E}\u{0641}\u{064E}\u{0639}\u{064E}\u{0647}\u{064F} [h:3280[\u{0671}]\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0625}\u{0650}\u{0644}\u{064E}\u{064A}\u{0652}\u{0647}\u{0650}\u{200C}\u{06DA} \u{0648}\u{064E}\u{0643}\u{064E}\u{0627}\u{0646}\u{064E} [h:2653[\u{0671}]\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0639}\u{064E}\u{0632}\u{0650}\u{064A}\u{0632}\u{064B}\u{0627} \u{062D}\u{064E}\u{0643}\u{0650}\u{064A}\u{0645}\u{064B}\u{0627}";
    const MURSALAT_77_20: &str = "\u{0623}\u{064E}\u{0644}\u{064E}\u{0645}\u{0652} \u{0646}\u{064E}\u{062E}\u{0652}\u{0644}\u{064F}[b:14217[\u{0642}]\u{0643}\u{0651}[w:14218[\u{064F}\u{0645} \u{0645}]\u{0651}[a:840[\u{0650}\u{0646} \u{0645}]\u{0651}[o[\u{064E}\u{0627}]\u{0653}[a:10629[\u{0621}\u{064D} \u{0645}]\u{0651}\u{064E}\u{0647}[p[\u{0650}\u{064A}]\u{0646}\u{064D}";
    const AS_SAJDAH_32_3: &str = "\u{0623}\u{064E}\u{0645}\u{0652} \u{064A}\u{064E}\u{0642}\u{064F}\u{0648}\u{0644}\u{064F}\u{0648}\u{0646}\u{064E} [h:6061[\u{0671}]\u{0641}\u{0652}\u{062A}\u{064E}\u{0631}\u{064E}[\u{066E}\u{0670}]\u{0647}\u{064F}\u{200C}\u{06DA} \u{0628}\u{064E}\u{0644}\u{0652} \u{0647}\u{064F}\u{0648}\u{064E} [h:8923[\u{0671}]\u{0644}\u{0652}\u{062D}\u{064E}\u{0642}\u{0651}\u{064F} \u{0645}[u:770[\u{0650}\u{0646} \u{0631}]\u{0651}\u{064E}\u{0628}\u{0651}\u{0650}\u{0643}\u{064E} \u{0644}\u{0650}\u{062A}\u{064F}[f:4582[\u{0646}\u{0630}]\u{0650}\u{0631}\u{064E} \u{0642}\u{064E}\u{0648}\u{0652}[a:10181[\u{0645}\u{064B}\u{0627} \u{0645}]\u{0651}[o[\u{064E}\u{0627}\u{0653}] \u{0623}\u{064E}\u{062A}\u{064E}[n[\u{0640}\u{0670}]\u{0647}[w:10182[\u{064F}\u{0645} \u{0645}]\u{0651}[a:10183[\u{0650}\u{0646} \u{0646}]\u{0651}\u{064E}\u{0630}\u{0650}\u{064A}[a:10184[\u{0631}\u{064D} \u{0645}]\u{0651}\u{0650}[f:18[\u{0646} \u{0642}]\u{064E}[q:19[\u{0628}\u{0652}]\u{0644}\u{0650}\u{0643}\u{064E} \u{0644}\u{064E}\u{0639}\u{064E}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F}\u{0645}\u{0652} \u{064A}\u{064E}\u{0647}\u{0652}\u{062A}\u{064E}\u{062F}[p[\u{064F}\u{0648}]\u{0646}\u{064E}";
    const AS_SAJDAH_32_3_PLAIN: &str = "\u{0623}\u{064E}\u{0645}\u{0652} \u{064A}\u{064E}\u{0642}\u{064F}\u{0648}\u{0644}\u{064F}\u{0648}\u{0646}\u{064E} \u{0671}\u{0641}\u{0652}\u{062A}\u{064E}\u{0631}\u{064E}\u{066E}\u{0670}\u{0647}\u{064F}\u{200C}\u{06DA} \u{0628}\u{064E}\u{0644}\u{0652} \u{0647}\u{064F}\u{0648}\u{064E} \u{0671}\u{0644}\u{0652}\u{062D}\u{064E}\u{0642}\u{0651}\u{064F} \u{0645}\u{0650}\u{0646} \u{0631}\u{0651}\u{064E}\u{0628}\u{0651}\u{0650}\u{0643}\u{064E} \u{0644}\u{0650}\u{062A}\u{064F}\u{0646}\u{0630}\u{0650}\u{0631}\u{064E} \u{0642}\u{064E}\u{0648}\u{0652}\u{0645}\u{064B}\u{0627} \u{0645}\u{0651}\u{064E}\u{0627}\u{0653} \u{0623}\u{064E}\u{062A}\u{064E}\u{0640}\u{0670}\u{0647}\u{064F}\u{0645} \u{0645}\u{0651}\u{0650}\u{0646} \u{0646}\u{0651}\u{064E}\u{0630}\u{0650}\u{064A}\u{0631}\u{064D} \u{0645}\u{0651}\u{0650}\u{0646} \u{0642}\u{064E}\u{0628}\u{0652}\u{0644}\u{0650}\u{0643}\u{064E} \u{0644}\u{064E}\u{0639}\u{064E}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F}\u{0645}\u{0652} \u{064A}\u{064E}\u{0647}\u{0652}\u{062A}\u{064E}\u{062F}\u{064F}\u{0648}\u{0646}\u{064E}";
    const BAQARAH_2_190_TAIL: &str = "\u{0648}\u{064E}\u{0644}\u{064E}\u{0627} \u{062A}\u{064E}\u{0639}\u{0652}\u{062A}\u{064E}\u{062F}[o[\u{064F}\u{0648}\u{0653}[s[\u{0627}\u{0652}]\u{200C}\u{06DA}]";
    const BAQARAH_2_190_TAIL_PLAIN: &str = "\u{0648}\u{064E}\u{0644}\u{064E}\u{0627} \u{062A}\u{064E}\u{0639}\u{0652}\u{062A}\u{064E}\u{062F}\u{064F}\u{0648}\u{0653}\u{0627}\u{0652}\u{200C}\u{06DA}";
    const BAQARAH_2_278_NESTED: &str = "[o[[s[\u{0648}]\u{0672}\u{0653}\u{0627}\u{0652}]";
    const MUHAMMAD_47_31_NESTED: &str = "[o[\u{064E}[s[\u{0627}\u{0652}]]";

    fn seg(text: &str, rule: Option<TajweedRule>) -> TajweedSegment {
        TajweedSegment {
            text: text.to_owned(),
            rule,
        }
    }

    fn joined(segments: &[TajweedSegment]) -> String {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn plain_text_is_one_run() {
        assert_eq!(
            parse_tajweed_segments("بِسْمِ ٱللَّهِ"),
            vec![seg("بِسْمِ ٱللَّهِ", None)]
        );
    }

    #[test]
    fn id_and_bare_rule_forms() {
        assert_eq!(
            parse_tajweed_segments(
                "\u{0671}\u{0644}\u{0644}\u{064E}\u{0651}\u{0647}\u{064F} \u{0644}[o[\u{064E}\u{0627}\u{0653}] \u{0625}\u{0650}\u{0644}\u{064E}[n[\u{0640}\u{0670}]\u{0647}\u{064E}"
            ),
            vec![
                seg("ٱللَّهُ ل", None),
                seg("\u{064E}\u{0627}\u{0653}", Some(R::MaddaObligatory)),
                seg(" إِلَ", None),
                seg("ـٰ", Some(R::MaddaNormal)),
                seg("هَ", None),
            ]
        );
    }

    #[test]
    fn fatihah_without_losing_or_inventing_text() {
        let s = parse_tajweed_segments(FATIHAH_1);
        assert!(s.iter().any(|x| x.rule.is_some()));
        assert_eq!(
            joined(&s),
            "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{0650} \u{0671}\u{0644}\u{0631}\u{0651}\u{064E}\u{062D}\u{0652}\u{0645}\u{064E}\u{0640}\u{0670}\u{0646}\u{0650} \u{0671}\u{0644}\u{0631}\u{0651}\u{064E}\u{062D}\u{0650}\u{064A}\u{0645}\u{0650}"
        );
    }

    #[test]
    fn literal_brackets_stay_plain() {
        assert_eq!(
            parse_tajweed_segments("[2] footnote [x[not a rule]"),
            vec![seg("[2] footnote [x[not a rule]", None)]
        );
    }

    #[test]
    fn unclosed_segment_keeps_text() {
        assert_eq!(
            parse_tajweed_segments("آمن [g[قلب"),
            vec![seg("آمن ", None), seg("قلب", Some(R::Ghunnah))]
        );
    }

    #[test]
    fn nested_groups() {
        assert_eq!(
            parse_tajweed_segments(BAQARAH_2_190_TAIL),
            vec![
                seg("وَلَا تَعْتَد", None),
                seg("ُوٓ", Some(R::MaddaObligatory)),
                seg("اْ", Some(R::Silent)),
                seg("\u{200c}ۚ", Some(R::MaddaObligatory)),
            ]
        );
        assert_eq!(
            parse_tajweed_segments(BAQARAH_2_278_NESTED),
            vec![
                seg("و", Some(R::Silent)),
                seg("ٲٓاْ", Some(R::MaddaObligatory))
            ]
        );
        assert_eq!(
            parse_tajweed_segments(MUHAMMAD_47_31_NESTED),
            vec![seg("َ", Some(R::MaddaObligatory)), seg("اْ", Some(R::Silent))]
        );
        assert_eq!(
            parse_tajweed_segments("[o[ُوٓ[s[اْ]\u{200c}ۚ"),
            vec![
                seg("ُوٓ", Some(R::MaddaObligatory)),
                seg("اْ", Some(R::Silent)),
                seg("\u{200c}ۚ", Some(R::MaddaObligatory)),
            ]
        );
    }

    #[test]
    fn malformed_id_forms_stay_literal() {
        assert_eq!(
            parse_tajweed_segments("[h:x[ٱ] [n["),
            vec![seg("[h:xٱ [n[", None)]
        );
    }

    #[test]
    fn letterless_group_32_3() {
        let s = parse_tajweed_segments(AS_SAJDAH_32_3);
        for x in &s {
            assert!(!x.text.contains(['[', ']']));
        }
        let bare = s
            .iter()
            .find(|x| x.text.contains("\u{066E}\u{0670}"))
            .unwrap();
        assert_eq!(bare.rule, None);
        assert_eq!(joined(&s), AS_SAJDAH_32_3_PLAIN);
    }

    #[test]
    fn rule_b_rows() {
        assert_eq!(
            parse_tajweed_segments(NISA_4_158),
            vec![
                seg("بَ", None),
                seg("ل", Some(R::IdghamMutaqaribayn)),
                seg(
                    " \u{0631}\u{0651}\u{064E}\u{0641}\u{064E}\u{0639}\u{064E}\u{0647}\u{064F} ",
                    None
                ),
                seg("ٱ", Some(R::HamzatWasl)),
                seg(
                    "\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0625}\u{0650}\u{0644}\u{064E}\u{064A}\u{0652}\u{0647}\u{0650}\u{200C}\u{06DA} \u{0648}\u{064E}\u{0643}\u{064E}\u{0627}\u{0646}\u{064E} ",
                    None
                ),
                seg("ٱ", Some(R::HamzatWasl)),
                seg(
                    "\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0639}\u{064E}\u{0632}\u{0650}\u{064A}\u{0632}\u{064B}\u{0627} \u{062D}\u{064E}\u{0643}\u{0650}\u{064A}\u{0645}\u{064B}\u{0627}",
                    None
                ),
            ]
        );
        assert_eq!(
            parse_tajweed_segments(MURSALAT_77_20)[..2],
            [seg("أَلَمْ نَخْلُ", None), seg("ق", Some(R::IdghamMutaqaribayn))]
        );
    }

    #[test]
    fn strip() {
        assert_eq!(
            strip_tajweed_markup(FATIHAH_1),
            "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650} \u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{0650} \u{0671}\u{0644}\u{0631}\u{0651}\u{064E}\u{062D}\u{0652}\u{0645}\u{064E}\u{0640}\u{0670}\u{0646}\u{0650} \u{0671}\u{0644}\u{0631}\u{0651}\u{064E}\u{062D}\u{0650}\u{064A}\u{0645}\u{0650}"
        );
        assert_eq!(
            strip_tajweed_markup("[2] tanzil footnote marker"),
            "[2] tanzil footnote marker"
        );
        assert_eq!(
            strip_tajweed_markup(AYAT_AL_KURSI_EXCERPT),
            "\u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0644}\u{064E}\u{0627}\u{0653} \u{0625}\u{0650}\u{0644}\u{064E}\u{0640}\u{0670}\u{0647}\u{064E} \u{0625}\u{0650}\u{0644}\u{0651}\u{064E}\u{0627} \u{0647}\u{064F}\u{0648}\u{064E} \u{0671}\u{0644}\u{0652}\u{062D}\u{064E}\u{0649}\u{0651}\u{064F}"
        );
        assert_eq!(
            strip_tajweed_markup(NISA_4_158),
            "\u{0628}\u{064E}\u{0644} \u{0631}\u{0651}\u{064E}\u{0641}\u{064E}\u{0639}\u{064E}\u{0647}\u{064F} \u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0625}\u{0650}\u{0644}\u{064E}\u{064A}\u{0652}\u{0647}\u{0650}\u{200C}\u{06DA} \u{0648}\u{064E}\u{0643}\u{064E}\u{0627}\u{0646}\u{064E} \u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0639}\u{064E}\u{0632}\u{0650}\u{064A}\u{0632}\u{064B}\u{0627} \u{062D}\u{064E}\u{0643}\u{0650}\u{064A}\u{0645}\u{064B}\u{0627}"
        );
        assert_eq!(
            strip_tajweed_markup(BAQARAH_2_190_TAIL),
            BAQARAH_2_190_TAIL_PLAIN
        );
        assert_eq!(strip_tajweed_markup(AS_SAJDAH_32_3), AS_SAJDAH_32_3_PLAIN);
    }

    #[test]
    fn letters_round_trip() {
        for r in TajweedRule::ALL {
            assert_eq!(TajweedRule::from_letter(r.letter() as u8), Some(r));
        }
        assert!(is_tajweed_script(QuranScript::Tajweed));
        assert!(!is_tajweed_script(QuranScript::Uthmani));
    }
}
