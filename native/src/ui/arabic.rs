//! Quran text on screen (PARITY C4, C5): the web's `ReadingAyah.svelte`,
//! `IndoPakAyah.svelte` and `AyahOrnament.svelte` as one text primitive.
//!
//! The text is drawn verbatim: every segment's string goes to the text
//! engine unchanged (no trim, no normalisation), as nested spans of one
//! paragraph, so the platform shapes the whole ayah at once (CoreText on
//! Apple, `StaticLayout` on Android, Pango on Linux, DirectWrite on
//! Windows). The paragraph's base direction is right to left whatever the
//! UI locale. The ayah-end ornament (U+06DD and the ayah number in
//! Arabic-Indic digits) is a span after the text, never part of it, in
//! its own face: "Ayah Ornament" (a Scheherazade New subset) in the
//! primary color, or the IndoPak face with Extended Arabic-Indic digits
//! in the text's own color for IndoPak.
//!
//! Faces (`lib/config/reader-fonts.ts`), shipped in `assets/fonts` and
//! named by their real family names (the web's `@font-face` aliases do
//! not exist natively): KFGQPC Uthmanic Hafs v22 (the default), KFGQPC
//! Uthman Taha Naskh v1 and its bold, IndoPak Reader Compat v4. Amiri,
//! Scheherazade New and Noto Naskh Arabic are selectable on the web from
//! npm packages this app does not ship; choosing one here names the
//! family and the platform falls back to its own Arabic face.
//!
//! The shipped `uthmanic-hafs-v22.ttf` carries one cmap entry the upstream
//! file lacks: U+06DF SMALL HIGH ROUNDED ZERO maps to `afii57458`, the
//! face's rounded-zero mark (the glyph KFGQPC's own text reaches through
//! U+0652). Upstream maps U+06DF (and U+06E3, U+06EB) to `uni06DF`, a
//! spacing dotted-circle placeholder (GDEF class 1, advance 1442), so the
//! Tanzil text's 3988 silent-letter marks drew as dotted circles in every
//! engine (CoreText and HarfBuzz alike). Glyphs, GSUB, GPOS untouched.

use nsh_core::LineHeight;
use nsh_core::element::WritingDirection;
use nsh_core::prelude::*;

use crate::ui::tokens::{self, TajweedClass};

/// The Arabic edition a text comes from (the web's `QuranSourceId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QuranScript {
    /// Tanzil Uthmani.
    #[default]
    Uthmani,
    /// The fully annotated Uthmani mushaf (pausal and waqf marks).
    UthmaniAnnotated,
    /// Tanzil simple-clean (no marks).
    SimpleClean,
    /// IndoPak (South Asian) script.
    IndoPak,
    /// Uthmani with tajweed rule segments.
    Tajweed,
}

impl QuranScript {
    /// Every script, in the web's order.
    pub const ALL: [QuranScript; 5] = [
        QuranScript::Uthmani,
        QuranScript::UthmaniAnnotated,
        QuranScript::SimpleClean,
        QuranScript::IndoPak,
        QuranScript::Tajweed,
    ];

    /// The source id (`uthmani`, `uthmani-annotated`, `simple-clean`,
    /// `indopak`, `tajweed`).
    pub fn source_id(self) -> &'static str {
        match self {
            QuranScript::Uthmani => "uthmani",
            QuranScript::UthmaniAnnotated => "uthmani-annotated",
            QuranScript::SimpleClean => "simple-clean",
            QuranScript::IndoPak => "indopak",
            QuranScript::Tajweed => "tajweed",
        }
    }

    /// The script of a source id.
    pub fn from_source_id(id: &str) -> Option<QuranScript> {
        QuranScript::ALL.into_iter().find(|s| s.source_id() == id)
    }
}

/// A reader face (`ArabicFontId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ArabicFont {
    /// `kfgqpc-hafs`: KFGQPC Uthmanic Hafs (Ver22), the default.
    #[default]
    KfgqpcHafs,
    /// `kfgqpc-taha-v1`: KFGQPC Uthman Taha Naskh v1.
    KfgqpcTahaV1,
    /// `kfgqpc-taha-v1-bold`: its bold.
    KfgqpcTahaV1Bold,
    /// `amiri` (not shipped natively: the platform's Arabic face).
    Amiri,
    /// `scheherazade-new` (not shipped natively).
    ScheherazadeNew,
    /// `noto-naskh-arabic` (not shipped natively).
    NotoNaskhArabic,
}

impl ArabicFont {
    /// Every face, in the web's order.
    pub const ALL: [ArabicFont; 6] = [
        ArabicFont::Amiri,
        ArabicFont::ScheherazadeNew,
        ArabicFont::NotoNaskhArabic,
        ArabicFont::KfgqpcHafs,
        ArabicFont::KfgqpcTahaV1,
        ArabicFont::KfgqpcTahaV1Bold,
    ];

