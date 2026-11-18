//! Images and video: an asset image behind a placeholder with a fade, a
//! blur-up (the tiny committed copy blurred while the full one shows),
//! and a local MP4 the app ships, played with controls of the page's own.
//! All sources are committed assets: nothing here needs a server.

use nsh_base::theme;
use nsh_core::{
    element::{FileAsset, Image, ImageAsset, asset},
    prelude::*,
};

use super::{caption, section, tool_button};

/// The full photo (128 × 64).
const PHOTO: ImageAsset = asset!("canvas/photo.png");

/// The same picture at 8 × 4: the blur-up's source.
const PHOTO_TINY: ImageAsset = asset!("canvas/photo-tiny.png");

/// The placeholder shown while an image loads.
const PLACEHOLDER: ImageAsset = asset!("canvas/placeholder.png");

/// A two-second clip the crate ships and plays from its bundle.
const CLIP: FileAsset = asset!("canvas/loop.mp4");

/// `seconds` as `m:ss`.
fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The frame an image sits in.
fn frame(image: Image) -> Image {
    image
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted)
}

/// Images: the placeholder-and-fade pair and the blur-up pair.
#[component]
fn Images() -> impl IntoElement {
    view()
        .gap_3()
        .child(
            view()
                .gap_2()
                .child(
                    frame(image(PHOTO))
                        .placeholder(PLACEHOLDER)
                        .fade_duration(250.0)
                        .object_fit(ResizeMode::Cover)
                        .w_full()
                        .h(px(110.0)),
                )
                .child(caption("image-placeholder-note", move || {
                    "placeholder while loading, a 250 ms fade in".to_owned()
                })),
        )
        .child(
            view()
                .flex_row()
                .gap_3()
                .child(
                    view()
                        .flex_1()
                        .gap_2()
                        .child(
                            frame(image(PHOTO_TINY))
                                .object_fit(ResizeMode::Cover)
                                .blur_radius(px(4.0))
                                .w_full()
                                .h(px(76.0)),
                        )
                        .child(caption("blur-up-tiny", move || "8 × 4, blurred".to_owned())),
                )
                .child(
                    view()
                        .flex_1()
                        .gap_2()
                        .child(
                            frame(image(PHOTO))
                                .object_fit(ResizeMode::Cover)
                                .w_full()
                                .h(px(76.0)),
                        )
                        .child(caption("blur-up-full", move || {
                            "128 × 64, sharp".to_owned()
                        })),
                ),
        )
}

/// The video: a bundled MP4 with the page's own play controls, honest
/// about errors (a platform that cannot play the clip says so here).
#[component]
fn Player() -> impl IntoElement {
    let playing = signal(false);
    let time = signal(0.0_f32);
    let duration = signal(0.0_f32);
    let error = signal(None::<String>);
    let loaded = computed(move || duration.get() > 0.0 || error.get().is_some());
    let readout = move || match error.get() {
        Some(message) => message,
        None => format!("{} / {}", clock(time.get()), clock(duration.get())),
    };
    view()
        .gap_3()
        .child(
            video(CLIP.uri)
                .paused(move || !playing.get())
                .muted(true)
                .repeat(true)
                .controls(false)
                .test_id("video")
                .label("Bundled clip")
                .on_load(move |info: &VideoInfo| {
                    batch(|| {
                        duration.set(info.duration);
                        error.set(None);
                    });
                })
                .on_progress(move |e: &VideoProgress| {
                    batch(|| {
                        time.set(e.current_time);
                        duration.set(e.duration);
                    });
                })
                .on_error(move |message: &str| error.set(Some(message.to_owned())))
                .rounded(move || theme().radius)
                .overflow_hidden()
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().muted)
                .h(px(120.0))
                .w_full(),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .items_center()
                .child(view().test_id("video-play").child(tool_button(
                    move || {
                        if playing.get() { "Pause" } else { "Play" }
                    },
                    move || !loaded.get(),
                    move || playing.update(|p| *p = !*p),
                )))
                .child(caption("video-time", readout)),
        )
}

/// The media section.
#[component]
pub(crate) fn Media() -> impl IntoElement {
    view().child(section(
        "Images and video",
        "asset images · bundled MP4",
        view()
            .gap_4()
            .child(images())
            .child(player())
            .child(caption("clip-note", move || {
                "the clip plays from the app bundle; no server".to_owned()
            })),
    ))
}
