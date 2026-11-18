//! The remote screen: the signed TSX package's delivery status, the
//! revocation control that models a host applying a revocation list, and the
//! embedding contract an NSH 0.5.0 host must provide to run the package. Owned
//! by the remote-screen area. The shell mounts `remote_screen_page`; keep
//! its name and signature.

mod delivery;

pub use delivery::state_path;

use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use delivery::{DeliveryState, read_delivery_state};
use nsh_base::theme;
use nsh_core::prelude::*;

/// The page the shell mounts. The delivery facts are read once, at build;
/// another `scripts/delivery.sh` run changes them only on the next visit.
pub fn remote_screen_page() -> impl IntoElement {
    let state = signal(read_delivery_state());
    let revoked = signal(false);
    scroll()
        .flex_1()
        .content_inset(crate::ui::layout::page_insets(0.0))
        .bg(move || theme().background)
        .child(
            crate::ui::layout::page_body()
                .child(page_title())
                .child(status_section(state, revoked))
                .child(facts_section(state))
                .child(revoke_section(revoked, state))
                .child(embedding_section()),
        )
}

fn page_title() -> impl IntoElement {
    crate::ui::text::page_intro(
        "Remote screen",
        "The signed TSX package in packages/remote-screen",
    )
}

fn status_section(state: Signal<Option<DeliveryState>>, revoked: Signal<bool>) -> impl IntoElement {
    let delivery_status = move || match (state.get(), revoked.get()) {
        (None, _) => "Not built",
        (_, true) => "Revoked",
        (Some(_), false) => "Verified",
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            text("Delivery status")
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
        .child(
            text(delivery_status)
                .text_sm()
                .text_color(move || match delivery_status() {
                    "Revoked" => theme().destructive,
                    "Not built" => theme().muted_foreground,
                    _ => theme().foreground,
                }),
        )
}

fn facts_section(state: Signal<Option<DeliveryState>>) -> impl IntoElement {
    view()
        .when_some(state.get(), |facts, s| {
            facts.child(
                view()
                    .gap_2()
                    .p_4()
                    .rounded(move || theme().radius)
                    .border_1()
                    .border_color(move || theme().border)
                    .bg(move || theme().card)
                    .children(vec![
                        fact_row(
                            "Package",
                            format!(
                                "{} {} (build {})",
                                s.package.id, s.package.version, s.package.build
                            ),
                        ),
                        fact_row("SHA-256", short_sha(&s.package.sha256)),
                        fact_row(
                            "Signed by",
                            format!("release key {}", s.signature.release_key_id),
                        ),
                        fact_row(
                            "Trust",
                            format!(
                                "root {}, certificate for {} ({} days)",
                                s.signature.root_key_id,
                                s.signature.certified_apps,
                                s.signature.cert_days
                            ),
                        ),
                        fact_row(
                            "Verified",
                            format!(
                                "nsh verify --root root.pub: valid, {} bytes, for runtime {} or newer",
                                s.package.bytes, s.signature.min_runtime
                            ),
                        ),
                        fact_row(
                            "Revocation",
                            format!("list {}: {}", s.revocation.sequence, s.revocation.verify_error),
                        ),
                    ]),
            )
        })
        .when(state.get().is_none(), |view| {
            view.child(
                text("No delivery record yet. Run packages/remote-screen/scripts/delivery.sh to key, publish, verify and revoke the package.")
                    .text_sm()
                    .text_color(move || theme().muted_foreground),
            )
        })
}

fn fact_row(label: &'static str, value: String) -> impl IntoElement {
    view()
        .flex_row()
        .items_start()
        .gap_3()
        .child(
            text(label)
                .text_sm()
                .w_28()
                .text_color(move || theme().muted_foreground),
        )
        .child(
            text(value)
                .flex_1()
                .text_sm()
                .text_color(move || theme().foreground),
        )
}

fn short_sha(sha: &str) -> String {
    format!("{}…{}", &sha[..12], &sha[sha.len() - 6..])
}

/// What a host does when it fetches and applies the newest revocation list,
/// before it opens a package: the build is refused from then on.
fn revoke_section(revoked: Signal<bool>, state: Signal<Option<DeliveryState>>) -> impl IntoElement {
    view()
        .gap_2()
        .child(button_with(
            "Apply revocation list",
            ButtonVariant::Danger,
            ButtonSize::Medium,
            move || revoked.get() || state.get().is_none(),
            // The visible label names it; `name` is for when it would not.
            None,
            move || revoked.set(true),
        )
        .self_start())
        .child(
            text(move || {
                if revoked.get() {
                    "This build is refused from now on: verification names the check, and the running app stays as it was. Lists are numbered, so an old list cannot be replayed to undo this."
                } else {
                    "Applying the list is what a host does before opening a package, having fetched it separately from the app."
                }
            })
            .text_sm()
            .text_color(move || theme().muted_foreground),
        )
}

/// Why the page documents a host instead of embedding the package itself,
/// and the error contract of the `host.call` seam the package already uses.
fn embedding_section() -> impl IntoElement {
    view()
        .gap_2()
        .p_4()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .child(
            text("Loading this package")
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(
            text("NSH 0.5.0 runs a verified package in a surface of its own: nsh_engine::load_package verifies against a TrustStore and hands its bundle to App::Js. This app's one surface runs its Rust root, and no element embeds a second engine in a page tree, so displaying the package needs an embedding host: NshAppView.loadPackage on iOS, NshView.runPackage on Android, or AppView::load(App::Js(bundle)) in a Rust host. tests/remote_screen.rs loads the published package into a headless engine of this crate, so the mixed path stays proven.")
                .flex_1()
                .text_sm()
                .text_color(move || theme().foreground),
        )
        .child(
            text("The package calls its host over host.call: it declares capabilities.host \"remote.*\" and calls remote.ping. A failed call rejects with:")
                .flex_1()
                .text_sm()
                .text_color(move || theme().foreground),
        )
        .child(
            text("ERR_ACCESS_DENIED - the name is not in capabilities.host.")
                .flex_1()
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
        .child(
            text("ERR_NSH_HOST_ERROR - the host has no handler for the name, or its handler threw.")
                .flex_1()
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
        .child(
            text("ERR_FEATURE_UNAVAILABLE_ON_PLATFORM - nothing embeds the app.")
                .flex_1()
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}
