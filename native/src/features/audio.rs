//! Audio dock and in-reader controls (PARITY D3): web
//! `components/audio/{AudioDock,RepeatPanel,AudioVerseActions,AudioSurahAction}.svelte`.
//! Dark while the catalogue lists no recording (`stores::audio::enabled`),
//! as on the web: nothing renders and no hotkey binds.
//! Contract: `pub fn dock() -> impl IntoElement`, mounted once by the shell.

use eq_core::{
    audio::{
        offline::CacheResult,
        policy::{OFFLINE_AUDIO_MAX_BYTES, PLAYBACK_RATES},
        queue::QueueSegment,
        repeat::{
            MAX_REPEAT_PRESETS, RepeatPlanInput, RepeatPreset, add_preset, load_presets,
            new_preset_id, normalize_plan, remove_preset, save_presets,
        },
    },
    i18n::{Arg, keys},
    quran::meta::meta,
    routes::{ReaderLink, ReaderScope, ReaderTarget},
};
use nsh_base::theme;
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    services::{audio::clear_files, storage::NshKv},
    shell::{desktop, nav, routes::Route},
    stores::{
        audio::{AudioStatus, AudioStore, audio_store, catalogue, enabled},
        locale::{locale, t, t_args, tr},
    },
    ui::{
        button::{
            ButtonSize, ButtonVariant, IconButtonSize, IconButtonVariant, button_with,
            choice_button, icon_button, icon_button_with,
        },
        controls::slider,
        field::text_field,
        icons::lucide,
        layout::tab_bar_room,
        number::number_field,
        text::{TextRole, role},
        tokens,
    },
};

const PLAY: &str = lucide!(r#"<polygon points="6 3 20 12 6 21 6 3"/>"#);
const PAUSE: &str = lucide!(
    r#"<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>"#
);
const REPEAT: &str = lucide!(
    r#"<path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/>"#
);
/// Speed labels, one per `PLAYBACK_RATES` entry (screen-reader names).
const RATE_LABELS: [&str; 5] = ["0.75x", "1x", "1.25x", "1.5x", "2x"];
/// Queue rows shown on each side of the current one (`WINDOW`).
const WINDOW: usize = 30;

fn ayah_count(surah: u16) -> u16 {
    meta().surah(surah).map_or(0, |s| s.ayah_count)
}

fn now_playing(surah: u16, ayah: u16) -> impl Fn() -> SharedString + 'static {
    t_args(keys::AUDIO_NOW_PLAYING, move || {
        vec![
            ("surah", Arg::from(u64::from(surah))),
            ("ayah", Arg::from(u64::from(ayah))),
        ]
    })
}

/// `statusText`.
fn status_text(store: AudioStore) -> Option<String> {
    match store.status() {
        AudioStatus::Error => Some(tr(keys::AUDIO_ERROR).to_owned()),
        AudioStatus::Loading if store.attempt() > 0 => Some(eq_core::i18n::tr_args(
            locale(),
            keys::AUDIO_RETRYING,
            &[("attempt", Arg::from(u64::from(store.attempt())))],
        )),
        AudioStatus::Loading => Some(tr(keys::AUDIO_LOADING).to_owned()),
        _ => None,
    }
}

/// Room under the dock: the floating tab bar on phone home pages.
fn bottom_room() -> f32 {
    if desktop() || nav::current().is_detail() {
        return 0.0;
    }
    tab_bar_room()
}

/// The dock: the bar while a queue is loaded, the full player when opened.
/// Media hotkeys bind in `shell::install` ([`install_hotkeys`]).
pub fn dock() -> impl IntoElement {
    if !enabled() {
        return view().into_element();
    }
    let store = audio_store();
    store.hydrate();
    store.install();
    let docked = computed(move || store.item().is_some() && store.recording().is_some());
    view()
        .absolute()
        .left_0()
        .right_0()
        .bottom(move || px(bottom_room()))
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        .on_layout(move |b: &Bounds<Points>| {
            let h = b.size.height.0;
            if dock_height_signal().get_untracked().0 != h {
                dock_height_signal().set(DockHeight(h));
            }
        })
        .child(when(move || docked.get(), move || dock_body(store)))
        .into_element()
}

/// The dock's measured height (web `--eq-dock-h`).
#[derive(Default, Clone, Copy, PartialEq)]
struct DockHeight(f32);

fn dock_height_signal() -> Signal<DockHeight> {
    global::<DockHeight>()
}

/// How far above the window's bottom edge the dock reaches, 0 while it is
/// hidden (tracked): the download bar and toasts stack above it, as the
/// web's `--eq-dock-h` does.
pub fn dock_top() -> f32 {
    let h = dock_height_signal().get().0;
    if h <= 0.0 {
        return 0.0;
    }
    h + bottom_room()
}

