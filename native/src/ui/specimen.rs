//! The Arabic specimen (a developer surface on the Diagnostics page): Al-
//! Fatiha and Ayat al-Kursi in each of the five scripts through
//! [`crate::ui::arabic`], a tajweed-colored rendering, and the palette and
//! mode switches, to check shaping, marks, ornaments and the theme by eye
//! on each platform. The strings below are the databases' own, pasted
//! once; the reader reads them from the databases.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::stores::appearance::{AppearanceMode, AppearanceStore};
use crate::ui::arabic::{
    ArabicFont, ArabicSegment, ArabicStyle, QuranScript, ayah_text, plain_ayah,
};
use crate::ui::button::choice_button;
use crate::ui::tokens::{Palette, TajweedClass};

/// Which card a developer asked for (`EQ_SPECIMEN=1..6`, debug builds):
/// the specimen shows only that one, so a window-sized capture holds it.
fn only_card() -> Option<usize> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var("EQ_SPECIMEN").ok()?.parse().ok()
}

/// Debug builds: `EQ_PALETTE` / `EQ_MODE` set the appearance as the
/// specimen opens, and `EQ_SHOT_DIR` starts [`capture`].
fn dev_setup(store: AppearanceStore) {
    if !cfg!(debug_assertions) {
        return;
    }
    if let Some(palette) = std::env::var("EQ_PALETTE")
        .ok()
        .and_then(|id| Palette::from_id(&id))
    {
        store.set_palette(palette);
    }
    if let Some(mode) = std::env::var("EQ_MODE")
        .ok()
        .and_then(|id| AppearanceMode::from_id(&id))
    {
        store.set_mode(mode);
    }
    #[cfg(all(target_os = "macos", debug_assertions))]
    if let Ok(dir) = std::env::var("EQ_SHOT_DIR") {
        capture::start(dir.into());
    }
}

