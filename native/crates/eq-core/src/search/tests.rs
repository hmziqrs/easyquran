//! Ports of the web's search tests that guard costly behaviour: Arabic
//! and Latin normalisation (shared parity corpus), highlight byte ranges
//! into verbatim text, go-to-verse parsing (`2:255`, ranges, Arabic-Indic
//! digits, keyword aliases, juz nicknames), surah folding, ranking.

use crate::quran::{
    Ayah, RangeText, SurahNormalization,
    meta::{Place, meta},
};

use super::{
    alignment::align_search_text,
    corpus::Highlight,
    highlights::highlight_segments,
    latin::{is_eligible_latin_query, normalize_latin, normalize_latin_with_map},
    nav::{
        JUZ_KEYWORDS, NavTarget, is_nav_candidate, juz_keyword_for, match_nav, parse_query,
        place_for_alias, split_place_token, strip_trailing_ref,
    },
    normalize::{is_eligible_query, normalize_arabic, normalize_arabic_with_map},
    palette::{Context, Group, Target, dedupe, sections, sync_entries},
    recents::Recents,
    suggest::suggest_surahs,
    translation::TranslationCorpus,
    translit::{SURAH_KEYWORDS, translit_key},
};

fn ids(q: &str) -> Vec<String> {
    match_nav(meta(), q, 12).into_iter().map(|m| m.id).collect()
}

fn ids_n(q: &str, n: usize) -> Vec<String> {
    match_nav(meta(), q, n).into_iter().map(|m| m.id).collect()
}

// ---- Arabic normalisation (parity.json) ----------------------------------

#[test]
fn arabic_normalisation_parity_corpus() {
    let cases = [
        ("بِسْمِ", "بسم"),
        ("ٱلْحَمْدُ لِلَّهِ", "الحمد لله"),
        ("مَرْحَبَة", "مرحبه"),
        ("إِنَّ آيَةً", "ان ايه"),
        ("كَلِمَةٌ   جَمِيلَة", "كلمه جميله"),
        ("ا۞ب", "ا۞ب"),
        ("اۥۦ۩", "اۥۦ۩"),
        ("بۗر", "بۗر"),
        ("الضآلين", "الضالين"),
    ];
    for (input, expected) in cases {
        assert_eq!(normalize_arabic(input), expected, "{input}");
    }
}

#[test]
fn arabic_map_spans_cover_dropped_marks_and_stay_on_boundaries() {
    let text = "  بِسْمِ   ٱللَّهِ ";
    let map = normalize_arabic_with_map(text);
    assert_eq!(map.normalized, "بسم الله");
    assert_eq!(map.starts.len(), map.normalized.chars().count());
    for (s, e) in map.starts.iter().zip(&map.ends) {
        assert!(s < e);
        assert!(text.is_char_boundary(*s as usize) && text.is_char_boundary(*e as usize));
    }
    // The first letter's span swallows its harakat.
    assert_eq!(&text[map.starts[0] as usize..map.ends[0] as usize], "بِ");
}

#[test]
fn eligible_query_floor_and_ornaments() {
    assert!(is_eligible_query(&normalize_arabic("الحمد")));
    assert!(!is_eligible_query(&normalize_arabic("ا")));
    assert!(!is_eligible_query(&normalize_arabic("من")));
    assert!(is_eligible_query(&normalize_arabic("۞")));
    assert!(is_eligible_query(&normalize_arabic("۩")));
}

// ---- Latin normalisation ---------------------------------------------------

#[test]
fn latin_normalisation() {
    assert_eq!(normalize_latin("  Mercy  "), "mercy");
    assert_eq!(normalize_latin("mécréant"), "mecreant");
    assert_eq!(normalize_latin("me\u{0301}rcy"), "mercy");
    assert_eq!(
        normalize_latin("don’t “seek” mercy"),
        "don't \"seek\" mercy"
    );
    assert_eq!(normalize_latin("ar–rahman"), "ar-rahman");
    assert_eq!(normalize_latin("ar−rahman"), "ar-rahman");
    assert_eq!(normalize_latin("the \t\n merciful"), "the merciful");
    assert_eq!(normalize_latin("ΑΣ"), "ασ");
    assert_eq!(normalize_latin("ας"), "ασ");
    assert!(!is_eligible_latin_query("me"));
    assert!(is_eligible_latin_query("mer"));
    assert!(is_eligible_latin_query(&"a".repeat(64)));
    assert!(!is_eligible_latin_query(&"a".repeat(65)));
}

