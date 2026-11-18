use super::*;
use crate::kv::{KeyValue, MemoryKv};

#[test]
fn every_en_key_resolves_to_text() {
    let mut count = 0;
    for key in Key::all() {
        assert_eq!(Key::from_name(key.name()), Some(key));
        assert!(!tr(Locale::En, key).is_empty(), "{} is empty", key.name());
        count += 1;
    }
    assert!(count > 1000, "catalogue too small: {count}");
}

#[test]
fn ar_missing_keys_are_listed_and_fall_back_to_english() {
    let missing = ar_missing();
    eprintln!("ar lacks {} key(s): {missing:?}", missing.len());
    for name in missing {
        let key = Key::from_name(name).unwrap();
        assert_eq!(tr(Locale::Ar, key), tr(Locale::En, key));
    }
}

#[test]
fn arabic_text_comes_from_the_ar_catalogue() {
    assert_eq!(tr(Locale::En, keys::READER_SHARE_CARD_BRAND), "EasyQuran");
    assert_eq!(tr(Locale::Ar, keys::READER_SHARE_CARD_BRAND), "إيزي قرآن");
    assert_eq!(tr_name(Locale::En, "no_such_key"), None);
}

#[test]
fn arabic_plural_categories() {
    use PluralCategory::*;
    let cases = [
        (0.0, Zero),
        (1.0, One),
        (2.0, Two),
        (3.0, Few),
        (10.0, Few),
        (11.0, Many),
        (99.0, Many),
        (100.0, Other),
        (101.0, Other),
        (102.0, Other),
        (103.0, Few),
        (111.0, Many),
        (1000.0, Other),
        (1003.0, Few),
        (2.5, Other),
        (f64::NAN, Other),
    ];
    for (n, want) in cases {
        assert_eq!(plural_category(Locale::Ar, n), want, "ar {n}");
    }
    assert_eq!(plural_category(Locale::En, 0.0), Other);
    assert_eq!(plural_category(Locale::En, 1.0), One);
    assert_eq!(plural_category(Locale::En, 2.0), Other);
}

#[test]
fn plural_messages_pick_the_cldr_variant() {
    let k = keys::READER_OFFLINE_ROUTES;
    let ar = |n: i64| tr_args(Locale::Ar, k, &[("count", n.into())]);
    assert_eq!(ar(0), "لا توجد مسارات");
    assert_eq!(ar(1), "مسار واحد");
    assert_eq!(ar(2), "مساران");
    assert_eq!(ar(3), "3 مسارات");
    assert_eq!(ar(11), "11 مسارًا");
    assert_eq!(ar(100), "100 مسار");
    let en = |n: i64| tr_args(Locale::En, k, &[("count", n.into())]);
    // en has no `zero` category: 0 takes `other`, as Intl.PluralRules does.
    assert_eq!(en(0), "0 routes");
    assert_eq!(en(1), "1 route");
    assert_eq!(en(5), "5 routes");

    let two_args = tr_args(
        Locale::Ar,
        keys::SETTINGS_STORAGE_PACK_ROUTES,
        &[("entries", 2.into()), ("size", "3 MB".into())],
    );
    assert_eq!(two_args, "مساران · 3 MB");
    assert_eq!(
        tr_args(
            Locale::Ar,
            keys::LANDING_SURAH_AYAHS,
            &[("count", 7.into())]
        ),
        "7 آيات"
    );
    // A string count is read as JS `Number()` reads it.
    assert_eq!(
        tr_args(
            Locale::Ar,
            keys::LANDING_SURAH_AYAHS,
            &[("count", "2".into())]
        ),
        "آيتان"
    );
}

#[test]
fn placeholders_substitute_and_missing_ones_stay() {
    assert_eq!(
        substitute("a {x} b {y}", &[("x", 1.5.into())]),
        "a 1.5 b {y}"
    );
    assert_eq!(substitute("{n}{n}", &[("n", 3.0.into())]), "33");
    assert_eq!(substitute("open { brace", &[]), "open { brace");
    let key = Key::all()
        .find(|k| matches!(message(Locale::En, *k), Msg::Text(t) if t.contains("{count}")))
        .unwrap();
    let out = tr_args(Locale::En, key, &[("count", 42.into())]);
    assert!(out.contains("42") && !out.contains("{count}"), "{out}");
}

