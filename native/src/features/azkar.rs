//! Azkar list and zikr detail (PARITY H1; web `(application)/azkar/**`,
//! `t/[lang]/[translator]/azkar/[slug]`, `RecitePanel.svelte`,
//! `AddCardInline.svelte`, `lib/home/next-zikr.ts`). Contract:
//! `pub fn page() -> impl IntoElement`,
//! `pub fn zikr_page(slug: String, translation: Option<TranslationRef>) -> impl IntoElement`.
//!
//! The zikr page is a zikr session surface: it enters a bounded reading
//! thread (`snippet:<slug>`), and on the Arabic route a matching snippet
//! card turns it into a live counter (the recite panel) with the next card
//! to recite; with no card yet the inline form adds one. Snippet data is
//! references only; the verse text comes verbatim from the Quran source.

use eq_core::{
    home::{
        next_zikr::{bound_card_for, is_target_done, next_zikr_card},
        snippets::{Snippet, SnippetKind, SnippetText, snippet_for_slug, snippets},
    },
    i18n::keys,
    quran::{catalogue, meta::meta, types::ArabicSource},
    store::{
        AppStore, StoreError, bookmarks,
        sessions::{self, EntryIntent, ReaderTarget as ThreadTarget, ReadingThread},
    },
};
use nsh_base::theme;
use nsh_core::prelude::*;
use serde_json::{Value, json};

use crate::{
    app::quran_store,
    features::{
        bookmarks::store,
        reader::share::{range_link, range_share_text, range_title, share_feedback, share_or_copy},
        yours::cards::{card_link, count_line, open_link, recite_button, target_bar, tile_copy},
    },
    shell::{
        nav,
        routes::{ReaderLink, ReaderScope, ReaderTarget, Route, TranslationRef},
    },
    stores::{
        home::{self, card_id, home_data, now_minute},
        locale::{locale, t, t_args, tr},
        sync::Area,
    },
    ui::{
        arabic::{ArabicStyle, QuranScript, plain_ayah},
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{page_body, page_insets},
        text::{body, muted, page_heading},
        tokens,
    },
};

const TARGET_FIRST: i64 = 33;
const TARGET_MAX: i64 = 10_000;

fn localized(text: &SnippetText) -> String {
    if locale().code() == "ar" {
        return text.ar.clone();
    }
    text.en.clone()
}

/// `SNIPPET_KIND_LABELS`, `SNIPPET_INDEX_TITLE`, `SNIPPET_INDEX_DESCRIPTION`
/// (bilingual in the web's data, not message keys).
fn kind_label(kind: SnippetKind) -> SnippetText {
    let (en, ar) = match kind {
        SnippetKind::Essential => ("Essential verses", "آيات أساسية"),
        SnippetKind::Azkar => ("Azkar", "أذكار"),
        SnippetKind::Dua => ("Dua", "دعاء"),
    };
    SnippetText {
        en: en.into(),
        ar: ar.into(),
    }
}

fn index_title() -> SnippetText {
    SnippetText {
        en: "Azkar & essential verses".into(),
        ar: "أذكار وآيات مختارة".into(),
    }
}

fn index_description() -> SnippetText {
    SnippetText {
        en: "Curated verses and remembrances — Ayat al-Kursi, the closing of Al-Baqarah, and more, with any translation you read in.".into(),
        ar: "آيات وأذكار مختارة — آية الكرسي وخواتيم البقرة وغيرها، مع الترجمة التي تقرأ بها.".into(),
    }
}

fn open_snippet(slug: String) {
    home::set_card_entry(EntryIntent {
        origin: "snippet".into(),
        ..EntryIntent::default()
    });
    nav::push(Route::Zikr {
        slug,
        translation: None,
    });
}

