//! The shell's typed routes: one page each, their keys and labels, and the
//! deep-link paths they answer to. Every page is mounted through
//! [`Route::page`], so a new route is one variant plus its arms here.
//!
//! `Route` is not `Copy` (reader targets carry translation names): clone it
//! into closures, compare by reference, iterate the static lists with
//! `.iter().cloned()`.
//!
//! Labels are English literals until the i18n foundation lands; each one is
//! marked `// i18n: F-i18n` where it is written.

use nsh_core::{element::AnyElement, prelude::*};

pub use eq_core::routes::{ReaderScope, ReaderTarget, TranslationRef};

use crate::features;

/// A section of the settings page (PARITY J1-J8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    Storage,
    Appearance,
    Reading,
    App,
    Privacy,
    Data,
    Account,
}

impl SettingsSection {
    /// Every section, in the settings page's order.
    pub const ALL: [SettingsSection; 7] = [
        SettingsSection::Storage,
        SettingsSection::Appearance,
        SettingsSection::Reading,
        SettingsSection::App,
        SettingsSection::Privacy,
        SettingsSection::Data,
        SettingsSection::Account,
    ];

    /// The path segment (`settings/<slug>`).
    pub fn slug(self) -> &'static str {
        match self {
            SettingsSection::Storage => "storage",
            SettingsSection::Appearance => "appearance",
            SettingsSection::Reading => "reading",
            SettingsSection::App => "app",
            SettingsSection::Privacy => "privacy",
            SettingsSection::Data => "data",
            SettingsSection::Account => "account",
        }
    }

    /// The section a slug names.
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }
}

/// The routes the shell answers to, `Home` the stack's root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    SurahIndex,
    JuzIndex,
    PagesIndex,
    Reader(ReaderTarget),
    Search,
    Bookmarks,
    Yours,
    YoursCard(String),
    Azkar,
    Zikr {
        slug: String,
        translation: Option<TranslationRef>,
    },
    DailyAyah,
    Downloads,
    Settings(SettingsSection),
    Account,
    Login,
    Register,
    VerifyEmail,
    ForgotPassword,
    Totp,
    Appeal(Option<String>),
    Report,
    NewsletterConfirm(String),
    About,
    Faq,
    Contact,
    Privacy,
    Terms,
    Diagnostics,
}

impl Route {
    /// The pages the phone's tab bar holds, in its order.
    pub const TABS: [Route; 5] = [
        Route::Home,
        Route::SurahIndex,
        Route::Search,
        Route::Bookmarks,
        Route::Yours,
    ];

    /// The desktop sidebar's rows, in order.
    pub const SIDEBAR: [Route; 10] = [
        Route::Home,
        Route::SurahIndex,
        Route::JuzIndex,
        Route::PagesIndex,
        Route::Search,
        Route::Bookmarks,
        Route::Yours,
        Route::Azkar,
        Route::Downloads,
        Route::Settings(SettingsSection::Storage),
    ];

    /// Every destination the palette and the desktop "Go" menu offer: the
    /// sidebar's, then the pages reached from elsewhere. Routes that need
    /// data (a reader target, a card id, a token) are not destinations.
    pub const DESTINATIONS: [Route; 18] = [
        Route::Home,
        Route::SurahIndex,
        Route::JuzIndex,
        Route::PagesIndex,
        Route::Search,
        Route::Bookmarks,
        Route::Yours,
        Route::Azkar,
        Route::DailyAyah,
        Route::Downloads,
        Route::Settings(SettingsSection::Storage),
        Route::Account,
        Route::About,
        Route::Faq,
        Route::Contact,
        Route::Privacy,
        Route::Terms,
        Route::Diagnostics,
    ];

    /// The stack key: the same route is the same page, so the pages below
    /// keep their state. Unique per route value.
    pub fn key(&self) -> String {
        match self {
            Route::Reader(target) => format!("reader:{target:?}"),
            Route::YoursCard(id) => format!("yours-card:{id}"),
            Route::Zikr { slug, translation } => format!("zikr:{slug}:{translation:?}"),
            Route::Settings(section) => format!("settings:{}", section.slug()),
            Route::Appeal(token) => format!("appeal:{token:?}"),
            Route::NewsletterConfirm(token) => format!("newsletter:{token}"),
            route => route.key_str().into(),
        }
    }

