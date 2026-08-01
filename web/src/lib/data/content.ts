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
    a: "Not to read — the reader opens straight away. An optional account is coming, so your bookmarks, notes and deeds can follow you across the web and the upcoming native apps.",
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
    q: "What's coming next?",
    a: "easyquran is growing into a complete platform. On the way: hadith and their explanations, an audio player, a deeds feature and translations in selected languages — followed by native desktop and mobile apps.",
  },
  {
    q: "Will you add translations and tafsir?",
    a: "Yes. Translations in a selected set of languages and short tafsir summaries are on the way, added surah by surah and each credited to its source.",
  },
  {
    q: "How do I back up my bookmarks?",
    a: "Today they live in your browser. Cloud sync through an optional account is on the way, with export-to-file as a fallback so you're never locked in.",
  },
];

export interface LegalSection {
  h: string;
  p: string;
}

export const PRIVACY_SECTIONS: LegalSection[] = [
  {
    h: "1. What we collect",
    p: "To keep easyquran running and to improve it, we collect limited data through Firebase: anonymous usage analytics (which features get used), crash reports, and performance metrics. We also assign your device an identifier so we can deliver push notifications to it. This is never used to build an advertising profile or sold to anyone.",
  },
  {
    h: "2. Accounts and sync",
    p: "Reading needs no account. If you choose to create one, your bookmarks, notes and deeds are linked to it so they sync across your devices and the native apps. You can delete your account and its data at any time.",
  },
  {
    h: "3. What stays on your device",
    p: "Until you sign in, bookmarks, notes and your reading position are stored in your browser's local storage. They leave your device only if you turn on sync.",
  },
  {
    h: "4. Hosting and delivery",
    p: "Our host and content delivery network record short-lived, standard logs (approximate region, browser type, requested file) to keep the service running and to detect abuse. Requesting recitation audio includes your IP address, as any web request does.",
  },
  {
    h: "5. Push notifications",
    p: "If you enable notifications, we store a device identifier so messages reach the right device. You can turn notifications off at any time in your device or browser settings.",
  },
  {
    h: "6. Children",
    p: "The app is suitable for all ages. We do not knowingly collect personal information from children beyond what is described here.",
  },
  {
    h: "7. Your choices",
    p: "You can use easyquran without an account, decline notifications, and — if you have one — delete your account and its synced data. Clearing your browser data removes anything stored locally.",
  },
  {
    h: "8. Changes to this policy",
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
    p: "easyquran provides a Qur'an reading interface, search, bookmarks, personal notes and optional recitation audio, free of charge and without warranty of uninterrupted availability. Hadith, explanations, translations and further features are added over time as the project grows.",
  },
  {
    h: "3. Accuracy of text",
    p: "We take great care with the Arabic text, translations and tafsir, but errors are possible. For matters of ruling or recitation, please rely on a printed mushaf and qualified scholarship.",
  },
  {
    h: "4. Your content",
    p: "Notes, bookmarks and deeds you create remain yours. If you use sync, you give us permission to store and transfer them solely to provide that feature. We claim no licence over your content beyond running the service.",
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

export interface Stat {
  value: string;
  label: string;
}
export const ABOUT_STATS: Stat[] = [
  { value: "Free", label: "Now, and as it grows" },
  { value: "Uthmani", label: "Script, carefully sourced" },
  { value: "Growing", label: "Hadith, audio, deeds & apps next" },
];

export const LEGAL_UPDATED = "30 July 2026";
