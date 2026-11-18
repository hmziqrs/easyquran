//! Generator `i18n`: the web's inlang catalogues into one static table.
//!
//! Sources, read by path relative to this crate (never copied):
//! `web/project.inlang/settings.json` names the locales and the
//! `plugin.inlang.messageFormat.pathPattern` files; each `{locale}` file set
//! is merged into one key space per locale, as inlang does. A key defined in
//! two files of one locale, a key in a non-base locale that the base lacks,
//! a missing file, a key that is not a Rust-friendly snake_case name, or a
//! message shape this generator does not understand stops the build.
//!
//! Output `OUT_DIR/i18n_table.rs`, included by `src/i18n/table.rs`:
//! `KEY_NAMES` (sorted), `EN`, `AR` (`None` where Arabic lacks the key),
//! `AR_MISSING`, and `pub mod keys` with one `Key` const per message.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

/// The locales the Rust side models (`Locale::{En, Ar}`), base first.
const LOCALES: [&str; 2] = ["en", "ar"];

pub fn generate(crate_dir: &Path, out_dir: &Path) -> Result<(), String> {
    let web = crate_dir.join("../../../web");
    let settings_path = web.join("project.inlang/settings.json");
    let settings = read_json(&settings_path)?;
    let (base, locales, patterns) = parse_settings(&settings, &settings_path)?;
    if base != LOCALES[0] || locales != LOCALES {
        return Err(format!(
            "{}: locales {locales:?} (base {base}) differ from the Rust side's {LOCALES:?}; \
             update eq_core::i18n::Locale and this generator together",
            settings_path.display()
        ));
    }

    let mut catalogues: Vec<BTreeMap<String, Message>> = Vec::new();
    for locale in LOCALES {
        let mut merged: BTreeMap<String, Message> = BTreeMap::new();
        let mut origin: BTreeMap<String, PathBuf> = BTreeMap::new();
        for pattern in &patterns {
            let path = web.join(pattern.trim_start_matches("./").replace("{locale}", locale));
            println!("cargo:rerun-if-changed={}", path.display());
            let json = read_json(&path)?;
            let Value::Object(map) = json else {
                return Err(format!("{}: top level is not an object", path.display()));
            };
            for (key, value) in map {
                if key.starts_with('$') {
                    continue;
                }
                check_key(&key, &path)?;
                if let Some(first) = origin.get(&key) {
                    return Err(format!(
                        "duplicate message key `{key}` in locale `{locale}`: {} and {}",
                        first.display(),
                        path.display()
                    ));
                }
                let message = parse_message(&key, &value, &path)?;
                origin.insert(key.clone(), path.clone());
                merged.insert(key, message);
            }
        }
        catalogues.push(merged);
    }

    let [en, ar] = <[BTreeMap<String, Message>; 2]>::try_from(catalogues)
        .map_err(|_| "expected exactly two catalogues".to_owned())?;
    if let Some(extra) = ar.keys().find(|k| !en.contains_key(*k)) {
        return Err(format!(
            "key `{extra}` exists in `ar` but not in the base locale `en`"
        ));
    }

    let code = emit(&en, &ar)?;
    fs::write(out_dir.join("i18n_table.rs"), code)
        .map_err(|e| format!("writing i18n_table.rs: {e}"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| {
        format!(
            "cannot read {} ({e}). The native i18n catalogue is generated from the web \
             catalogues; the web/ tree must sit beside native/ in the monorepo.",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|e| format!("{}: invalid JSON: {e}", path.display()))
}

fn parse_settings(
    settings: &Value,
    path: &Path,
) -> Result<(String, Vec<String>, Vec<String>), String> {
    let err = |what: &str| format!("{}: {what}", path.display());
    let base = settings["baseLocale"]
        .as_str()
        .ok_or_else(|| err("no baseLocale"))?
        .to_owned();
    let locales = settings["locales"]
        .as_array()
        .ok_or_else(|| err("no locales"))?
        .iter()
        .map(|l| {
            l.as_str()
                .map(str::to_owned)
                .ok_or_else(|| err("locale not a string"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let patterns = match &settings["plugin.inlang.messageFormat"]["pathPattern"] {
        Value::String(one) => vec![one.clone()],
        Value::Array(many) => many
            .iter()
            .map(|p| {
                p.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| err("pathPattern entry not a string"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(err("no plugin.inlang.messageFormat.pathPattern")),
    };
    Ok((base, locales, patterns))
}

fn check_key(key: &str, path: &Path) -> Result<(), String> {
    let mut chars = key.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    if first_ok && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Ok(());
    }
    Err(format!(
        "{}: key `{key}` is not snake_case ([a-z][a-z0-9_]*); the generated `keys::` const needs one",
        path.display()
    ))
}

/// One message: plain text, or a match over plural / exact selectors.
enum Message {
    Text(String),
    Match {
        /// (input name, whether it goes through `plural`).
        selectors: Vec<(String, bool)>,
        /// (one value per selector, `*` for any; the text).
        variants: Vec<(Vec<String>, String)>,
    },
}

impl Message {
    /// The text an English reader of the generated docs sees.
    fn doc(&self) -> String {
        match self {
            Message::Text(text) => text.clone(),
            Message::Match { variants, .. } => variants
                .iter()
                .map(|(when, text)| format!("({}) {text}", when.join(",")))
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }
}

fn parse_message(key: &str, value: &Value, path: &Path) -> Result<Message, String> {
    let err = |what: String| format!("{}: message `{key}`: {what}", path.display());
    let entry = match value {
        Value::String(text) => return Ok(Message::Text(text.clone())),
        Value::Array(entries) if entries.len() == 1 => &entries[0],
        _ => return Err(err("expected a string or a one-element array".into())),
    };
    let declarations = entry["declarations"]
        .as_array()
        .ok_or_else(|| err("no declarations".into()))?;
    let mut inputs: BTreeSet<String> = BTreeSet::new();
    // local name -> (input, plural)
    let mut locals: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for declaration in declarations {
        let declaration = declaration
            .as_str()
            .ok_or_else(|| err("declaration not a string".into()))?
            .trim();
        if let Some(input) = declaration.strip_prefix("input ") {
            inputs.insert(input.trim().to_owned());
        } else if let Some(local) = declaration.strip_prefix("local ") {
            let (name, expr) = local
                .split_once('=')
                .ok_or_else(|| err(format!("cannot read `{declaration}`")))?;
            let (input, function) = expr
                .split_once(':')
                .ok_or_else(|| err(format!("cannot read `{declaration}`")))?;
            let function = function.trim();
            if function != "plural" {
                return Err(err(format!(
                    "function `{function}` unsupported (only `plural`); extend gen/i18n.rs"
                )));
            }
            locals.insert(name.trim().to_owned(), (input.trim().to_owned(), true));
        } else {
            return Err(err(format!("cannot read declaration `{declaration}`")));
        }
    }
    let selectors = entry["selectors"]
        .as_array()
        .ok_or_else(|| err("no selectors".into()))?
        .iter()
        .map(|s| {
            let name = s
                .as_str()
                .ok_or_else(|| err("selector not a string".into()))?;
            if let Some(local) = locals.get(name) {
                return Ok((name.to_owned(), local.clone()));
            }
            if inputs.contains(name) {
                return Ok((name.to_owned(), (name.to_owned(), false)));
            }
            Err(err(format!("selector `{name}` is not declared")))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let Value::Object(matches) = &entry["match"] else {
        return Err(err("no match object".into()));
    };
    let mut variants = Vec::new();
    for (condition, text) in matches {
        let text = text
            .as_str()
            .ok_or_else(|| err(format!("variant `{condition}` is not a string")))?;
        let mut when = vec![String::from("*"); selectors.len()];
        for part in condition.split(',') {
            let (name, value) = part
                .trim()
                .split_once('=')
                .ok_or_else(|| err(format!("cannot read variant `{condition}`")))?;
            let index = selectors
                .iter()
                .position(|(s, _)| s == name.trim())
                .ok_or_else(|| err(format!("variant `{condition}` names an unknown selector")))?;
            when[index] = value.trim().to_owned();
        }
        variants.push((when, text.to_owned()));
    }
    if variants.is_empty() {
        return Err(err("no variants".into()));
    }
    Ok(Message::Match {
        selectors: selectors.into_iter().map(|(_, s)| s).collect(),
        variants,
    })
}

fn lit(s: &str) -> String {
    format!("{s:?}")
}

fn emit_message(message: &Message) -> String {
    match message {
        Message::Text(text) => format!("Msg::Text({})", lit(text)),
        Message::Match {
            selectors,
            variants,
        } => {
            let mut out = String::from("Msg::Match(&Match { selectors: &[");
            for (input, plural) in selectors {
                let _ = write!(
                    out,
                    "Selector {{ input: {}, plural: {plural} }}, ",
                    lit(input)
                );
            }
            out.push_str("], variants: &[");
            for (when, text) in variants {
                out.push_str("Variant { when: &[");
                for w in when {
                    let _ = write!(out, "{}, ", lit(w));
                }
                let _ = write!(out, "], text: {} }}, ", lit(text));
            }
            out.push_str("] })");
            out
        }
    }
}

fn emit(en: &BTreeMap<String, Message>, ar: &BTreeMap<String, Message>) -> Result<String, String> {
    let n = en.len();
    if n > usize::from(u16::MAX) {
        return Err("more messages than a u16 key holds".into());
    }
    let mut out = String::new();
    out.push_str("// @generated by eq-core/gen/i18n.rs from web/messages/**. Do not edit.\n");
    let _ = writeln!(out, "pub(crate) const KEY_COUNT: usize = {n};");

    let _ = write!(out, "pub(crate) static KEY_NAMES: [&str; {n}] = [");
    for key in en.keys() {
        let _ = write!(out, "{},", lit(key));
    }
    out.push_str("];\n");

    let _ = write!(out, "pub(crate) static EN: [Msg; {n}] = [");
    for message in en.values() {
        let _ = writeln!(out, "{},", emit_message(message));
    }
    out.push_str("];\n");

    let _ = write!(out, "pub(crate) static AR: [Option<Msg>; {n}] = [");
    let mut missing = Vec::new();
    for key in en.keys() {
        match ar.get(key) {
            Some(message) => {
                let _ = writeln!(out, "Some({}),", emit_message(message));
            }
            None => {
                missing.push(key.clone());
                out.push_str("None,\n");
            }
        }
    }
    out.push_str("];\n");

    let _ = write!(
        out,
        "pub(crate) static AR_MISSING: [&str; {}] = [",
        missing.len()
    );
    for key in &missing {
        let _ = write!(out, "{},", lit(key));
    }
    out.push_str("];\n");

    out.push_str(
        "/// One const per web message key, upper-cased (`nav_home` -> `NAV_HOME`).\n\
         /// An unknown key fails to compile. Each const's doc is the English text.\n\
         pub mod keys {\n    use super::Key;\n",
    );
    for (index, (key, message)) in en.iter().enumerate() {
        let _ = writeln!(
            out,
            "    #[doc = {}]\n    pub const {}: Key = Key({index});",
            lit(&format!("`{key}`: {}", message.doc())),
            key.to_ascii_uppercase()
        );
    }
    out.push_str("}\n");
    Ok(out)
}
