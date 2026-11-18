//! "Your reading place" (web `ReadingBookmarkCard.svelte`, PARITY F3): the
//! page the reader chose to come back to (`reading.bookmark`, synced with
//! the preferences), shown only while one is set.

use eq_core::{
    i18n::keys,
    store::preferences::{self, ReadingBookmark},
};
use nsh_core::prelude::*;

use crate::{
    features::bookmarks::{fmt, medium_date, open_verse, source_label, surah_name},
    stores::{locale::t, sync::Area},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{card, eyebrow},
        tokens,
    },
};

fn place() -> Signal<Option<ReadingBookmark>> {
    let out = signal(None);
    let sync = crate::features::bookmarks::store();
    effect(move || {
        let _ = sync.version(Area::Preferences);
        if !sync.ready() {
            return;
        }
        untracked(|| {
            sync.run(
                |s| preferences::reading_bookmark(s),
                move |r| match r {
                    Ok(next) => {
                        let _ = out.try_set(next);
                    }
                    Err(e) => log::warn!("reading bookmark read failed: {e}"),
                },
            );
        });
    });
    out
}

pub(crate) fn reading_bookmark_card() -> impl IntoElement {
    let place = place();
    when(
        move || place.get().is_some(),
        move || {
            let detail = move || {
                place.get().map_or_else(SharedString::default, |p| {
                    SharedString::from(fmt(
                        keys::READER_READING_BOOKMARK_DETAIL,
                        &[
                            ("surah", surah_name(p.surah).into()),
                            ("ayah", p.ayah.into()),
                        ],
                    ))
                })
            };
            let set_on = move || {
                place.get().map_or_else(SharedString::default, |p| {
                    SharedString::from(fmt(
                        keys::READER_READING_BOOKMARK_SET_ON,
                        &[
                            ("source", source_label(p.source_id.as_deref()).into()),
                            ("date", medium_date(p.set_at).into()),
                        ],
                    ))
                })
            };
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(eyebrow(t(keys::READER_READING_BOOKMARK_TITLE)))
                .child(
                    card()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            view()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex_col()
                                .gap_0p5()
                                .child(text(detail).text_sm().font_weight(FontWeight::MEDIUM))
                                .child(text(set_on).text_xs().text_color(tokens::muted_text)),
                        )
                        .child(button_with(
                            t(keys::READER_READING_BOOKMARK_OPEN),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            move || {
                                if let Some(p) = place.get_untracked() {
                                    open_verse(p.surah, p.ayah, p.source_id.as_deref());
                                }
                            },
                        )),
                )
        },
    )
}
