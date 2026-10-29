use std::borrow::Cow;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AppContext as _, Context, FocusHandle, IntoElement, InteractiveElement as _, KeyBinding,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled, TextAlign, Window, div, px, relative,
};
use serde::Deserialize;

gpui_kit::actions!(reader, [ScrollDown, ScrollUp]);

/// Family name stored inside uthmanic-hafs-v22.ttf (not the filename).
const HAFS_FAMILY: &str = "kfgqpc_hafs_uthmanic_script";
const URDU_FAMILY: &str = "Mehr Nastaliq Web";

#[derive(Deserialize)]
struct SampleData {
    surahs: Vec<SampleSurah>,
}

#[derive(Deserialize)]
struct SampleSurah {
    number: u32,
    name: SampleName,
    ayahs: Vec<SampleAyah>,
}

#[derive(Deserialize)]
struct SampleName {
    en: String,
    ar: String,
}

#[derive(Deserialize)]
struct SampleAyah {
    n: u32,
    ar: String,
    ur: String,
}

impl From<&SampleAyah> for Ayah {
    fn from(s: &SampleAyah) -> Self {
        Self {
            n: s.n,
            ar: s.ar.clone().into(),
            ur: s.ur.clone().into(),
        }
    }
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
    surahs: Vec<Surah>,
    scroll: ScrollHandle,
    focus: FocusHandle,
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

impl ReaderView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            surahs: Self::load_surahs(),
            scroll: ScrollHandle::new(),
            focus,
        }
    }

    fn load_surahs() -> Vec<Surah> {
        let raw = include_str!("../data/quran-sample.json");
        let sample: SampleData = serde_json::from_str(raw).expect("parse quran-sample.json");
        sample
            .surahs
            .into_iter()
            .map(|s| Surah {
                number: s.number,
                name_en: s.name.en.into(),
                name_ar: s.name.ar.into(),
                ayahs: s.ayahs.iter().map(Ayah::from).collect(),
            })
            .collect()
    }

    /// GPUI scroll offsets run from 0 down to -max_offset.y.
    fn scroll_by(&self, dy: f32) {
        let mut next = self.scroll.offset();
        next.y -= px(dy);
        let max = self.scroll.max_offset();
        next.y = next.y.clamp(-max.y, px(0.));
        self.scroll.set_offset(next);
    }

    fn render_ayah(&self, ayah: &Ayah, cx: &Context<Self>) -> impl IntoElement {
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
            .child(
                div()
                    .font_family(HAFS_FAMILY)
                    .w_full()
                    .text_align(TextAlign::Right)
                    .text_size(px(30.))
                    .line_height(relative(1.9))
                    .text_color(cx.theme().foreground)
                    .child(ayah.ar.clone()),
            )
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
                    .font_family(HAFS_FAMILY)
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
                    .font_family(HAFS_FAMILY)
                    .text_size(px(28.))
                    .text_color(cx.theme().primary)
                    .child(surah.name_ar.clone()),
            )
    }
}

impl Render for ReaderView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            reader = reader.children(surah.ayahs.iter().map(|a| self.render_ayah(a, cx)));
        }

        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
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
                    .size_full()
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

            cx.text_system()
                .add_fonts(vec![
                    // GPUI's Apple text system refuses to load any font whose
                    // cmap lacks a Latin 'm' (its em-measurement guard), and
                    // the original Quran/Urdu fonts have no Latin at all — so
                    // ship copies with 'm' mapped to the space glyph.
                    Cow::Borrowed(include_bytes!("../fonts/uthmanic-hafs-appui.ttf").as_slice()),
                    Cow::Borrowed(include_bytes!("../fonts/MehrNastaliqWeb-appui.ttf").as_slice()),
                ])
                .expect("Failed to load Quran fonts");

            let families = cx.text_system().all_font_names();
            assert!(
                families.iter().any(|f| f == HAFS_FAMILY),
                "hafs font missing"
            );
            assert!(
                families.iter().any(|f| f == URDU_FAMILY),
                "urdu font missing"
            );

            cx.bind_keys([
                KeyBinding::new("pagedown", ScrollDown, Some("reader")),
                KeyBinding::new("pageup", ScrollUp, Some("reader")),
            ]);

            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds::new(
                        gpui_kit::point(px(60.), px(60.)),
                        gpui_kit::size(px(900.), px(760.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| ReaderView::new(window, cx)),
            )
            .expect("Failed to open window");
        });
}