    /// The id the web persists.
    pub fn id(self) -> &'static str {
        match self {
            ArabicFont::KfgqpcHafs => "kfgqpc-hafs",
            ArabicFont::KfgqpcTahaV1 => "kfgqpc-taha-v1",
            ArabicFont::KfgqpcTahaV1Bold => "kfgqpc-taha-v1-bold",
            ArabicFont::Amiri => "amiri",
            ArabicFont::ScheherazadeNew => "scheherazade-new",
            ArabicFont::NotoNaskhArabic => "noto-naskh-arabic",
        }
    }

    /// The face of a persisted id.
    pub fn from_id(id: &str) -> Option<ArabicFont> {
        ArabicFont::ALL.into_iter().find(|f| f.id() == id)
    }

    /// The family the platform knows the face by (the font file's name
    /// table, not the web's alias).
    pub fn family(self) -> &'static str {
        match self {
            ArabicFont::KfgqpcHafs => HAFS_FAMILY,
            ArabicFont::KfgqpcTahaV1 | ArabicFont::KfgqpcTahaV1Bold => TAHA_FAMILY,
            ArabicFont::Amiri => "Amiri",
            ArabicFont::ScheherazadeNew => "Scheherazade New",
            ArabicFont::NotoNaskhArabic => "Noto Naskh Arabic",
        }
    }

    /// The weight that picks the face inside its family.
    pub fn weight(self) -> FontWeight {
        match self {
            ArabicFont::KfgqpcTahaV1Bold => FontWeight::BOLD,
            _ => FontWeight::NORMAL,
        }
    }

    /// Whether the app ships the face.
    pub fn shipped(self) -> bool {
        matches!(
            self,
            ArabicFont::KfgqpcHafs | ArabicFont::KfgqpcTahaV1 | ArabicFont::KfgqpcTahaV1Bold
        )
    }
}

/// `uthmanic-hafs-v22.ttf`'s family.
pub const HAFS_FAMILY: &str = "kfgqpc_hafs_uthmanic_script";
/// `uthman-taha-naskh-v1.ttf` (400) and `-bold.ttf` (700)'s family.
pub const TAHA_FAMILY: &str = "KFGQPC Uthman Taha Naskh";
/// `indopak-reader-compat-v4.ttf`'s family.
pub const INDOPAK_FAMILY: &str = "IndoPak Reader Compat";
/// `ayah-ornament.ttf`'s family (U+06DD and Arabic-Indic digits only).
pub const ORNAMENT_FAMILY: &str = "Ayah Ornament";

/// The font files under `assets/fonts` of the source tree, read for a
/// `cargo run` of the desktop binary (which has no asset host, so the
/// registry's fonts never load); empty where the tree is not on disk.
pub fn dev_font_files() -> Vec<Vec<u8>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ttf" || x == "otf"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| std::fs::read(p).ok())
        .collect()
}

/// The web's IndoPak `size-adjust: 125%`: the compat face draws small.
const INDOPAK_SIZE_ADJUST: f32 = 1.25;
/// The ornament's size against the text (`.ayah-ornament`, 0.86em).
const ORNAMENT_SCALE: f32 = 0.86;
/// The IndoPak ornament's size (`[data-indopak-ornament]`, 0.68em).
const INDOPAK_ORNAMENT_SCALE: f32 = 0.68;

/// One run of an ayah: verbatim text, and its tajweed class when it is a
/// tajweed segment (`TajweedSegment`).
#[derive(Clone, Debug, PartialEq)]
pub struct ArabicSegment {
    /// The text, exactly as stored.
    pub text: SharedString,
    /// The rule that colors it; `None` for a plain run.
    pub class: Option<TajweedClass>,
}

impl ArabicSegment {
    /// A plain run.
    pub fn plain(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            class: None,
        }
    }

    /// A run colored by `class`.
    pub fn rule(text: impl Into<SharedString>, class: TajweedClass) -> Self {
        Self {
            text: text.into(),
            class: Some(class),
        }
    }
}

/// How an ayah is set: its edition, face, size and line height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArabicStyle {
    /// The edition (picks the IndoPak face and digits).
    pub script: QuranScript,
    /// The reader face (not used for IndoPak, which has its own).
    pub font: ArabicFont,
    /// The text size in points (the web's `--reader-arabic-size`, 22-56,
    /// 33 by default).
    pub size: f32,
    /// The line height as a multiple of the size (2.15 on the web).
    pub line_height: f32,
}

impl Default for ArabicStyle {
    fn default() -> Self {
        Self {
            script: QuranScript::Uthmani,
            font: ArabicFont::KfgqpcHafs,
            size: 33.0,
            line_height: 2.15,
        }
    }
}