/// The specimen section.
pub fn arabic_specimen() -> impl IntoElement {
    let store = AppearanceStore::get();
    dev_setup(store);
    let only = only_card();
    let shown = move |at: usize| only.is_none_or(|n| n == at);
    let mut column = view()
        .w_full()
        .flex_col()
        .gap_6()
        .test_id("arabic-specimen")
        .child(crate::ui::text::section_title("Arabic specimen"))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(Palette::ALL.into_iter().map(move |palette| {
                    choice_button(
                        palette.id(),
                        palette.id(),
                        move || store.palette() == palette,
                        false,
                        move || store.set_palette(palette),
                    )
                }))
                .children(AppearanceMode::ALL.into_iter().map(move |mode| {
                    choice_button(
                        mode.id(),
                        mode.id(),
                        move || store.mode() == mode,
                        false,
                        move || store.set_mode(mode),
                    )
                })),
        );
    let sets: [(
        &'static str,
        QuranScript,
        ArabicFont,
        &[(u32, u32, &'static str); 8],
    ); 5] = [
        (
            "Uthmani · KFGQPC Hafs",
            QuranScript::Uthmani,
            ArabicFont::KfgqpcHafs,
            &UTHMANI,
        ),
        (
            "Annotated Uthmani · KFGQPC Hafs",
            QuranScript::UthmaniAnnotated,
            ArabicFont::KfgqpcHafs,
            &UTHMANI_ANNOTATED,
        ),
        (
            "Simple clean · Uthman Taha Naskh",
            QuranScript::SimpleClean,
            ArabicFont::KfgqpcTahaV1,
            &SIMPLE_CLEAN,
        ),
        (
            "IndoPak · IndoPak Reader Compat",
            QuranScript::IndoPak,
            ArabicFont::KfgqpcHafs,
            &INDOPAK,
        ),
        (
            "Uthmani · Uthman Taha Naskh Bold",
            QuranScript::Uthmani,
            ArabicFont::KfgqpcTahaV1Bold,
            &UTHMANI,
        ),
    ];
    for (at, (title, script, font, rows)) in sets.into_iter().enumerate() {
        if !shown(at + 1) {
            continue;
        }
        let style = move || ArabicStyle {
            script,
            font,
            ..ArabicStyle::default()
        };
        // Ayat al-Kursi first: the densest marks, inside the first screen;
        // the annotated card adds 2:257's silent-letter marks (U+06DF).
        let extra = (script == QuranScript::UthmaniAnnotated).then_some(&SILENT_LETTER_MARKS);
        column = column.child(specimen_card(
            title,
            rows[7..]
                .iter()
                .chain(extra)
                .chain(&rows[..7])
                .map(|(_, ayah, verse)| plain_ayah(*verse, Some(*ayah), style).into_element())
                .collect(),
        ));
    }
    if shown(7) {
        column = column.child(ornament_forms());
    }
    if !shown(6) {
        return column;
    }
    let tajweed_style = || ArabicStyle::of(QuranScript::Tajweed);
    column.child(specimen_card(
        "Tajweed · KFGQPC Hafs, colored runs",
        {
            let mut rows = tajweed_rows();
            rows.rotate_right(1);
            rows
        }
        .into_iter()
        .map(|(_, ayah, runs)| ayah_text(runs, Some(ayah), tajweed_style).into_element())
        .collect(),
    ))
}

/// The ayah-end mark in each face and isolation the platform could
/// compose it with: what the ornament span uses is decided from this.
fn ornament_forms() -> impl IntoElement {
    use crate::ui::arabic::{HAFS_FAMILY, INDOPAK_FAMILY, ORNAMENT_FAMILY};
    let row = |label: &'static str, family: &'static str, mark: &'static str| {
        view()
            .flex_row()
            .items_center()
            .gap_4()
            .child(
                text(label)
                    .w(px(220.0))
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
            )
            .child(
                text(mark)
                    .font_family(family)
                    .text_size(px(36.0))
                    .text_color(move || theme().primary),
            )
            .into_element()
    };
    specimen_card(
        "Ornament forms (U+06DD + 255)",
        vec![
            row(
                "Ayah Ornament, bare",
                ORNAMENT_FAMILY,
                "\u{06DD}\u{0662}\u{0665}\u{0665}",
            ),
            row(
                "Ayah Ornament, LRI…PDI",
                ORNAMENT_FAMILY,
                "\u{2066}\u{06DD}\u{0662}\u{0665}\u{0665}\u{2069}",
            ),
            row(
                "KFGQPC Hafs, bare",
                HAFS_FAMILY,
                "\u{06DD}\u{0662}\u{0665}\u{0665}",
            ),
            row(
                "KFGQPC Hafs, LRI…PDI",
                HAFS_FAMILY,
                "\u{2066}\u{06DD}\u{0662}\u{0665}\u{0665}\u{2069}",
            ),
            row(
                "IndoPak, Urdu digits",
                INDOPAK_FAMILY,
                "\u{06DD}\u{06F2}\u{06F5}\u{06F5}",
            ),
        ],
    )
}

fn specimen_card(title: &'static str, ayahs: Vec<AnyElement>) -> impl IntoElement {
    crate::ui::layout::card()
        .bg(crate::ui::tokens::reader_background)
        .child(
            text(title)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().muted_foreground),
        )
        .children(ayahs)
}

/// Debug builds on macOS: pictures of the app's own window for visual
/// review, which need no Screen Recording permission (the window server
/// lets an app picture its own windows; a separate `screencapture` is
/// refused). A thread polls `<dir>/shot.req` holding `<window id> <name>`
/// and writes `<dir>/<name>.png`, then removes the request.
#[cfg(all(target_os = "macos", debug_assertions))]
pub mod capture {
    use std::ffi::{CString, c_char, c_void};
    use std::path::PathBuf;
    use std::sync::Once;
    use std::time::Duration;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CgPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CgSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CgRect {
        origin: CgPoint,
        size: CgSize,
    }

