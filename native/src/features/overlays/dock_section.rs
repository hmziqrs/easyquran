//! The `DockArea` section: a small workspace — editor tabs in the center,
//! files in the left dock and a terminal below — whose tabs drag between
//! groups and whose docks toggle; the layout lives in the app's data.

use super::{ButtonVariant, caption, overlay_button, status_line};
use nsh_base::dock::{
    DockAreaApi, DockAreaEvent, DockAreaLayout, DockInfo, DockLayout, DockNode, DockPanel,
    DockPlacement, DockRegion, DockTab, PanelId,
};
use nsh_base::{DockArea, dock_area, theme};
use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

/// What the panels are called in tabs and the status line.
fn title(panel: &str) -> &'static str {
    match panel {
        "files" => "Files",
        "readme" => "Readme",
        "main" => "main.rs",
        "preview" => "Preview",
        "terminal" => "Terminal",
        _ => "…",
    }
}

/// The workspace at first: two editor groups side by side, files on the
/// left, a terminal below.
fn initial_layout() -> DockAreaLayout {
    DockAreaLayout::new(
        DockLayout::h_split()
            .child(
                DockLayout::tabs()
                    .panel("readme")
                    .panel("main")
                    .active_index(1),
                None,
            )
            .child(DockLayout::tabs().panel("preview"), None),
    )
    .with_dock(DockPlacement::Left, {
        let mut region = DockRegion::new(DockLayout::tabs().panel("files"));
        region.size = Some(px(200.0));
        region
    })
    .with_dock(DockPlacement::Bottom, {
        let mut region = DockRegion::new(DockLayout::tabs().panel("terminal"));
        region.size = Some(px(160.0));
        region
    })
}

/// A layout as one line: `row(...)` / `column(...)` for splits, `[a, b]`
/// for tab groups, so tests and people can read the shape.
fn describe(node: &DockNode) -> String {
    match node {
        DockNode::Tabs { panels, .. } => {
            let names: Vec<&str> = panels.iter().map(|p| title(p.as_str())).collect();
            format!("[{}]", names.join(", "))
        }
        DockNode::Split { axis, children, .. } => {
            let parts: Vec<String> = children.iter().map(describe).collect();
            format!(
                "{}({})",
                if *axis == Axis::Horizontal {
                    "row"
                } else {
                    "column"
                },
                parts.join(" ")
            )
        }
    }
}

/// The panel bodies, keyed by their id, so content keeps its state
/// wherever the panel moves.
fn panel_content_owned(panel: String) -> impl IntoElement {
    let panel = panel.as_str();
    match panel {
        "files" => view().flex_1().flex_col().gap_1().p_2().children(
            ["src/", "src/app.rs", "src/lib.rs", "Cargo.toml"]
                .iter()
                .map(|name| {
                    view()
                        .h_7()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .rounded(move || theme().radius)
                        .hover(|s| s.bg(move || theme().accent))
                        .child(
                            text((*name).to_owned())
                                .text_xs()
                                .font_family(crate::ui::text::mono_family)
                                .text_color(move || theme().foreground),
                        )
                        .test_id("dock-file-row")
                })
                .collect::<Vec<_>>(),
        ),
        "terminal" => view()
            .flex_1()
            .flex_col()
            .gap_1()
            .p_3()
            .child(
                text("$ cargo test")
                    .text_xs()
                    .font_family(crate::ui::text::mono_family)
                    .text_color(move || theme().foreground),
            )
            .child(
                text("   Compiling nsh-starter v0.1.0")
                    .text_xs()
                    .font_family(crate::ui::text::mono_family)
                    .text_color(move || theme().muted_foreground),
            )
            .child(
                text("        Finished in 1.9s")
                    .text_xs()
                    .font_family(crate::ui::text::mono_family)
                    .text_color(move || theme().primary),
            ),
        "preview" => view()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_1()
            .child(
                text("Live preview")
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(move || theme().foreground),
            )
            .child(
                text("Follows the editor beside it")
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
            ),
        _ => view()
            .flex_1()
            .flex_col()
            .gap_1()
            .p_4()
            .child(
                text(match panel {
                    "readme" => "# Workspace".to_owned(),
                    _ => "fn main() { run(app); }".to_owned(),
                })
                .text_xs()
                .font_family(crate::ui::text::mono_family)
                .text_color(move || theme().foreground),
            )
            .child(
                text("A dockable panel; drag its tab onto another group.")
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
            ),
    }
}