impl ArabicStyle {
    /// The default setting of `script`.
    pub fn of(script: QuranScript) -> Self {
        Self {
            script,
            ..Self::default()
        }
    }

    fn indopak(&self) -> bool {
        self.script == QuranScript::IndoPak
    }

    /// The family the text is set in.
    pub fn family(&self) -> &'static str {
        if self.indopak() {
            INDOPAK_FAMILY
        } else {
            self.font.family()
        }
    }

    /// The text's drawn size (IndoPak's face is scaled up, as the web's
    /// `size-adjust` does).
    pub fn drawn_size(&self) -> f32 {
        if self.indopak() {
            self.size * INDOPAK_SIZE_ADJUST
        } else {
            self.size
        }
    }
}

/// Which numerals an ornament carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Digits {
    /// Arabic-Indic (U+0660-0669), as the Uthmani editions print them.
    Arabic,
    /// Extended Arabic-Indic (U+06F0-06F9), as IndoPak mushafs print them.
    Urdu,
}

/// `n` in Arabic-Indic digits (`toArabicDigits`).
pub fn arabic_digits(n: u32) -> String {
    map_digits(n, '\u{0660}')
}

/// `n` in Extended Arabic-Indic digits (`toEasternDigits`).
pub fn urdu_digits(n: u32) -> String {
    map_digits(n, '\u{06F0}')
}

fn map_digits(n: u32, zero: char) -> String {
    n.to_string()
        .chars()
        .map(|d| {
            d.to_digit(10)
                .and_then(|v| char::from_u32(zero as u32 + v))
                .unwrap_or(d)
        })
        .collect()
}

/// The ornament's characters: U+06DD END OF AYAH, then the number.
pub fn ornament_text(number: u32, digits: Digits) -> String {
    let numerals = match digits {
        Digits::Arabic => arabic_digits(number),
        Digits::Urdu => urdu_digits(number),
    };
    format!("\u{06DD}{numerals}")
}

/// An ayah: its runs in order (one plain run for every script but
/// tajweed), then the ornament when `number` is given. `style` is a
/// binding: the face, size and line height follow it.
pub fn ayah_text(
    segments: Vec<ArabicSegment>,
    number: Option<u32>,
    style: impl Fn() -> ArabicStyle + Copy + 'static,
) -> Text {
    // The face's weight is fixed with the element (a weight is not a
    // binding); a face change that flips it remakes the ayah (key it).
    let initial = untracked(style);
    let script = initial.script;
    let mut paragraph = text("")
        .writing_direction(WritingDirection::Rtl)
        .font_family(move || SharedString::from(style().family()))
        .font_weight(initial.font.weight())
        .text_size(move || px(style().drawn_size()))
        .line_height(move || LineHeight::Multiple(style().line_height))
        .text_color(tokens::quran_foreground);
    for segment in cluster_runs(segments) {
        paragraph = paragraph.child(run(segment));
    }
    if let Some(number) = number {
        paragraph = paragraph.child(ayah_ornament(number, script, style));
    }
    paragraph
}

/// One plain ayah (every script but tajweed): `ayah_text` of one run.
pub fn plain_ayah(
    verse: impl Into<SharedString>,
    number: Option<u32>,
    style: impl Fn() -> ArabicStyle + Copy + 'static,
) -> Text {
    ayah_text(vec![ArabicSegment::plain(verse)], number, style)
}

/// Whether `c` is a combining mark that shapes onto the letter before it
/// (Unicode Mn in the Arabic blocks, and the generic combining blocks).
fn is_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E4}'
            | '\u{06E7}'..='\u{06E8}'
            | '\u{06EA}'..='\u{06ED}'
            | '\u{08D3}'..='\u{08E1}'
            | '\u{08E3}'..='\u{08FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Moves span boundaries off grapheme clusters, so no span starts with a
/// combining mark: a mark split from its letter shapes alone, and the
/// platform draws it on a dotted circle (U+25CC). The bytes, and their
/// order, are unchanged; only where one run ends and the next begins
/// moves. A run's leading marks join the run before it (they belong to its
/// last letter's cluster); a colored run made of marks only instead takes
/// that letter's whole cluster, so its rule stays visible.
pub fn cluster_runs(segments: Vec<ArabicSegment>) -> Vec<ArabicSegment> {
    let mut out: Vec<(String, Option<TajweedClass>)> = Vec::with_capacity(segments.len());
    for segment in segments {
        let mut text = segment.text.to_string();
        let lead = text
            .find(|c: char| !is_combining_mark(c))
            .unwrap_or(text.len());
        if lead > 0 && !out.is_empty() {
            if lead == text.len() && segment.class.is_some() {
                pull_cluster(&mut out, &mut text);
            } else if let Some((last, _)) = out.last_mut() {
                last.extend(text.drain(..lead));
            }
        }
        if !text.is_empty() {
            out.push((text, segment.class));
        }
    }
    out.into_iter()
        .map(|(text, class)| ArabicSegment {
            text: text.into(),
            class,
        })
        .collect()
}

