//! The macOS command bar: the palette as a window of its own, Raycast
//! style. A borderless, non-activating panel on Liquid Glass
//! (`NSGlassEffectView`; the HUD material before macOS 26) with a search
//! field and the command list, opened by Cmd+K in the app, by Option+Space
//! from any app, and from the menu bar item, which works with the main
//! window hidden.
//!
//! It was built on NSH 0.4.0, which had no second window, status item or
//! global shortcut, so AppKit draws all of it. 0.5.0 has all three
//! (`Window`, `Tray`, global shortcuts); moving onto them is a follow-up. A hidden native component in the shell
//! ([`bridge`]) hands the panel its commands and carries the chosen one back
//! into the app, where [`palette::dispatch`] runs it. Elsewhere the in-app
//! palette stays.

use nsh_core::{JsonValue, element::NativeEvent, prelude::*};

use crate::shell::{palette, routes::Route};

/// The bridge's name among the native components.
pub const BRIDGE: &str = "starter.command-bar";

/// The global shortcut, as the menu bar item shows it.
pub const HOTKEY_LABEL: &str = "⌥Space";

/// The command bar replaces the in-app palette here.
pub fn available() -> bool {
    cfg!(target_os = "macos") && os() == nsh_core::platform::Os::MacOs
}

/// How many times the app asked for the bar: the bridge opens it on each
/// new value.
#[derive(Default, Clone, PartialEq)]
struct Requests(u64);

/// Opens the bar (Cmd+K, the header's button).
pub fn open() {
    global::<Requests>().update(|r| r.0 += 1);
}

/// One command the bar lists.
struct Command {
    id: &'static str,
    title: &'static str,
    group: &'static str,
    symbol: &'static str,
    shortcut: &'static str,
    /// Brings the main window forward first (a destination).
    front: bool,
}

fn route_symbol(route: Route) -> &'static str {
    match route {
        Route::Home => "house",
        Route::UiGallery => "square.grid.2x2",
        Route::NativeVsApp => "rectangle.split.2x1",
        Route::Forms => "square.and.pencil",
        Route::Data | Route::DataDetail(_) => "tray.full",
        Route::QueryDevtools => "cylinder.split.1x2",
        Route::Lists => "list.bullet",
        Route::Canvas => "paintpalette",
        Route::Motion => "wind",
        Route::Overlays => "square.stack.3d.up",
        Route::Platform => "desktopcomputer",
        Route::Device => "iphone",
        Route::RemoteScreen => "shippingbox",
        Route::Diagnostics => "waveform.path.ecg",
    }
}

fn commands() -> Vec<Command> {
    let mut list: Vec<Command> = Route::ALL
        .iter()
        .map(|route| Command {
            id: route.key_str(),
            title: route.label_str(),
            group: "Go to",
            symbol: route_symbol(*route),
            shortcut: "",
            front: true,
        })
        .collect();
    list.extend([
        Command {
            id: "action:undo",
            title: "Undo",
            group: "Action",
            symbol: "arrow.uturn.backward",
            shortcut: "⌘Z",
            front: false,
        },
        Command {
            id: "action:redo",
            title: "Redo",
            group: "Action",
            symbol: "arrow.uturn.forward",
            shortcut: "⇧⌘Z",
            front: false,
        },
        Command {
            id: "action:appearance",
            title: "Toggle appearance",
            group: "Action",
            symbol: "circle.lefthalf.filled",
            shortcut: "",
            front: false,
        },
    ]);
    list
}

fn commands_json() -> JsonValue {
    JsonValue::from(
        commands()
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "title": c.title,
                    "group": c.group,
                    "symbol": c.symbol,
                    "shortcut": c.shortcut,
                    "front": c.front,
                })
            })
            .collect::<Vec<_>>(),
    )
}

/// The hidden element joining the shell to the bar; nothing elsewhere.
pub fn bridge() -> AnyElement {
    if !available() {
        return view().into_element();
    }
    let requests = global::<Requests>();
    nsh_core::element::native(BRIDGE)
        .prop("items", commands_json())
        .prop("open", move || JsonValue::from(requests.get().0))
        .on_native_event(move |e: &NativeEvent| {
            if &*e.name == "run"
                && let Some(id) = e.data["id"].as_str()
            {
                palette::dispatch(id);
            }
        })
        .absolute()
        .w(px(0.0))
        .h(px(0.0))
        .into_element()
}

/// How well `title` (or its `group`) matches `query`: a prefix beats a
/// word start, which beats a substring, which beats the letters in order.
/// `None`: no match.
pub fn score(query: &str, title: &str, group: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let title = title.to_lowercase();
    if title.starts_with(&query) {
        return Some(400);
    }
    if title
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(&query))
    {
        return Some(300);
    }
    if title.contains(&query) {
        return Some(200);
    }
    if group.to_lowercase().starts_with(&query) {
        return Some(150);
    }
    let mut letters = title.chars();
    query
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|q| letters.any(|t| t == q))
        .then_some(100)
}

/// The indices of `titles` matching `query`, best first, ties in list
/// order.
pub fn rank(query: &str, entries: &[(String, String)]) -> Vec<usize> {
    let mut hits: Vec<(u32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(at, (title, group))| score(query, title, group).map(|s| (s, at)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, at)| at).collect()
}