fn snippet_row(sn: &'static Snippet) -> impl IntoElement {
    let slug = sn.slug.clone();
    crate::ui::layout::pressable_card(localized(&sn.title), move || open_snippet(slug.clone()))
        .gap_0p5()
        .child(
            text(localized(&sn.title))
                .text_sm()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(
            text(localized(&sn.description))
                .text_xs()
                .text_color(tokens::foreground_secondary),
        )
}

pub fn page() -> impl IntoElement {
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(
                view()
                    .w_full()
                    .flex_col()
                    .gap_2()
                    .child(page_heading(SharedString::from(localized(&index_title()))))
                    .child(body(localized(&index_description()))),
            )
            .children(SnippetKind::ORDER.into_iter().filter_map(|kind| {
                let group: Vec<&'static Snippet> =
                    snippets().iter().filter(|s| s.kind == kind).collect();
                (!group.is_empty()).then(|| {
                    view()
                        .w_full()
                        .flex_col()
                        .gap_3()
                        .child(
                            view()
                                .w_full()
                                .pb_2()
                                .border_b_1()
                                .border_color(tokens::reader_divider)
                                .child(
                                    text(localized(&kind_label(kind)))
                                        .text_base()
                                        .font_weight(FontWeight::MEDIUM),
                                ),
                        )
                        .children(group.into_iter().map(|s| snippet_row(s).into_element()))
                        .into_element()
                })
            }))
            .text_color(move || theme().foreground),
    )
}

// --- zikr page ----------------------------------------------------------------

/// One ref resolved: its verses (Arabic, optional translation).
#[derive(Debug, Clone, PartialEq)]
struct Group {
    surah: u16,
    start: u16,
    end: u16,
    rows: Vec<(u16, String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
enum Load {
    #[default]
    Loading,
    Failed,
    Ready(Vec<Group>),
}

async fn load_groups(sn: &'static Snippet, translation: Option<String>) -> Load {
    let store = quran_store();
    let mut out = Vec::new();
    for r in &sn.refs {
        let (Some(from), Some(to)) = (
            meta().global_index_of(r.surah, r.ayah_start),
            meta().global_index_of(r.surah, r.ayah_end),
        ) else {
            return Load::Failed;
        };
        let Ok((arabic, _)) = store
            .read_source(ArabicSource::Uthmani.id(), from, to)
            .await
        else {
            return Load::Failed;
        };
        let lane = match &translation {
            Some(id) => store
                .read_source(id, from, to)
                .await
                .ok()
                .map(|(t, _)| t.ayahs),
            None => None,
        };
        let rows = arabic
            .ayahs
            .into_iter()
            .map(|a| {
                let tr_text = lane
                    .as_ref()
                    .and_then(|l| l.iter().find(|x| x.ayah == a.ayah && x.surah == a.surah))
                    .map(|x| x.text.clone());
                (a.ayah, a.text, tr_text)
            })
            .collect();
        out.push(Group {
            surah: r.surah,
            start: r.ayah_start,
            end: r.ayah_end,
            rows,
        });
    }
    Load::Ready(out)
}

fn extent(g: &Group) -> String {
    if g.start == g.end {
        return format!("{}:{}", g.surah, g.start);
    }
    format!("{}:{}-{}", g.surah, g.start, g.end)
}

fn group_view(g: Group, ctx: Option<TranslationRef>) -> AnyElement {
    let surah = meta().surah(g.surah);
    let name = surah.map(|s| s.name.clone()).unwrap_or_default();
    let arabic_name = surah.map(|s| s.arabic.clone()).unwrap_or_default();
    let link = Route::Reader(ReaderLink::new(ReaderTarget {
        scope: ReaderScope::Surah(g.surah),
        translation: ctx,
        ayah: Some(g.start),
    }));
    let label = format!("{} →", extent(&g));
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_3()
                .py_3()
                .border_b_1()
                .border_color(tokens::reader_divider)
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(text(name).text_sm().font_weight(FontWeight::MEDIUM))
                        .child(
                            text(arabic_name)
                                .text_base()
                                .text_color(tokens::foreground_secondary),
                        ),
                )
                .child(button_with(
                    SharedString::from(label),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || {
                        nav::push(link.clone());
                    },
                )),
        )
        .children(g.rows.into_iter().map(|(ayah, arabic, translation)| {
            view()
                .w_full()
                .flex_col()
                .gap_1p5()
                .child(
                    plain_ayah(arabic, Some(u32::from(ayah)), || {
                        ArabicStyle::of(QuranScript::Uthmani)
                    })
                    .text_color(tokens::quran_foreground),
                )
                .child(translation.map(|line| {
                    text(line)
                        .text_base()
                        .text_color(tokens::translation_foreground)
                }))
                .into_element()
        }))
        .into_element()
}

