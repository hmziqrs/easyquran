//! Static pages (PARITY A4, A5): about, FAQ, contact, privacy, terms, and
//! the footer. Contract: `pub fn about_page()`, `faq_page()`,
//! `contact_page()`, `privacy_page()`, `terms_page()`, each
//! `-> impl IntoElement`; `pub fn footer() -> impl IntoElement` for the
//! home screen (and the not-found page) to mount.
//!
//! Mirrors `web/src/routes/(marketing)/{about,faq,contact,privacy,terms}`
//! and `lib/components/footer/Footer.svelte`. All copy is the web's
//! catalogue (`about_*`, `faq_*`, `contact_*`, `legal_*`, `footer_*`).
//! Each page body is made again on a locale switch (`dynamic` over
//! `locale()`), so lists and spoken names follow it.
//!
//! Divergence: the contact page's X row uses the web's fallback owner
//! profile (`lib/server/owner.ts` `FALLBACK_PROFILE`, `@hmziqrs`); the web
//! server may replace it with a remote profile this app does not fetch.

use eq_core::i18n::{self, Key, Locale, keys};
use eq_core::quran::catalogue;
use nsh_core::prelude::*;

use crate::shell::{
    nav,
    routes::{Route, route_from_link},
};
use crate::stores::locale::locale;
use crate::ui::{
    brand, button,
    collapse::{AccordionEntry, accordion_of},
    icons::lucide,
    layout::{self, PanelVariant},
    link::link_out,
    text::{self, TextRole},
    tokens,
};

// --- Site facts (`lib/config/site.ts`): proper nouns and addresses, not copy --

const OWNER_URL: &str = "https://hmziq.rs";
const OWNER: &str = "hmziq.rs";
const MAKER_URL: &str = "https://oxlabs.dev";
const MAKER: &str = "oxlabs.dev";
const TANZIL_URL: &str = "https://tanzil.net";
const CONTACT_EMAIL: &str = "support@easyquran.fyi";
const ABUSE_EMAIL: &str = "abuse@easyquran.fyi";
const OWNER_X_URL: &str = "https://x.com/hmziqrs";
const OWNER_X_HANDLE: &str = "@hmziqrs";

/// A data source on the about page: its published name and site, if any.
struct Source {
    name: &'static str,
    href: Option<&'static str>,
}

const ARABIC_SOURCES: [Source; 5] = [
    Source {
        name: "Tanzil.net",
        href: Some(TANZIL_URL),
    },
    Source {
        name: "quran.com",
        href: Some("https://quran.com"),
    },
    Source {
        name: "Dar Al-Islam",
        href: None,
    },
    Source {
        name: "Naveed Ahmad",
        href: None,
    },
    Source {
        name: "Al Quran Cloud",
        href: Some("https://alquran.cloud"),
    },
];

const TRANSLATION_SOURCES: [Source; 3] = [
    Source {
        name: "Tanzil.net",
        href: Some(TANZIL_URL),
    },
    Source {
        name: "QuranEnc",
        href: Some("https://quranenc.com"),
    },
    Source {
        name: "QUL (Tarteel)",
        href: Some("https://qul.tarteel.ai"),
    },
];

const MAIL_ICON: &str = lucide!(
    r#"<rect width="20" height="16" x="2" y="4" rx="2"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/>"#
);
const ARROW_RIGHT: &str = lucide!(r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#);
const ARROW_LEFT: &str = lucide!(r#"<path d="M19 12H5"/><path d="m12 19-7-7 7-7"/>"#);
/// The X mark (`x-brand`), filled in `currentColor`.
const X_ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932ZM17.61 20.644h2.039L6.486 3.24H4.298Z"/></svg>"#;

/// The forward arrow, pointing with the reading direction.
pub(crate) fn forward_arrow(l: Locale) -> &'static str {
    if l.is_rtl() {
        return ARROW_LEFT;
    }
    ARROW_RIGHT
}

// --- Frame --------------------------------------------------------------------

/// A static page: one scroll, the page column capped at a reading measure,
/// its content made for the current locale.
fn static_page(content: impl Fn(Locale) -> AnyElement + 'static) -> impl IntoElement {
    scroll()
        .flex_1()
        .content_inset(layout::page_insets(0.0))
        .child(
            layout::page_body().items_center().child(
                view()
                    .w_full()
                    .max_w(px(680.0))
                    .child(dynamic(move || content(locale()))),
            ),
        )
}

fn tr(l: Locale, key: Key) -> &'static str {
    i18n::tr(l, key)
}