/// `mod+shift+space` play/pause, `mod+shift+←/→` previous/next ayah (the
/// dock's hotkeys on the web). Only while audio is enabled.
pub fn install_hotkeys() {
    if !enabled() {
        return;
    }
    let store = audio_store();
    on_key("mod+shift+space", move |_| store.toggle());
    on_key("mod+shift+right", move |_| store.next());
    on_key("mod+shift+left", move |_| store.prev());
}

fn dock_body(store: AudioStore) -> AnyElement {
    let notice = signal(None::<String>);
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .px_4()
        .pt_2()
        .pb_safe_area_plus(px(8.0))
        .border_t_1()
        .border_color(move || theme().border)
        .bg(move || theme().background)
        .label(tr(keys::AUDIO_DOCK_LABEL))
        .child(bar(store))
        .child(when(
            move || store.status() == AudioStatus::Error,
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        role(t(keys::AUDIO_ERROR), TextRole::Caption)
                            .text_color(move || theme().destructive),
                    )
                    .child(button_with(
                        t(keys::AUDIO_RETRY),
                        ButtonVariant::Secondary,
                        ButtonSize::Small,
                        false,
                        None,
                        move || store.retry(),
                    ))
            },
        ))
        .child(when(
            move || store.panel_open(),
            move || full_player(store, notice),
        ))
        .into_element()
}

fn bar(store: AudioStore) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(icon_button(
            "«",
            tr(keys::AUDIO_PREVIOUS_SURAH),
            move || store.prev_surah(),
        ))
        .child(icon_button("‹", tr(keys::AUDIO_PREVIOUS), move || {
            store.prev()
        }))
        .child(dynamic(move || {
            let (svg, name) = if store.is_playing() {
                (PAUSE, tr(keys::AUDIO_PAUSE))
            } else {
                (PLAY, tr(keys::AUDIO_PLAY))
            };
            icon_button_with(
                svg,
                name,
                IconButtonVariant::Primary,
                IconButtonSize::Small,
                move || store.toggle(),
            )
            .into_element()
        }))
        .child(icon_button("›", tr(keys::AUDIO_NEXT), move || {
            store.next()
        }))
        .child(icon_button("»", tr(keys::AUDIO_NEXT_SURAH), move || {
            store.next_surah()
        }))
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x(px(8.0))
                .child(
                    role(
                        move || {
                            let item = store.item();
                            item.map_or_else(SharedString::default, |i| {
                                now_playing(i.surah, i.ayah)()
                            })
                        },
                        TextRole::Body,
                    )
                    .number_of_lines(1),
                )
                .child(
                    role(
                        move || SharedString::from(status_text(store).unwrap_or_default()),
                        TextRole::Caption,
                    )
                    .text_color(tokens::muted_text),
                ),
        )
        .child(button_with(
            move || {
                if store.panel_open() {
                    return SharedString::from(tr(keys::AUDIO_COLLAPSE));
                }
                SharedString::from(tr(keys::AUDIO_EXPAND))
            },
            ButtonVariant::Ghost,
            ButtonSize::Small,
            false,
            None,
            move || store.set_panel_open(!untracked(|| store.panel_open())),
        ))
}

fn megabytes(bytes: u64) -> u64 {
    (bytes as f64 / (1024.0 * 1024.0)).round() as u64
}

/// Opens the playing ayah in the reader, in the translation being read.
fn open_ayah(surah: u16, ayah: u16) {
    let translation = match nav::current() {
        Route::Reader(link) => link.target.translation,
        _ => None,
    };
    nav::push(Route::Reader(ReaderLink::new(ReaderTarget {
        scope: ReaderScope::Surah(surah),
        translation,
        ayah: Some(ayah),
    })));
}

