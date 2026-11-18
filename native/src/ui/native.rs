//! The platform's own controls: a system button, switch and segmented
//! control drawn by UIKit on iOS and AppKit on macOS, registered from this
//! crate as native components (`#[native_components]` in `lib.rs`).
//!
//! Elsewhere (Android, Linux, Windows, the web, the headless tests) each
//! function falls back to the app's own styled control, so a screen can use
//! them everywhere. NSH 0.5.0 registers an app crate's native components on
//! iOS, macOS, Linux and Windows only; Android's come from the Java host.
//!
//! The native views keep the system font and metrics on purpose: they are
//! the system's controls. The app's accent tints them, so they sit with the
//! rest of the app.

use nsh_base::theme;
use nsh_core::{JsonValue, color::Rgba, element::NativeEvent, prelude::*};

use crate::ui::button::{ButtonSize, ButtonVariant, button_with, choice_button};

/// The system button's name among the native components.
pub const BUTTON: &str = "starter.native-button";
/// The system switch's name.
pub const SWITCH: &str = "starter.native-switch";
/// The system segmented control's name.
pub const SEGMENTED: &str = "starter.native-segmented";

/// A system button's look. iOS draws each as its `UIButton.Configuration`;
/// macOS maps them to its bezels (glass is the default push button there).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NativeButtonStyle {
    /// The accent fill: the one main action.
    Filled,
    /// An accent-tinted fill: a secondary action.
    Tinted,
    /// A neutral fill.
    Gray,
    /// Text only.
    Plain,
    /// iOS 26's Liquid Glass (gray on older systems and on macOS).
    Glass,
}

impl NativeButtonStyle {
    /// The style's key, as the native side reads the `style` prop.
    pub fn key(self) -> &'static str {
        match self {
            NativeButtonStyle::Filled => "filled",
            NativeButtonStyle::Tinted => "tinted",
            NativeButtonStyle::Gray => "gray",
            NativeButtonStyle::Plain => "plain",
            NativeButtonStyle::Glass => "glass",
        }
    }

    /// The app's own button variant the fallback draws.
    fn variant(self) -> ButtonVariant {
        match self {
            NativeButtonStyle::Filled => ButtonVariant::Primary,
            NativeButtonStyle::Tinted | NativeButtonStyle::Gray => ButtonVariant::Secondary,
            NativeButtonStyle::Plain => ButtonVariant::Ghost,
            NativeButtonStyle::Glass => ButtonVariant::Outline,
        }
    }
}

/// Whether this platform draws the controls natively (the crate registers
/// them on iOS and macOS).
pub fn is_native() -> bool {
    use nsh_core::platform::Os;
    // The build and the running platform both: the headless tests run on a
    // macOS host as an iOS phone, with no UIKit to draw these.
    (cfg!(target_os = "ios") && os() == Os::Ios) || (cfg!(target_os = "macos") && os() == Os::MacOs)
}

/// The theme's accent for the current appearance, as `[r, g, b]` from 0
/// to 1: the native side tints its control with it.
fn accent() -> JsonValue {
    let primary = theme().primary;
    let c = Rgba::from(if dark() { primary.dark } else { primary.light });
    JsonValue::from(vec![f64::from(c.r), f64::from(c.g), f64::from(c.b)])
}

/// A system button: `title` with an optional SF Symbol (`symbol`, iOS and
/// macOS 11+), in `style`; `on_press` runs on a press.
pub fn native_button(
    title: &'static str,
    style: NativeButtonStyle,
    symbol: Option<&'static str>,
    on_press: impl Fn() + 'static,
) -> AnyElement {
    if !is_native() {
        return button_with(
            title,
            style.variant(),
            ButtonSize::Medium,
            false,
            None,
            on_press,
        )
        .into_element();
    }
    nsh_core::element::native(BUTTON)
        .prop("title", JsonValue::from(title))
        .prop("style", JsonValue::from(style.key()))
        .prop("symbol", symbol.map_or(JsonValue::Null, JsonValue::from))
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if &*e.name == "press" {
                on_press();
            }
        })
        .self_start()
        .into_element()
}

/// A labelled system switch writing `on`.
pub fn native_switch(label: &'static str, on: Signal<bool>) -> AnyElement {
    if !is_native() {
        return crate::ui::controls::switch_row(label, on).into_element();
    }
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(crate::ui::text::body(label))
        .child(
            nsh_core::element::native(SWITCH)
                .prop("on", move || JsonValue::from(on.get()))
                .prop("tint", accent)
                .on_native_event(move |e: &NativeEvent| {
                    if &*e.name == "change" {
                        on.set(e.data["on"].as_bool().unwrap_or_default());
                    }
                }),
        )
        .into_element()
}