/// The recite panel: the bound card's live tally, target, recite, next.
fn recite_panel(card: Value, arrival: i64, slug: String) -> AnyElement {
    let data = home_data();
    let id = card_id(&card).to_owned();
    let count = card.get("count").and_then(Value::as_i64).unwrap_or(0);
    let target = card.get("target").and_then(Value::as_i64);
    let done = is_target_done(&card);
    let revealed = if target.is_none() {
        count != arrival
    } else {
        done
    };
    let open_id = id.clone();
    let next = data.with_untracked(|d| next_zikr_card(&d.cards, &id, &slug, now_minute()).cloned());
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .p_4()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || {
            if done {
                tokens::border_strong()
            } else {
                theme().border
            }
        })
        .bg(move || {
            if done {
                tokens::primary_soft()
            } else {
                tokens::surface()
            }
        })
        .child(
            view()
                .w_full()
                .flex_row()
                .items_center()
                .gap_3()
                .child(view().flex_1().child(count_line(
                    count,
                    target,
                    keys::READER_ZIKR_OF_TARGET,
                )))
                .child(button_with(
                    t(keys::READER_CARDS_OPEN),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || {
                        nav::push(Route::YoursCard(open_id.clone()));
                    },
                )),
        )
        .child(target_bar(count, target))
        .child(recite_button(id, done, slug))
        .child(match (revealed, next) {
            (true, Some(next)) => card_link(&next).map(|link| {
                let title = tile_copy(&next).title;
                button_with(
                    t_args(keys::READER_ZIKR_NEXT, move || {
                        vec![("title", title.clone().into())]
                    }),
                    if done {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Outline
                    },
                    ButtonSize::Medium,
                    false,
                    None,
                    move || open_link(link.clone()),
                )
                .into_element()
            }),
            (_, None) if done => Some(muted(t(keys::READER_ZIKR_ALL_DONE)).into_element()),
            _ => None,
        })
        .into_element()
}

/// No card for this set yet: the "Add card" disclosure with an optional
/// target stepper.
fn add_card_inline(slug: String, created: Signal<Option<String>>) -> impl IntoElement {
    let open = signal(false);
    let full = signal(false);
    let target = signal::<Option<i64>>(None);
    let add = move || {
        let mut card =
            json!({ "kind": "snippet", "slug": slug.clone(), "sourceId": Value::Null, "count": 0 });
        if let Some(tv) = target.get_untracked() {
            card["target"] = json!(tv);
        }
        home::add_card(card, move |added| match added {
            Some(id) => {
                crate::ui::toast::notify(tr(keys::READER_CARDS_ADDED));
                created.set(Some(id));
                open.set(false);
                target.set(None);
            }
            None => full.set(true),
        });
    };
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(view().flex_row().child(button_with(
            t(keys::READER_CARDS_ADD),
            ButtonVariant::Outline,
            ButtonSize::Small,
            false,
            None,
            move || {
                full.set(false);
                open.set(!open.get_untracked());
                target.set(None);
            },
        )))
        .child(when(
            move || open.get(),
            move || {
                let add = add.clone();
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        text(t(keys::READER_CARDS_TARGET_LABEL))
                            .text_sm()
                            .text_color(tokens::muted_text),
                    )
                    .child(button_with(
                        "−",
                        ButtonVariant::Outline,
                        ButtonSize::Small,
                        move || target.get().is_none(),
                        Some(tr(keys::READER_ZIKR_TARGET_DOWN)),
                        move || {
                            target.update(|v| *v = v.map(|x| (x - 1).max(1)));
                        },
                    ))
                    .child(
                        text(move || {
                            SharedString::from(
                                target
                                    .get()
                                    .map_or_else(|| "—".to_owned(), |v| v.to_string()),
                            )
                        })
                        .text_sm(),
                    )
                    .child(button_with(
                        "+",
                        ButtonVariant::Outline,
                        ButtonSize::Small,
                        false,
                        Some(tr(keys::READER_ZIKR_TARGET_UP)),
                        move || {
                            target.update(|v| {
                                *v = Some(v.map_or(TARGET_FIRST, |x| (x + 1).min(TARGET_MAX)))
                            });
                        },
                    ))
                    .child(button_with(
                        t(keys::READER_ZIKR_ADD_CONFIRM),
                        ButtonVariant::Primary,
                        ButtonSize::Small,
                        false,
                        None,
                        add,
                    ))
                    .child(button_with(
                        t(keys::READER_CARDS_CANCEL),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || {
                            open.set(false);
                            target.set(None);
                        },
                    ))
                    .into_element()
            },
        ))
        .child(when(
            move || full.get(),
            || {
                text(t(keys::READER_CARDS_FULL))
                    .text_sm()
                    .text_color(tokens::danger)
                    .into_element()
            },
        ))
}

