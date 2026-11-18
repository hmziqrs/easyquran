//! More of the platform's own controls, beside `ui::native`'s button,
//! switch and segmented control: slider, checkbox, radio group, pop-up
//! button, stepper, progress, spinner, date picker, color well, text and
//! search fields, and tabs. AppKit draws them on macOS, UIKit on iOS where
//! it has the control; everywhere else (and for the iOS gaps: checkbox,
//! radio group, tabs) each function falls back to the app's own control,
//! bound to the same signal, so a screen can swap one for the other.
//!
//! Alerts and action sheets need no component: `nsh_base::alert_dialog`
//! already shows an `NSAlert` sheet on macOS and a `UIAlertController` on
//! iOS (see [`native_alert`]).

use nsh_base::{Date, theme};
use nsh_core::{JsonValue, color::Rgba, element::NativeEvent, prelude::*};

use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use crate::ui::{controls, field, pickers, select, tabs, text};

/// The components' names among the native components.
pub const SLIDER: &str = "starter.kit-slider";
pub const CHECKBOX: &str = "starter.kit-checkbox";
pub const RADIO: &str = "starter.kit-radio";
pub const POPUP: &str = "starter.kit-popup";
pub const STEPPER: &str = "starter.kit-stepper";
pub const PROGRESS: &str = "starter.kit-progress";
pub const SPINNER: &str = "starter.kit-spinner";
pub const DATE: &str = "starter.kit-date";
pub const COLOR: &str = "starter.kit-color";
pub const TEXT: &str = "starter.kit-text";
pub const SEARCH: &str = "starter.kit-search";
pub const TABS: &str = "starter.kit-tabs";
pub const GLASS_BUTTON: &str = "starter.kit-glass-button";
pub const GLASS_BAR: &str = "starter.kit-glass-bar";
pub const MATERIAL: &str = "starter.kit-material";

/// AppKit draws these controls here.
pub fn on_macos() -> bool {
    cfg!(target_os = "macos") && os() == nsh_core::platform::Os::MacOs
}

/// UIKit draws these controls here.
pub fn on_ios() -> bool {
    cfg!(target_os = "ios") && os() == nsh_core::platform::Os::Ios
}

fn rgb_of(color: Hsla) -> JsonValue {
    let c = Rgba::from(color);
    JsonValue::from(vec![f64::from(c.r), f64::from(c.g), f64::from(c.b)])
}

fn accent() -> JsonValue {
    let primary = theme().primary;
    rgb_of(if dark() { primary.dark } else { primary.light })
}

fn number(value: f64) -> JsonValue {
    serde_json::Number::from_f64(value).map_or(JsonValue::Null, JsonValue::Number)
}

/// A date at noon UTC, as seconds since 1970: noon keeps the same day in
/// every time zone the native picker might show it in.
fn epoch_of(date: Option<Date>) -> JsonValue {
    date.and_then(|d| d.start())
        .and_then(|d| d.and_hms_opt(12, 0, 0))
        .map_or(JsonValue::Null, |d| number(d.and_utc().timestamp() as f64))
}

fn date_of(epoch: f64) -> Option<Date> {
    chrono::DateTime::from_timestamp(epoch as i64, 0).map(|d| Date::from(d.date_naive()))
}

/// A system slider from `min` to `max`, writing `value`.
pub fn native_slider(value: Signal<f64>, min: f64, max: f64) -> AnyElement {
    if !(on_macos() || on_ios()) {
        return view()
            .w_full()
            .child(controls::slider(value, min, max))
            .into_element();
    }
    nsh_core::element::native(SLIDER)
        .prop("min", number(min))
        .prop("max", number(max))
        .prop("value", move || number(value.get()))
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if let Some(v) = e.data["value"].as_f64() {
                value.set(v);
            }
        })
        .w_full()
        .into_element()
}

/// A labelled checkbox (AppKit's; iOS has none, so the app's draws there).
pub fn native_checkbox(label: &'static str, on: Signal<bool>) -> AnyElement {
    if !on_macos() {
        return controls::checkbox_row(label, on).into_element();
    }
    nsh_core::element::native(CHECKBOX)
        .prop("title", JsonValue::from(label))
        .prop("on", move || JsonValue::from(on.get()))
        .on_native_event(move |e: &NativeEvent| {
            if let Some(next) = e.data["on"].as_bool() {
                on.set(next);
            }
        })
        .self_start()
        .into_element()
}

/// A radio group over `options` writing the chosen label (AppKit's radio
/// buttons; iOS has none).
pub fn native_radio_group(
    options: &'static [&'static str],
    value: Signal<SharedString>,
) -> AnyElement {
    if !on_macos() {
        return controls::radio_group_control(options, value).into_element();
    }
    nsh_core::element::native(RADIO)
        .prop("items", JsonValue::from(options.to_vec()))
        .prop("selected", move || {
            let picked = value.get();
            JsonValue::from(
                options
                    .iter()
                    .position(|o| *o == picked.as_ref())
                    .map_or(-1, |at| at as i64),
            )
        })
        .on_native_event(move |e: &NativeEvent| {
            if let Some(o) = e.data["index"]
                .as_u64()
                .and_then(|at| options.get(at as usize))
            {
                value.set(SharedString::from(*o));
            }
        })
        .self_start()
        .into_element()
}

/// A pop-up button over `options`: `NSPopUpButton` on macOS, a `UIButton`
/// whose menu changes its selection on iOS.
pub fn native_popup(
    name: &'static str,
    options: &'static [&'static str],
    value: Signal<Option<SharedString>>,
) -> AnyElement {
    if !(on_macos() || on_ios()) {
        let items = options.iter().map(|o| select::choice(o, o)).collect();
        return select::select_field(name, items, value, "Choose").into_element();
    }
    nsh_core::element::native(POPUP)
        .prop("items", JsonValue::from(options.to_vec()))
        .prop("selected", move || {
            let picked = value.get();
            JsonValue::from(
                options
                    .iter()
                    .position(|o| Some(*o) == picked.as_deref())
                    .map_or(0, |at| at as i64),
            )
        })
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if let Some(o) = e.data["index"]
                .as_u64()
                .and_then(|at| options.get(at as usize))
            {
                value.set(Some(SharedString::from(*o)));
            }
        })
        .self_start()
        .into_element()
}

/// A stepper nudging `value` by `step` within `min..=max`.
pub fn native_stepper(value: Signal<f64>, min: f64, max: f64, step: f64) -> AnyElement {
    if !(on_macos() || on_ios()) {
        let nudge = move |by: f64| value.set((value.get() + by).clamp(min, max));
        return view()
            .flex_row()
            .gap_1()
            .child(button_with(
                "−",
                ButtonVariant::Outline,
                ButtonSize::Small,
                false,
                Some("Decrease"),
                move || nudge(-step),
            ))
            .child(button_with(
                "+",
                ButtonVariant::Outline,
                ButtonSize::Small,
                false,
                Some("Increase"),
                move || nudge(step),
            ))
            .into_element();
    }
    nsh_core::element::native(STEPPER)
        .prop("min", number(min))
        .prop("max", number(max))
        .prop("step", number(step))
        .prop("value", move || number(value.get()))
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if let Some(v) = e.data["value"].as_f64() {
                value.set(v);
            }
        })
        .self_start()
        .into_element()
}

/// A determinate progress bar at `value` (0 to 1).
pub fn native_progress(value: impl Fn() -> f64 + Copy + 'static) -> AnyElement {
    if !(on_macos() || on_ios()) {
        // The app's bar runs from 0 to 100.
        return controls::progress(move || (value() * 100.0) as f32).into_element();
    }
    nsh_core::element::native(PROGRESS)
        .prop("value", move || number(value()))
        .prop("tint", accent)
        .w_full()
        .into_element()
}