/// The page's heading in the ramp's h1, where the platform does not show
/// it in its own header.
fn heading(label: &'static str) -> impl IntoElement {
    when(text::page_heading_shown, move || {
        text::role(label, TextRole::H2).into_element()
    })
}

/// Secondary body copy (`text-foreground-secondary`).
fn secondary(label: impl Into<SharedString>, role: TextRole) -> Text {
    text::role(label.into(), role).text_color(tokens::foreground_secondary)
}

/// "Prompt [link]." — an in-app link at the end of a sentence.
fn prompt_with_link(prompt: &'static str, link: &'static str, to: Route) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(secondary(prompt, TextRole::Body))
        .child(button::button_with(
            link,
            button::ButtonVariant::Ghost,
            button::ButtonSize::Small,
            false,
            None,
            move || nav::push(to.clone()),
        ))
}

// --- About --------------------------------------------------------------------

/// The catalogue's totals (`about/+page.server.ts`): translations and
/// distinct languages.
fn catalogue_counts() -> (usize, usize) {
    let all = catalogue::translations();
    let mut languages: Vec<&str> = all.iter().map(|t| t.language.as_str()).collect();
    languages.sort_unstable();
    languages.dedup();
    (all.len(), languages.len())
}

pub fn about_page() -> impl IntoElement {
    let (translations, languages) = catalogue_counts();
    static_page(move |l| {
        let features = [
            tr(l, keys::ABOUT_FEATURE_SCRIPTS).to_owned(),
            i18n::tr_args(
                l,
                keys::ABOUT_FEATURE_TRANSLATIONS,
                &[
                    ("translations", translations.to_string().into()),
                    ("languages", languages.to_string().into()),
                ],
            ),
            tr(l, keys::ABOUT_FEATURE_TOOLS).to_owned(),
        ];
        let groups = [
            (tr(l, keys::ABOUT_SOURCES_ARABIC), &ARABIC_SOURCES[..]),
            (
                tr(l, keys::ABOUT_SOURCES_TRANSLATIONS),
                &TRANSLATION_SOURCES[..],
            ),
        ];
        view()
            .w_full()
            .flex_col()
            .gap(px(40.0))
            .child(
                view()
                    .flex_col()
                    .gap_4()
                    .child(heading(tr(l, keys::ABOUT_HEADING)))
                    .child(secondary(tr(l, keys::ABOUT_INTRO), TextRole::BodyL))
                    .child(
                        view()
                            .flex_col()
                            .gap_2p5()
                            .children(features.into_iter().map(feature_bullet)),
                    ),
            )
            .child(
                view()
                    .flex_col()
                    .gap_3()
                    .child(layout::divider())
                    .child(text::role(tr(l, keys::ABOUT_SOURCES_HEADING), TextRole::H3).pt_4())
                    .child(
                        view().flex_col().gap_2().children(
                            groups
                                .into_iter()
                                .map(|(label, sources)| source_group(label, sources)),
                        ),
                    )
                    .child(
                        text::role(tr(l, keys::ABOUT_SOURCES_NOTE), TextRole::Caption)
                            .text_color(tokens::muted_text),
                    ),
            )
            .child(layout::divider())
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(secondary(
                        tr(l, keys::ABOUT_CREDIT_PROJECT_BY),
                        TextRole::Body,
                    ))
                    .child(link_out(OWNER_URL, OWNER))
                    .child(secondary(
                        format!(", {}", tr(l, keys::ABOUT_CREDIT_BUILT_BY)),
                        TextRole::Body,
                    ))
                    .child(link_out(MAKER_URL, MAKER))
                    .child(secondary(".", TextRole::Body))
                    .child(button::button_with(
                        tr(l, keys::ABOUT_CTA),
                        button::ButtonVariant::Ghost,
                        button::ButtonSize::Small,
                        false,
                        None,
                        || nav::push(Route::Contact),
                    )),
            )
            .into_element()
    })
}

