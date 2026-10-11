# A2 — False positives and collateral damage (adversarial audit of DESIGN-v0)

Auditor: adversarial FP/collateral-damage pass. Date: 2026-10-12.
Inputs: `DESIGN-v0.md`, `R2-fingerprint-science.md`, `R2-ban-operations.md`, plus the repo itself
(`rust/backend/api/src/modules/device_v1/*`, `admin_bans_v1/dto.rs`, `services/escalation.rs`,
`middlewares/auth_guard.rs`, `web/src/routes/**`, `rust/backend/api/src/modules/*/mod.rs`).

**Excluded from this report (already known, not re-reported):** Safari 26 AFP noising; Brave farbling
revertibility; Firefox RFP being private/ETP-strict-only; VM/devtools/VPN base rates; India DPDP under-18;
no vendor publishing ban precision; shared-IP collateral (schools/cafés/CGNAT).

**Repo facts that change the audit** (verified in-tree, not in the design doc):

| Fact                                                                                                                                                                                                                                                                                                            | Consequence for this audit                                                                                                                                                                                                             |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `modules/device_v1/` **already exists**: `/register`, `/list`, `/delete`, per-user `devices` table, `token TEXT UNIQUE`, `MAX_DEVICES_PER_USER = 20`, oldest-evicted, and re-registering the same token **transfers ownership** (test: `registering_same_token_transfers_ownership_without_duplicate_delivery`) | The repo already models "one device token, several accounts" as normal and non-suspicious. DESIGN-v0 invents a _second_ device identity with different semantics (`UNIQUE(device_key)` + ban axis). Two device concepts will disagree. |
| `BanUnit` is `IpNet`; parse rejects `user@example.com`, `totp:42`, `login:someone` (unit tests assert this)                                                                                                                                                                                                     | `BanUnit::Device(device_key)` is an enum change on a type that today can only hold an IP network. Nothing else in the ban path understands non-network units.                                                                          |
| `services/escalation.rs` is **default-OFF, IP-only, allowlist-gated, bounded LRU, never permanent**, store keyed `quran-ban:{unit}`                                                                                                                                                                             | There is a working, conservative precedent. DESIGN-v0's device axis does not inherit any of those guards.                                                                                                                              |
| `middlewares/auth_guard.rs` ban check: 5-min cache, **fail-closed**; `pool = 1` SQLite connection                                                                                                                                                                                                               | Any new per-request device write is on the single connection, and any new fail-closed lookup converts DB latency into user-facing denials.                                                                                             |
| User-generated surface exists: `post_comment_v1` (create/update/delete/flag + admin hide), `media_v1` (create/delete), `bookmark_v1`, `user_v1` profile, `notification_v1`                                                                                                                                      | The ban ladder has a real target (comments/media/profile), so "read-only cooldown" is meaningful — but it also means a shared-device cooldown blocks a co-user's _comments_, not their reading.                                        |
| `web/src/routes/llms.txt`, `llms-full.txt`, `[slug].txt`, `[surah].slug.md`                                                                                                                                                                                                                                     | Machine/text consumers are a **deliberate** audience. A fingerprint-first pipeline must not treat them as incoherent humans.                                                                                                           |
| `bun run test` = vitest + happy-dom: **no canvas/WebGL/audio/UA-CH/IDB**                                                                                                                                                                                                                                        | The collector and its coherence booleans cannot be exercised by the existing gates. First real validation is production traffic.                                                                                                       |

---

## Part 1 — Population × misfire matrix

Misfire legend: **(a)** fires a challenge/step-up on a legitimate reader · **(b)** fires a ban/suspension
on a legitimate reader · **(c)** silently merges two different humans (or two different machines) into one
`device_record`, which then propagates (a)/(b) to strangers.

### A. One device, N humans (shared hardware)

| #   | Population / scenario                                                                       | Mechanism                                                                                                                                                                                                                                                                                                                                                                                                                                        | Misfire                                              | Base rate                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Sev      |
| --- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| A1  | **Family phone/tablet.** Several accounts log in on one handset.                            | Ladder rows 5–8 all key off "parent match". On a shared device **every** co-user has the parent match. Rows 5/6 accept "behavioural similarity" and "duplicated content" — both inheritable (family reads the same surah at the same hour; a shared phone has one language, one timezone, one IP). Row 7 suspends the _new_ account; row 8 permanently bans it on a second event. One teenager's ban becomes the sibling's and the parent's ban. | (b), then (a) for everyone else on that phone        | Pew Research, _Mobile Divides in Emerging Economies_ (2019): in India **30% of adults do not own a mobile phone**; among non-owners across 8 countries a median of **48%** say "another family member has a phone" (India **56%**, Philippines 59%); in India significantly **more women (20%) than men (5%)** borrow phones. IAMAI/Kantar _Internet in India 2024_: rural shared-device users **+24%** in a year; ~**80%** of shared-device users are rural; of rural shared-device users **76% are under 19, 58% female, 62% NCCS CDE**. | Critical |
| A2  | **Madrasa / school kiosk and PC lab.** Many students, few machines, one NAT.                | (i) Every student's session carries the same `device_key` → ladder row 2 ("≥3 accounts, all benign") only _observes_, but any later ban on one student activates rows 4–7 for all. (ii) School NAT is one IPv4 `/32` in this design → IP-axis ban takes out the school. (iii) Lab machines are cloned images → different machines collapse into one record (see C2).                                                                             | (b) whole-school block; (a) every student challenged | No published rate for kiosks. Adjacent: Wikipedia's shared-IP doctrine and IP block exemption (666 accounts holding the flag at time of reading, R2-ban-operations §1); Fingerprint's _Schoolhouse_ case study shows device-ban deployment on an education platform with minors — cited here as a **warning, not a template**. R2-ban-operations already recommends kiosk registration + `ip_legit_share`; DESIGN-v0 adopts neither.                                                                                                       | Critical |
| A3  | **Cybercafé / community centre / library.** Unrelated people, same PC, sequential sessions. | The record is a public square. A ban on the device is collective punishment for everyone who sits down next. "Device change mid-session" step-up fires when a user moves to another PC to finish a task.                                                                                                                                                                                                                                         | (b), (a)                                             | No published rate; structurally identical to Wikipedia's "two or more editors use the same computer" warning (WP:Sockpuppetry § Sharing an IP address).                                                                                                                                                                                                                                                                                                                                                                                    | High     |

### B. One human, N device records (false "new device" → friction)