/// A system segmented control over `items`, writing the chosen index to
/// `selected`.
pub fn native_segmented(items: &'static [&'static str], selected: Signal<usize>) -> AnyElement {
    if !is_native() {
        return view()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .children(items.iter().enumerate().map(|(at, item)| {
                choice_button(
                    *item,
                    item,
                    move || selected.get() == at,
                    false,
                    move || selected.set(at),
                )
            }))
            .into_element();
    }
    nsh_core::element::native(SEGMENTED)
        .prop("items", JsonValue::from(items.to_vec()))
        .prop("selected", move || JsonValue::from(selected.get()))
        .prop("tint", accent)
        .on_native_event(move |e: &NativeEvent| {
            if &*e.name == "change"
                && let Some(at) = e.data["index"].as_u64()
            {
                selected.set(at as usize);
            }
        })
        .self_start()
        .into_element()
}

/// The `tint` prop as a color's three components.
#[cfg(any(target_os = "ios", target_os = "macos"))]
fn rgb(value: Option<&JsonValue>) -> Option<[f64; 3]> {
    let list = value?.as_array()?;
    Some([
        list.first()?.as_f64()?,
        list.get(1)?.as_f64()?,
        list.get(2)?.as_f64()?,
    ])
}

/// Which control a component is.
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Button,
    Switch,
    Segmented,
}

