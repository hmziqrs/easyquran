//! Verse sharing (PARITY C9): web `reader-share.svelte.ts` (plain-text
//! bodies and deep links), `share/range.ts`, `share/verse-card.ts` and
//! `ShareDialog.svelte`.
//!
//! Share (`shareVerse`, `shareRange`) shows the system's share sheet
//! (`nsh_core` `share`) where the platform has one (iOS, Android, macOS);
//! elsewhere (Windows, Linux) it takes the web's no-`navigator.share` path:
//! the text goes to the clipboard. Copy stays a separate, clipboard-only
//! action, as on the web. Image format renders a 1080 px branded PNG via
//! `eq_core::share::render_verse_card` and saves it (desktop save dialog) or
//! shares the PNG file (mobile share sheet).

use std::path::PathBuf;

use eq_core::{
    i18n::keys,
    quran::{
        meta::meta,
        types::ArabicSource,
        view::{
            group_range, tajweed::strip_tajweed_markup, text_marks::strip_transliteration_markup,
        },
    },
    routes::{HOSTS, TranslationRef, surah_ayah_path_for, surah_path_for},
    share::{CardInput, CardVerse, render_verse_card},
};
use nsh_core::{
    file_dialog::{FileFilter, SaveDialog},
    prelude::*,
    share::{ShareAction, ShareRequest},
};

use crate::{
    services::files,
    stores::locale::{t, t_args, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        controls::checkbox_row,
        dialog::dialog_card,
        field::numeric_field,
        toast::notify,
        tokens,
    },
};

const HAFS_FONT: &[u8] = include_bytes!("../../../assets/fonts/uthmanic-hafs-v22.ttf");
const INDOPAK_FONT: &[u8] = include_bytes!("../../../assets/fonts/indopak-reader-compat-v4.ttf");
const LATIN_FONT: &[u8] = include_bytes!("../../../assets/fonts/Onest.ttf");

/// Most ayahs one share may carry (`SHARE_SPAN_MAX`).
pub const SHARE_SPAN_MAX: i64 = 50;

/// Why a range cannot be shared (`RangeProblem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeProblem {
    Invalid,
    TooLong,
}

/// `rangeProblem`: an inclusive ayah range inside one surah.
pub fn range_problem(from: Option<i64>, to: Option<i64>, ayah_count: u16) -> Option<RangeProblem> {
    let (Some(from), Some(to)) = (from, to) else {
        return Some(RangeProblem::Invalid);
    };
    if from < 1 || to > i64::from(ayah_count) || from > to {
        return Some(RangeProblem::Invalid);
    }
    if to - from + 1 > SHARE_SPAN_MAX {
        return Some(RangeProblem::TooLong);
    }
    None
}

/// The public site the links point at.
pub fn origin() -> String {
    format!("https://{}", HOSTS[0])
}

fn surah_name(surah: u16) -> String {
    meta()
        .surah(surah)
        .map_or_else(|| format!("Surah {surah}"), |s| s.name.clone())
}

fn surah_slug(surah: u16) -> String {
    meta()
        .surah(surah)
        .map_or_else(|| format!("surah-{surah}"), |s| s.slug.clone())
}

/// `verseRef`: "Al-Baqarah 2:255".
pub fn verse_ref(surah: u16, ayah: u16) -> String {
    format!("{} {surah}:{ayah}", surah_name(surah))
}

/// `verseDeepLink`: the reader URL for one ayah, in the open translation.
pub fn verse_link(ctx: Option<&TranslationRef>, surah: u16, ayah: u16) -> String {
    format!(
        "{}{}",
        origin(),
        surah_ayah_path_for(ctx, &surah_slug(surah), surah, ayah)
    )
}

/// `rangeDeepLink`: `…/{slug}?ayah=2:255-2:286`.
pub fn range_link(ctx: Option<&TranslationRef>, surah: u16, from: u16, to: u16) -> String {
    format!(
        "{}{}?ayah={surah}:{from}-{surah}:{to}",
        origin(),
        surah_path_for(ctx, &surah_slug(surah))
    )
}

/// The plain-text view of a row's text: tajweed and transliteration
/// markup never leak into copied text.
pub fn plain(text: &str) -> String {
    strip_tajweed_markup(&strip_transliteration_markup(text))
}

/// `verseShareText`.
pub fn verse_share_text(text: &str, surah: u16, ayah: u16, link: &str) -> String {
    format!("{}\n{}\n{link}", plain(text), verse_ref(surah, ayah))
}