    type CreateImage = unsafe extern "C" fn(CgRect, u32, u32, u32) -> *mut c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: *const c_void,
            buffer: *const u8,
            length: isize,
            is_directory: bool,
        ) -> *const c_void;
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            text: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFRelease(object: *const c_void);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGImageRelease(image: *mut c_void);
    }

    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        fn CGImageDestinationCreateWithURL(
            url: *const c_void,
            kind: *const c_void,
            count: usize,
            options: *const c_void,
        ) -> *mut c_void;
        fn CGImageDestinationAddImage(
            destination: *mut c_void,
            image: *mut c_void,
            properties: *const c_void,
        );
        fn CGImageDestinationFinalize(destination: *mut c_void) -> bool;
    }

    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    /// `RTLD_DEFAULT` on Apple platforms.
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
    /// `kCGWindowListOptionIncludingWindow`.
    const INCLUDING_WINDOW: u32 = 1 << 3;
    /// `kCGWindowImageBoundsIgnoreFraming | kCGWindowImageBestResolution`.
    const IMAGE_OPTIONS: u32 = 1 | (1 << 3);
    /// `kCFStringEncodingUTF8`.
    const UTF8: u32 = 0x0800_0100;

    static STARTED: Once = Once::new();

    /// Starts the polling thread once.
    pub fn start(dir: PathBuf) {
        STARTED.call_once(move || {
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(300));
                    let request = dir.join("shot.req");
                    let Ok(line) = std::fs::read_to_string(&request) else {
                        continue;
                    };
                    let _ = std::fs::remove_file(&request);
                    let mut parts = line.split_whitespace();
                    let (Some(id), Some(name)) = (parts.next(), parts.next()) else {
                        continue;
                    };
                    let Ok(id) = id.parse::<u32>() else {
                        continue;
                    };
                    let out = dir.join(format!("{name}.png"));
                    if let Err(error) = shot(id, &out) {
                        log::warn!("capture {name}: {error}");
                    }
                }
            });
        });
    }

    /// Pictures window `id` into the PNG file `out`.
    fn shot(id: u32, out: &std::path::Path) -> Result<(), String> {
        let name = CString::new("CGWindowListCreateImage").map_err(|e| e.to_string())?;
        // SAFETY: dlsym only looks the exported symbol up.
        let symbol = unsafe { dlsym(RTLD_DEFAULT, name.as_ptr()) };
        if symbol.is_null() {
            return Err("CGWindowListCreateImage is missing".into());
        }
        // SAFETY: the symbol is CoreGraphics' function of this signature.
        let create: CreateImage = unsafe { std::mem::transmute(symbol) };
        let null_rect = CgRect {
            origin: CgPoint {
                x: f64::INFINITY,
                y: f64::INFINITY,
            },
            size: CgSize {
                width: 0.0,
                height: 0.0,
            },
        };
        // SAFETY: CGRectNull, an option set and a window number; the
        // result is a +1 image or null.
        let image = unsafe { create(null_rect, INCLUDING_WINDOW, id, IMAGE_OPTIONS) };
        if image.is_null() {
            return Err("no picture of the window".into());
        }
        let path = out.to_string_lossy().into_owned();
        let kind = CString::new("public.png").map_err(|e| e.to_string())?;
        // SAFETY: CoreFoundation and ImageIO calls with live arguments;
        // every +1 object is released once.
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                path.as_ptr(),
                path.len() as isize,
                false,
            );
            let uti = CFStringCreateWithCString(std::ptr::null(), kind.as_ptr(), UTF8);
            let destination = CGImageDestinationCreateWithURL(url, uti, 1, std::ptr::null());
            let written = !destination.is_null() && {
                CGImageDestinationAddImage(destination, image, std::ptr::null());
                CGImageDestinationFinalize(destination)
            };
            if !destination.is_null() {
                CFRelease(destination);
            }
            CFRelease(uti);
            CFRelease(url);
            CGImageRelease(image);
            if written {
                Ok(())
            } else {
                Err("the PNG was not written".into())
            }
        }
    }
}

