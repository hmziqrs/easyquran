//! The dock area: a workspace of dockable, tabbed panels. The app owns
//! the layout; the area's tabs and bars are styled here.

use nsh_base::dock::{DockAreaLayout, DockPanel, DockTab, dock_area};
use nsh_base::theme;
use nsh_core::prelude::*;

/// One workspace panel: its key names it in the layout, its title names
/// its tab, its content is drawn per placement.
pub struct DockPanelSpec {
    /// The panel's key in the layout.
    pub key: &'static str,
    /// What its tab says.
    pub title: &'static str,
    /// What the panel shows; made again when the panel moves, so state
    /// that must survive belongs to the caller.
    pub content: SharedString,
}

/// A dock area over `panels`, its layout in the app's hands (`layout`
/// holds what `on_layout_change` reported, saved with settings).
pub fn dock_workspace(
    panels: Vec<DockPanelSpec>,
    layout: Signal<DockAreaLayout>,
) -> nsh_base::DockAreaElement {
    let keys = panels
        .iter()
        .map(|panel| (panel.key.to_string(), panel.title.to_string()))
        .collect::<Vec<_>>();
    let contents = panels
        .iter()
        .map(|panel| (panel.key.to_string(), panel.content.clone()))
        .collect::<Vec<_>>();
    dock_area()
        .layout(move || layout.get())
        .on_layout_change(move |next: &DockAreaLayout| layout.set(next.clone()))
        .size_full()
        .tab_bar_class("flex-row items-center gap-1 px-2 h-9 border-b border-border bg-muted")
        .tab_class(
            "px-3 h-full flex-row items-center justify-center rounded-t-md text-xs \
             text-muted-foreground hover:text-foreground selected:bg-card \
             selected:text-foreground selected:font-medium",
        )
        // The drag boundary is a hairline; the thumb appears on hover.
        .handle_class("w-1 bg-border group-hover:bg-ring group-active:bg-ring")
        .handle_thumb_class(
            "h-8 w-1 rounded-full bg-muted-foreground opacity-0 group-hover:opacity-60 \
             transition-opacity",
        )
        .render_tab(move |tab: &DockTab| {
            let key = tab.panel.to_string();
            let title = keys
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, title)| title.clone())
                .unwrap_or_else(|| key.clone());
            // The displayed tab lifts onto the card: an active fill
            // behind the text, which `tab_class` states also carry.
            let active = tab.active;
            view()
                .px_3()
                .h_full()
                .flex_row()
                .items_center()
                .justify_center()
                .rounded_t(move || theme().radius)
                .style_when(active, |s| s.bg(move || theme().card))
                .child(
                    text(title)
                        .text_xs()
                        .font_weight(if tab.active {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(move || theme().foreground),
                )
        })
        .render_panel(move |panel: &DockPanel| {
            let key = panel.id.to_string();
            let body = contents
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, body)| body.clone())
                .unwrap_or_default();
            view()
                .flex_1()
                .p_3()
                .bg(move || theme().card)
                .text_color(move || theme().card_foreground)
                .child(
                    text(body)
                        .text_sm()
                        .text_color(move || theme().card_foreground),
                )
        })
}