/// The system's activity indicator.
pub fn native_spinner() -> AnyElement {
    if !(on_macos() || on_ios()) {
        return controls::progress_indeterminate().into_element();
    }
    nsh_core::element::native(SPINNER)
        .self_start()
        .into_element()
}

/// A system date picker writing `value`: compact (a field with a stepper
/// on macOS, a button opening a calendar on iOS), or the full `calendar`.
pub fn native_date(value: Signal<Option<Date>>, calendar: bool) -> AnyElement {
    if !(on_macos() || on_ios()) {
        return if calendar {
            pickers::calendar_of(value).into_element()
        } else {
            pickers::date_picker_of("Date", value, "Pick a date").into_element()
        };
    }
    nsh_core::element::native(DATE)
        .prop(
            "style",
            JsonValue::from(if calendar { "calendar" } else { "compact" }),
        )
        .prop("value", move || epoch_of(value.get()))
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if let Some(epoch) = e.data["value"].as_f64() {
                value.set(date_of(epoch));
            }
        })
        .self_start()
        .into_element()
}

/// The system color well: `NSColorWell`, `UIColorWell`. Opens the
/// system's color panel or sheet.
pub fn native_color_well(value: Signal<Hsla>) -> AnyElement {
    if !(on_macos() || on_ios()) {
        return crate::ui::color::color_picker_of(value, |_| {}).into_element();
    }
    nsh_core::element::native(COLOR)
        .prop("color", move || rgb_of(value.get()))
        .on_native_event(move |e: &NativeEvent| {
            let c = &e.data["rgb"];
            if let (Some(r), Some(g), Some(b)) = (c[0].as_f64(), c[1].as_f64(), c[2].as_f64()) {
                value.set(Hsla::from(Rgba {
                    r: r as f32,
                    g: g as f32,
                    b: b as f32,
                    a: 1.0,
                }));
            }
        })
        .self_start()
        .into_element()
}

/// A system text field (`search`: the search field, with its glass and
/// clear button) writing `value` as the user types.
pub fn native_text_field(
    value: Signal<String>,
    placeholder: &'static str,
    search: bool,
) -> AnyElement {
    if !(on_macos() || on_ios()) {
        return field::text_field(value, placeholder).into_element();
    }
    nsh_core::element::native(if search { SEARCH } else { TEXT })
        .prop("placeholder", JsonValue::from(placeholder))
        .prop("text", move || JsonValue::from(value.get()))
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if let Some(next) = e.data["text"].as_str()
                && next != value.get()
            {
                value.set(next.to_owned());
            }
        })
        .w_full()
        .into_element()
}

/// Tabs over `labels`, each showing its line of `bodies`: on macOS a box
/// headed by a segmented control, as `NSTabView` draws them; iOS has no tab control inside a page, so its segmented control
/// heads the body there; elsewhere the app's tab row.
pub fn native_tabs(
    labels: &'static [&'static str],
    bodies: &'static [&'static str],
    selected: Signal<usize>,
) -> AnyElement {
    let body = move || {
        view().p_3().child(text::body(move || {
            SharedString::from(*bodies.get(selected.get()).unwrap_or(&""))
        }))
    };
    if on_ios() {
        return view()
            .flex_col()
            .gap_2()
            .child(crate::ui::native::native_segmented(labels, selected))
            .child(body())
            .into_element();
    }
    if !on_macos() {
        return view()
            .flex_col()
            .gap_2()
            .child(tabs::tab_bar(
                move || selected.get(),
                labels,
                move |at| selected.set(at),
            ))
            .child(body())
            .into_element();
    }
    nsh_core::element::native(TABS)
        .prop("items", JsonValue::from(labels.to_vec()))
        .prop("bodies", JsonValue::from(bodies.to_vec()))
        .prop("selected", move || JsonValue::from(selected.get()))
        .on_native_event(move |e: &NativeEvent| {
            if let Some(at) = e.data["index"].as_u64()
                && selected.get() != at as usize
            {
                selected.set(at as usize);
            }
        })
        .w_full()
        .h(px(150.0))
        .into_element()
}

/// A Liquid Glass button: `NSBezelStyleGlass` on macOS 26 and later, the
/// glass button configuration on iOS 26 and later (`prominent`: tinted
/// with the accent). Older systems draw their plain bezel.
pub fn native_glass_button(
    title: &'static str,
    symbol: Option<&'static str>,
    prominent: bool,
    on_press: impl Fn() + 'static,
) -> AnyElement {
    if !(on_macos() || on_ios()) {
        let variant = if prominent {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Outline
        };
        return button_with(title, variant, ButtonSize::Medium, false, None, on_press)
            .into_element();
    }
    nsh_core::element::native(GLASS_BUTTON)
        .prop("title", JsonValue::from(title))
        .prop("symbol", JsonValue::from(symbol.unwrap_or_default()))
        .prop(
            "style",
            JsonValue::from(if prominent { "prominent" } else { "regular" }),
        )
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if e.data["index"].is_number() {
                on_press();
            }
        })
        .self_start()
        .into_element()
}

/// A toolbar icon button: an image-only system button drawing SF Symbol
/// `symbol` (Liquid Glass on macOS 26 and iOS 26, borderless before),
/// named `label` for VoiceOver and its tooltip. Elsewhere the app's icon
/// button drawing `svg`.
pub fn native_icon_button(
    symbol: &'static str,
    svg: &'static str,
    label: &'static str,
    on_press: impl Fn() + 'static,
) -> AnyElement {
    if !(on_macos() || on_ios()) {
        return crate::ui::button::icon_svg_button(svg, label, on_press).into_element();
    }
    nsh_core::element::native(GLASS_BUTTON)
        .prop("title", JsonValue::from(""))
        .prop("symbol", JsonValue::from(symbol))
        .prop("text", JsonValue::from(label))
        .prop("style", JsonValue::from("regular"))
        .on_native_event(move |e: &NativeEvent| {
            if e.data["index"].is_number() {
                on_press();
            }
        })
        .into_element()
}

/// The system's window material behind a bar (`material`: "titlebar",
/// "header", "sidebar" or "window"), blending with what is behind the
/// window as AppKit's own bars do; an absolutely placed layer filling its
/// parent. `None` where AppKit does not draw it: the bar keeps its fill.
pub fn native_material(material: &'static str) -> Option<AnyElement> {
    if !on_macos() {
        return None;
    }
    Some(
        nsh_core::element::native(MATERIAL)
            .prop("style", JsonValue::from(material))
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .into_element(),
    )
}

/// A floating toolbar of SF Symbol buttons on one Liquid Glass pill:
/// `NSGlassEffectView`, `UIGlassEffect` (`clear`: the clear glass, for
/// busy backgrounds). Elsewhere the app's ghost buttons, named `labels`.
pub fn native_glass_bar(
    symbols: &'static [&'static str],
    labels: &'static [&'static str],
    clear: bool,
    on_press: impl Fn(usize) + 'static,
) -> AnyElement {
    if !(on_macos() || on_ios()) {
        let on_press = std::rc::Rc::new(on_press);
        return view()
            .flex_row()
            .self_start()
            .gap_1()
            .p_1()
            .rounded_full()
            // No system glass control here: NSH's glass (its blur where
            // the platform has no Liquid Glass) under a translucent fill.
            .glass_effect(nsh_core::GlassEffect::Clear)
            .backdrop_blur(px(30.0))
            .bg(move || theme().card.opacity(0.45))
            .border_1()
            .border_color(move || theme().border.opacity(0.5))
            .children(labels.iter().enumerate().map(|(at, label)| {
                let on_press = on_press.clone();
                button_with(
                    *label,
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || on_press(at),
                )
            }))
            .into_element();
    }
    nsh_core::element::native(GLASS_BAR)
        .prop("items", JsonValue::from(symbols.to_vec()))
        .prop("bodies", JsonValue::from(labels.to_vec()))
        .prop(
            "style",
            JsonValue::from(if clear { "clear" } else { "regular" }),
        )
        .on_native_event(move |e: &NativeEvent| {
            if let Some(at) = e.data["index"].as_u64() {
                on_press(at as usize);
            }
        })
        .self_start()
        .into_element()
}