#[test]
fn latin_map_keeps_combining_marks_inside_the_span() {
    let original = "Me\u{0301}rcy and MERCY";
    let map = normalize_latin_with_map(original);
    assert_eq!(map.normalized, "mercy and mercy");
    let first = &original[map.starts[0] as usize..map.ends[4] as usize];
    assert_eq!(first, "Me\u{0301}rcy");
}

// ---- Highlights ------------------------------------------------------------

#[test]
fn highlight_segments_partition_without_rewriting() {
    let text = "قبل 😀 بعد";
    let start = text.find('😀').unwrap();
    let segments = highlight_segments(
        text,
        &[Highlight {
            start,
            end: start + 4,
        }],
    );
    let joined: String = segments.iter().map(|s| s.text(text)).collect();
    assert_eq!(joined, text);
    let lit: Vec<&str> = segments
        .iter()
        .filter(|s| s.highlighted)
        .map(|s| s.text(text))
        .collect();
    assert_eq!(lit, ["😀"]);
}

#[test]
fn highlight_segments_sort_clamp_merge_and_drop_bad_ranges() {
    let segments = highlight_segments(
        "abcdef",
        &[
            Highlight { start: 3, end: 99 },
            Highlight { start: 1, end: 4 },
            Highlight { start: 9, end: 12 },
        ],
    );
    let parts: Vec<(usize, usize, bool)> = segments
        .iter()
        .map(|s| (s.start, s.end, s.highlighted))
        .collect();
    assert_eq!(parts, [(0, 1, false), (1, 6, true)]);
    // A range inside a multi-byte char is dropped, never split.
    let text = "بسم";
    assert!(
        highlight_segments(text, &[Highlight { start: 1, end: 3 }])
            .iter()
            .all(|s| !s.highlighted)
    );
}

// ---- Alignment -------------------------------------------------------------

#[test]
fn alignment_maps_every_match_char_inside_the_display_text() {
    let text = "بِسْمِ ٱللَّهِ";
    let a = align_search_text(text, text);
    assert_eq!(a.norm.chars().count(), a.starts.len());
    assert!(a.ends.iter().all(|e| *e as usize <= text.len()));
    let display = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ";
    let a = align_search_text(text, display);
    assert_eq!(a.norm.chars().count(), a.starts.len());
    assert!(
        a.starts
            .iter()
            .chain(&a.ends)
            .all(|o| display.is_char_boundary(*o as usize))
    );
}

// ---- Corpora ---------------------------------------------------------------

fn translation_rows(texts: &[(u16, u16, &str)]) -> RangeText {
    RangeText {
        ayahs: texts
            .iter()
            .enumerate()
            .map(|(i, (s, a, t))| Ayah {
                key: format!("{s}:{a}"),
                surah: *s,
                ayah: *a,
                global_index: i as u32 + 1,
                text: (*t).to_owned(),
            })
            .collect(),
        normalizations: vec![SurahNormalization::translation("en.test", 1)],
    }
}

#[test]
fn translation_ranking_and_highlights_point_into_verbatim_text() {
    let rows = translation_rows(&[
        (1, 1, "In the name of God, the Merciful"),
        (1, 2, "Mercy, Mercy and mercy"),
        (1, 3, "unmerciful things"),
    ]);
    let corpus = TranslationCorpus::build("en.test", &rows).unwrap();
    let page = corpus.search("MERCY", 20, 0);
    assert_eq!(page.total, 3);
    // Term frequency wins over mushaf order.
    assert_eq!(page.results[0].ayah, 2);
    let hit = &page.results[0];
    assert_eq!(hit.text, "Mercy, Mercy and mercy");
    let lit: Vec<&str> = hit
        .highlights
        .iter()
        .map(|h| &hit.text[h.start..h.end])
        .collect();
    assert_eq!(lit, ["Mercy", "Mercy", "mercy"]);
    // Too short: nothing.
    assert_eq!(corpus.search("me", 20, 0).total, 0);
}