#[test]
fn locale_codes_and_suggestion() {
    assert_eq!(Locale::from_code("AR"), Some(Locale::Ar));
    assert_eq!(Locale::from_code("ar-SA"), None);
    assert!(Locale::Ar.is_rtl() && !Locale::En.is_rtl());
    assert_eq!(
        locale_from_languages(["fr-FR", "ar-EG", "en"]),
        Some(Locale::Ar)
    );
    assert_eq!(locale_from_languages(["en_GB"]), Some(Locale::En));
    assert_eq!(locale_from_languages(["fr"]), None);

    let kv = MemoryKv::default();
    let none = SuggestResolved::load(&kv);
    assert_eq!(
        suggest_locale(["ar-SA"], Locale::En, &none),
        Some(Locale::Ar)
    );
    assert_eq!(suggest_locale(["ar-SA"], Locale::Ar, &none), None);
    let map = SuggestResolved::resolve(&kv, Locale::Ar, SuggestResolution::Dismissed).unwrap();
    assert_eq!(suggest_locale(["ar-SA"], Locale::En, &map), None);
    let stored: serde_json::Value =
        serde_json::from_str(&kv.get(LOCALE_SUGGEST_STORAGE_KEY).unwrap()).unwrap();
    assert_eq!(
        stored,
        serde_json::json!({"v": 1, "resolved": {"ar": "dismissed"}})
    );
    // Web decode rules: future schema and junk drop out.
    assert_eq!(
        SuggestResolved::decode(r#"{"v":2,"resolved":{"ar":"accepted"}}"#),
        none
    );
    assert_eq!(
        SuggestResolved::decode(r#"{"resolved":{"ar":"maybe","xx":"accepted"}}"#),
        none
    );
    assert_eq!(SuggestResolved::decode("not json"), none);

    assert_eq!(load_locale_pref(&kv), None);
    save_locale_pref(&kv, Locale::Ar).unwrap();
    assert_eq!(load_locale_pref(&kv), Some(Locale::Ar));
    clear_locale_pref(&kv);
    assert_eq!(load_locale_pref(&kv), None);
}

#[test]
fn relative_time_matches_intl() {
    const MIN: i64 = 60_000;
    let now = relative_time::parse_iso_ms("2026-11-18T12:00:00Z").unwrap();
    let ago = |l, ms: i64| time_ago_ms(now - ms, l, now);
    assert_eq!(ago(Locale::En, 30_000), "this minute");
    assert_eq!(ago(Locale::En, 5 * MIN), "5 min. ago");
    assert_eq!(ago(Locale::En, 3 * 60 * MIN), "3 hr. ago");
    assert_eq!(ago(Locale::En, 24 * 60 * MIN), "yesterday");
    assert_eq!(ago(Locale::En, 1000 * 24 * 60 * MIN), "1,000 days ago");
    assert_eq!(ago(Locale::Ar, 0), "هذه الدقيقة");
    assert_eq!(ago(Locale::Ar, MIN), "قبل دقيقة واحدة");
    assert_eq!(ago(Locale::Ar, 2 * MIN), "قبل دقيقتين");
    assert_eq!(ago(Locale::Ar, 11 * MIN), "قبل 11 دقيقة");
    assert_eq!(ago(Locale::Ar, 2 * 24 * 60 * MIN), "أول أمس");
    assert_eq!(ago(Locale::Ar, 11 * 24 * 60 * MIN), "قبل 11 يومًا");
    assert_eq!(ago(Locale::Ar, 103 * 24 * 60 * MIN), "قبل 103 أيام");
    assert_eq!(ago(Locale::Ar, 1000 * 24 * 60 * MIN), "قبل 1,000 يوم");
    // Future stamps read as now; junk is empty.
    assert_eq!(ago(Locale::En, -5 * MIN), "this minute");
    assert_eq!(time_ago("garbage", Locale::En, now), "");

    let base = relative_time::parse_iso_ms("2026-11-18T12:00:00Z").unwrap();
    assert_eq!(base, 1_795_003_200_000);
    assert_eq!(
        relative_time::parse_iso_ms("2026-11-18T14:00:00.250+02:00"),
        Some(base + 250)
    );
    assert_eq!(
        relative_time::parse_iso_ms("2026-11-18"),
        Some(base - 12 * 60 * MIN)
    );
    assert_eq!(relative_time::parse_iso_ms("2026-02-30"), None);
}
