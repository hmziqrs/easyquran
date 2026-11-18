//! The app's [`CardCatalog`]: what home-card validation may reference
//! (web `cards.svelte.ts`: `TRANSLATION_CATALOGUE_BY_ID` for `sourceId`,
//! `snippetForSlug` for snippet slugs).

use crate::{quran::catalogue, store::home::CardCatalog};

use super::snippets::snippet_for_slug;

/// The baked translation catalogue plus the baked snippet list.
#[derive(Debug, Default, Clone, Copy)]
pub struct AppCatalog;

impl CardCatalog for AppCatalog {
    /// A card's pinned translation: a catalogue translation id (Arabic is
    /// `null` on the wire, never an id).
    fn known_source(&self, source_id: &str) -> bool {
        catalogue::translation(source_id).is_some()
    }

    fn known_snippet(&self, slug: &str) -> bool {
        snippet_for_slug(slug).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_snippets_and_rejects_unknown_ids() {
        assert!(AppCatalog.known_snippet("ayat-al-kursi"));
        assert!(!AppCatalog.known_snippet("not-a-snippet"));
        assert!(!AppCatalog.known_source("no-such-translation"));
        let first = catalogue::translations().first().expect("catalogue");
        assert!(AppCatalog.known_source(&first.id));
    }
}
