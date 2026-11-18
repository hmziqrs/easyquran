//! The color picker: the area, the sliders and the field composed into
//! one panel, plus a swatch for a saved color.

use nsh_base::{
    ColorChannel, ColorFormat, ColorPickerEvent, ColorSpace, color_area, color_field, color_picker,
    color_slider, color_swatch, theme,
};
use nsh_core::prelude::*;

/// A picker panel: the saturation area over the hue slider and the
/// hex field, editing one color; every frame of a drag reports, the end
/// of the change reports once.
pub fn color_picker_of(
    value: Signal<Hsla>,
    on_change_end: impl Fn(Hsla) + 'static,
) -> impl IntoElement {
    color_picker()
        .value(move || Some(value.get()))
        .on_change(move |e: &ColorPickerEvent| {
            if let ColorPickerEvent::Change(color, _) = e {
                value.set(*color);
            }
        })
        .on_change_end(move |e: &ColorPickerEvent| {
            if let ColorPickerEvent::ChangeEnd(color, _) = e {
                on_change_end(*color);
            }
        })
        .flex_col()
        .gap_3()
        .child(
            color_area()
                .accessibility_label("Color area")
                .flex_1()
                .min_h(px(140.0))
                .rounded(move || theme().radius)
                .panel_class("rounded-md border border-border")
                .thumb_class("size-4 rounded-full border-2 border-background shadow-sm"),
        )
        .child(
            color_slider()
                .accessibility_label("Hue")
                .channel(ColorChannel::Hue)
                .color_space(ColorSpace::Hsb)
                .h(px(16.0))
                .justify_center()
                .track_style(|s| s.h(px(10.0)).rounded_full())
                .thumb_class("size-4 rounded-full border-2 border-background shadow-sm"),
        )
        .child(
            color_field()
                .accessibility_label("Hex")
                .format(ColorFormat::Hex)
                .h(px(36.0))
                .px_3()
                .text_sm()
                .text_color(move || theme().foreground),
        )
}

/// A saved color as a small swatch; `selected` draws the focus ring.
pub fn color_swatch_of(color: Hsla, selected: bool) -> impl IntoElement {
    color_swatch()
        .color(Some(color))
        .accessibility_label("Saved color")
        .size(px(28.0))
        .rounded_full()
        .selected_style(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .when(selected, |v| {
            v.border_2().border_color(move || theme().ring)
        })
}