fn feature_bullet(body: String) -> impl IntoElement {
    view()
        .flex_row()
        .items_start()
        .gap_3()
        .child(
            view()
                .mt(px(10.0))
                .size(px(6.0))
                .flex_none()
                .rounded_full()
                .bg(tokens::border_strong)
                .accessible(false),
        )
        .child(view().flex_1().child(secondary(body, TextRole::Body)))
}

fn source_group(label: &'static str, sources: &'static [Source]) -> impl IntoElement {
    let wide = is_desktop();
    view()
        .flex_col()
        .gap_0p5()
        .when(wide, |v| v.flex_row().gap_4())
        .child(
            text::role(label, TextRole::Body)
                .text_color(tokens::muted_text)
                .when(wide, |t| t.w(px(128.0)))
                .flex_none(),
        )
        .child(
            view()
                .flex_1()
                .flex_row()
                .flex_wrap()
                .gap_x(px(16.0))
                .gap_y(px(4.0))
                .children(sources.iter().map(|source| match source.href {
                    Some(href) => link_out(href, source.name).into_element(),
                    None => secondary(source.name, TextRole::Body).into_element(),
                })),
        )
}

// --- FAQ ----------------------------------------------------------------------

const FAQ: [(&str, Key, Key); 7] = [
    ("free", keys::FAQ_FREE_QUESTION, keys::FAQ_FREE_ANSWER),
    (
        "account",
        keys::FAQ_ACCOUNT_QUESTION,
        keys::FAQ_ACCOUNT_ANSWER,
    ),
    ("script", keys::FAQ_SCRIPT_QUESTION, keys::FAQ_SCRIPT_ANSWER),
    (
        "offline",
        keys::FAQ_OFFLINE_QUESTION,
        keys::FAQ_OFFLINE_ANSWER,
    ),
    ("next", keys::FAQ_NEXT_QUESTION, keys::FAQ_NEXT_ANSWER),
    (
        "translations",
        keys::FAQ_TRANSLATIONS_QUESTION,
        keys::FAQ_TRANSLATIONS_ANSWER,
    ),
    ("backup", keys::FAQ_BACKUP_QUESTION, keys::FAQ_BACKUP_ANSWER),
];

pub fn faq_page() -> impl IntoElement {
    // One open entry at a time (`type="single"`), kept across a locale
    // switch: entries are named by stable ids.
    let open = signal(Vec::<SharedString>::new());
    static_page(move |l| {
        let entries: Vec<AccordionEntry> = FAQ
            .iter()
            .map(|&(id, q, a)| AccordionEntry {
                id,
                title: tr(l, q),
                content: tr(l, a).into(),
            })
            .collect();
        view()
            .w_full()
            .flex_col()
            .gap(px(30.0))
            .child(
                view()
                    .flex_col()
                    .gap_3()
                    .child(layout::eyebrow(tr(l, keys::FAQ_EYEBROW)))
                    .child(heading(tr(l, keys::FAQ_HEADING))),
            )
            .child(
                view()
                    .border_t_1()
                    .border_color(tokens::border_strong)
                    .child(accordion_of(&entries, open, false)),
            )
            .child(prompt_with_link(
                tr(l, keys::FAQ_MISSING_PROMPT),
                tr(l, keys::FAQ_MISSING_LINK),
                Route::Contact,
            ))
            .into_element()
    })
}

// --- Contact ------------------------------------------------------------------

