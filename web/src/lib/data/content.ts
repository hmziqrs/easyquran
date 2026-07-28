/* ════════════════════════════════════════════════════════════════════════
   content.ts — site copy that isn't the Quran text: the FAQ accordion, the
   privacy & terms section bodies, the contact-form topic chips, and the stat
   tiles on the about page. Kept in code (not markdown) so the components can
   render it with full styling control.
   ════════════════════════════════════════════════════════════════════════ */

export interface Faq {
  q: string;
  a: string;
}

export const FAQS: Faq[] = [
  {
    q: "Is easyquran free?",
    a: "Yes, and it stays free. No ads, no paid tier, no upsell inside the reader. If we ever need help with hosting we'll ask openly rather than sell your attention.",
  },
  {
    q: "Do I need an account?",
    a: "No. Bookmarks, notes and your last read position are saved in your browser. Nothing is sent to a server, so there is nothing to log into.",
  },
  {
    q: "Which Arabic text do you use?",
    a: "The Uthmani script, matching the standard printed mushaf. If you spot a discrepancy, email corrections@easyquran.app — we treat script accuracy as the highest priority in the project.",
  },
  {
    q: "Does it work offline?",
    a: "Once the app has loaded, the surahs you've opened stay available offline. Recitation audio needs a connection the first time you play it.",
  },
  {
    q: "Will you add translations and tafsir?",
    a: "Translations and short tafsir summaries are being added surah by surah, each credited to its source. This demo shows sample text in those slots.",
  },
  {
    q: "How do I back up my bookmarks?",
    a: "Because everything lives on your device, clearing browser data clears your bookmarks too. An export-to-file option is on the way.",
  },
];

export interface LegalSection {
  h: string;
  p: string;
}

export const PRIVACY_SECTIONS: LegalSection[] = [
  {
    h: "1. What we collect",
    p: "Nothing that identifies you. easyquran has no accounts, no sign-in, no advertising SDKs and no third-party analytics. We do not build a profile of your reading.",
  },
  {
    h: "2. What stays on your device",
    p: "Bookmarks, personal notes, reading position, chosen script size and light or dark preference are written to your browser's local storage. They never leave your device unless you copy them yourself.",
  },
  {
    h: "3. Hosting logs",
    p: "Our host records standard, short-lived server logs (approximate region, browser type, requested file) to keep the service running and to detect abuse. These are not linked to any individual and are discarded on a rolling basis.",
  },
  {
    h: "4. Recitation audio",
    p: "When you play recitation, audio files are requested from our content delivery network. That request includes your IP address, as any web request does. The CDN does not receive your bookmarks, notes or reading history.",
  },
  {
    h: "5. Children",
    p: "The app is suitable for all ages and, because it collects no personal information, is safe for children to use without supervision or consent flows.",
  },
  {
    h: "6. Your choices",
    p: "Clearing your browser data removes everything easyquran has stored. There is no server-side copy for us to delete on your behalf, and no export we can be compelled to hand over.",
  },
  {
    h: "7. Changes to this policy",
    p: "If this policy changes we will update the date at the top and note what changed. Substantive changes will be announced in the app before they take effect.",
  },
];

export const TERMS_SECTIONS: LegalSection[] = [
  {
    h: "1. Acceptance",
    p: "By using easyquran you agree to these terms. If you do not agree with them, please do not use the app. They apply to the website and the reader alike.",
  },
  {
    h: "2. The service",
    p: "easyquran provides a Qur'an reading interface, search, bookmarks, personal notes and optional recitation audio, free of charge and without warranty of uninterrupted availability.",
  },
  {
    h: "3. Accuracy of text",
    p: "We take great care with the Arabic text, translations and tafsir, but errors are possible. For matters of ruling or recitation, please rely on a printed mushaf and qualified scholarship.",
  },
  {
    h: "4. Your content",
    p: "Notes you write remain yours and stay on your device. We claim no licence over them and, because we never receive them, we cannot moderate, restore or recover them.",
  },
  {
    h: "5. Acceptable use",
    p: "Please do not attempt to disrupt the service, scrape it at abusive volume, or redistribute the app while presenting it as your own product. Quoting and sharing verses is, of course, encouraged.",
  },
  {
    h: "6. Third-party content",
    p: "Recitations, translations and tafsir are used with permission from their respective holders and remain their property. Credits are listed beside each source in the app.",
  },
  {
    h: "7. Liability",
    p: "The app is provided 'as is'. To the extent permitted by law, we are not liable for any loss arising from its use, including loss of locally stored notes or bookmarks.",
  },
  {
    h: "8. Contact",
    p: "Questions about these terms can be sent to salam@easyquran.app and we will respond within a few days.",
  },
];

/** Selectable chips on the contact form. */
export const CONTACT_TOPICS: string[] = [
  "Bug report",
  "Text correction",
  "Feature idea",
  "Something else",
];

/** The three stat tiles on the about page. */
export interface Stat {
  value: string;
  label: string;
}
export const ABOUT_STATS: Stat[] = [
  { value: "0", label: "Ads, ever" },
  { value: "0", label: "Trackers or analytics SDKs" },
  { value: "100%", label: "Stored on your device" },
];

export const LEGAL_UPDATED = "12 July 2026";
