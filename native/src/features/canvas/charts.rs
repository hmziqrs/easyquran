//! The `nsh/base/plot` charts: a line with an area and a grid over one
//! series, and grouped bars over two, both from rows generated from a
//! seed; "New data" sets new rows and the plots move to the new domains.

use nsh_base::plot::{
    AxisPlacement, BandScale, BarCornerRadii, Curve, Scale, area, bar, grid, line, plot, plot_axis,
};
use nsh_base::theme;
use nsh_core::{
    element::{RowAccessor, Rows},
    prelude::*,
};

use super::{caption, section, series_tint, tool_button};

/// The days both charts show.
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// A band scale of [`DAYS`], with the gaps of grouped bars, built from
/// `default()` because the struct is `#[non_exhaustive]`.
fn band_scale() -> Scale {
    let mut band = BandScale::default();
    band.domain = Some(DAYS.iter().map(|day| SharedString::from(*day)).collect());
    band.padding_inner = percentage(0.3);
    band.padding_outer = percentage(0.15);
    band.group_padding = percentage(0.1);
    Scale::Band(band)
}

/// A small seeded generator (mulberry32), so the fictional data is stable
/// per seed and the same on every run.
fn seeded(seed: u32) -> impl FnMut() -> f64 {
    let mut s = seed;
    move || {
        s = s.wrapping_add(0x6d2b_79f5);
        let mut t = s;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4_294_967_296.0
    }
}

/// A week of sessions, `seed`-shaped: a weekday-heavy curve with noise.
fn sessions(seed: u32) -> Vec<f64> {
    let mut rand = seeded(seed);
    DAYS.iter()
        .enumerate()
        .map(|(day, _)| {
            let weekday = if day >= 5 { 0.45 } else { 1.0 };
            (900.0 + 1900.0 * rand() * weekday).round()
        })
        .collect()
}

/// A week of orders per channel, `seed`-shaped.
fn orders(seed: u32) -> (Vec<f64>, Vec<f64>) {
    let mut rand = seeded(seed.wrapping_add(11));
    let web = DAYS
        .iter()
        .map(|_| (120.0 + 340.0 * rand()).round())
        .collect();
    let store = DAYS
        .iter()
        .map(|_| (60.0 + 260.0 * rand()).round())
        .collect();
    (web, store)
}

/// One legend entry: a swatch of the series' color and its name.
fn legend_entry(tint: usize, label: &'static str) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            view()
                .size(px(10.0))
                .rounded(px(3.0))
                .bg(move || series_tint(tint)),
        )
        .child(
            text(label)
                .text_xs()
                .text_color(move || theme().muted_foreground),
        )
}

/// A legend row: the marks' colors and names, in the marks' order.
fn legend(entries: &[(usize, &'static str)]) -> impl IntoElement {
    view().flex_row().flex_wrap().gap_3().children(
        entries
            .iter()
            .copied()
            .map(|(tint, label)| legend_entry(tint, label)),
    )
}

/// The line and the bar charts of the same generated week.
#[component]
pub(crate) fn Charts() -> impl IntoElement {
    let seed = signal(7_u32);
    let line_rows = computed(move || {
        let values = sessions(seed.get());
        Rows::from_columns(values.len(), [("sessions".into(), values)])
    });
    let bar_rows = computed(move || {
        let (web, store) = orders(seed.get());
        Rows::from_columns(DAYS.len(), [("web".into(), web), ("store".into(), store)])
    });
    let line_readout = computed(move || {
        let total: f64 = line_rows.with(|rows| rows.column("sessions").unwrap_or(&[]).iter().sum());
        format!("batch #{} · {} sessions", seed.get(), total as u32)
    });
    let store_readout = computed(move || {
        let total: f64 = bar_rows.with(|rows| rows.column("store").unwrap_or(&[]).iter().sum());
        format!("batch #{} · {} store orders", seed.get(), total as u32)
    });
    view().child(section(
        "Charts",
        "nsh/base/plot · line and bar from generated rows",
        view()
            .gap_4()
            .child(
                view()
                    .gap_2()
                    .child(
                        plot()
                            .data(line_rows)
                            .y(RowAccessor::key("sessions"))
                            .accessibility_label("Sessions per day")
                            .w_full()
                            .h(px(150.0))
                            .child(
                                grid()
                                    .y(vec![0.0, 1500.0, 3000.0])
                                    .stroke_color(move || theme().border),
                            )
                            .child(
                                area()
                                    .curve(Curve::Natural)
                                    .fill_color(move || theme().chart_1.opacity(0.15)),
                            )
                            .child(
                                line()
                                    .curve(Curve::Natural)
                                    .stroke_color(move || theme().chart_1)
                                    .stroke_width(px(2.0)),
                            ),
                    )
                    .child(legend(&[(0, "Sessions")]))
                    .child(caption("line-total", move || line_readout.get())),
            )
            .child(
                view()
                    .gap_2()
                    .child(
                        plot()
                            .data(bar_rows)
                            .x_scale(Some(band_scale()))
                            .y_scale(Some(Scale::Linear {
                                domain: Some([0.0, 500.0]),
                                reverse: false,
                            }))
                            .accessibility_label("Orders per day and channel")
                            .w_full()
                            .h(px(150.0))
                            .child(
                                grid()
                                    .y(vec![0.0, 250.0, 500.0])
                                    .stroke_color(move || theme().border),
                            )
                            .child(plot_axis().placement(AxisPlacement::Bottom))
                            .child(
                                bar()
                                    .value(RowAccessor::key("web"))
                                    .fill_color(move || theme().chart_2)
                                    .corner_radii(BarCornerRadii::all(px(3.0))),
                            )
                            .child(
                                bar()
                                    .value(RowAccessor::key("store"))
                                    .fill_color(move || theme().chart_3)
                                    .corner_radii(BarCornerRadii::all(px(3.0))),
                            ),
                    )
                    .child(legend(&[(1, "Web"), (2, "Store")]))
                    .child(caption("bar-total", move || store_readout.get())),
            )
            .child(tool_button("New data", false, move || {
                seed.update(|s| *s += 1)
            })),
    ))
}