pub fn contact_page() -> impl IntoElement {
    static_page(|l| {
        let rows = [
            (
                MAIL_ICON,
                keys::CONTACT_EMAIL_TITLE,
                CONTACT_EMAIL,
                format!("mailto:{CONTACT_EMAIL}"),
            ),
            (
                MAIL_ICON,
                keys::CONTACT_ABUSE_TITLE,
                ABUSE_EMAIL,
                format!("mailto:{ABUSE_EMAIL}"),
            ),
            (
                X_ICON,
                keys::CONTACT_X_TITLE,
                OWNER_X_HANDLE,
                OWNER_X_URL.to_owned(),
            ),
        ];
        view()
            .w_full()
            .flex_col()
            .gap_8()
            .child(
                view()
                    .flex_col()
                    .gap_3()
                    .child(heading(tr(l, keys::CONTACT_HEADING)))
                    .child(secondary(tr(l, keys::CONTACT_INTRO), TextRole::BodyL)),
            )
            .child(
                view()
                    .w_full()
                    .flex_col()
                    .gap_2()
                    .children(rows.into_iter().map(move |(icon, title, value, href)| {
                        contact_row(icon, tr(l, title), value, href, l)
                    })),
            )
            .into_element()
    })
}

/// One way to reach us: icon tile, caption over the address, an arrow;
/// the press opens it (mail app, browser).
fn contact_row(
    icon: &'static str,
    title: &'static str,
    value: &'static str,
    href: String,
    l: Locale,
) -> impl IntoElement {
    let (pad_x, pad_y) = if is_desktop() {
        (16.0, 10.0)
    } else {
        (20.0, 16.0)
    };
    layout::pressable_card(format!("{title}: {value}"), move || {
        let href = href.clone();
        spawn(async move {
            open_url(href).await;
        });
    })
    .flex_row()
    .items_center()
    .gap_4()
    .px(px(pad_x))
    .py(px(pad_y))
    .child(
        view()
            .size(px(36.0))
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(tokens::radius_md)
            .bg(tokens::background_subtle)
            .child(icon_svg(icon, 17.0, tokens::foreground_secondary)),
    )
    .child(
        view()
            .flex_1()
            .min_w(px(0.0))
            .flex_col()
            .child(text::role(title, TextRole::Caption).text_color(tokens::muted_text))
            .child(
                text::role(value, TextRole::Body)
                    .font_weight(FontWeight::MEDIUM)
                    .number_of_lines(1),
            ),
    )
    .child(icon_svg(forward_arrow(l), 15.0, tokens::muted_text))
}

pub(crate) fn icon_svg(
    markup: &'static str,
    side: f32,
    color: fn() -> AdaptiveColor,
) -> impl IntoElement {
    svg(nsh_core::element::ImageSrc::svg_markup(markup))
        .size(px(side))
        .flex_none()
        .accessible(false)
        .text_color(color)
}

// --- Privacy and terms --------------------------------------------------------

const PRIVACY_SECTIONS: [(&str, Key, Key); 8] = [
    (
        "collect",
        keys::LEGAL_PRIVACY_COLLECT_HEADING,
        keys::LEGAL_PRIVACY_COLLECT_BODY,
    ),
    (
        "accounts",
        keys::LEGAL_PRIVACY_ACCOUNTS_HEADING,
        keys::LEGAL_PRIVACY_ACCOUNTS_BODY,
    ),
    (
        "device",
        keys::LEGAL_PRIVACY_DEVICE_HEADING,
        keys::LEGAL_PRIVACY_DEVICE_BODY,
    ),
    (
        "hosting",
        keys::LEGAL_PRIVACY_HOSTING_HEADING,
        keys::LEGAL_PRIVACY_HOSTING_BODY,
    ),
    (
        "notifications",
        keys::LEGAL_PRIVACY_NOTIFICATIONS_HEADING,
        keys::LEGAL_PRIVACY_NOTIFICATIONS_BODY,
    ),
    (
        "children",
        keys::LEGAL_PRIVACY_CHILDREN_HEADING,
        keys::LEGAL_PRIVACY_CHILDREN_BODY,
    ),
    (
        "choices",
        keys::LEGAL_PRIVACY_CHOICES_HEADING,
        keys::LEGAL_PRIVACY_CHOICES_BODY,
    ),
    (
        "changes",
        keys::LEGAL_PRIVACY_CHANGES_HEADING,
        keys::LEGAL_PRIVACY_CHANGES_BODY,
    ),
];