/// The system's alert (`sheet`: its action sheet on iOS; macOS shows both
/// as a sheet on the window). `on_choice` gets the chosen action's label,
/// `None` on cancel.
pub fn native_alert(
    title: &'static str,
    message: &'static str,
    actions: &'static [&'static str],
    destructive: Option<&'static str>,
    sheet: bool,
    on_choice: impl Fn(Option<&'static str>) + 'static,
) {
    spawn(async move {
        let mut dialog = nsh_base::alert_dialog()
            .title(title)
            .description(message)
            .cancel("cancel", "Cancel");
        for action in actions {
            dialog = dialog.action(*action, *action);
        }
        if let Some(label) = destructive {
            dialog = dialog.destructive(label, label);
        }
        if sheet {
            dialog = dialog.action_sheet();
        }
        let answer = dialog.show().await.ok().and_then(|r| r.action);
        let chosen = answer.and_then(|id| {
            actions
                .iter()
                .chain(destructive.iter())
                .find(|a| **a == id.as_ref())
                .copied()
        });
        on_choice(chosen);
    });
}

/// The props a component keeps, to rebuild itself as they change.
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[derive(Default)]
struct Props {
    title: String,
    items: Vec<String>,
    bodies: Vec<String>,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    selected: isize,
    on: bool,
    text: String,
    placeholder: String,
    style: String,
    symbol: String,
    tint: Option<[f64; 3]>,
    color: Option<[f64; 3]>,
    /// A date value was given (`value` holds its epoch seconds).
    has_date: bool,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn rgb(value: Option<&JsonValue>) -> Option<[f64; 3]> {
    let list = value?.as_array()?;
    Some([
        list.first()?.as_f64()?,
        list.get(1)?.as_f64()?,
        list.get(2)?.as_f64()?,
    ])
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn strings(value: Option<&JsonValue>) -> Vec<String> {
    value
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Props {
    fn new() -> Self {
        Props {
            max: 1.0,
            step: 1.0,
            selected: -1,
            ..Props::default()
        }
    }

    /// Takes prop `name`; false for a prop it does not know.
    fn take(&mut self, name: &str, value: Option<&JsonValue>) -> bool {
        let text = || {
            value
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let num = || value.and_then(JsonValue::as_f64);
        match name {
            "title" => self.title = text(),
            "text" => self.text = text(),
            "placeholder" => self.placeholder = text(),
            "style" => self.style = text(),
            "symbol" => self.symbol = text(),
            "items" => self.items = strings(value),
            "bodies" => self.bodies = strings(value),
            "value" => {
                self.has_date = num().is_some();
                self.value = num().unwrap_or_default();
            }
            "min" => self.min = num().unwrap_or_default(),
            "max" => self.max = num().unwrap_or(1.0),
            "step" => self.step = num().unwrap_or(1.0),
            "selected" => self.selected = value.and_then(JsonValue::as_i64).unwrap_or(-1) as isize,
            "on" => self.on = value.and_then(JsonValue::as_bool).unwrap_or_default(),
            "tint" => self.tint = rgb(value),
            "color" => self.color = rgb(value),
            _ => return false,
        }
        true
    }
}

/// Which control a component is.
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Slider,
    Checkbox,
    Radio,
    Popup,
    Stepper,
    Progress,
    Spinner,
    Date,
    Color,
    Text,
    Search,
    Tabs,
    GlassButton,
    GlassBar,
    Material,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
const KINDS: [(&str, Kind); 15] = [
    (SLIDER, Kind::Slider),
    (CHECKBOX, Kind::Checkbox),
    (RADIO, Kind::Radio),
    (POPUP, Kind::Popup),
    (STEPPER, Kind::Stepper),
    (PROGRESS, Kind::Progress),
    (SPINNER, Kind::Spinner),
    (DATE, Kind::Date),
    (COLOR, Kind::Color),
    (TEXT, Kind::Text),
    (SEARCH, Kind::Search),
    (TABS, Kind::Tabs),
    (GLASS_BUTTON, Kind::GlassButton),
    (GLASS_BAR, Kind::GlassBar),
    (MATERIAL, Kind::Material),
];

#[cfg(target_os = "macos")]
pub use macos::register as register_macos;

#[cfg(target_os = "ios")]
pub use ios::register as register_ios;

/// AppKit's controls.
#[cfg(target_os = "macos")]
mod macos {
    use std::{
        ffi::{CStr, CString, c_char},
        rc::Rc,
    };

    use nsh_core::JsonValue;
    use nsh_platform_macos::{
        ComponentContext, Components, NativeComponent,
        objc2::{
            DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
            rc::Retained,
            runtime::{AnyClass, AnyObject, NSObject},
            sel,
        },
        objc2_app_kit::NSView,
        objc2_foundation::{NSPoint, NSRect, NSSize},
    };

    use super::{KINDS, Kind, Props};

    /// What the target reports for: its control's kind, and the component.
    struct Ivars {
        kind: Kind,
        context: ComponentContext,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the target only
        // reads its control and forwards to the component's context.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "NshStarterKitTarget"]
        #[ivars = Ivars]
        struct Target;

        impl Target {
            #[unsafe(method(act:))]
            fn act(&self, sender: &AnyObject) {
                let ivars = self.ivars();
                // SAFETY: each read is the documented getter of the control
                // `kind` made, `sender` being that control.
                let data = unsafe {
                    match ivars.kind {
                        Kind::Slider | Kind::Stepper => {
                            let v: f64 = msg_send![sender, doubleValue];
                            serde_json::json!({ "value": v })
                        }
                        Kind::Checkbox => {
                            let state: isize = msg_send![sender, state];
                            serde_json::json!({ "on": state == 1 })
                        }
                        Kind::Radio | Kind::GlassButton | Kind::GlassBar => {
                            let tag: isize = msg_send![sender, tag];
                            serde_json::json!({ "index": tag })
                        }
                        Kind::Popup => {
                            let at: isize = msg_send![sender, indexOfSelectedItem];
                            serde_json::json!({ "index": at })
                        }
                        Kind::Date => {
                            let date: Option<Retained<AnyObject>> = msg_send![sender, dateValue];
                            let Some(date) = date else { return };
                            let v: f64 = msg_send![&*date, timeIntervalSince1970];
                            serde_json::json!({ "value": v })
                        }
                        Kind::Color => {
                            let color: Retained<AnyObject> = msg_send![sender, color];
                            let space: Retained<AnyObject> =
                                msg_send![class(c"NSColorSpace"), sRGBColorSpace];
                            let srgb: Option<Retained<AnyObject>> =
                                msg_send![&*color, colorUsingColorSpace: &*space];
                            let Some(c) = srgb else { return };
                            let r: f64 = msg_send![&*c, redComponent];
                            let g: f64 = msg_send![&*c, greenComponent];
                            let b: f64 = msg_send![&*c, blueComponent];
                            serde_json::json!({ "rgb": [r, g, b] })
                        }
                        Kind::Text | Kind::Search => {
                            serde_json::json!({ "text": string_value(sender) })
                        }
                        Kind::Tabs => {
                            let at: isize = msg_send![sender, selectedSegment];
                            serde_json::json!({ "index": at })
                        }
                        Kind::Progress | Kind::Spinner | Kind::Material => return,
                    }
                };
                ivars.context.emit("change", data);
            }

            #[unsafe(method(controlTextDidChange:))]
            fn text_changed(&self, note: &AnyObject) {
                // SAFETY: NSNotification's `object`, here the text field.
                let field: Option<Retained<AnyObject>> = unsafe { msg_send![note, object] };
                if let Some(field) = field {
                    let text = unsafe { string_value(&field) };
                    self.ivars()
                        .context
                        .emit("change", serde_json::json!({ "text": text }));
                }
            }
        }
    );

    impl Target {
        fn new(mtm: MainThreadMarker, kind: Kind, context: ComponentContext) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars { kind, context });
            // SAFETY: NSObject's designated initializer.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn class(name: &CStr) -> &'static AnyClass {
        AnyClass::get(name).expect("an AppKit class")
    }

    fn ns_string(text: &str) -> Retained<AnyObject> {
        let text = CString::new(text.replace('\0', "")).unwrap_or_default();
        // SAFETY: NSString's documented factory, with a NUL-terminated
        // UTF-8 buffer that outlives the call.
        unsafe { msg_send![class(c"NSString"), stringWithUTF8String: text.as_ptr()] }
    }

    /// An NSControl's `stringValue` as Rust text.
    ///
    /// # Safety
    /// `control` answers `stringValue`.
    unsafe fn string_value(control: &AnyObject) -> String {
        // SAFETY: the caller's promise, then NSString's `UTF8String`.
        unsafe {
            let s: Option<Retained<AnyObject>> = msg_send![control, stringValue];
            let Some(s) = s else {
                return String::new();
            };
            let p: *const c_char = msg_send![&*s, UTF8String];
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    fn color([r, g, b]: [f64; 3]) -> Retained<AnyObject> {
        // SAFETY: NSColor's documented factory.
        unsafe {
            msg_send![
                class(c"NSColor"),
                colorWithSRGBRed: r,
                green: g,
                blue: b,
                alpha: 1.0_f64
            ]
        }
    }

    /// Liquid Glass exists here (macOS 26 and later).
    fn glass() -> bool {
        AnyClass::get(c"NSGlassEffectView").is_some()
    }

    fn rect(w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h))
    }

    struct Control {
        kind: Kind,
        view: Retained<NSView>,
        target: Retained<Target>,
        context: ComponentContext,
        props: Props,
        /// The items the control was last built from (radio, pop-up, tabs).
        built: Vec<String>,
        /// Tabs: the segmented control heading the box, and the box's line.
        tabs: Option<(Retained<AnyObject>, Retained<AnyObject>)>,
    }

    impl Control {
        /// Rebuilds the radio buttons, pop-up items or tabs when the item
        /// list changed.
        ///
        /// # Safety
        /// The view is the control `kind` made.
        unsafe fn rebuild_items(&mut self) {
            // Tabs read their bodies too, which may arrive after the items.
            let wanted: Vec<String> = self
                .props
                .items
                .iter()
                .chain(&self.props.bodies)
                .cloned()
                .collect();
            if self.built == wanted {
                return;
            }
            self.built = wanted;
            let view: &AnyObject = &self.view;
            // SAFETY: the caller's promise; each call is that control's
            // documented AppKit API.
            unsafe {
                match self.kind {
                    Kind::Radio => {
                        let old: Retained<AnyObject> = msg_send![view, arrangedSubviews];
                        let count: usize = msg_send![&*old, count];
                        for at in 0..count {
                            let radio: Retained<AnyObject> = msg_send![&*old, objectAtIndex: at];
                            // A stack view drops an arranged view that
                            // leaves it.
                            let _: () = msg_send![&*radio, removeFromSuperview];
                        }
                        for (at, item) in self.props.items.iter().enumerate() {
                            let radio: Retained<AnyObject> = msg_send![
                                class(c"NSButton"),
                                radioButtonWithTitle: &*ns_string(item),
                                target: &*self.target,
                                action: sel!(act:)
                            ];
                            let _: () = msg_send![&*radio, setTag: at as isize];
                            let _: () = msg_send![view, addArrangedSubview: &*radio];
                        }
                    }
                    Kind::GlassBar => {
                        let stack: Retained<AnyObject> = if glass() {
                            msg_send![view, contentView]
                        } else {
                            let subviews: Retained<AnyObject> = msg_send![view, subviews];
                            msg_send![&*subviews, firstObject]
                        };
                        let old: Retained<AnyObject> = msg_send![&*stack, arrangedSubviews];
                        let count: usize = msg_send![&*old, count];
                        for at in 0..count {
                            let b: Retained<AnyObject> = msg_send![&*old, objectAtIndex: at];
                            let _: () = msg_send![&*b, removeFromSuperview];
                        }
                        for (at, symbol) in self.props.items.iter().enumerate() {
                            let name = self.props.bodies.get(at).cloned().unwrap_or_default();
                            let image: Option<Retained<AnyObject>> = msg_send![
                                class(c"NSImage"),
                                imageWithSystemSymbolName: &*ns_string(symbol),
                                accessibilityDescription: &*ns_string(&name)
                            ];
                            let Some(image) = image else { continue };
                            let b: Retained<AnyObject> = msg_send![
                                class(c"NSButton"),
                                buttonWithImage: &*image,
                                target: &*self.target,
                                action: sel!(act:)
                            ];
                            let _: () = msg_send![&*b, setBordered: false];
                            let _: () = msg_send![&*b, setTag: at as isize];
                            let _: () = msg_send![&*b, setToolTip: &*ns_string(&name)];
                            let _: () = msg_send![&*stack, addArrangedSubview: &*b];
                        }
                    }
                    Kind::Popup => {
                        let _: () = msg_send![view, removeAllItems];
                        for item in &self.props.items {
                            let _: () = msg_send![view, addItemWithTitle: &*ns_string(item)];
                        }
                    }
                    Kind::Tabs => {
                        if let Some((segmented, _)) = &self.tabs {
                            let n = self.props.items.len() as isize;
                            let _: () = msg_send![&**segmented, setSegmentCount: n];
                            for (at, item) in self.props.items.iter().enumerate() {
                                let _: () = msg_send![
                                    &**segmented,
                                    setLabel: &*ns_string(item),
                                    forSegment: at as isize
                                ];
                            }
                            let _: () = msg_send![&**segmented, sizeToFit];
                        }
                    }
                    _ => {}
                }
            }
        }

        /// Applies every prop, then reports the natural size.
        fn apply(&mut self) {
            // SAFETY: the view is the control `kind` made.
            unsafe { self.rebuild_items() };
            let view: &AnyObject = &self.view;
            let props = &self.props;
            let tint = props.tint.map(color);
            // SAFETY: each call is the documented AppKit API (macOS 11 and
            // later) of the control `kind` made.
            unsafe {
                match self.kind {
                    Kind::Slider => {
                        let _: () = msg_send![view, setMinValue: props.min];
                        let _: () = msg_send![view, setMaxValue: props.max];
                        let _: () = msg_send![view, setDoubleValue: props.value];
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setTrackFillColor: &**tint];
                        }
                    }
                    Kind::Stepper => {
                        let _: () = msg_send![view, setMinValue: props.min];
                        let _: () = msg_send![view, setMaxValue: props.max];
                        let _: () = msg_send![view, setIncrement: props.step];
                        let _: () = msg_send![view, setDoubleValue: props.value];
                    }
                    Kind::Checkbox => {
                        let _: () = msg_send![view, setTitle: &*ns_string(&props.title)];
                        let _: () = msg_send![view, setState: isize::from(props.on)];
                    }
                    Kind::Radio => {
                        let radios: Retained<AnyObject> = msg_send![view, arrangedSubviews];
                        let count: usize = msg_send![&*radios, count];
                        for at in 0..count {
                            let radio: Retained<AnyObject> = msg_send![&*radios, objectAtIndex: at];
                            let on = at as isize == props.selected;
                            let _: () = msg_send![&*radio, setState: isize::from(on)];
                        }
                    }
                    Kind::Popup => {
                        let _: () = msg_send![view, selectItemAtIndex: props.selected.max(0)];
                    }
                    Kind::Progress => {
                        let _: () = msg_send![view, setDoubleValue: props.value];
                    }
                    Kind::Spinner => {}
                    Kind::GlassButton => {
                        let _: () = msg_send![view, setTitle: &*ns_string(&props.title)];
                        if glass() {
                            // NSBezelStyleGlass (macOS 26).
                            let _: () = msg_send![view, setBezelStyle: 16_usize];
                        }
                        let none: Option<&AnyObject> = None;
                        let bezel = if props.style == "prominent" {
                            tint.as_deref()
                        } else {
                            none
                        };
                        let _: () = msg_send![view, setBezelColor: bezel];
                        if !props.symbol.is_empty() {
                            let image: Option<Retained<AnyObject>> = msg_send![
                                class(c"NSImage"),
                                imageWithSystemSymbolName: &*ns_string(&props.symbol),
                                accessibilityDescription: none
                            ];
                            let _: () = msg_send![view, setImage: image.as_deref()];
                            // NSImageOnly for an icon button, else
                            // NSImageLeading.
                            let position: usize = if props.title.is_empty() { 1 } else { 7 };
                            let _: () = msg_send![view, setImagePosition: position];
                        }
                        if props.title.is_empty() {
                            // Before glass, a toolbar icon has no bezel.
                            let _: () = msg_send![view, setBordered: glass()];
                            if !props.text.is_empty() {
                                let name = ns_string(&props.text);
                                let _: () = msg_send![view, setToolTip: &*name];
                                let _: () = msg_send![view, setAccessibilityLabel: &*name];
                            }
                        }
                    }
                    Kind::Material => {
                        // NSVisualEffectMaterial: Titlebar, HeaderView,
                        // Sidebar, WindowBackground.
                        let material: isize = match props.style.as_str() {
                            "header" => 10,
                            "sidebar" => 7,
                            "window" => 12,
                            _ => 3,
                        };
                        let _: () = msg_send![view, setMaterial: material];
                        // Behind the window; following its active state.
                        let _: () = msg_send![view, setBlendingMode: 0_isize];
                        let _: () = msg_send![view, setState: 0_isize];
                    }
                    Kind::GlassBar => {
                        if glass() {
                            // NSGlassEffectViewStyleRegular, Clear.
                            let style: isize = isize::from(props.style == "clear");
                            let _: () = msg_send![view, setStyle: style];
                        }
                    }
                    Kind::Date => {
                        // NSDatePickerStyleTextFieldAndStepper, ClockAndCalendar.
                        let style: usize = if props.style == "calendar" { 1 } else { 0 };
                        let _: () = msg_send![view, setDatePickerStyle: style];
                        if props.has_date {
                            let date: Retained<AnyObject> = msg_send![
                                class(c"NSDate"),
                                dateWithTimeIntervalSince1970: props.value
                            ];
                            let _: () = msg_send![view, setDateValue: &*date];
                        }
                    }
                    Kind::Color => {
                        if let Some(c) = props.color {
                            let _: () = msg_send![view, setColor: &*color(c)];
                        }
                    }
                    Kind::Text | Kind::Search => {
                        let _: () = msg_send![
                            view,
                            setPlaceholderString: &*ns_string(&props.placeholder)
                        ];
                        if string_value(view) != props.text {
                            let _: () = msg_send![view, setStringValue: &*ns_string(&props.text)];
                        }
                    }
                    Kind::Tabs => {
                        if let Some((segmented, line)) = &self.tabs {
                            let _: () = msg_send![&**segmented, setSelectedSegment: props.selected];
                            if let Some(tint) = &tint {
                                let _: () =
                                    msg_send![&**segmented, setSelectedSegmentBezelColor: &**tint];
                            }
                            let body = usize::try_from(props.selected)
                                .ok()
                                .and_then(|at| props.bodies.get(at))
                                .cloned()
                                .unwrap_or_default();
                            let _: () = msg_send![&**line, setStringValue: &*ns_string(&body)];
                            // Centered over the box's top edge, as AppKit's
                            // tab view draws its tabs.
                            let bounds: NSRect = msg_send![view, bounds];
                            let frame: NSRect = msg_send![&**segmented, frame];
                            let x = ((bounds.size.width - frame.size.width) / 2.0).max(0.0);
                            let y = bounds.size.height - frame.size.height;
                            let _: () = msg_send![
                                &**segmented,
                                setFrameOrigin: NSPoint::new(x, y)
                            ];
                        }
                    }
                }
                let size: NSSize = msg_send![view, fittingSize];
                let (w, h) = match self.kind {
                    // The width comes from the layout; AppKit's bar and
                    // field heights.
                    Kind::Progress => (0.0, size.height.max(20.0)),
                    Kind::Spinner => (32.0, 32.0),
                    // Each symbol button gets a 36 pt square on the pill.
                    Kind::GlassBar => (self.props.items.len() as f64 * 36.0 + 16.0, 44.0),
                    Kind::Color => (size.width.max(44.0), size.height.max(24.0)),
                    _ => (size.width.max(0.0), size.height.max(0.0)),
                };
                self.context.set_size(w as f32, h as f32);
            }
        }
    }

    impl NativeComponent for Control {
        fn view(&self) -> Retained<NSView> {
            self.view.clone()
        }

        fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
            if self.props.take(name, value) {
                self.apply();
            }
        }
    }

