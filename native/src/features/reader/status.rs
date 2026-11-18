//! The reader's states around the text: the status banner
//! (`ReaderStatusBanner`, `reader-degradation`), the full-page failure,
//! the resume pill (`ResumePill`) and the neighbour navigation
//! (`ReaderPageNav`, the range readers' prev/next row).

use eq_core::{
    i18n::keys,
    quran::{meta::meta, read::ServedBy},
    routes::ReaderScope,
};
use nsh_core::prelude::*;

use super::model::Failure;
use crate::{
    shell::{nav, routes::Route},
    stores::locale::{t, t_args, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        tokens,
    },
};

/// What the banner says under the bar while the text shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// The script is not on the device: read from the network.
    OnlineOnly,
    /// A stacked translation could not be read.
    TranslationMissing,
}

/// The notice for a finished read.
pub fn notice_for(served: ServedBy, failed_lanes: bool) -> Option<Notice> {
    if served == ServedBy::Api {
        return Some(Notice::OnlineOnly);
    }
    failed_lanes.then_some(Notice::TranslationMissing)
}

fn downloads_button() -> impl IntoElement {
    button_with(
        t(keys::DOWNLOADS_TITLE),
        ButtonVariant::Outline,
        ButtonSize::Small,
        false,
        None,
        || nav::push(Route::Downloads),
    )
}

/// The one-line banner (`ReaderStatusBanner`).
pub fn banner(notice: Notice) -> impl IntoElement {
    let (message, downloads) = match notice {
        Notice::OnlineOnly => (keys::READER_OFFLINE_COPY_UNAVAILABLE, true),
        Notice::TranslationMissing => (keys::READER_RANGE_TRANSLATION_UNAVAILABLE, false),
    };
    view()
        .w_full()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(move || nsh_base::theme().border)
        .bg(tokens::background_subtle)
        .role(AccessibilityRole::Summary)
        .child(
            text(t(message))
                .flex_1()
                .min_w(px(0.0))
                .text_sm()
                .text_color(tokens::foreground_secondary),
        )
        .child(downloads.then(downloads_button))
}

/// The page when the Arabic could not be read: why, Retry, and Downloads
/// when the script is simply not on the device.
pub fn failure_view(failure: Failure, retry: impl Fn() + 'static) -> impl IntoElement {
    let message = match failure {
        Failure::NotDownloaded => keys::READER_OFFLINE_TITLE,
        Failure::Network => keys::READER_NETWORK_UNAVAILABLE,
        Failure::Data => keys::READER_OFFLINE_DATA_UNAVAILABLE,
    };
    let detail = (failure == Failure::NotDownloaded).then(|| {
        text(t(keys::READER_OFFLINE_DATA_UNAVAILABLE))
            .text_sm()
            .text_color(tokens::foreground_secondary)
    });
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .p_6()
        .gap_3()
        .role(AccessibilityRole::Alert)
        .child(
            text(t(message))
                .text_base()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || nsh_base::theme().foreground),
        )
        .child(detail)
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(button_with(
                    t(keys::READER_RETRY),
                    ButtonVariant::Primary,
                    ButtonSize::Small,
                    false,
                    None,
                    retry,
                ))
                .child((failure == Failure::NotDownloaded).then(downloads_button)),
        )
}

/// "Picked up where you left off · Start over · Close" (`ResumePill`).
pub fn resume_pill(
    start_over: impl Fn() + 'static,
    close: impl Fn() + 'static,
) -> impl IntoElement {
    view().w_full().items_center().py_2().child(
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .pl_4()
            .pr_1()
            .py_1()
            .rounded_full()
            .border_1()
            .border_color(move || nsh_base::theme().border)
            .bg(move || nsh_base::theme().popover)
            .shadow_sm()
            .role(AccessibilityRole::Summary)
            .child(
                text(t(keys::READER_THREADS_RESUMED))
                    .number_of_lines(1)
                    .text_sm()
                    .text_color(tokens::foreground_secondary),
            )
            .child(button_with(
                t(keys::READER_THREADS_START_OVER),
                ButtonVariant::Secondary,
                ButtonSize::Small,
                false,
                None,
                start_over,
            ))
            .child(button_with(
                t(keys::READER_THREADS_CLOSE),
                ButtonVariant::Ghost,
                ButtonSize::Small,
                false,
                None,
                close,
            )),
    )
}

/// The label of a neighbouring scope.
fn scope_label(scope: ReaderScope) -> Box<dyn Fn() -> SharedString> {
    let (key, index) = match scope {
        ReaderScope::Surah(n) => {
            let name = meta()
                .surah(n)
                .map(|s| format!("{} {}", s.num, s.name))
                .unwrap_or_default();
            return Box::new(move || SharedString::from(name.clone()));
        }
        ReaderScope::Page(n) => (keys::READER_PAGE_ITEM, i64::from(n)),
        ReaderScope::Juz(n) => (keys::READER_JUZ_ITEM, i64::from(n)),
        ReaderScope::Hizb(n) => (keys::READER_HIZB_ITEM, i64::from(n)),
    };
    Box::new(t_args(key, move || vec![("index", index.into())]))
}

/// Previous / next under the text: surahs for a surah reader (the next
/// one filled), pages, ajzāʼ or ahzāb for a range.
pub fn neighbour_nav(
    previous: Option<ReaderScope>,
    next: Option<ReaderScope>,
    go: impl Fn(ReaderScope) + Copy + 'static,
) -> impl IntoElement {
    let surah = matches!(previous.or(next), Some(ReaderScope::Surah(_)));
    let (prev_name, next_name) = if surah {
        (tr(keys::READER_PREV_SURAH), tr(keys::READER_NEXT_SURAH))
    } else {
        ("←", "→")
    };
    let side = move |scope: Option<ReaderScope>, arrow_first: bool, variant: ButtonVariant| {
        scope.map(|scope| {
            let label = scope_label(scope);
            let label = move || -> SharedString {
                if arrow_first {
                    format!("← {}", label()).into()
                } else {
                    format!("{} →", label()).into()
                }
            };
            button_with(
                label,
                variant,
                ButtonSize::Medium,
                false,
                Some(if arrow_first { prev_name } else { next_name }),
                move || go(scope),
            )
        })
    };
    view()
        .w_full()
        .flex_row()
        .flex_wrap()
        .justify_between()
        .gap_2()
        .px_4()
        .py_6()
        .label(tr(keys::READER_SURAH_NAV_LABEL))
        .child(side(previous, true, ButtonVariant::Outline))
        .child(view().flex_1())
        .child(side(
            next,
            false,
            if surah {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Outline
            },
        ))
}