| #   | Population / scenario                                                                | Mechanism                                                                                                                                                                                                                                                                                                                                                 | Misfire                               | Base rate                                                                                                                                                                                                                                                                                                                                                              | Sev                      |
| --- | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| B1  | **Storage-clearing / reinstall users** (low-end Android, weekly clear).              | Token gone → unknown device → step-up at login. Component key may also shift. See Part 4 for the full engine view.                                                                                                                                                                                                                                        | (a) weekly                            | Li & Cao, IMC 2020: **32% of browser instances cleared cookies** during an 8-month study (largely ITP). Safari ITP deletes **all script-writable storage** after 7 days without interaction (WebKit _Tracking Prevention_; WebKit blog 10218). Chrome/Firefox evict origin storage **under storage pressure, LRU first** (MDN _Storage quotas and eviction criteria_). | High                     |
| B2  | **Private/incognito readers.**                                                       | Storage is per-session → unknown device every session. (Inverse harm: if the component key is the identity, incognito is silently linked to the same person's normal browsing — clear-site-data defeated.)                                                                                                                                                | (a) every session; privacy harm       | No published rate for a specific site. Firefox RFP scoping is out of scope (known).                                                                                                                                                                                                                                                                                    | Medium-High              |
| B3  | **iOS reader with a home-screen web app** (+ Safari occasionally).                   | Home-screen web apps are **storage-isolated from Safari** → one human = two device identities, two `trust_state`s.                                                                                                                                                                                                                                        | (a) on every context switch           | WebKit bug 181849: "Web apps run from the home screen don't share storage with Safari."                                                                                                                                                                                                                                                                                | High                     |
| B4  | **In-app browsers** (WhatsApp / Instagram / Facebook).                               | Android WebView is **excluded from UA reduction and UA-CH** (Chromium UA-Reduction doc: "no current plans … on iOS and Android WebView"), and has separate storage from Chrome → a new, UA-CH-less device identity per app. WebView fingerprints differ from Chrome on the _same_ device and can carry account-specific information (Tiwari et al. 2022). | (a); (c) same human split across apps | ~**31% of mobile web sessions** happen in in-app browsers (MobiLoud 2026 — secondary, flagged; treat as indicative). For a Quran app whose links are shared on WhatsApp, expect more.                                                                                                                                                                                  | High                     |
| B5  | **Browser switchers** (Chrome ↔ Samsung Internet ↔ UC ↔ Opera ↔ Firefox).            | Each browser = new token + new component key + a different coherence profile.                                                                                                                                                                                                                                                                             | (a)                                   | StatCounter, India mobile, Sept 2026: Chrome **90.62%**, Safari 3.61%, Opera 3.51%, UC 0.97%, Samsung Internet 0.79%, Firefox 0.27% → **~9.4% non-Chrome**, each switch a new device.                                                                                                                                                                                  | Medium-High              |
| B6  | **Device/OS/browser/driver updates.**                                                | Component-key churn. FP-Stalker: **45.52% of fingerprints changed after one day**; Li & Cao: changes cluster on real release dates.                                                                                                                                                                                                                       | (a) in waves on release days          | FP-Stalker (USENIX 2018) 45.52%/1 day; Li & Cao IMC 2020 (release-date clustering).                                                                                                                                                                                                                                                                                    | High                     |
| B7  | **Zoom / display scaling / external monitor / rotation / split-screen / foldables.** | The screen bucket changes → key changes. Foldables change screen metrics when unfolded; Android split-screen changes `innerWidth/Height` mid-session; Safari 26 fixes `screen.*` to `inner.*` (so a "screen vs inner" rule means different things per platform).                                                                                          | (a)                                   | Screen entropy is stable in the population (4.8–4.9 bits, Eckersley 2010 → Gómez-Boix 2018) but **churns on monitor/dock/zoom** (R2-fingerprint-science §2.4).                                                                                                                                                                                                         | Medium                   |
| B8  | **PWA standalone mode** (Android/iOS).                                               | Standalone viewport ≠ browser viewport → screen/inner coherence can differ; display-mode is a different context.                                                                                                                                                                                                                                          | (a)                                   | No published rate.                                                                                                                                                                                                                                                                                                                                                     | Medium                   |
| B9  | **VPN / censorship circumvention users.**                                            | IP↔timezone↔locale coherence fires; ~1 in 5 sessions globally are VPN (FingerprintJS 2026: ~20% overall, 33% Chromium desktop, 13% mobile) and in censorship countries VPN use is _mandatory_ for this audience. R2-fingerprint-science §7.4 rule 10 recommends exactly this check.                                                                       | (a) if rule 10 is imported            | FingerprintJS Device Intelligence Report 2026 (~1 in 5). Country-level: Cybernews VPN index 2025 puts UAE ~85%, Qatar ~58% (download-based, indicative).                                                                                                                                                                                                               | High if rule 10 imported |
| B10 | **Mobile network churn (CGNAT, IPv6 rotation, Wi-Fi↔data).**                         | If "session stability / IP change mid-session" is a risk feature (R2-session-binding §5), mobile users fire it constantly.                                                                                                                                                                                                                                | (a)                                   | RFC 6888 documents many subscribers per public IP; IPv6 privacy addresses rotate (RFC 8981). Shared-IP _collateral_ is on the excluded list; this entry is the **per-user churn** angle, which is new.                                                                                                                                                                 | Medium                   |

### C. Two humans / two machines → one `device_record` (silent merges)

| #   | Population / scenario                                                                                | Mechanism                                                                                                                                                                                                                                                                                                                                                                                                                | Misfire                                    | Base rate                                                                                                                                                                                                                                                                                                                                                                                                                | Sev      |
| --- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| C1  | **Mass-market Android cohort.** Same model, same ROM, same locale, same carrier, same screen bucket. | `device_key = HMAC(components)` with **deliberately coarse** components (screen "coarse bucket only"; hardware "family"; no canvas/audio/font keys). The key therefore identifies a _cohort_, not a device. `UNIQUE(device_key)` asserts 1:1 — false in both directions. On a phone-first, locale-concentrated audience (Arabic/Urdu/Indonesian/Bengali) the key space is small enough that a single ban hits strangers. | (c) → (a)/(b)                              | Gómez-Boix et al., WWW 2018: **mobile fingerprints are only 18.5% unique** → ≥81.5% shared, and that study still used canvas/WebGL/audio. 2026 vendor masking shrinks the space further: Chrome freezes the Android UA to "Android 10; K" for everyone; Safari 26 freezes the iOS UA OS token; Brave v1.93 collapses WebGL vendor/renderer to one string and noises extensions; Firefox FPP clamps cores→2, touch→0/1/5. | Critical |
| C2  | **Corporate VDI, thin clients, cloned school images.**                                               | Identical images → identical component sets → one record for hundreds of machines. Any action on the record hits the whole fleet. Software rendering (SwiftShader) is both a VM tell and a legitimate thin-client tell.                                                                                                                                                                                                  | (c) → (b)                                  | FingerprintJS 2026: **VMs appear in 13% of Chromium-desktop identifications**; tampering 4.4% desktop.                                                                                                                                                                                                                                                                                                                   | Critical |
| C3  | **Two siblings with identical handsets, same school Wi-Fi, same locale.**                            | C1 + A1 combined: identical hardware merges _different devices_; shared household merges _different humans_. Each sibling's device is the other's "already seen" device.                                                                                                                                                                                                                                                 | (c) → (b)                                  | Top-selling Android models ship in tens of millions per SKU; no per-model public base rate available.                                                                                                                                                                                                                                                                                                                    | High     |
| C4  | **Attack: deliberate key collision (see Part 3).**                                                   | Components are client-supplied, so an attacker _chooses_ their key: spoof to the modal config of a country, or to a specific victim's known components. The key then accumulates violations; genuine users of that bucket pay.                                                                                                                                                                                           | (b) at population scale; (a) as cohort DoS | No published rate; the mechanism is the design's own "challenge freely" + client-chosen key.                                                                                                                                                                                                                                                                                                                             | Critical |

### D. Signals structurally absent or vendor-masked

| #   | Population                                                                               | Mechanism                                                                                                                                                                                                                                                 | Misfire                                                  | Base rate                                                                                                                                                                                                                               | Sev    |
| --- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| D1  | **KaiOS / JioPhone.**                                                                    | Gecko 48 lineage: no WASM (WASM shipped in Firefox 52), no UA-CH, limited/absent WebGL → "hardware class" absent and JS-timing signature fails.                                                                                                           | (a) if absence is scored                                 | JioPhone was the volume KaiOS device in India; no verified shipment figure in this session — treat as "large, low-end, India-concentrated".                                                                                             | Medium |
| D2  | **Old Android WebView (Android ≤7).**                                                    | UA format `Version/4.0 Chrome/xx`; feature matrix stale; no UA-CH; `Intl` gaps.                                                                                                                                                                           | (a)                                                      | Android version distribution is long-tailed in low-income markets; no single citable figure used here.                                                                                                                                  | Medium |
| D3  | **Tor / LibreWolf / Mull / Bromite / GrapheneOS / /e/OS / LineageOS / Lockdown Mode.**   | These are _designed_ to be uniform or incoherent-looking: letterboxed 200×100 windows, UTC timezone, uniform UA, blocked canvas/audio, disabled JIT/fonts, spoofed UAs. Every one of them trips the design's coherence booleans by construction.          | (a) on every session; (b) if coherence feeds a threshold | Tor: uniformity is the protection (Tor design doc); LibreWolf/Mull ship RFP-hardened defaults; /e/OS ships a UA-spoofing default browser. For a Quran app this is the censorship-country audience R2-evasion-counters names explicitly. | High   |
| D4  | **Proxy-rendered browsers** (Opera Mini, extreme data-saver modes, transcoding proxies). | JS executes **on the proxy**, so every user of the same Opera Mini version presents the _proxy's_ JS environment → one `device_key` for the whole population, and the UA looks ancient (`Opera/9.80 … Opera Mini/xx`) so version-gated checks fire too.   | (c) mass merge + (a)                                     | Opera is **3.51%** of Indian mobile traffic (StatCounter Sept 2026), materially higher in parts of Africa; Opera's own docs confirm server-side JS ("Opera Mini and JavaScript").                                                       | High   |
| D5  | **Text / LLM / crawler clients.**                                                        | No canvas/WebGL/UA-CH/IDB at all; and the repo _intends_ to serve them (`llms.txt`, `llms-full.txt`, `.md`, `.txt`). R2-fingerprint-science §6.3: AI agents doing legitimate work are a real traffic class and blanket automation blocking is incoherent. | (a) or wrong classification                              | No base rate; the repo's own route table is the evidence.                                                                                                                                                                               | Medium |
| D6  | **The repo's own test/CI environment.**                                                  | happy-dom has no canvas/WebGL/audio/UA-CH/IDB, so the collector is untestable by `bun run test`; CI smoke tests look headless.                                                                                                                            | Rules ship unvalidated; CI may trip its own challenges   | Repo fact (design doc line 13 + `web/package.json` test stack).                                                                                                                                                                         | Medium |

---

## Part 2 — Attack on the "coherence booleans"

### 2.0 The structural problem

Three rules are being violated by the design as written:

1. **Absence is treated as inconsistency.** Every signal in Layer 1 is optional in the web platform. A
   boolean like `ua_ch_matches_ua` is undefined — not `false` — when UA-CH does not exist (Safari, Firefox,
   Android WebView, Chrome iOS, Opera Mini, KaiOS, text clients).
2. **Coherence has no per-family baseline.** Chrome, Safari, Firefox and Brave have _different, documented_
   coherent states. A rule is only meaningful relative to (browser family × platform × version) support.
3. **Masking is a vendor feature, not an attack.** Brave, Safari (AFP), Firefox (FPP), Tor and Lockdown Mode
   deliberately produce values that _look_ incoherent against an unmasked baseline. Scoring them as
   "spoofing" penalises the privacy-protected — the exact users Layer 0 promises not to penalise.

The **sharpest concrete case**: Chrome's reduced UA says `Linux; Android 10; K` for **every** Android Chrome
user and, per Chromium's own doc, "these strings are literal values; they will not update even if a user is
on an updated operating system or device" — while `Sec-CH-UA-Platform-Version` / `UADataValues.model` return
the **real** Android version and model. A naive `UA ↔ UA-CH` coherence check therefore flags **Chrome-on-Android
— 90.62% of Indian mobile traffic (StatCounter, Sept 2026)** — as inconsistent. The design's own flagship
coherence check, implemented literally, misfires on the majority of its users.

### 2.1 Per-case verdicts

| Configuration                                                                                                       | Why it looks incoherent                                                                             | Does DESIGN-v0 false-positive?               | What it must do instead                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **UA-CH absent on Safari**                                                                                          | No `navigator.userAgentData`                                                                        | **Yes** if `UA↔UA-CH` is scored              | `ua_ch = unavailable`; skip the check. Safari is ~3.6% of Indian mobile traffic and the default on iOS.                                         |
| **UA-CH absent on Firefox**                                                                                         | Same                                                                                                | **Yes**                                      | Skip; Firefox is 0.27% India but material in EU/US/IR.                                                                                          |
| **UA-CH absent on Android WebView / Chrome iOS**                                                                    | Chromium: reduction/UA-CH not planned for iOS or Android WebView                                    | **Yes**                                      | Detect WebView/`CriOS` and skip; these are link-open contexts, not devices.                                                                     |
| **`navigator.platform` = "Linux x86_64" on ChromeOS**                                                               | Reduced value is fixed per OS; ChromeOS and Linux both map to `Linux x86_64` (Chromium doc)         | **Yes** if platform is cross-checked         | Drop `navigator.platform` from coherence entirely; use UA-CH `platform` when present, else UA family only.                                      |
| **`navigator.platform` = "Linux armv81" on Android**                                                                | Fixed reduced value, unrelated to UA-CH `Android`                                                   | **Yes**                                      | Same.                                                                                                                                           |
| **Android Chrome reporting "Android 10" in the UA on Android 14**                                                   | Frozen literal in reduced UA; real value in UA-CH                                                   | **Yes, at population scale**                 | Treat the frozen Android/`K` tokens as a _known_ mismatch class; compare only major version downward (UA ≤ UA-CH).                              |
| **iPad in desktop mode** (`Macintosh; Intel Mac OS X 10_15_7` + `maxTouchPoints = 5`)                               | "Desktop claiming touch"                                                                            | **Yes**                                      | `MacIntel` + touch>0 is the iPad signature: an explicit allowlist entry.                                                                        |
| **Firefox FPP clamps** (`hardwareConcurrency=2`, `maxTouchPoints∈{0,1,5}`, `height−48`)                             | Looks like a low-end desktop or a spoofer                                                           | **Yes** (hardware-triple + screen rules)     | Recognise the FPP clamp signature and _lower_ risk; never score the clamped values as hardware facts.                                           |
| **Brave ≥1.93** (generic WebGL vendor/renderer, noised extension list, emptied WebGPU descriptors)                  | GPU class unknown; extensions unstable                                                              | **Yes**                                      | `renderer = masked` is a first-class state. Never incoherent; never "VM".                                                                       |
| **Safari 26 / iOS** (AFP noise on canvas/audio readback, `screen.* = inner.*`, `screenX/Y = 0`, frozen UA OS token) | Screen rules behave differently; UA never churns                                                    | **Yes** (screen-vs-inner, update-churn)      | Per-platform expected relations; for Safari expect `screen==inner` and **no** UA churn.                                                         |
| **Tor Browser**                                                                                                     | Letterboxed window, UTC timezone, uniform UA, no canvas/audio, no UA-CH                             | **Yes**, on many booleans                    | Detect and exempt; never challenge (see F8 for the challenge-host problem).                                                                     |
| **LibreWolf / Mull**                                                                                                | RFP-hardened defaults                                                                               | **Yes**                                      | Allowlist by known-good masking signature.                                                                                                      |
| **Lockdown Mode (iOS/macOS)**                                                                                       | Fonts blocked, JIT off, canvas/WebGL restricted                                                     | **Yes**, plus collector failure              | Treat as maximum-privacy; risk lower, never higher.                                                                                             |
| **GrapheneOS / /e/OS / LineageOS / Bromite**                                                                        | Hardened or UA-spoofing defaults, older WebView                                                     | **Yes**                                      | Allowlist "privacy-hardened Android"; do not score vendor spoofing.                                                                             |
| **Firefox ESR**                                                                                                     | A year behind; version-gated API checks fail                                                        | **Yes**                                      | Version-gate only downward and only within Chromium; ESR is common in schools/enterprise.                                                       |
| **Chromebooks**                                                                                                     | `CrOS` UA, `Linux x86_64` platform, school-shared                                                   | **Yes**                                      | Platform family from UA only; shared-device handling (Part 5).                                                                                  |
| **Android tablets**                                                                                                 | Touch + non-`Mobile` UA; UA-CH `formFactors` often absent                                           | **Yes**                                      | Device class from UA-CH `mobile`/UA, not from touch counts alone.                                                                               |
| **PWA standalone**                                                                                                  | Different viewport; separate storage on iOS                                                         | **Yes**                                      | Compare within display-mode; record mode as an attribute.                                                                                       |
| **In-app browsers (IG/WA/FB)**                                                                                      | WebView UA, no UA-CH, separate storage                                                              | **Yes**                                      | Context attribute; never step-up on context change.                                                                                             |
| **Samsung Internet / Yandex / UC / Opera**                                                                          | Non-Google Chromium forks; UA/brand lists differ; some don't send UA-CH                             | **Yes** if brand-list equality is checked    | Check "contains Chromium", never equality; unknown browser ⇒ unknown, not suspicious.                                                           |
| **Opera Mini / data-saver proxies**                                                                                 | UA looks ancient; JS ran on the proxy                                                               | **Yes**, catastrophically                    | Classify proxy-rendered; exclude from the device axis entirely.                                                                                 |
| **KaiOS**                                                                                                           | No WASM/WebGL/UA-CH                                                                                 | **Yes** if absence is scored                 | Degrade to "no hardware class"; reader must work without the collector.                                                                         |
| **Corporate-managed browsers**                                                                                      | Policies disable WebGL/hardware accel, override UA (`UserAgentReduction` policy exists), block APIs | **Yes**                                      | Policy-shaped absence = unavailable; corporate NAT/image handled as shared fleet.                                                               |
| **Accessibility & translation extensions**                                                                          | Override native functions; rewrite DOM; alter font usage                                            | **Yes** if "native-fn tampering" is scored   | Context only, never punitive. Assistive tech is legally protected use and is used disproportionately by the app's elderly/low-literacy readers. |
| **Privacy extensions (Canvas Blocker etc.)**                                                                        | Detectable by design (Vastel 2024; ~20K users)                                                      | **Yes**                                      | Layer 0 already says "no penalizing privacy browsers"; the tamper flags contradict it. Resolve in favour of Layer 0.                            |
| **Headless CI / crawlers / LLM agents**                                                                             | No signals; `webdriver` etc.                                                                        | By design (true positive) but must be scoped | Scope automation rules to authenticated **write** paths; never gate the repo's own `.md`/`llms.txt` read routes.                                |
| **Foldables / rotation / split-screen**                                                                             | Screen metrics change mid-session                                                                   | **Yes**                                      | Screen class is a _set over the session_ (max/modal), never per-request equality.                                                               |

### 2.2 Required coherence contract (normative)

1. Every signal has state `{value, masked, unavailable, spoofed}`. Only `spoofed` can flag, and `spoofed`
   requires **positive, self-contradictory evidence** (e.g. native-function tampering _plus_ a value that
   contradicts its own claimed engine) — never mere absence or a vendor clamp.
2. Checks are **directional**: "feature present that the claimed engine cannot have" is a flag;
   "feature missing that the claimed version should have" is not (flags, origin trials, enterprise policy,
   ESR, frozen UA).
3. A versioned **per-family support matrix** (Chromium / WebKit / Gecko × platform × major) is required,
   with a "known-masked configurations" allowlist (Brave, Safari AFP, Firefox FPP, Tor, Lockdown, ESR,
   Chromebooks, iPad desktop mode, WebView).
4. Coherence booleans are **excluded from ban thresholds entirely** (see F4/F6). They may raise review
   priority; they may never be a ban input.
5. Release-day damping: for N days after a major Chrome/Safari/Firefox release, thresholds rise and
   update-shaped deltas are treated as the _same_ device (fuzzy re-link).
6. Instrument for bias: challenge rate by browser family, by country, by device class, by display mode.
   A materially higher rate for Safari/Brave/Firefox/WebView/Tor than Chrome Android is a bug report,
   not a finding about users.

---

## Part 3 — Stress test: "challenge freely, ban only with corroboration"

### 3.1 What a shared device can and cannot distinguish

Let `D` = device match, `P` = prior ban of a linked parent account, `C` = corroborators. The design's rule is
effectively `punish ⇔ D ∧ P ∧ C`.

**The inheritance lemma.** On a shared device, a legitimate co-user inherits _every_ signal the banned user
produced on that hardware: the device key, the network, the locale/timezone, the screen, the reading trail,
the timing distribution, and often the language of the content they read. Therefore any corroborator drawn
from {device, network, locale, screen, behaviour, reading pattern, content-topic} is **inheritable** and
cannot distinguish "same person evading" from "different person sharing". Those families are _inadmissible_.

**What survives** is person-level and out-of-band:

| Admissible corroborator                                                                                                                               | Forgeable by a legitimate co-user?                                                                              | Forgeable by a malicious co-user (on the same device)?                                       | Verdict                                                                                     |
| ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Credential/identity reuse: same normalised email/phone, same OAuth subject, same passkey/platform credential, reused password (keyed hash comparison) | Rarely, but family-shared email/phone is common in this population (Pew: the family phone _is_ the access path) | **Yes** — the attacker can register with the family email or reuse a shared recovery channel | Admissible as a _strong_ signal; must not alone authorise L6; pair with the functional test |
| Identical **violating** content (spam payload, threat text, same target) — never Quran text, which is identical by design (R2-ban-operations §2)      | No                                                                                                              | **Yes** — reproduce the payload to frame a co-user                                           | Admissible; human review before L5+                                                         |
| Declared/verified link (recovery address points at the banned account; same org/school code; explicit disclosure)                                     | Family links are legitimate                                                                                     | **Yes**                                                                                      | Admissible as a lead, not a verdict                                                         |
| Payment instrument                                                                                                                                    | n/a (free reader)                                                                                               | n/a                                                                                          | Absent                                                                                      |

**Conclusion for the design:** the minimal corroboration set that a legitimate co-user cannot manufacture is
`{P} ∧ {credential/identity reuse ∨ identical violating content ∨ declared link}`, and even that set is
forgeable by a _malicious_ co-user. Therefore device-derived evidence must never be sufficient for a ban;
the design must additionally apply the **functional test** ("does the second account let the user do
something their first account was prevented from doing?" — Stack Exchange doctrine, R2-ban-operations §3),
the **continuity test**, and **human review** before L5+.

### 3.2 Attacker model: the malicious user _on_ the shared device

Three attacks the design does not currently answer:

1. **Frame the kiosk.** Post violating content from a madrasa/family device. The device key accrues the
   violation; ladder rows 4–7 then challenge/hold/suspend the _next_ innocent user of that device, and the
   kiosk's users cannot clear it (no self-service path exists in DESIGN-v0).
2. **Frame a stranger by cohort.** Because the key is coarse and client-chosen, the attacker spoofs to the
   modal Android configuration of a target country. Every genuine user of that bucket now carries an
   elevated key. With `BanUnit::Device`, that is a population-scale ban (F1/F2).
3. **Frame a specific victim.** If the victim's device model/OS/locale is known (public for mass-market
   SKUs), the attacker replicates those components, triggers violations under that key, and the victim's
   own sessions become "device match to a banned token".

**Mitigations that follow:**

- The unit of enforcement is the **account**, always. Device evidence may only raise review priority.
- Any device-linked action is capped at **L2** (challenge) and must be **self-service clearable**
  ("who else uses this device?"), with the clearing scoped to the _device_ and never to the _account_.
- A per-key **action budget** (at most one review case per key per window) turns cohort-burn from a
  population DoS into a nuisance.
- A **key population threshold**: a key seen with more than N distinct accounts or M distinct tokens is
  marked non-actionable (log only). This is the single cheapest guard against C1/C2/C4.
- **Device reputation decays** (e.g. 30 days) and is not inherited by a new account on the same device.
- The reverse case must be handled too: a shared-device claim must not become an evasion tool (it clears
  the _device_, never the _account's_ ban), which is exactly why account-level enforcement must be
  independent of the claim.

### 3.3 The honest accounting

If device-only can never ban, and the key is coarse, the device axis's marginal value over
account+content+credential is: **challenge spam on co-users plus a small recapture gain on naive evaders**.
R2-ban-operations already states the realistic posture: _"recapture naive evasion, accept that a determined
evader on a different phone is uncatchable, and never let a fingerprint alone cost a real reader their
account."_ DESIGN-v0 should therefore **drop `BanUnit::Device`** and keep `device_records` as an evidence
and review-prioritisation store. This is not a weakening; it is the only version of the design that survives
contact with A1–A3 and C1–C4.

---

## Part 4 — Storage clearing: what the engine sees, and the trap

### 4.1 The engine's view of a weekly-clearing user

| Step    | What happens                                                                                                                                | Design reaction                                                                                                                                         |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Week 1  | New account, no token → `device_key` computed from components, `device_records` row created, `trust_state = unknown`                        | Step-up "new device at login" (Layer 3)                                                                                                                 |
| Week 2  | User cleared storage → token gone. Components mostly unchanged → same `device_key` → **row reused**, `seen_count++`, `first_seen` unchanged | If keyed on components: _no_ new device — the clear is invisible. If keyed on the token: new record, trust resets again                                 |
| Week 3+ | Repeat                                                                                                                                      | Either (a) trust never accrues (token-keyed), or (b) the user is silently re-identified across an explicit storage clear (component-keyed)              |
| Ongoing | `device_records` indexed on `user_id` → "account with N devices" is a directly computable feature                                           | This is the textbook account-takeover/abuse shape in anti-fraud scoring; the engine will score the most memory-constrained users as the most suspicious |

### 4.2 The trap

Both keying choices are wrong in different ways:

- **Token as identity** → churn is visible and the user is punished for being memory-constrained
  (step-up at login every week, `trust_state` never stabilises, account flagged for device fan-out).
- **Components as identity** → the user is _re-identified_ after explicitly clearing site data. For a
  **religious** app whose users may clear storage precisely because the phone is shared, that is a serious
  privacy harm and it defeats the clear-site-data control that Safari/Chrome users rely on. It also makes
  the token decorative: a spoofer who keeps their components can be linked forever, while a legitimate
  user cannot escape the link.

### 4.3 What to do instead

1. **Account-level trust.** Trust, step-up state and challenge outcomes attach to the account, not to the
   device. A user who clears storage and logs back into the same account is the same user.
2. **Device churn is not a risk feature.** Never score "new device", "N devices on account", or "device
   first seen today". Delete any such feature from the rule set.
3. **Do not re-identify after an explicit storage clear.** If the token is absent, the device is unknown —
   accept it. Continuity comes from the account session and the credential.
4. **Unknown device = zero friction** (Part 5, posture). Step-up only when the _action_ is high-risk
   (credential change, new email/phone) and there is no fresh-auth grant.
5. **SLO on unknown-device share.** Track the fraction of sessions with no device token. On the expected
   population (Safari ITP, low-end Android eviction, privacy clearers) a large steady share is _normal_.
   If it is high, that is a signal the device signal is not a device signal — not that users are suspicious.
6. **One storage location.** The design stores the token in localStorage **and** IDB. If they disagree
   (partial clear, Safari's 7-day cap applying to script-writable storage), treat it as _unknown_, never as
   tampering, and rewrite both.
7. **Never let a cleared-storage user be more suspicious than a brand-new user.** If the engine cannot
   distinguish them, it must treat both as low-risk, which is the correct default for a free reader.

---

## Part 5 — Required countermeasures and the "unknown device" posture

### 5.1 Per-scenario countermeasures

| Scenario                          | Required countermeasure                                                                                                                                                                                     |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1 family phone                   | Account-scoped enforcement; device-triggered actions capped at L2; "who else uses this device" self-service claim; device reputation decay; never propagate a ban to co-users                               |
| A2 madrasa/school                 | Kiosk/organisation registration from day one (token scoped to the org); `ip_legit_share` suppression (R2 §3); step down one level for any action on a registered shared device; never IP-`/32`-ban a school |
| A3 café/library                   | Device record treated as a public location; no punitive device actions; no step-up on "device change mid-session"                                                                                           |
| B1/B2 storage clearing, incognito | Part 4 countermeasures; unknown = fail-open; no re-identification after clear                                                                                                                               |
| B3 iOS home-screen app            | Display-mode attribute; account continuity across Safari↔standalone; never a step-up trigger                                                                                                                |
| B4 in-app browsers                | Context attribute (`webview`); never step-up; never treat as a second device in scoring                                                                                                                     |
| B5 browser switching              | One account, many device records is normal; no fan-out feature                                                                                                                                              |
| B6 updates                        | Update-shaped deltas = same device (fuzzy re-link); release-day damping                                                                                                                                     |
| B7 screen changes                 | Screen class = session-stable set (max/modal); never per-request equality                                                                                                                                   |
| B9 VPN / censorship               | IP↔tz check probabilistic and _advisory only_; never a challenge trigger; Tor/VPN users exempt                                                                                                              |
| C1/C2/C3 merges                   | Key population audit + non-actionable threshold; never `UNIQUE(device_key)`; never `BanUnit::Device`                                                                                                        |
| C4 attacker-chosen key            | Server-derived punitive evidence only; per-key action budget; challenge-storm circuit breaker                                                                                                               |
| D3 privacy browsers/OSes          | Known-good masking allowlist; matching it lowers risk                                                                                                                                                       |
| D4 proxy browsers                 | Detect (`Opera Mini` token, `X-OperaMini-Phone-UA`); exclude from device axis entirely                                                                                                                      |
| D5 text/LLM clients               | Exempt declared read-only clients; device rules scoped to authenticated writes                                                                                                                              |
| D6 test env / CI                  | Shadow mode + rule replay + golden vectors; kill switch; never enforce a rule that has not passed a precision gate                                                                                          |
| All                               | Bias dashboard: challenge/step-up rate by browser family, country, device class, display mode                                                                                                               |

### 5.2 Default posture for "unknown device" (normative)

1. **Unknown is the default state, not a risk state.** Most sessions will be unknown at any moment
   (ITP, eviction, privacy clearing, in-app browsers, new devices). Treat it as the baseline.
2. **Reading is never gated.** No challenge, no step-up, no redirect on reader routes or read APIs — for
   any device state. (The design already bypasses `/t/**`; extend the principle to all reads.)
3. **Login is never gated by device state.** The session cookie is the auth. A device signal may not
   prevent a login, a password reset, or an appeal.
4. **Step-up only for high-risk _writes_** (credential/email/phone change, new account + write burst), and
   only when there is no fresh-auth grant. Never "new device at login" as a standalone trigger.
5. **Every challenge has a fallback that does not depend on `challenges.cloudflare.com`** (email OTP link,
   or read-only-until-verified). If the challenge widget cannot load, the action queues; it does not block.
6. **Unknown device never contributes to a ban score.** Bans require account-level, person-level evidence
   (Part 3) and human review.
7. **Never punish the privacy-protected configuration.** Matching a vendor-masking signature lowers risk.
8. **Any device-derived friction is self-service clearable** in one click, in the user's language, and
   reachable while suspended.

---

## Findings (12, all NEW)

### F1 — `device_key` is a cohort key, not a device key; `BanUnit::Device` bans populations

**Severity: Critical**
**Mechanism.** `device_key = HMAC(coarse components)` + `UNIQUE(device_key)` + `BanUnit::Device(device_key)`
assert 1 key = 1 device. The components are deliberately coarse (screen "coarse bucket only", hardware
"family", canvas/audio/font excluded as keys) and the population is phone-first and locale-concentrated, so
the key identifies a bucket of thousands. Mobile fingerprints are only 18.5% unique in the canonical
large-scale study (Gómez-Boix et al., WWW 2018) — and that was _with_ canvas/WebGL/audio; 2026 vendor
masking (Chrome frozen Android UA "Android 10; K", Safari frozen iOS UA OS token, Brave collapsed
vendor/renderer, Firefox FPP clamps) shrinks the space further. Corporate VDI and cloned school images are
the desktop version (13% of Chromium-desktop identifications are VMs — FingerprintJS 2026).
**Fix.** Token is the device identity; components are evidence only. Drop `UNIQUE(device_key)`; key the table
on a token hash and store the component key as a non-unique attribute. **Delete `BanUnit::Device`.** Add a
per-key population audit with a hard non-actionable threshold; require a server-side signal for any punitive
action. Reuse `device_v1`'s existing semantics (token UNIQUE, ownership transfers, per-user cap) instead of
inventing a second device concept.

### F2 — Client-chosen key ⇒ attacker-chosen ban unit (cohort poisoning and targeted framing)

**Severity: Critical**
**Mechanism.** Components come from JS, so an attacker _selects_ their key: spoof the modal Android config of
a target country, or a specific victim's known components, then generate violations under that key. Genuine
users of the bucket are then challenged/held (and, with F1, banned). "Challenge freely" makes cohort-burn
cheap and indistinguishable from a bug to the victims. This is the design's device axis being _weaponizable_,
not merely weak.
**Fix.** Punitive evidence must be server-derived (credential, content, network-with-account-history).
Device evidence never actionable alone. Per-key action budget + challenge-storm circuit breaker. Treat
high-population keys as non-actionable. Add a "device-key abuse" case type to the review queue.

### F3 — Shared-device families, madrasas and kiosks: one person's ban propagates to every co-user

**Severity: High**
**Mechanism.** Ladder rows 5–8 key off "parent match"; on a shared device every co-user has the parent match,
and the corroborators (network, locale, timing, reading trail) are inherited. Base rates: India 30% of adults
do not own a mobile phone; a median of 48% of non-owners (India 56%) rely on a family member's phone (Pew
2019); rural shared-device users grew 24% in a year, ~80% are rural, and 76% are under 19, 58% female, 62%
NCCS CDE (IAMAI/Kantar _Internet in India 2024_). The modal user of this app _is_ the co-user of a shared
handset. DESIGN-v0 has no kiosk registration and no `ip_legit_share` suppression.
**Fix.** Account-scoped enforcement; device-triggered actions capped at L2; kiosk/org registration from day
one; self-service shared-device claim that clears device reputation but never an account ban; annotate the
token `shared` after a claim; step down one level for any action on a registered shared device.

### F4 — The corroboration rule is unsafe or unsatisfiable: nothing observable on a shared device separates evader from co-user

**Severity: High**
**Mechanism.** The inheritance lemma (Part 3): device, network, locale, screen, behaviour, reading pattern and
content topic are all inherited by a legitimate co-user. Only credential/identity reuse and _identical
violating content_ survive — and a malicious co-user can forge both (register with the family email;
reproduce the payload to frame someone). So `D ∧ P ∧ C` is either satisfied by innocent people (unsafe) or
unsatisfiable for a cautious design (useless).
**Fix.** Encode the admissible corroborator list; make network/device/behaviour/locale/reading-trail
_inadmissible_. Require `P` ∧ {credential reuse ∨ identical violating content ∨ declared link} ∧ the
functional test ∧ human review before L5+; never auto-L6; freeze evidence at proposal time.

### F5 — Storage-clearing/ITP/eviction users: unknown device is normal, and the design turns it into permanent friction plus churn-as-risk

**Severity: High**
**Mechanism.** Safari ITP deletes all script-writable storage after 7 days without interaction (WebKit);
Chrome/Firefox evict origin storage under storage pressure, LRU first (MDN); Li & Cao measured 32% of browser
instances clearing cookies in 8 months. The engine then sees "new device at login" repeatedly, a per-user
device fan-out (indexed on `user_id` — a classic takeover feature), and a `trust_state` that resets so trust
never accrues. The alternative (component key) silently re-identifies the user after an explicit clear —
defeating clear-site-data on a religious app.
**Fix.** Account-level trust; device churn is never a risk feature; unknown = zero friction; no
re-identification after an explicit clear; single token storage (mismatch = unknown); make unknown-device
share an SLO, not a suspicion metric.

### F6 — Coherence booleans false-positive on legitimate vendor-masked and absent-signal configurations

**Severity: High**
**Mechanism.** Absence is not inconsistency and masking is not spoofing. The sharpest case: Chrome's reduced
UA says `Android 10; K` for every Android Chrome user and never updates, while UA-CH returns the real
version/model — a naive `UA↔UA-CH` check flags 90.62% of Indian mobile traffic (StatCounter, Sept 2026).
`navigator.platform` is reduced to fixed per-OS strings (ChromeOS and Linux both `Linux x86_64`, Android
`Linux armv81`). UA-CH is absent on Safari, Firefox, Android WebView and Chrome iOS. GPU renderer is masked
by Brave/Safari/Firefox FPP/Tor/Lockdown. iPad desktop mode is `MacIntel` + 5 touch points. Foldables,
rotation and split-screen change screen vs inner. The FP burden lands on the non-Chrome minority — i.e. on
the privacy-conscious and censorship-country users.
**Fix.** The Part 2.2 contract: `{value, masked, unavailable, spoofed}`; directional checks only; per-family
support matrix + known-masked allowlist; coherence excluded from ban thresholds; release-day damping; bias
dashboard by browser family.

### F7 — Version-gated feature-matrix checks create a release-day false-positive wave

**Severity: Medium**
**Mechanism.** Fingerprint dynamics cluster on browser/OS release dates (Li & Cao, IMC 2020), so "claimed
version vs API presence" is wrong on release day, on origin-trial/flag-enabled APIs, on
enterprise-policy-disabled features, on Firefox ESR, and on Chrome's frozen UA. It also requires a
permanently maintained browser-feature database — an FP generator for a small team.
**Fix.** Only downward consistency; prefer UA-CH `fullVersionList`; unknown/absent = pass; damp thresholds
for N days after major releases; shadow-mode + rule replay with a precision gate before enabling.

### F8 — "Challenge freely" is not cheap for this audience, and a failed challenge can lock users out

**Severity: High**
**Mechanism.** The CSP allowlists `challenges.cloudflare.com`; the widget is blocked/throttled on
censored/filtered networks and cannot complete in Tor Browser, so for exactly the readers R2 names ("behind
national filters… Tor users in jurisdictions where reading is risky") a challenge is a hard block. On
low-end Android with metered data it costs bytes and seconds; on a shared phone whoever holds the phone
solves it; low-literacy users and minors without email have no recovery path. A step-up on "new device at
login" that fails means the user cannot read _and_ cannot reach the appeal the design promises must work
while suspended.
**Fix.** Never gate login, reading, or recovery on a device signal; scope challenges to writes; provide a
non-CAPTCHA fallback that does not depend on the challenge host (email OTP / read-only-until-verified);
make the notice/appeal path independent of the challenge; budget challenges per account; measure challenge
abandonment by country and device class.

### F9 — The ladder's behavioural-similarity rung is behavioural biometrics and contradicts Layer 0

**Severity: Medium**
**Mechanism.** Row 5 makes "device match + behavioural similarity (timing/navigation)" punitive, while
Layer 0 promises "No behavioral biometrics (GDPR Art 9; 80% F1; false positives)". R2-fingerprint-science
§6.3 says behaviour must never be a hard gate (accessibility, assistive tech, IME, AI agents), and on a
shared device behaviour is inherited.
**Fix.** Remove behaviour from the punitive path or restate the policy explicitly; if kept, restrict it to
server-side aggregate features, exclude a11y-sensitive input cadence, never above L3, never a sole
corroborator.

### F10 — Proxy-rendered browsers collapse many humans into one fingerprint; text/LLM clients have no fingerprint at all

**Severity: Medium-High**
**Mechanism.** Opera Mini executes JavaScript on Opera's servers (Opera help: "Opera Mini and JavaScript";
Wikimedia proxy-browser guide) → every user of a version presents the _proxy's_ environment, so the whole
population shares one `device_key` (mass false merge + mass false challenge), and its ancient-looking UA
trips version checks. Opera is 3.51% of Indian mobile traffic (StatCounter, Sept 2026) and higher in parts of
Africa. Separately, the repo deliberately serves `llms.txt`, `llms-full.txt`, `.md` and `.txt` routes —
text/LLM consumers have no canvas/WebGL/UA-CH at all, and the science report says AI agents doing legitimate
work are a real class.
**Fix.** Detect and classify proxy-rendered clients (`Opera Mini`, `X-OperaMini-Phone-UA`, data-saver modes)
and exclude them from the device axis entirely; exempt declared read-only text/LLM/crawler clients; scope
device rules to authenticated write paths.

### F11 — In-app browsers, PWA standalone and iOS home-screen apps give one human several device identities on the dominant link-open path

**Severity: High**
**Mechanism.** Android WebView is excluded from UA reduction and UA-CH (Chromium doc) and has separate storage
from Chrome, so opening a WhatsApp-shared Quran link creates a new, UA-CH-less device identity; iOS
home-screen web apps are storage-isolated from Safari (WebKit bug 181849) so the same reader is two devices;
PWA standalone changes viewport relationships. ~31% of mobile web sessions are in-app-browser sessions
(MobiLoud 2026 — secondary, flagged). Every context switch = "new device"; and if one context is ever
flagged, the human's other contexts look like evasion.
**Fix.** Record context (browser | webview | standalone) as an attribute, never as a device identity; never
step-up on a context change; make account continuity the join; never count contexts as devices in scoring.

### F12 — Per-request device writes on a 1-connection pool plus fail-closed guards create systemic false positives, and the coherence logic cannot be tested in this repo's stack

**Severity: Medium**
**Mechanism.** The collector attaches to every `/api/**` call while `pool = 1` and guards fail-closed; a
device write on the request path serialises traffic, and when it fails or times out, fail-closed turns a
database problem into challenges/denials for everyone. Second: `bun run test` uses happy-dom, which has no
canvas/WebGL/audio/UA-CH/IDB, so the collector and its booleans cannot be exercised by the existing gates —
the rules first run against real users. Third: `device_v1` already exists with different semantics
(push-token registry, per-user cap 20, ownership transfer = shared devices are explicitly non-suspicious).
**Fix.** Async/batched evidence writes off the request path; unknown ⇒ fail-open for reads and for the
collector (fail-closed only for ban enforcement); shadow-mode logging + rule-replay harness + precision gate

- kill switch; unit-test coherence as pure functions with golden vectors; reconcile with `device_v1` rather
  than creating a second device identity.

---

## Sources

Repo (verified in-tree)

- `rust/backend/api/src/modules/device_v1/controller.rs` (MAX_DEVICES_PER_USER=20, ownership-transfer test), `validator.rs`
- `rust/backend/api/src/modules/admin_bans_v1/dto.rs` + `controller.rs` tests (BanUnit = IpNet; non-CIDR rejected)
- `rust/backend/api/src/services/escalation.rs` (default-OFF, IP-only, allowlist, bounded, never permanent)
- `rust/backend/api/src/modules/{post_comment_v1,media_v1,bookmark_v1,user_v1,notification_v1}/mod.rs`
- `web/src/routes/{llms.txt,llms-full.txt,[slug].txt}`; design doc line 13 (happy-dom test limits); `AGENTS.MD`

Base rates and mechanisms

- Pew Research Center, _Mobile Divides in Emerging Economies_ (Nov 2019): https://www.pewresearch.org/global/2019/11/20/mobile-divides-in-emerging-economies/ (India 30% non-owners; median 7% borrow; median 48% of non-owners cite a family member's phone, India 56%, Philippines 59%; India women 20% vs men 5% borrowing)
- Pew Research Center, _Mobile Connectivity in Emerging Economies_ (Mar 2019): https://www.pewresearch.org/global/2019/03/07/mobile-connectivity-in-emerging-economies-2/
- IAMAI & Kantar, _Internet in India 2024_: https://www.iamai.in/sites/default/files/research/Kantar_%20IAMAI%20report_2024_.pdf (rural shared-device users +24%; rural shared-device profile 62% NCCS CDE / 58% female / 76% under 19); The Hindu summary (≈80% of shared-device users rural): https://www.thehindu.com/sci-tech/technology/india-now-has-958-million-active-internet-users-57-of-these-are-from-rural-areas/article70566646.ece
- Gómez-Boix, Laperdrix, Baudry, _Hiding in the Crowd_, WWW 2018 (mobile 18.5% unique): https://dl.acm.org/doi/abs/10.1145/3178876.3186097
- Vastel et al., _FP-Stalker_, USENIX Security 2018 (45.52% change in 1 day): https://www.usenix.org/conference/usenixsecurity18/presentation/vastel
- Li & Cao, _Who Touched My Browser Fingerprint?_, IMC 2020 (32% cleared cookies; release-date clustering): https://dl.acm.org/doi/10.1145/3419394.3423614
- FingerprintJS, _Device Intelligence Report 2026_ (VMs 13% Chromium desktop; VPN ~1 in 5): https://fingerprint.com/blog/device-intelligence-report-2026/
- Tiwari et al., _Fingerprinting the Android WebView_, 2022: https://arxiv.org/abs/2208.01968
- Chromium, _User-Agent Reduction_ (reduced `navigator.platform` values; frozen Android "10; K"; no reduction on iOS/Android WebView): https://www.chromium.org/updates/ua-reduction/
- WebKit, _Tracking Prevention_ (7-day deletion of all script-writable storage): https://webkit.org/tracking-prevention/ ; _Full Third-Party Cookie Blocking and More_: https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/
- MDN, _Storage quotas and eviction criteria_ (eviction under storage pressure): https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria
- WebKit bug 181849, "Add to homescreen apps don't share storage with Safari": https://bugs.webkit.org/show_bug.cgi?id=181849
- Opera, _Opera Mini and JavaScript_ (proxy executes JS): https://help.opera.com/en/opera-mini-and-javascript/ ; Wikimedia, _Proxy browsers_: https://wikitech-static.wikimedia.org/wiki/Performance/Guides/Proxy_browsers.html
- StatCounter, _Mobile Browser Market Share India_, Sept 2026 (Chrome 90.62%, Safari 3.61%, Opera 3.51%, UC 0.97%, Samsung Internet 0.79%, Firefox 0.27%): https://gs.statcounter.com/browser-market-share/mobile/india
- MobiLoud, mobile traffic update 2026 (~31% of mobile web sessions in in-app browsers) — secondary, indicative: https://www.mobiloud.com/blog/what-percentage-of-internet-traffic-is-mobile/
- Cybernews VPN adoption index (country-level VPN adoption, indicative): https://cybernews.com/best-vpn/vpn-usage-by-country/
- RFC 6888 (CGNAT): https://www.rfc-editor.org/rfc/rfc6888.html ; RFC 8981 (IPv6 temporary addresses): https://www.rfc-editor.org/rfc/rfc8981.html

Flagged as unverified in this session

- KaiOS/JioPhone installed base; per-model Android SKU counts; a published rate for kiosk/cybercafé device reuse;
  the in-app-browser share (secondary source only). Treat as directional, not citable.
