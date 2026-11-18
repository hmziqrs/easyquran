//! The system tab bar (`nsh/mobile`): iOS's UITabBar and Android's
//! navigation bar, in the system look. It takes no room on desktops and
//! the web, where the shell shows its tabs with `nsh/base`.

use nsh_core::mobile::{TabBarItem, tab_bar};
use nsh_core::prelude::*;

/// One tab of the mobile bar: a title and, per platform, an icon name (an
/// SF Symbol on iOS, a drawable on Android).
pub fn tab_item(title: &'static str, icon: Option<&'static str>) -> TabBarItem {
    let item = TabBarItem::new(title);
    match icon {
        Some(icon) => item.icon(icon),
        None => item,
    }
}

/// The mobile tab bar over `items`, the chosen one in `selected`; the
/// user's choice is reported and the caller shows that tab's page.
pub fn mobile_tab_bar(items: Vec<TabBarItem>, selected: Signal<u32>) -> impl IntoElement {
    tab_bar()
        .items(items)
        .selected_index(selected)
        .on_change(move |index: &u32| selected.set(*index))
}

/// One tab of [`themed_tab_bar`]: its label, its icon's SVG document (drawn
/// in `currentColor`) and the test id its pressable view carries.
pub struct ThemedTab {
    pub label: &'static str,
    pub svg: &'static str,
    pub test_id: &'static str,
}

/// A Material 3 navigation bar drawn in the theme's colors, for Android:
/// the system bar paints its own surface color and takes no app colors, so
/// it stood out from every page as a hard band. This one sits on the page's
/// background with a hairline above, reaches under the gesture bar, and
/// marks the selected tab with the primary color and a soft pill.
pub fn themed_tab_bar(tabs: Vec<ThemedTab>, selected: Signal<u32>) -> impl IntoElement {
    use nsh_base::theme;
    use nsh_core::element::ImageSrc;

    let mut bar = view()
        .w_full()
        .flex_row()
        .pt_1p5()
        .pb_safe_area_plus(px(6.0))
        .border_t_1()
        .border_color(move || theme().border.opacity(0.6))
        // Floats over the page, which shows faintly through it as it
        // scrolls under (Android has no backdrop blur to soften it more).
        .bg(move || theme().background.opacity(0.95));
    for (index, tab) in tabs.into_iter().enumerate() {
        let index = index as u32;
        let on = move || selected.get() == index;
        let color = move || {
            if on() {
                theme().primary
            } else {
                theme().muted_foreground
            }
        };
        bar = bar.child(
            view()
                .test_id(tab.test_id)
                .accessible(true)
                .label(tab.label)
                .flex_1()
                .items_center()
                .gap_1()
                .on_press(move || selected.set(index))
                .child(
                    view()
                        .w(px(56.0))
                        .h(px(30.0))
                        .rounded_full()
                        .items_center()
                        .justify_center()
                        .bg(move || {
                            if on() {
                                theme().primary.opacity(0.14)
                            } else {
                                theme().primary.opacity(0.0)
                            }
                        })
                        .child(
                            svg(ImageSrc::svg_markup(tab.svg))
                                .size(px(22.0))
                                .text_color(color),
                        ),
                )
                .child(
                    text(tab.label)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .number_of_lines(1)
                        .text_color(color),
                ),
        );
    }
    bar
}
