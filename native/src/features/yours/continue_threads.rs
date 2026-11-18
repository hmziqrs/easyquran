//! Continue (web `ContinueThreads.svelte`, PARITY G3, G2): every open
//! reading thread, newest first. The first leads as the big card (verse of
//! total, progress, Continue), the rest are one-tap rows. A thread last
//! moved on another device says so ("on your phone · 5 min ago", or the
//! device's own name: contracts/devices.md D5).

use eq_core::{
    i18n::keys,
    store::sessions::{self, DeviceKind, HandoffPlace, HereDevice, ReadingThread},
};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    features::bookmarks::{fmt, now_ms, surah_name},
    stores::{
        locale::{t, time_ago, tr},
        sync::Area,
    },
    ui::{
        button::{
            ButtonSize, ButtonVariant, IconButtonSize, IconButtonVariant, button_with,
            icon_button_with,
        },
        controls::progress,
        icons::lucide,
        layout::{card, pressable_card},
        text::body,
        tokens,
    },
};

const X_ICON: &str = lucide!(r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#);
const BOOK_ICON: &str =
    lucide!(r#"<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/>"#);
/// Continue shows this many threads.
const LIMIT: usize = 5;

/// Open threads, this device, and the sessions domain's sync counts.
#[derive(Clone, PartialEq, Default)]
pub(crate) struct Threads {
    pub open: Vec<ReadingThread>,
    pub here: Option<HereDevice>,
    pub rejected: usize,
    pub capped: usize,
}

/// Re-reads whenever the sessions area changes.
pub(crate) fn threads() -> Signal<Threads> {
    let out = signal(Threads::default());
    let sync = crate::features::bookmarks::store();
    effect(move || {
        let _ = sync.version(Area::Sessions);
        if !sync.ready() {
            return;
        }
        untracked(|| {
            sync.run(
                |store| {
                    Ok(Threads {
                        open: sessions::open(store)?,
                        here: Some(sessions::here(store)),
                        rejected: sessions::rejected_count(store)?,
                        capped: sessions::capped_count(store),
                    })
                },
                move |r| match r {
                    Ok(next) => {
                        let _ = out.try_set(next);
                    }
                    Err(e) => log::warn!("threads read failed: {e}"),
                },
            );
        });
    });
    out
}

fn device_label(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Phone => tr(keys::READER_THREADS_ON_PHONE),
        DeviceKind::Tablet => tr(keys::READER_THREADS_ON_TABLET),
        DeviceKind::Desktop => tr(keys::READER_THREADS_ON_DESKTOP),
    }
}

/// "5 min. ago", or "on your phone · 5 min. ago" / "on Pixel · …".
pub(crate) fn where_label(thread: &ReadingThread, here: Option<&HereDevice>) -> String {
    let ago = time_ago(&thread.updated_at, now_ms());
    let Some(here) = here else {
        return ago;
    };
    match sessions::handoff_place(thread, here) {
        HandoffPlace::Here => ago,
        HandoffPlace::Named(name) => format!(
            "{} · {ago}",
            fmt(keys::READER_THREADS_ON_DEVICE, &[("name", name.into())])
        ),
        HandoffPlace::Device(kind) => format!("{} · {ago}", device_label(kind)),
    }
}

/// Opens the reader where the thread stands, in its translation (else
/// Arabic).
pub(crate) fn resume(thread: &ReadingThread) {
    crate::features::bookmarks::open_verse(thread.surah, thread.ayah, thread.source_id.as_deref());
}

fn dismiss(id: String) {
    crate::features::bookmarks::store().write(
        Area::Sessions,
        move |s| sessions::dismiss(s, &id),
        |r| {
            if let Err(e) = r {
                log::warn!("dismiss failed: {e}");
            }
        },
    );
}

fn dismiss_button(id: impl Fn() -> String + 'static) -> impl IntoElement {
    icon_button_with(
        X_ICON,
        tr(keys::READER_THREADS_DISMISS),
        IconButtonVariant::Ghost,
        IconButtonSize::Small,
        move || dismiss(id()),
    )
}

fn lead_card(
    data: Signal<Threads>,
    lead: impl Fn() -> Option<ReadingThread> + Copy + 'static,
) -> impl IntoElement {
    let total = move || lead().map_or(0, |t| crate::features::bookmarks::ayah_count(t.surah));
    let detail = move || {
        let Some(thread) = lead() else {
            return SharedString::default();
        };
        let n = total();
        let place = data.with(|d| where_label(&thread, d.here.as_ref()));
        let verse = if n > 0 {
            fmt(
                keys::READER_THREADS_VERSE_OF,
                &[("ayah", thread.ayah.into()), ("total", n.into())],
            )
        } else {
            format!("{}:{}", thread.surah, thread.ayah)
        };
        SharedString::from(format!("{verse} · {place}"))
    };
    card()
        .child(
            view()
                .flex_row()
                .items_start()
                .gap_3()
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_col()
                        .gap_1()
                        .child(
                            text(move || {
                                SharedString::from(
                                    lead().map(|t| surah_name(t.surah)).unwrap_or_default(),
                                )
                            })
                            .text_xl()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .text_color(move || theme().foreground),
                        )
                        .child(
                            text(detail)
                                .text_sm()
                                .text_color(tokens::foreground_secondary),
                        ),
                )
                .child(dismiss_button(move || {
                    lead().map(|t| t.id).unwrap_or_default()
                })),
        )
        .child(when(
            move || total() > 0,
            move || {
                progress(move || {
                    let n = total();
                    lead().map_or(0.0, |t| (t.ayah as f32 / n.max(1) as f32).min(1.0))
                })
            },
        ))
        .child(view().flex_row().child(button_with(
            t(keys::READER_JUMP),
            ButtonVariant::Primary,
            ButtonSize::Medium,
            false,
            None,
            move || {
                if let Some(thread) = lead() {
                    resume(&thread);
                }
            },
        )))
}

