//! The command palette: searchable commands, grouped, keyboard first.
//! The shell mounts one; its items come from the command registry.

use nsh_base::{CommandEntry, CommandItem, command, theme};
use nsh_core::prelude::*;

/// One command of the palette: its stable id, its row and keywords.
pub fn command_entry(id: &'static str, label: &'static str) -> CommandItem {
    CommandItem::new(id, label)
}

/// The palette: type to filter `items`; choosing one reports its id.
pub fn command_palette(
    items: Vec<CommandItem>,
    placeholder: impl Into<String>,
    on_select: impl Fn(SharedString) + 'static,
) -> impl IntoElement {
    let placeholder = placeholder.into();
    command()
        .items(items)
        .searchable(true)
        .filterable(true)
        .auto_focus(true)
        .separator(true)
        .accessibility_label("Command palette")
        .placeholder(SharedString::from(placeholder.clone()))
        .search_bar_style(|s| s.p_2().border_b_1().border_color(move || theme().border))
        .search_style(|s| {
            s.h(px(36.0))
                .px_3()
                .text_sm()
                .text_color(move || theme().popover_foreground)
        })
        .list_style(|s| s.p_1().gap(px(2.0)))
        .item_class("rounded-md px-2 py-1.5 text-sm focus:bg-accent selected:font-medium")
        .group_style(|s| {
            s.pt_2()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().muted_foreground)
        })
        .separator_style(|s| s.h(px(1.0)).my_1().bg(move || theme().border))
        .render_item(|entry: &CommandEntry| {
            view().px_2().py_1p5().child(
                text(entry.label.clone())
                    .text_sm()
                    .text_color(move || theme().popover_foreground),
            )
        })
        .on_select(move |id: &SharedString| on_select(id.clone()))
}