/// `rangeShareText`: verse text, optional ref per verse, optional link.
pub fn range_share_text(
    surah: u16,
    verses: &[(u16, String)],
    link: &str,
    include_reference: bool,
    include_link: bool,
) -> String {
    let body: Vec<String> = verses
        .iter()
        .map(|(ayah, text)| {
            let text = plain(text);
            if include_reference {
                return format!("{text}\n{}", verse_ref(surah, *ayah));
            }
            text
        })
        .collect();
    let mut parts = vec![body.join("\n\n")];
    if include_link {
        parts.push(link.to_owned());
    }
    parts.join("\n\n")
}

/// `rangeTitle`: "Al-Baqarah 2:255–257".
pub fn range_title(surah: u16, from: u16, to: u16) -> String {
    format!("{} {surah}:{from}\u{2013}{to}", surah_name(surah))
}

/// How a share ended (web `ShareResult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareResult {
    /// The share sheet sent it.
    Shared,
    /// No share sheet here: the text is on the clipboard.
    Copied,
    /// The sheet failed: the link is on the clipboard instead (FLOW-05).
    LinkCopied,
    /// The user closed the sheet.
    Cancelled,
}

/// The share sheet with `title`, `text` and `link` where the platform has
/// one, else `text` to the clipboard. A sheet that fails copies the link.
pub async fn share_or_copy(title: String, text: String, link: String) -> ShareResult {
    if !share_available().await {
        set_clipboard(text);
        return ShareResult::Copied;
    }
    let request = ShareRequest::new()
        .title(title)
        .text(text)
        .url(link.clone());
    match share(request).await {
        Ok(outcome) if outcome.action == ShareAction::Dismissed => ShareResult::Cancelled,
        Ok(_) => ShareResult::Shared,
        Err(e) => {
            log::info!("share failed: {}", e.code.as_deref().unwrap_or("none"));
            set_clipboard(link);
            ShareResult::LinkCopied
        }
    }
}

/// The feedback the web gives for a share's end: "Copied" for the
/// clipboard path, the failure note when only the link was copied.
pub fn share_feedback(result: ShareResult) -> Option<&'static str> {
    match result {
        ShareResult::Copied => Some(tr(keys::READER_COPIED)),
        ShareResult::LinkCopied => Some(tr(keys::READER_SHARE_FAILED)),
        ShareResult::Shared | ShareResult::Cancelled => None,
    }
}

/// `shareVerse`: the sheet with the verse text, reference and link.
pub fn share_verse(ctx: Option<&TranslationRef>, surah: u16, ayah: u16, text: &str) {
    let link = verse_link(ctx, surah, ayah);
    let body = verse_share_text(text, surah, ayah, &link);
    let title = verse_ref(surah, ayah);
    spawn(async move {
        if let Some(message) = share_feedback(share_or_copy(title, body, link).await) {
            notify(message);
        }
    });
}

/// `copyVerse`: text, reference, link to the clipboard.
pub fn copy_verse(ctx: Option<&TranslationRef>, surah: u16, ayah: u16, text: &str) {
    set_clipboard(verse_share_text(
        text,
        surah,
        ayah,
        &verse_link(ctx, surah, ayah),
    ));
}

fn problem_message(problem: RangeProblem) -> SharedString {
    match problem {
        RangeProblem::TooLong => t_args(keys::READER_SHARE_RANGE_TOO_LONG, || {
            vec![("max", SHARE_SPAN_MAX.into())]
        })(),
        RangeProblem::Invalid => tr(keys::READER_SHARE_RANGE_INVALID).into(),
    }
}

/// Reads `from..=to` of `surah` in `script`, as the rows show it.
async fn read_verses(
    script: ArabicSource,
    surah: u16,
    from: u16,
    to: u16,
) -> Option<Vec<(u16, String)>> {
    let m = meta();
    let (a, b) = (
        m.global_index_of(surah, from)?,
        m.global_index_of(surah, to)?,
    );
    let (range, _) = crate::app::quran_store()
        .read_source(script.id(), a, b)
        .await
        .ok()?;
    let groups = group_range(&range.ayahs, &range.normalizations, m).ok()?;
    Some(
        groups
            .into_iter()
            .flat_map(|g| g.verses)
            .map(|v| (v.ayah, v.body))
            .collect(),
    )
}