fn full_player(store: AudioStore, notice: Signal<Option<String>>) -> impl IntoElement {
    let busy = signal(false);
    let save_offline = move || {
        busy.set(true);
        spawn(async move {
            let result = store.cache_queue().await;
            busy.set(false);
            let text = match result {
                CacheResult::Ok => tr(keys::AUDIO_OFFLINE_SAVED).to_owned(),
                CacheResult::Limit => eq_core::i18n::tr_args(
                    locale(),
                    keys::AUDIO_OFFLINE_LIMIT,
                    &[("mb", Arg::from(megabytes(OFFLINE_AUDIO_MAX_BYTES)))],
                ),
                CacheResult::Failed => tr(keys::AUDIO_OFFLINE_FAILED).to_owned(),
            };
            notice.set(Some(text));
        });
    };
    let clear_offline = move || {
        spawn(async move {
            clear_files().await;
            notice.set(Some(tr(keys::AUDIO_OFFLINE_CLEARED).to_owned()));
        });
    };
    view().flex_col().gap_3().py_3().max_h(px(480.0)).child(
        scroll().flex_1().child(
            view()
                .flex_col()
                .gap_3()
                .child(player_header(store))
                .child(seek_bar(store))
                .child(speed_row(store))
                .child(dynamic(move || {
                    let Some(item) = store.item() else {
                        return view().into_element();
                    };
                    button_with(
                        t_args(keys::AUDIO_OPEN_AYAH, move || {
                            vec![
                                ("surah", Arg::from(u64::from(item.surah))),
                                ("ayah", Arg::from(u64::from(item.ayah))),
                            ]
                        }),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || open_ayah(item.surah, item.ayah),
                    )
                    .into_element()
                }))
                .child(queue_window(store))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(button_with(
                            t(keys::AUDIO_OFFLINE_SAVE),
                            ButtonVariant::Secondary,
                            ButtonSize::Small,
                            move || busy.get() || store.caching(),
                            None,
                            save_offline,
                        ))
                        .child(button_with(
                            t(keys::AUDIO_OFFLINE_CLEAR),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            clear_offline,
                        ))
                        .child(role(
                            move || SharedString::from(notice.get().unwrap_or_default()),
                            TextRole::Caption,
                        )),
                )
                .child(dynamic(move || {
                    let surah = repeat_surah(store);
                    let count = ayah_count(surah);
                    let recording = store.recording();
                    match recording {
                        Some(r) if count > 0 => {
                            repeat_panel(store, surah, count, r.id).into_element()
                        }
                        _ => view().into_element(),
                    }
                })),
        ),
    )
}

/// The repeat panel follows a range the reader requested, else the
/// playing surah.
fn repeat_surah(store: AudioStore) -> u16 {
    if let Some(request) = store.repeat_request() {
        return request.surah;
    }
    store.item().map_or(0, |i| i.surah)
}

fn player_header(store: AudioStore) -> impl IntoElement {
    view().flex_col().gap_1().child(dynamic(move || {
        let (Some(item), Some(recording)) = (store.item(), store.recording()) else {
            return view().into_element();
        };
        let reciter = catalogue()
            .reciter_by_id(&recording.reciter_id)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        let credit = match locale() {
            eq_core::i18n::Locale::Ar => recording.attribution.ar.clone(),
            eq_core::i18n::Locale::En => recording.attribution.en.clone(),
        };
        let licence_line = eq_core::i18n::tr_args(
            locale(),
            keys::AUDIO_ATTRIBUTION,
            &[
                ("reciter", Arg::from(reciter.as_str())),
                ("licence", Arg::from(recording.licence.name.as_str())),
            ],
        );
        let source = recording.attribution.url.clone();
        view()
            .flex_col()
            .gap_1()
            .child(role(now_playing(item.surah, item.ayah), TextRole::BodyL))
            .child(
                role(
                    format!("{reciter} · {}", recording.label),
                    TextRole::Caption,
                )
                .text_color(tokens::muted_text),
            )
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_x(px(6.0))
                    .child(role(format!("{licence_line} {credit}"), TextRole::Micro))
                    .child(button_with(
                        t(keys::AUDIO_ATTRIBUTION_SOURCE),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || {
                            let url = source.clone();
                            spawn(async move {
                                open_url(url).await;
                            });
                        },
                    )),
            )
            .into_element()
    }))
}

/// The position slider in 0.1 s steps. The slider's value follows the
/// player; a change the player did not make is a seek.
fn seek_bar(store: AudioStore) -> impl IntoElement {
    const STEPS: f64 = 1000.0;
    let value = signal(0.0_f64);
    let synced = std::rc::Rc::new(std::cell::Cell::new(0.0_f64));
    {
        let synced = synced.clone();
        effect(move || {
            let (position, duration) = (store.position(), store.duration());
            let at = if duration > 0.0 {
                (position / duration * STEPS).round()
            } else {
                0.0
            };
            synced.set(at);
            value.set(at);
        });
    }
    effect(move || {
        let at = value.get();
        if (at - synced.get()).abs() < 1.0 {
            return;
        }
        let duration = untracked(|| store.duration());
        store.seek(at / STEPS * duration);
    });
    view()
        .w_full()
        .label(tr(keys::AUDIO_SEEK))
        .child(slider(value, 0.0, STEPS))
}