#[cfg(target_os = "macos")]
pub use macos::register as register_macos;

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        cell::{OnceCell, RefCell},
        ffi::{CStr, CString, c_char, c_void},
        rc::Rc,
    };

    use nsh_core::JsonValue;
    use nsh_platform_macos::{
        ComponentContext, Components, NativeComponent,
        objc2::{
            DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
            rc::Retained,
            runtime::{AnyClass, AnyObject, Bool, ClassBuilder, NSObject, Sel},
            sel,
        },
        objc2_app_kit::NSView,
        objc2_foundation::{NSPoint, NSRect, NSSize},
    };

    use super::{BRIDGE, HOTKEY_LABEL, rank};

    const WIDTH: f64 = 680.0;
    const HEIGHT: f64 = 430.0;
    const TOP: f64 = 64.0;
    const FOOTER: f64 = 36.0;
    const ROW: f64 = 44.0;
    const RADIUS: f64 = 18.0;

    /// One command, as the bridge sent it.
    #[derive(Clone)]
    struct Item {
        id: String,
        title: String,
        group: String,
        symbol: String,
        shortcut: String,
        front: bool,
    }

    /// The panel's views, made on first use.
    struct Views {
        panel: Retained<AnyObject>,
        field: Retained<AnyObject>,
        list: Retained<AnyObject>,
    }

    #[derive(Default)]
    struct State {
        items: Vec<Item>,
        /// The listed items, best match first.
        shown: Vec<usize>,
        selected: usize,
        /// The row backgrounds, for the highlight.
        rows: Vec<Retained<AnyObject>>,
        views: Option<Views>,
        /// The bridge: its context reaches the app, its view's window is
        /// the main window.
        context: Option<ComponentContext>,
        bridge: Option<Retained<NSView>>,
        /// The menu bar item (kept: the status bar does not retain it).
        status: Option<Retained<AnyObject>>,
        main_patched: bool,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; the controller
        // only receives AppKit's actions, delegate calls and timers.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "NshStarterCommandBar"]
        #[ivars = RefCell<State>]
        struct Controller;

        impl Controller {
            #[unsafe(method(controlTextDidChange:))]
            fn text_changed(&self, _note: &AnyObject) {
                self.refilter();
            }

            #[unsafe(method(control:textView:doCommandBySelector:))]
            fn command(&self, _control: &AnyObject, _view: &AnyObject, command: Sel) -> Bool {
                if command == sel!(moveDown:) {
                    self.step(1);
                } else if command == sel!(moveUp:) {
                    self.step(-1);
                } else if command == sel!(insertNewline:) {
                    let at = self.ivars().borrow().selected;
                    self.run(at);
                } else if command == sel!(cancelOperation:) {
                    self.hide();
                } else {
                    return Bool::NO;
                }
                Bool::YES
            }

            #[unsafe(method(windowDidResignKey:))]
            fn resigned(&self, _note: &AnyObject) {
                self.hide();
            }

            #[unsafe(method(pick:))]
            fn pick(&self, sender: &AnyObject) {
                // SAFETY: the row buttons' `tag`.
                let tag: isize = unsafe { msg_send![sender, tag] };
                self.run(tag.max(0) as usize);
            }

            #[unsafe(method(openBar:))]
            fn open_bar(&self, _sender: Option<&AnyObject>) {
                self.show();
            }

            #[unsafe(method(showMain:))]
            fn show_main_action(&self, _sender: Option<&AnyObject>) {
                self.show_main();
            }

            #[unsafe(method(hideMain:))]
            fn hide_main(&self, _sender: Option<&AnyObject>) {
                if let Some(window) = self.main_window() {
                    let none: Option<&AnyObject> = None;
                    // SAFETY: NSWindow's `orderOut:`.
                    let _: () = unsafe { msg_send![&*window, orderOut: none] };
                }
            }

            #[unsafe(method(patchMain:))]
            fn patch_main_action(&self, _sender: Option<&AnyObject>) {
                self.patch_main();
            }
        }
    );

    thread_local! {
        static CONTROLLER: OnceCell<Retained<Controller>> = const { OnceCell::new() };
    }

    fn controller() -> Retained<Controller> {
        CONTROLLER.with(|cell| {
            cell.get_or_init(|| {
                let mtm =
                    MainThreadMarker::new().expect("the command bar lives on the main thread");
                let this = Controller::alloc(mtm).set_ivars(RefCell::new(State::default()));
                // SAFETY: NSObject's designated initializer.
                let this: Retained<Controller> = unsafe { msg_send![super(this), init] };
                this.install();
                this
            })
            .clone()
        })
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

    /// The field's text.
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

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    fn symbol(name: &str) -> Option<Retained<AnyObject>> {
        let none: Option<&AnyObject> = None;
        // SAFETY: NSImage's SF Symbols factory (macOS 11).
        unsafe {
            msg_send![
                class(c"NSImage"),
                imageWithSystemSymbolName: &*ns_string(name),
                accessibilityDescription: none
            ]
        }
    }

    /// A one-line label in `size` points, `secondary` in the quieter color.
    ///
    /// # Safety
    /// On the main thread.
    unsafe fn label(text: &str, frame: NSRect, size: f64, secondary: bool) -> Retained<AnyObject> {
        // SAFETY: the caller's promise; NSTextField, NSFont and NSColor's
        // documented factories.
        unsafe {
            let label: Retained<AnyObject> =
                msg_send![class(c"NSTextField"), labelWithString: &*ns_string(text)];
            let _: () = msg_send![&*label, setFrame: frame];
            // NSFontWeightMedium for titles, regular for the rest.
            let weight = if secondary { 0.0_f64 } else { 0.23_f64 };
            let font: Retained<AnyObject> =
                msg_send![class(c"NSFont"), systemFontOfSize: size, weight: weight];
            let _: () = msg_send![&*label, setFont: &*font];
            let color: Retained<AnyObject> = if secondary {
                msg_send![class(c"NSColor"), secondaryLabelColor]
            } else {
                msg_send![class(c"NSColor"), labelColor]
            };
            let _: () = msg_send![&*label, setTextColor: &*color];
            let _: () = msg_send![&*label, setLineBreakMode: 4_usize];
            label
        }
    }

    /// A subclass of `superclass` answering `selector` with YES: a panel
    /// that can take the keyboard while borderless, a list laid out top
    /// down.
    fn yes_subclass(name: &CStr, superclass: &CStr, selector: Sel) -> &'static AnyClass {
        if let Some(made) = AnyClass::get(name) {
            return made;
        }
        extern "C-unwind" fn yes(_this: &AnyObject, _cmd: Sel) -> Bool {
            Bool::YES
        }
        let mut builder =
            ClassBuilder::new(name, class(superclass)).expect("a new class name for the bar");
        // SAFETY: the method's signature is the getter's (`-(BOOL)x`).
        unsafe {
            builder.add_method(selector, yes as extern "C-unwind" fn(_, _) -> _);
        }
        builder.register()
    }

    // Carbon's global hot keys: the one API that sees a shortcut while
    // another app is active, without the accessibility permission.
    #[repr(C)]
    struct EventTypeSpec {
        event_class: u32,
        event_kind: u32,
    }

    #[repr(C)]
    struct EventHotKeyId {
        signature: u32,
        id: u32,
    }

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn GetApplicationEventTarget() -> *mut c_void;
        fn InstallEventHandler(
            target: *mut c_void,
            handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
            count: u32,
            types: *const EventTypeSpec,
            user_data: *mut c_void,
            out: *mut *mut c_void,
        ) -> i32;
        fn RegisterEventHotKey(
            key_code: u32,
            modifiers: u32,
            id: EventHotKeyId,
            target: *mut c_void,
            options: u32,
            out: *mut *mut c_void,
        ) -> i32;
    }

    extern "C" fn hot_key(_next: *mut c_void, _event: *mut c_void, _data: *mut c_void) -> i32 {
        controller().toggle();
        0
    }

    fn register_hot_key() {
        // kEventClassKeyboard ('keyb'), kEventHotKeyPressed; kVK_Space
        // with optionKey.
        let pressed = EventTypeSpec {
            event_class: u32::from_be_bytes(*b"keyb"),
            event_kind: 5,
        };
        let id = EventHotKeyId {
            signature: u32::from_be_bytes(*b"nshs"),
            id: 1,
        };
        let mut handler = std::ptr::null_mut();
        let mut key = std::ptr::null_mut();
        // SAFETY: Carbon's documented calls, on the main thread, with the
        // application's own event target; the handler lives for the app.
        unsafe {
            let target = GetApplicationEventTarget();
            let installed = InstallEventHandler(
                target,
                hot_key,
                1,
                &pressed,
                std::ptr::null_mut(),
                &mut handler,
            );
            let registered = RegisterEventHotKey(49, 0x0800, id, target, 0, &mut key);
            if installed != 0 || registered != 0 {
                log::warn!(
                    "command bar: the {HOTKEY_LABEL} shortcut is unavailable \
                     (install {installed}, register {registered})"
                );
            }
        }
    }

    impl Controller {
        /// The menu bar item and the global shortcut, once per app.
        fn install(&self) {
            register_hot_key();
            // SAFETY: NSStatusBar, NSStatusItem and NSMenu's documented
            // API, on the main thread.
            let status = unsafe {
                let bar: Retained<AnyObject> = msg_send![class(c"NSStatusBar"), systemStatusBar];
                // NSVariableStatusItemLength.
                let item: Retained<AnyObject> = msg_send![&*bar, statusItemWithLength: -1.0_f64];
                let button: Option<Retained<AnyObject>> = msg_send![&*item, button];
                if let (Some(button), Some(image)) = (button, symbol("command.square")) {
                    let _: () = msg_send![&*image, setTemplate: true];
                    let _: () = msg_send![&*button, setImage: &*image];
                    let _: () = msg_send![&*button, setToolTip: &*ns_string("nsh-starter")];
                }
                let menu: Retained<AnyObject> = msg_send![
                    msg_send![class(c"NSMenu"), alloc],
                    initWithTitle: &*ns_string("nsh-starter")
                ];
                let add = |title: &str, action: Sel, key: &str| -> Retained<AnyObject> {
                    let entry: Retained<AnyObject> = msg_send![
                        &*menu,
                        addItemWithTitle: &*ns_string(title),
                        action: action,
                        keyEquivalent: &*ns_string(key)
                    ];
                    let _: () = msg_send![&*entry, setTarget: self];
                    entry
                };
                let open = add("Open Command Bar", sel!(openBar:), " ");
                // NSEventModifierFlagOption: shown beside the entry.
                let _: () = msg_send![&*open, setKeyEquivalentModifierMask: 1_usize << 19];
                add("Show nsh-starter", sel!(showMain:), "");
                let separator: Retained<AnyObject> = msg_send![class(c"NSMenuItem"), separatorItem];
                let _: () = msg_send![&*menu, addItem: &*separator];
                // No target: `terminate:` goes up the responder chain to NSApp.
                let _: Retained<AnyObject> = msg_send![
                    &*menu,
                    addItemWithTitle: &*ns_string("Quit nsh-starter"),
                    action: sel!(terminate:),
                    keyEquivalent: &*ns_string("q")
                ];
                let _: () = msg_send![&*item, setMenu: &*menu];
                item
            };
            self.ivars().borrow_mut().status = Some(status);
        }

        /// The panel and its views, made on first use.
        fn views(&self) -> (Retained<AnyObject>, Retained<AnyObject>) {
            if let Some(views) = &self.ivars().borrow().views {
                return (views.panel.clone(), views.field.clone());
            }
            // SAFETY: NSPanel, NSGlassEffectView (or NSVisualEffectView),
            // NSTextField, NSScrollView and NSBox's documented API, on the
            // main thread; frames are in each superview's coordinates.
            let views = unsafe {
                let panel_class = yes_subclass(
                    c"NshStarterCommandPanel",
                    c"NSPanel",
                    sel!(canBecomeKeyWindow),
                );
                // NSWindowStyleMaskBorderless | NonactivatingPanel; buffered.
                let panel: Retained<AnyObject> = msg_send![
                    msg_send![panel_class, alloc],
                    initWithContentRect: rect(0.0, 0.0, WIDTH, HEIGHT),
                    styleMask: 1_usize << 7,
                    backing: 2_usize,
                    defer: false
                ];
                let clear: Retained<AnyObject> = msg_send![class(c"NSColor"), clearColor];
                let _: () = msg_send![&*panel, setOpaque: false];
                let _: () = msg_send![&*panel, setBackgroundColor: &*clear];
                let _: () = msg_send![&*panel, setHasShadow: true];
                let _: () = msg_send![&*panel, setFloatingPanel: true];
                let _: () = msg_send![&*panel, setHidesOnDeactivate: false];
                let _: () = msg_send![&*panel, setReleasedWhenClosed: false];
                let _: () = msg_send![&*panel, setMovableByWindowBackground: true];
                // NSStatusWindowLevel: over full-screen apps too.
                let _: () = msg_send![&*panel, setLevel: 25_isize];
                // CanJoinAllSpaces | FullScreenAuxiliary.
                let _: () = msg_send![&*panel, setCollectionBehavior: 1_usize | (1_usize << 8)];
                let _: () = msg_send![&*panel, setDelegate: self];

                // The window's content: the backdrop at the bottom, the
                // bar's views over it as a sibling. Inside the glass's own
                // content view they would take its tint and vibrancy.
                let frame = rect(0.0, 0.0, WIDTH, HEIGHT);
                let root: Retained<NSView> =
                    msg_send![msg_send![class(c"NSView"), alloc], initWithFrame: frame];
                let _: () = msg_send![&*root, setWantsLayer: true];
                let _: () = msg_send![&*panel, setContentView: &*root];
                // The glass draws its own edge; the system shadow framed it
                // as a hard box.
                let _: () = msg_send![&*panel, setHasShadow: false];
                let backdrop: Retained<AnyObject> = match AnyClass::get(c"NSGlassEffectView") {
                    Some(glass) => {
                        let backdrop: Retained<AnyObject> =
                            msg_send![msg_send![glass, alloc], initWithFrame: frame];
                        let _: () = msg_send![&*backdrop, setCornerRadius: RADIUS];
                        // NSGlassEffectViewStyleClear: the raw glass, not the
                        // appearance's dark or light panel.
                        let _: () = msg_send![&*backdrop, setStyle: 1_isize];
                        // The heavily frosted variant (private, so asked for
                        // only where it answers; gpui-starter's launcher).
                        if backdrop.class().responds_to(sel!(set_variant:)) {
                            let _: () = msg_send![&*backdrop, set_variant: 3_isize];
                        }
                        // A breath of tint keeps text readable over a bright
                        // desktop without turning the glass into a fill.
                        let tint: Retained<AnyObject> = msg_send![
                            class(c"NSColor"),
                            colorWithRed: 0.0_f64,
                            green: 0.0_f64,
                            blue: 0.0_f64,
                            alpha: 0.06_f64
                        ];
                        let _: () = msg_send![&*backdrop, setTintColor: &*tint];
                        // Glass renders only around a layer-backed content
                        // view.
                        let inner: Retained<NSView> =
                            msg_send![msg_send![class(c"NSView"), alloc], initWithFrame: frame];
                        let _: () = msg_send![&*inner, setWantsLayer: true];
                        let _: () = msg_send![&*inner, setAutoresizingMask: 2_usize | 16_usize];
                        let _: () = msg_send![&*backdrop, setContentView: &*inner];
                        backdrop
                    }
                    None => {
                        // Before macOS 26: the HUD material, behind the
                        // window, always active, rounded by its layer.
                        let backdrop: Retained<AnyObject> = msg_send![
                            msg_send![class(c"NSVisualEffectView"), alloc],
                            initWithFrame: frame
                        ];
                        let _: () = msg_send![&*backdrop, setMaterial: 13_isize];
                        let _: () = msg_send![&*backdrop, setBlendingMode: 0_isize];
                        let _: () = msg_send![&*backdrop, setState: 1_isize];
                        let _: () = msg_send![&*backdrop, setWantsLayer: true];
                        let layer: Retained<AnyObject> = msg_send![&*backdrop, layer];
                        let _: () = msg_send![&*layer, setCornerRadius: RADIUS];
                        let _: () = msg_send![&*layer, setMasksToBounds: true];
                        backdrop
                    }
                };
                let _: () = msg_send![&*backdrop, setAutoresizingMask: 2_usize | 16_usize];
                let _: () = msg_send![&*root, addSubview: &*backdrop];
                let content: Retained<NSView> =
                    msg_send![msg_send![class(c"NSView"), alloc], initWithFrame: frame];
                let _: () = msg_send![&*root, addSubview: &*content];

                // The search row.
                if let Some(image) = symbol("magnifyingglass") {
                    let icon: Retained<AnyObject> =
                        msg_send![class(c"NSImageView"), imageViewWithImage: &*image];
                    let _: () = msg_send![&*icon, setFrame: rect(24.0, HEIGHT - 42.0, 20.0, 20.0)];
                    let gray: Retained<AnyObject> =
                        msg_send![class(c"NSColor"), secondaryLabelColor];
                    let _: () = msg_send![&*icon, setContentTintColor: &*gray];
                    let _: () = msg_send![&*content, addSubview: &*icon];
                }
                let field: Retained<AnyObject> = msg_send![
                    msg_send![class(c"NSTextField"), alloc],
                    initWithFrame: rect(56.0, HEIGHT - 47.0, WIDTH - 80.0, 30.0)
                ];
                let _: () = msg_send![&*field, setBezeled: false];
                let _: () = msg_send![&*field, setBordered: false];
                let _: () = msg_send![&*field, setDrawsBackground: false];
                // NSFocusRingTypeNone.
                let _: () = msg_send![&*field, setFocusRingType: 1_usize];
                let font: Retained<AnyObject> =
                    msg_send![class(c"NSFont"), systemFontOfSize: 21.0_f64, weight: 0.0_f64];
                let _: () = msg_send![&*field, setFont: &*font];
                let _: () = msg_send![
                    &*field,
                    setPlaceholderString: &*ns_string("Search pages and actions…")
                ];
                let _: () = msg_send![&*field, setDelegate: self];
                let _: () = msg_send![&*content, addSubview: &*field];

                // NSBoxSeparator lines under the search row and over the
                // footer.
                for y in [HEIGHT - TOP, FOOTER] {
                    let line: Retained<AnyObject> = msg_send![
                        msg_send![class(c"NSBox"), alloc],
                        initWithFrame: rect(0.0, y, WIDTH, 1.0)
                    ];
                    let _: () = msg_send![&*line, setBoxType: 2_usize];
                    let _: () = msg_send![&*content, addSubview: &*line];
                }

                // The list, top down in a scroll view.
                let list_class = yes_subclass(c"NshStarterCommandList", c"NSView", sel!(isFlipped));
                let list_height = HEIGHT - TOP - FOOTER - 2.0;
                let list: Retained<AnyObject> = msg_send![
                    msg_send![list_class, alloc],
                    initWithFrame: rect(0.0, 0.0, WIDTH, list_height)
                ];
                let scroll: Retained<AnyObject> = msg_send![
                    msg_send![class(c"NSScrollView"), alloc],
                    initWithFrame: rect(0.0, FOOTER + 1.0, WIDTH, list_height)
                ];
                let _: () = msg_send![&*scroll, setDrawsBackground: false];
                let _: () = msg_send![&*scroll, setHasVerticalScroller: true];
                let _: () = msg_send![&*scroll, setAutohidesScrollers: true];
                let clip: Retained<AnyObject> = msg_send![&*scroll, contentView];
                let _: () = msg_send![&*clip, setDrawsBackground: false];
                let _: () = msg_send![&*scroll, setDocumentView: &*list];
                let _: () = msg_send![&*content, addSubview: &*scroll];

                // The footer: the keys, then the global shortcut.
                let keys = label(
                    "↵ Open     ↑↓ Move     esc Close",
                    rect(24.0, 10.0, 360.0, 16.0),
                    12.0,
                    true,
                );
                let _: () = msg_send![&*content, addSubview: &*keys];
                let anywhere = label(
                    &format!("{HOTKEY_LABEL} from anywhere"),
                    rect(WIDTH - 224.0, 10.0, 200.0, 16.0),
                    12.0,
                    true,
                );
                // NSTextAlignmentRight.
                let _: () = msg_send![&*anywhere, setAlignment: 1_isize];
                let _: () = msg_send![&*content, addSubview: &*anywhere];

                Views { panel, field, list }
            };
            let handles = (views.panel.clone(), views.field.clone());
            self.ivars().borrow_mut().views = Some(views);
            handles
        }

        fn show(&self) {
            let (panel, field) = self.views();
            // SAFETY: NSScreen and NSWindow's documented API.
            unsafe {
                let screen: Option<Retained<AnyObject>> = msg_send![class(c"NSScreen"), mainScreen];
                if let Some(screen) = screen {
                    let frame: NSRect = msg_send![&*screen, visibleFrame];
                    let x = frame.origin.x + (frame.size.width - WIDTH) / 2.0;
                    let y = frame.origin.y + frame.size.height * 0.78 - HEIGHT;
                    let _: () = msg_send![&*panel, setFrameOrigin: NSPoint::new(x, y)];
                }
                let _: () = msg_send![&*field, setStringValue: &*ns_string("")];
            }
            self.refilter();
            let none: Option<&AnyObject> = None;
            // SAFETY: as above.
            unsafe {
                let _: () = msg_send![&*panel, makeKeyAndOrderFront: none];
                let _: bool = msg_send![&*panel, makeFirstResponder: &*field];
            }
        }

        fn hide(&self) {
            let panel = self
                .ivars()
                .borrow()
                .views
                .as_ref()
                .map(|v| v.panel.clone());
            if let Some(panel) = panel {
                let none: Option<&AnyObject> = None;
                // SAFETY: NSWindow's `orderOut:`.
                let _: () = unsafe { msg_send![&*panel, orderOut: none] };
            }
        }

        fn toggle(&self) {
            let panel = self
                .ivars()
                .borrow()
                .views
                .as_ref()
                .map(|v| v.panel.clone());
            // SAFETY: NSWindow's `isVisible`.
            let visible = panel.is_some_and(|p| unsafe { msg_send![&*p, isVisible] });
            if visible {
                self.hide();
            } else {
                self.show();
            }
        }

        /// Ranks the commands against the field and redraws the rows.
        fn refilter(&self) {
            let field = self
                .ivars()
                .borrow()
                .views
                .as_ref()
                .map(|v| v.field.clone());
            // SAFETY: the field is an NSTextField.
            let query = field.map_or_else(String::new, |f| unsafe { string_value(&f) });
            {
                let mut state = self.ivars().borrow_mut();
                let entries: Vec<(String, String)> = state
                    .items
                    .iter()
                    .map(|i| (i.title.clone(), i.group.clone()))
                    .collect();
                state.shown = rank(&query, &entries);
                state.selected = 0;
            }
            self.draw_rows();
        }

        fn draw_rows(&self) {
            let (list, shown, items) = {
                let state = self.ivars().borrow();
                let Some(views) = &state.views else { return };
                (views.list.clone(), state.shown.clone(), state.items.clone())
            };
            let mut rows = Vec::new();
            // SAFETY: NSView, NSBox, NSImageView, NSTextField and NSButton's
            // documented API; `list` is the flipped document view.
            unsafe {
                let old: Retained<AnyObject> = msg_send![&*list, subviews];
                let count: usize = msg_send![&*old, count];
                let old: Vec<Retained<AnyObject>> = (0..count)
                    .map(|at| msg_send![&*old, objectAtIndex: at])
                    .collect();
                for view in old {
                    let _: () = msg_send![&*view, removeFromSuperview];
                }
                let visible = HEIGHT - TOP - FOOTER - 2.0;
                let height = (shown.len() as f64 * ROW + 12.0).max(visible);
                let _: () = msg_send![&*list, setFrameSize: NSSize::new(WIDTH, height)];
                if shown.is_empty() {
                    let empty = label(
                        "No pages or actions match",
                        rect(24.0, 24.0, WIDTH - 48.0, 20.0),
                        14.0,
                        true,
                    );
                    let _: () = msg_send![&*list, addSubview: &*empty];
                }
                let w = WIDTH - 20.0;
                for (at, index) in shown.iter().enumerate() {
                    let item = &items[*index];
                    // NSBoxCustom: a rounded fill, shown on the selected row.
                    let row: Retained<AnyObject> = msg_send![
                        msg_send![class(c"NSBox"), alloc],
                        initWithFrame: rect(10.0, 6.0 + at as f64 * ROW, w, ROW - 4.0)
                    ];
                    let _: () = msg_send![&*row, setBoxType: 4_usize];
                    let _: () = msg_send![&*row, setTitlePosition: 0_usize];
                    let _: () = msg_send![&*row, setBorderWidth: 0.0_f64];
                    let _: () = msg_send![&*row, setCornerRadius: 10.0_f64];
                    let _: () = msg_send![&*row, setContentViewMargins: NSSize::new(0.0, 0.0)];
                    let label_color: Retained<AnyObject> = msg_send![class(c"NSColor"), labelColor];
                    let fill: Retained<AnyObject> =
                        msg_send![&*label_color, colorWithAlphaComponent: 0.1_f64];
                    let _: () = msg_send![&*row, setFillColor: &*fill];
                    let _: () = msg_send![&*row, setTransparent: true];
                    let body: Retained<AnyObject> = msg_send![&*row, contentView];
                    if let Some(image) = symbol(&item.symbol) {
                        let icon: Retained<AnyObject> =
                            msg_send![class(c"NSImageView"), imageViewWithImage: &*image];
                        let _: () = msg_send![&*icon, setFrame: rect(14.0, 10.0, 20.0, 20.0)];
                        let _: () = msg_send![&*body, addSubview: &*icon];
                    }
                    let title = label(&item.title, rect(46.0, 11.0, w - 250.0, 18.0), 14.0, false);
                    let _: () = msg_send![&*body, addSubview: &*title];
                    let hint = if item.shortcut.is_empty() {
                        &item.group
                    } else {
                        &item.shortcut
                    };
                    let hint = label(hint, rect(w - 200.0, 12.0, 184.0, 16.0), 12.0, true);
                    let _: () = msg_send![&*hint, setAlignment: 1_isize];
                    let _: () = msg_send![&*body, addSubview: &*hint];
                    // A borderless, transparent button over the row: the
                    // click.
                    let hit: Retained<AnyObject> = msg_send![
                        msg_send![class(c"NSButton"), alloc],
                        initWithFrame: rect(0.0, 0.0, w, ROW - 4.0)
                    ];
                    let _: () = msg_send![&*hit, setTitle: &*ns_string("")];
                    let _: () = msg_send![&*hit, setBordered: false];
                    let _: () = msg_send![&*hit, setTransparent: true];
                    let _: () = msg_send![&*hit, setTag: at as isize];
                    let _: () = msg_send![&*hit, setTarget: self];
                    let _: () = msg_send![&*hit, setAction: sel!(pick:)];
                    let _: () = msg_send![&*body, addSubview: &*hit];
                    let _: () = msg_send![&*list, addSubview: &*row];
                    rows.push(row);
                }
            }
            self.ivars().borrow_mut().rows = rows;
            self.highlight();
        }

        fn highlight(&self) {
            let (rows, selected, list) = {
                let state = self.ivars().borrow();
                let Some(views) = &state.views else { return };
                (state.rows.clone(), state.selected, views.list.clone())
            };
            // SAFETY: the rows are NSBoxes in the flipped list.
            unsafe {
                for (at, row) in rows.iter().enumerate() {
                    let _: () = msg_send![&**row, setTransparent: at != selected];
                }
                if rows.get(selected).is_some() {
                    let frame = rect(0.0, selected as f64 * ROW, WIDTH, ROW + 12.0);
                    let _: bool = msg_send![&*list, scrollRectToVisible: frame];
                }
            }
        }

        fn step(&self, by: isize) {
            {
                let mut state = self.ivars().borrow_mut();
                let count = state.shown.len() as isize;
                if count == 0 {
                    return;
                }
                state.selected = (state.selected as isize + by).rem_euclid(count) as usize;
            }
            self.highlight();
        }

        /// Runs the listed command `at`: hides the bar, brings the main
        /// window forward for a destination, and hands the id to the app.
        fn run(&self, at: usize) {
            let picked = {
                let state = self.ivars().borrow();
                state
                    .shown
                    .get(at)
                    .and_then(|i| state.items.get(*i))
                    .cloned()
                    .zip(state.context.clone())
            };
            self.hide();
            let Some((item, context)) = picked else {
                return;
            };
            if item.front {
                self.show_main();
            }
            context.emit("run", serde_json::json!({ "id": item.id }));
        }

        fn main_window(&self) -> Option<Retained<AnyObject>> {
            let bridge = self.ivars().borrow().bridge.clone()?;
            // SAFETY: NSView's `window`.
            unsafe { msg_send![&*bridge, window] }
        }

        fn show_main(&self) {
            let Some(window) = self.main_window() else {
                return;
            };
            let none: Option<&AnyObject> = None;
            // SAFETY: NSApplication and NSWindow's documented API.
            unsafe {
                let app: Retained<AnyObject> =
                    msg_send![class(c"NSApplication"), sharedApplication];
                let _: () = msg_send![&*app, activateIgnoringOtherApps: true];
                let minimized: bool = msg_send![&*window, isMiniaturized];
                if minimized {
                    let _: () = msg_send![&*window, deminiaturize: none];
                }
                let _: () = msg_send![&*window, makeKeyAndOrderFront: none];
            }
        }

        /// The main window's close button and Cmd+W hide it instead: the
        /// runner quits when its window closes (`keep_alive_on_close` is
        /// not reachable from an app), and the menu bar item must outlive
        /// it. The Dock icon and "Show nsh-starter" bring it back.
        fn patch_main(&self) {
            if self.ivars().borrow().main_patched {
                return;
            }
            let Some(window) = self.main_window() else {
                // Not in its window yet: try again shortly.
                let none: Option<&AnyObject> = None;
                // SAFETY: NSObject's `performSelector:withObject:afterDelay:`.
                let _: () = unsafe {
                    msg_send![
                        self,
                        performSelector: sel!(patchMain:),
                        withObject: none,
                        afterDelay: 0.25_f64
                    ]
                };
                return;
            };
            // SAFETY: NSWindow, NSButton, NSApplication and NSMenu's
            // documented API.
            unsafe {
                // NSWindowCloseButton.
                let close: Option<Retained<AnyObject>> =
                    msg_send![&*window, standardWindowButton: 0_usize];
                if let Some(close) = close {
                    let _: () = msg_send![&*close, setTarget: self];
                    let _: () = msg_send![&*close, setAction: sel!(hideMain:)];
                }
                let app: Retained<AnyObject> =
                    msg_send![class(c"NSApplication"), sharedApplication];
                let main: Option<Retained<AnyObject>> = msg_send![&*app, mainMenu];
                if let Some(main) = main {
                    let tops: Retained<AnyObject> = msg_send![&*main, itemArray];
                    let count: usize = msg_send![&*tops, count];
                    for at in 0..count {
                        let top: Retained<AnyObject> = msg_send![&*tops, objectAtIndex: at];
                        let menu: Option<Retained<AnyObject>> = msg_send![&*top, submenu];
                        let Some(menu) = menu else { continue };
                        let entries: Retained<AnyObject> = msg_send![&*menu, itemArray];
                        let n: usize = msg_send![&*entries, count];
                        for i in 0..n {
                            let entry: Retained<AnyObject> = msg_send![&*entries, objectAtIndex: i];
                            let action: Option<Sel> = msg_send![&*entry, action];
                            if action == Some(sel!(performClose:)) {
                                let _: () = msg_send![&*entry, setTarget: self];
                                let _: () = msg_send![&*entry, setAction: sel!(hideMain:)];
                            }
                        }
                    }
                }
            }
            self.ivars().borrow_mut().main_patched = true;
        }
    }

    /// The bridge: an empty view in the main window.
    struct Bridge {
        view: Retained<NSView>,
        /// The last open request seen.
        opened: u64,
    }

    impl NativeComponent for Bridge {
        fn view(&self) -> Retained<NSView> {
            self.view.clone()
        }

        fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
            let bar = controller();
            match name {
                "items" => {
                    let items = value
                        .and_then(JsonValue::as_array)
                        .map(|list| {
                            list.iter()
                                .map(|c| {
                                    let text =
                                        |k: &str| c[k].as_str().unwrap_or_default().to_owned();
                                    Item {
                                        id: text("id"),
                                        title: text("title"),
                                        group: text("group"),
                                        symbol: text("symbol"),
                                        shortcut: text("shortcut"),
                                        front: c["front"].as_bool().unwrap_or_default(),
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    bar.ivars().borrow_mut().items = items;
                }
                "open" => {
                    let seen = value.and_then(JsonValue::as_u64).unwrap_or_default();
                    if seen > self.opened {
                        self.opened = seen;
                        bar.toggle();
                    }
                }
                _ => {}
            }
        }
    }

    fn make(context: ComponentContext) -> Box<dyn NativeComponent> {
        let bar = controller();
        // SAFETY: NSView's designated initializer.
        let view: Retained<NSView> = unsafe {
            msg_send![
                msg_send![class(c"NSView"), alloc],
                initWithFrame: rect(0.0, 0.0, 0.0, 0.0)
            ]
        };
        {
            let mut state = bar.ivars().borrow_mut();
            state.context = Some(context);
            state.bridge = Some(view.clone());
        }
        bar.patch_main();
        Box::new(Bridge { view, opened: 0 })
    }

    /// Registers the bridge; the bar, the menu bar item and the shortcut
    /// come with the first one made.
    pub fn register(components: &Components) {
        components.register(BRIDGE, Rc::new(make));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(titles: &[(&str, &str)]) -> Vec<(String, String)> {
        titles
            .iter()
            .map(|(t, g)| ((*t).to_owned(), (*g).to_owned()))
            .collect()
    }

    #[test]
    fn a_prefix_ranks_above_a_word_start_and_letters_in_order() {
        let list = entries(&[
            ("Query devtools", "Go to"),
            ("Diagnostics", "Go to"),
            ("Data", "Go to"),
            ("Device", "Go to"),
        ]);
        assert_eq!(rank("d", &list), vec![1, 2, 3, 0]);
        assert_eq!(rank("dev", &list), vec![3, 0]);
        assert_eq!(rank("qdt", &list), vec![0]);
        assert_eq!(rank("zz", &list), Vec::<usize>::new());
    }

    #[test]
    fn an_empty_query_keeps_the_list_order() {
        let list = entries(&[("Home", "Go to"), ("Undo", "Action")]);
        assert_eq!(rank("  ", &list), vec![0, 1]);
        assert_eq!(rank("act", &list), vec![1]);
    }

    #[test]
    fn every_route_and_action_is_a_command() {
        let ids: Vec<&str> = commands().iter().map(|c| c.id).collect();
        for route in Route::ALL {
            assert!(ids.contains(&route.key_str()), "{route:?} missing");
        }
        assert!(ids.contains(&"action:appearance"));
    }
}
