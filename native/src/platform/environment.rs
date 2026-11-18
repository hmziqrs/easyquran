//! The device and environment page: what NSH 0.5.0 exposes about the surface,
//! read in bindings so a change (a rotation, an appearance switch, a test's
//! pin) re-renders it. The device model is not exposed on 0.5.0 and says so.
//! Word values translate with the locale; the system's numbers and tags stay
//! as reported.

use nsh_base::theme;
use nsh_core::{
    platform::{Idiom, Lifecycle},
    prelude::*,
};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::{
    locale,
    widgets::{hint, info_row, localized, panel, rule},
};

/// A yes / no read as a word of the current locale, live.
fn yes_no(value: impl Fn() -> bool + Copy + 'static) -> impl Fn() -> String {
    move || {
        locale::app_locale().get();
        if value() {
            t!("platform.value.yes").to_string()
        } else {
            t!("platform.value.no").to_string()
        }
    }
}

/// The appearance, as a word of the current locale.
fn appearance_value() -> impl Fn() -> String {
    move || {
        locale::app_locale().get();
        if dark() {
            t!("platform.value.dark").to_string()
        } else {
            t!("platform.value.light").to_string()
        }
    }
}

/// The idiom, as a word of the current locale (its `Debug` name is not copy).
fn idiom_value() -> impl Fn() -> String {
    move || {
        locale::app_locale().get();
        let key = match idiom() {
            Idiom::Phone => "platform.idiom.phone",
            Idiom::Pad => "platform.idiom.pad",
            Idiom::Mac => "platform.idiom.mac",
            Idiom::Tv => "platform.idiom.tv",
            Idiom::Desktop => "platform.idiom.desktop",
            Idiom::Terminal => "platform.idiom.terminal",
        };
        t!(key).to_string()
    }
}

/// The lifecycle, as a word of the current locale.
fn lifecycle_value() -> impl Fn() -> String {
    move || {
        locale::app_locale().get();
        let key = match lifecycle() {
            Lifecycle::Active => "platform.lifecycle.active",
            Lifecycle::Inactive => "platform.lifecycle.inactive",
            Lifecycle::Background => "platform.lifecycle.background",
        };
        t!(key).to_string()
    }
}

/// One safe-area inset, as a row of its points.
fn edge_row(label: &'static str, edge: impl Fn() -> f32 + 'static) -> AnyElement {
    info_row(label, move || format!("{} pt", edge().round()))
}

/// The device card: OS, idiom, locale, appearance and the rest of the
/// environment's plain reads. The OS name, version and locale tag are the
/// system's identifiers and stay as reported.
fn device_card() -> AnyElement {
    panel(
        "platform.device.title",
        [
            info_row("platform.device.os", || os().name().to_owned()),
            info_row("platform.device.version", || {
                let (major, minor) = os_version();
                format!("{major}.{minor}")
            }),
            info_row("platform.device.idiom", idiom_value()),
            info_row("platform.device.locale", || locale().as_str().to_owned()),
            info_row("platform.device.appearance", appearance_value()),
            rule(),
            info_row("platform.device.font_scale", || {
                format!("{:.2}×", font_scale())
            }),
            info_row("platform.device.pixel_ratio", || {
                format!("{:.2}×", pixel_ratio())
            }),
            rule(),
            info_row("platform.device.reader", yes_no(screen_reader)),
            info_row("platform.device.reduce_motion", yes_no(reduce_motion)),
            info_row("platform.device.lifecycle", lifecycle_value()),
            rule(),
            info_row("platform.device.window", || {
                let window = window();
                format!(
                    "{} × {} pt",
                    window.width.0.round(),
                    window.height.0.round()
                )
            }),
            info_row("platform.device.catalyst", yes_no(is_catalyst)),
        ],
    )
}

/// The safe-area card: the four insets, live in points.
fn safe_area_card() -> AnyElement {
    panel(
        "platform.safe_area.title",
        [
            edge_row("platform.safe_area.top", || safe_area().top.0),
            edge_row("platform.safe_area.right", || safe_area().right.0),
            edge_row("platform.safe_area.bottom", || safe_area().bottom.0),
            edge_row("platform.safe_area.left", || safe_area().left.0),
            rule(),
            hint("platform.safe_area.hint"),
        ],
    )
}

/// What the platform keeps to itself on 0.5.0, so a missing read reads as a
/// boundary, not a bug.
fn limits_card() -> AnyElement {
    panel(
        "platform.limits.title",
        [view()
            .flex_row()
            .justify_between()
            .gap_3()
            .min_w(px(0.0))
            .test_id("platform-device-model")
            .child(
                text(localized("platform.device.model"))
                    .flex_1()
                    .text_sm()
                    .text_color(theme().muted_foreground),
            )
            .child(text(localized("platform.limits.not_exposed")).text_sm())
            .into_element()],
    )
}

/// The device and environment page's cards.
pub(crate) fn cards() -> Vec<AnyElement> {
    vec![device_card(), safe_area_card(), limits_card()]
}