/// Enters the snippet's bounded thread (a card entry binds to its card).
fn enter_thread(
    sn: &'static Snippet,
    source_id: Option<String>,
    thread: Signal<Option<ReadingThread>>,
) {
    let (Some(first), Some(last)) = (sn.refs.first(), sn.refs.last()) else {
        return;
    };
    let target = ThreadTarget {
        scope: format!("snippet:{}", sn.slug),
        surah: i64::from(first.surah),
        ayah: i64::from(first.ayah_start),
        exact: false,
        bound_end: Some(format!("{}:{}", last.surah, last.ayah_end)),
        source_id,
    };
    let intent = home::take_card_entry();
    let came_from_reader = crate::features::reader::session::take_from_reader();
    store().write(
        Area::Sessions,
        move |s: &mut AppStore| sessions::enter(s, &target, intent, came_from_reader),
        move |r: Result<sessions::EnterResult, StoreError>| match r {
            Ok(res) => {
                let _ = thread.try_set(Some(res.thread));
            }
            Err(e) => log::warn!("zikr: session enter failed: {e}"),
        },
    );
}

fn header_actions(
    sn: &'static Snippet,
    groups: Signal<Load>,
    ctx: Option<TranslationRef>,
) -> impl IntoElement {
    let marked = signal(false);
    let refs: Vec<(i64, i64, i64)> = sn
        .refs
        .iter()
        .map(|r| {
            (
                i64::from(r.surah),
                i64::from(r.ayah_start),
                i64::from(r.ayah_end),
            )
        })
        .collect();
    let sync = store();
    let refs_read = refs.clone();
    effect(move || {
        let _ = sync.version(Area::Bookmarks);
        if !sync.ready() {
            return;
        }
        let refs = refs_read.clone();
        untracked(move || {
            sync.run(
                move |s| {
                    for (su, a, b) in &refs {
                        if !bookmarks::is_range_marked(s, *su, *a, *su, *b)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                },
                move |r| {
                    if let Ok(all) = r {
                        let _ = marked.try_set(all);
                    }
                },
            );
        });
    });
    let note = signal::<Option<&'static str>>(None);
    let toggle = move || {
        let refs = refs.clone();
        store().write(
            Area::Bookmarks,
            move |s| {
                let all = refs.iter().try_fold(true, |acc, (su, a, b)| {
                    Ok::<bool, StoreError>(acc && bookmarks::is_range_marked(s, *su, *a, *su, *b)?)
                })?;
                for (su, a, b) in &refs {
                    if all || !bookmarks::is_range_marked(s, *su, *a, *su, *b)? {
                        bookmarks::toggle_range(s, *su, *a, *su, *b)?;
                    }
                }
                Ok(())
            },
            |_| (),
        );
    };
    let share = move || {
        let Load::Ready(gs) = groups.get_untracked() else {
            return;
        };
        let (Some(first), Some(last)) = (gs.first(), gs.last()) else {
            return;
        };
        let verses: Vec<(u16, String)> = gs
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| (r.0, r.1.clone())))
            .collect();
        let link = range_link(ctx.as_ref(), first.surah, first.start, last.end);
        let title = range_title(first.surah, first.start, last.end);
        let text = range_share_text(first.surah, &verses, &link, false, true);
        spawn(async move {
            let Some(message) = share_feedback(share_or_copy(title, text, link).await) else {
                return;
            };
            note.set(Some(message));
            drop(set_timeout(std::time::Duration::from_secs(4), move || {
                note.set(None)
            }));
        });
    };
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(crate::ui::button::choice_button(
            t(keys::READER_BOOKMARK_RANGE),
            tr(keys::READER_BOOKMARK_RANGE),
            move || marked.get(),
            false,
            toggle,
        ))
        .child(button_with(
            t(keys::READER_SHARE),
            ButtonVariant::Outline,
            ButtonSize::Small,
            false,
            None,
            share,
        ))
        .child(dynamic(move || {
            note.get().map(|n| {
                text(n)
                    .text_xs()
                    .text_color(tokens::muted_text)
                    .into_element()
            })
        }))
}