/// `quran-uthmani.sqlite`, verbatim.
pub const UTHMANI: [(u32, u32, &str); 8] = [
    (1, 1, "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ"),
    (1, 2, "ٱلْحَمْدُ لِلَّهِ رَبِّ ٱلْعَـٰلَمِينَ"),
    (1, 3, "ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ"),
    (1, 4, "مَـٰلِكِ يَوْمِ ٱلدِّينِ"),
    (1, 5, "إِيَّاكَ نَعْبُدُ وَإِيَّاكَ نَسْتَعِينُ"),
    (1, 6, "ٱهْدِنَا ٱلصِّرَٰطَ ٱلْمُسْتَقِيمَ"),
    (1, 7, "صِرَٰطَ ٱلَّذِينَ أَنْعَمْتَ عَلَيْهِمْ غَيْرِ ٱلْمَغْضُوبِ عَلَيْهِمْ وَلَا ٱلضَّآلِّينَ"),
    (
        2,
        255,
        "ٱللَّهُ لَآ إِلَـٰهَ إِلَّا هُوَ ٱلْحَىُّ ٱلْقَيُّومُ ۚ لَا تَأْخُذُهُۥ سِنَةٌ وَلَا نَوْمٌ ۚ لَّهُۥ مَا فِى ٱلسَّمَـٰوَٰتِ وَمَا فِى ٱلْأَرْضِ ۗ مَن ذَا ٱلَّذِى يَشْفَعُ عِندَهُۥٓ إِلَّا بِإِذْنِهِۦ ۚ يَعْلَمُ مَا بَيْنَ أَيْدِيهِمْ وَمَا خَلْفَهُمْ ۖ وَلَا يُحِيطُونَ بِشَىْءٍ مِّنْ عِلْمِهِۦٓ إِلَّا بِمَا شَآءَ ۚ وَسِعَ كُرْسِيُّهُ ٱلسَّمَـٰوَٰتِ وَٱلْأَرْضَ ۖ وَلَا يَـُٔودُهُۥ حِفْظُهُمَا ۚ وَهُوَ ٱلْعَلِىُّ ٱلْعَظِيمُ",
    ),
];

/// `quran-uthmani-annotated.sqlite`, verbatim.
pub const UTHMANI_ANNOTATED: [(u32, u32, &str); 8] = [
    (1, 1, "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ"),
    (1, 2, "ٱلْحَمْدُ لِلَّهِ رَبِّ ٱلْعَـٰلَمِينَ"),
    (1, 3, "ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ"),
    (1, 4, "مَـٰلِكِ يَوْمِ ٱلدِّينِ"),
    (1, 5, "إِيَّاكَ نَعْبُدُ وَإِيَّاكَ نَسْتَعِينُ"),
    (1, 6, "ٱهْدِنَا ٱلصِّرَٰطَ ٱلْمُسْتَقِيمَ"),
    (1, 7, "صِرَٰطَ ٱلَّذِينَ أَنْعَمْتَ عَلَيْهِمْ غَيْرِ ٱلْمَغْضُوبِ عَلَيْهِمْ وَلَا ٱلضَّآلِّينَ"),
    (
        2,
        255,
        "ٱللَّهُ لَآ إِلَـٰهَ إِلَّا هُوَ ٱلْحَىُّ ٱلْقَيُّومُ ۚ لَا تَأْخُذُهُۥ سِنَةٌۭ وَلَا نَوْمٌۭ ۚ لَّهُۥ مَا فِى ٱلسَّمَـٰوَٰتِ وَمَا فِى ٱلْأَرْضِ ۗ مَن ذَا ٱلَّذِى يَشْفَعُ عِندَهُۥٓ إِلَّا بِإِذْنِهِۦ ۚ يَعْلَمُ مَا بَيْنَ أَيْدِيهِمْ وَمَا خَلْفَهُمْ ۖ وَلَا يُحِيطُونَ بِشَىْءٍۢ مِّنْ عِلْمِهِۦٓ إِلَّا بِمَا شَآءَ ۚ وَسِعَ كُرْسِيُّهُ ٱلسَّمَـٰوَٰتِ وَٱلْأَرْضَ ۖ وَلَا يَـُٔودُهُۥ حِفْظُهُمَا ۚ وَهُوَ ٱلْعَلِىُّ ٱلْعَظِيمُ",
    ),
];

