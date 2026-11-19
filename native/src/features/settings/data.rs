//! Your data (PARITY J7; web `DataSection.svelte`, `data-export.ts`):
//! export everything this device holds to one JSON file, import a file
//! from another device (adds only what is missing; settings applied whole
//! with consent off).
//!
//! Export: the system save dialog on the desktops. Phones have no save
//! dialog, so the file is written to `${dataDir}/exports/` and handed to
//! the system share sheet (save to Files, send to another app); where no
//! sheet shows, a toast names the file.

use std::collections::HashSet;

use eq_core::{
    home::catalog::AppCatalog,
    i18n::{self, keys},
    store::export::{
        ExportNote, NotesPort, ParseFailure, apply_import, build_export, parse_export,
    },
    sync::clock::iso_from_ms,
};
use nsh_core::{
    element::AnyElement,
    file_dialog::{FileFilter, SaveDialog},
    picker::FilePickRequest,
    prelude::*,
    share::ShareRequest,
};

use super::section_frame;
use crate::{
    app::sync_store,
    features::downloads::rows::{caption, list_card, row_frame, row_label},
    services::files,
    shell::routes::SettingsSection,
    stores::{
        locale::{locale, t, tr},
        notes::notes_store,
        settings::{apply_document, document},
        sync::Area,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        toast::notify,
    },
};

fn now_ms() -> i64 {
    nsh_core::time::unix_ms() as i64
}

fn json_filter() -> FileFilter {
    FileFilter::new("JSON", ["json"])
}

/// Notes the import adds, collected off the engine thread and written
/// through the notes store after.
struct CollectNotes {
    existing: HashSet<(i64, i64)>,
    added: Vec<ExportNote>,
}

impl NotesPort for CollectNotes {
    fn note(&self, surah: i64, ayah: i64) -> String {
        if self.existing.contains(&(surah, ayah))
            || self
                .added
                .iter()
                .any(|n| (n.surah, n.ayah) == (surah, ayah))
        {
            return "-".to_owned();
        }
        String::new()
    }

    fn set_note(&mut self, surah: i64, ayah: i64, text: &str, source_id: Option<&str>) {
        self.added.push(ExportNote {
            surah,
            ayah,
            text: text.to_owned(),
            source_id: source_id.map(str::to_owned),
            updated_at: None,
        });
    }
}

async fn save(bytes: Vec<u8>, name: String) {
    if is_desktop() {
        let dialog = SaveDialog::new()
            .file_name(name.clone())
            .filter(json_filter());
        match pick_save_path(dialog).await {
            Ok(Some(path)) => {
                let written = blocking(move || std::fs::write(&path, &bytes)).await;
                if !matches!(written, Ok(Ok(()))) {
                    notify(tr(keys::SETTINGS_DATA_ERROR_READ));
                }
                return;
            }
            Ok(None) => return,
            Err(e) => log::info!("export: no save dialog ({e})"),
        }
    }
    let Some(dir) = files::app_data_dir().map(|d| d.join("exports")) else {
        return;
    };
    let path = dir.join(&name);
    let shown = path.display().to_string();
    let target = path.clone();
    let written = blocking(move || {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&target, &bytes)
    })
    .await;
    if !matches!(written, Ok(Ok(()))) {
        notify(tr(keys::SETTINGS_DATA_ERROR_READ));
        return;
    }
    if share_available().await {
        match share(ShareRequest::new().title(name).file(path)).await {
            Ok(_) => return,
            Err(e) => log::info!("export: share failed ({})", e.message),
        }
    }
    notify(shown);
}

/// `downloadExportFile`.
pub(crate) fn export() {
    let doc = document();
    let notes = notes_store().export();
    sync_store().run(
        move |s| build_export(s, &doc, &notes, &AppCatalog),
        |result| {
            let value = match result {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("export: {e}");
                    return;
                }
            };
            let day: String = iso_from_ms(now_ms()).chars().take(10).collect();
            let name = format!("easyquran-export-{day}.json");
            let bytes = serde_json::to_vec(&value).unwrap_or_default();
            spawn(save(bytes, name));
        },
    );
}

fn failure_key(reason: ParseFailure) -> i18n::Key {
    match reason {
        ParseFailure::FutureVersion => keys::SETTINGS_DATA_ERROR_FUTURE,
        ParseFailure::NotExport => keys::SETTINGS_DATA_ERROR_NOT_EXPORT,
        ParseFailure::Malformed => keys::SETTINGS_DATA_ERROR_MALFORMED,
    }
}

/// The import of a picked file (`onPick`).
fn import() {
    spawn(async move {
        let picked = pick_files(FilePickRequest::new(false, 1).filter(json_filter())).await;
        let Some(file) = picked.into_iter().next() else {
            return;
        };
        let Ok(raw) = serde_json::from_slice::<serde_json::Value>(file.data.bytes()) else {
            notify(tr(keys::SETTINGS_DATA_ERROR_MALFORMED));
            return;
        };
        let parsed = match parse_export(&raw, &AppCatalog, now_ms()) {
            Ok(p) => p,
            Err(reason) => {
                notify(tr(failure_key(reason)));
                return;
            }
        };
        apply_document(&parsed.settings);
        let existing: HashSet<(i64, i64)> = notes_store()
            .entries()
            .iter()
            .map(|e| (e.surah, e.ayah))
            .collect();
        let sync = sync_store();
        sync.write(
            Area::Bookmarks,
            move |s| {
                let mut port = CollectNotes {
                    existing,
                    added: Vec::new(),
                };
                let summary = apply_import(s, &parsed, &mut port, &AppCatalog)?;
                Ok((summary, port.added))
            },
            move |result| {
                let (summary, notes) = match result {
                    Ok(out) => out,
                    Err(e) => {
                        log::warn!("import: {e}");
                        notify(tr(keys::SETTINGS_DATA_ERROR_READ));
                        return;
                    }
                };
                for n in notes {
                    notes_store().set_note(n.surah, n.ayah, &n.text, n.source_id);
                }
                sync.kick();
                let mut message = i18n::tr_args(
                    locale(),
                    keys::SETTINGS_DATA_IMPORT_DONE,
                    &[
                        ("added", summary.added.into()),
                        ("skipped", summary.skipped.into()),
                    ],
                );
                if summary.invalid > 0 {
                    message.push(' ');
                    message.push_str(&i18n::tr_args(
                        locale(),
                        keys::SETTINGS_DATA_IMPORT_INVALID,
                        &[("count", summary.invalid.into())],
                    ));
                }
                notify(message);
            },
        );
    });
}

fn action_row(label: i18n::Key, note: i18n::Key, on_press: impl Fn() + 'static) -> AnyElement {
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_0p5()
                .child(row_label(t(label)))
                .child(caption(t(note))),
        )
        .child(button_with(
            t(label),
            ButtonVariant::Outline,
            ButtonSize::Medium,
            false,
            None,
            on_press,
        ))
        .into_element()
}

pub fn section() -> AnyElement {
    section_frame(
        SettingsSection::Data,
        keys::SETTINGS_DATA_INTRO,
        list_card(vec![
            action_row(
                keys::SETTINGS_DATA_EXPORT,
                keys::SETTINGS_DATA_EXPORT_NOTE,
                export,
            ),
            action_row(
                keys::SETTINGS_DATA_IMPORT,
                keys::SETTINGS_DATA_IMPORT_NOTE,
                import,
            ),
        ]),
    )
}
