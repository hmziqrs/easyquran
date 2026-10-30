use std::borrow::Cow;

use gpui_kit::base::h_flex;
use gpui_kit::component::button::Button;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::{IndexPath, Sizable as _};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, InteractiveElement as _,
    KeyBinding, ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled, Subscription, TextAlign, Window, div, px, relative,
};
use serde::Deserialize;

gpui_kit::actions!(reader, [ScrollDown, ScrollUp, NextFont, NextDataset]);

/// GPUI's Apple text system refuses any font whose cmap lacks a Latin 'm'
/// (em-measurement guard) and the Quran faces have no Latin at all — every
/// TTF in ../fonts is a converted/patched copy with 'm' → space.
const URDU_FAMILY: &str = "Mehr Nastaliq Web";
const URDU_FONT: &[u8] = include_bytes!("../fonts/mehr-nastaliq.ttf");

struct ArabicFont {
    /// Font id used by the web app (`web/src/lib/config/reader-fonts.ts`).
    id: &'static str,
    /// Family name stored inside the TTF.
    family: &'static str,
    weight: Option<gpui_kit::FontWeight>,
    file: &'static [u8],
}

macro_rules! font {
    ($id:literal, $family:literal, $weight:expr, $file:literal) => {
        ArabicFont {
            id: $id,
            family: $family,
            weight: $weight,
            file: include_bytes!(concat!("../fonts/", $file)),
        }
    };
}

/// Every Arabic face the web reader offers, same ids as `ARABIC_FONTS`.
const ARABIC_FONTS: &[ArabicFont] = &[
    font!("amiri", "Amiri", None, "amiri.ttf"),
    font!("scheherazade-new", "Scheherazade New", None, "scheherazade-new.ttf"),
    font!("noto-naskh-arabic", "Noto Naskh Arabic", None, "noto-naskh-arabic.ttf"),
    font!("kfgqpc-hafs", "kfgqpc_hafs_uthmanic_script", None, "kfgqpc-hafs.ttf"),
    font!("kfgqpc-taha-v1", "KFGQPC Uthman Taha Naskh", None, "kfgqpc-taha-v1.ttf"),
    font!(
        "kfgqpc-taha-v1-bold",
        "KFGQPC Uthman Taha Naskh",
        Some(gpui_kit::FontWeight::BOLD),
        "kfgqpc-taha-v1-bold.ttf"
    ),
];

const DATASETS: &[&str] = &["uthmani", "simple", "indopak"];
const DEFAULT_FONT_IX: usize = 3; // kfgqpc-hafs, the web reader default
const DEFAULT_DATASET: usize = 0; // uthmani

#[derive(Deserialize)]
struct SampleFile {
    arabic: std::collections::BTreeMap<String, DatasetData>,
    translations: TranslationsData,
}

#[derive(Deserialize)]
struct TranslationsData {
    ur: std::collections::BTreeMap<String, Vec<AyahData>>,
}

#[derive(Deserialize)]
struct DatasetData {
    surahs: Vec<SurahData>,
}

#[derive(Deserialize)]
struct SurahData {
    number: u32,
    name_en: String,
    name_ar: String,
    ayahs: Vec<AyahData>,
}

#[derive(Deserialize)]
struct AyahData {
    n: u32,
    text: String,
}

struct Ayah {
    n: u32,
    ar: SharedString,
    ur: SharedString,
}

struct Surah {
    number: u32,
    name_en: SharedString,
    name_ar: SharedString,
    ayahs: Vec<Ayah>,
}

struct ReaderView {
    font_ix: usize,
    dataset_ix: usize,
    surahs: Vec<Surah>,
    scroll: ScrollHandle,
    focus: FocusHandle,
    font_select: Entity<SelectState<Vec<String>>>,
    db_select: Entity<SelectState<Vec<String>>>,
    _subscriptions: Vec<Subscription>,
}