#[test]
fn translation_corpus_rejects_gaps() {
    let mut rows = translation_rows(&[(1, 1, "a"), (1, 2, "b")]);
    rows.ayahs[1].global_index = 5;
    assert!(TranslationCorpus::build("en.test", &rows).is_err());
}

// ---- Go-to-verse -----------------------------------------------------------

#[test]
fn verse_refs_in_every_shape() {
    for q in [
        "2:255",
        "2 255",
        "2.255",
        "2-255",
        "٢:٢٥٥",
        "۲:۲۵۵",
        "surah 2:255",
        " 2 : 255 ",
    ] {
        assert_eq!(ids(q), ["ayah:2:255"], "{q}");
    }
    assert!(ids("1:8").is_empty());
    assert!(ids("2:287").is_empty());
    let parsed = parse_query("2-255");
    assert_eq!(parsed.numbers, [2, 255]);
}

#[test]
fn ranges_land_on_their_first_verse_and_bad_ends_drop() {
    assert_eq!(ids("2:255-286"), ["ayah:2:255"]);
    assert_eq!(ids("2:255-2:286"), ["ayah:2:255"]);
    assert!(ids("2:255-287").is_empty());
    assert!(ids("2:255-2:287").is_empty());
    assert!(ids("2:255-1:8").is_empty());
}

#[test]
fn keyword_targets() {
    assert_eq!(ids("surah 18"), ["surah:18"]);
    assert_eq!(ids("سورة ١٨"), ["surah:18"]);
    assert!(ids("surah 115").is_empty());
    assert!(ids("surah 0").is_empty());
    assert_eq!(ids("juz 5"), ["juz:5"]);
    assert_eq!(ids("para 5"), ["juz:5"]);
    assert_eq!(ids("جزء ٥"), ["juz:5"]);
    assert!(ids("juz 31").is_empty());
    assert_eq!(ids("page 100"), ["page:100"]);
    assert_eq!(ids("p 100"), ["page:100"]);
    assert_eq!(ids("صفحة ١٠٠"), ["page:100"]);
    assert!(ids("page 605").is_empty());
}

#[test]
fn juz_nicknames_anywhere() {
    assert_eq!(ids("amma"), ["juz:30"]);
    assert_eq!(ids("juz amma"), ["juz:30"]);
    assert_eq!(ids("ama"), ["juz:30"]);
    assert_eq!(ids("tabarak"), ["juz:29"]);
    assert_eq!(ids("mubarak"), ["juz:29"]);
    assert_eq!(ids("qad sama"), ["juz:28"]);
    // No two nicknames fold onto different juz.
    for (a, ja) in JUZ_KEYWORDS {
        for (b, jb) in JUZ_KEYWORDS {
            if translit_key(a) == translit_key(b) {
                assert_eq!(ja, jb, "{a} / {b}");
            }
        }
    }
    for (a, sa) in SURAH_KEYWORDS {
        for (b, sb) in SURAH_KEYWORDS {
            if translit_key(a) == translit_key(b) {
                assert_eq!(sa, sb, "{a} / {b}");
            }
        }
    }
    assert_eq!(juz_keyword_for("Amma"), Some(("amma", 30)));
}

#[test]
fn sajdas() {
    let all = match_nav(meta(), "sajda", 20);
    assert_eq!(all.len(), 15);
    assert!(all.iter().all(|m| m.score == 0.5));
    assert_eq!(ids("sajda 7"), ["sajda:7:206"]);
    assert_eq!(ids("سجدة ٧"), ["sajda:7:206"]);
    assert_eq!(ids("sajda 96:19"), ["sajda:96:19"]);
    assert!(ids("sajda 2").is_empty());
    assert_eq!(ids_n("sajda", 5).len(), 5);
}

