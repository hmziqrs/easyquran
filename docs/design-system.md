# EasyQuran Design System

> **Audited, themeable design system for a modern Quran reading and study product**  
> Version: 1.0 · August 2026

---

## 0. Executive Summary

This system synthesizes the strongest patterns from five Quran/Islamic products without copying any one of them directly:

- [Ayah](https://www.getayah.com/#quran) — bilingual editorial language, generous whitespace, learning/progress cues, restrained presentation.
- [Quran.com](https://quran.com/) — mature Quran-reader information architecture, reading modes, study tools, personalization, search, translations, word-by-word, audio, Light/Dark/Sepia support.
- [Niyat](https://niyatapp.com/) — warm ivory surfaces, forest-green brand language, calm cards, soft dividers, high-quality mobile Quran presentation.
- [Quran Companion](https://quran-companion.co/prophets) — chronology, prophets, relationships, timelines, filters, connected-study patterns.
- [Quran Gate](https://qurangate.app/) — habits, streaks, reading progress, sequential continuation, ritual/progress mechanics.

The resulting direction is called **Sacred Editorial**:

> **A calm reading room, not a SaaS dashboard.**

The Quran remains the focal point. UI chrome should recede. Arabic typography, whitespace, hierarchy, and reading comfort should do more visual work than decoration.

### Core visual formula

| Layer        | Direction                                                                    |
| ------------ | ---------------------------------------------------------------------------- |
| Brand        | Deep forest green + warm ivory + muted antique gold                          |
| Typography   | Newsreader for editorial moments, Inter for UI, dedicated Arabic/Quran fonts |
| Surfaces     | Warm, low-contrast, subtly layered                                           |
| Geometry     | Soft but not bubbly; mostly 10–18px radii                                    |
| Icons        | Lucide-style thin line icons                                                 |
| Quran reader | Border-light, spacious, book-like                                            |
| Dark mode    | Warm charcoal rather than generic blue-black                                 |
| Motion       | Quiet, brief, functional                                                     |
| Decoration   | Rare and subtle                                                              |
| Study tools  | Functional density only where needed                                         |

The default theme is **Sacred Editorial**, but the component architecture is deliberately palette-agnostic. Four full palette families are defined later:

1. **Sacred Editorial** — forest / ivory / antique gold
2. **Ink** — strict black / white / graphite
3. **Mushaf Sepia** — parchment / umber / bronze
4. **Midnight Sapphire** — slate / deep navy / cool blue / brass

Every palette supports both light and dark mode.

---

# 1. Audit of the Original Proposal

The first design direction was strong, but a deep consistency audit revealed several things that should be corrected before implementation.

## 1.1 What was already strong

### A. The reference weighting was directionally correct

The best division of responsibilities remains:

- **Niyat + Ayah** → visual language
- **Quran.com** → reader and study UX architecture
- **Quran Companion** → connected study, chronology, relationships
- **Quran Gate** → progress and habit mechanics

Trying to visually blend all five equally would create a confused product. The system should have one visual voice while borrowing behavior patterns from the others.

### B. The warm background + forest-primary direction fits long-form Quran reading

A slightly warm neutral is less sterile than pure white and makes the Quran surface feel more intentional. It also prevents the design from looking like a generic productivity product.

### C. Separating Quran typography from UI Arabic typography is essential

The Quran text should never inherit a generic Arabic UI font. Quran rendering is a specialized layer with its own font, line-height, glyph handling, Tajweed behavior, and layout requirements.

### D. The proposal correctly avoided cardifying every ayah

A Quran reader should not look like a feed of 114 SaaS cards. Verse boundaries can be communicated using whitespace, subtle dividers, hover states, and verse markers.

### E. Gold was correctly treated as a secondary sacred/accent color

Gold should not become the normal CTA color. Its strongest use is for sacred metadata, small ceremonial accents, Makki labels, ornament, and carefully selected featured content.

---

# 2. Audit Corrections

## 2.1 Accessibility correction: muted text

The old light-mode muted color was:

```css
#7B837E
```

against:

```css
#F8F7F2
```

Its contrast is roughly **3.63:1**, which is too low for ordinary small text.

The corrected muted token is:

```css
--muted: #68716B;
```

Contrast against the default background is roughly **4.70:1**, making it suitable for normal text at standard sizes.

### Rule

Never intentionally make metadata unreadable to make the interface look “soft.” Visual hierarchy should come from weight, size, spacing, and chroma—not insufficient contrast.

---

## 2.2 Accessibility correction: gold

The original light gold:

```css
#B6914C
```

is attractive as decoration, but it only reaches roughly **2.74:1** against the warm page background.

Therefore gold is split into two roles:

```css
--accent: #B6914C;        /* decorative / large graphical use */
--accent-strong: #715625; /* text, icons, labels */
--accent-soft: #F3EAD8;
```

`#715625` on `#F3EAD8` is roughly **5.74:1**.

### Rule

Never use decorative gold as body copy or small metadata text.

---

## 2.3 Accessibility correction: dark primary buttons

The old proposal implicitly kept white button text in both modes.

That fails when dark mode uses a light mint primary such as:

```css
#77BDA4
```

White on that green is only roughly **2.19:1**.

Dark mode therefore gets an explicit semantic foreground:

```css
--primary: #77BDA4;
--primary-foreground: #0D1210;
```

This reaches roughly **8.64:1**.

### Rule

Components must never assume `color: white` on `background: primary`.

They must use:

```css
color: var(--primary-foreground);
```

---

## 2.4 The original sidebar was slightly too “dashboard-like”

A permanent 240–264px sidebar is reasonable for study/explore views, but not ideal as the default reading posture.

### Revised shell behavior

- Home / Explore / Collections: expanded sidebar is allowed.
- Quran index: compact rail or expanded sidebar.
- Quran reading: sidebar collapses automatically.
- Focus mode: all nonessential chrome disappears.
- Mobile: use bottom navigation + contextual top bar.

The application shell adapts to the task rather than forcing every page into one desktop admin layout.

---

## 2.5 The typography system needed stricter role boundaries

Four font families can be justified here, but only if their roles never overlap randomly.

### Correct role model

| Role                       | Font                              |
| -------------------------- | --------------------------------- |
| English UI                 | Inter                             |
| Editorial/display headings | Newsreader                        |
| Arabic UI labels           | Noto Sans Arabic                  |
| Quran text                 | QPC Hafs / appropriate Quran font |

### Never do this

- Newsreader in settings menus
- Quran font in navigation
- Noto Sans Arabic for rendered Quran text
- Random serif subtitles throughout the application

Newsreader should feel special because it is **not everywhere**.

---

## 2.6 Bilingual Arabic eyebrows were at risk of becoming a gimmick

This pattern is beautiful:

```text
الْقُرْآن
THE QURAN
```

But repeating it on every card would become ornamental noise.

### Revised rule

Use bilingual editorial labels on:

- major landing-page sections
- page introductions
- Prophet/topic editorial headers
- key empty/onboarding states

Avoid them on:

- buttons
- every Surah row
- preference forms
- dense tables
- every dashboard card

A good maximum is roughly **one bilingual eyebrow per major viewport section**.

---

## 2.7 Radius hierarchy was too broad

The previous system exposed too many radii. That often leads to arbitrary component styling.

### Revised radius set

```css
--radius-sm: 8px;
--radius-md: 12px;
--radius-lg: 18px;
--radius-xl: 24px;
--radius-pill: 999px;
```

Usage:

- inputs / buttons → 10–12px
- normal cards → 12–18px
- hero/featured cards → 18–24px
- chips → pill

Do not make ordinary content cards 28px rounded unless they are intentionally large feature surfaces.

---

## 2.8 Progress mechanics needed quieter hierarchy

Quran Gate-inspired progress is useful, but EasyQuran should not feel like a fitness tracker.

### Revised rule

The home screen may prominently show **one** progress surface at a time:

- continue reading, or
- daily goal, or
- active plan

Secondary metrics such as streak, reading time, and ayahs read should be visually subordinate.

The spiritual content should outrank the metric.

---

# 3. Design Principles

## Principle 1 — Quran first

If the Quran is visible, it should be the strongest visual element on the page.

The visual hierarchy should generally be:

1. Quran Arabic
2. Current reading context / Surah
3. Translation
4. Study material
5. Controls
6. Metadata

---

## Principle 2 — Calm is created by subtraction

Do not simulate calm using only beige backgrounds and rounded corners.

Calm comes from:

- fewer simultaneous controls
- strong content hierarchy
- generous whitespace
- minimal border noise
- restrained motion
- consistent alignment
- predictable navigation

---

## Principle 3 — Study density and reading serenity are different modes

Reading and studying are related but not identical tasks.

### Reading mode

- narrow content column
- minimal controls
- generous vertical spacing
- hidden advanced actions
- persistent audio only if active

### Study mode

- tabs / side panel allowed
- related Hadith / Tafsir / notes / word data
- compare mode allowed
- denser navigation
- additional metadata

Do not compromise reading serenity just so all study tools remain visible at all times.

---

## Principle 4 — Semantic tokens, never component colors

A button should not know that the brand is green.

It should know:

```css
background: var(--primary);
color: var(--primary-foreground);
```

This is what makes alternate themes possible.

---

## Principle 5 — Sacred does not mean ornamental

Islamic identity should come primarily from:

- Quran typography
- Arabic language
- editorial composition
- intentional color
- content itself

not from repeated mosque silhouettes, crescents, stars, arches, gradients, gold borders, or arabesque textures.

---

# 4. Theme Architecture

Treat **palette** and **mode** as separate concepts.

```html
<html data-palette="sacred" data-mode="light">
```

Examples:

```html
<html data-palette="ink" data-mode="dark">
<html data-palette="sepia" data-mode="light">
<html data-palette="sapphire" data-mode="dark">
```

This makes it possible to switch palette without rewriting components.

## Semantic token contract

Every theme must provide:

```css
--background;
--background-subtle;
--surface;
--surface-raised;
--surface-hover;

--foreground;
--foreground-secondary;
--muted;

--border;
--border-strong;

--primary;
--primary-hover;
--primary-foreground;
--primary-soft;

--accent;
--accent-strong;
--accent-soft;

--success;
--warning;
--danger;

--focus-ring;

--reader-background;
--quran-foreground;
--translation-foreground;
--reader-divider;
```

No component should use a raw hex color except specialized Quran/Tajweed rendering.

---

# 5. Theme 01 — Sacred Editorial

**Recommended default.**

Character:

> warm · scholarly · premium · gentle · modern

## Light

```css
[data-palette="sacred"][data-mode="light"] {
  --background: #F8F7F2;
  --background-subtle: #F2F0E9;

  --surface: #FFFEFA;
  --surface-raised: #FFFFFF;
  --surface-hover: #F1F0E9;

  --foreground: #18211D;
  --foreground-secondary: #505A54;
  --muted: #68716B;

  --border: #DDDCD3;
  --border-strong: #C9C7BD;

  --primary: #145A43;
  --primary-hover: #104B38;
  --primary-foreground: #FFFFFF;
  --primary-soft: #E6F0EB;

  --accent: #B6914C;
  --accent-strong: #715625;
  --accent-soft: #F3EAD8;

  --success: #397A57;
  --warning: #8A681E;
  --danger: #B34A4A;

  --focus-ring: #2D7B61;

  --reader-background: #FFFEFA;
  --quran-foreground: #17201C;
  --translation-foreground: #4D5751;
  --reader-divider: #E5E2D9;
}
```

## Dark

```css
[data-palette="sacred"][data-mode="dark"] {
  --background: #0D1210;
  --background-subtle: #111814;

  --surface: #151D19;
  --surface-raised: #1A231F;
  --surface-hover: #202A25;

  --foreground: #F0EFE9;
  --foreground-secondary: #BCC2BD;
  --muted: #89938D;

  --border: #29332E;
  --border-strong: #39443E;

  --primary: #77BDA4;
  --primary-hover: #91CEB9;
  --primary-foreground: #0D1210;
  --primary-soft: #17372C;

  --accent: #D0B06B;
  --accent-strong: #D0B06B;
  --accent-soft: #332A19;

  --success: #78BD91;
  --warning: #D9B55E;
  --danger: #DF7777;

  --focus-ring: #91CEB9;

  --reader-background: #111713;
  --quran-foreground: #F4F2EA;
  --translation-foreground: #C2C6C1;
  --reader-divider: #28332D;
}
```

### Use when

- You want the strongest Islamic/editorial identity.
- You want the site to feel warmer than Quran.com without becoming ornamental.
- The product is reading-first.

---

# 6. Theme 02 — Ink

A highly neutral black/white option for testing whether the design works **without relying on brand color**.

Character:

> editorial · timeless · stark · minimal · typographic

This is also the best palette for auditing hierarchy. If the interface looks good in Ink, the layout is doing real work rather than depending on decorative color.

## Light

```css
[data-palette="ink"][data-mode="light"] {
  --background: #F7F7F5;
  --background-subtle: #EFEFED;

  --surface: #FFFFFF;
  --surface-raised: #FFFFFF;
  --surface-hover: #F0F0EE;

  --foreground: #121212;
  --foreground-secondary: #4B4B4B;
  --muted: #686868;

  --border: #DADADA;
  --border-strong: #BDBDBD;

  --primary: #111111;
  --primary-hover: #2A2A2A;
  --primary-foreground: #FFFFFF;
  --primary-soft: #ECECEC;

  --accent: #777777;
  --accent-strong: #4B4B4B;
  --accent-soft: #F1F1F1;

  --success: #356B4B;
  --warning: #775F25;
  --danger: #A33F3F;

  --focus-ring: #111111;

  --reader-background: #FFFFFF;
  --quran-foreground: #101010;
  --translation-foreground: #444444;
  --reader-divider: #E6E6E6;
}
```

## Dark / OLED

```css
[data-palette="ink"][data-mode="dark"] {
  --background: #000000;
  --background-subtle: #080808;

  --surface: #0D0D0D;
  --surface-raised: #151515;
  --surface-hover: #1A1A1A;

  --foreground: #F5F5F5;
  --foreground-secondary: #C8C8C8;
  --muted: #9A9A9A;

  --border: #292929;
  --border-strong: #3D3D3D;

  --primary: #F2F2F2;
  --primary-hover: #FFFFFF;
  --primary-foreground: #090909;
  --primary-soft: #1D1D1D;

  --accent: #C8C8C8;
  --accent-strong: #C8C8C8;
  --accent-soft: #1A1A1A;

  --success: #79B991;
  --warning: #D8BA70;
  --danger: #E07B7B;

  --focus-ring: #FFFFFF;

  --reader-background: #050505;
  --quran-foreground: #F5F5F5;
  --translation-foreground: #C6C6C6;
  --reader-divider: #242424;
}
```

### Use when

- You want maximum typographic purity.
- You are testing whether the layout itself is beautiful.
- You want an OLED-friendly pure-black mode.
- You want a restrained alternative to “Islamic green.”

### Important

Do not add decorative gray gradients. The appeal of this palette comes from type, rhythm, and proportion.

---

# 7. Theme 03 — Mushaf Sepia

Inspired by printed pages and long-form reading rather than a “vintage website.”

Character:

> paper · contemplative · traditional · warm · low-fatigue

Quran.com currently exposes Light, Dark, and Sepia reading themes, and Quran Foundation’s Tajweed font guidance also explicitly accounts for Light/Dark/Sepia theme palettes. This makes Sepia especially practical for a Quran reader.

## Light

```css
[data-palette="sepia"][data-mode="light"] {
  --background: #F4ECD8;
  --background-subtle: #EEE2C8;

  --surface: #FFF9E9;
  --surface-raised: #FFFCF3;
  --surface-hover: #EFE4CF;

  --foreground: #2E271C;
  --foreground-secondary: #5F5342;
  --muted: #70624F;

  --border: #D9C9AA;
  --border-strong: #C2AD87;

  --primary: #6E4F27;
  --primary-hover: #5B401F;
  --primary-foreground: #FFF9E9;
  --primary-soft: #EADCBF;

  --accent: #A47B36;
  --accent-strong: #684B1F;
  --accent-soft: #EEE0C2;

  --success: #52704A;
  --warning: #806021;
  --danger: #9D493D;

  --focus-ring: #6E4F27;

  --reader-background: #FBF3DF;
  --quran-foreground: #2B2419;
  --translation-foreground: #5E5140;
  --reader-divider: #DDCFB4;
}
```

## Dark

```css
[data-palette="sepia"][data-mode="dark"] {
  --background: #18130E;
  --background-subtle: #1C160F;

  --surface: #211A13;
  --surface-raised: #292017;
  --surface-hover: #30251A;

  --foreground: #F0E5CF;
  --foreground-secondary: #C9B99A;
  --muted: #9D8C6F;

  --border: #403324;
  --border-strong: #574632;

  --primary: #D1AE71;
  --primary-hover: #E0BF82;
  --primary-foreground: #1B1309;
  --primary-soft: #3A2C1B;

  --accent: #E0C48B;
  --accent-strong: #E0C48B;
  --accent-soft: #332719;

  --success: #90B47D;
  --warning: #E0BC6F;
  --danger: #DB8274;

  --focus-ring: #D1AE71;

  --reader-background: #1C160F;
  --quran-foreground: #F0E5CF;
  --translation-foreground: #CBBEA4;
  --reader-divider: #3B3023;
}
```

### Use when

- Reading comfort is the dominant priority.
- You want a visual relationship to physical Quran pages without fake textures.
- You want an alternative to both green and monochrome.

---

# 8. Theme 04 — Midnight Sapphire

A cooler, more contemporary theme that still avoids generic “developer dark mode.”

Character:

> scholarly · contemporary · cool · refined · nocturnal

## Light

```css
[data-palette="sapphire"][data-mode="light"] {
  --background: #F7F8FA;
  --background-subtle: #EEF1F5;

  --surface: #FFFFFF;
  --surface-raised: #FFFFFF;
  --surface-hover: #EEF2F7;

  --foreground: #171C24;
  --foreground-secondary: #4E5968;
  --muted: #667384;

  --border: #D9DFE7;
  --border-strong: #BEC7D2;

  --primary: #244E8A;
  --primary-hover: #1D4277;
  --primary-foreground: #FFFFFF;
  --primary-soft: #E8EEF8;

  --accent: #A28759;
  --accent-strong: #6F5A37;
  --accent-soft: #F1EBDD;

  --success: #39705A;
  --warning: #80611F;
  --danger: #AE4747;

  --focus-ring: #3567A9;

  --reader-background: #FFFFFF;
  --quran-foreground: #18202A;
  --translation-foreground: #4A5666;
  --reader-divider: #E3E7ED;
}
```

## Dark

```css
[data-palette="sapphire"][data-mode="dark"] {
  --background: #0B1018;
  --background-subtle: #0E1520;

  --surface: #111824;
  --surface-raised: #172131;
  --surface-hover: #1B2738;

  --foreground: #EFF3F8;
  --foreground-secondary: #B9C3D0;
  --muted: #8895A5;

  --border: #273446;
  --border-strong: #394A60;

  --primary: #8FB5EA;
  --primary-hover: #A6C7F2;
  --primary-foreground: #0A111B;
  --primary-soft: #1A2A3F;

  --accent: #D2B47C;
  --accent-strong: #D2B47C;
  --accent-soft: #302719;

  --success: #80BFA0;
  --warning: #D7B66E;
  --danger: #E07A7A;

  --focus-ring: #A6C7F2;

  --reader-background: #0E151F;
  --quran-foreground: #F1F4F8;
  --translation-foreground: #C0CAD6;
  --reader-divider: #263447;
}
```

### Use when

- You want a cooler visual identity.
- You want to distinguish the product from the common cream/green Quran aesthetic.
- Dark mode is a major use case.

---

# 9. Theme Contrast Audit

Approximate WCAG contrast checks were run against each palette background.

| Theme          | Primary text | Secondary | Muted | Primary | Primary button | Accent chip |
| -------------- | -----------: | --------: | ----: | ------: | -------------: | ----------: |
| Sacred Light   |        15.37 |      6.68 |  4.70 |    7.61 |           8.16 |        5.74 |
| Sacred Dark    |        16.41 |     10.43 |  5.96 |    8.64 |           8.64 |        6.80 |
| Ink Light      |        17.46 |      8.13 |  5.19 |   17.60 |          18.88 |        7.72 |
| Ink Dark       |        19.26 |     12.55 |  7.46 |   18.76 |          17.79 |       10.40 |
| Sepia Light    |        12.53 |      6.36 |  5.03 |    6.35 |           7.11 |        6.15 |
| Sepia Dark     |        14.77 |      9.57 |  5.63 |    8.79 |           8.75 |        8.61 |
| Sapphire Light |        16.09 |      6.69 |  4.54 |    7.81 |           8.29 |        5.53 |
| Sapphire Dark  |        17.11 |     10.69 |  6.25 |    9.05 |           8.99 |        7.38 |

All normal text tokens are designed to meet or exceed the normal-text 4.5:1 target against the base background.

Do not assume every arbitrary token pairing is accessible. Component pairings still need to be tested in Storybook/Playwright/axe.

---

# 10. Typography

## English UI

```css
--font-sans: "Inter", system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
```

Use for:

- navigation
- buttons
- controls
- translations
- forms
- settings
- metadata
- dense study interfaces

---

## Editorial display

```css
--font-display: "Newsreader", Georgia, serif;
```

Use for:

- home hero
- section headings
- Surah introductions
- Prophet profiles
- topic pages
- Daily Ayah editorial cards
- major empty states

Do not use Newsreader for everyday UI controls.

---

## Arabic UI

```css
--font-arabic-ui: "Noto Sans Arabic", sans-serif;
```

Use for Arabic interface labels and non-Quran Arabic prose.

---

## Quran text

Use a dedicated Quran font stack, preferably Quran Foundation/QPC resources appropriate to the selected script and rendering mode.

```css
--font-quran: "UthmanicHafs", serif;
```

Possible reader script modes include:

- Uthmani / QPC Hafs
- IndoPak
- Tajweed-capable font rendering
- physical Mushaf/page-layout mode where applicable

Quran Foundation documentation notes that larger font scales eventually need relaxed line wrapping instead of strict physical Mushaf line fidelity. Treat accessibility as more important than reproducing line boundaries when the user deliberately selects large type.

---

# 11. Type Scale

```text
Display XL    64 / 68    Newsreader 500
Display L     52 / 58    Newsreader 500
H1            40 / 46    Newsreader 500
H2            32 / 39    Newsreader 500
H3            24 / 31    Newsreader 550

Body XL       20 / 32    Inter 400
Body L        18 / 29    Inter 400
Body          16 / 26    Inter 400
Body S        14 / 22    Inter 400
Caption       12 / 18    Inter 500
Micro         11 / 16    Inter 600
```

## Quran reader scale

Suggested default:

```text
Desktop       36px / 2.20
Tablet        34px / 2.20
Mobile        30px / 2.15
```

User options should allow significantly larger values.

### Reader width

Translation prose:

```text
~60–75 characters per line
```

Quran Arabic should be judged separately because script shape and word spacing make Latin line-length heuristics inappropriate.

---

# 12. Spacing

Use an 8px base rhythm with a 4px micro-step.

```css
--space-1: 4px;
--space-2: 8px;
--space-3: 12px;
--space-4: 16px;
--space-5: 20px;
--space-6: 24px;
--space-8: 32px;
--space-10: 40px;
--space-12: 48px;
--space-16: 64px;
--space-20: 80px;
--space-24: 96px;
--space-32: 128px;
```

### Typical usage

| Context                   |  Spacing |
| ------------------------- | -------: |
| icon ↔ label              |      8px |
| form control group        |  12–16px |
| card padding              |  20–24px |
| reader ayah vertical gap  |  28–40px |
| page section gap          |  64–96px |
| editorial landing section | 80–128px |

---

# 13. Radius and Shape

```css
--radius-sm: 8px;
--radius-md: 12px;
--radius-lg: 18px;
--radius-xl: 24px;
--radius-pill: 999px;
```

### Shape rules

- buttons → 10–12px
- text inputs → 10–12px
- normal cards → 12–18px
- feature cards → 18–24px
- chips → pill
- Quran ayahs → usually **no card radius because they are not cards**

---

# 14. Borders and Elevation

Prefer borders and tonal surfaces over shadows.

## Light elevation

```css
--shadow-sm:
  0 1px 2px rgb(20 30 25 / 0.03),
  0 6px 18px rgb(20 30 25 / 0.03);

--shadow-md:
  0 2px 4px rgb(20 30 25 / 0.04),
  0 12px 32px rgb(20 30 25 / 0.06);
```

## Dark elevation

Use almost no drop shadow.

Elevation should primarily come from:

1. surface luminance
2. border contrast
3. backdrop separation

---

# 15. Application Shell

## Desktop

```text
┌──────────────────────────────────────────────────────────────┐
│ Brand        Search Quran…                 Theme  Lang  User │
├───────────┬──────────────────────────────────────────────────┤
│           │                                                  │
│ Context   │                    CONTENT                       │
│ nav       │                                                  │
│           │                                                  │
└───────────┴──────────────────────────────────────────────────┘
```

### Dimensions

```text
Header:           64px
Expanded sidebar: 240–256px
Collapsed rail:   64–72px
Main max width:   1280px
Reader max width: 760–860px depending on mode
```

### Adaptive behavior

| Page             | Navigation behavior       |
| ---------------- | ------------------------- |
| Home             | expanded sidebar optional |
| Quran index      | expanded or compact rail  |
| Explore          | expanded sidebar          |
| Prophet/timeline | expanded sidebar          |
| Reading          | compact/collapsed         |
| Focus reading    | hidden                    |

---

# 16. Navigation

Primary navigation:

```text
Read
Explore
Topics
Prophets
Collections
```

Secondary:

```text
Bookmarks
History
Settings
```

Active item:

```css
background: var(--primary-soft);
color: var(--primary);
```

Never communicate active state using color alone. Use at least one of:

- icon emphasis
- inset marker
- text weight
- selected background

---

# 17. Search / Command Palette

Search is a first-class Quran navigation primitive.

```text
⌕ Search surahs, ayahs, topics, prophets…                 ⌘ K
```

Suggested control:

```text
height: 44–48px
radius: 12px
```

Results are grouped semantically:

```text
SURAH
Al-Baqarah
البقرة

AYAH
2:255 — Ayatul Kursi

TOPIC
Patience

PROPHET
Musa عليه السلام
```

The search UI may become a full command palette on desktop and full-screen search on mobile.

---

# 18. Quran Index

Niyat’s mobile Quran list demonstrates the value of a restrained list with:

- number marker
- English name
- Arabic name
- ayah count
- Makki/Madani label
- search and Surah/Juz/Bookmark switching

For desktop, keep that scanning efficiency rather than converting all 114 Surahs into oversized cards.

```text
╭────────────────────────────────────────────────────────────╮
│ 02    Al-Baqarah                         البقرة            │
│       The Cow · 286 ayahs                 Madani           │
╰────────────────────────────────────────────────────────────╯
```

### Default row

- no heavy shadow
- subtle divider
- 64–76px minimum row height

### Hover

```css
background: var(--surface-hover);
```

### Selected

```css
background: var(--primary-soft);
```

plus a small logical-inline selection marker.

---

# 19. Makki / Madani Badges

Do not encode meaning using color alone.

Use text + optional icon.

### Makki

```css
background: var(--accent-soft);
color: var(--accent-strong);
```

### Madani

```css
background: var(--primary-soft);
color: var(--primary);
```

Dimensions:

```text
height: 24px
padding-inline: 8px
font: 11/16 medium
radius: pill
```

---

# 20. Quran Reader

The reader is the core product surface.

```text
← Quran                       Al-Baqarah                    ⚙

                        سُورَةُ الْبَقَرَةِ
                           Al-Baqarah
                            The Cow

                               ﷽

──────────────────────────────────────────────────────────────

2:1                                                     ⋯  ♡  ▶

                           الٓمٓ

Alif, Lam, Meem.

Sahih International

──────────────────────────────────────────────────────────────
```

### Hierarchy

1. Quran Arabic
2. translation
3. source/translation metadata
4. actions

### Critical rule

Do not show every available action all the time.

---

# 21. Ayah Interaction

Desktop default actions:

```text
▶ Listen
♡ Save
▣ Note
⋯ More
```

Additional actions:

```text
Tafsir
Word by word
Transliteration
Repeat
Share
Copy Arabic
Copy translation
Add to collection
Compare
Open Study Mode
```

### Default

```css
background: transparent;
```

### Hover

```css
background: var(--surface-hover);
```

### Playing

```css
background: var(--primary-soft);
border-inline-start: 3px solid var(--primary);
```

### Bookmarked

Use icon + accessible label. A gold/accent icon is acceptable, but the state must not depend exclusively on color.

---

# 22. Bismillah

Treat the Bismillah ceremonially but minimally.

```text
                              ﷽
```

Suggested:

```text
font-size: 42–48px
margin-block: 40–52px
```

Avoid:

- giant mosque illustrations
- patterned backgrounds
- luminous gradients
- thick gold frames

---

# 23. Word-by-Word Mode

```text
        ٱلْحَمْدُ
         praise
```

### Interaction

```css
border-radius: 6px;
```

Hover/focus:

```css
background: var(--primary-soft);
```

Currently playing word:

```css
background: var(--accent-soft);
color: var(--accent-strong);
```

### Accessibility

- word targets should remain at least ~44px high on touch screens where practical
- provide keyboard focus
- audio state must be represented beyond color
- Arabic word and gloss need proper language/`dir` attributes

---

# 24. Reader Modes

Quran.com’s current direction validates separating multiple ways of reading rather than forcing one layout to do everything.

Recommended EasyQuran modes:

### Verse mode

Arabic + per-ayah translation + actions.

### Translation book mode

Continuous translation reading with verse references integrated into prose.

### Mushaf/page mode

Physical-page-oriented layout where font/rendering supports it.

### Word-by-word mode

Interactive word study.

### Study mode

Focused ayah with Tafsir, Hadith, reflections, related verses, word details, notes, and compare tools.

---

# 25. Reader Settings

Use a drawer on desktop and bottom sheet on mobile.

```text
Reading settings

SCRIPT
○ Uthmani
○ IndoPak
○ Tajweed

ARABIC SIZE
−      36      +

TRANSLATION
Sahih International ▾

TRANSLATION SIZE
−      16      +

DISPLAY
☑ Transliteration
☑ Word by word

PALETTE
Sacred Editorial ▾

MODE
○ Light   ○ Dark   ○ System
```

### Important architecture

`Palette` and `Mode` are separate settings.

Do not label the palette dropdown “Theme” if “Dark/Light” is also called theme elsewhere.

Use:

- **Palette:** Sacred / Ink / Sepia / Sapphire
- **Appearance:** Light / Dark / System

---

# 26. Tajweed and Theme Compatibility

Quran Foundation’s current font-rendering guidance includes Light, Dark, and Sepia Tajweed palettes. COLRv1 browsers can use CSS `font-palette`, while Firefox may require theme-specific OT-SVG font files depending on the rendering path.

### Implication for EasyQuran

Do not assume that switching arbitrary application colors automatically makes Tajweed colors readable.

For each palette:

1. test Quran/Tajweed colors separately
2. ensure rule colors remain distinguishable
3. test against `--reader-background`
4. provide a known-good fallback palette

Recommended mapping:

| App palette    | Tajweed base                 |
| -------------- | ---------------------------- |
| Sacred Light   | Light                        |
| Sacred Dark    | Dark                         |
| Ink Light      | Light                        |
| Ink Dark       | Dark                         |
| Sepia Light    | Sepia                        |
| Sepia Dark     | Dark, then visually validate |
| Sapphire Light | Light                        |
| Sapphire Dark  | Dark                         |

---

# 27. Home Screen

Avoid a crowded dashboard.

```text
السَّلَامُ عَلَيْكُمْ

Continue your reading.

Al-Baqarah
Ayah 152 of 286

[ Continue reading → ]

────────────────────────────────────────

AYAH OF THE DAY

إِنَّ مَعَ الْعُسْرِ يُسْرًا

Indeed, with hardship comes ease.

Ash-Sharh · 94:6

────────────────────────────────────────

Explore
[ Topics ] [ Prophets ] [ Timeline ]
```

Progress metrics can follow below, but should not dominate the first viewport.

---

# 28. Continue Reading Card

```text
╭──────────────────────────────────────────────╮
│ CONTINUE READING                             │
│                                              │
│ Al-Baqarah                      البقرة       │
│ Ayah 152 of 286                              │
│                                              │
│ ████████████████░░░░░░  53%                 │
│                                              │
│ Continue →                                   │
╰──────────────────────────────────────────────╯
```

This is one of the few places where a stronger primary-colored feature surface is appropriate.

Do not place multiple equally strong progress cards beside it.

---

# 29. Daily Ayah

```text
A MOMENT TO REFLECT

              وَهُوَ مَعَكُمْ أَيْنَ مَا كُنتُمْ

       “And He is with you wherever you are.”

                       Al-Hadid · 57:4

                      Read context →
```

This card should use whitespace as the main aesthetic device.

No carousel dots unless it is actually a carousel.

---

# 30. Topics

Editorial card:

```text
┌────────────────────────┐
│ PATIENCE               │
│                        │
│ Sabr                    │
│ الصبر                   │
│                        │
│ 102 verses             │
└────────────────────────┘
```

Suggested topics:

```text
Mercy
Patience
Prayer
Forgiveness
Parents
Marriage
Justice
Creation
Death
Paradise
Knowledge
Prophets
```

Use a 2–4 column grid depending on viewport.

---

# 31. Prophet Explorer

Quran Companion’s connected-study model is particularly useful here.

```text
PROPHET

Musa
موسى عليه السلام

Mentioned across multiple surahs

Story · Timeline · Surahs · Relationships

Born in Egypt
     ↓
Raised in Pharaoh's household
     ↓
Leaves Egypt
     ↓
Receives revelation
     ↓
Returns to Pharaoh
```

Relationship cards:

```text
Harun
Brother

Fir'awn
Opponent

Bani Isra'il
Nation
```

The design should prioritize connected information rather than turning every relation into a decorative collectible card.

---

# 32. Historical Timeline

```text
CREATION ─ EARLY PROPHETS ─ IBRAHIM ─ MUSA ─ ISA ─ MUHAMMAD ﷺ
```

Events:

```text
━━━━━━━━●━━━━━━━━━━━━━━━━━━━━━━

       The Flood
       Nuh عليه السلام

       Hud 11:25–49
       Al-Mu'minun 23:23–30
```

### Color rule

Era differentiation should primarily use:

- labels
- position
- subtle soft backgrounds

not a rainbow palette.

---

# 33. Filter Chips

```text
[ All eras ]
[ Early Meccan ]
[ Middle Meccan ]
[ Late Meccan ]
[ Medinan ]
```

Active:

```css
background: var(--primary);
color: var(--primary-foreground);
```

Inactive:

```css
background: var(--surface);
border: 1px solid var(--border);
color: var(--foreground-secondary);
```

---

# 34. Progress and Habit Layer

Quran Gate demonstrates useful patterns such as:

- sequential reading progress
- daily goal
- streak
- 90-day activity visualization
- lifetime reading metrics

EasyQuran should make them quieter.

```text
Your Quran journey

18 day streak
1,842 ayahs read
9h 42m reading

██████████
██████████
███████░░░
```

### Rules

- never shame a broken streak
- never use aggressive red warning language
- no fake urgency
- no confetti during ordinary Quran reading
- celebration can be subtle for meaningful milestones
- content should remain more visually prominent than scorekeeping

---

# 35. Buttons

## Primary

```css
.button-primary {
  min-height: 44px;
  padding-inline: 18px;
  border-radius: 10px;
  background: var(--primary);
  color: var(--primary-foreground);
  font-weight: 550;
}
```

## Secondary

```css
background: var(--surface);
border: 1px solid var(--border);
color: var(--foreground);
```

## Ghost

Transparent until hover/focus.

## Accent/gold button

Avoid as a general CTA pattern.

Reserve accent color for exceptional or ceremonial contexts.

---

# 36. Icons

Use one icon family consistently, preferably Lucide or a similarly restrained line set.

```text
Standard UI: 20px
Navigation: 18px
Feature: 24px
Stroke: ~1.75
```

Do not mix:

- filled Material icons
- outline Lucide icons
- emojis
- custom heavy icons

in the same visual layer.

---

# 37. Inputs

```text
height: 44px
radius: 10–12px
border: 1px solid var(--border)
background: var(--surface)
```

Focus:

```css
outline: 2px solid var(--focus-ring);
outline-offset: 2px;
```

Do not remove browser focus indication without replacing it.

---

# 38. Tabs and Segmented Controls

## Tabs

Use for content destinations:

```text
Translation   Tafsir   Hadith   Notes
───────────
```

## Segmented control

Use for mutually exclusive view modes:

```text
[ Quranic order | Revelation order ]
```

Do not use pill segmented controls for six or seven unrelated navigation destinations.

---

# 39. Drawers, Dialogs, and Bottom Sheets

## Dialog

Use for:

- destructive confirmation
- share
- quick small action

## Right drawer

Use for:

- Tafsir
- reader settings
- verse information
- bookmarks
- collection editing

Suggested width:

```text
420–480px desktop
```

## Bottom sheet

Use on mobile for the same contextual actions.

---

# 40. Tafsir / Study Layout

Desktop split view:

```text
QURAN READER                       TAFSIR
────────────────                  ───────────────
Arabic                             Source selector
Translation                        Ayah reference
                                   Tafsir content
Arabic
Translation
```

Suggested split:

```text
Reader 60–65%
Study panel 35–40%
```

Or retain the 760–860px reader and overlay a 420–480px side drawer if preserving reader measure is more important.

---

# 41. Focus Reading Mode

This should be a signature feature.

When active:

- sidebar disappears
- top navigation reduces to Back / Surah / settings
- ayah actions are hidden until interaction
- reader centers
- study tools close
- search is removed from immediate view
- inactive audio bar collapses

Suggested width:

```text
max-width: 720–780px
```

Focus mode should work in every palette.

---

# 42. Reader Surface

A tiny luminance difference between app and reader creates a book-like surface without texture.

Example Sacred Light:

```text
App:    #F8F7F2
Reader: #FFFEFA
```

Example Sacred Dark:

```text
App:    #0D1210
Reader: #111713
```

Avoid fake paper grain behind Quran glyphs because it reduces clarity and complicates contrast.

---

# 43. Decorative Language

Allowed:

- one subtle arch frame around a featured Ayah
- small 8-point star as a section ornament
- hairline geometric divider
- low-opacity corner motif

Avoid:

- large mosque silhouettes behind text
- gold gradients
- glowing crescents
- patterned Quran reading backgrounds
- decorative geometry in every card

The Quran script itself already provides substantial visual richness.

---

# 44. Editorial Section Pattern

Signature pattern:

```text
الْأَنْبِيَاء
04 · PROPHETS

Stories connected across revelation.
────────────────────────────────────────────
```

Alternative compact version:

```text
الْقُرْآن · THE QURAN
```

Limit this treatment to major editorial moments.

---

# 45. Audio Player

Sticky player:

```text
╭─────────────────────────────────────────────────────────╮
│ ▶  2:255    Mishary Alafasy       ━━━━━━━────   03:14 │
│    Repeat verse   1×                    Volume     ⌃    │
╰─────────────────────────────────────────────────────────╯
```

Recommended:

```text
Desktop: 64px collapsed
Mobile:  60–64px collapsed
Expanded mobile: 120–160px
```

Do not reserve a large permanent audio bar if playback has not started.

---

# 46. Motion

```text
Hover                 120ms
Button                 140ms
Popover                160ms
Drawer                 220ms
Page transition        220–240ms
```

Curve:

```css
cubic-bezier(.2, .8, .2, 1)
```

Respect:

```css
@media (prefers-reduced-motion: reduce) {
  /* remove nonessential transforms/transitions */
}
```

Avoid bounce, springy card movement, parallax during reading, and looping ambient animations.

---

# 47. Responsive Layout

```text
sm    640
md    768
lg   1024
xl   1280
2xl  1536
```

Containers:

```css
/* desktop */
width: min(100% - 64px, 1280px);

/* tablet */
width: calc(100% - 48px);

/* mobile */
width: calc(100% - 32px);
```

---

# 48. Mobile Navigation

Bottom navigation:

```text
Read        Explore        Search        Saved
 ▱             ◇             ⌕             ♡
```

Keep it to four primary destinations.

Settings/profile belongs in the contextual top bar.

---

# 49. Mobile Reader Header

```text
‹                              ⋯

             البقرة
           Al-Baqarah
            The Cow

       286 Ayahs · Madani
```

As the user scrolls, this can collapse into:

```text
‹  Al-Baqarah · 2:37                      ⋯
```

---

# 50. Bidirectional / Arabic Layout Rules

This product must treat RTL as a first-class design constraint.

### Required

- set `dir="rtl"` directly on Quran/Arabic regions
- use logical CSS properties (`margin-inline`, `padding-inline`, `border-inline-start`)
- do not globally flip icons that should remain semantically fixed
- test mixed Arabic + Latin verse references
- isolate translation text in its own direction context
- use locale-aware numerals intentionally rather than accidentally

### Example

```html
<p lang="ar" dir="rtl" class="quran-text">...</p>
<p lang="en" dir="ltr" class="translation">...</p>
```

---

# 51. Accessibility Baseline

Minimum requirements:

- WCAG AA normal text contrast where applicable
- 44×44px touch targets for core interactive controls
- visible keyboard focus
- skip-to-content support
- headings in semantic order
- button names for icon-only actions
- screen-reader labels for playback state
- no state conveyed only by color
- `aria-current` for current Surah/navigation
- reduced-motion support
- zoom up to 200% without loss of core function
- large Quran text mode allowed to reflow instead of forcing Mushaf line fidelity

---

# 52. Component Inventory

## Shell

```text
AppShell
AppHeader
Sidebar
NavigationRail
MobileNavigation
PageContainer
ReaderContainer
```

## Primitives

```text
Button
IconButton
Badge
Chip
Tabs
SegmentedControl
Dropdown
Popover
Tooltip
Dialog
Drawer
BottomSheet
Divider
Skeleton
Toast
```

## Search

```text
SearchInput
CommandPalette
SearchResultGroup
SearchResultItem
```

## Content surfaces

```text
Card
EditorialCard
FeatureCard
StatCard
EmptyState
```

## Quran

```text
SurahRow
SurahHeader
Ayah
AyahMarker
AyahActions
QuranWord
Bismillah
Translation
TranslationSource
ReaderToolbar
ReaderSettings
AudioPlayer
StudyPanel
TafsirPanel
HadithPanel
ComparePanel
```

## Home / habit

```text
ContinueReadingCard
DailyAyah
ReadingGoal
ReadingProgress
StreakCalendar
ReadingStats
```

## Explore

```text
TopicCard
ProphetCard
ProphetRelationship
Timeline
TimelineEvent
ConnectionCard
```

---

# 53. Three Signature EasyQuran Components

These should give EasyQuran its own identity.

## 53.1 AyahCanvas

A nearly borderless Quran reading surface driven by Arabic typography, spacing, subtle ayah separators, and contextual actions.

This should become the product’s most recognizable interaction surface.

---

## 53.2 SacredHeader

```text
الْقُرْآن
THE QURAN

Read. Understand. Return.
```

Composition:

- Arabic eyebrow
- small numbered/English category label when useful
- Newsreader title
- restrained supporting copy

---

## 53.3 ConnectionCard

```text
A CONNECTION

Musa appears across
many Surahs.

Explore his story across
revelation →

موسى عليه السلام
```

This brings Quran Companion-style connected study into the product without turning the whole experience into a timeline app.

---

# 54. Things to Avoid

## Visual anti-patterns

- generic emerald-to-teal gradients
- every card having a shadow
- every container having 24–32px radius
- excessive Islamic ornament
- giant hero mosque photography behind text
- translucent glass panels in the Quran reader
- overly desaturated metadata that fails contrast
- gold used as ordinary body text
- pure white Quran text on pure black as the only dark mode
- multicolor dashboards
- card grids for content that is better scanned as a list

## UX anti-patterns

- permanent verse action toolbars
- study controls invading reading mode
- hiding the selected translation source
- losing the reader’s position when switching views
- making theme changes reset reading preferences
- using streak guilt or punitive language
- requiring horizontal scrolling for enlarged Quran text

---

# 55. Recommended Implementation Order

## Phase 1 — Foundations

1. semantic tokens
2. Sacred + Ink palettes
3. typography
4. spacing / radii / border / focus tokens
5. Button / Input / IconButton / Chip / Tabs

## Phase 2 — Quran core

1. AppShell
2. Quran index
3. Surah header
4. AyahCanvas
5. reader settings
6. audio player
7. word-by-word
8. Focus Reading

## Phase 3 — Study

1. Study Mode shell
2. Tafsir panel
3. Hadith panel
4. notes / collections
5. compare mode

## Phase 4 — Explore

1. topics
2. prophets
3. chronology
4. relationships
5. ConnectionCard

## Phase 5 — Habit

1. Continue Reading
2. reading goal
3. history
4. streak / activity view

## Phase 6 — Additional palettes

1. Sepia
2. Sapphire
3. Tajweed validation per palette

---

# 56. Theme QA Checklist

Every component must be reviewed in:

```text
Sacred Light
Sacred Dark
Ink Light
Ink Dark
Sepia Light
Sepia Dark
Sapphire Light
Sapphire Dark
```

For each combination verify:

- normal text contrast
- selected states
- hover states
- disabled states
- focus rings
- primary button foreground
- destructive button contrast
- icon-only actions
- tooltip/popover surface hierarchy
- Quran text clarity
- translation contrast
- Tajweed colors
- audio playback state
- bookmark state
- Makki/Madani labels
- skeleton/loading states

---

# 57. Visual QA Checklist

A screen is visually healthy when:

- there is one obvious focal point
- no more than one primary CTA competes in a region
- surfaces differ intentionally rather than randomly
- headings use consistent font roles
- card radii come from the shared scale
- icon sizes are consistent
- accent/gold is scarce
- alignment follows one grid
- spacing is more prominent than borders
- the Quran is visually stronger than controls

---

# 58. Reader QA Checklist

Before considering the reader complete, verify:

- Arabic remains readable at all supported sizes
- large type reflows gracefully
- translation does not exceed comfortable measure
- changing translation does not shift control placement unpredictably
- opening Study Mode preserves the current ayah
- returning from Study Mode preserves scroll position
- audio follows the currently visible/selected ayah
- keyboard users can reach ayah actions
- touch users can open actions without precision tapping
- Focus Mode actually removes distraction
- dark mode does not create excessively harsh white-on-black contrast

---

# 59. Final Recommended Direction

For the initial public design, ship:

### Default

```text
Palette: Sacred Editorial
Appearance: System
```

### Reader alternatives

Expose:

```text
Sacred Editorial
Ink
Mushaf Sepia
Midnight Sapphire
```

with:

```text
Light
Dark
System
```

separately.

This gives the user meaningful visual choice without fragmenting the product into different design systems.

## Final weighting of inspirations

```text
Niyat              35% visual warmth / Quran-list restraint
Ayah               20% editorial typography / bilingual framing
Quran.com          25% reader + study UX architecture
Quran Companion    10% connected-study model
Quran Gate         10% progress / habit mechanics
```

The weighting is intentionally not 1:1 with the earlier draft. After the deeper audit, Quran.com deserves slightly more influence because its current 2026 reader separates reading, Study Mode, personalization, translation-focused reading, compare, collections, and related study tools more explicitly than a purely aesthetic review suggests.

---

# 60. Reference Notes

The design system above is an original synthesis. The source products were used as references for interaction and visual direction, not as assets to copy.

### Ayah

- [Ayah](https://www.getayah.com/)
- Strong bilingual Arabic/English section framing.
- Quran tab and word-by-word learning are part of its current product direction.
- Uses restrained editorial composition and progress language.

### Quran.com

- [Quran.com](https://quran.com/)
- [Build Your Personalized Quran Experience](https://quran.com/es/explore/build-your-personalized-quran-experience)
- [New Study Mode](https://quran.com/product-updates/new-study-mode-on-quran-com)
- [New Translation Reading Mode](https://quran.com/product-updates/new-translation-reading-mode-read-like-a-book)
- [Product Updates](https://quran.com/product-updates)
- Current personalization includes multiple reading views and Light/Dark/Sepia appearance choices.
- Study Mode exposes deeper ayah/word exploration.
- 2026 updates include collections, reading bookmarks, verse comparison, translation reading, multiple Surah information sources, and related Hadith within Study Mode.

### Quran Foundation font documentation

- [Integrating Quran Font Rendering](https://api-docs.quran.com/docs/tutorials/fonts/font-rendering/)
- [Page Layout API Guide](https://api-docs.quran.com/docs/tutorials/fonts/page-layout/)
- Tajweed rendering includes theme handling, including Light/Dark/Sepia palettes.
- Large Quran font scaling may require relaxed wrapping instead of strict Mushaf line boundaries.

### Niyat

- [Niyat](https://niyatapp.com/)
- Current presentation emphasizes a calm warm-neutral background, green primary language, Quran search, Surah/Juz/Bookmark organization, Arabic names, and Makki/Madani metadata.
- Its public product language explicitly positions the Quran as a beautifully presented, low-distraction part of one calm Islamic companion.

### Quran Companion

- [Quran Companion](https://quran-companion.co/)
- [Chronological Reader](https://quran-companion.co/reader)
- [Prophets](https://quran-companion.co/prophets)
- Useful patterns include chronological vs Quranic ordering, Meccan/Medinan era filters, historical timeline, Prophets Explorer, relationship maps, parallel narratives, and character connections.

### Quran Gate

- [Quran Gate](https://qurangate.app/)
- Current behavior design centers sequential reading, daily goals, streaks, activity tracking, and progress mechanics.
- Its own product writing emphasizes a calm rather than punitive ritual, which is the correct principle to preserve even if EasyQuran does not implement app gating.

---

# 61. Short Design Brief for an AI/Coding Agent

If this document is being handed to an implementation agent, the minimum instruction is:

> Build EasyQuran using the semantic tokens and component rules in this document. The Quran reader is the highest-priority surface. Default to Sacred Editorial, but never hard-code Sacred colors into components; all components must work under Sacred, Ink, Sepia, and Sapphire palettes in light and dark mode. Preserve strict typography roles, accessible contrast, restrained radius/shadow usage, RTL correctness, and a reading-first hierarchy. Study tools may become dense, but Quran Reading and Focus Reading must remain visually quiet. Avoid gradients, glassmorphism, excessive Islamic ornament, generic SaaS dashboard styling, and aggressive gamification.

---

**End of design system.**