    /// The route's family key, as a literal: palette command ids, test ids.
    /// Routes of one family (every reader target) share it.
    pub fn key_str(&self) -> &'static str {
        match self {
            Route::Home => "home",
            Route::SurahIndex => "surah",
            Route::JuzIndex => "juz",
            Route::PagesIndex => "pages",
            Route::Reader(_) => "reader",
            Route::Search => "search",
            Route::Bookmarks => "bookmarks",
            Route::Yours => "yours",
            Route::YoursCard(_) => "yours-card",
            Route::Azkar => "azkar",
            Route::Zikr { .. } => "zikr",
            Route::DailyAyah => "daily-ayah",
            Route::Downloads => "downloads",
            Route::Settings(_) => "settings",
            Route::Account => "account",
            Route::Login => "login",
            Route::Register => "register",
            Route::VerifyEmail => "verify-email",
            Route::ForgotPassword => "forgot-password",
            Route::Totp => "totp",
            Route::Appeal(_) => "appeal",
            Route::Report => "report",
            Route::NewsletterConfirm(_) => "newsletter-confirm",
            Route::About => "about",
            Route::Faq => "faq",
            Route::Contact => "contact",
            Route::Privacy => "privacy",
            Route::Terms => "terms",
            Route::Diagnostics => "diagnostics",
        }
    }

    /// What the sidebar, the tabs, the palette and the tests call it.
    pub fn label(&self) -> String {
        self.label_str().into()
    }

    /// The label as a literal (static contexts: tabs, palette entries).
    // i18n: F-i18n — every literal below moves to the catalogue.
    pub fn label_str(&self) -> &'static str {
        match self {
            Route::Home => "Home",
            Route::SurahIndex => "Read",
            Route::JuzIndex => "Juz",
            Route::PagesIndex => "Pages",
            Route::Reader(_) => "Reader",
            Route::Search => "Search",
            Route::Bookmarks => "Bookmarks",
            Route::Yours => "Yours",
            Route::YoursCard(_) => "Card",
            Route::Azkar => "Azkar",
            Route::Zikr { .. } => "Zikr",
            Route::DailyAyah => "Daily ayah",
            Route::Downloads => "Downloads",
            Route::Settings(_) => "Settings",
            Route::Account => "Account",
            Route::Login => "Log in",
            Route::Register => "Create account",
            Route::VerifyEmail => "Verify email",
            Route::ForgotPassword => "Forgot password",
            Route::Totp => "Two-factor code",
            Route::Appeal(_) => "Appeal",
            Route::Report => "Report",
            Route::NewsletterConfirm(_) => "Newsletter",
            Route::About => "About",
            Route::Faq => "FAQ",
            Route::Contact => "Contact",
            Route::Privacy => "Privacy",
            Route::Terms => "Terms",
            Route::Diagnostics => "Diagnostics",
        }
    }

    /// The system header's title.
    pub fn title(&self) -> SharedString {
        self.label().into()
    }

    /// The tab bar's icon: an SF Symbol on iOS, a system drawable on
    /// Android (`nsh package` cannot add app drawables); none elsewhere.
    pub fn tab_icon(&self) -> Option<&'static str> {
        use nsh_core::platform::Os;
        let (ios, android) = match self {
            Route::Home => ("house", "android:ic_menu_compass"),
            Route::SurahIndex => ("book", "android:ic_menu_agenda"),
            Route::Search => ("magnifyingglass", "android:ic_menu_search"),
            Route::Bookmarks => ("bookmark", "android:ic_menu_save"),
            Route::Yours => ("person", "android:ic_menu_myplaces"),
            _ => return None,
        };
        match os() {
            Os::Ios => Some(ios),
            Os::Android => Some(android),
            _ => None,
        }
    }

    /// The route's Lucide icon (ISC), drawn in `currentColor`: the desktop
    /// sidebar's rows, and the themed tab bar Android shows.
    pub fn tab_svg(&self) -> &'static str {
        use crate::ui::icons::lucide;
        match self {
            Route::Home => lucide!(
                r#"<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#
            ),
            Route::SurahIndex | Route::Reader(_) => lucide!(
                r#"<path d="M12 7v14"/><path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/>"#
            ),
            Route::JuzIndex => lucide!(
                r#"<path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>"#
            ),
            Route::Search => {
                lucide!(r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>"#)
            }
            Route::Bookmarks => {
                lucide!(r#"<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>"#)
            }
            Route::Yours | Route::YoursCard(_) | Route::Account => lucide!(
                r#"<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>"#
            ),
            Route::Azkar | Route::Zikr { .. } | Route::DailyAyah => lucide!(
                r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>"#
            ),
            Route::Downloads => lucide!(
                r#"<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/>"#
            ),
            Route::Settings(_) => lucide!(
                r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>"#
            ),
            Route::Diagnostics => lucide!(r#"<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>"#),
            _ => lucide!(
                r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M16 13H8M16 17H8M10 9H8"/>"#
            ),
        }
    }

    /// The SF Symbol the macOS command bar shows for the route.
    pub fn symbol(&self) -> &'static str {
        match self {
            Route::Home => "house",
            Route::SurahIndex | Route::Reader(_) => "book",
            Route::JuzIndex => "square.stack.3d.up",
            Route::PagesIndex => "doc.text",
            Route::Search => "magnifyingglass",
            Route::Bookmarks => "bookmark",
            Route::Yours | Route::YoursCard(_) | Route::Account => "person",
            Route::Azkar | Route::Zikr { .. } | Route::DailyAyah => "sun.max",
            Route::Downloads => "arrow.down.circle",
            Route::Settings(_) => "gearshape",
            Route::Diagnostics => "waveform.path.ecg",
            _ => "doc",
        }
    }

    /// The test id of the route's tab in the themed (Android) tab bar.
    pub fn tab_id(&self) -> &'static str {
        match self {
            Route::Home => "tab-home",
            Route::SurahIndex => "tab-surah",
            Route::Search => "tab-search",
            Route::Bookmarks => "tab-bookmarks",
            _ => "tab-yours",
        }
    }

    /// The sidebar row's test id (`nav-<key>`).
    pub fn nav_row_id(&self) -> &'static str {
        match self {
            Route::Home => "nav-home",
            Route::SurahIndex => "nav-surah",
            Route::JuzIndex => "nav-juz",
            Route::PagesIndex => "nav-pages",
            Route::Search => "nav-search",
            Route::Bookmarks => "nav-bookmarks",
            Route::Yours => "nav-yours",
            Route::Azkar => "nav-azkar",
            Route::Downloads => "nav-downloads",
            Route::Settings(_) => "nav-settings",
            _ => "nav-other",
        }
    }

    /// One of the phone's tabs: a root there, so it shows no back button.
    pub fn is_tab(&self) -> bool {
        Route::TABS.contains(self)
    }

    /// A page pushed over its parent rather than opened in its place; the
    /// phone's tab bar steps aside while it is up.
    pub fn is_detail(&self) -> bool {
        matches!(
            self,
            Route::Reader(_) | Route::YoursCard(_) | Route::Zikr { .. }
        )
    }

    /// The route a detail sits on.
    pub fn parent(&self) -> Option<Route> {
        match self {
            Route::Reader(_) => Some(Route::SurahIndex),
            Route::YoursCard(_) => Some(Route::Yours),
            Route::Zikr { .. } => Some(Route::Azkar),
            _ => None,
        }
    }

    /// The page's content, made by the feature areas. Each arm calls the
    /// feature's page fn; those signatures are the mounting contract.
    pub fn page(&self) -> AnyElement {
        match self.clone() {
            Route::Home => features::home::page().into_element(),
            Route::SurahIndex => features::index::surah_index_page().into_element(),
            Route::JuzIndex => features::index::juz_index_page().into_element(),
            Route::PagesIndex => features::index::pages_index_page().into_element(),
            Route::Reader(target) => features::reader::page(target).into_element(),
            Route::Search => features::search::page().into_element(),
            Route::Bookmarks => features::bookmarks::page().into_element(),
            Route::Yours => features::yours::page().into_element(),
            Route::YoursCard(id) => features::yours::card_page(id).into_element(),
            Route::Azkar => features::azkar::page().into_element(),
            Route::Zikr { slug, translation } => {
                features::azkar::zikr_page(slug, translation).into_element()
            }
            Route::DailyAyah => features::daily_ayah::page().into_element(),
            Route::Downloads => features::downloads::page().into_element(),
            Route::Settings(section) => features::settings::page(section).into_element(),
            Route::Account => features::account::page().into_element(),
            Route::Login => features::auth::login_page().into_element(),
            Route::Register => features::auth::register_page().into_element(),
            Route::VerifyEmail => features::auth::verify_email_page().into_element(),
            Route::ForgotPassword => features::auth::forgot_password_page().into_element(),
            Route::Totp => features::auth::totp_page().into_element(),
            Route::Appeal(token) => features::appeal::page(token).into_element(),
            Route::Report => features::report::page().into_element(),
            Route::NewsletterConfirm(token) => {
                features::newsletter::confirm_page(token).into_element()
            }
            Route::About => features::marketing::about_page().into_element(),
            Route::Faq => features::marketing::faq_page().into_element(),
            Route::Contact => features::marketing::contact_page().into_element(),
            Route::Privacy => features::marketing::privacy_page().into_element(),
            Route::Terms => features::marketing::terms_page().into_element(),
            Route::Diagnostics => crate::foundation::diagnostics_content().into_element(),
        }
    }
}