/// The props a control keeps to rebuild itself as they change.
#[cfg(any(target_os = "ios", target_os = "macos"))]
struct Props {
    title: String,
    style: String,
    symbol: Option<String>,
    tint: Option<[f64; 3]>,
    items: Vec<String>,
    /// The segmented control's selected index (-1: none).
    selected: isize,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Default for Props {
    fn default() -> Self {
        Props {
            title: String::new(),
            style: String::new(),
            symbol: None,
            tint: None,
            items: Vec::new(),
            selected: -1,
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Props {
    /// Takes prop `name`; true when the control must be rebuilt.
    fn take(&mut self, name: &str, value: Option<&JsonValue>) -> bool {
        let text = || value.and_then(JsonValue::as_str).map(str::to_owned);
        match name {
            "title" => self.title = text().unwrap_or_default(),
            "style" => self.style = text().unwrap_or_default(),
            "symbol" => self.symbol = text(),
            "tint" => self.tint = rgb(value),
            "selected" => self.selected = value.and_then(JsonValue::as_i64).unwrap_or(-1) as isize,
            "items" => {
                self.items = value
                    .and_then(JsonValue::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|i| i.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
            }
            _ => return false,
        }
        true
    }
}

#[cfg(target_os = "ios")]
pub use ios::register as register_ios;

#[cfg(target_os = "macos")]
pub use macos::register as register_macos;

/// UIKit's controls.
#[cfg(target_os = "ios")]
mod ios {
    use std::{ffi::CString, rc::Rc};

    use nsh_core::JsonValue;
    use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
    use objc2::{
        DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, NSObject, Sel},
        sel,
    };
    use objc2_core_foundation::CGSize;
    use objc2_ui_kit::UIView;

    use super::{BUTTON, Kind, Props, SEGMENTED, SWITCH};

    /// `UIControlEventValueChanged` and `UIControlEventPrimaryActionTriggered`.
    const VALUE_CHANGED: usize = 1 << 12;
    const PRIMARY_ACTION: usize = 1 << 13;

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the target only
        // forwards its controls' actions to the component's context.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "NshStarterNativeTarget"]
        #[ivars = ComponentContext]
        struct Target;

        impl Target {
            #[unsafe(method(pressed:))]
            fn pressed(&self, _sender: &AnyObject) {
                self.ivars().emit("press", JsonValue::Null);
            }

            #[unsafe(method(switched:))]
            fn switched(&self, sender: &AnyObject) {
                // SAFETY: UISwitch's `isOn`.
                let on: bool = unsafe { msg_send![sender, isOn] };
                self.ivars().emit("change", serde_json::json!({ "on": on }));
            }

            #[unsafe(method(segmented:))]
            fn segmented(&self, sender: &AnyObject) {
                // SAFETY: UISegmentedControl's `selectedSegmentIndex`.
                let index: isize = unsafe { msg_send![sender, selectedSegmentIndex] };
                self.ivars().emit("change", serde_json::json!({ "index": index }));
            }
        }
    );

    impl Target {
        fn new(mtm: MainThreadMarker, context: ComponentContext) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(context);
            // SAFETY: NSObject's designated initializer.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn class(name: &std::ffi::CStr) -> &'static AnyClass {
        AnyClass::get(name).expect("a UIKit class")
    }

    fn ns_string(text: &str) -> Retained<AnyObject> {
        let text = CString::new(text.replace('\0', "")).unwrap_or_default();
        // SAFETY: NSString's documented factory, with a NUL-terminated
        // UTF-8 buffer that outlives the call.
        unsafe { msg_send![class(c"NSString"), stringWithUTF8String: text.as_ptr()] }
    }

    fn color([r, g, b]: [f64; 3]) -> Retained<AnyObject> {
        // SAFETY: UIColor's documented factory.
        unsafe { msg_send![class(c"UIColor"), colorWithRed: r, green: g, blue: b, alpha: 1.0_f64] }
    }

    struct Control {
        kind: Kind,
        view: Retained<UIView>,
        // Kept alive while the control targets it (controls do not retain
        // their targets).
        _target: Retained<Target>,
        context: ComponentContext,
        props: Props,
    }

    impl Control {
        /// Applies the props to the view, then reports its natural size.
        fn apply(&self) {
            let view: &AnyObject = &self.view;
            let props = &self.props;
            // SAFETY: each call below is the documented UIKit API of the
            // control `kind` made (checked with respondsToSelector where it
            // is newer than iOS 15).
            unsafe {
                match self.kind {
                    Kind::Button => {
                        let configs = class(c"UIButtonConfiguration");
                        let glass = sel!(glassButtonConfiguration);
                        let config: Retained<AnyObject> = match props.style.as_str() {
                            "filled" => msg_send![configs, filledButtonConfiguration],
                            "tinted" => msg_send![configs, tintedButtonConfiguration],
                            "plain" => msg_send![configs, plainButtonConfiguration],
                            "glass" if configs.responds_to(glass) => {
                                msg_send![configs, glassButtonConfiguration]
                            }
                            _ => msg_send![configs, grayButtonConfiguration],
                        };
                        let _: () = msg_send![&*config, setTitle: &*ns_string(&props.title)];
                        if let Some(symbol) = &props.symbol {
                            let image: Option<Retained<AnyObject>> = msg_send![
                                class(c"UIImage"),
                                systemImageNamed: &*ns_string(symbol)
                            ];
                            let _: () = msg_send![&*config, setImage: image.as_deref()];
                            let _: () = msg_send![&*config, setImagePadding: 6.0_f64];
                        }
                        // UIButtonConfigurationCornerStyleCapsule.
                        let _: () = msg_send![&*config, setCornerStyle: 4_isize];
                        let _: () = msg_send![view, setConfiguration: &*config];
                        if let Some(tint) = props.tint {
                            let _: () = msg_send![view, setTintColor: &*color(tint)];
                        }
                    }
                    Kind::Switch => {
                        if let Some(tint) = props.tint {
                            let _: () = msg_send![view, setOnTintColor: &*color(tint)];
                        }
                    }
                    Kind::Segmented => {
                        let _: () = msg_send![view, removeAllSegments];
                        for (at, item) in props.items.iter().enumerate() {
                            let _: () = msg_send![
                                view,
                                insertSegmentWithTitle: &*ns_string(item),
                                atIndex: at,
                                animated: false
                            ];
                        }
                        let _: () = msg_send![view, setSelectedSegmentIndex: props.selected];
                        if let Some(tint) = props.tint {
                            let _: () = msg_send![view, setSelectedSegmentTintColor: &*color(tint)];
                        }
                    }
                }
                let size: CGSize = msg_send![view, intrinsicContentSize];
                self.context
                    .set_size(size.width.max(0.0) as f32, size.height.max(0.0) as f32);
            }
        }
    }

    impl NativeComponent for Control {
        fn view(&self) -> &UIView {
            &self.view
        }

        fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
            let view: &AnyObject = &self.view;
            match (self.kind, name) {
                (Kind::Switch, "on") => {
                    let on = value.and_then(JsonValue::as_bool).unwrap_or_default();
                    // SAFETY: UISwitch's `setOn:animated:`.
                    unsafe { msg_send![view, setOn: on, animated: true] }
                }
                (Kind::Segmented, "selected") => {
                    self.props.take(name, value);
                    // SAFETY: UISegmentedControl's `setSelectedSegmentIndex:`.
                    unsafe { msg_send![view, setSelectedSegmentIndex: self.props.selected] }
                }
                _ => {
                    if self.props.take(name, value) {
                        self.apply();
                    }
                }
            }
        }
    }

    fn make(kind: Kind, context: ComponentContext) -> Box<dyn NativeComponent> {
        let mtm = MainThreadMarker::new().expect("components are made on the main thread");
        let target = Target::new(mtm, context.clone());
        let (name, action, events): (&std::ffi::CStr, Sel, usize) = match kind {
            Kind::Button => (c"UIButton", sel!(pressed:), PRIMARY_ACTION),
            Kind::Switch => (c"UISwitch", sel!(switched:), VALUE_CHANGED),
            Kind::Segmented => (c"UISegmentedControl", sel!(segmented:), VALUE_CHANGED),
        };
        // SAFETY: the control's designated initializer, then UIControl's
        // documented target-action registration.
        let control: Retained<UIView> = unsafe {
            let control: Retained<UIView> = msg_send![msg_send![class(name), alloc], init];
            let _: () = msg_send![
                &*control,
                addTarget: &*target,
                action: action,
                forControlEvents: events
            ];
            control
        };
        let component = Control {
            kind,
            view: control,
            _target: target,
            context,
            props: Props::default(),
        };
        component.apply();
        Box::new(component)
    }

    /// Registers the three controls.
    pub fn register(components: &Components) {
        for (name, kind) in [
            (BUTTON, Kind::Button),
            (SWITCH, Kind::Switch),
            (SEGMENTED, Kind::Segmented),
        ] {
            components.register(
                name,
                Rc::new(move |context: ComponentContext| make(kind, context)),
            );
        }
    }
}

/// AppKit's controls.
#[cfg(target_os = "macos")]
mod macos {
    use std::{ffi::CString, rc::Rc};

    use nsh_core::JsonValue;
    use nsh_platform_macos::{
        ComponentContext, Components, NativeComponent,
        objc2::{
            DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
            rc::Retained,
            runtime::{AnyClass, AnyObject, NSObject, Sel},
            sel,
        },
        objc2_app_kit::NSView,
        objc2_foundation::NSSize,
    };

    use super::{BUTTON, Kind, Props, SEGMENTED, SWITCH};

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the target only
        // forwards its controls' actions to the component's context.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "NshStarterNativeTarget"]
        #[ivars = ComponentContext]
        struct Target;

        impl Target {
            #[unsafe(method(pressed:))]
            fn pressed(&self, _sender: &AnyObject) {
                self.ivars().emit("press", JsonValue::Null);
            }

            #[unsafe(method(switched:))]
            fn switched(&self, sender: &AnyObject) {
                // SAFETY: NSSwitch's `state` (NSControlStateValueOn is 1).
                let state: isize = unsafe { msg_send![sender, state] };
                self.ivars()
                    .emit("change", serde_json::json!({ "on": state == 1 }));
            }

            #[unsafe(method(segmented:))]
            fn segmented(&self, sender: &AnyObject) {
                // SAFETY: NSSegmentedControl's `selectedSegment`.
                let index: isize = unsafe { msg_send![sender, selectedSegment] };
                self.ivars().emit("change", serde_json::json!({ "index": index }));
            }
        }
    );

    impl Target {
        fn new(mtm: MainThreadMarker, context: ComponentContext) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(context);
            // SAFETY: NSObject's designated initializer.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn class(name: &std::ffi::CStr) -> &'static AnyClass {
        AnyClass::get(name).expect("an AppKit class")
    }

    fn ns_string(text: &str) -> Retained<AnyObject> {
        let text = CString::new(text.replace('\0', "")).unwrap_or_default();
        // SAFETY: NSString's documented factory, with a NUL-terminated
        // UTF-8 buffer that outlives the call.
        unsafe { msg_send![class(c"NSString"), stringWithUTF8String: text.as_ptr()] }
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

    struct Control {
        kind: Kind,
        view: Retained<NSView>,
        _target: Retained<Target>,
        context: ComponentContext,
        props: Props,
    }

    impl Control {
        fn apply(&self) {
            let view: &AnyObject = &self.view;
            let props = &self.props;
            // SAFETY: each call is the documented AppKit API of the control
            // `kind` made (macOS 11 and later).
            unsafe {
                match self.kind {
                    Kind::Button => {
                        let _: () = msg_send![view, setTitle: &*ns_string(&props.title)];
                        // NSBezelStylePush.
                        let _: () = msg_send![view, setBezelStyle: 1_usize];
                        let _: () = msg_send![view, setBordered: props.style != "plain"];
                        let tint = props.tint.map(color);
                        let none: Option<&AnyObject> = None;
                        match (props.style.as_str(), tint.as_deref()) {
                            ("filled", Some(tint)) => {
                                let _: () = msg_send![view, setBezelColor: tint];
                                let _: () = msg_send![view, setContentTintColor: none];
                            }
                            ("tinted" | "plain", Some(tint)) => {
                                let _: () = msg_send![view, setBezelColor: none];
                                let _: () = msg_send![view, setContentTintColor: tint];
                            }
                            _ => {
                                let _: () = msg_send![view, setBezelColor: none];
                                let _: () = msg_send![view, setContentTintColor: none];
                            }
                        }
                        if let Some(symbol) = &props.symbol {
                            let none: Option<&AnyObject> = None;
                            let image: Option<Retained<AnyObject>> = msg_send![
                                class(c"NSImage"),
                                imageWithSystemSymbolName: &*ns_string(symbol),
                                accessibilityDescription: none
                            ];
                            let _: () = msg_send![view, setImage: image.as_deref()];
                            // NSImageLeading.
                            let _: () = msg_send![view, setImagePosition: 7_usize];
                        }
                    }
                    Kind::Switch => {}
                    Kind::Segmented => {
                        let count: isize = msg_send![view, segmentCount];
                        if count as usize != props.items.len() {
                            let _: () =
                                msg_send![view, setSegmentCount: props.items.len() as isize];
                        }
                        for (at, item) in props.items.iter().enumerate() {
                            let _: () = msg_send![
                                view,
                                setLabel: &*ns_string(item),
                                forSegment: at as isize
                            ];
                        }
                        let _: () = msg_send![view, setSelectedSegment: props.selected];
                        if let Some(tint) = props.tint {
                            let _: () =
                                msg_send![view, setSelectedSegmentBezelColor: &*color(tint)];
                        }
                    }
                }
                let size: NSSize = msg_send![view, fittingSize];
                self.context
                    .set_size(size.width.max(0.0) as f32, size.height.max(0.0) as f32);
            }
        }
    }

    impl NativeComponent for Control {
        fn view(&self) -> Retained<NSView> {
            self.view.clone()
        }

        fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
            let view: &AnyObject = &self.view;
            match (self.kind, name) {
                (Kind::Switch, "on") => {
                    let on = value.and_then(JsonValue::as_bool).unwrap_or_default();
                    // SAFETY: NSSwitch's `setState:`.
                    unsafe { msg_send![view, setState: isize::from(on)] }
                }
                (Kind::Segmented, "selected") => {
                    self.props.take(name, value);
                    // SAFETY: NSSegmentedControl's `setSelectedSegment:`.
                    unsafe { msg_send![view, setSelectedSegment: self.props.selected] }
                }
                _ => {
                    if self.props.take(name, value) {
                        self.apply();
                    }
                }
            }
        }
    }

    fn make(kind: Kind, context: ComponentContext) -> Box<dyn NativeComponent> {
        let mtm = MainThreadMarker::new().expect("components are made on the main thread");
        let target = Target::new(mtm, context.clone());
        let (name, action): (&std::ffi::CStr, Sel) = match kind {
            Kind::Button => (c"NSButton", sel!(pressed:)),
            Kind::Switch => (c"NSSwitch", sel!(switched:)),
            Kind::Segmented => (c"NSSegmentedControl", sel!(segmented:)),
        };
        // SAFETY: the control's initializer, then NSControl's documented
        // target and action.
        let control: Retained<NSView> = unsafe {
            let control: Retained<NSView> = msg_send![msg_send![class(name), alloc], init];
            let _: () = msg_send![&*control, setTarget: &*target];
            let _: () = msg_send![&*control, setAction: action];
            if kind == Kind::Segmented {
                // NSSegmentSwitchTrackingSelectOne.
                let _: () = msg_send![&*control, setTrackingMode: 0_usize];
            }
            control
        };
        let component = Control {
            kind,
            view: control,
            _target: target,
            context,
            props: Props::default(),
        };
        component.apply();
        Box::new(component)
    }

    /// Registers the three controls.
    pub fn register(components: &Components) {
        for (name, kind) in [
            (BUTTON, Kind::Button),
            (SWITCH, Kind::Switch),
            (SEGMENTED, Kind::Segmented),
        ] {
            components.register(
                name,
                Rc::new(move |context: ComponentContext| make(kind, context)),
            );
        }
    }
}