fn rest_row(data: Signal<Threads>, item: ReadSignal<ReadingThread>) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py(px(10.0))
        .border_t_1()
        .border_color(move || theme().border)
        .child(
            pressable_card("", move || item.with_untracked(resume))
                .accessibility_label(move || {
                    SharedString::from(
                        item.with(|t| format!("{} {}:{}", surah_name(t.surah), t.surah, t.ayah)),
                    )
                })
                .flex_1()
                .min_w(px(0.0))
                .gap_0p5()
                .p_0()
                .border_none()
                .bg(AdaptiveColor::TRANSPARENT)
                .child(
                    text(move || {
                        SharedString::from(
                            item.with(|t| {
                                format!("{} {}:{}", surah_name(t.surah), t.surah, t.ayah)
                            }),
                        )
                    })
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .text_color(move || theme().foreground),
                )
                .child(
                    text(move || {
                        SharedString::from(
                            item.with(|t| data.with(|d| where_label(t, d.here.as_ref()))),
                        )
                    })
                    .text_xs()
                    .text_color(tokens::muted_text),
                ),
        )
        .child(dismiss_button(move || {
            item.with_untracked(|t| t.id.clone())
        }))
}

/// The Continue section's body.
pub(crate) fn continue_threads() -> impl IntoElement {
    let data = threads();
    let lead = move || data.with(|d| d.open.first().cloned());
    let rest = move || {
        data.with(|d| {
            d.open
                .iter()
                .take(LIMIT)
                .skip(1)
                .cloned()
                .collect::<Vec<_>>()
        })
    };
    let rejected = move || data.with(|d| d.rejected);
    let capped = move || data.with(|d| d.capped);
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(when(
            move || crate::features::bookmarks::store().status().degraded,
            || {
                text(t(keys::READER_THREADS_SYNC_ERROR))
                    .text_sm()
                    .text_color(tokens::danger)
            },
        ))
        .child(when(
            move || rejected() > 0,
            move || {
                text(move || {
                    SharedString::from(fmt(
                        keys::READER_THREADS_ROWS_REJECTED,
                        &[("count", rejected().into())],
                    ))
                })
                .text_sm()
                .text_color(tokens::danger)
            },
        ))
        .child(when(
            move || capped() > 0,
            move || {
                text(move || {
                    SharedString::from(fmt(
                        keys::READER_THREADS_ROWS_CAPPED,
                        &[("count", capped().into())],
                    ))
                })
                .text_sm()
                .text_color(tokens::muted_text)
            },
        ))
        .child(
            when(
                move || lead().is_none(),
                || {
                    card()
                        .flex_row()
                        .items_center()
                        .gap(px(14.0))
                        .child(
                            view()
                                .size(px(44.0))
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .rounded(tokens::radius_sm)
                                .bg(move || tokens::hue_soft(2))
                                .child(
                                    svg(nsh_core::element::ImageSrc::svg_markup(BOOK_ICON))
                                        .size(px(20.0))
                                        .text_color(move || tokens::hue_legible(2)),
                                ),
                        )
                        .child(view().flex_1().child(body(t(keys::READER_THREADS_EMPTY))))
                },
            )
            .otherwise(move || {
                view()
                    .w_full()
                    .flex_col()
                    .gap_0()
                    .child(lead_card(data, lead))
                    .child(each(
                        rest,
                        |t| t.id.clone(),
                        move |item, _| rest_row(data, item),
                    ))
            }),
        )
}
