//! Curated verse snippets (`web/src/lib/data/snippets.ts`): azkar, duas and
//! essential verses as *references only* (never Quran text). Baked from the
//! web file at compile time (`include_str!`) and parsed once, so the two
//! lists cannot drift. Editorial text is bilingual (`en`/`ar`) in the data.

use std::sync::LazyLock;

const SOURCE: &str = include_str!("../../../../../web/src/lib/data/snippets.ts");

/// `essential` | `azkar` | `dua`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnippetKind {
    Essential,
    Azkar,
    Dua,
}

impl SnippetKind {
    /// Display order of the azkar index sections (`SNIPPET_KIND_ORDER`).
    pub const ORDER: [SnippetKind; 3] =
        [SnippetKind::Essential, SnippetKind::Azkar, SnippetKind::Dua];

    fn parse(s: &str) -> Option<Self> {
        match s {
            "essential" => Some(SnippetKind::Essential),
            "azkar" => Some(SnippetKind::Azkar),
            "dua" => Some(SnippetKind::Dua),
            _ => None,
        }
    }
}

/// One verse range of a snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnippetRef {
    pub surah: u16,
    pub ayah_start: u16,
    pub ayah_end: u16,
}

/// Bilingual editorial text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetText {
    pub en: String,
    pub ar: String,
}

/// One curated snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub slug: String,
    pub kind: SnippetKind,
    pub title: SnippetText,
    pub description: SnippetText,
    pub refs: Vec<SnippetRef>,
}

static SNIPPETS: LazyLock<Vec<Snippet>> = LazyLock::new(|| parse(SOURCE));

/// Every snippet, in the web's order.
pub fn snippets() -> &'static [Snippet] {
    &SNIPPETS
}

/// The snippet with `slug` (`snippetForSlug`); unknown slugs are not snippets.
pub fn snippet_for_slug(slug: &str) -> Option<&'static Snippet> {
    SNIPPETS.iter().find(|s| s.slug == slug)
}

/// The double-quoted string starting after `key` in `text`, unescaped.
fn quoted_after<'a>(text: &'a str, key: &str) -> Option<(String, &'a str)> {
    let start = text.find(key)? + key.len();
    let rest = text[start..].trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &rest[i + 1..])),
            '\\' => {
                if let Some((_, escaped)) = chars.next() {
                    out.push(escaped);
                }
            }
            c => out.push(c),
        }
    }
    None
}

fn number_after(text: &str, key: &str) -> Option<u16> {
    let start = text.find(key)? + key.len();
    let digits: String = text[start..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn text_block(text: &str, key: &str) -> Option<SnippetText> {
    let block = &text[text.find(key)?..];
    let (en, rest) = quoted_after(block, "en:")?;
    let (ar, _) = quoted_after(rest, "ar:")?;
    Some(SnippetText { en, ar })
}

fn parse_entry(chunk: &str) -> Option<Snippet> {
    let (slug, _) = quoted_after(chunk, "slug:")?;
    let (kind, _) = quoted_after(chunk, "kind:")?;
    let refs_at = chunk.find("refs:")?;
    let refs = chunk[refs_at..]
        .split("surah:")
        .skip(1)
        .filter_map(|r| {
            let r = format!("surah:{r}");
            Some(SnippetRef {
                surah: number_after(&r, "surah:")?,
                ayah_start: number_after(&r, "ayahStart:")?,
                ayah_end: number_after(&r, "ayahEnd:")?,
            })
        })
        .collect();
    Some(Snippet {
        slug,
        kind: SnippetKind::parse(&kind)?,
        title: text_block(chunk, "title:")?,
        description: text_block(chunk, "description:")?,
        refs,
    })
}

/// Parses the `SNIPPETS` array of the web module.
fn parse(source: &str) -> Vec<Snippet> {
    let Some(start) = source.find("export const SNIPPETS") else {
        return Vec::new();
    };
    let body = &source[start..];
    let body = &body[..body.find("\n];").unwrap_or(body.len())];
    let mut chunks: Vec<&str> = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("slug:") {
        let next = rest[at + 5..].find("slug:").map(|n| at + 5 + n);
        let end = next.unwrap_or(rest.len());
        chunks.push(&rest[at..end]);
        rest = &rest[end..];
    }
    chunks.into_iter().filter_map(parse_entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bakes_every_web_snippet() {
        let entries = SOURCE.matches("    slug: \"").count();
        assert!(entries > 0);
        assert_eq!(snippets().len(), entries, "every entry parses");
        let kursi = snippet_for_slug("ayat-al-kursi").expect("ayat al-kursi");
        assert_eq!(kursi.kind, SnippetKind::Essential);
        assert_eq!(
            kursi.refs,
            vec![SnippetRef {
                surah: 2,
                ayah_start: 255,
                ayah_end: 255
            }]
        );
        assert!(snippet_for_slug("nope").is_none());
    }
}
