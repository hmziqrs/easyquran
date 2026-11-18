//! History (web `ThreadHistory.svelte` + the archive half of the sessions
//! store, PARITY G3): finished, restarted, removed and paused (30 days
//! untouched) threads, kept and never shown in Continue. Signed in, pages
//! come from `GET /reading-session/v1/archive`; signed out, from this
//! device. "Clear all" reads the whole history first (nothing is cleared
//! when it cannot), then offers Undo.

use eq_core::{
    i18n::keys,
    store::sessions::{self, ARCHIVE_PAGE, HistoryClear, ReadingThread},
    sync::transport::SyncGet,
};
use nsh_base::theme;
use nsh_core::prelude::*;

use super::continue_threads::resume;
use crate::{
    app::auth_store,
    features::bookmarks::{now_ms, offer, surah_name},
    stores::{
        locale::{t, time_ago, tr},
        sync::Area,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{BadgeVariant, badge_of, pressable_card},
        popups::{destructive_menu_entry, dropdown_menu},
        text::muted,
        toast::notify,
        tokens,
    },
};

/// Pages read for "Clear all" before giving up.
const MAX_ARCHIVE_PAGES: usize = 50;

/// What the section shows (`ArchiveView`).
#[derive(Clone, PartialEq, Default)]
struct Archive {
    items: Vec<ReadingThread>,
    next: Option<String>,
    loading: bool,
    loaded: bool,
}

fn signed_in_id() -> Option<i32> {
    auth_store().user().map(|u| u.id)
}

/// One archive page (signed in) or the device's history (signed out).
/// Resolves with the archive as it then stands.
async fn load(archive: Signal<Archive>, reset: bool) {
    let prev = archive.get_untracked();
    if prev.loading || (!reset && prev.loaded && prev.next.is_none()) {
        return;
    }
    let owner = untracked(signed_in_id);
    let Some(_) = owner else {
        let (tx, rx) = futures_channel();
        crate::features::bookmarks::store().run(
            |s| sessions::local_history(s),
            move |r| {
                let _ = tx.send(r);
            },
        );
        let mut items = rx.await.and_then(Result::ok).unwrap_or_default();
        items.sort_by(sessions::by_newest);
        let _ = archive.try_set(Archive {
            items,
            next: None,
            loading: false,
            loaded: true,
        });
        return;
    };
    let cursor = if reset { None } else { prev.next.clone() };
    let _ = archive.try_set(Archive {
        items: if reset {
            Vec::new()
        } else {
            prev.items.clone()
        },
        loading: true,
        ..prev
    });
    let Some(client) = untracked(|| auth_store().client()) else {
        archive.update(|a| a.loading = false);
        return;
    };
    let path = sessions::archive_path(cursor.as_deref(), ARCHIVE_PAGE);
    let page = client
        .get_json(&path)
        .await
        .ok()
        .and_then(|v| sessions::decode_archive_page(&v));
    if untracked(signed_in_id) != owner {
        return;
    }
    let Some(page) = page else {
        if archive.try_get().is_some() {
            archive.update(|a| a.loading = false);
        }
        return;
    };
    if archive.try_get().is_none() {
        return;
    }
    archive.update(|a| {
        let fresh: Vec<ReadingThread> = page
            .sessions
            .into_iter()
            .filter(|t| !a.items.iter().any(|x| x.id == t.id))
            .collect();
        a.items.extend(fresh);
        a.next = page.next;
        a.loading = false;
        a.loaded = true;
    });
}

/// A one-shot channel over a signal-free cell, for awaiting a store read.
fn futures_channel<T: 'static>() -> (Sender<T>, Receiver<T>) {
    let cell = std::rc::Rc::new(std::cell::RefCell::new(None::<T>));
    let waker = std::rc::Rc::new(std::cell::RefCell::new(None::<std::task::Waker>));
    (
        Sender {
            cell: cell.clone(),
            waker: waker.clone(),
        },
        Receiver { cell, waker },
    )
}

struct Sender<T> {
    cell: std::rc::Rc<std::cell::RefCell<Option<T>>>,
    waker: std::rc::Rc<std::cell::RefCell<Option<std::task::Waker>>>,
}

impl<T> Sender<T> {
    fn send(self, value: T) -> Result<(), ()> {
        *self.cell.borrow_mut() = Some(value);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake();
        }
        Ok(())
    }
}

struct Receiver<T> {
    cell: std::rc::Rc<std::cell::RefCell<Option<T>>>,
    waker: std::rc::Rc<std::cell::RefCell<Option<std::task::Waker>>>,
}

impl<T> std::future::Future for Receiver<T> {
    type Output = Option<T>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<T>> {
        if let Some(v) = self.cell.borrow_mut().take() {
            return std::task::Poll::Ready(Some(v));
        }
        *self.waker.borrow_mut() = Some(cx.waker().clone());
        std::task::Poll::Pending
    }
}

/// Every page, stopping early if the server stops advancing.
async fn load_whole(archive: Signal<Archive>) {
    load(archive, true).await;
    for _ in 0..MAX_ARCHIVE_PAGES {
        let (loaded, cursor) = archive.with_untracked(|a| (a.loaded, a.next.clone()));
        let Some(cursor) = cursor.filter(|_| loaded) else {
            return;
        };
        load(archive, false).await;
        if archive.with_untracked(|a| a.next.as_deref() == Some(cursor.as_str())) {
            return;
        }
    }
}