/// Moves the last cluster (base letter and its marks) of the runs in `out`
/// to the front of `text`, dropping runs left empty.
fn pull_cluster(out: &mut Vec<(String, Option<TajweedClass>)>, text: &mut String) {
    while let Some((last, _)) = out.last_mut() {
        let base = last.rfind(|c: char| !is_combining_mark(c));
        let from = base.unwrap_or(0);
        text.insert_str(0, &last[from..]);
        last.truncate(from);
        if last.is_empty() {
            out.pop();
        }
        if base.is_some() {
            return;
        }
    }
}

/// A run as a span: plain runs inherit the paragraph's color, tajweed
/// runs paint their class's.
fn run(segment: ArabicSegment) -> Text {
    let span = text(segment.text);
    match segment.class {
        Some(class) => span.text_color(move || tokens::tajweed(class)),
        None => span,
    }
}

/// The ayah-end ornament as a span of an ayah's paragraph: U+06DD with the
/// number, which the face composes into one medallion. Uthmani editions:
/// "Ayah Ornament" in the primary color at 0.86 of the text; IndoPak: the
/// IndoPak face's own medallion, Urdu digits, the text's color, 0.68.
pub fn ayah_ornament(
    number: u32,
    script: QuranScript,
    style: impl Fn() -> ArabicStyle + Copy + 'static,
) -> Text {
    if script == QuranScript::IndoPak {
        return text(ornament_text(number, Digits::Urdu))
            .font_family(INDOPAK_FAMILY)
            .font_weight(FontWeight::NORMAL)
            .text_size(move || px(style().drawn_size() * INDOPAK_ORNAMENT_SCALE));
    }
    text(ornament_text(number, Digits::Arabic))
        .font_family(ORNAMENT_FAMILY)
        .font_weight(FontWeight::NORMAL)
        .text_size(move || px(style().size * ORNAMENT_SCALE))
        .text_color(move || nsh_base::theme().primary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eq_core::quran::db::QuranDb;
    use eq_core::quran::view::tajweed::parse_tajweed_segments;

    fn segments_of(text: &str) -> Vec<ArabicSegment> {
        parse_tajweed_segments(text)
            .into_iter()
            .map(|s| ArabicSegment {
                text: s.text.into(),
                class: s.rule.and_then(|r| TajweedClass::from_letter(r.letter())),
            })
            .collect()
    }

    fn joined(segments: &[ArabicSegment]) -> String {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn leading_marks_join_the_letter_before() {
        let runs = cluster_runs(vec![
            ArabicSegment::plain("صِر"),
            ArabicSegment::rule("\u{064E}\u{0672}", TajweedClass::MaddaNormal),
            ArabicSegment::plain("\u{0651}طَ"),
        ]);
        assert_eq!(
            runs,
            vec![
                ArabicSegment::plain("صِر\u{064E}"),
                ArabicSegment::rule("\u{0672}\u{0651}", TajweedClass::MaddaNormal),
                ArabicSegment::plain("طَ"),
            ]
        );
        // A colored run of marks only takes the whole cluster.
        let runs = cluster_runs(vec![
            ArabicSegment::plain("بِسْ"),
            ArabicSegment::rule("\u{0650}", TajweedClass::MaddaNormal),
        ]);
        assert_eq!(
            runs,
            vec![
                ArabicSegment::plain("بِ"),
                ArabicSegment::rule("سْ\u{0650}", TajweedClass::MaddaNormal),
            ]
        );
    }

    /// Quran-text guard: re-cutting the runs of every tajweed verse keeps
    /// the text byte for byte, and no run but the first starts with a mark.
    #[test]
    fn tajweed_corpus_runs_keep_text_verbatim() {
        let root = std::env::var_os("EQ_QURAN_DB")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/Users/hmziq/os/easyquran/db/quran"));
        let path = root.join("arabic/quran-tajweed.sqlite");
        if !path.is_file() {
            eprintln!(
                "NOTICE: {} absent; tajweed run guard skipped",
                path.display()
            );
            return;
        }
        let rows = QuranDb::open(&path).unwrap().all().unwrap();
        assert_eq!(rows.len(), 6236);
        for row in &rows {
            let parsed = segments_of(&row.text);
            let runs = cluster_runs(parsed.clone());
            assert_eq!(joined(&runs), joined(&parsed), "{}", row.key);
            for run in runs.iter().skip(1) {
                assert!(!run.text.is_empty(), "{} empty run", row.key);
                let first = run.text.chars().next().unwrap_or(' ');
                assert!(
                    !is_combining_mark(first),
                    "{} run starts with a mark",
                    row.key
                );
            }
        }
    }
}
