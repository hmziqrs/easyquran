//! Copy the web writes outside its inlang catalogues, so it has no message
//! key: the not-found page (`routes/+error.svelte`, English literals) and
//! the English-only page labels (`lib/i18n/base-english-copy.ts`).
//! Transcribed exactly. The web shows these strings in English under both
//! locales, so the Arabic fns return the same English text until the web
//! catalogues them; then they move to `web/messages` and this file goes.

use super::Locale;

/// The site's domain, as the not-found page names it (`SITE.domain`).
pub const SITE_DOMAIN: &str = "easyquran.fyi";

/// One quick link of the not-found page: its label and the hint under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickLink {
    pub label: &'static str,
    pub hint: &'static str,
}

/// The not-found page's copy (`+error.svelte`, the 404 branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotFoundCopy {
    /// `<title>`: "Page not found".
    pub page_title: &'static str,
    pub heading: &'static str,
    /// The body; `{domain}` is [`SITE_DOMAIN`].
    pub body: &'static str,
    pub back_home: &'static str,
    pub browse_surahs: &'static str,
    pub search_quran: &'static str,
    pub have_account: &'static str,
    pub sign_in: &'static str,
    pub new_here: &'static str,
    pub create_account: &'static str,
    pub often_opened: &'static str,
    pub all_surahs: &'static str,
    /// A popular surah's verse count; `{count}` is the number.
    pub verses: &'static str,
    pub keep_reading: &'static str,
    pub search: QuickLink,
    pub browse_juz: QuickLink,
    pub browse_pages: QuickLink,
    pub history: QuickLink,
    pub account_and_tools: &'static str,
    pub sign_in_link: QuickLink,
    pub create_account_link: QuickLink,
    pub bookmarks: QuickLink,
    pub settings: QuickLink,
}

const NOT_FOUND_EN: NotFoundCopy = NotFoundCopy {
    page_title: "Page not found",
    heading: "This page wandered off the path",
    body: "The page you're looking for isn't on {domain}. It may have moved, or the link has a typo — pick a door below instead.",
    back_home: "Back to home",
    browse_surahs: "Browse surahs",
    search_quran: "Search the Qur\u{2019}an",
    have_account: "Have an account?",
    sign_in: "Sign in",
    new_here: "New here?",
    create_account: "Create account",
    often_opened: "Often opened",
    all_surahs: "All 114 surahs",
    verses: "{count} verses",
    keep_reading: "Keep reading",
    search: QuickLink {
        label: "Search the Qur'an",
        hint: "Words, phrases, references",
    },
    browse_juz: QuickLink {
        label: "Browse juz",
        hint: "All 30 parts",
    },
    browse_pages: QuickLink {
        label: "Browse pages",
        hint: "All 604 mushaf pages",
    },
    history: QuickLink {
        label: "Your history",
        hint: "Pick up where you left off",
    },
    account_and_tools: "Account and tools",
    sign_in_link: QuickLink {
        label: "Sign in",
        hint: "Sync bookmarks across devices",
    },
    create_account_link: QuickLink {
        label: "Create account",
        hint: "Free, in under a minute",
    },
    bookmarks: QuickLink {
        label: "Bookmarks",
        hint: "Your saved verses",
    },
    settings: QuickLink {
        label: "Settings",
        hint: "Reading and appearance",
    },
};

/// The not-found copy, English.
pub fn not_found_en() -> &'static NotFoundCopy {
    &NOT_FOUND_EN
}

/// The not-found copy, Arabic: the web shows English here too.
pub fn not_found_ar() -> &'static NotFoundCopy {
    &NOT_FOUND_EN
}

/// The not-found copy in `locale`.
pub fn not_found(locale: Locale) -> &'static NotFoundCopy {
    match locale {
        Locale::En => not_found_en(),
        Locale::Ar => not_found_ar(),
    }
}

impl NotFoundCopy {
    /// The body with the domain in it.
    pub fn body_text(&self) -> String {
        self.body.replace("{domain}", SITE_DOMAIN)
    }

    /// "7 verses".
    pub fn verse_count(&self, count: u16) -> String {
        self.verses.replace("{count}", &count.to_string())
    }
}

/// A marketing page (`MarketingPageId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketingPage {
    Home,
    About,
    Faq,
    Contact,
    Privacy,
    Terms,
}

/// A page's English label, title and description (`BaseEnglishPageCopy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageCopy {
    pub label: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

/// `baseEnglishPageCopy`, English.
pub fn page_copy_en(page: MarketingPage) -> PageCopy {
    match page {
        MarketingPage::Home => PageCopy {
            label: "Home",
            title: "EasyQuran · the Quran, made easy to read",
            description: "Read the Quran. Free, no ads, and fast — nothing in the way of the text.",
        },
        MarketingPage::About => PageCopy {
            label: "About",
            title: "About · EasyQuran",
            description: "What EasyQuran is, who it's for, and how it's built.",
        },
        MarketingPage::Faq => PageCopy {
            label: "FAQ",
            title: "FAQ · EasyQuran",
            description: "Questions about EasyQuran, answered plainly.",
        },
        MarketingPage::Contact => PageCopy {
            label: "Contact",
            title: "Contact · EasyQuran",
            description: "Get in touch — bug reports, script corrections and feature ideas.",
        },
        MarketingPage::Privacy => PageCopy {
            label: "Privacy",
            title: "Privacy · EasyQuran",
            description: "What EasyQuran collects, what it doesn't, and why.",
        },
        MarketingPage::Terms => PageCopy {
            label: "Terms",
            title: "Terms · EasyQuran",
            description: "The terms under which EasyQuran is provided.",
        },
    }
}

/// `baseEnglishPageCopy`, Arabic: English-only on the web.
pub fn page_copy_ar(page: MarketingPage) -> PageCopy {
    page_copy_en(page)
}