    fn make(kind: Kind, context: ComponentContext) -> Box<dyn NativeComponent> {
        let mtm = MainThreadMarker::new().expect("components are made on the main thread");
        let target = Target::new(mtm, kind, context.clone());
        // SAFETY: each control's documented initializer, then NSControl's
        // target and action (or the delegate the kind reports through).
        let view: Retained<NSView> = unsafe {
            let name: &CStr = match kind {
                Kind::Slider => c"NSSlider",
                Kind::Checkbox => c"NSButton",
                Kind::Radio => c"NSStackView",
                Kind::Popup => c"NSPopUpButton",
                Kind::Stepper => c"NSStepper",
                Kind::Progress | Kind::Spinner => c"NSProgressIndicator",
                Kind::Date => c"NSDatePicker",
                Kind::Color => c"NSColorWell",
                Kind::Text => c"NSTextField",
                Kind::Search => c"NSSearchField",
                Kind::Tabs => c"NSView",
                Kind::GlassButton => c"NSButton",
                // An older macOS gets the vibrant material in its place.
                Kind::GlassBar if glass() => c"NSGlassEffectView",
                Kind::GlassBar | Kind::Material => c"NSVisualEffectView",
            };
            let view: Retained<NSView> =
                msg_send![msg_send![class(name), alloc], initWithFrame: rect(160.0, 24.0)];
            let v: &AnyObject = &view;
            match kind {
                Kind::Radio => {
                    // NSUserInterfaceLayoutOrientationVertical, leading.
                    let _: () = msg_send![v, setOrientation: 1_isize];
                    let _: () = msg_send![v, setAlignment: 5_isize];
                    let _: () = msg_send![v, setSpacing: 6.0_f64];
                }
                // A backdrop: no target, no action.
                Kind::Tabs | Kind::Material => {}
                Kind::GlassBar => {
                    let stack: Retained<NSView> = msg_send![
                        msg_send![class(c"NSStackView"), alloc],
                        initWithFrame: rect(160.0, 44.0)
                    ];
                    // Horizontal, buttons spread evenly over the pill.
                    let _: () = msg_send![&*stack, setOrientation: 0_isize];
                    let _: () = msg_send![&*stack, setDistribution: 1_isize];
                    let _: () = msg_send![&*stack, setSpacing: 0.0_f64];
                    if glass() {
                        let _: () = msg_send![v, setCornerRadius: 22.0_f64];
                        let _: () = msg_send![v, setContentView: &*stack];
                    } else {
                        let _: () = msg_send![v, setWantsLayer: true];
                        let layer: Retained<AnyObject> = msg_send![v, layer];
                        let _: () = msg_send![&*layer, setCornerRadius: 22.0_f64];
                        let _: () = msg_send![&*stack, setAutoresizingMask: 2_usize | 16_usize];
                        let _: () = msg_send![v, addSubview: &*stack];
                    }
                }
                Kind::Text | Kind::Search => {
                    let _: () = msg_send![v, setDelegate: &*target];
                    if kind == Kind::Text {
                        let _: () = msg_send![v, setBezeled: true];
                        // NSTextFieldRoundedBezel.
                        let _: () = msg_send![v, setBezelStyle: 1_usize];
                    }
                }
                Kind::Progress | Kind::Spinner => {
                    let spinning = kind == Kind::Spinner;
                    // NSProgressIndicatorStyleBar, Spinning.
                    let _: () = msg_send![v, setStyle: usize::from(spinning)];
                    let _: () = msg_send![v, setIndeterminate: spinning];
                    let _: () = msg_send![v, setMinValue: 0.0_f64];
                    let _: () = msg_send![v, setMaxValue: 1.0_f64];
                    if spinning {
                        let none: Option<&AnyObject> = None;
                        let _: () = msg_send![v, startAnimation: none];
                    }
                }
                _ => {
                    let _: () = msg_send![v, setTarget: &*target];
                    let _: () = msg_send![v, setAction: sel!(act:)];
                }
            }
            match kind {
                // NSButtonTypeSwitch.
                Kind::Checkbox => {
                    let _: () = msg_send![v, setButtonType: 3_usize];
                }
                Kind::Slider => {
                    let _: () = msg_send![v, setContinuous: true];
                }
                Kind::Stepper => {
                    let _: () = msg_send![v, setValueWraps: false];
                }
                Kind::Date => {
                    // NSDatePickerElementFlagYearMonthDay.
                    let _: () = msg_send![v, setDatePickerElements: 0x00e0_usize];
                    let _: () = msg_send![v, setBezeled: true];
                }
                // NSColorWellStyleExpanded (macOS 13): the swatch with the
                // panel's button beside it.
                Kind::Color if v.class().responds_to(sel!(setColorWellStyle:)) => {
                    let _: () = msg_send![v, setColorWellStyle: 2_isize];
                }
                _ => {}
            }
            view
        };
        // SAFETY: as above: NSBox, NSSegmentedControl and NSTextField's
        // documented API.
        let tabs = (kind == Kind::Tabs).then(|| unsafe { tab_parts(&view, &target) });
        let mut control = Control {
            kind,
            view,
            target,
            context,
            props: Props::new(),
            built: Vec::new(),
            tabs,
        };
        control.apply();
        Box::new(control)
    }

