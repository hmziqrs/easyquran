import type { IconName } from "$lib/components";

export const HERO = {
  badge: "Free forever · no ads · no account needed",
  title: "The simplest way to read the Qur'an.",
  sub: "Open it, read, close it. Search, bookmarks and recitation are there when you need them — invisible when you don't.",
  primary: { label: "Start reading", href: "/app" },
  secondary: { label: "Why we built it", href: "/about" },
} as const;

export interface ValueItem {
  icon: IconName;
  title: string;
  body: string;
  short: string;
}

export const VALUES: ValueItem[] = [
  {
    icon: "arrow-right",
    title: "Instant",
    body: "Opens straight to the text — no install, no account, no setup.",
    short: "no install · no account",
  },
  {
    icon: "plus",
    title: "Your size",
    body: "Scale the Arabic script until it sits comfortably for your eyes.",
    short: "scalable script",
  },
  {
    icon: "check",
    title: "Free, no ads",
    body: "No paywall, no ad tracking, and nothing you have to sign up for.",
    short: "no paywall · no ad tracking",
  },
  {
    icon: "book",
    title: "Authentic text",
    body: "Uthmani script — the same wording as the standard printed mushaf.",
    short: "verbatim Uthmani",
  },
];

export interface RoadmapItem {
  title: string;
  body: string;
}

export const ROADMAP: RoadmapItem[] = [
  { title: "Native apps", body: "Desktop and mobile apps, so it feels at home on every device." },
  {
    title: "Sync across devices",
    body: "An optional account carries your bookmarks, notes and last page wherever you read.",
  },
  {
    title: "Recitation, for real",
    body: "Full audio recitation — for now the player is only a preview.",
  },
  {
    title: "Hadith, tafsir & translations",
    body: "More to read beside the Qur'an, added carefully and each credited.",
  },
];

export const FACTS: { value: string; label: string }[] = [
  { value: "114", label: "surahs" },
  { value: "6,236", label: "ayahs" },
  { value: "30", label: "juz" },
  { value: "0", label: "ads, ever" },
];

export const SECTIONS = {
  today: {
    eyebrow: "Today",
    title: "Small on purpose. Fast where it matters.",
    body: "Nothing between you and the text — here's what's already in the reader today.",
  },
  soon: {
    eyebrow: "On the way",
    title: "A reader now. A platform next.",
    body: "Today the reader holds the full 114 chapters. Everything below is on the way.",
  },
} as const;