#[test]
fn bare_numbers_are_ambiguous() {
    assert_eq!(ids("5"), ["surah:5", "juz:5", "page:5"]);
    assert_eq!(ids("40"), ["surah:40", "page:40"]);
    assert_eq!(ids("200"), ["page:200"]);
    assert!(ids("9999").is_empty());
}

#[test]
fn places_and_plain_text() {
    let s = split_place_token("meccan kahf");
    assert_eq!((s.place, s.text.as_str()), (Some(Place::Meccan), "kahf"));
    let s = split_place_token("kahf meccan");
    assert_eq!((s.place, s.text.as_str()), (Some(Place::Meccan), "kahf"));
    assert_eq!(split_place_token("مدنية").place, Some(Place::Medinan));
    assert_eq!(split_place_token("the cow").place, None);
    assert_eq!(place_for_alias("مدنيه"), Some(Place::Medinan));
    let medinan = match_nav(meta(), "medinan", 12);
    assert!(matches!(
        medinan[0].target,
        NavTarget::Place {
            place: Place::Medinan,
            surah_count: 28
        }
    ));
    assert!(ids("").is_empty());
    assert!(ids("kahf").is_empty());
    assert!(ids("mercy").is_empty());
    assert!(is_nav_candidate("55"));
    assert!(is_nav_candidate("2:255"));
    assert!(is_nav_candidate("juz"));
    assert!(!is_nav_candidate("s"));
    assert!(!is_nav_candidate("ab"));
    assert_eq!(strip_trailing_ref("baqarah 255"), "baqarah");
    assert_eq!(strip_trailing_ref("البقرة ٢٥٥"), "البقرة");
}

// ---- Surah names -------------------------------------------------------------

#[test]
fn surah_names_fold_across_spellings_and_scripts() {
    for q in [
        "baqarah",
        "Al-Baqarah",
        "baqara",
        "bakara",
        "البقرة",
        "the cow",
    ] {
        let top = suggest_surahs(meta(), q, 5);
        assert_eq!(top.first().map(|s| s.surah.num), Some(2), "{q}");
    }
    assert_eq!(
        suggest_surahs(meta(), "tabarak", 5)
            .first()
            .map(|s| s.surah.num),
        Some(67)
    );
}

#[test]
fn palette_named_verse_and_dedupe() {
    let ctx = Context {
        meta: meta(),
        parsed: parse_query("baqarah 255"),
        route: None,
        recents: vec![],
        resume: None,
        authenticated: false,
    };
    let entries = dedupe(sync_entries(&ctx));
    let first = &entries[0];
    assert_eq!(first.group, Group::JumpTo);
    assert_eq!(first.dedupe_key.as_deref(), Some("ayah:2:255"));
    assert!(matches!(first.target, Target::Verse { surah: 2, .. }));
    let secs = sections(entries);
    assert_eq!(secs[0].group, Group::JumpTo);

    // A bare `2`: jump-to surah wins, the catalogue row for it is deduped.
    let ctx = Context {
        parsed: parse_query("2"),
        ..ctx
    };
    let entries = dedupe(sync_entries(&ctx));
    let surah2 = entries
        .iter()
        .filter(|e| e.dedupe_key.as_deref() == Some("surah:2"))
        .count();
    assert_eq!(surah2, 1);
}

// ---- Recents ---------------------------------------------------------------

#[test]
fn recents_record_cap_pause_and_decode() {
    let mut r = Recents::default();
    for i in 0..12 {
        assert!(r.record(&format!("query {i}"), i));
    }
    assert_eq!(r.entries.len(), 10);
    assert_eq!(r.entries[0].q, "query 11");
    r.record("query 5", 99);
    assert_eq!(r.entries[0].q, "query 5");
    assert_eq!(r.entries.iter().filter(|e| e.q == "query 5").count(), 1);
    r.paused = true;
    assert!(!r.record("new", 100));
    let back = Recents::decode(Some(&r.encode()));
    assert_eq!(back, r);
    assert_eq!(
        Recents::decode(Some(r#"{"v":2,"entries":[{"q":"x","at":1}]}"#)),
        Recents::default()
    );
}
