import { uiDirection, type UiDirection, type UiLocale } from "$lib/i18n/locales";
import type {
  FooterResolvedCopy,
  NavResolvedCopy,
  TweaksResolvedCopy,
} from "$lib/i18n/marketing-copy";
import { m } from "$lib/paraglide/messages.js";
import { getLocale } from "$lib/paraglide/runtime.js";

type BrowseMode = "surah" | "ayah" | "juz" | "page";
type RangeKind = "juz" | "page";

export interface ReaderUiCopy {
  readonly locale: UiLocale;
  readonly direction: UiDirection;
  readonly skipToContent: string;
  readonly shell: {
    readonly opening: string;
    readonly surahPage: (surah: number, page: number, count: number) => string;
    readonly surahPageTitle: (name: string, page: number, count: number) => string;
    readonly pageOf: (page: number, count: number) => string;
    readonly surahPagesLabel: string;
    readonly arabicTextSizeLabel: string;
    readonly smallerArabicTextLabel: string;
    readonly largerArabicTextLabel: string;
    readonly readingModeLabel: string;
    readonly ayahByAyah: string;
    readonly ayahs: string;
    readonly reading: string;
    readonly continueReading: (reference: string) => string;
    readonly jump: string;
    readonly retry: string;
    readonly translationUnavailable: string;
    readonly pageUnavailable: (page: number) => string;
    readonly offlineCopyUnavailable: string;
    readonly networkUnavailable: string;
    readonly offlineDataUnavailable: string;
    readonly moreAyahsUnavailable: string;
  };
  readonly sidebar: {
    readonly searchPlaceholder: string;
    readonly searchLabel: string;
    readonly clearSearch: string;
    readonly browseLabel: string;
    readonly mode: (mode: BrowseMode) => string;
    readonly loadingNavigation: string;
    readonly navigationError: string;
    readonly navigationDescription: string;
    readonly tip: string;
    readonly pageAbbreviation: string;
    readonly juzLabel: string;
    readonly rangeItem: (kind: RangeKind, index: number) => string;
  };
  readonly search: {
    readonly searching: string;
    readonly noVerseMatches: (query: string) => string;
    readonly surahMatches: (count: number, query: string) => string;
    readonly textMatches: (count: number, query: string) => string;
    readonly openSurah: string;
    readonly unavailable: string;
    readonly surahOpener: (name: string) => string;
    readonly ayah: (name: string, surah: number, ayah: number) => string;
  };
  readonly range: {
    readonly juz: string;
    readonly page: string;
    readonly pageAbbreviation: string;
    readonly fullSurah: string;
    readonly translationUnavailable: string;
    readonly item: (kind: RangeKind, index: number) => string;
    readonly juzCount: (count: number) => string;
  };
  readonly sources: {
    readonly source: string;
    readonly arabic: string;
    readonly original: string;
    readonly default: string;
    readonly noTranslations: string;
  };
  readonly verse: {
    readonly bookmark: string;
    readonly removeBookmark: string;
    readonly bookmarkVerse: string;
    readonly copy: string;
    readonly copied: string;
    readonly copyAyah: string;
    readonly share: string;
    readonly shareVerse: string;
    readonly noteTafsir: string;
    readonly openNoteTafsir: string;
    readonly closeNoteTafsir: string;
    readonly tafsir: string;
    readonly yourNote: string;
    readonly notePlaceholder: string;
    readonly noteSaved: string;
  };
  readonly nav: NavResolvedCopy & {
    readonly sidebarToggle: string;
    readonly sidebarTitle: string;
    readonly homeLabel: (siteName: string) => string;
  };
  readonly settings: TweaksResolvedCopy & {
    readonly background: string;
    readonly pop: string;
    readonly accentColour: (label: string) => string;
    readonly accentReset: (label: string) => string;
    readonly themeDerivedNote: string;
  };
  readonly footer: FooterResolvedCopy;
  readonly footerLinks: {
    readonly readQuran: string;
    readonly bookmarks: string;
    readonly whatsInside: string;
    readonly about: string;
    readonly faq: string;
    readonly contact: string;
    readonly privacy: string;
    readonly terms: string;
  };
  readonly offline: {
    readonly label: string;
    readonly packReady: string;
    readonly preparingPack: string;
    readonly downloadingPack: string;
    readonly stagingPack: string;
    readonly downloadFailed: string;
    readonly download: string;
    readonly remove: string;
    readonly working: string;
    readonly routes: (count: number) => string;
    readonly savedOn: (date: string) => string;
    readonly storage: (usage: string, quota: string) => string;
    readonly preparingQuran: string;
  };
  readonly notifications: {
    readonly title: string;
    readonly enable: string;
    readonly disable: string;
    readonly blocked: string;
    readonly unsupported: string;
    readonly unavailable: string;
    readonly checking: string;
    readonly browserUnsupported: string;
    readonly blockedDetail: string;
    readonly on: string;
    readonly offUpdates: string;
    readonly off: string;
    readonly dismiss: string;
    readonly open: string;
  };
  readonly update: {
    readonly ready: string;
    readonly reloadDescription: string;
    readonly reloadOpenTabs: string;
    readonly dismiss: string;
  };
  readonly seo: {
    readonly home: string;
    readonly quran: string;
    readonly juzIndexTitle: string;
    readonly juzIndexDescription: string;
    readonly juzTitle: (index: number, first: string, last: string) => string;
    readonly juzDescription: (index: number, first: string, last: string) => string;
    readonly pageTitle: (index: number, first: string, last: string) => string;
    readonly pageDescription: (index: number, first: string, last: string) => string;
    readonly translationJuzDescription: (index: number, first: string, last: string) => string;
    readonly translationPageDescription: (index: number, first: string, last: string) => string;
    readonly surahTitle: (surah: number, name: string) => string;
    readonly surahPageTitle: (surah: number, name: string, page: number, count: number) => string;
    readonly surahDescriptionUthmani: (
      name: string,
      arabic: string,
      page: number,
      count: number,
      start: number,
      end: number,
    ) => string;
    readonly surahDescriptionTranslation: (
      name: string,
      arabic: string,
      page: number,
      count: number,
      start: number,
      end: number,
    ) => string;
    readonly breadcrumbSurah: (name: string) => string;
    readonly quranBook: string;
  };
}