/// 2:257 of `quran-uthmani-annotated.sqlite`, verbatim: U+06DF after the
/// alef of `ءَامَنُوا۟` and `كَفَرُوٓا۟`.
pub const SILENT_LETTER_MARKS: (u32, u32, &str) = (
    2,
    257,
    "\u{0671}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{064F} \u{0648}\u{064E}\u{0644}\u{0650}\u{0649}\u{0651}\u{064F} \u{0671}\u{0644}\u{0651}\u{064E}\u{0630}\u{0650}\u{064A}\u{0646}\u{064E} \u{0621}\u{064E}\u{0627}\u{0645}\u{064E}\u{0646}\u{064F}\u{0648}\u{0627}\u{06DF} \u{064A}\u{064F}\u{062E}\u{0652}\u{0631}\u{0650}\u{062C}\u{064F}\u{0647}\u{064F}\u{0645} \u{0645}\u{0651}\u{0650}\u{0646}\u{064E} \u{0671}\u{0644}\u{0638}\u{0651}\u{064F}\u{0644}\u{064F}\u{0645}\u{064E}\u{0640}\u{0670}\u{062A}\u{0650} \u{0625}\u{0650}\u{0644}\u{064E}\u{0649} \u{0671}\u{0644}\u{0646}\u{0651}\u{064F}\u{0648}\u{0631}\u{0650} \u{06D6} \u{0648}\u{064E}\u{0671}\u{0644}\u{0651}\u{064E}\u{0630}\u{0650}\u{064A}\u{0646}\u{064E} \u{0643}\u{064E}\u{0641}\u{064E}\u{0631}\u{064F}\u{0648}\u{0653}\u{0627}\u{06DF} \u{0623}\u{064E}\u{0648}\u{0652}\u{0644}\u{0650}\u{064A}\u{064E}\u{0627}\u{0653}\u{0624}\u{064F}\u{0647}\u{064F}\u{0645}\u{064F} \u{0671}\u{0644}\u{0637}\u{0651}\u{064E}\u{0640}\u{0670}\u{063A}\u{064F}\u{0648}\u{062A}\u{064F} \u{064A}\u{064F}\u{062E}\u{0652}\u{0631}\u{0650}\u{062C}\u{064F}\u{0648}\u{0646}\u{064E}\u{0647}\u{064F}\u{0645} \u{0645}\u{0651}\u{0650}\u{0646}\u{064E} \u{0671}\u{0644}\u{0646}\u{0651}\u{064F}\u{0648}\u{0631}\u{0650} \u{0625}\u{0650}\u{0644}\u{064E}\u{0649} \u{0671}\u{0644}\u{0638}\u{0651}\u{064F}\u{0644}\u{064F}\u{0645}\u{064E}\u{0640}\u{0670}\u{062A}\u{0650} \u{06D7} \u{0623}\u{064F}\u{0648}\u{06DF}\u{0644}\u{064E}\u{0640}\u{0670}\u{0653}\u{0626}\u{0650}\u{0643}\u{064E} \u{0623}\u{064E}\u{0635}\u{0652}\u{062D}\u{064E}\u{0640}\u{0670}\u{0628}\u{064F} \u{0671}\u{0644}\u{0646}\u{0651}\u{064E}\u{0627}\u{0631}\u{0650} \u{06D6} \u{0647}\u{064F}\u{0645}\u{0652} \u{0641}\u{0650}\u{064A}\u{0647}\u{064E}\u{0627} \u{062E}\u{064E}\u{0640}\u{0670}\u{0644}\u{0650}\u{062F}\u{064F}\u{0648}\u{0646}\u{064E}",
);

