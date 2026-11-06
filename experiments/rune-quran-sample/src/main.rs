//! Same reader, ids, order and defaults as ../flutter-quran-sample/lib/main.dart and
//! ../nsh-quran-sample/App.tsx. Fonts and text come from the Flutter sample.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use rune::color::hex;
use rune::{AppSpec, Color, Cx, El, Family, ScrollSpec, TextStyle, Tx, Waker, col, row, text};
use rune_kit::{Kit, PaletteSeed, Select};
use serde::Deserialize;

macro_rules! font {
    ($file:literal) => {
        include_bytes!(concat!("../../flutter-quran-sample/fonts/", $file))
    };
}

const FONTS: &[&[u8]] = &[
    font!("amiri.ttf"),
    font!("scheherazade-new.ttf"),
    font!("noto-naskh-arabic.ttf"),
    font!("kfgqpc-hafs.ttf"),
    font!("kfgqpc-taha-v1.ttf"),
    font!("kfgqpc-taha-v1-bold.ttf"),
    font!("noto-nastaliq-urdu.ttf"),
    font!("Gulzar-Regular.ttf"),
    font!("AwamiNastaliq-Regular.ttf"),
    font!("mehr-nastaliq.ttf"),
    font!("katibeh.ttf"),
    font!("mirza.ttf"),
    font!("Lateef-Regular.ttf"),
    font!("aref-ruqaa.ttf"),
    font!("lalezar.ttf"),
    font!("noto-sans-arabic.ttf"),
];

const HAFS: &str = "kfgqpc_hafs_uthmanic_script";
const TAHA: &str = "KFGQPC Uthman Taha Naskh";

/// id, family, weight
const ARABIC: [(&str, &str, f32); 6] = [
    ("amiri", "Amiri", 400.0),
    ("scheherazade-new", "Scheherazade New", 400.0),
    ("noto-naskh-arabic", "Noto Naskh Arabic", 400.0),
    ("kfgqpc-hafs", HAFS, 400.0),
    ("kfgqpc-taha-v1", TAHA, 400.0),
    ("kfgqpc-taha-v1-bold", TAHA, 700.0),
];

/// id, family, line height as a multiple of the size
const URDU: [(&str, &str, f32); 12] = [
    ("noto-nastaliq-urdu", "Noto Nastaliq Urdu", 2.5),
    ("gulzar", "Gulzar", 2.5),
    ("awami-nastaliq", "Awami Nastaliq", 2.6),
    ("mehr-nastaliq-web", "Mehr Nastaliq Web", 2.9),
    ("katibeh", "Katibeh", 2.2),
    ("mirza", "Mirza", 2.3),
    ("lateef", "Lateef", 2.2),
    ("scheherazade-new", "Scheherazade New", 2.2),
    ("amiri", "Amiri", 2.2),
    ("aref-ruqaa", "Aref Ruqaa", 2.0),
    ("lalezar", "Lalezar", 2.0),
    ("noto-sans-arabic", "Noto Sans Arabic", 2.4),
];

const DATASETS: [&str; 3] = ["uthmani", "simple", "indopak"];

#[derive(Deserialize)]
struct Ayah {
    n: u32,
    text: String,
}

#[derive(Deserialize)]
struct Surah {
    number: u32,
    name_en: String,
    name_ar: String,
    ayahs: Vec<Ayah>,
}

#[derive(Deserialize)]
struct Dataset {
    surahs: Vec<Surah>,
}

#[derive(Deserialize)]
struct Translations {
    ur: HashMap<String, Vec<Ayah>>,
}

#[derive(Deserialize)]
struct Sample {
    translations: Translations,
    arabic: HashMap<String, Dataset>,
}

fn sample() -> &'static Sample {
    static SAMPLE: OnceLock<Sample> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        serde_json::from_str(include_str!("../../flutter-quran-sample/assets/quran-sample.json"))
            .expect("sample JSON parses")
    })
}

fn urdu_for(surah: u32, n: u32) -> &'static str {
    sample().translations.ur.get(&surah.to_string())
        .and_then(|ayahs| ayahs.iter().find(|a| a.n == n))
        .map_or("", |a| a.text.as_str())
}