fn speed_row(store: AudioStore) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1()
        .label(tr(keys::AUDIO_SPEED))
        .child(role(t(keys::AUDIO_SPEED), TextRole::Caption).text_color(tokens::muted_text))
        .children(PLAYBACK_RATES.iter().zip(RATE_LABELS).map(|(&rate, name)| {
            choice_button(
                name,
                name,
                move || (store.rate() - rate).abs() < f32::EPSILON,
                false,
                move || store.set_rate(f64::from(rate)),
            )
        }))
}

fn queue_window(store: AudioStore) -> impl IntoElement {
    view()
        .flex_col()
        .gap_1()
        .label(tr(keys::AUDIO_QUEUE))
        .child(role(t(keys::AUDIO_QUEUE), TextRole::Caption).text_color(tokens::muted_text))
        .child(dynamic(move || {
            let Some(queue) = store.queue() else {
                return view().into_element();
            };
            let start = queue.index.saturating_sub(WINDOW);
            let end = queue.items.len().min(queue.index + WINDOW + 1);
            let rows = (start..end).map(|index| {
                let item = queue.items[index];
                let variant = if index == queue.index {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                };
                button_with(
                    format!("{}:{}", item.surah, item.ayah),
                    variant,
                    ButtonSize::Small,
                    false,
                    None,
                    move || store.jump_to(index),
                )
            });
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(rows)
                .into_element()
        }))
}

fn repeat_panel(
    store: AudioStore,
    surah: u16,
    count: u16,
    recording_id: String,
) -> impl IntoElement {
    let from = signal(Some(1.0_f64));
    let to = signal(Some(3.0_f64));
    let ayah_repeat = signal(Some(1.0_f64));
    let set_repeat = signal(Some(1.0_f64));
    let delay_ms = signal(Some(500.0_f64));
    let preset_name = signal(String::new());
    let presets = signal(load_presets(&NshKv));
    let message = signal(None::<&'static str>);
    // A reader's "Repeat this verse" fills the range (on mount too).
    effect(move || {
        if let Some(request) = store.repeat_request() {
            from.set(Some(f64::from(request.from)));
            to.set(Some(f64::from(request.to)));
        }
    });
    let input = move || RepeatPlanInput {
        surah,
        from: from.get().unwrap_or(f64::NAN),
        to: to.get().unwrap_or(f64::NAN),
        ayah_repeat: ayah_repeat.get().unwrap_or(f64::NAN),
        set_repeat: set_repeat.get().unwrap_or(f64::NAN),
        delay_ms: delay_ms.get().unwrap_or(f64::NAN),
    };
    let plan_valid = move || normalize_plan(input(), count).is_some();
    let rec = recording_id.clone();
    let start = move || {
        let plan = untracked(input);
        let ok = normalize_plan(plan, count).is_some() && store.start_repeat(&rec, plan, count);
        message.set((!ok).then_some(tr(keys::AUDIO_REPEAT_INVALID)));
    };
    let save_preset = move || {
        let name = preset_name.get_untracked().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let list = presets.get_untracked();
        if list.len() >= MAX_REPEAT_PRESETS {
            message.set(Some(tr(keys::AUDIO_PRESET_FULL)));
            return;
        }
        let preset = RepeatPreset {
            id: new_preset_id(),
            name,
            input: untracked(input),
        };
        let next = add_preset(&list, preset);
        let saved = save_presets(&NshKv, &next);
        presets.set(next);
        preset_name.set(String::new());
        message.set((!saved).then_some(tr(keys::AUDIO_PRESET_FAILED)));
    };
    let number = |key, value: Signal<Option<f64>>| {
        view()
            .flex_col()
            .gap_1()
            .child(role(t(key), TextRole::Caption))
            .child(number_field(tr(key), value, ""))
    };
    view()
        .flex_col()
        .gap_2()
        .pt_2()
        .border_t_1()
        .border_color(move || theme().border)
        .child(role(t(keys::AUDIO_REPEAT_TITLE), TextRole::BodyL))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(number(keys::AUDIO_REPEAT_FROM, from))
                .child(number(keys::AUDIO_REPEAT_TO, to))
                .child(number(keys::AUDIO_REPEAT_AYAH_COUNT, ayah_repeat))
                .child(number(keys::AUDIO_REPEAT_SET_COUNT, set_repeat))
                .child(number(keys::AUDIO_REPEAT_DELAY, delay_ms)),
        )
        .child(dynamic(move || {
            if store.repeat_step().is_some() {
                return button_with(
                    t(keys::AUDIO_REPEAT_STOP),
                    ButtonVariant::Secondary,
                    ButtonSize::Medium,
                    false,
                    None,
                    move || store.stop_repeat(),
                )
                .into_element();
            }
            button_with(
                t(keys::AUDIO_REPEAT_START),
                ButtonVariant::Primary,
                ButtonSize::Medium,
                move || !plan_valid(),
                None,
                start.clone(),
            )
            .into_element()
        }))
        .child(role(
            move || {
                let Some(step) = store.repeat_step() else {
                    return SharedString::default();
                };
                SharedString::from(eq_core::i18n::tr_args(
                    locale(),
                    keys::AUDIO_REPEAT_PROGRESS,
                    &[
                        ("ayah", Arg::from(u64::from(step.ayah))),
                        ("pass", Arg::from(u64::from(step.pass))),
                        ("passes", Arg::from(u64::from(step.passes))),
                    ],
                ))
            },
            TextRole::Caption,
        ))
        .child(role(
            move || SharedString::from(message.get().unwrap_or_default()),
            TextRole::Caption,
        ))
        .child(role(t(keys::AUDIO_REPEAT_AWAKE), TextRole::Micro).text_color(tokens::muted_text))
        .child(role(t(keys::AUDIO_PRESETS), TextRole::Caption))
        .child(dynamic(move || {
            let rows = presets
                .get()
                .into_iter()
                .filter(|p| p.input.surah == surah)
                .map(|preset| {
                    let id = preset.id.clone();
                    let loaded = preset.input;
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(role(preset.name.clone(), TextRole::Body).flex_1())
                        .child(button_with(
                            t(keys::AUDIO_PRESET_LOAD),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            move || {
                                from.set(Some(loaded.from));
                                to.set(Some(loaded.to));
                                ayah_repeat.set(Some(loaded.ayah_repeat));
                                set_repeat.set(Some(loaded.set_repeat));
                                delay_ms.set(Some(loaded.delay_ms));
                            },
                        ))
                        .child(button_with(
                            t(keys::AUDIO_PRESET_DELETE),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            move || {
                                let next = remove_preset(&presets.get_untracked(), &id);
                                let saved = save_presets(&NshKv, &next);
                                presets.set(next);
                                message.set((!saved).then_some(tr(keys::AUDIO_PRESET_FAILED)));
                            },
                        ))
                });
            view().flex_col().gap_1().children(rows).into_element()
        }))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(text_field(preset_name, tr(keys::AUDIO_PRESET_NAME)).flex_1())
                .child(button_with(
                    t(keys::AUDIO_PRESET_SAVE),
                    ButtonVariant::Secondary,
                    ButtonSize::Small,
                    move || preset_name.get().trim().is_empty(),
                    None,
                    save_preset,
                )),
        )
}