/// `quran-simple-clean.sqlite`, verbatim.
pub const SIMPLE_CLEAN: [(u32, u32, &str); 8] = [
    (1, 1, "بسم الله الرحمن الرحيم"),
    (1, 2, "الحمد لله رب العالمين"),
    (1, 3, "الرحمن الرحيم"),
    (1, 4, "مالك يوم الدين"),
    (1, 5, "إياك نعبد وإياك نستعين"),
    (1, 6, "اهدنا الصراط المستقيم"),
    (1, 7, "صراط الذين أنعمت عليهم غير المغضوب عليهم ولا الضالين"),
    (
        2,
        255,
        "الله لا إله إلا هو الحي القيوم لا تأخذه سنة ولا نوم له ما في السماوات وما في الأرض من ذا الذي يشفع عنده إلا بإذنه يعلم ما بين أيديهم وما خلفهم ولا يحيطون بشيء من علمه إلا بما شاء وسع كرسيه السماوات والأرض ولا يئوده حفظهما وهو العلي العظيم",
    ),
];

/// `quran-indopak.sqlite`, verbatim.
pub const INDOPAK: [(u32, u32, &str); 8] = [
    (1, 1, "بِسۡمِ اللهِ الرَّحۡمٰنِ الرَّحِيۡمِ"),
    (1, 2, "اَلۡحَمۡدُ لِلّٰهِ رَبِّ الۡعٰلَمِيۡنَۙ"),
    (1, 3, "الرَّحۡمٰنِ الرَّحِيۡمِۙ"),
    (1, 4, "مٰلِكِ يَوۡمِ الدِّيۡنِؕ"),
    (1, 5, "اِيَّاكَ نَعۡبُدُ وَاِيَّاكَ نَسۡتَعِيۡنُؕ"),
    (1, 6, "اِهۡدِنَا الصِّرَاطَ الۡمُسۡتَقِيۡمَۙ"),
    (
        1,
        7,
        "صِرَاطَ الَّذِيۡنَ اَنۡعَمۡتَ عَلَيۡهِمۡ ۙ غَيۡرِ الۡمَغۡضُوۡبِ عَلَيۡهِمۡ وَلَا الضَّآلِّيۡنَ",
    ),
    (
        2,
        255,
        "اللّٰهُ لَاۤ اِلٰهَ اِلَّا هُوَ الۡحَـىُّ الۡقَيُّوۡمُۚ لَا تَاۡخُذُهٗ سِنَةٌ وَّلَا نَوۡمٌ\u{200b}ؕ لَهٗ مَا فِى السَّمٰوٰتِ وَمَا فِى الۡاَرۡضِ\u{200b}ؕ مَنۡ ذَا الَّذِىۡ يَشۡفَعُ عِنۡدَهٗۤ اِلَّا بِاِذۡنِهٖ\u{200b}ؕ يَعۡلَمُ مَا بَيۡنَ اَيۡدِيۡهِمۡ وَمَا خَلۡفَهُمۡ\u{200b}ۚ وَلَا يُحِيۡطُوۡنَ بِشَىۡءٍ مِّنۡ عِلۡمِهٖۤ اِلَّا بِمَا شَآءَ ۚ وَسِعَ كُرۡسِيُّهُ السَّمٰوٰتِ وَالۡاَرۡضَ\u{200b}\u{200b}ۚ وَلَا يَـــُٔوۡدُهٗ حِفۡظُهُمَا \u{200b}ۚ وَ هُوَ الۡعَلِىُّ الۡعَظِيۡمُ",
    ),
];