const TERMS_SECTIONS: [(&str, Key, Key); 8] = [
    (
        "acceptance",
        keys::LEGAL_TERMS_ACCEPTANCE_HEADING,
        keys::LEGAL_TERMS_ACCEPTANCE_BODY,
    ),
    (
        "service",
        keys::LEGAL_TERMS_SERVICE_HEADING,
        keys::LEGAL_TERMS_SERVICE_BODY,
    ),
    (
        "accuracy",
        keys::LEGAL_TERMS_ACCURACY_HEADING,
        keys::LEGAL_TERMS_ACCURACY_BODY,
    ),
    (
        "content",
        keys::LEGAL_TERMS_CONTENT_HEADING,
        keys::LEGAL_TERMS_CONTENT_BODY,
    ),
    (
        "use",
        keys::LEGAL_TERMS_USE_HEADING,
        keys::LEGAL_TERMS_USE_BODY,
    ),
    (
        "third-party",
        keys::LEGAL_TERMS_THIRD_PARTY_HEADING,
        keys::LEGAL_TERMS_THIRD_PARTY_BODY,
    ),
    (
        "liability",
        keys::LEGAL_TERMS_LIABILITY_HEADING,
        keys::LEGAL_TERMS_LIABILITY_BODY,
    ),
    (
        "contact",
        keys::LEGAL_TERMS_CONTACT_HEADING,
        keys::LEGAL_TERMS_CONTACT_BODY,
    ),
];

/// A legal page (`LegalPageCopy`): eyebrow, heading, the updated line, an
/// optional summary panel, the sections, then the outro link.
struct Legal {
    heading: Key,
    summary: Option<Key>,
    sections: &'static [(&'static str, Key, Key)],
    outro_prompt: Key,
    outro_link: Key,
    outro_to: Route,
}

fn legal_page(legal: Legal) -> impl IntoElement {
    static_page(move |l| {
        let updated = i18n::tr_args(
            l,
            keys::LEGAL_UPDATED,
            &[("date", tr(l, keys::LEGAL_UPDATED_DATE).into())],
        );
        view()
            .w_full()
            .flex_col()
            .gap(px(26.0))
            .child(
                view()
                    .flex_col()
                    .gap_2p5()
                    .child(layout::eyebrow(tr(l, keys::LEGAL_EYEBROW)))
                    .child(heading(tr(l, legal.heading)))
                    .child(
                        text::role(
                            format!("{updated} · {}", tr(l, keys::LEGAL_PLACEHOLDER_NOTE)),
                            TextRole::Caption,
                        )
                        .text_color(tokens::muted_text),
                    ),
            )
            .children(legal.summary.map(|key| {
                layout::panel(PanelVariant::Soft)
                    .px(px(22.0))
                    .py_5()
                    .child(text::role(tr(l, key), TextRole::Body))
            }))
            .child(
                view()
                    .flex_col()
                    .gap(px(26.0))
                    .children(legal.sections.iter().map(|&(_, h, b)| {
                        view()
                            .flex_col()
                            .gap(px(9.0))
                            .child(text::role(tr(l, h), TextRole::BodyL))
                            .child(secondary(tr(l, b), TextRole::Body))
                    })),
            )
            .child(layout::divider())
            .child(prompt_with_link(
                tr(l, legal.outro_prompt),
                tr(l, legal.outro_link),
                legal.outro_to.clone(),
            ))
            .into_element()
    })
}

pub fn privacy_page() -> impl IntoElement {
    legal_page(Legal {
        heading: keys::LEGAL_PRIVACY_HEADING,
        summary: Some(keys::LEGAL_PRIVACY_SUMMARY),
        sections: &PRIVACY_SECTIONS,
        outro_prompt: keys::LEGAL_PRIVACY_QUESTIONS_PROMPT,
        outro_link: keys::LEGAL_PRIVACY_QUESTIONS_LINK,
        outro_to: Route::Contact,
    })
}