/// The verse tool strip's audio actions (`AudioVerseActions`): play from
/// this verse to the surah's end, and repeat this verse. `None` while dark.
pub fn verse_actions(surah: u16, ayah: u16) -> Option<AnyElement> {
    let recording_id = catalogue().default_recording_id()?.to_owned();
    let store = audio_store();
    let count = ayah_count(surah);
    Some(
        view()
            .flex_row()
            .items_center()
            .child(icon_button_with(
                PLAY,
                tr(keys::AUDIO_PLAY_FROM_HERE),
                IconButtonVariant::Ghost,
                IconButtonSize::Small,
                move || {
                    if count < ayah {
                        return;
                    }
                    let segment = QueueSegment {
                        surah,
                        from: ayah,
                        to: count,
                    };
                    store.play_queue(&recording_id, &[segment]);
                },
            ))
            .child(icon_button_with(
                REPEAT,
                tr(keys::AUDIO_REPEAT_VERSE),
                IconButtonVariant::Ghost,
                IconButtonSize::Small,
                move || store.request_repeat(surah, ayah, ayah),
            ))
            .into_element(),
    )
}

/// The reader header's "Play surah" (`AudioSurahAction`) for the surah
/// `surah` names now. `None` while dark.
pub fn surah_action(surah: impl Fn() -> Option<u16> + 'static) -> Option<AnyElement> {
    let recording_id = catalogue().default_recording_id()?.to_owned();
    let store = audio_store();
    let surah = std::rc::Rc::new(surah);
    Some(
        button_with(
            t(keys::AUDIO_PLAY_SURAH),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            false,
            Some(tr(keys::AUDIO_PLAY_SURAH)),
            move || {
                let Some(num) = untracked(|| surah()) else {
                    return;
                };
                let count = ayah_count(num);
                if count < 1 {
                    return;
                }
                let segment = QueueSegment {
                    surah: num,
                    from: 1,
                    to: count,
                };
                store.play_queue(&recording_id, &[segment]);
            },
        )
        .into_element(),
    )
}