/// `DockArea` with draggable docked panels; the toolbar's buttons drive
/// the area's own methods, and every edit lands back in `layout`.
pub(crate) fn dock_section() -> impl IntoElement {
    let layout = signal(initial_layout());
    let status = signal(String::new());
    let area = component_ref::<DockArea>();
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    overlay_button("Reset layout", ButtonVariant::Outline, {
                                                move || layout.set(initial_layout())
                    }),
                )
                .child(
                    overlay_button("Toggle files", ButtonVariant::Ghost, {
                                                move || {
                            if let Some(api) = area.api::<DockAreaApi>() {
                                api.toggle_dock(DockPlacement::Left);
                            }
                        }
                    }),
                )
                .child(
                    overlay_button("Toggle terminal", ButtonVariant::Ghost, {
                                                move || {
                            if let Some(api) = area.api::<DockAreaApi>() {
                                api.toggle_dock(DockPlacement::Bottom);
                            }
                        }
                    }),
                ),
        )
        .child(caption(
            "Drag a tab onto another group or a group's edge; the line below is the layout as data, updated by every edit and by Reset.",
        ))
        .child(
            view()
                .flex_1()
                .h(px(420.0))
                .flex_col()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().card)
                .overflow_hidden()
                .child(
                    dock_area()
                        .component_ref(&area)
                        .flex_1()
                        .layout(move || Some(layout.get()))
                        .on_layout_change({
                                                                                    move |next: &DockAreaLayout| {
                                status.set(describe(&next.center));
                                layout.set(next.clone());
                            }
                        })
                        .on_panel_active_change({
                                                        move |e: &DockAreaEvent| {
                                if let DockAreaEvent::PanelActiveChange(panel, true) = e {
                                    status.set(format!("Showing {}", title(panel.as_str())));
                                }
                            }
                        })
                        .on_panel_remove({
                                                        move |panel: &PanelId| {
                                status.set(format!("Closed {}", title(panel.as_str())));
                            }
                        })
                        .handle_accessibility_label("Resize panels")
                        .handle_style(|s| s.bg(move || theme().border))
                        .handle_thumb_style(|s| {
                            s.h(px(20.0))
                                .w(px(3.0))
                                .rounded_full()
                                .bg(move || theme().muted_foreground)
                        })
                        .group_style(|s| s.flex_1().bg(move || theme().card))
                        .tab_bar_style(|s| {
                            s.flex_row()
                                .gap_1()
                                .border_b_1()
                                .border_color(move || theme().border)
                        })
                        .content_style(|s| s.flex_1().overflow_hidden())
                        .dock_style(|s| s.bg(move || theme().card))
                        .drawer_style(|s| {
                            s.border_r_1().border_color(move || theme().border).bg(move || theme().card)
                        })
                        .sheet_style(|s| {
                            s.rounded_t(move || theme().radius)
                                .border_t_1()
                                .border_color(move || theme().border)
                                .bg(move || theme().card)
                        })
                        .backdrop_style(super::scrim)
                        .render_tab(|tab: &DockTab| {
                            let name = title(tab.panel.as_str());
                            let active = tab.active;
                            view()
                                .h_9()
                                .flex_row()
                                .items_center()
                                .px_3()
                                .test_id(&format!("tab-{}", tab.panel.as_str()))
                                .child(
                                    text(name)
                                        .text_xs()
                                        .font_weight(if active {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::MEDIUM
                                        })
                                        .text_color(move || {
                                            if active {
                                                theme().foreground
                                            } else {
                                                theme().muted_foreground
                                            }
                                        }),
                                )
                                .child(
                                    view()
                                        .h(px(2.0))
                                        .w_full()
                                        .rounded_full()
                                        .bg(move || theme().primary)
                                        .opacity(move || if active { 1.0_f32 } else { 0.0_f32 }),
                                )
                        })
                        .render_panel(|panel: &DockPanel| {
                            let key = panel.id.as_str().to_owned();
                            panel_content_owned(key)
                        })
                        .render_dock(|dock: &DockInfo| {
                            view()
                                .h_8()
                                .flex_row()
                                .items_center()
                                .border_b_1()
                                .border_color(move || theme().border)
                                .px_2()
                                .child(
                                    text(title(dock_name(dock.placement)))
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(move || theme().muted_foreground),
                                )
                        })
                        .render_empty(|_: &DockPlacement| {
                            view().flex_1().items_center().justify_center().child(
                                text("Every panel is closed; Reset brings them back")
                                    .text_sm()
                                    .text_color(move || theme().muted_foreground),
                            )
                        }),
                ),
        )
        .child(status_line({ let s = status; move || s.get() }))
        .child(status_line(move || format!("Layout: {}", describe(&layout.get().center))))
}

/// A dock placement's key.
fn dock_name(placement: DockPlacement) -> &'static str {
    match placement {
        DockPlacement::Left => "files",
        DockPlacement::Bottom => "terminal",
        DockPlacement::Right => "right",
        DockPlacement::Center => "center",
    }
}
