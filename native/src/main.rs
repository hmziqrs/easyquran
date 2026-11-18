//! Runs the Rust application directly on the machine's native backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    env_logger::init();
    // The desktop's deep-link seam: `nsh-starter nsh-starter://data/item/3`
    // opens that route once the shell is up.
    for arg in std::env::args().skip(1) {
        if arg.starts_with(nsh_starter::shell::deep_link::SCHEME) {
            nsh_starter::shell::deep_link::from_launch(arg);
        }
    }
    // The identity comes from nsh.json (compile time), so scripts/rename.sh
    // renames this binary by rewriting the manifest alone.
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "windows")),
        allow(unused_variables)
    )]
    let (name, id, _) = nsh_starter::foundation::manifest_info();
    platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<nsh_starter::App>()),
        platform::AppOptions {
            title: name.to_string(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: id.to_string(),
            // Wide enough for the desktop shell (the sidebar), per the
            // shell's 700-point threshold on the desktops.
            size: (1280, 800),
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; use the TSX entry with nsh run for iOS and Android."
);