impl ReaderView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();

        let sample: SampleFile =
            serde_json::from_str(include_str!("../data/quran-sample.json"))
                .expect("parse quran-sample.json");

        let font_select = cx.new(|cx| {
            SelectState::new(
                ARABIC_FONTS.iter().map(|f| f.id.to_string()).collect(),
                Some(IndexPath::new(DEFAULT_FONT_IX)),
                window,
                cx,
            )
        });
        let db_select = cx.new(|cx| {
            SelectState::new(
                DATASETS.iter().map(|id| id.to_string()).collect(),
                Some(IndexPath::new(DEFAULT_DATASET)),
                window,
                cx,
            )
        });

        let mut _subscriptions = Vec::new();
        _subscriptions.push(cx.subscribe_in(
            &font_select,
            window,
            |this, _state, event: &SelectEvent<Vec<String>>, _window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if let Some(ix) = ARABIC_FONTS.iter().position(|f| f.id == id.as_str()) {
                        this.font_ix = ix;
                        this.scroll.set_offset(Default::default());
                        cx.notify();
                    }
                }
            },
        ));
        let surahs = Self::build_surahs(&sample, DATASETS[DEFAULT_DATASET]);
        let sample_for_db = std::cell::RefCell::new(Some(sample));
        let sample_for_db = std::rc::Rc::new(sample_for_db);
        let sample_for_db_listener = std::rc::Rc::clone(&sample_for_db);
        _subscriptions.push(cx.subscribe_in(
            &db_select,
            window,
            move |this, _state, event: &SelectEvent<Vec<String>>, _window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if let Some(ix) = DATASETS.iter().position(|d| *d == id.as_str()) {
                        let sample = sample_for_db_listener.borrow_mut().take().unwrap_or_else(|| {
                            serde_json::from_str(include_str!("../data/quran-sample.json"))
                                .expect("parse quran-sample.json")
                        });
                        this.dataset_ix = ix;
                        this.surahs = Self::build_surahs(&sample, id.as_str());
                        this.scroll.set_offset(Default::default());
                        cx.notify();
                    }
                }
            },
        ));

        let view = Self {
            font_ix: DEFAULT_FONT_IX,
            dataset_ix: DEFAULT_DATASET,
            surahs,
            scroll: ScrollHandle::new(),
            focus,
            font_select,
            db_select,
            _subscriptions,
        };
        // Focus the reader last: SelectState grabs focus while built above.
        view.focus.focus(window, cx);
        view
    }

    fn build_surahs(sample: &SampleFile, dataset: &str) -> Vec<Surah> {
        let arabic = &sample.arabic[dataset];
        let urdu = &sample.translations.ur;
        arabic
            .surahs
            .iter()
            .map(|surah| {
                let ur_list = urdu.get(&surah.number.to_string());
                Surah {
                    number: surah.number,
                    name_en: surah.name_en.clone().into(),
                    name_ar: surah.name_ar.clone().into(),
                    ayahs: surah
                        .ayahs
                        .iter()
                        .map(|ayah| Ayah {
                            n: ayah.n,
                            ar: ayah.text.clone().into(),
                            ur: ur_list
                                .and_then(|list| {
                                    list.iter().find(|u| u.n == ayah.n).map(|u| u.text.clone())
                                })
                                .unwrap_or_default()
                                .into(),
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// Select the next Arabic font (wraps) and sync the picker.
    fn cycle_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.font_ix = (self.font_ix + 1) % ARABIC_FONTS.len();
        self.font_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(self.font_ix)), window, cx);
        });
        // set_selected_index moves focus into the picker; take it back so
        // further reader keys keep dispatching.
        self.focus.focus(window, cx);
        self.scroll.set_offset(Default::default());
        cx.notify();
    }

    /// Select the next Arabic dataset (wraps) and sync the picker.
    fn cycle_dataset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dataset_ix = (self.dataset_ix + 1) % DATASETS.len();
        let dataset = DATASETS[self.dataset_ix];
        let sample: SampleFile = serde_json::from_str(include_str!("../data/quran-sample.json"))
            .expect("parse quran-sample.json");
        self.surahs = Self::build_surahs(&sample, dataset);
        self.db_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(self.dataset_ix)), window, cx);
        });
        self.focus.focus(window, cx);
        self.scroll.set_offset(Default::default());
        cx.notify();
    }

    /// GPUI scroll offsets run from 0 down to -max_offset.y.
    fn scroll_by(&self, dy: f32) {
        let mut next = self.scroll.offset();
        next.y -= px(dy);
        let max = self.scroll.max_offset();
        next.y = next.y.clamp(-max.y, px(0.));
        self.scroll.set_offset(next);
    }

    fn render_ayah(&self, ayah: &Ayah, font: &ArabicFont, cx: &Context<Self>) -> impl IntoElement {
        let mut arabic = div()
            .font_family(font.family)
            .w_full()
            .text_align(TextAlign::Right)
            .text_size(px(30.))
            .line_height(relative(1.9))
            .text_color(cx.theme().foreground)
            .child(ayah.ar.clone());
        if let Some(weight) = font.weight {
            arabic = arabic.font_weight(weight);
        }

        div()
            .flex()
            .flex_col()
            .items_end()
            .w_full()
            .px_8()
            .py_4()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(arabic)
            .child(
                div()
                    .font_family(URDU_FAMILY)
                    .w_full()
                    .text_align(TextAlign::Right)
                    .text_size(px(20.))
                    .line_height(relative(2.9))
                    .text_color(cx.theme().muted_foreground)
                    .child(ayah.ur.clone()),
            )
            .child(
                div()
                    .font_family(ARABIC_FONTS[DEFAULT_FONT_IX].family)
                    .text_size(px(16.))
                    .text_color(cx.theme().primary)
                    .child(ayah_marker(ayah.n)),
            )
    }

    fn render_surah_header(&self, surah: &Surah, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .py_5()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child(format!(
                        "{} — Surah {}",
                        surah.name_en.clone(),
                        surah.number
                    )),
            )
            .child(
                div()
                    .font_family(ARABIC_FONTS[DEFAULT_FONT_IX].family)
                    .text_size(px(28.))
                    .text_color(cx.theme().primary)
                    .child(surah.name_ar.clone()),
            )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let font = &ARABIC_FONTS[self.font_ix];
        h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(div().text_size(px(13.)).child("Arabic font"))
            .child(Select::new(&self.font_select).w(px(240.)))
            .child(div().text_size(px(13.)).child("Database"))
            .child(Select::new(&self.db_select).w(px(160.)))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} / {}", font.id, font.family)),
            )
            .child(
                Button::new("scroll-top")
                    .xsmall()
                    .label("Top")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.scroll.set_offset(Default::default());
                        cx.notify();
                    })),
            )
    }
}