pub fn terms_page() -> impl IntoElement {
    legal_page(Legal {
        heading: keys::LEGAL_TERMS_HEADING,
        summary: None,
        sections: &TERMS_SECTIONS,
        outro_prompt: keys::LEGAL_TERMS_PRIVACY_PROMPT,
        outro_link: keys::LEGAL_TERMS_PRIVACY_LINK,
        outro_to: Route::Privacy,
    })
}

// --- Footer -------------------------------------------------------------------

/// A footer column: its heading, its spoken name and its links (web
/// hrefs, opened through the route grammar).
type FooterColumn = (Key, Key, &'static [(Key, &'static str)]);

const FOOTER_COLUMNS: [FooterColumn; 3] = [
    (
        keys::FOOTER_PRODUCT_HEADING,
        keys::FOOTER_PRODUCT_LABEL,
        &[
            (keys::FOOTER_READ_QURAN, "/surah"),
            (keys::FOOTER_BOOKMARKS, "/bookmarks"),
            (keys::FOOTER_WHATS_INSIDE, "/#surahs"),
        ],
    ),
    (
        keys::FOOTER_COMPANY_HEADING,
        keys::FOOTER_COMPANY_LABEL,
        &[
            (keys::FOOTER_ABOUT, "/about"),
            (keys::FOOTER_FAQ, "/faq"),
            (keys::FOOTER_CONTACT, "/contact"),
        ],
    ),
    (
        keys::FOOTER_LEGAL_HEADING,
        keys::FOOTER_LEGAL_LABEL,
        &[
            (keys::FOOTER_PRIVACY, "/privacy"),
            (keys::FOOTER_TERMS, "/terms"),
        ],
    ),
];

/// The route a footer href opens; the home page for one the grammar does
/// not know (`/#surahs`, the landing's surah list, is the home page).
fn footer_route(href: &str) -> Route {
    eq_core::routes::parse_path(href).map_or(Route::Home, route_from_link)
}

/// The site footer (`Footer.svelte`): the brand and blurb, the product,
/// company and legal columns, the copyright and credits.
pub fn footer() -> impl IntoElement {
    view()
        .w_full()
        .flex_col()
        .border_t_1()
        .border_color(tokens::border_strong)
        .bg(tokens::surface)
        .child(dynamic(move || footer_body(locale()).into_element()))
}

fn footer_body(l: Locale) -> impl IntoElement {
    let brand_name = "easyquran";
    view()
        .w_full()
        .flex_col()
        .gap_8()
        .px_6()
        .pt(px(40.0))
        .pb(px(32.0))
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .gap(px(32.0))
                .child(
                    view()
                        .flex_col()
                        .gap_3()
                        .min_w(px(240.0))
                        .flex_1()
                        .child(brand::brand(false))
                        .child(
                            secondary(tr(l, keys::FOOTER_BLURB), TextRole::Body).max_w(px(300.0)),
                        ),
                )
                .children(
                    FOOTER_COLUMNS
                        .iter()
                        .map(move |&(heading_key, label_key, links)| {
                            view()
                                .flex_col()
                                .gap_2()
                                .min_w(px(120.0))
                                .label(tr(l, label_key))
                                .child(
                                    text::role(tr(l, heading_key), TextRole::Body)
                                        .font_weight(FontWeight::MEDIUM),
                                )
                                .children(links.iter().map(move |&(key, href)| {
                                    button::button_with(
                                        tr(l, key),
                                        button::ButtonVariant::Ghost,
                                        button::ButtonSize::Small,
                                        false,
                                        None,
                                        move || nav::push(footer_route(href)),
                                    )
                                    .self_start()
                                }))
                        }),
                ),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_1()
                .child(
                    text::role(
                        format!(
                            "© {} {brand_name}. {}",
                            current_year(),
                            tr(l, keys::FOOTER_BUILT_BY)
                        ),
                        TextRole::Body,
                    )
                    .text_color(tokens::muted_text),
                )
                .child(link_out(MAKER_URL, MAKER))
                .child(text::role("&", TextRole::Body).text_color(tokens::muted_text))
                .child(link_out(OWNER_URL, OWNER)),
        )
}

/// The current year (UTC), for the copyright line.
fn current_year() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400;
    if month <= 2 {
        return year + 1;
    }
    year
}
