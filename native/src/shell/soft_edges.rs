//! UIKit's soft scroll edge effect on iOS: content that scrolls under the
//! transparent header (and toward the floating tab bar) blurs and fades
//! lightly instead of meeting a flat band with a hard line. The navigation
//! bar is kept in the iPad style (NSH keeps it in the window), which gives
//! scroll views UIKit's hard edge effect; this view sets every scroll view
//! in its window to the soft one when it lands there. Before iOS 26 there
//! is no edge effect and it does nothing.
//!
//! [`soft_edges`] is the element a page places; [`register`] is the iOS
//! factory the crate root registers.

use nsh_core::prelude::*;

/// The registered name of the native view.
pub const NAME: &str = "starter.soft-edges";

/// The invisible element that softens its window's scroll edges (iOS only;
/// elsewhere nothing).
pub fn soft_edges() -> impl IntoElement {
    when(
        || os() == nsh_core::platform::Os::Ios,
        || {
            nsh_core::element::native(NAME)
                // The theme's font, for the header's titles.
                .prop("font", || {
                    let family = nsh_base::theme().font_sans.clone();
                    nsh_core::JsonValue::from(
                        family.as_deref().and_then(first_family).unwrap_or_default(),
                    )
                })
                .absolute()
                .top_0()
                .left_0()
                .w(px(1.0))
                .h(px(1.0))
                .pointer_events(nsh_core::PointerEvents::None)
                .into_element()
        },
    )
}

/// The first family of a CSS font list (`"Onest, sans-serif"`), unless it
/// is a generic one (then the system's font stays).
fn first_family(list: &str) -> Option<String> {
    let first = list.split(',').next()?.trim().trim_matches(['"', '\'']);
    let generic = [
        "system-ui",
        "sans-serif",
        "serif",
        "monospace",
        "ui-sans-serif",
        "-apple-system",
    ];
    (!first.is_empty() && !generic.contains(&first)).then(|| first.to_owned())
}

/// An invisible strip of `height` points at its parent's bottom, behind a
/// floating bar: UIKit draws the soft edge of the page's scroll view under
/// it, as it does under its own bars (iOS only; elsewhere nothing). The
/// parent is the page, whose first scroll view it follows.
pub fn soft_bottom_edge(height: impl Fn() -> f32 + 'static) -> impl IntoElement {
    let height = std::rc::Rc::new(height);
    when(
        || os() == nsh_core::platform::Os::Ios,
        move || {
            let height = height.clone();
            nsh_core::element::native(NAME)
                .prop("edge", nsh_core::JsonValue::from("bottom"))
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(move || px(height()))
                .pointer_events(nsh_core::PointerEvents::None)
                .into_element()
        },
    )
}

/// Both soft edges for a page that is not the shell's own (a feature's
/// stack, such as Forms): the header's and the floating tab bar's, the
/// same as `shell::page` gives every other page. Its last child, over the
/// page's scroll. iOS phones only; elsewhere nothing.
pub fn page_edges() -> impl IntoElement {
    when(
        || os() == nsh_core::platform::Os::Ios && !is_desktop(),
        || {
            view()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .pointer_events(nsh_core::PointerEvents::None)
                .child(soft_edges())
                .child(soft_bottom_edge(|| {
                    crate::ui::layout::tab_bar_room() + safe_area().bottom.0
                }))
                .into_element()
        },
    )
}

#[cfg(target_os = "ios")]
pub use ios::register;

#[cfg(target_os = "ios")]
mod ios {
    use std::rc::Rc;