pub fn zikr_page(slug: String, translation: Option<TranslationRef>) -> impl IntoElement {
    crate::features::downloads::dev_capture();
    let Some(sn) = snippet_for_slug(&slug) else {
        return scroll()
            .flex_1()
            .content_inset(page_insets(0.0))
            .child(page_body().child(muted(t(keys::READER_CARDS_NOT_FOUND))))
            .into_element();
    };
    let translation_id = translation.as_ref().map(TranslationRef::id);
    let translation_name = translation_id
        .as_deref()
        .and_then(catalogue::translation)
        .map(|t| t.name.clone());
    let groups = signal(Load::Loading);
    let tid = translation_id.clone();
    spawn(async move {
        let loaded = load_groups(sn, tid).await;
        let _ = groups.try_set(loaded);
    });
    let thread = signal::<Option<ReadingThread>>(None);
    enter_thread(sn, translation_id.clone(), thread);

    // The recite panel lives on the Arabic route only, as on the web.
    let data = home_data();
    let created = signal::<Option<String>>(None);
    let arrival = signal::<Option<(String, i64)>>(None);
    let slug_m = slug.clone();
    let matched = move || -> Option<Value> {
        data.with(|d| {
            let bound = thread.with(|th| bound_card_for(th.as_ref(), &d.cards, &slug_m).cloned());
            let made = created.with(|c| {
                c.as_ref()
                    .and_then(|id| d.card(id))
                    .filter(|c| c.get("slug").and_then(Value::as_str) == Some(&slug_m))
                    .cloned()
            });
            bound.or(made).or_else(|| {
                d.cards
                    .iter()
                    .find(|c| {
                        c.get("kind").and_then(Value::as_str) == Some("snippet")
                            && c.get("slug").and_then(Value::as_str) == Some(&slug_m)
                    })
                    .cloned()
            })
        })
    };
    let arabic_route = translation.is_none();
    let slug_p = slug.clone();
    let panel = dynamic(move || {
        if !arabic_route || !data.with(|d| d.loaded) {
            return None;
        }
        Some(match matched() {
            Some(card) => {
                let id = card_id(&card).to_owned();
                let count = card.get("count").and_then(Value::as_i64).unwrap_or(0);
                // The arrival count is per bound card (a re-bind starts over).
                let base = untracked(|| arrival.get())
                    .filter(|(a, _)| *a == id)
                    .map(|(_, c)| c);
                let base = base.unwrap_or_else(|| {
                    untracked(|| arrival.set(Some((id.clone(), count))));
                    count
                });
                recite_panel(card, base, slug_p.clone())
            }
            None => add_card_inline(slug_p.clone(), created).into_element(),
        })
    });

    let ctx = translation.clone();
    scroll()
        .flex_1()
        .content_inset(page_insets(0.0))
        .child(
            page_body()
                .child(
                    view()
                        .w_full()
                        .flex_col()
                        .gap_2()
                        .child(
                            view().flex_row().child(
                                view()
                                    .px_2()
                                    .py_0p5()
                                    .rounded(tokens::radius_pill)
                                    .border_1()
                                    .border_color(tokens::border_strong)
                                    .child(
                                        text(localized(&kind_label(sn.kind)))
                                            .text_xs()
                                            .text_color(tokens::muted_text),
                                    ),
                            ),
                        )
                        .child(page_heading(SharedString::from(localized(&sn.title))))
                        .child(body(localized(&sn.description)))
                        .child(
                            translation_name
                                .map(|n| text(n).text_xs().text_color(tokens::muted_text)),
                        )
                        .child(header_actions(sn, groups, ctx.clone())),
                )
                .child(panel)
                .child(dynamic(move || match groups.get() {
                    Load::Loading => muted("…").into_element(),
                    // The web page is prerendered; the app says the text is offline.
                    Load::Failed => muted(t(keys::READER_OFFLINE_TITLE)).into_element(),
                    Load::Ready(gs) => view()
                        .w_full()
                        .flex_col()
                        .gap_8()
                        .children(gs.into_iter().map(|g| group_view(g, ctx.clone())))
                        .into_element(),
                }))
                .text_color(move || theme().foreground),
        )
        .into_element()
}