/// `quran-tajweed.sqlite`, verbatim.
pub const TAJWEED_RAW: [(u32, u32, &str); 8] = [
    (
        1,
        1,
        "بِسْمِ [h:1[ٱ]للَّهِ [h:2[ٱ][l[ل]رَّحْمَ[n[ـٰ]نِ [h:3[ٱ][l[ل]رَّح[p[ِي]مِ",
    ),
    (1, 2, "ٱلْحَمْدُ لِلَّهِ رَبِّ [h:4[ٱ]لْعَ[n[ـٰ]لَم[p[ِي]نَ"),
    (1, 3, "ٱ[l[ل]رَّحْمَ[n[ـٰ]نِ [h:3[ٱ][l[ل]رَّح[p[ِي]مِ"),
    (1, 4, "مَ[n[ـٰ]لِكِ يَوْمِ [h:5[ٱ][l[ل]دّ[p[ِي]نِ"),
    (1, 5, "إِيَّاكَ نَعْبُدُ وَإِيَّاكَ نَسْتَع[p[ِي]نُ"),
    (1, 6, "ٱهْدِنَا [h:6[ٱ][l[ل]صِّر[n[َٲ]طَ [h:7[ٱ]لْمُسْتَق[p[ِي]مَ"),
    (
        1,
        7,
        "صِر[n[َٲ]طَ [h:8[ٱ]لَّذِينَ أَنْعَمْتَ عَلَيْهِمْ غَيْرِ [h:9[ٱ]لْمَغْضُوبِ عَلَيْهِمْ وَلَا [h:10[ٱ][l[ل]ضّ[m[َا]ٓلّ[p[ِي]نَ",
    ),
    (
        2,
        255,
        "ٱللَّهُ ل[o[َآ] إِلَ[n[ـٰ]هَ إِلَّا هُوَ [h:1468[ٱ]لْحَىُّ [h:1469[ٱ]لْقَيُّومُ\u{200c}ۚ لَا تَأْخُذُهُ[n[ۥ] سِنَ[a:1470[ةٌ و]َلَا نَوْ[u:1471[مٌ\u{200c}ۚ ل]َّهُ[n[ۥ] مَا فِى [h:609[ٱ][l[ل]سَّمَ[n[ـٰ]و[n[َٲ]تِ وَمَا فِى [h:44[ٱ]لْأَرْضِ\u{200c}ۗ مَ[f:1375[ن ذ]َا [h:1376[ٱ]لَّذِى يَشْفَعُ عِ[f:719[ند]َهُ[o[ۥٓ] إِلَّا بِإِذْنِه[n[ِۦ]\u{200c}ۚ يَعْلَمُ مَا بَيْنَ أَيْدِيهِمْ وَمَا خَلْفَهُمْ\u{200c}ۖ وَلَا يُحِيطُونَ بِش[a:1472[َىْءٍ م]ِّنْ عِلْمِه[o[ِۦٓ] إِلَّا بِمَا ش[o[َا]ٓءَ\u{200c}ۚ وَسِعَ كُرْسِيُّهُ [h:1473[ٱ][l[ل]سَّمَ[n[ـٰ]و[n[َٲ]تِ وَ[h:1474[ٱ]لْأَرْضَ\u{200c}ۖ وَلَا يَــُٔودُهُ[n[ۥ] حِفْظُهُمَا\u{200c}ۚ وَهُوَ [h:1475[ٱ]لْعَلِىُّ [h:1476[ٱ]لْعَظ[p[ِي]مُ",
    ),
];