    use nsh_core::JsonValue;
    use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
    use objc2::{
        MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, NSObjectProtocol},
        sel,
    };
    use objc2_core_foundation::CGPoint;
    use objc2_ui_kit::{UIScrollView, UIView};

    define_class!(
        // SAFETY: UIView has no subclassing requirements beyond calling
        // super, which `didMoveToWindow` does.
        #[unsafe(super(UIView))]
        #[thread_kind = MainThreadOnly]
        #[name = "NshStarterSoftEdges"]
        struct SoftEdgesView;

        impl SoftEdgesView {
            #[unsafe(method(didMoveToWindow))]
            fn did_move_to_window(&self) {
                // SAFETY: UIView's own method, on super.
                let _: () = unsafe { msg_send![super(self), didMoveToWindow] };
                if let Some(window) = self.window() {
                    soften(&window);
                    // After this run loop turn, when the header has its
                    // large title laid out (the top view only).
                    // SAFETY: NSObject's documented delayed perform, of
                    // this class's own `restAtTop` method.
                    let _: () = unsafe {
                        msg_send![self, performSelector: sel!(restAtTop), withObject: std::ptr::null::<AnyObject>(), afterDelay: 0.0_f64]
                    };
                }
            }

            #[unsafe(method(restAtTop))]
            fn rest_at_top(&self) {
                if self.tag() != BOTTOM && self.window().is_some() {
                    self.style_titles();
                    self.open_large_title();
                }
            }

            // NSH styles the header again when the appearance flips; the
            // titles take the font again after it.
            #[unsafe(method(traitCollectionDidChange:))]
            fn trait_collection_did_change(&self, previous: Option<&AnyObject>) {
                // SAFETY: UIView's own method, on super; then NSObject's
                // documented delayed perform of this class's method.
                unsafe {
                    let _: () = msg_send![super(self), traitCollectionDidChange: previous];
                    let _: () = msg_send![self, performSelector: sel!(restyleTitles), withObject: std::ptr::null::<AnyObject>(), afterDelay: 0.0_f64];
                }
            }

            #[unsafe(method(restyleTitles))]
            fn restyle_titles(&self) {
                if self.tag() != BOTTOM && self.window().is_some() {
                    self.style_titles();
                }
            }

            // Laid out once it is in the window with its props and the page
            // beside it: the strip joins its scroll view's bottom edge then.
            #[unsafe(method(layoutSubviews))]
            fn layout_subviews(&self) {
                // SAFETY: UIView's own method, on super.
                let _: () = unsafe { msg_send![super(self), layoutSubviews] };
                if self.tag() == BOTTOM && self.window().is_some() && self.interactions().is_empty() {
                    self.follow_bottom_edge();
                }
            }
        }
    );

    std::thread_local! {
        /// The theme's font family for the header titles (`font` prop).
        static TITLE_FONT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    }

    fn ns_string(text: &str) -> Retained<AnyObject> {
        let text = std::ffi::CString::new(text.replace('\0', "")).unwrap_or_default();
        // SAFETY: NSString's documented factory, with a NUL-terminated
        // UTF-8 buffer that outlives the call.
        unsafe {
            msg_send![AnyClass::get(c"NSString").expect("NSString"), stringWithUTF8String: text.as_ptr()]
        }
    }

    /// The registered `family` at `size` points and `weight` (UIFontWeight:
    /// 0.3 semibold, 0.4 bold), or `None` when UIKit has no such family.
    fn font(family: &str, size: f64, weight: f64) -> Option<Retained<AnyObject>> {
        let (descriptors, fonts, numbers, dictionaries) = (
            AnyClass::get(c"UIFontDescriptor")?,
            AnyClass::get(c"UIFont")?,
            AnyClass::get(c"NSNumber")?,
            AnyClass::get(c"NSMutableDictionary")?,
        );
        // SAFETY: the documented factories of NSNumber, NSMutableDictionary,
        // UIFontDescriptor and UIFont; the keys are the string values of
        // UIFontWeightTrait, UIFontDescriptorTraitsAttribute and
        // UIFontDescriptorFamilyAttribute.
        unsafe {
            let weight: Retained<AnyObject> = msg_send![numbers, numberWithDouble: weight];
            let traits: Retained<AnyObject> = msg_send![dictionaries, dictionary];
            let _: () = msg_send![&*traits, setObject: &*weight, forKey: &*ns_string("NSCTFontWeightTrait")];
            let attrs: Retained<AnyObject> = msg_send![dictionaries, dictionary];
            let _: () = msg_send![&*attrs, setObject: &*ns_string(family), forKey: &*ns_string("NSFontFamilyAttribute")];
            let _: () = msg_send![&*attrs, setObject: &*traits, forKey: &*ns_string("NSCTFontTraitsAttribute")];
            let descriptor: Retained<AnyObject> =
                msg_send![descriptors, fontDescriptorWithFontAttributes: &*attrs];
            msg_send![fonts, fontWithDescriptor: &*descriptor, size: size]
        }
    }

    /// The view's `tag` when it is a bottom strip (`edge: "bottom"`).
    const BOTTOM: isize = 1;

    impl SoftEdgesView {
        /// Sets the theme's font (the `font` prop) on the header titles of
        /// the page this view is on: its view controller's navigation item
        /// carries the appearances NSH styled, so the font joins their
        /// colors there.
        fn style_titles(&self) {
            let Some(family) = TITLE_FONT.with(|font| font.borrow().clone()) else {
                return;
            };
            let Some(controller_class) = AnyClass::get(c"UIViewController") else {
                return;
            };
            // SAFETY: UIResponder's `nextResponder` and NSObject's
            // `isKindOfClass:`, walking up to the page's view controller;
            // then UINavigationItem's documented appearance getters and
            // setters, UINavigationBarAppearance's title attributes (copied,
            // never mutated in place) and UIFont's descriptor factory.
            unsafe {
                let mut responder: Option<Retained<AnyObject>> = msg_send![self, nextResponder];
                let controller = loop {
                    let Some(next) = responder else { return };
                    let is_controller: bool = msg_send![&*next, isKindOfClass: controller_class];
                    if is_controller {
                        break next;
                    }
                    responder = msg_send![&*next, nextResponder];
                };
                let item: Retained<AnyObject> = msg_send![&*controller, navigationItem];
                let title = font(&family, 17.0, 0.3);
                let large = font(&family, 34.0, 0.4);
                let pairs = [
                    (sel!(standardAppearance), sel!(setStandardAppearance:)),
                    (sel!(scrollEdgeAppearance), sel!(setScrollEdgeAppearance:)),
                    (sel!(compactAppearance), sel!(setCompactAppearance:)),
                ];
                for (get, set) in pairs {
                    let appearance: Option<Retained<AnyObject>> =
                        msg_send![&*item, performSelector: get];
                    let Some(appearance) = appearance else {
                        continue;
                    };
                    let copy: Retained<AnyObject> = msg_send![&*appearance, copy];
                    for (attrs_get, attrs_set, font) in [
                        (
                            sel!(titleTextAttributes),
                            sel!(setTitleTextAttributes:),
                            &title,
                        ),
                        (
                            sel!(largeTitleTextAttributes),
                            sel!(setLargeTitleTextAttributes:),
                            &large,
                        ),
                    ] {
                        let Some(font) = font else { continue };
                        let attrs: Retained<AnyObject> =
                            msg_send![&*copy, performSelector: attrs_get];
                        let attrs: Retained<AnyObject> = msg_send![&*attrs, mutableCopy];
                        let _: () =
                            msg_send![&*attrs, setObject: &**font, forKey: &*ns_string("NSFont")];
                        let _: Option<Retained<AnyObject>> =
                            msg_send![&*copy, performSelector: attrs_set, withObject: &*attrs];
                    }
                    let _: Option<Retained<AnyObject>> =
                        msg_send![&*item, performSelector: set, withObject: &*copy];
                }
            }
        }

        /// A page that appears with its scroll view untouched rests at the
        /// very top, under the expanded large title: UIKit otherwise keeps
        /// the offset it set before the title was measured (the launch page
        /// opened collapsed).
        fn open_large_title(&self) {
            let Some(scroll) = self.page_scroll_view() else {
                return;
            };
            let Ok(scroll) = scroll.downcast::<UIScrollView>() else {
                return;
            };
            // The bar's own height first: a large title measured before its
            // page was laid out stays collapsed until the bar sizes again.
            if let Some(window) = self.window() {
                size_navigation_bars(&window);
            }
            let offset = scroll.contentOffset();
            let top = -scroll.adjustedContentInset().top;
            if !scroll.isDragging() && !scroll.isTracking() && offset.y <= 0.0 && offset.y > top {
                scroll.setContentOffset(CGPoint {
                    x: offset.x,
                    y: top,
                });
            }
        }

        /// The first scroll view of the page around this view: NSH wraps
        /// the view in a container of its own, so the page is the nearest
        /// ancestor holding one.
        fn page_scroll_view(&self) -> Option<Retained<UIView>> {
            let mut ancestor = self.superview();
            loop {
                let view = ancestor?;
                if let Some(scroll) = first_scroll_view(&view) {
                    return Some(scroll);
                }
                ancestor = view.superview();
            }
        }

        /// Registers this strip as a bottom edge element of the first
        /// scroll view beside it, so UIKit draws that scroll view's soft
        /// bottom edge behind it (iOS 26's
        /// `UIScrollEdgeElementContainerInteraction`).
        fn follow_bottom_edge(&self) {
            let Some(class) = AnyClass::get(c"UIScrollEdgeElementContainerInteraction") else {
                return;
            };
            let Some(scroll) = self.page_scroll_view() else {
                return;
            };
            // SAFETY: the interaction's designated initializer, then its
            // documented `scrollView` and `edge` setters (UIRectEdgeBottom
            // is 4), added to this view as any UIInteraction is.
            unsafe {
                let interaction: Retained<AnyObject> = msg_send![msg_send![class, alloc], init];
                let _: () = msg_send![&*interaction, setScrollView: &*scroll];
                let _: () = msg_send![&*interaction, setEdge: 4_usize];
                let _: () = msg_send![self, addInteraction: &*interaction];
            }
        }
    }

    /// The first scroll view under `view`, breadth first.
    fn first_scroll_view(view: &UIView) -> Option<Retained<UIView>> {
        let mut queue = std::collections::VecDeque::from([view.retain()]);
        while let Some(view) = queue.pop_front() {
            if view.respondsToSelector(sel!(topEdgeEffect)) {
                return Some(view);
            }
            queue.extend(view.subviews().iter());
        }
        None
    }

    /// Sizes every navigation bar under `view` to fit its large title.
    fn size_navigation_bars(view: &UIView) {
        let mut stack = vec![view.retain()];
        while let Some(view) = stack.pop() {
            if view.respondsToSelector(sel!(prefersLargeTitles)) {
                view.sizeToFit();
                continue;
            }
            stack.extend(view.subviews().iter());
        }
    }

    /// Sets the soft style on the top and bottom edge effects of every
    /// scroll view under `view`.
    fn soften(view: &UIView) {
        let Some(styles) = AnyClass::get(c"UIScrollEdgeEffectStyle") else {
            return;
        };
        // SAFETY: the documented class property, answering a style object.
        let soft: Option<Retained<AnyObject>> = unsafe { msg_send![styles, softStyle] };
        let Some(soft) = soft else { return };
        let mut stack = vec![view.retain()];
        while let Some(view) = stack.pop() {
            if view.respondsToSelector(sel!(topEdgeEffect)) {
                for getter in [sel!(topEdgeEffect), sel!(bottomEdgeEffect)] {
                    // SAFETY: UIScrollView's iOS 26 edge effect getters
                    // (checked above), answering a UIScrollEdgeEffect.
                    let effect: Option<Retained<AnyObject>> =
                        unsafe { msg_send![&*view, performSelector: getter] };
                    if let Some(effect) = effect {
                        // SAFETY: the effect's `style` setter, given a style.
                        let _: () = unsafe { msg_send![&*effect, setStyle: &*soft] };
                    }
                }
            }
            stack.extend(view.subviews().iter());
        }
    }

    struct SoftEdges {
        view: Retained<UIView>,
    }

    impl NativeComponent for SoftEdges {
        fn view(&self) -> &UIView {
            &self.view
        }

        fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
            if name == "font" {
                let family = value
                    .and_then(JsonValue::as_str)
                    .filter(|f| !f.is_empty())
                    .map(str::to_owned);
                TITLE_FONT.with(|font| *font.borrow_mut() = family);
            }
            if name == "edge" {
                let bottom = value.and_then(JsonValue::as_str) == Some("bottom");
                self.view.setTag(if bottom { BOTTOM } else { 0 });
            }
        }
    }

    /// Registers the soft edges view.
    pub fn register(components: &Components) {
        components.register(
            super::NAME,
            Rc::new(|_context: ComponentContext| -> Box<dyn NativeComponent> {
                let mtm = MainThreadMarker::new().expect("components are made on the main thread");
                // SAFETY: UIView's designated initializer.
                let view: Retained<SoftEdgesView> =
                    unsafe { msg_send![SoftEdgesView::alloc(mtm), init] };
                view.setUserInteractionEnabled(false);
                Box::new(SoftEdges {
                    view: Retained::into_super(view),
                })
            }),
        );
    }
}