/// The range share dialog (`ShareDialog`), opened from verse `ayah`.
pub fn share_dialog(
    open: Signal<bool>,
    surah: u16,
    ayah: u16,
    ctx: Option<TranslationRef>,
    script: ArabicSource,
) -> impl IntoElement {
    dialog_card(
        open,
        tr(keys::READER_SHARE_DIALOG_TITLE).into(),
        move || open.set(false),
        move || share_body(open, surah, ayah, ctx.clone(), script),
        || view().into_element(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShareFormat {
    Text,
    Image,
}

fn card_arabic_font(script: ArabicSource) -> &'static [u8] {
    if script == ArabicSource::IndoPak {
        return INDOPAK_FONT;
    }
    HAFS_FONT
}

async fn write_card_to_data_dir(file_name: String, bytes: Vec<u8>) -> Option<PathBuf> {
    let dir = files::app_data_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("easyquran"))
        .join("share");
    blocking(move || {
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(file_name);
        std::fs::write(&path, bytes).ok()?;
        Some(path)
    })
    .await
    .ok()
    .flatten()
}

/// Renders and saves/shares the verse card PNG (`onDownload` in `ShareDialog.svelte`).
async fn save_or_share_card(
    script: ArabicSource,
    surah: u16,
    from: u16,
    to: u16,
    input: CardInput,
) {
    let arabic_font = card_arabic_font(script);
    let rendered = blocking(move || render_verse_card(&input, arabic_font, LATIN_FONT)).await;
    let Ok(Ok(bytes)) = rendered else {
        notify(tr(keys::READER_SHARE_IMAGE_FAILED));
        return;
    };
    let file_name = format!("easyquran-{surah}-{from}-{to}.png");
    let title = range_title(surah, from, to);

    if is_desktop() {
        let dialog = SaveDialog::new()
            .file_name(file_name.clone())
            .filter(FileFilter::new("PNG", ["png"]));
        match pick_save_path(dialog).await {
            Ok(Some(path)) => {
                let ok = blocking(move || std::fs::write(path, bytes).is_ok())
                    .await
                    .unwrap_or(false);
                notify(if ok {
                    tr(keys::READER_SHARE_IMAGE_SAVED)
                } else {
                    tr(keys::READER_SHARE_IMAGE_FAILED)
                });
                return;
            }
            Ok(None) => return,
            Err(_) => {
                // Save dialog unavailable (e.g. headless): fall through to data dir / share sheet.
            }
        }
    }

    let Some(path) = write_card_to_data_dir(file_name, bytes).await else {
        notify(tr(keys::READER_SHARE_IMAGE_FAILED));
        return;
    };
    if share_available().await {
        let request = ShareRequest::new().title(title).file(path);
        match share(request).await {
            Ok(outcome) if outcome.action == ShareAction::Dismissed => {}
            Ok(_) => notify(tr(keys::READER_SHARE_IMAGE_SAVED)),
            Err(_) => notify(tr(keys::READER_SHARE_IMAGE_FAILED)),
        }
        return;
    }
    notify(tr(keys::READER_SHARE_IMAGE_SAVED));
}

fn share_body(
    open: Signal<bool>,
    surah: u16,
    ayah: u16,
    ctx: Option<TranslationRef>,
    script: ArabicSource,
) -> nsh_core::element::AnyElement {
    let ayah_count = meta().surah(surah).map_or(0, |s| s.ayah_count);
    let from = signal(ayah.to_string());
    let to = signal(ayah.to_string());
    let format = signal(ShareFormat::Text);
    let busy = signal(false);
    let include_reference = signal(true);
    let include_link = signal(true);
    let verses = signal(None::<Vec<(u16, String)>>);
    let generation = stored(0_u64);
    let parse = |s: Signal<String>| s.get().trim().parse::<i64>().ok();
    let problem = move || range_problem(parse(from), parse(to), ayah_count);
    let bounds = move || {
        let f = u16::try_from(parse(from)?).ok()?;
        let t = u16::try_from(parse(to)?).ok()?;
        Some((f, t))
    };

    // The reader's text for each ayah in range; None while loading.
    effect(move || {
        let range = bounds().filter(|_| problem().is_none());
        let g = generation.with(|g| {
            *g += 1;
            *g
        });
        verses.set(None);
        let Some((f, t)) = range else {
            return;
        };
        spawn(async move {
            let read = read_verses(script, surah, f, t).await;
            if generation.get() == g {
                verses.set(read);
            }
        });
    });

    let link = {
        let ctx = ctx.clone();
        move || {
            bounds()
                .filter(|_| problem().is_none())
                .map(|(f, t)| range_link(ctx.as_ref(), surah, f, t))
        }
    };
    let copy_link = {
        let link = link.clone();
        move || match link() {
            Some(l) => {
                set_clipboard(l);
                notify(tr(keys::READER_COPIED));
            }
            None => notify(tr(keys::READER_SHARE_UNAVAILABLE)),
        }
    };
    let copy_text = {
        let link = link.clone();
        move || {
            let (Some(list), Some(l)) = (verses.get_untracked(), link()) else {
                return;
            };
            set_clipboard(range_share_text(
                surah,
                &list,
                &l,
                include_reference.get_untracked(),
                include_link.get_untracked(),
            ));
            notify(tr(keys::READER_COPIED));
            open.set(false);
        }
    };
    let download_image = move || {
        if busy.get_untracked() {
            return;
        }
        let (Some(list), Some((f, t_to)), Some(l)) = (verses.get_untracked(), bounds(), link())
        else {
            return;
        };
        let card_verses: Vec<CardVerse> = list
            .into_iter()
            .map(|(n, text)| CardVerse {
                text: plain(&text),
                reference: verse_ref(surah, n),
            })
            .collect();
        let input = CardInput {
            verses: card_verses,
            include_reference: include_reference.get_untracked(),
            link: include_link.get_untracked().then_some(l),
            brand: tr(keys::READER_SHARE_CARD_BRAND).to_owned(),
            footer: tr(keys::READER_SHARE_CARD_FOOTER).to_owned(),
        };
        busy.set(true);
        spawn(async move {
            save_or_share_card(script, surah, f, t_to, input).await;
            let _ = busy.try_set(false);
        });
    };
    let field = |label, value: Signal<String>| {
        view()
            .flex_1()
            .gap_1()
            .child(
                text(t(label))
                    .text_xs()
                    .text_color(tokens::foreground_secondary),
            )
            .child(numeric_field(value, ""))
    };
    let format_button = move |target: ShareFormat, label_key: eq_core::i18n::Key| {
        dynamic(move || {
            let active = format.get() == target;
            let variant = if active {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            };
            untracked(move || {
                button_with(
                    t(label_key),
                    variant,
                    ButtonSize::Small,
                    false,
                    None,
                    move || {
                        format.set(target);
                    },
                )
                .into_element()
            })
        })
    };
    view()
        .gap_4()
        .child(
            text(t(keys::READER_SHARE_DIALOG_DESCRIPTION))
                .text_sm()
                .text_color(tokens::foreground_secondary),
        )
        .child(
            view()
                .flex_row()
                .gap_3()
                .child(field(keys::READER_SHARE_FROM, from))
                .child(field(keys::READER_SHARE_TO, to)),
        )
        .child(dynamic(move || {
            if let Some(p) = problem() {
                return text(problem_message(p))
                    .text_xs()
                    .text_color(tokens::danger)
                    .into_element();
            }
            if verses.with(Option::is_none) {
                return text(t(keys::READER_SHARE_LOADING))
                    .text_xs()
                    .text_color(tokens::muted_text)
                    .into_element();
            }
            view().into_element()
        }))
        .child(
            view()
                .gap_2()
                .child(
                    text(t(keys::READER_SHARE_FORMAT))
                        .text_xs()
                        .text_color(tokens::foreground_secondary),
                )
                .child(
                    view()
                        .flex_row()
                        .gap_2()
                        .child(format_button(
                            ShareFormat::Text,
                            keys::READER_SHARE_FORMAT_TEXT,
                        ))
                        .child(format_button(
                            ShareFormat::Image,
                            keys::READER_SHARE_FORMAT_IMAGE,
                        )),
                ),
        )
        .child(
            view()
                .gap_2()
                .child(checkbox_row(
                    tr(keys::READER_SHARE_INCLUDE_REFERENCE),
                    include_reference,
                ))
                .child(checkbox_row(
                    tr(keys::READER_SHARE_INCLUDE_LINK),
                    include_link,
                )),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .justify_end()
                .gap_2()
                .child(button_with(
                    t(keys::READER_SHARE_COPY_LINK),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    move || problem().is_some(),
                    None,
                    copy_link,
                ))
                .child(when(
                    move || format.get() == ShareFormat::Text,
                    move || {
                        let copy_text = copy_text.clone();
                        button_with(
                            t(keys::READER_SHARE_COPY),
                            ButtonVariant::Primary,
                            ButtonSize::Small,
                            move || verses.with(Option::is_none),
                            None,
                            copy_text,
                        )
                        .into_element()
                    },
                ))
                .child(when(
                    move || format.get() == ShareFormat::Image,
                    move || {
                        let download_image = download_image.clone();
                        button_with(
                            t(keys::READER_SHARE_DOWNLOAD),
                            ButtonVariant::Primary,
                            ButtonSize::Small,
                            move || verses.with(Option::is_none) || busy.get(),
                            None,
                            download_image,
                        )
                        .into_element()
                    },
                )),
        )
        .into_element()
}
