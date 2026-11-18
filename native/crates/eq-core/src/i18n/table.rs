//! The catalogue `gen/i18n.rs` generates from `web/messages/**`, and the
//! shapes it is written in.

use super::Key;

/// One message as the inlang catalogue has it.
#[derive(Debug)]
pub(crate) enum Msg {
    /// Text with `{param}` placeholders.
    Text(&'static str),
    /// A match over selectors (plural categories), first matching variant
    /// wins.
    Match(&'static Match),
}

#[derive(Debug)]
pub(crate) struct Match {
    pub(crate) selectors: &'static [Selector],
    pub(crate) variants: &'static [Variant],
}

/// What a variant's value is compared with: the plural category of input
/// `input` (`local xPlural = x: plural`), or the input itself.
#[derive(Debug)]
pub(crate) struct Selector {
    pub(crate) input: &'static str,
    pub(crate) plural: bool,
}

/// One value per selector (`*` matches any) and the text it shows.
#[derive(Debug)]
pub(crate) struct Variant {
    pub(crate) when: &'static [&'static str],
    pub(crate) text: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/i18n_table.rs"));