fn clear_all(archive: Signal<Archive>) {
    spawn(async move {
        load_whole(archive).await;
        let (loaded, complete, items) =
            archive.with_untracked(|a| (a.loaded, a.next.is_none(), a.items.clone()));
        if !loaded || !complete {
            notify(tr(keys::READER_THREADS_CLEAR_FAILED));
            return;
        }
        crate::features::bookmarks::store().write(
            Area::Sessions,
            move |s| sessions::clear_history(s, Some(&items)),
            move |r| match r {
                Ok(Some(cleared)) => {
                    let _ = archive.try_set(Archive {
                        loaded: true,
                        ..Archive::default()
                    });
                    offer(
                        tr(keys::READER_THREADS_CLEARED),
                        tr(keys::READER_BOOKMARKS_UNDO),
                        move || restore(archive, cleared.clone()),
                    );
                }
                _ => notify(tr(keys::READER_THREADS_CLEAR_FAILED)),
            },
        );
    });
}

/// Undo: the threads back, and the view back to exactly that snapshot.
fn restore(archive: Signal<Archive>, cleared: HistoryClear) {
    let mut items = cleared.threads.clone();
    crate::features::bookmarks::store().write(
        Area::Sessions,
        move |s| sessions::restore_history(s, &cleared),
        move |r| {
            if r.is_err() {
                return;
            }
            items.sort_by(sessions::by_newest);
            let _ = archive.try_set(Archive {
                items,
                next: None,
                loading: false,
                loaded: true,
            });
        },
    );
}

fn remove(archive: Signal<Archive>, thread: ReadingThread) {
    archive.update(|a| a.items.retain(|t| t.id != thread.id));
    crate::features::bookmarks::store().write(
        Area::Sessions,
        move |s| sessions::remove(s, &thread.id, Some(&thread)),
        |r| {
            if let Err(e) = r {
                log::warn!("history remove failed: {e}");
            }
        },
    );
}

fn status_badge(item: ReadSignal<ReadingThread>) -> impl IntoElement {
    dynamic(move || {
        if item.with(|t| t.end_reason.as_deref() == Some("completed")) {
            return badge_of(t(keys::READER_THREADS_COMPLETED), BadgeVariant::Default);
        }
        badge_of(t(keys::READER_THREADS_PAUSED), BadgeVariant::Secondary)
    })
}

fn history_row(archive: Signal<Archive>, item: ReadSignal<ReadingThread>) -> impl IntoElement {
    let title = move || item.with(|t| format!("{} {}:{}", surah_name(t.surah), t.surah, t.ayah));
    view()
        .w_full()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py(px(10.0))
        .border_b_1()
        .border_color(move || theme().border)
        .child(
            pressable_card("", move || item.with_untracked(resume))
                .accessibility_label(move || SharedString::from(title()))
                .flex_1()
                .min_w(px(0.0))
                .gap_0p5()
                .p_0()
                .border_none()
                .bg(AdaptiveColor::TRANSPARENT)
                .child(
                    text(move || SharedString::from(title()))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .text_color(move || theme().foreground),
                )
                .child(
                    text(move || {
                        SharedString::from(item.with(|t| time_ago(&t.updated_at, now_ms())))
                    })
                    .text_xs()
                    .text_color(tokens::muted_text),
                ),
        )
        .child(status_badge(item))
        .child(dropdown_menu(
            "⋯",
            [destructive_menu_entry(
                t(keys::READER_THREADS_DELETE),
                move || {
                    remove(archive, item.get_untracked());
                },
            )],
        ))
}

/// The History section's body; `limit` rows until "Show more".
pub(crate) fn thread_history(limit: usize) -> impl IntoElement {
    let archive = signal(Archive::default());
    let expanded = signal(false);
    // First page now, and again when the account changes.
    effect(move || {
        let _ = signed_in_id();
        spawn(async move { load(archive, true).await });
    });
    let items = move || {
        archive.with(|a| {
            if expanded.get() {
                return a.items.clone();
            }
            a.items.iter().take(limit).cloned().collect::<Vec<_>>()
        })
    };
    let can_show_more = move || {
        archive.with(|a| {
            let more_here = !expanded.get() && a.items.len() > limit;
            let more_there = expanded.get() && a.next.is_some();
            more_here || more_there
        })
    };
    let show_more = move || {
        if !expanded.get_untracked() {
            expanded.set(true);
            return;
        }
        spawn(async move { load(archive, false).await });
    };
    let loading = move || archive.with(|a| a.loading);
    dynamic(move || {
        let (loaded, empty) = archive.with(|a| (a.loaded, a.items.is_empty()));
        if loaded && empty {
            return Some(muted(t(keys::READER_THREADS_HISTORY_EMPTY)).into_element());
        }
        if empty {
            return None;
        }
        Some(
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(view().flex_row().justify_end().child(button_with(
                    t(keys::READER_THREADS_CLEAR_ALL),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    loading,
                    None,
                    move || clear_all(archive),
                )))
                .child(
                    view()
                        .w_full()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(tokens::radius_md)
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(tokens::surface)
                        .child(each(
                            items,
                            |t| t.id.clone(),
                            move |item, _| history_row(archive, item),
                        )),
                )
                .child(when(can_show_more, move || {
                    view().flex_row().child(button_with(
                        t(keys::READER_THREADS_SHOW_MORE),
                        ButtonVariant::Secondary,
                        ButtonSize::Small,
                        loading,
                        None,
                        show_more,
                    ))
                }))
                .into_element(),
        )
    })
}