/// The tajweed rows as runs, split by the web's `parseTajweedSegments`
/// rules: markup dropped, text verbatim.
pub fn tajweed_rows() -> Vec<(u32, u32, Vec<ArabicSegment>)> {
    use TajweedClass as C;
    vec![
        (
            1,
            1,
            vec![
                ArabicSegment::plain("بِسْمِ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("للَّهِ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("رَّحْمَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("نِ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("رَّح"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("مِ"),
            ],
        ),
        (
            1,
            2,
            vec![
                ArabicSegment::plain("ٱلْحَمْدُ لِلَّهِ رَبِّ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْعَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("لَم"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("نَ"),
            ],
        ),
        (
            1,
            3,
            vec![
                ArabicSegment::plain("ٱ"),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("رَّحْمَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("نِ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("رَّح"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("مِ"),
            ],
        ),
        (
            1,
            4,
            vec![
                ArabicSegment::plain("مَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("لِكِ يَوْمِ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("دّ"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("نِ"),
            ],
        ),
        (
            1,
            5,
            vec![
                ArabicSegment::plain("إِيَّاكَ نَعْبُدُ وَإِيَّاكَ نَسْتَع"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("نُ"),
            ],
        ),
        (
            1,
            6,
            vec![
                ArabicSegment::plain("ٱهْدِنَا "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("صِّر"),
                ArabicSegment::rule("َٲ", C::MaddaNormal),
                ArabicSegment::plain("طَ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْمُسْتَق"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("مَ"),
            ],
        ),
        (
            1,
            7,
            vec![
                ArabicSegment::plain("صِر"),
                ArabicSegment::rule("َٲ", C::MaddaNormal),
                ArabicSegment::plain("طَ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لَّذِينَ أَنْعَمْتَ عَلَيْهِمْ غَيْرِ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْمَغْضُوبِ عَلَيْهِمْ وَلَا "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("ضّ"),
                ArabicSegment::rule("َا", C::MaddaNecessary),
                ArabicSegment::plain("ٓلّ"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("نَ"),
            ],
        ),
        (
            2,
            255,
            vec![
                ArabicSegment::plain("ٱللَّهُ ل"),
                ArabicSegment::rule("َآ", C::MaddaObligatory),
                ArabicSegment::plain(" إِلَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("هَ إِلَّا هُوَ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْحَىُّ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْقَيُّومُ\u{200c}ۚ لَا تَأْخُذُهُ"),
                ArabicSegment::rule("ۥ", C::MaddaNormal),
                ArabicSegment::plain(" سِنَ"),
                ArabicSegment::rule("ةٌ و", C::IdghamGhunnah),
                ArabicSegment::plain("َلَا نَوْ"),
                ArabicSegment::rule("مٌ\u{200c}ۚ ل", C::IdghamNoGhunnah),
                ArabicSegment::plain("َّهُ"),
                ArabicSegment::rule("ۥ", C::MaddaNormal),
                ArabicSegment::plain(" مَا فِى "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("سَّمَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("و"),
                ArabicSegment::rule("َٲ", C::MaddaNormal),
                ArabicSegment::plain("تِ وَمَا فِى "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْأَرْضِ\u{200c}ۗ مَ"),
                ArabicSegment::rule("ن ذ", C::Ikhfa),
                ArabicSegment::plain("َا "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لَّذِى يَشْفَعُ عِ"),
                ArabicSegment::rule("ند", C::Ikhfa),
                ArabicSegment::plain("َهُ"),
                ArabicSegment::rule("ۥٓ", C::MaddaObligatory),
                ArabicSegment::plain(" إِلَّا بِإِذْنِه"),
                ArabicSegment::rule("ِۦ", C::MaddaNormal),
                ArabicSegment::plain("\u{200c}ۚ يَعْلَمُ مَا بَيْنَ أَيْدِيهِمْ وَمَا خَلْفَهُمْ\u{200c}ۖ وَلَا يُحِيطُونَ بِش"),
                ArabicSegment::rule("َىْءٍ م", C::IdghamGhunnah),
                ArabicSegment::plain("ِّنْ عِلْمِه"),
                ArabicSegment::rule("ِۦٓ", C::MaddaObligatory),
                ArabicSegment::plain(" إِلَّا بِمَا ش"),
                ArabicSegment::rule("َا", C::MaddaObligatory),
                ArabicSegment::plain("ٓءَ\u{200c}ۚ وَسِعَ كُرْسِيُّهُ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::rule("ل", C::LamShamsiyyah),
                ArabicSegment::plain("سَّمَ"),
                ArabicSegment::rule("ـٰ", C::MaddaNormal),
                ArabicSegment::plain("و"),
                ArabicSegment::rule("َٲ", C::MaddaNormal),
                ArabicSegment::plain("تِ وَ"),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْأَرْضَ\u{200c}ۖ وَلَا يَــُٔودُهُ"),
                ArabicSegment::rule("ۥ", C::MaddaNormal),
                ArabicSegment::plain(" حِفْظُهُمَا\u{200c}ۚ وَهُوَ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْعَلِىُّ "),
                ArabicSegment::rule("ٱ", C::HamzatWasl),
                ArabicSegment::plain("لْعَظ"),
                ArabicSegment::rule("ِي", C::MaddaPermissible),
                ArabicSegment::plain("مُ"),
            ],
        ),
    ]
}