/// Ornate-parenthesized Arabic-Indic ayah number, e.g. ﴿٧﴾.
fn ayah_marker(n: u32) -> String {
    let digits: String = n
        .to_string()
        .chars()
        .map(|c| char::from_u32(0x0660 + c.to_digit(10).unwrap_or(0)).unwrap_or(c))
        .collect();
    format!("﴿{digits}﴾")
}

impl Render for ReaderView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let font = &ARABIC_FONTS[self.font_ix];
        let mut reader = div()
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .min_w_0()
            .children(
                self.surahs
                    .iter()
                    .map(|s| self.render_surah_header(s, cx)),
            );
        for surah in &self.surahs {
            reader = reader.children(
                surah
                    .ayahs
                    .iter()
                    .map(|a| self.render_ayah(a, font, cx)),
            );
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .id("reader-scroll")
                    .key_context("reader")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|this, _: &ScrollDown, _, cx| {
                        this.scroll_by(360.);
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &ScrollUp, _, cx| {
                        this.scroll_by(-360.);
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &NextFont, window, cx| {
                        this.cycle_font(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &NextDataset, window, cx| {
                        this.cycle_dataset(window, cx);
                    }))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(reader),
            )
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);

            let mut font_files: Vec<Cow<'static, [u8]>> =
                ARABIC_FONTS.iter().map(|f| Cow::Borrowed(f.file)).collect();
            font_files.push(Cow::Borrowed(URDU_FONT));
            cx.text_system()
                .add_fonts(font_files)
                .expect("Failed to load Quran fonts");

            for family in ARABIC_FONTS.iter().map(|f| f.family).chain([URDU_FAMILY]) {
                assert!(
                    cx.text_system().all_font_names().iter().any(|f| f == family),
                    "font family missing: {family}"
                );
            }

            cx.bind_keys([
                KeyBinding::new("pagedown", ScrollDown, Some("reader")),
                KeyBinding::new("pageup", ScrollUp, Some("reader")),
                KeyBinding::new("ctrl-f", NextFont, Some("reader")),
                KeyBinding::new("ctrl-d", NextDataset, Some("reader")),
            ]);

            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds::new(
                        gpui_kit::point(px(60.), px(60.)),
                        gpui_kit::size(px(940.), px(860.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| ReaderView::new(window, cx)),
            )
            .expect("Failed to open window");

            cx.activate(true);
        });
}
