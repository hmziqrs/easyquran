//! The file-dialogs card: the system's open and save dialogs on the
//! desktops. iOS, Android and the web have none; the request answers with an
//! error there, which the card shows rather than hiding.

use nsh_core::{
    file_dialog::{OpenDialog, SaveDialog},
    prelude::*,
};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::{
    locale,
    widgets::{hint, localized, panel, quiet_button},
};

/// Shows one dialog outcome: the path, cancelled, or the failure.
fn set_answer(answer: Signal<String>, outcome: Result<Option<String>, String>) {
    answer.set(match outcome {
        Ok(Some(shown)) => shown,
        Ok(None) => t!("platform.files.cancelled").to_string(),
        Err(error) => error,
    });
}

/// The desktop card: open file, open folder and save, with the last answer.
#[component]
fn FilesCard() -> impl IntoElement {
    // What the last dialog answered, as the card shows it.
    let answer = signal(String::new());
    let owner = Owner::current();
    panel(
        "platform.files.title",
        [
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(quiet_button(
                    "platform-files-open-file",
                    localized("platform.files.open_file"),
                    {
                        move || {
                            spawn(async move {
                                let outcome = pick_paths(OpenDialog::files().title("nsh-starter"))
                                    .await
                                    .map(|picked| {
                                        picked.map(|paths| {
                                            paths
                                                .iter()
                                                .map(|path| path.display().to_string())
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        })
                                    });
                                if owner.is_live() {
                                    set_answer(answer, outcome);
                                }
                            });
                        }
                    },
                ))
                .child(quiet_button(
                    "platform-files-open-folder",
                    localized("platform.files.open_folder"),
                    {
                        move || {
                            spawn(async move {
                                let outcome =
                                    pick_paths(OpenDialog::folders().title("nsh-starter"))
                                        .await
                                        .map(|picked| {
                                            picked.map(|paths| {
                                                paths
                                                    .iter()
                                                    .map(|path| path.display().to_string())
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            })
                                        });
                                if owner.is_live() {
                                    set_answer(answer, outcome);
                                }
                            });
                        }
                    },
                ))
                .child(quiet_button(
                    "platform-files-save",
                    localized("platform.files.save"),
                    {
                        move || {
                            spawn(async move {
                                let outcome = pick_save_path(
                                    SaveDialog::new()
                                        .file_name("notes.txt")
                                        .title("nsh-starter"),
                                )
                                .await
                                .map(|path| path.map(|path| path.display().to_string()));
                                if owner.is_live() {
                                    set_answer(answer, outcome);
                                }
                            });
                        }
                    },
                ))
                .into_element(),
            text(move || answer.get())
                .text_sm()
                .tabular_nums()
                .test_id("platform-files-answer")
                .into_element(),
            hint("platform.files.hint"),
        ],
    )
}

/// What the platform pages show for files: the dialogs on a desktop, the
/// boundary note elsewhere (decided once; a platform never changes mid-run).
pub(crate) fn card() -> AnyElement {
    when(is_desktop(), || view().child(files_card()).into_element())
        .otherwise(|| {
            view()
                .child(
                    panel(
                        "platform.files.title",
                        [text(move || {
                            locale::app_locale().get();
                            t!("platform.files.unavailable").to_string()
                        })
                        .text_sm()
                        .into_element()],
                    )
                    .into_element(),
                )
                .into_element()
        })
        .into_element()
}