    /// A tab view as AppKit draws one: a box with a segmented control over
    /// its top edge. `NSTabView` itself draws its tab strip blank inside the
    /// nsh host (both appearances), so its parts are composed here.
    ///
    /// # Safety
    /// `container` is a plain NSView.
    unsafe fn tab_parts(
        container: &NSView,
        target: &Target,
    ) -> (Retained<AnyObject>, Retained<AnyObject>) {
        let (w, h) = (400.0, 150.0);
        // SAFETY: the caller's promise; NSView, NSBox, NSSegmentedControl
        // and NSTextField's documented API.
        unsafe {
            let _: () = msg_send![container, setFrame: rect(w, h)];
            let boxed: Retained<NSView> =
                msg_send![msg_send![class(c"NSBox"), alloc], initWithFrame: rect(w, h - 11.0)];
            // NSNoTitle; width and height follow the container.
            let _: () = msg_send![&*boxed, setTitlePosition: 0_usize];
            let _: () = msg_send![&*boxed, setAutoresizingMask: 2_usize | 16_usize];
            let _: () = msg_send![container, addSubview: &*boxed];
            let line: Retained<AnyObject> = msg_send![
                class(c"NSTextField"),
                wrappingLabelWithString: &*ns_string("")
            ];
            let content: Retained<NSView> = msg_send![&*boxed, contentView];
            let inner: NSRect = msg_send![&*content, bounds];
            let _: () = msg_send![
                &*line,
                setFrame: NSRect::new(
                    NSPoint::new(12.0, inner.size.height - 52.0),
                    NSSize::new(inner.size.width - 24.0, 36.0)
                )
            ];
            // Width sizable, pinned to the top.
            let _: () = msg_send![&*line, setAutoresizingMask: 2_usize | 8_usize];
            let _: () = msg_send![&*content, addSubview: &*line];
            let segmented: Retained<AnyObject> = msg_send![
                msg_send![class(c"NSSegmentedControl"), alloc],
                initWithFrame: rect(240.0, 24.0)
            ];
            // NSSegmentSwitchTrackingSelectOne.
            let _: () = msg_send![&*segmented, setTrackingMode: 0_usize];
            let _: () = msg_send![&*segmented, setTarget: target];
            let _: () = msg_send![&*segmented, setAction: sel!(act:)];
            // Centered, at the top, as the container resizes.
            let _: () = msg_send![&*segmented, setAutoresizingMask: 1_usize | 4_usize | 8_usize];
            let _: () = msg_send![container, addSubview: &*segmented];
            (segmented, line)
        }
    }

