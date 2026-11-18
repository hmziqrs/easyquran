//! Toolbar icons: Lucide (ISC) outlines stroked in `currentColor`, so the
//! caller's `text_color` tints them.

macro_rules! lucide {
    ($body:literal) => {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><g fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2">"#,
            $body,
            "</g></svg>"
        )
    };
}
pub(crate) use lucide;

/// `panel-left`: shows or hides the sidebar.
pub const SIDEBAR: &str =
    lucide!(r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>"#);

/// `chevron-left`: back.
pub const BACK: &str = lucide!(r#"<path d="m15 18-6-6 6-6"/>"#);

/// `search`: the command palette.
pub const SEARCH: &str = lucide!(r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#);