/// The route a deep-link path names: the segments after `easyquran://`,
/// in order. `None` when the path is not the app's.
///
/// TODO(A2): the reader URL grammar (`/{surah}`, `/page/n`, `/juz/n`,
/// `/hizb/n`, `/t/{lang}/{translator}/…`, `?ayah=`, `newsletter/confirm?token=`)
/// is parsed by `eq_core::routes` once it lands; this fn then delegates to it
/// and keeps only the mapping to `Route`. Static paths only for now.
pub fn route_from_deep_link(seg: &[&str]) -> Option<Route> {
    match seg {
        [] | [""] => Some(Route::Home),
        ["surah"] => Some(Route::SurahIndex),
        ["juz"] => Some(Route::JuzIndex),
        ["pages"] => Some(Route::PagesIndex),
        ["search"] => Some(Route::Search),
        ["bookmarks"] => Some(Route::Bookmarks),
        ["yours"] => Some(Route::Yours),
        ["yours", "cards", id] => Some(Route::YoursCard((*id).to_owned())),
        ["azkar"] => Some(Route::Azkar),
        ["azkar", slug] => Some(Route::Zikr {
            slug: (*slug).to_owned(),
            translation: None,
        }),
        ["daily-ayah"] => Some(Route::DailyAyah),
        ["downloads"] => Some(Route::Downloads),
        ["settings"] => Some(Route::Settings(SettingsSection::Storage)),
        ["settings", slug] => SettingsSection::from_slug(slug).map(Route::Settings),
        ["account"] => Some(Route::Account),
        ["login"] => Some(Route::Login),
        ["register"] => Some(Route::Register),
        ["verify-email"] => Some(Route::VerifyEmail),
        ["forgot-password"] => Some(Route::ForgotPassword),
        ["appeal"] => Some(Route::Appeal(None)),
        ["appeal", token] => Some(Route::Appeal(Some((*token).to_owned()))),
        ["report"] => Some(Route::Report),
        ["about"] => Some(Route::About),
        ["faq"] => Some(Route::Faq),
        ["contact"] => Some(Route::Contact),
        ["privacy"] => Some(Route::Privacy),
        ["terms"] => Some(Route::Terms),
        ["diagnostics"] => Some(Route::Diagnostics),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_per_route_value() {
        let a = Route::YoursCard("a".into());
        let b = Route::YoursCard("b".into());
        assert_ne!(a.key(), b.key());
        assert_eq!(a.key_str(), b.key_str());
        let r1 = Route::Reader(ReaderTarget::arabic(ReaderScope::Surah(1)));
        let r2 = Route::Reader(ReaderTarget::arabic(ReaderScope::Page(1)));
        assert_ne!(r1.key(), r2.key());
    }

    #[test]
    fn destination_family_keys_are_distinct() {
        let mut keys: Vec<&str> = Route::DESTINATIONS.iter().map(Route::key_str).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), Route::DESTINATIONS.len());
    }
}