    /// Registers the controls.
    pub fn register(components: &Components) {
        for (name, kind) in KINDS {
            components.register(
                name,
                Rc::new(move |context: ComponentContext| make(kind, context)),
            );
        }
    }
}

/// UIKit's controls. Checkbox, radio group and tabs have no UIKit control;
/// the Rust side never asks for them on iOS.
#[cfg(target_os = "ios")]
mod ios {
    use std::{
        ffi::{CStr, CString, c_char},
        rc::Rc,
    };

    use nsh_core::JsonValue;
    use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
    use objc2::{
        DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, NSObject},
        sel,
    };
    use objc2_core_foundation::CGSize;
    use objc2_ui_kit::UIView;

    use super::{KINDS, Kind, Props};

    /// `UIControlEventValueChanged`, `UIControlEventEditingChanged`.
    const VALUE_CHANGED: usize = 1 << 12;
    const EDITING_CHANGED: usize = 1 << 17;
    /// `UIControlEventPrimaryActionTriggered`.
    const PRIMARY_ACTION: usize = 1 << 13;

    struct Ivars {
        kind: Kind,
        context: ComponentContext,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the target only
        // reads its control and forwards to the component's context.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "NshStarterKitTarget"]
        #[ivars = Ivars]
        struct Target;

        impl Target {
            #[unsafe(method(act:))]
            fn act(&self, sender: &AnyObject) {
                let ivars = self.ivars();
                // SAFETY: each read is the documented getter of the control
                // `kind` made, `sender` being that control.
                let data = unsafe {
                    match ivars.kind {
                        Kind::Slider => {
                            let v: f32 = msg_send![sender, value];
                            serde_json::json!({ "value": v })
                        }
                        Kind::Stepper => {
                            let v: f64 = msg_send![sender, value];
                            serde_json::json!({ "value": v })
                        }
                        Kind::Date => {
                            let date: Retained<AnyObject> = msg_send![sender, date];
                            let v: f64 = msg_send![&*date, timeIntervalSince1970];
                            serde_json::json!({ "value": v })
                        }
                        Kind::Color => {
                            let c: Option<Retained<AnyObject>> = msg_send![sender, selectedColor];
                            let Some(c) = c else { return };
                            let (mut r, mut g, mut b, mut a) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
                            let _: bool = msg_send![
                                &*c,
                                getRed: &mut r,
                                green: &mut g,
                                blue: &mut b,
                                alpha: &mut a
                            ];
                            serde_json::json!({ "rgb": [r, g, b] })
                        }
                        Kind::GlassButton | Kind::GlassBar => {
                            let tag: isize = msg_send![sender, tag];
                            serde_json::json!({ "index": tag })
                        }
                        Kind::Text | Kind::Search => {
                            let s: Option<Retained<AnyObject>> = msg_send![sender, text];
                            serde_json::json!({ "text": rust_string(s.as_deref()) })
                        }
                        _ => return,
                    }
                };
                ivars.context.emit("change", data);
            }
        }
    );

    impl Target {
        fn new(mtm: MainThreadMarker, kind: Kind, context: ComponentContext) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars { kind, context });
            // SAFETY: NSObject's designated initializer.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn class(name: &CStr) -> &'static AnyClass {
        AnyClass::get(name).expect("a UIKit class")
    }

    fn ns_string(text: &str) -> Retained<AnyObject> {
        let text = CString::new(text.replace('\0', "")).unwrap_or_default();
        // SAFETY: NSString's documented factory, with a NUL-terminated
        // UTF-8 buffer that outlives the call.
        unsafe { msg_send![class(c"NSString"), stringWithUTF8String: text.as_ptr()] }
    }

    fn rust_string(s: Option<&AnyObject>) -> String {
        let Some(s) = s else {
            return String::new();
        };
        // SAFETY: NSString's `UTF8String`, valid while `s` lives.
        unsafe {
            let p: *const c_char = msg_send![s, UTF8String];
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    fn color([r, g, b]: [f64; 3]) -> Retained<AnyObject> {
        // SAFETY: UIColor's documented factory.
        unsafe { msg_send![class(c"UIColor"), colorWithRed: r, green: g, blue: b, alpha: 1.0_f64] }
    }

    struct Control {
        kind: Kind,
        view: Retained<UIView>,
        target: Retained<Target>,
        context: ComponentContext,
        props: Props,
        built: Vec<String>,
        built_selected: isize,
    }

    impl Control {
        /// The pop-up's menu: one action per item, the selected one on.
        ///
        /// # Safety
        /// The view is a UIButton.
        unsafe fn rebuild_menu(&mut self) {
            if self.built == self.props.items && self.built_selected == self.props.selected {
                return;
            }
            self.built = self.props.items.clone();
            self.built_selected = self.props.selected;
            // SAFETY: the caller's promise; UIAction, UIMenu and NSArray's
            // documented factories (iOS 14 and later).
            unsafe {
                let mut actions: Vec<Retained<AnyObject>> = Vec::new();
                for (at, item) in self.props.items.iter().enumerate() {
                    let context = self.context.clone();
                    let handler = block2::RcBlock::new(move |_: *mut AnyObject| {
                        context.emit("change", serde_json::json!({ "index": at }));
                    });
                    let none: Option<&AnyObject> = None;
                    let action: Retained<AnyObject> = msg_send![
                        class(c"UIAction"),
                        actionWithTitle: &*ns_string(item),
                        image: none,
                        identifier: none,
                        handler: &*handler
                    ];
                    // UIMenuElementStateOn.
                    if at as isize == self.props.selected.max(0) {
                        let _: () = msg_send![&*action, setState: 1_isize];
                    }
                    actions.push(action);
                }
                let ptrs: Vec<*const AnyObject> = actions.iter().map(Retained::as_ptr).collect();
                let list: Retained<AnyObject> = msg_send![
                    class(c"NSArray"),
                    arrayWithObjects: ptrs.as_ptr(),
                    count: ptrs.len()
                ];
                let menu: Retained<AnyObject> =
                    msg_send![class(c"UIMenu"), menuWithChildren: &*list];
                let view: &AnyObject = &self.view;
                let _: () = msg_send![view, setMenu: &*menu];
            }
        }

        /// The glass bar's buttons, one per SF Symbol in `items`.
        ///
        /// # Safety
        /// The view is the bar's UIVisualEffectView, holding its stack.
        unsafe fn rebuild_bar(&mut self) {
            if self.built == self.props.items {
                return;
            }
            self.built = self.props.items.clone();
            // SAFETY: the caller's promise; UIStackView, UIButton and
            // UIImage's documented API.
            unsafe {
                let content: Retained<AnyObject> = msg_send![&*self.view, contentView];
                let subviews: Retained<AnyObject> = msg_send![&*content, subviews];
                let stack: Retained<AnyObject> = msg_send![&*subviews, firstObject];
                let old: Retained<AnyObject> = msg_send![&*stack, arrangedSubviews];
                let count: usize = msg_send![&*old, count];
                for at in 0..count {
                    let b: Retained<AnyObject> = msg_send![&*old, objectAtIndex: at];
                    let _: () = msg_send![&*b, removeFromSuperview];
                }
                for (at, symbol) in self.props.items.iter().enumerate() {
                    let b: Retained<AnyObject> =
                        msg_send![class(c"UIButton"), buttonWithType: 1_isize];
                    let image: Option<Retained<AnyObject>> =
                        msg_send![class(c"UIImage"), systemImageNamed: &*ns_string(symbol)];
                    // UIControlStateNormal.
                    let _: () = msg_send![&*b, setImage: image.as_deref(), forState: 0_usize];
                    if let Some(name) = self.props.bodies.get(at) {
                        let _: () = msg_send![&*b, setAccessibilityLabel: &*ns_string(name)];
                    }
                    let _: () = msg_send![&*b, setTag: at as isize];
                    let _: () = msg_send![
                        &*b,
                        addTarget: &*self.target,
                        action: sel!(act:),
                        forControlEvents: PRIMARY_ACTION
                    ];
                    let _: () = msg_send![&*stack, addArrangedSubview: &*b];
                }
            }
        }

        fn apply(&mut self) {
            if self.kind == Kind::Popup {
                // SAFETY: a pop-up is a UIButton.
                unsafe { self.rebuild_menu() };
            }
            if self.kind == Kind::GlassBar {
                // SAFETY: made as the bar in `make`.
                unsafe { self.rebuild_bar() };
            }
            let view: &AnyObject = &self.view;
            let props = &self.props;
            let tint = props.tint.map(color);
            // SAFETY: each call is the documented UIKit API (iOS 15 and
            // later) of the control `kind` made.
            unsafe {
                match self.kind {
                    Kind::Slider => {
                        let _: () = msg_send![view, setMinimumValue: props.min as f32];
                        let _: () = msg_send![view, setMaximumValue: props.max as f32];
                        let _: () = msg_send![view, setValue: props.value as f32, animated: false];
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setMinimumTrackTintColor: &**tint];
                        }
                    }
                    Kind::Stepper => {
                        let _: () = msg_send![view, setMinimumValue: props.min];
                        let _: () = msg_send![view, setMaximumValue: props.max];
                        let _: () = msg_send![view, setStepValue: props.step];
                        let _: () = msg_send![view, setValue: props.value];
                    }
                    Kind::Progress => {
                        let _: () =
                            msg_send![view, setProgress: props.value as f32, animated: false];
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setProgressTintColor: &**tint];
                        }
                    }
                    Kind::Date => {
                        // UIDatePickerStyleCompact, Inline.
                        let style: isize = if props.style == "calendar" { 3 } else { 2 };
                        let _: () = msg_send![view, setPreferredDatePickerStyle: style];
                        if props.has_date {
                            let date: Retained<AnyObject> = msg_send![
                                class(c"NSDate"),
                                dateWithTimeIntervalSince1970: props.value
                            ];
                            let _: () = msg_send![view, setDate: &*date];
                        }
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setTintColor: &**tint];
                        }
                    }
                    Kind::Color => {
                        if let Some(c) = props.color {
                            let _: () = msg_send![view, setSelectedColor: &*color(c)];
                        }
                    }
                    Kind::Text | Kind::Search => {
                        let _: () =
                            msg_send![view, setPlaceholder: &*ns_string(&props.placeholder)];
                        let now: Option<Retained<AnyObject>> = msg_send![view, text];
                        if rust_string(now.as_deref()) != props.text {
                            let _: () = msg_send![view, setText: &*ns_string(&props.text)];
                        }
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setTintColor: &**tint];
                        }
                    }
                    Kind::Popup => {
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setTintColor: &**tint];
                        }
                    }
                    Kind::GlassButton => {
                        let configs = class(c"UIButtonConfiguration");
                        let prominent = props.style == "prominent";
                        // The glass configurations are iOS 26's.
                        let config: Retained<AnyObject> = if prominent
                            && configs.responds_to(sel!(prominentGlassButtonConfiguration))
                        {
                            msg_send![configs, prominentGlassButtonConfiguration]
                        } else if configs.responds_to(sel!(glassButtonConfiguration)) {
                            msg_send![configs, glassButtonConfiguration]
                        } else if prominent {
                            msg_send![configs, filledButtonConfiguration]
                        } else {
                            msg_send![configs, grayButtonConfiguration]
                        };
                        let _: () = msg_send![&*config, setTitle: &*ns_string(&props.title)];
                        if !props.symbol.is_empty() {
                            let image: Option<Retained<AnyObject>> = msg_send![
                                class(c"UIImage"),
                                systemImageNamed: &*ns_string(&props.symbol)
                            ];
                            let _: () = msg_send![&*config, setImage: image.as_deref()];
                            let _: () = msg_send![&*config, setImagePadding: 6.0_f64];
                        }
                        let _: () = msg_send![view, setConfiguration: &*config];
                        if let Some(tint) = &tint {
                            let _: () = msg_send![view, setTintColor: &**tint];
                        }
                    }
                    Kind::GlassBar => {
                        if let Some(glass) = AnyClass::get(c"UIGlassEffect") {
                            // UIGlassEffectStyleRegular, Clear.
                            let style = isize::from(props.style == "clear");
                            let effect: Retained<AnyObject> =
                                msg_send![glass, effectWithStyle: style];
                            let _: () = msg_send![view, setEffect: &*effect];
                        }
                        let content: Retained<AnyObject> = msg_send![view, contentView];
                        let bounds: objc2_core_foundation::CGRect = msg_send![&*content, bounds];
                        let subviews: Retained<AnyObject> = msg_send![&*content, subviews];
                        let stack: Retained<AnyObject> = msg_send![&*subviews, firstObject];
                        let _: () = msg_send![&*stack, setFrame: bounds];
                    }
                    _ => {}
                }
                let size: CGSize = msg_send![view, intrinsicContentSize];
                let (w, h) = match self.kind {
                    Kind::Progress => (0.0, 4.0),
                    Kind::Text | Kind::Search => (0.0, size.height.max(36.0)),
                    Kind::Color => (36.0, 36.0),
                    Kind::GlassBar => (props.items.len() as f64 * 44.0 + 16.0, 48.0),
                    _ => (size.width.max(0.0), size.height.max(0.0)),
                };
                self.context.set_size(w as f32, h as f32);
            }
        }
    }

    impl NativeComponent for Control {
        fn view(&self) -> &UIView {
            &self.view
        }

        fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
            if self.props.take(name, value) {
                self.apply();
            }
        }
    }

    fn make(kind: Kind, context: ComponentContext) -> Box<dyn NativeComponent> {
        let mtm = MainThreadMarker::new().expect("components are made on the main thread");
        let target = Target::new(mtm, kind, context.clone());
        // SAFETY: each control's documented initializer, then UIControl's
        // target-action registration.
        let view: Retained<UIView> = unsafe {
            let view: Retained<UIView> = match kind {
                Kind::Popup => {
                    // UIButtonTypeSystem.
                    let b: Retained<UIView> =
                        msg_send![class(c"UIButton"), buttonWithType: 1_isize];
                    let _: () = msg_send![&*b, setShowsMenuAsPrimaryAction: true];
                    let _: () = msg_send![&*b, setChangesSelectionAsPrimaryAction: true];
                    b
                }
                Kind::Progress => {
                    // UIProgressViewStyleDefault.
                    msg_send![msg_send![class(c"UIProgressView"), alloc], initWithProgressViewStyle: 0_isize]
                }
                Kind::GlassButton => msg_send![class(c"UIButton"), buttonWithType: 1_isize],
                Kind::GlassBar => {
                    // UIGlassEffectStyleRegular; iOS before 26 gets the
                    // system material.
                    let effect: Retained<AnyObject> = match AnyClass::get(c"UIGlassEffect") {
                        Some(glass) => msg_send![glass, effectWithStyle: 0_isize],
                        None => msg_send![class(c"UIBlurEffect"), effectWithStyle: 8_isize],
                    };
                    let bar: Retained<UIView> = msg_send![
                        msg_send![class(c"UIVisualEffectView"), alloc],
                        initWithEffect: &*effect
                    ];
                    let layer: Retained<AnyObject> = msg_send![&*bar, layer];
                    let _: () = msg_send![&*layer, setCornerRadius: 24.0_f64];
                    let _: () = msg_send![&*bar, setClipsToBounds: true];
                    let stack: Retained<AnyObject> =
                        msg_send![msg_send![class(c"UIStackView"), alloc], init];
                    // UIStackViewDistributionFillEqually; flexible width
                    // and height.
                    let _: () = msg_send![&*stack, setDistribution: 1_isize];
                    let _: () = msg_send![&*stack, setAutoresizingMask: 2_usize | 16_usize];
                    let content: Retained<AnyObject> = msg_send![&*bar, contentView];
                    let _: () = msg_send![&*content, addSubview: &*stack];
                    bar
                }
                Kind::Spinner => {
                    // UIActivityIndicatorViewStyleMedium.
                    let s: Retained<UIView> = msg_send![
                        msg_send![class(c"UIActivityIndicatorView"), alloc],
                        initWithActivityIndicatorStyle: 100_isize
                    ];
                    let _: () = msg_send![&*s, startAnimating];
                    s
                }
                _ => {
                    let name: &CStr = match kind {
                        Kind::Slider => c"UISlider",
                        Kind::Stepper => c"UIStepper",
                        Kind::Date => c"UIDatePicker",
                        Kind::Color => c"UIColorWell",
                        Kind::Text => c"UITextField",
                        Kind::Search => c"UISearchTextField",
                        // Checkbox, radio and tabs: never asked for here.
                        _ => c"UIView",
                    };
                    msg_send![msg_send![class(name), alloc], init]
                }
            };
            let v: &AnyObject = &view;
            if kind == Kind::Date {
                // UIDatePickerModeDate.
                let _: () = msg_send![v, setDatePickerMode: 1_isize];
            }
            if kind == Kind::Text {
                // UITextBorderStyleRoundedRect.
                let _: () = msg_send![v, setBorderStyle: 3_isize];
            }
            let events = match kind {
                Kind::GlassButton => PRIMARY_ACTION,
                Kind::Text | Kind::Search => EDITING_CHANGED,
                Kind::Slider | Kind::Stepper | Kind::Date | Kind::Color => VALUE_CHANGED,
                _ => 0,
            };
            if events != 0 {
                let _: () = msg_send![
                    v,
                    addTarget: &*target,
                    action: sel!(act:),
                    forControlEvents: events
                ];
            }
            view
        };
        let mut control = Control {
            kind,
            view,
            target,
            context,
            props: Props::new(),
            built: Vec::new(),
            built_selected: -2,
        };
        control.apply();
        Box::new(control)
    }

    /// Registers the controls UIKit has.
    pub fn register(components: &Components) {
        for (name, kind) in KINDS {
            if matches!(
                kind,
                Kind::Checkbox | Kind::Radio | Kind::Tabs | Kind::Material
            ) {
                continue;
            }
            components.register(
                name,
                Rc::new(move |context: ComponentContext| make(kind, context)),
            );
        }
    }
}