fn ayah_marker(n: u32) -> String {
    let digits: String = n.to_string().chars()
        .filter_map(|c| c.to_digit(10).and_then(|d| char::from_u32(0x0660 + d)))
        .collect();
    // Rune has no paragraph direction setting and detects it from the first strong letter.
    // Arabic digits are not strong, so without the RLM this line lays out left to right.
    format!("\u{200F}﴿{digits}﴾")
}

fn env_u32(name: &str) -> u32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(0)
}

struct State {
    arabic: usize,
    urdu: usize,
    dataset: usize,
    open: Option<&'static str>,
    /// Bench only: copies of the sample, and how many font switches to run.
    repeat: u32,
    cycles_left: u32,
    next_cycle: f64,
}

fn init() -> State {
    State {
        arabic: 3, // kfgqpc-hafs, the web reader default
        urdu: 0,
        dataset: 0,
        open: None,
        repeat: env_u32("QURAN_BENCH_REPEAT").max(1),
        cycles_left: env_u32("QURAN_BENCH_CYCLES"),
        next_cycle: 5.0,
    }
}

/// Bench only: a window at rest draws no frames and so polls nothing, so a thread wakes it.
fn setup(s: &mut State, waker: Waker) {
    let cycles = s.cycles_left;
    if cycles == 0 {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        for _ in 0..cycles {
            waker();
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

/// Bench only: from 5 s, every 500 ms pick the next Arabic font, as the NSH sample does.
fn poll(s: &mut State, now: f64, _: &mut Tx) -> bool {
    if s.cycles_left == 0 || now < s.next_cycle {
        return false;
    }
    s.next_cycle = now + 0.4;
    s.cycles_left -= 1;
    s.arabic = (s.arabic + 1) % ARABIC.len();
    true
}

fn c(value: &str) -> Color {
    hex(value).expect("valid colour")
}

fn selector(
    k: &Kit,
    layer: &mut Vec<El>,
    label: &'static str,
    ids: &[&str],
    current: &str,
    open: bool,
    pick: fn(&mut State, usize),
) -> El {
    let names: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
    let for_choose = names.clone();
    let select = Select::new(label, &names)
        .label(label)
        .width(220.0)
        .current(Some(current))
        .open(open)
        .on_toggle(move |s: &mut State| s.open = if s.open == Some(label) { None } else { Some(label) })
        .on_close(|s: &mut State| s.open = None)
        .on_choose(move |s: &mut State, v: &str, _: &mut Tx| {
            if let Some(i) = for_choose.iter().position(|n| n == v) {
                pick(s, i);
            }
            s.open = None;
        });
    row().items_center().gap(8.0)
        .child(text(label).font(TextStyle::new(13.0, 400.0)).color(Color::BLACK))
        .child(select.build(k, layer))
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let rule = c("#E0E0E0");
    let accent = c("#00695C");
    let k = Kit::from_palette(PaletteSeed { bg: Color::WHITE, fg: Color::BLACK, accent });
    let (arabic_id, arabic_family, arabic_weight) = ARABIC[s.arabic];
    let (urdu_id, urdu_family, urdu_height) = URDU[s.urdu];

    let arabic_style = TextStyle { line_height: 57.0, ..TextStyle::new(30.0, arabic_weight).family(Family::Named(arabic_family)) };
    let urdu_style = TextStyle { line_height: 20.0 * urdu_height, ..TextStyle::new(20.0, 400.0).family(Family::Named(urdu_family)) };
    let marker_style = TextStyle::new(16.0, 400.0).family(Family::Named(HAFS));
    let title_style = TextStyle::new(15.0, 600.0);
    let name_style = TextStyle::new(28.0, 400.0).family(Family::Named(HAFS));

    let mut layer = Vec::new();
    let arabic_ids = ARABIC.map(|f| f.0);
    let urdu_ids = URDU.map(|f| f.0);
    let toolbar = row().flex_wrap().items_center().gap(12.0).px(16.0).pt(36.0).pb(8.0).w_full()
        .child(selector(&k, &mut layer, "Arabic font", &arabic_ids, arabic_id, s.open == Some("Arabic font"), |s, i| s.arabic = i))
        .child(selector(&k, &mut layer, "Urdu", &urdu_ids, urdu_id, s.open == Some("Urdu"), |s, i| s.urdu = i))
        .child(selector(&k, &mut layer, "Database", &DATASETS, DATASETS[s.dataset], s.open == Some("Database"), |s, i| s.dataset = i));

    let mut list = col().w_full();
    for copy in 0..s.repeat {
        for surah in &sample().arabic[DATASETS[s.dataset]].surahs {
            list = list.child(
                col().key(format!("h{copy}-{}", surah.number)).w_full().items_center().py(20.0).fill(c("#F5F5F5"))
                    .child(text(format!("{} — Surah {}", surah.name_en, surah.number)).font(title_style).color(Color::BLACK))
                    .child(text(surah.name_ar.clone()).font(name_style).color(accent).mt(4.0)),
            );
            list = list.child(col().w_full().h(1.0).fill(rule));
            for ayah in &surah.ayahs {
                list = list.child(
                    col().key(format!("a{copy}-{}-{}", surah.number, ayah.n)).w_full().px(32.0).py(16.0).gap(8.0)
                        .child(text(ayah.text.clone()).font(arabic_style).color(Color::BLACK).align(rune::Align::End).w_full())
                        .child(text(urdu_for(surah.number, ayah.n)).font(urdu_style).color(c("#00000089")).align(rune::Align::End).w_full())
                        .child(text(ayah_marker(ayah.n)).font(marker_style).color(accent).align(rune::Align::End).w_full()),
                );
                list = list.child(col().w_full().h(1.0).fill(rule));
            }
        }
    }

    let body = col().w_full().h_full().fill(Color::WHITE)
        .child(toolbar)
        .child(col().w_full().h(1.0).fill(rule))
        .child(col().key("reader").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list));
    rune_kit::root(body, layer)
}

/// Headless: `--snap out.png --scene ur-gulzar`.
const SCENES: &[(&str, fn(&mut State))] = &[
    ("ar-amiri", |s| s.arabic = 0),
    ("ar-scheherazade-new", |s| s.arabic = 1),
    ("ar-noto-naskh-arabic", |s| s.arabic = 2),
    ("ar-kfgqpc-hafs", |s| s.arabic = 3),
    ("ar-kfgqpc-taha-v1", |s| s.arabic = 4),
    ("ar-kfgqpc-taha-v1-bold", |s| s.arabic = 5),
    ("ur-noto-nastaliq-urdu", |s| s.urdu = 0),
    ("ur-gulzar", |s| s.urdu = 1),
    ("ur-awami-nastaliq", |s| s.urdu = 2),
    ("ur-mehr-nastaliq-web", |s| s.urdu = 3),
    ("ur-katibeh", |s| s.urdu = 4),
    ("ur-mirza", |s| s.urdu = 5),
    ("ur-lateef", |s| s.urdu = 6),
    ("ur-scheherazade-new", |s| s.urdu = 7),
    ("ur-amiri", |s| s.urdu = 8),
    ("ur-aref-ruqaa", |s| s.urdu = 9),
    ("ur-lalezar", |s| s.urdu = 10),
    ("ur-noto-sans-arabic", |s| s.urdu = 11),
    ("db-uthmani", |s| s.dataset = 0),
    ("db-simple", |s| s.dataset = 1),
    ("db-indopak", |s| s.dataset = 2),
    ("open-arabic", |s| s.open = Some("Arabic font")),
];

fn main() {
    rune::run(AppSpec::<State> {
        fonts: FONTS,
        families: ("Noto Sans Arabic", "Noto Sans Arabic"),
        setup: Some(setup),
        poll: Some(poll),
        scenes: SCENES,
        ..AppSpec::new("Quran sample (Rune)", (1200.0, 830.0), init, view)
    });
}