function createReaderUiCopy(locale: UiLocale): ReaderUiCopy {
  const options: { locale: UiLocale } = { locale };
  const noArgs = <Inputs>(
    message: (inputs?: Inputs, options?: { locale?: UiLocale }) => string,
  ): string =>
    message(undefined, options);

  const mode = (value: BrowseMode): string => {
    switch (value) {
      case "surah":
        return noArgs(m.reader_browse_surah);
      case "ayah":
        return noArgs(m.reader_browse_ayah);
      case "juz":
        return noArgs(m.reader_browse_juz);
      case "page":
        return noArgs(m.reader_browse_page);
    }
  };

  const rangeItem = (kind: RangeKind, index: number): string =>
    kind === "juz"
      ? m.reader_juz_item({ index }, options)
      : m.reader_page_item({ index }, options);

  return {
    locale,
    direction: uiDirection(locale),
    skipToContent: noArgs(m.skip_to_content),
    shell: {
      opening: noArgs(m.reader_opening),
      surahPage: (surah, page, count) => m.reader_surah_page({ surah, page, count }, options),
      surahPageTitle: (name, page, count) =>
        m.reader_surah_page_title({ name, page, count }, options),
      pageOf: (page, count) => m.reader_page_of({ page, count }, options),
      surahPagesLabel: noArgs(m.reader_surah_pages),
      arabicTextSizeLabel: noArgs(m.reader_arabic_text_size),
      smallerArabicTextLabel: noArgs(m.reader_smaller_arabic_text),
      largerArabicTextLabel: noArgs(m.reader_larger_arabic_text),
      readingModeLabel: noArgs(m.reader_reading_mode),
      ayahByAyah: noArgs(m.reader_ayah_by_ayah),
      ayahs: noArgs(m.reader_ayahs),
      reading: noArgs(m.reader_reading),
      continueReading: (reference) => m.reader_continue_reading({ reference }, options),
      jump: noArgs(m.reader_jump),
      retry: noArgs(m.reader_retry),
      translationUnavailable: noArgs(m.reader_translation_unavailable),
      pageUnavailable: (page) => m.reader_page_unavailable({ page }, options),
      offlineCopyUnavailable: noArgs(m.reader_offline_copy_unavailable),
      networkUnavailable: noArgs(m.reader_network_unavailable),
      offlineDataUnavailable: noArgs(m.reader_offline_data_unavailable),
      moreAyahsUnavailable: noArgs(m.reader_more_ayahs_unavailable),
    },
    sidebar: {
      searchPlaceholder: noArgs(m.reader_search_placeholder),
      searchLabel: noArgs(m.reader_search_label),
      clearSearch: noArgs(m.reader_clear_search),
      browseLabel: noArgs(m.reader_browse),
      mode,
      loadingNavigation: noArgs(m.reader_navigation_loading),
      navigationError: noArgs(m.reader_navigation_error),
      navigationDescription: noArgs(m.reader_navigation_description),
      tip: noArgs(m.reader_search_tip),
      pageAbbreviation: noArgs(m.reader_page_abbreviation),
      juzLabel: noArgs(m.reader_juz),
      rangeItem,
    },
    search: {
      searching: noArgs(m.reader_searching),
      noVerseMatches: (query) => m.reader_no_verse_matches({ query }, options),
      surahMatches: (count, query) => m.reader_search_surah_matches({ count, query }, options),
      textMatches: (count, query) => m.reader_search_text_matches({ count, query }, options),
      openSurah: noArgs(m.reader_search_open_surah),
      unavailable: noArgs(m.reader_search_unavailable),
      surahOpener: (name) => m.reader_search_surah_opener({ name }, options),
      ayah: (name, surah, ayah) => m.reader_search_ayah({ name, surah, ayah }, options),
    },
    range: {
      juz: noArgs(m.reader_juz),
      page: noArgs(m.reader_browse_page),
      pageAbbreviation: noArgs(m.reader_page_abbreviation),
      fullSurah: noArgs(m.reader_full_surah),
      translationUnavailable: noArgs(m.reader_range_translation_unavailable),
      item: rangeItem,
      juzCount: (count) => m.reader_juz_count({ count }, options),
    },
    sources: {
      source: noArgs(m.reader_source),
      arabic: noArgs(m.reader_arabic),
      original: noArgs(m.reader_original),
      default: noArgs(m.reader_default),
      noTranslations: noArgs(m.reader_no_translations),
    },
    verse: {
      bookmark: noArgs(m.reader_bookmark),
      removeBookmark: noArgs(m.reader_remove_bookmark),
      bookmarkVerse: noArgs(m.reader_bookmark_verse),
      copy: noArgs(m.reader_copy),
      copied: noArgs(m.reader_copied),
      copyAyah: noArgs(m.reader_copy_ayah),
      share: noArgs(m.reader_share),
      shareVerse: noArgs(m.reader_share_verse),
      noteTafsir: noArgs(m.reader_note_tafsir),
      openNoteTafsir: noArgs(m.reader_open_note_tafsir),
      closeNoteTafsir: noArgs(m.reader_close_note_tafsir),
      tafsir: noArgs(m.reader_tafsir),
      yourNote: noArgs(m.reader_your_note),
      notePlaceholder: noArgs(m.reader_note_placeholder),
      noteSaved: noArgs(m.reader_note_saved),
    },
    nav: {
      primaryLabel: noArgs(m.reader_primary_nav),
      offlineLabel: noArgs(m.reader_offline),
      offlineTitle: noArgs(m.reader_offline_title),
      offlineDetail: noArgs(m.reader_offline_cached),
      searchQuran: noArgs(m.reader_search_quran),
      account: noArgs(m.reader_account),
      signIn: noArgs(m.reader_sign_in),
      openPanel: noArgs(m.reader_open_panel),
      closePanel: noArgs(m.reader_close_panel),
      sitePanel: noArgs(m.reader_site_panel),
      appearance: noArgs(m.reader_appearance),
      toggleTheme: noArgs(m.reader_toggle_theme),
      theme: noArgs(m.reader_theme),
      language: noArgs(m.nav_language),
      changeLanguage: noArgs(m.nav_change_language),
      themeNames: {
        dark: noArgs(m.reader_dark),
        light: noArgs(m.reader_light),
      },
      sidebarToggle: noArgs(m.reader_sidebar_toggle),
      sidebarTitle: noArgs(m.reader_sidebar_title),
      homeLabel: (siteName) => m.reader_home_label({ siteName }, options),
    },
    settings: {
      settings: noArgs(m.reader_settings),
      theme: noArgs(m.reader_theme),
      closePanel: noArgs(m.reader_close_appearance),
      mode: noArgs(m.reader_mode),
      surface: noArgs(m.reader_surface),
      accent: noArgs(m.reader_accent),
      customColours: noArgs(m.reader_custom_colours),
      clear: noArgs(m.reader_clear),
      seedNames: {
        bg: noArgs(m.reader_background),
        accent: noArgs(m.reader_accent),
        pop: noArgs(m.reader_pop),
      },
      colourLabel: noArgs(m.tweaks_colour_label),
      accentOptionLabel: (name) => m.tweaks_accent_option({ name }, options),
      colourInputLabel: (name) => m.tweaks_colour_input({ name }, options),
      preset: noArgs(m.tweaks_preset),
      resetToPresetLabel: (name) => m.tweaks_reset_to_preset({ name }, options),
      toggleStatusLabel: (name, status) => m.tweaks_toggle_status({ name, status }, options),
      derivedColours: noArgs(m.reader_theme_derived_note),
      copied: noArgs(m.reader_copied),
      copyCss: noArgs(m.reader_copy_css),
      reset: noArgs(m.reader_reset),
      dataPrivacy: noArgs(m.reader_data_privacy),
      analytics: noArgs(m.reader_analytics),
      performance: noArgs(m.reader_performance),
      on: noArgs(m.reader_on),
      off: noArgs(m.reader_off),
      performanceReload: noArgs(m.reader_performance_reload),
      customizeAppearance: noArgs(m.reader_customize_appearance),
      background: noArgs(m.reader_background),
      pop: noArgs(m.reader_pop),
      themeNames: {
        dark: noArgs(m.reader_dark),
        light: noArgs(m.reader_light),
      },
      surfaces: {
        ink: { label: noArgs(m.reader_surface_ink), note: noArgs(m.reader_surface_ink_note) },
        paper: { label: noArgs(m.reader_surface_paper), note: noArgs(m.reader_surface_paper_note) },
        slate: { label: noArgs(m.reader_surface_slate), note: noArgs(m.reader_surface_slate_note) },
        mocha: { label: noArgs(m.reader_surface_mocha), note: noArgs(m.reader_surface_mocha_note) },
        contrast: {
          label: noArgs(m.reader_surface_contrast),
          note: noArgs(m.reader_surface_contrast_note),
        },
      },
      accents: {
        emerald: noArgs(m.reader_accent_teal),
        gold: noArgs(m.reader_accent_gold),
        azure: noArgs(m.reader_accent_azure),
        plum: noArgs(m.reader_accent_plum),
      },
      accentColour: (label) => m.reader_accent_colour({ label }, options),
      accentReset: (label) => m.reader_accent_reset({ label }, options),
      themeDerivedNote: noArgs(m.reader_theme_derived_note),
    },
    footer: {
      blurb: noArgs(m.footer_blurb),
      socialX: noArgs(m.footer_social_x),
      productHeading: noArgs(m.footer_product_heading),
      productLabel: noArgs(m.footer_product_label),
      companyHeading: noArgs(m.footer_company_heading),
      companyLabel: noArgs(m.footer_company_label),
      legalHeading: noArgs(m.footer_legal_heading),
      legalLabel: noArgs(m.footer_legal_label),
      builtBy: noArgs(m.footer_built_by),
      projectBy: noArgs(m.footer_project_by),
    },
    footerLinks: {
      readQuran: noArgs(m.footer_read_quran),
      bookmarks: noArgs(m.footer_bookmarks),
      whatsInside: noArgs(m.footer_whats_inside),
      about: noArgs(m.footer_about),
      faq: noArgs(m.footer_faq),
      contact: noArgs(m.footer_contact),
      privacy: noArgs(m.footer_privacy),
      terms: noArgs(m.footer_terms),
    },
    offline: {
      label: noArgs(m.reader_offline_label),
      packReady: noArgs(m.reader_offline_pack_ready),
      preparingPack: noArgs(m.reader_preparing_offline_pack),
      downloadingPack: noArgs(m.reader_downloading_offline_pack),
      stagingPack: noArgs(m.reader_staging_offline_pack),
      downloadFailed: noArgs(m.reader_offline_download_failed),
      download: noArgs(m.reader_offline_download),
      remove: noArgs(m.reader_remove_offline),
      working: noArgs(m.reader_offline_working),
      routes: (count) => m.reader_offline_routes({ count }, options),
      savedOn: (date) => m.reader_saved_on({ date }, options),
      storage: (usage, quota) => m.reader_storage({ usage, quota }, options),
      preparingQuran: noArgs(m.reader_downloading_quran),
    },
    notifications: {
      title: noArgs(m.reader_notifications),
      enable: noArgs(m.reader_enable),
      disable: noArgs(m.reader_disable),
      blocked: noArgs(m.reader_blocked),
      unsupported: noArgs(m.reader_unsupported),
      unavailable: noArgs(m.reader_notifications_unavailable),
      checking: noArgs(m.reader_notifications_checking),
      browserUnsupported: noArgs(m.reader_notifications_browser_unsupported),
      blockedDetail: noArgs(m.reader_notifications_blocked),
      on: noArgs(m.reader_notifications_on),
      offUpdates: noArgs(m.reader_notifications_off_updates),
      off: noArgs(m.reader_notifications_off),
      dismiss: noArgs(m.reader_dismiss_notification),
      open: noArgs(m.reader_open),
    },
    update: {
      ready: noArgs(m.reader_new_version_ready),
      reloadDescription: noArgs(m.reader_reload_update),
      reloadOpenTabs: noArgs(m.reader_reload_open_tabs),
      dismiss: noArgs(m.reader_dismiss_update),
    },
    seo: {
      home: noArgs(m.reader_seo_home),
      quran: noArgs(m.reader_seo_quran),
      juzIndexTitle: noArgs(m.reader_seo_juz_index_title),
      juzIndexDescription: noArgs(m.reader_seo_juz_index_description),
      juzTitle: (index, first, last) => m.reader_seo_juz_title({ index, first, last }, options),
      juzDescription: (index, first, last) =>
        m.reader_seo_juz_description({ index, first, last }, options),
      pageTitle: (index, first, last) => m.reader_seo_page_title({ index, first, last }, options),
      pageDescription: (index, first, last) =>
        m.reader_seo_page_description({ index, first, last }, options),
      translationJuzDescription: (index, first, last) =>
        m.reader_seo_translation_juz_description({ index, first, last }, options),
      translationPageDescription: (index, first, last) =>
        m.reader_seo_translation_page_description({ index, first, last }, options),
      surahTitle: (surah, name) => m.reader_seo_surah_title({ surah, name }, options),
      surahPageTitle: (surah, name, page, count) =>
        m.reader_seo_surah_page_title({ surah, name, page, count }, options),
      surahDescriptionUthmani: (name, arabic, page, count, start, end) =>
        m.reader_seo_surah_description_uthmani(
          { name, arabic, page, count, start, end },
          options,
        ),
      surahDescriptionTranslation: (name, arabic, page, count, start, end) =>
        m.reader_seo_surah_description_translation(
          { name, arabic, page, count, start, end },
          options,
        ),
      breadcrumbSurah: (name) => m.reader_seo_breadcrumb_surah({ name }, options),
      quranBook: noArgs(m.reader_quran_book),
    },
  };
}

export function getReaderUiCopy(locale: UiLocale = getLocale() as UiLocale): ReaderUiCopy {
  return createReaderUiCopy(locale);
}
