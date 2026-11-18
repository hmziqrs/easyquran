//! `web_view`: an inline HTML page with messages both ways — the page
//! posts with `window.nsh.postMessage`, the app answers through the ref's
//! `post_message`. The manifest grants `"webview": { "hosts": [] }`, and
//! inline HTML needs the grant alone (`allows_webviews`, access.rs:1308):
//! the hosts list only constrains http/https navigations.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{caption, section, tool_button};

/// `color` as CSS's `#rrggbb`, for the page's own stylesheet.
fn css(color: Hsla) -> String {
    color.alpha(1.0).to_hex().to_ascii_lowercase()
}

/// The inline page in the app's tokens for the appearance shown; it
/// reloads when the appearance changes.
fn page() -> String {
    let t = theme();
    let (inset, text, muted, border) = (
        css(color(t.muted)),
        css(color(t.foreground)),
        css(color(t.muted_foreground)),
        css(color(t.border)),
    );
    format!(
        r#"<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1">
<style>
  body {{ margin: 0; padding: 16px; background: {inset}; color: {text}; font-family: -apple-system, system-ui, sans-serif; font-size: 15px; line-height: 1.4; }}
  h3 {{ margin: 0 0 4px; font-size: 16px; font-weight: 600; }}
  p {{ margin: 0 0 12px; color: {muted}; font-size: 14px; }}
  button {{ height: 44px; padding: 0 16px; border: 1px solid {border}; border-radius: 10px; background: transparent; color: {text}; font-family: inherit; font-size: 14px; }}
  #got {{ margin-top: 12px; padding: 10px 12px; border-radius: 10px; border: 1px solid {border}; color: {muted}; font-size: 14px; }}
</style></head>
<body>
<h3>An inline page</h3>
<p>HTML from a string. It sends messages to the app and receives them.</p>
<button onclick="window.nsh.postMessage('hello from the page')">Post to the app</button>
<div id="got">No message from the app yet.</div>
<script>window.addEventListener('message', (e) => {{ document.getElementById('got').textContent = 'From the app: ' + e.data; }});</script>
</body></html>"#
    )
}

/// The messaging section: the web view, the last message each way, and
/// the button that posts to the page.
#[component]
pub(crate) fn Messaging() -> impl IntoElement {
    let web = element_ref();
    // What the page posted last, and how many posts the app sent.
    let message = signal(String::new());
    let sent = signal(0_u32);
    let from_page = move || {
        if message.get().is_empty() {
            "—".to_owned()
        } else {
            message.get()
        }
    };
    let to_page = move || {
        if sent.get() > 0 {
            format!("hi #{}", sent.get())
        } else {
            "—".to_owned()
        }
    };
    view().child(section(
        "Web view",
        "inline HTML · messages both ways",
        view()
            .gap_3()
            .child(
                web_view_html(page, None)
                    .element_ref(&web)
                    .h(px(190.0))
                    .rounded(move || theme().radius)
                    .overflow_hidden()
                    .border_1()
                    .border_color(move || theme().border)
                    .bg(move || theme().muted)
                    .on_message(move |data: &str| message.set(data.to_owned())),
            )
            .child(
                view()
                    .gap_1()
                    .child(caption("from-page", from_page))
                    .child(caption("to-page", to_page)),
            )
            .child(view().test_id("post-to-page").child(tool_button(
                "Post to the page",
                false,
                move || {
                    let n = sent.get() + 1;
                    sent.set(n);
                    web.post_message(format!("hi #{n}"));
                },
            ))),
    ))
}
