# R2 — Browser Fingerprinting Signal Science

**Measured entropy, stability, reliability of every browser-side signal, and what vendors are doing to fingerprints in 2026.**

Compiled: October 2026. Research agent output. All numbers are cited to their source; provenance and confidence are flagged inline.

---

## 0. How to read this document

**Entropy convention.** All figures are Shannon entropy in bits, `H(X) = -Σ p log₂ p`, computed over the _observed population_ of a study. This is the standard convention in the field (Eckersley 2010; Laperdrix et al. 2019 survey). Two caveats that invalidate naive use:

1. **Entropy is population-dependent.** A value with 8 bits of entropy in AmIUnique's self-selected privacy-aware sample may carry 5 bits on a mainstream commercial site. The 2018 "Hiding in the Crowd" study is the canonical demonstration: same attribute family, half the entropy, because the audience was a general-audience French commercial site rather than a privacy-testing site.
2. **Per-signal entropy sums are wrong.** Correlated signals have joint entropy far below the sum of marginals. Google's 2024 study (Bacis et al., WWW 2024) built the whole method around this: they bound joint entropy with the Chow–Liu decomposition `H(X_S) ≤ Σ H(X_s) − Σ I(X_s; X_s′)`, subtracting pairwise mutual information. Language and keyboard layout are their worked example of a pair whose joint entropy is much lower than the sum.

**Confidence legend used in the table:**

- **[P]** = primary measurement published by the researchers who ran the study.
- **[V]** = vendor claim (FingerprintJS, Brave, Mozilla, Google, Apple). Directionally useful, not peer-reviewed.
- **[S]** = secondary source (blog, aggregator, AI-generated explainer). Treat the number as indicative only; do not cite downstream.
- **[—]** = no credible published measurement found. Flagged as a research gap.

**A standing warning about the 2024–2026 literature.** A large fraction of search results on this topic are now SEO/AI-generated pages that recycle plausible-looking entropy numbers with no measurement behind them (e.g. "audio entropy exceeds 17 bits per sample", "WebGL entropy 14–16 bits", "canvas fingerprints cover 47,000+ device configurations"). Where I could not trace a number to a real study, it is marked **[—]** or **[S]** rather than presented as fact.

---

## 1. Signal table

| Signal                                                                                                                      | Entropy (bits)                                                                                                           | Stability                                             | Attacker-spoofable?                     | Client-side collectable?                                                   | Notes / source                                                                                                                                                                                                                                                                         |
| --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- | --------------------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **UA string (legacy)**                                                                                                      | 10.00 [P] 2010 · 9.78 [P] 2016 · 7.15 [P] 2018                                                                           | Med (changes every browser/OS release)                | Yes, trivially                          | Yes + HTTP header                                                          | Eckersley 2010; Laperdrix 2016; Gómez-Boix 2018. Reduced hard in Chrome (110+, GA ~113, Apr 2023) and frozen on iOS in Safari 26.                                                                                                                                                      |
| **UA-Client-Hints (low entropy)**                                                                                           | ~1–2 [—]                                                                                                                 | High                                                  | Yes                                     | Yes + header                                                               | `Sec-CH-UA`, `-Mobile`, `-Platform` sent by default. Coarse by design.                                                                                                                                                                                                                 |
| **UA-CH high entropy** (`getHighEntropyValues`: arch, bitness, model, platformVersion, fullVersionList, wow64, formFactors) | **[—]** no published measurement                                                                                         | High for a device; changes on OS/browser update       | Yes                                     | Yes (permission-gated via `Permissions-Policy: ch-ua-high-entropy-values`) | MDN. Full version string alone re-creates most of the old UA entropy. Underexplored in the literature — a real gap.                                                                                                                                                                    |
| **Canvas 2D**                                                                                                               | 8.28 [P] 2016 · 8.55 [P] 2018                                                                                            | Med-Low (see §3)                                      | Yes                                     | Yes                                                                        | AmIUnique 2016 / Hiding in the Crowd 2018. Mowery & Shacham 2012 saw only 50 distinct renders from 300 samples (largest cluster 172) — entropy is real but the _rendering_ signal is coarser than the hash suggests; the hash's uniqueness comes from hashing a large noisy string.    |
| **WebGL vendor**                                                                                                            | 2.14 [P] 2016 · 2.28 [P] 2018                                                                                            | High                                                  | Yes                                     | Yes                                                                        | `WEBGL_debug_renderer_info` / `gl.getParameter`.                                                                                                                                                                                                                                       |
| **WebGL renderer**                                                                                                          | 3.41 [P] 2016 · 5.54 [P] 2018                                                                                            | High                                                  | Yes                                     | Yes                                                                        | e.g. `ANGLE (Apple, ANGLE Metal Renderer: Apple M5 Max)`. Brave began scrubbing vendor+renderer in v1.93 (Aug 2026).                                                                                                                                                                   |
| **WebGL parameters** (`getParameter`, 30+ pnames)                                                                           | **[—]** individually; clustered with "core device properties" [P]                                                        | High                                                  | Yes                                     | Yes                                                                        | Google WWW 2024 found WebGL parameters form their own correlated cluster with core device properties.                                                                                                                                                                                  |
| **WebGL extensions list**                                                                                                   | **[—]**                                                                                                                  | Med (driver updates)                                  | Yes                                     | Yes                                                                        | Varies by GPU+driver; hashed into a compact ID. Brave now injects per-session/per-eTLD+1 noise into it (Aug 2026).                                                                                                                                                                     |
| **WebGL shader precision** (`getShaderPrecisionFormat`)                                                                     | **[—]**                                                                                                                  | High                                                  | Yes                                     | Yes                                                                        | Cheap, tiny output, high agreement with GPU family. Widely used but never separately measured.                                                                                                                                                                                         |
| **WebGL 3D rendering tasks**                                                                                                | >99% of 1,903 devices uniquely identified ⇒ ≈10.9 bits on that sample [P]                                                | High                                                  | Yes                                     | Yes                                                                        | Cao et al. 2017, 31 rendering tasks; also cross-browser stable (OS/hardware layer).                                                                                                                                                                                                    |
| **AudioContext oscillator + compressor (offline render)**                                                                   | 5.4 [P] on 18,500 devices / 713 distinct values                                                                          | **High**                                              | Yes                                     | Yes                                                                        | DarkwaveTechnologies measurement. FingerprintJS' own assessment: audio "contributes only slightly to uniqueness but is highly stable, resulting in a slight net increase in fingerprint accuracy."                                                                                     |
| **AudioContext hardware params** (`sampleRate`, `maxChannelCount`, `baseLatency`)                                           | ~2–3 [—]                                                                                                                 | High                                                  | Yes                                     | Yes                                                                        | Coarse but stable; Safari 26 normalizes `sampleRate` to 48,000 for known fingerprinting scripts [V].                                                                                                                                                                                   |
| **Font list (enumeration)**                                                                                                 | 13.90 [P] 2010 · 8.38 [P] 2016 · 6.90 [P] 2018                                                                           | **High** (changes only on install/uninstall)          | Yes                                     | Yes                                                                        | The classic high-entropy signal. Firefox FPP now blocks local fonts except a language-keyed allowlist [V].                                                                                                                                                                             |
| **Font metrics / glyph bounding boxes**                                                                                     | 43 characters uniquely identify 34% of 1,000+ browsers ⇒ ≈4–5 bits [P]                                                   | High                                                  | Yes                                     | Yes                                                                        | Fifield & Egelman 2015, 125,000 glyphs tested. Metrics survive font-list blocking because they use _system_ fonts.                                                                                                                                                                     |
| **`document.fonts.check()`**                                                                                                | **[—]**                                                                                                                  | High                                                  | Yes                                     | Yes                                                                        | Subset-probing variant of font enumeration; no published entropy. Also the mechanism used by font-detection libraries.                                                                                                                                                                 |
| **Screen resolution + color depth**                                                                                         | 4.83 [P] 2010 · 4.89 [P] 2016 · 4.85 [P] 2018                                                                            | Low-Med (window resize, zoom, external monitor, dock) | Yes                                     | Yes                                                                        | Remarkably stable entropy across 8 years. Firefox FPP reports `height − 48px`; Safari AFP fixes `screen.width/height` to `innerWidth/innerHeight`.                                                                                                                                     |
| **`devicePixelRatio`**                                                                                                      | ~1–2 [—]                                                                                                                 | Low (zoom, monitor change)                            | Yes                                     | Yes                                                                        | Correlated with OS + screen; counted inside Google's "core device properties" cluster.                                                                                                                                                                                                 |
| **Viewport / `innerWidth` / `outerWidth` / `screenX/Y`**                                                                    | ~2–4 [—]                                                                                                                 | **Low** (resize, maximize, DevTools)                  | Yes                                     | Yes                                                                        | Deliberately excluded from most serious fingerprints because it is so unstable. Safari AFP zeroes `screenX/screenY`.                                                                                                                                                                   |
| **Timezone**                                                                                                                | 3.04 [P] 2010 · 3.34 [P] 2016 · 0.16 [P] 2018                                                                            | High (only changes on travel/DST config)              | Yes                                     | Yes                                                                        | The 2016→2018 collapse (3.34 → 0.16) is a _population_ effect, not a technical one: a general-audience site has one dominant timezone. Timezone is also the strongest IP-consistency check (§5).                                                                                       |
| **`Intl` locale / calendar / numbering / `Intl.DateTimeFormat().resolvedOptions()`**                                        | ~2–5 combined [—]; content-language header 5.92 [P] 2016                                                                 | High                                                  | Yes                                     | Yes                                                                        | Rarely separately measured. `Intl` is one of the five language signals DataDome cross-checks for coherence [V].                                                                                                                                                                        |
| **`navigator.languages` / `language` / `Accept-Language`**                                                                  | Accept 1.38 [P] 2016 · 0.73 [P] 2018                                                                                     | High                                                  | Yes                                     | Yes + header                                                               | Low standalone entropy but a powerful _consistency_ signal (§5).                                                                                                                                                                                                                       |
| **`navigator.platform`**                                                                                                    | 2.31 [P] 2016 · 1.20 [P] 2018                                                                                            | High                                                  | Yes                                     | Yes                                                                        | Chrome reduced it with UA reduction.                                                                                                                                                                                                                                                   |
| **`navigator.hardwareConcurrency`**                                                                                         | **[—]** ≈2–3 est.                                                                                                        | High                                                  | Yes                                     | Yes                                                                        | Chrome clamps to a small set; Firefox FPP reports a constant **2** [V]. Very coarse but a good coherence check against UA/device class.                                                                                                                                                |
| **`navigator.deviceMemory`**                                                                                                | ≤3 (bucketed to {0.25, 0.5, 1, 2, 4, 8}) [—]                                                                             | High                                                  | Yes                                     | Yes                                                                        | Chrome-only. FP-Radar detected its abuse in the wild _before_ it was publicly disclosed as a fingerprinting vector.                                                                                                                                                                    |
| **`navigator.maxTouchPoints`**                                                                                              | ~1–2 [—]                                                                                                                 | High                                                  | Yes                                     | Yes                                                                        | Firefox FPP now reports only 0, 1, or 5 [V]. Key desktop-vs-mobile coherence check.                                                                                                                                                                                                    |
| **`navigator.plugins` / `mimeTypes`**                                                                                       | 15.40 [P] 2010 · 11.06 [P] 2016 · 9.49 [P] 2018                                                                          | **High** but **degenerate in 2026**                   | Yes                                     | Yes                                                                        | Was the single highest-entropy signal in 2010. Now a fixed 5-entry stub in Chrome/Firefox — entropy has collapsed in practice even though the 2018 study still measured 9.49 (its population included legacy browsers). A textbook example of why you cannot cite old entropy numbers. |
| **Media codec support (`canPlayType`, `MediaCapabilities`, `isTypeSupported`)**                                             | **[—]**; Google used _platform_ entropy as a proxy because codec surfaces were "highly correlated with the platform" [P] | High                                                  | Yes                                     | Yes                                                                        | Proprietary-codec presence (H.264/AAC, Widevine) is a strong Chrome-vs-Chromium-vs-Firefox discriminator and a classic headless tell.                                                                                                                                                  |
| **WebRTC ICE candidates**                                                                                                   | **[—]**                                                                                                                  | Med                                                   | Yes                                     | Yes                                                                        | Since RFC 8828 + mDNS ICE candidates, private host IPs are replaced by `<uuid>.local`. Chrome/Firefox/Edge still expose a default-route host candidate; Safari exposes none by default [P, IETF]. SRFLX still reveals the _public_ IP to the page.                                     |
| **Storage `estimate()` / quota**                                                                                            | ~2–4 [—]                                                                                                                 | Med-High                                              | Yes                                     | Yes                                                                        | Quota ≈ a fraction of free disk; a coarse disk-size bucket. Also used as a "is this a fresh profile" tell.                                                                                                                                                                             |
| **Battery Status API**                                                                                                      | **[—]** (was used as a short-term identifier)                                                                            | **Low** (discharge slope is a timer)                  | Yes                                     | Mostly **removed**                                                         | Firefox removed it (Fx 52, 2017); Safari removed it; Chrome still ships a coarsened version. Olejnik et al. 2016/2017 documented the whole history.                                                                                                                                    |
| **Sensors (motion, orientation, ambient light, magnetometer)**                                                              | **[—]**                                                                                                                  | Med                                                   | Yes                                     | Permission-gated on iOS; available on Android                              | 3,695 of top-100K sites used sensor fingerprinting by 2018 (Das et al.). FP-Radar later found _undisclosed_ abuse of `illuminance`, `Magnetometer`, `rotationRate`.                                                                                                                    |
| **WebAssembly features / `wasm` timing**                                                                                    | Guri & Fibert 2025: distinguishes Chrome vs Edge with UA fully spoofed, **FPR <1%** [P]                                  | High                                                  | Hard to spoof (native code paths)       | Yes                                                                        | Compile-time and CPU-bound timing across Intel/AMD/ARM, Windows/macOS/Android/iOS, VMware/KVM/VirtualBox. One of the most spoof-resistant signals known.                                                                                                                               |
| **Math / JS engine quirks (trig precision, `Math.fround`, `expm1`, `sinh`…)**                                               | **[—]**                                                                                                                  | High                                                  | Yes (but must match the claimed engine) | Yes                                                                        | Mozilla explicitly lists "tiny differences in how it performs math" among the vectors it patches [V]. Engine-level, so it separates V8/JSC/SpiderMonkey and sometimes versions.                                                                                                        |
| **Speech synthesis voices (`getVoices()`)**                                                                                 | **[—]**; on Windows SAPI/OneCore can be 10–40+ voices                                                                    | High (changes on language-pack install)               | Yes                                     | Yes                                                                        | Safari 26 blocks voice enumeration _for known fingerprinting scripts_ [V]. Rarely measured academically — a gap.                                                                                                                                                                       |
| **`navigator.connection` (Network Information)**                                                                            | **[—]**; `downlink` quantized to 25 kbps, `rtt` to 25 ms                                                                 | **Low-Med** (moves with network conditions)           | Yes                                     | Chrome-only                                                                | FP-Radar found `rtt`/`downlink` abused in the wild 2 years _before_ public disclosure.                                                                                                                                                                                                 |
| **`performance` timing resolution**                                                                                         | 0 bits by itself; enables timing side-channels                                                                           | High (it's a browser constant)                        | Partly                                  | Yes                                                                        | `performance.now()` coarsened to **100 µs** normally, **5 µs** when cross-origin isolated [P, MDN]. Firefox RFP clamps further.                                                                                                                                                        |
| **WebGPU adapter info**                                                                                                     | **[—]** published in-text; Brave cites `{vendor:'apple', architecture:'metal-3'}`-level detail                           | **High**                                              | Yes                                     | Yes                                                                        | W3C removed `requestAdapterInfo()` from the spec; `GPUAdapter.info` remains. Brave empties adapter descriptors (v1.93, Aug 2026). ACM 2025 paper "Unveiling Privacy Risks in WebGPU through Hardware-based Device Fingerprinting" exists but I could not verify any bit figure.        |
| **Error stack traces**                                                                                                      | **[—]**                                                                                                                  | High                                                  | Yes                                     | Yes                                                                        | Engine-specific format/columns. Doubles as a spoofing detector: the _stack itself_ leaks whether native functions were replaced (§5).                                                                                                                                                  |
| **`Intl.DateTimeFormat` resolver / `Intl.RelativeTimeFormat`**                                                              | **[—]**                                                                                                                  | High                                                  | Yes                                     | Yes                                                                        | One of DataDome's five cross-checked language signals [V].                                                                                                                                                                                                                             |
| **CSS feature support (`CSS.supports`)**                                                                                    | **[—]**                                                                                                                  | Med (browser-version coupled)                         | Yes                                     | Yes                                                                        | Cheap, hundreds of probes. Historically used to identify browser family/version (Unger et al. 2013).                                                                                                                                                                                   |
| **Key event codes / `KeyboardEvent.code` + `getLayoutMap()`**                                                               | **[—]**                                                                                                                  | High                                                  | Yes                                     | Yes                                                                        | Keyboard layout is _highly mutually informative_ with language — Google's WWW 2024 study named this pair explicitly as an example where joint entropy ≪ sum of marginals.                                                                                                              |
| **Ad blocker / extension presence**                                                                                         | 1.00 [P] 2016 · 0.05 [P] 2018                                                                                            | Med                                                   | Partly                                  | Yes                                                                        | Extension detection is itself a fingerprinting surface (Sjösten 2017; Starov 2017; Gulyás 2018: 39.29% of Chrome users unique from extensions alone; 485 well-chosen probes suffice).                                                                                                  |
| **TLS/JA3/JA4 (server-side)**                                                                                               | **[—]**                                                                                                                  | High                                                  | Hard-ish                                | No (server)                                                                | Out of scope for client-side collection but the strongest non-JS cross-check.                                                                                                                                                                                                          |
| **HTTP header set / `Accept` / encoding**                                                                                   | 1.38 / 1.53 [P] 2016 · 0.73 / 0.38 [P] 2018                                                                              | High                                                  | Yes                                     | Server                                                                     | `Accept`, `Accept-Encoding`, `Sec-Fetch-*` ordering is a real discriminator.                                                                                                                                                                                                           |

**Rough total.** Panopticlick's worst case was `H_M = 18.84` bits for its 2010 attribute set; AmIUnique 2016 = 16.86; Hiding in the Crowd 2018 = 20.98. Google's 2024 session-level measurement is the honest modern figure and it is _not_ a single number: the distribution is bimodal, with nearly every site's session entropy either well below or well above **20 bits**, and a mean per-surface entropy on the order of **0.5 bits** (from an internal Google pilot; used to size their sampling budget).

---

## 2. Combined-hash entropy and real-world accuracy

### 2.1 Uniqueness in the three canonical large studies

| Study                                        | Population                                         | N                                                           | % unique                                                    | Notes                                                                                                                     |
| -------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Panopticlick (Eckersley 2010)                | Self-selected test site, 2 weeks                   | 470,161                                                     | **83.6%** (94.2% with Flash/Java)                           | Top attributes: plugins 15.4, fonts 13.9, UA 10.0 bits.                                                                   |
| AmIUnique (Laperdrix et al. 2016)            | Self-selected, 1 year                              | 118,934                                                     | **89.4%** desktop, **81%** mobile                           | Canvas/WebGL replaced plugins.                                                                                            |
| Hiding in the Crowd (Gómez-Boix et al. 2018) | **One top-15 French commercial site**              | 2,067,942                                                   | **33.6%** overall; 35.7% desktop, **18.5%** mobile          | The single most important methodological result in the field: changing the audience halves-to-thirds the uniqueness rate. |
| Guess Who? (Andriamilanto et al. 2021)       | Same French site, 6 months, 216 attributes         | ~4M FPs / ~2M visitors                                      | **81%** unique; >90% of attributes unchanged after 6 months | Larger attribute set restores high uniqueness.                                                                            |
| Google (Bacis et al., WWW 2024)              | Real Chrome clients, in-the-wild, passive sampling | **>10M clients, >100K sites, 5,383 surfaces from 161 APIs** | not a uniqueness % — reports entropy distributions          | Session entropy bimodal at ~20 bits. Estimates accurate to **±1.5 bits @ 90% confidence**.                                |

### 2.2 Why ">99% accuracy" claims are not the same as ">99% entropy"

Commercial vendors (FingerprintJS, and the anti-fraud vendors in the Laperdrix 2019 survey: ThreatMetrix, Distil, MaxMind, PerimeterX, Sift, Iovation) advertise >99% identification. Read these as **match-accuracy on their own traffic under their own linking algorithm**, not as "the fingerprint is unique." Three things inflate such numbers:

1. **Fuzzy matching over time.** FingerprintJS explicitly states its identifiers are "stable for months or even years, even after browser updates, when the user uses a VPN, or is in incognito mode" and that "accuracy improves when fingerprints evolve over time instead of relying on static, one-time measurements." That is a _re-identification algorithm_, not a static hash.
2. **Server-side signals.** FingerprintJS Pro combines "over 100 individual signals" with "server-side analysis and machine learning" — i.e. TLS, headers, IP, reputation.
3. **Traffic mix.** Their own 2026 Device Intelligence Report (23 billion identification events, >7 billion browsers/devices) shows 67% of traffic scoring in the lowest risk bucket and 85% below Suspect Score 10 — most traffic is _easy_.

Independent measurement of a _static_ hash is far less flattering. Li & Cao (IMC 2020) measured the state-of-the-art evolution-aware tool (FP-Stalker) on 7.25M fingerprints from 1.33M browser instances and found:

- FP-Stalker rule-based **F1 drops from 86.1% → 75.9%** (top-10 candidates) as the database grows from 100K to 1M fingerprints.
- Matching latency grows from **~100 ms → ~1 s**.
- The learning-based variant **cannot scale past ~300K** fingerprints.
- Their own Browser ID (username hash + stable features) achieved **0.1% false positive / 0.3% false negative** — and note that it is _not_ a pure fingerprint, it is anchored to a login.

### 2.3 Stability over time — the numbers

**FP-Stalker (Vastel et al., USENIX Security 2018)** — daily fingerprints from volunteers:

- **45.52%** of fingerprints had changed after **one day**.
- Average trackable duration with a fingerprint every 3 days: **51.8 days**.
- **26%** of devices trackable for **>100 days**.
- Three evolution classes identified: automatic (software updates), context-dependent (environment), user-triggered (preference changes).

**Li & Cao (IMC 2020)** — 960,853 observed dynamics, classified into exactly three causes:

1. **browser or OS updates**
2. **user actions**
3. **system environment updates**

Four findings that matter for a security-only fingerprint:

- **Dynamics are correlated even when features are not.** Chrome's audio sample rate changes together with the GPU renderer, because a DirectX update moves both. So a fingerprint that watches _deltas_ sees structure that a fingerprint watching _values_ does not.
- **Update timing is correlated with real-world release events** — fingerprint changes cluster around browser/OS release dates. This is exploitable in both directions (predict churn; detect spoofers who don't churn).
- **Font list changes leak installed software** (e.g. a Microsoft Office install or update).
- **Cookie-based ground truth is broken:** **32%** of browser instances cleared cookies during the study, largely due to Intelligent Tracking Prevention. Any "99% accuracy" number validated against cookies inherits this error.

### 2.4 What actually changes a canvas/WebGL hash

Synthesizing FP-Stalker, Li & Cao, and vendor reports, the concrete triggers are:

| Trigger                                                      | Direction                                                                                | Magnitude                              |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------------- | -------------------------------------- |
| GPU driver update                                            | canvas + WebGL renderer + extensions                                                     | high (often total hash change)         |
| OS major/minor update                                        | canvas, fonts, audio                                                                     | high                                   |
| Browser major update                                         | audio, canvas, WebGL params                                                              | med-high                               |
| Hardware acceleration on/off (VM, remote desktop, safe mode) | canvas, WebGL                                                                            | **total** — software vs GPU rasterizer |
| Zoom / `devicePixelRatio` change                             | canvas (if drawn at non-integer DPR), screen metrics                                     | med                                    |
| External monitor attach/detach                               | screen metrics, DPR                                                                      | med                                    |
| Dark mode / theme change                                     | low (does not affect canvas pixels unless the script is theme-aware)                     | low                                    |
| VPN toggle                                                   | **0 for browser signals**; changes IP, and therefore breaks IP↔timezone/locale coherence | n/a (but see §5)                       |
| Font install/uninstall                                       | font list, glyph metrics                                                                 | med                                    |
| Language pack install                                        | `getVoices()`, `Intl`                                                                    | med                                    |
| Browser version bump                                         | UA/UA-CH, audio, canvas                                                                  | med                                    |

**VPN toggling does not perturb browser-side signals at all.** That is precisely why VPN usage exploded to ~1 in 5 identification events in 2025 (1 in 3 on Chromium desktop) without breaking fingerprints — and why IP↔signal coherence is now a _detector_ rather than a protection.

---

## 3. Where fingerprints are fragile and where they produce false positives

This is the section that matters most for a security-only deployment.

### 3.1 The core asymmetry

A fingerprint used for abuse detection must answer two different questions, and they have opposite error costs:

- _"Is this the same device as before?"_ → wants **stability**. Failure = false negative (you miss a returning abuser).
- _"Is this a real user's real browser?"_ → wants **authenticity**. Failure = false positive (you block a legitimate customer).

Almost every anti-fingerprinting mechanism in existence attacks the first question. Almost every _spoofing_ technique attacks the second. A well-designed security fingerprint uses a _small number of high-stability signals_ for the first question and _coherence checks_ for the second — not a large hash.

### 3.2 Mobile Safari and iOS

- **iOS 26 (Sept 2025) turned Advanced Fingerprinting Protection on for _all_ Safari browsing, not just Private Browsing** [V, WebKit/multiple]. It injects noise into 2D canvas, WebGL readback, and `AudioBuffer.getChannelData()`; it also fixes `screen.width/height` to `innerWidth/innerHeight`, `screenX/Y` to `(0,0)`, and `outerWidth/Height` to the inner dimensions.
- **Critically, the noise is not per-session-random for the attacker's purposes — it is defeatable.** FingerprintJS published a technique to bypass Safari 17+ audio noise by collecting multiple samples and rounding, and states they maintain accuracy. Safari 17.5 specifically hardened this (per-buffer persistent noise, normally-distributed instead of uniform, adaptive magnitude), which is an arms race, not a wall.
- **Safari 26's other mitigations are blocklist-scoped.** WebKit's own description: Safari "prevents **known** fingerprinting scripts from reliably accessing web APIs… such as screen dimensions, hardware concurrency, the list of voices available through the SpeechSynthesis API, Apple Pay payment capabilities, web audio readback, 2D canvas and more." A first-party security script is not on that list. **This is the single most important 2026 fact for a security-only fingerprint: Safari's strongest protections target third-party trackers, not first-party scripts.**
- **iOS update cadence** is the dominant stability risk on mobile: an iOS point release changes Safari's WebKit build, the audio pipeline, and often canvas output. Plan for a re-identification window after every iOS release.
- **Mobile unicity is low.** Hiding in the Crowd measured only 18.5% unique mobile fingerprints. Mobile fingerprints are _weak identifiers_; on mobile you should be leaning on app-integrity signals, not the web fingerprint.

### 3.3 Android OEM variance

The Android ecosystem is the inverse of iOS: extremely high between-device variance (OEM skins, GPU drivers, WebView versions, font packs) and high _within-device_ stability, because the WebView/Chrome update channel is decoupled from the OS. Practical consequences:

- WebGL renderer strings are far more diverse than on iOS — good for identification, bad for building any allowlist.
- WebView-in-app fingerprints differ from Chrome-on-the-same-device fingerprints. Tiwari et al. (2022) studied >20,000 hybrid apps and found WebView fingerprints can contain **account-specific and device-specific** information identifying users across devices, and that the embedded browser **does not always honor standard browser privacy policies**.
- Android version numbers are visible through WebView/Chrome version strings and correlate with device age — a useful risk feature, not an identifier.

### 3.4 Virtualization, VMs, corporate images

From FingerprintJS' 2026 report (23B events):

- **VMs appeared in 13%** of Chromium-desktop identifications.
- **DevTools were open in 6%** of desktop sessions.
- **Browser tampering in 4.4%** of desktop identifications, up from 2.6% in 2024 (Chromium desktop: 4.8%).
- **VPN usage ~20% overall**, 33% on Chromium desktop, 13% mobile.

Corporate images and VDI are the classic false-positive source: hundreds of identical machines _should_ share a fingerprint, and GPU is often software-rendered (`SwiftShader`/`ANGLE (…, SwiftShader …)`), which is both a VM tell and a legitimate thin-client tell. Rules that treat "software WebGL renderer ⇒ bot" will block entire call centres.

### 3.5 Browser extensions

Extensions are simultaneously (a) a fingerprinting surface and (b) a false-positive generator:

- Detecting an extension is a fingerprint in itself. Gulyás et al. (2018, 16,393 participants): **39.29%** of Chrome users were unique from extension detection alone, and **485 well-chosen probes** sufficed.
- **Anti-fingerprinting extensions are detectable and make you _more_ identifiable, not less.** Vastel (2024) demonstrated two working detections: pixel-deviation checks against a known-drawn canvas (detects Canvas Fingerprint Defender on Chrome/Firefox with >50 pixel differences), and reading `e.stack` after deliberately calling `getImageData` wrongly, which leaks `chrome-extension://nomnklagbgmgghhjidfhnoelnjfndfpd/…` — the Canvas Blocker extension ID. Canvas Blocker had ~20K users at the time. **Users of niche privacy extensions are a tiny, highly identifiable cohort.**
- This cuts both ways for security: an extension ID in a stack trace is a _superb_ signal for detecting anti-detect tooling, and a _terrible_ signal for "this user is suspicious" (legitimate users install Canvas Blocker).

### 3.6 Anti-fingerprinting browsers — and the Brave farbling problem

**Brave ("farbling").** Per-session, per-eTLD+1, deterministic randomization. Three levels: Disabled / Standard (default) / Strict.

The security-critical detail, from FingerprintJS' own analysis of Brave's implementation: **Standard farbling is a multiplication by a per-domain "fudge" factor, and the fudge factor can be recovered.** A script can call `AudioBuffer.getChannelData()` on a 1-sample buffer of value 1 to read the factor directly, then divide it out of the audio fingerprint. FingerprintJS measured the recovered fingerprints:

| Environment                            | Fingerprint                             | Δ vs macOS Chrome baseline    |
| -------------------------------------- | --------------------------------------- | ----------------------------- |
| macOS Chrome 89 (baseline)             | 124.0434806260746                       | —                             |
| macOS Brave 1.21 (after un-farbling)   | 124.04347912294482 … 124.04348024313667 | **0.00000014% – 0.00000214%** |
| Windows Chrome 89                      | 124.04347527516074                      | 0.00000431%                   |
| Windows Brave 1.21 (after un-farbling) | …                                       | 0.00000364% – 0.00000679%     |
| macOS Safari 14                        | 35.10893232002854                       | 71.7%                         |
| macOS Firefox 86                       | 35.7383295930922                        | 71.2%                         |

The conclusion is unavoidable and generalizable: **revertible randomization is worse than no randomization.** A Brave-on-macOS user, after un-farbling, lands within ~2e-8 relative distance of the _unfarbled Chrome-on-macOS_ cohort, i.e. Brave users are effectively identifiable as a sub-population while appearing randomized. Brave's Strict mode (replace the wave with a pseudo-random sequence) does not have this property. Brave also blocks Web Audio entirely in some configurations, and as of v1.93 (Aug 2026) scrubs WebGL vendor/renderer to a single generic string and injects per-session/per-site noise into the WebGL extension list — that last one is _good_ (irreversible), the vendor/renderer collapse is _mixed_ (it makes all Brave users identical, which is herd immunity, but makes "Brave" itself a detectable cohort).

**Tor Browser.** Uniformity, not randomization. Letterboxing rounds the content window to 200×100 buckets; UA is standardized per-platform; canvas extraction is blocked; first-party isolation; NoScript. Known brittlenesses: the Tor fingerprint is _known_ (so Tor users are trivially identifiable as Tor users — 3.67% of top-1000 Alexa sites blocked or degraded Tor traffic in 2016), and maximizing the window leaks the monitor size.

**Tor Browser 14.5 (April 2025) removed HTTP-header OS spoofing.** Previously all desktop Tor users reported Windows in HTTP headers; since 14.0 (Oct 2024) / 14.5, headers are consistent with the JS `navigator` values. The stated reason is entropy — the mismatch between a Windows UA and Linux/macOS JS APIs was itself a fingerprint. The consequence is that desktop Tor users now sort into Windows / macOS / Linux HTTP-header buckets instead of one. **This is the clearest recent example of a vendor trading uniformity for coherence, and it is a direct precedent for §5: coherence beats uniformity.**

**Firefox.** Two modes:

- **ETP Strict / Private Browsing (FPP, `privacy.fingerprintingProtection`)** — the mainstream mode.
- **`privacy.resistFingerprinting` (RFP)** — the Tor-derived mode: letterboxing, canvas blocking, UA/timezone/screen spoofing, Math function spoofing.

Firefox 145 (Nov 2025) completed "Phase 2" of FPP. Mozilla's own numbers:

| State               | Users still uniquely trackable |
| ------------------- | ------------------------------ |
| No protections      | **65%**                        |
| Phase 1 protections | **~35%**                       |
| Phase 2 protections | **20%**                        |

Phase 2 specifics [V, Mozilla + BleepingComputer]:

- Noise added to canvas **background images only on readback**, not on display.
- **Local fonts blocked**, except a language-keyed allowlist (Japanese, Thai, Arabic, Chinese, Korean, Hebrew).
- `maxTouchPoints` reported as **0, 1, or 5**.
- Screen resolution reported as **height − 48 px**.
- `hardwareConcurrency` reported as **2**.
- "Tiny differences in how it performs math" patched (Math trig/exp/log family).

Mozilla is explicit that these are **not enabled by default** — they ship in Private Browsing and ETP Strict first, and "we work to enable them by default." As of October 2026 that has not happened for desktop default browsing. Mozilla's stated reason for not going further: aggressive blocking "is guaranteed to break legitimate website features," and calendar/scheduling/conferencing tools legitimately need the real timezone. **Note the escape hatch: a first-party security script on a site the user has exempted, or in default mode, sees the real values.**

### 3.7 The general lesson

Datta et al. (2019) evaluated **26 anti-fingerprinting tools** and found that for **24 of them** the protection was "so marginal that it makes almost no difference." The most common failure is the **inconsistency problem**: a tool changes one attribute but not its correlated partners, which makes the user _more_ unique than no tool at all. Eckersley named this in 2010 as the "Paradox of Fingerprintable Privacy Enhancing Technologies." FP-Scanner (USENIX Security 2018) turned this into a general attack: it detects countermeasures from the inconsistencies they introduce, and in many cases recovers the **original unaltered value**.

---

## 4. What Chromium, Safari, and Firefox announced for 2025–2026

### 4.1 Chrome / Chromium

**User-Agent reduction — DONE, and effectively frozen.** UA reduction began Q2 2022, was on by default from Chrome 110, and the deprecation trial ended after **Chrome 113 (April 2023)**. All resource requests now carry the reduced UA; `navigator.userAgent`, `appVersion`, and `platform` are all reduced. Detail moved to UA-CH. The Chromium docs note reduction does **not** cover iOS or Android WebView. There is no announced plan to remove UA-CH.

**Third-party cookies — NOT deprecated.** Chrome announced in July 2024 that it would "maintain our current approach to offering users third-party cookie choice" rather than deprecate. Third-party cookies remain supported in Chrome as of 2026.

**Privacy Sandbox — largely retired (Oct 17, 2025).** Anthony Chavez (VP, Privacy Sandbox) announced the retirement of, in Google's own words, technologies retired due to "low levels of adoption":

| Retired                                                                           | Kept                            |
| --------------------------------------------------------------------------------- | ------------------------------- |
| Attribution Reporting API (Chrome + Android)                                      | **CHIPS** (partitioned cookies) |
| **IP Protection**                                                                 | **FedCM**                       |
| On-Device Personalization                                                         | **Private State Tokens**        |
| Private Aggregation (incl. Shared Storage)                                        | Bounce tracking mitigations     |
| Protected Audience / FLEDP (Chrome + Android)                                     | UA reduction                    |
| Protected App Signals                                                             | Script Blocking                 |
| Related Website Sets (incl. `requestStorageAccessFor`, Related Website Partition) |                                 |
| SelectURL                                                                         |                                 |
| SDK Runtime                                                                       |                                 |
| Topics API (Chrome + Android)                                                     |                                 |

**This is the biggest reversal in the 2025–2026 window and it matters directly to fingerprinting:** the _only_ anti-tracking technologies Chrome committed to keep are the ones that address **stateful** tracking (CHIPS, FedCM, bounce tracking, Private State Tokens) plus **passive** fingerprinting reduction (UA reduction). Chrome is **not** deploying IP masking, and Google's own IP Protection page (last updated 2025-10-27) still describes the two-hop proxy design as if shipping, while the retirement post lists it as retired — the doc set is inconsistent, but the October 2025 news post is the authoritative statement. **Plan for Chrome to have no IP masking.**

**Bounce tracking mitigations — shipped, but conditional.** Launched by default in Chrome in **October 2023**, but only "for users who have opted-in to blocking third-party cookies." Mechanism: Chrome flags sites that participate in a navigation bounce, then periodically checks whether the user interacted with that site in the last **45 days**; if not, and third-party cookies are blocked, the site's storage is deleted after the next redirect through it. Federated auth/SSO/payments are out of scope. **Because third-party cookies are not deprecated, this protection only fires for the minority of users who turned 3PC off.**

**Private State Tokens (formerly Trust Tokens) — kept, GA.** An anti-fraud primitive: conveys a limited amount of trust from one browsing context to another without passive tracking. Google explicitly cites anti-fraud as a reason to maintain it, and the October 2025 post says it will "maintain Private State Tokens and explore additional approaches to help developers reduce fraud and abuse." **This is the sanctioned, privacy-preserving path for exactly the use case in this audit** — worth evaluating as a complement to a fingerprint.

**Script Blocking** remains listed under "Prevent covert tracking" in the Privacy Sandbox protections nav.

### 4.2 Safari / WebKit

**Private Browsing 2.0 (Safari 17.0, Sept 2023; extended 17.2, 17.5).** WebKit's framing is unusually candid and is the best available statement of vendor fingerprinting policy. They separate:

- _device fingerprinting_, _network/geographic fingerprinting_, _user-settings fingerprinting_, _user-behavior fingerprinting_, _user-traits fingerprinting_;
- and three privacy problems: **cross-site tracking**, **per-site user recall** (fingerprinting defeats "clear website data"), and **per-site visitor uniqueness**.

WebKit's stated approach: **"Make the fingerprint unique per website, and generate a new unique fingerprint for every fresh start such as at website data removal."** Their tools: multi-hop proxies for IP; limit fingerprintable APIs; **inject small amounts of noise**.

Concrete mechanisms:

- **Canvas/WebGL:** noise injected **only in regions where drawing occurs**; also applied to offscreen canvas readback in service workers and shared workers (17.5).
- **Web Audio:** noise on `AudioBuffer.getChannelData()`; since 17.5 the noise is _persistent per buffer_ (defeats averaging), _normally distributed_ (converges slowly to the true value), and _adaptive_ (larger where nodes are known to leak OS/hardware differences). Also applies to AudioWorklets.
- **Screen/window metrics:** `screen.width/height` → `innerWidth/innerHeight`; `screenX/Y` → `(0,0)`; `outerWidth/Height` → inner dimensions. Media queries observing screen size are also affected.
- **Link Tracking Protection:** strips known tracking query params **before navigation** (never sent over the wire), and hides query params + fragment + `document.referrer` from third-party scripts after a cross-site navigation.
- **Network:** Oblivious DNS-over-HTTPS by default; unencrypted HTTP proxied in Private Browsing; iCloud Private Relay gives **separate egress IP per tab** (defeats tab-linking by IP).

**Safari 26.0 (Sept 15, 2025) — the big change.** WebKit's release notes:

> "Safari 26.0 now prevents **known fingerprinting scripts** from reliably accessing web APIs that may reveal device characteristics, such as **screen dimensions, hardware concurrency, the list of voices available through the SpeechSynthesis API, Apple Pay payment capabilities, web audio readback, 2D canvas and more**. Safari additionally prevents these scripts from setting long-lived script-written storage such as cookies or LocalStorage. And lastly, Safari prevents known fingerprinting scripts from reading state that could be used for navigational tracking, such as query parameters and `document.referrer`."

And: **Advanced Fingerprinting Protection is now on for all browsing**, not just Private Browsing (announced WWDC 2025, shipped with iOS 26 / macOS 26 in Sept 2025).

**Safari 26 also froze the OS version in the iOS UA string** — `iPhone OS 18_6` now stays `18_6` on Safari 26, matching the macOS freeze that has existed since 2017. WebKit explicitly recommends feature detection over UA sniffing.

**Safari 26.0 shipped WebGPU** on macOS/iOS/iPadOS/visionOS — a new fingerprinting surface arriving in the same release that expanded fingerprinting protections.

**Lockdown Mode** now evaluates web fonts with a new **Safe Font Parser** and allows fonts that pass, rather than banning web fonts.

### 4.3 Firefox / Mozilla

**Firefox 145 (Nov 10, 2025)** — "Firefox expands fingerprint protections." Phase 2 completion. Mozilla's framing: they analyzed "how real people's browsers can be fingerprinted" and target "the most leaky fingerprinting vectors… while preserving functionality many sites need to work normally." Headline: **"the amount of Firefox users trackable by fingerprinters is reduced by half"** (65% → 20% trackable across Phase 1 + Phase 2). Shipped in Private Browsing and ETP Strict; "while we work to enable them by default."

**Firefox 72 (2020)** started ETP blocking of known fingerprinters; **Total Cookie Protection** remains the stateful defence.

**Firefox has not removed the UA string**, and has not frozen the OS token the way Chrome and Safari have. RFP mode standardizes UA per-platform (Windows → Windows 10; macOS → OS X 10.15; Android → Android 10; everything else → Linux X11).

**Firefox removed the Battery Status API (Fx 52, 2017)** after Olejnik et al.'s disclosure.

### 4.4 Cross-vendor summary for 2026

| Capability                                             | Chrome                              | Safari                                                 | Firefox                           |
| ------------------------------------------------------ | ----------------------------------- | ------------------------------------------------------ | --------------------------------- |
| Third-party cookies                                    | **Not deprecated** (kept)           | Blocked (ITP)                                          | Partitioned (TCP)                 |
| UA string                                              | Reduced + frozen (113, Apr 2023)    | Frozen OS version on iOS (Safari 26)                   | Not frozen; RFP-standardized      |
| Client Hints                                           | Yes (UA-CH)                         | No                                                     | No                                |
| Canvas/WebGL noise                                     | No (not a browser feature)          | **Yes** — Private Browsing + AFP-all-browsing (iOS 26) | Yes (FPP Phase 2 readback noise)  |
| Audio noise                                            | No                                  | **Yes**, adaptive/normal/persistent (17.5+)            | Partially                         |
| Screen/window metric normalization                     | No                                  | **Yes**                                                | Partial (height − 48)             |
| Local font blocking                                    | No                                  | Content-blocker-scoped                                 | **Yes**, language-keyed allowlist |
| `hardwareConcurrency` clamp                            | No                                  | For known FP scripts                                   | **Yes → 2**                       |
| `maxTouchPoints` clamp                                 | No                                  | No                                                     | **Yes → 0/1/5**                   |
| IP masking                                             | **No — IP Protection retired**      | Private Relay (iCloud+, per-tab egress IP)             | No                                |
| Anti-fraud trust primitive                             | **Private State Tokens (kept, GA)** | Private Access Tokens / Web AdAttributionKit           | No equivalent                     |
| Bounce tracking mitigation                             | Yes (only when 3PC blocked)         | ITP + cloaked-IP cookie cap                            | Yes                               |
| Blocklist-scoped API denial for "known fingerprinters" | Script Blocking                     | **Yes** (Safari 26)                                    | **Yes** (ETP)                     |

---

## 5. Fingerprint consistency / coherence checks

Coherence checking is where the modern field has actually moved, and it is the single most transferable idea for a security-only fingerprint.

### 5.1 The formal result

Google's WWW 2024 study is the definitive statement: **the information revealed by many Web APIs is often highly correlated**, and ignoring this "may grossly overestimate fingerprinting risk." They clustered 60 "core" surfaces and found stable clusters:

- **Device properties** cluster, subdivided into _core device properties_ (language, pixel ratio, screen width/height), _WebGL parameters_, and _independent low-entropy properties_.
- **User-agent** cluster, with cross-cluster correlation to device properties.
- Their worked example of high mutual information: **language setting ↔ keyboard layout**.

**Design consequence:** collecting one signal from each cluster adds real information; collecting five signals from the same cluster adds almost nothing. A 40-signal fingerprint built from correlated signals is a 6-signal fingerprint with 34 redundant fields and 34 ways to be inconsistent.

### 5.2 The empirical result (evasive bots)

**FP-Inconsistent (Venugopalan et al., 2024; extended 2025)** is the strongest available measurement. They deployed a honey site with two commercial anti-bot services (DataDome and BotD) and solicited traffic from **20 bot services selling "realistic and undetectable traffic"**:

- **Half a million requests.**
- **52.93% average evasion rate against DataDome; 44.56% against BotD.**
- Bots that evade alter _different_ fingerprint attributes depending on which service they target.
- **Inconsistent fingerprint attributes are pervasive in evasive bot traffic.**
- They derived data-driven rules over **space** (two attributes in one fingerprint) and **time** (one attribute at two points in time). Applying those rules **reduced the evasion rate by 48.11% against DataDome and 44.95% against BotD** — i.e. _coherence checking roughly halved bot evasion without adding a single new signal_.

### 5.3 The cross-validations that actually work

Ordered roughly by signal-to-noise ratio:

**A. UA / UA-CH / JS feature matrix.** The classic triple: legacy UA string vs `Sec-CH-UA*` headers vs JS API presence. Chrome's UA reduction deliberately keeps them consistent, so mismatches indicate tampering. Concrete checks:

- Claimed Chrome major version vs presence of version-gated JS APIs.
- `navigator.userAgentData.brands` vs `navigator.userAgent`.
- Chrome-only APIs (`navigator.connection`, `deviceMemory`) present while claiming Firefox/Safari.
- Safari-only APIs present while claiming Chrome.

**B. GPU renderer vs WebGL extensions vs WebGL parameters vs UA platform.** Brave's own crawl found most sites use these APIs "exclusively for browser fingerprinting." For security:

- `ANGLE (… Metal …)` on a UA claiming Windows → inconsistent.
- `SwiftShader` / software renderer on a UA claiming a high-end discrete GPU → VM/headless.
- Extension list inconsistent with the claimed renderer/driver generation.
- WebGPU adapter `{vendor, architecture}` inconsistent with WebGL renderer.

**C. Timezone vs geolocation vs IP.** The canonical three-way check, and the one DataDome names as a "weak signal" combination. VPN usage makes this _more_ valuable, not less: 1 in 5 sessions in 2025 involved VPN, so the check must be probabilistic (a VPN user's timezone legitimately disagrees with the IP's geo), not a hard rule.

**D. Language quintuple.** DataDome's example, and the cleanest implementation pattern in the public literature. Five independent sources of "what language is this browser":

1. `Accept-Language` HTTP header (server-side)
2. `navigator.languages`
3. `navigator.language`
4. `speechSynthesis.getVoices()` default voice `lang`
5. `Intl.RelativeTimeFormat().resolvedOptions().locale`

Their recommendation: derive `hasConsistentLanguagesSignals` (is the primary `Accept-Language` value present in all others?) and `hasStrictConsistentLanguagesSignals` (is it _first_ in all others?). Their worked counterexample: `accept-language: en-GB` with a default speech voice of Russian.

**E. Hardware coherence: `hardwareConcurrency` × `deviceMemory` × `maxTouchPoints` × screen × UA.** Firefox's FPP clamps are the tell: a browser reporting `hardwareConcurrency: 2` alongside a high-end GPU renderer is running Firefox FPP. More generally, cores and memory are correlated with device class, which is correlated with screen size and platform.

**F. Native-function integrity.** The strongest single spoofing detector, because it targets the _mechanism_ of spoofing rather than its output:

```js
try {
  ctx.getImageData(canvas);
} catch (e) {
  // deliberately wrong arity
  if (e.stack.includes("chrome-extension://")) {
    /* extension */
  }
}
```

Vastel's 2024 demo leaks the Canvas Blocker extension ID this way. The same trick generalizes: any overridden native function will show a non-native frame in its stack, and V8's lazy `Error.stack` formatting is what makes the CDP detection (§6) work too.

**G. Canvas readback vs canvas display.** Safari's noise is applied only on readback, in drawn regions only. A script that draws a known solid color and reads it back can therefore _measure the noise_ — which is itself a signal that the browser is Safari-with-AFP, and (as Brave's farbling showed) may allow recovery of the original.

**H. `screen.*` vs `innerWidth/Height` vs media queries.** Safari 26 deliberately makes these agree. A browser where `screen.width === innerWidth` _and_ the UA claims Safari is consistent; a spoofer that patches only `screen.width` will fail the media-query cross-check (which WebKit explicitly notes is also mitigated).

**I. Update-churn coherence.** From Li & Cao: fingerprint changes cluster around real browser/OS release dates. A device that claims Chrome 131 for 18 months while never churning its audio/WebGL signatures is not a device that has been running Chrome 131 for 18 months.

**J. `Notification.permission` and permissions anomalies.** Headless Chrome historically reported `Notification.permission === 'denied'` while `navigator.permissions.query({name:'notifications'})` returned `prompt`. Cheap, high-value, and part of the standard headless test suites (bot.sannysoft, CreepJS).

---

## 6. Behavioral and automation signals for abuse detection

### 6.1 Automation artifacts

**Tier 1 — trivially defeated, but still worth collecting (they catch the lazy majority).**

- `navigator.userAgent.includes("HeadlessChrome")` — unmodified Puppeteer sends `… HeadlessChrome/131.0.0.0 Safari/537.36`.
- `navigator.webdriver === true`.
- `$cdc_` / `$chrome_asyncScriptInfo` globals (Chromedriver).
- Empty `navigator.plugins` / `navigator.mimeTypes`.

**Tier 2 — CDP side effects. The single best generic automation signal as of 2025–2026.**

```js
var detected = false;
var e = new Error();
Object.defineProperty(e, "stack", {
  get() {
    detected = true;
  },
});
console.log(e); // console.log is part of the challenge — do not remove
```

Why it works: CDP clients that send `Runtime.enable` (Puppeteer, Playwright, Selenium, ChromeDriver) cause V8 to serialize the logged object, and V8 defers `Error.stack` formatting until the property is accessed — so the getter fires. Verified independently by DataDome (June 2024) and Castle/Vastel (March 2025).

Known limitations, stated by the researchers themselves:

- **False positives: human users with DevTools open.** Mitigating by also checking whether the DevTools _UI_ is open fails, because bot developers discovered anti-bot vendors added that rule and now launch with `args: ['--auto-open-devtools-for-tabs']`.
- **Bypassable.** `nodriver` (successor to undetected-chromedriver) and Selenium-Driverless avoid `Runtime.enable` entirely and reimplement automation over low-level CDP commands. Bypass is now documented and part of anti-detect test suites.

**Tier 3 — integrity of the runtime itself.**

- Native-function tampering (see §5F). This catches the whole class of "override the API to lie" evasion.
- `Function.prototype.toString()` on natives — overridden functions lose `[native code]`.
- Proxy detection on `HTMLCanvasElement.prototype.toBlob/toDataURL` and `CanvasRenderingContext2D.prototype.getImageData` — the exact functions Canvas Blocker wraps.

**Tier 4 — behavioral. This is where the 2026 research says the future is.**

**FP-Agent (Wang, Shafiq, Vekaria; May 2026)** is the most important recent result and it directly answers the "what do we do about AI browsing agents" question:

- First controlled measurement of **7 AI browsing agents vs human users** on an instrumented honey site (flight booking, online shopping, forum interaction).
- **Browser fingerprints provide limited discriminative power** when several AI agents share a fingerprint — because they run real browsers.
- **Behavioral fingerprints are distinctive**: typing, scrolling, and mouse dynamics separate agents from humans _and from one another_.
- **FP-Agent detected all 7 AI browsing agents. Cloudflare's bot detection detected 1.**

That is a 7× detection gap in favour of behavioural signals, measured, on the same traffic.

**FP-Inconsistent** (§5.2) supports the same conclusion from the other direction: static fingerprint attributes alone leave ~50% evasion; coherence rules over space and time cut that roughly in half; the researchers explicitly conclude that detection "goes beyond fingerprinting" and must combine behaviour, IP reputation, and contextual signals.

### 6.2 Behavioral signals that are collectable and useful

| Signal                                                                              | Notes                                                                                                                             |
| ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Keystroke cadence (dwell/flight times, digraph latency)                             | Strong human/agent separator per FP-Agent. Beware accessibility tools and IME.                                                    |
| Scroll dynamics (velocity profile, easing, wheel deltas vs programmatic `scrollTo`) | Programmatic scroll is instantaneous and perfectly linear.                                                                        |
| Mouse movement (entropy, curvature, velocity distribution)                          | Humans show sub-movement correction; agents show either straight lines or synthetic Bezier.                                       |
| Touch dynamics                                                                      | Fewer artifacts available on mobile; also less data.                                                                              |
| Navigation timing / request sequencing                                              | Server-side. Ordering, inter-request gaps, and the browsing graph are cheap and hard to fake.                                     |
| Form-fill timing and field-order                                                    | Humans pause, backspace, re-focus. Agents fill in DOM order.                                                                      |
| Pointer events vs touch events coherence                                            | A UA claiming iPhone with only mouse events.                                                                                      |
| Page-visibility / focus behaviour                                                   | FP-Radar found `visibilityState` abused for **ephemeral fingerprinting** in the wild (first detection of this in the literature). |
| Time-of-day / request origin                                                        | Named by DataDome as a "weak signal" that is cheap and non-identifying.                                                           |

### 6.3 Interaction-cadence caveats

- **Accessibility and assistive tech** produce non-human-looking input cadence. Never make behaviour a hard gate.
- **AI agents doing legitimate work** (agentic commerce, browser assistants) are now a real and growing traffic class. FingerprintJS explicitly warns that "programmatic browser patterns fraud teams have historically treated as red flags are worth re-examining" and shipped an AI Agent Detection product. Blocking all automation is no longer a coherent policy.
- **Server-side is the right home for behavioural scoring.** Client-side collection is trivially forgeable; the value is in the _distribution_ over many sessions, which only the server sees.

---

## 7. What this means for a security-only (not ad-tracking) fingerprint

### 7.1 The governing constraints

1. **You are first-party.** Safari 26's strongest mitigations and Firefox's ETP blocklist are scoped to _known third-party fingerprinting scripts_. A first-party script on your own origin, in default browsing mode, sees substantially unmodified values on Safari and Firefox. This is the single largest asymmetry in your favour and it should drive signal selection. (Do not abuse it — it is contingent on the blocklist-scoping continuing.)
2. **Chrome is now the least-mitigated browser.** No IP masking (retired), no UA detail (reduced), no canvas/audio noise. Chrome is where a fingerprint works best — and it is also the majority of your traffic and the majority of sophisticated abuse (per FingerprintJS' 2026 report: "desktop is where your enforcement effort has the highest return").
3. **Coherence beats collection.** FP-Inconsistent halved evasion with _zero_ new signals, purely by cross-validating. Google showed that correlated signals add almost nothing.
4. **Prefer stability over uniqueness.** You need to recognise a returning abuser across a browser update, not to distinguish two identical corporate laptops.
5. **Expect re-identification windows.** iOS/Chrome/macOS release days will break a fraction of your fingerprints. Design for fuzzy re-linking (Li & Cao's lesson: rule-based FP-Stalker at 1M fingerprints has F1 ≈ 76%, not 99%) rather than exact-hash equality.
6. **Use the sanctioned primitives where they exist.** Private State Tokens survived the Privacy Sandbox purge _specifically_ for anti-fraud. That is the vendor-blessed path.

### 7.2 Best stability-per-privacy-cost — the short list

Ranked by (stability × spoof-resistance) ÷ (privacy cost + false-positive risk):

**Tier A — collect these. High stability, low privacy cost, hard to spoof consistently.**

| Signal                                                                                                    | Why                                                                                                                                                                                                                                                                                                |
| --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Hardware/GPU family** (WebGL renderer _class_, WebGL extensions hash, WebGPU adapter, shader precision) | Highest stability of any browser signal; survives browser updates; changes only on GPU/driver change. Spoofing requires faking a whole correlated cluster. Caveat: on Safari 26 / Brave the _string_ may be scrubbed or noised — so use it as a _class_ match and a _coherence_ input, not a hash. |
| **WebAssembly / native-code timing signature**                                                            | Guri & Fibert 2025: Chrome vs Edge separable with UA fully spoofed, FPR <1%. Survives UA/JS spoofing. Low privacy cost (it's a capability measurement, not a user attribute).                                                                                                                      |
| **Native-function integrity + stack-trace forensics**                                                     | Not a user attribute at all — it detects _spoofing_ rather than identifying users. Effectively zero privacy cost, and it is the highest-signal detector of anti-detect tooling.                                                                                                                    |
| **CDP side-effect detection**                                                                             | Same category: detects the mechanism, not the person. Documented false positive = DevTools open (6% of desktop sessions per FingerprintJS). Handle as a risk _feature_, never a block.                                                                                                             |
| **Coherence rules (space + time)**                                                                        | FP-Inconsistent: ~50% evasion reduction for zero new collection. The best privacy-per-bit trade in the entire field.                                                                                                                                                                               |

**Tier B — collect, but expect churn and mitigate.**

| Signal                                                           | Why                                                                                                                                                                                                                           |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Screen metrics (resolution, DPR) + screen/window consistency** | 4.8–4.9 bits, stable across a decade of studies. Churns on monitor/dock/zoom. Safari 26 normalises `screen.*` to `inner*` — so compare _sets_ of related values rather than a single field.                                   |
| **Timezone + `Intl` locale/calendar/numbering**                  | ~3 bits, very stable, and the anchor for the IP-coherence check. Firefox deliberately does _not_ spoof timezone because calendar/scheduling tools need it — so it survives. Low privacy cost.                                 |
| **Language quintuple**                                           | Low standalone entropy, high coherence value, near-zero privacy cost.                                                                                                                                                         |
| **Font _metrics_ (glyph bounding boxes), not font list**         | 43 characters identify 34% of browsers. Metrics use _system_ fonts, so they partly survive Firefox's local-font blocking. Much lower privacy cost than full enumeration (no "what software do you have installed" inference). |
| **Platform/CPU/OS-family class from UA-CH high-entropy hints**   | Coarse by design, low privacy cost, and _permission-gated_ — which means it is a deliberate signal, not a covert one.                                                                                                         |
| **Media codec support**                                          | Stable, correlates with platform (Google used platform as its proxy), and doubles as a headless/Chromium-vs-Chrome tell.                                                                                                      |
| **`hardwareConcurrency` / `deviceMemory` / `maxTouchPoints`**    | Coarse, stable, excellent coherence inputs. Note Firefox clamps them (2, and 0/1/5) — treat the clamped values as "Firefox FPP" rather than as hardware facts.                                                                |

**Tier C — use only as coherence inputs, never as identity.**

| Signal                                                   | Why not identity                                                                                                                                                                                                                                                                                                                                   |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Canvas 2D hash**                                       | 8.3–8.5 bits — but the _underlying_ render diversity is low (Mowery & Shacham: 50 distinct renders from 300 samples), the hash's uniqueness comes from hashing a long noisy string, and it is noised on Safari (all browsing since iOS 26), Firefox (readback), and Brave (per-session). **Use it as one input to a fuzzy match, never as a key.** |
| **AudioContext fingerprint**                             | 5.4 bits and highly stable — genuinely useful — but noised on Safari (17.5+, adaptive/normal/persistent), farbled on Brave (and _revertible_), disabled on Tor. FingerprintJS' own judgement: "slightly to uniqueness, highly stable, slight net increase." That's Tier B-ish on Chrome, Tier C on Safari/Brave.                                   |
| **`navigator.plugins` / `mimeTypes`**                    | 9.49 bits in 2018, but **degenerate in 2026** — a fixed stub in Chrome and Firefox. Do not carry over a 2018 number.                                                                                                                                                                                                                               |
| **`navigator.connection`**                               | Chrome-only, moves with network conditions, and quantized (`downlink` 25 kbps, `rtt` 25 ms). Use only as a coarse "is this a mobile/constrained network" input.                                                                                                                                                                                    |
| **Viewport / `innerWidth` / `outerWidth` / `screenX/Y`** | Too unstable. Safari zeroes `screenX/Y`.                                                                                                                                                                                                                                                                                                           |
| **Battery**                                              | Removed or coarsened on most browsers. Not worth collecting.                                                                                                                                                                                                                                                                                       |
| **Storage quota / `estimate()`**                         | Coarse disk bucket, and it changes as the disk fills. Weak.                                                                                                                                                                                                                                                                                        |
| **Speech synthesis voices**                              | Useful on Windows (10–40 voices), empty on Linux/Chrome, and Safari 26 denies it to known fingerprinting scripts. Population-dependent; use as a coherence input against language.                                                                                                                                                                 |
| **Sensors**                                              | Permission-gated on iOS; on Android, availability is a platform signal rather than a device signal.                                                                                                                                                                                                                                                |

### 7.3 What to avoid

1. **Do not build a mega-hash.** Google's correlation result plus FP-Inconsistent's evasion rates both say the marginal signal is near zero and the marginal _inconsistency surface_ is large. Six orthogonal signals beat forty correlated ones.
2. **Do not use reversible randomization.** Brave's Standard farbling is recoverable by dividing out the fudge factor; the un-farbled fingerprints land within ~2e-8 of the Chrome-on-same-OS cohort. If you ever randomize, use an irreversible transform.
3. **Do not treat anti-fingerprinting browsers as suspicious by default.** Firefox ETP Strict, Private Browsing, Safari Private Browsing, and Brave are _legitimate, mainstream_ configurations. Detecting "this user has protections on" is a useful _context_ feature; using it as a block rule will systematically reject privacy-conscious legitimate users and is a compliance risk under GDPR legitimate-interest reasoning.
4. **Do not treat VM/devtools/automation presence as a block.** FingerprintJS' own baseline: 13% VM, 6% devtools, 4.4% tampering, ~20% VPN. These are the _base rates_. Only stacked, rare combinations are actionable ("less than 4% of traffic scored above 20 on the Suspect Score").
5. **Do not validate against cookies.** Li & Cao: 32% of browser instances cleared cookies during an 8-month study. Any accuracy metric anchored to cookie continuity is measuring cookie survival, not fingerprint quality.
6. **Do not cite 2010–2018 entropy numbers as current.** `navigator.plugins` is the proof: 15.4 → 11.06 → 9.49 bits in the literature, then a fixed stub in practice.

### 7.4 Recommended signal set (concrete)

**Identity (fuzzy-matched, never exact-hashed):**

1. WebGL/WebGPU hardware **class** + extensions hash + shader precision (3 probes, one cluster)
2. WebAssembly/native timing signature (1–2 probes)
3. Screen metric **set** (resolution + DPR + screen/inner/outer consistency)
4. Timezone + `Intl` locale/calendar/numbering
5. Font **metrics** via a fixed 43-character glyph set (not enumeration)
6. Platform class from UA-CH high-entropy hints (permission-gated)
7. Codec support set

**Coherence (boolean features, not values):** 8. UA ↔ UA-CH ↔ JS feature-matrix agreement 9. GPU renderer ↔ extensions ↔ UA platform agreement 10. Timezone ↔ IP-geo agreement (probabilistic; VPN-aware) 11. Language quintuple agreement (`hasConsistentLanguagesSignals`, `hasStrictConsistentLanguagesSignals`) 12. Hardware triple agreement (`hardwareConcurrency` × `deviceMemory` × `maxTouchPoints` vs device class) 13. `screen.*` ↔ `innerWidth/Height` ↔ media-query agreement 14. Fingerprint-delta vs expected update churn (time-series feature)

**Integrity / automation (boolean risk features):** 15. Native-function tampering (arity-error stack inspection) 16. CDP side-effect detection (`Error.stack` getter) 17. `navigator.webdriver`, `HeadlessChrome` substring, `$cdc_`, plugins stub 18. `Notification.permission` vs `permissions.query` consistency 19. Extension ID leakage in stack traces (context, not block) 20. Behavioral: keystroke cadence, scroll dynamics, mouse dynamics, form-fill ordering (server-side scoring)

**Plus:** evaluate **Private State Tokens** as a complement — it is the only anti-fraud primitive Google explicitly committed to keeping in the October 2025 Privacy Sandbox wind-down.

---

## 8. Research gaps found

These are signals requested in the brief for which **no credible published entropy measurement exists**. Anyone building on them is measuring empirically rather than citing:

1. **UA-CH high-entropy values** (`architecture`, `bitness`, `model`, `platformVersion`, `fullVersionList`, `wow64`, `formFactors`) — no published entropy. The full version string likely restores most of the legacy UA entropy.
2. **WebGL parameters and extensions separately** — Google clustered them but published no per-surface bits.
3. **WebGL shader precision** — universally used, never measured.
4. **`document.fonts.check()`** — no published entropy.
5. **`Intl.DateTimeFormat().resolvedOptions()`** as a distinct signal — bundled into "language" everywhere.
6. **`Intl` calendar/numbering system** (`calendar`, `numberingSystem`, `hourCycle`) — no measurement at all.
7. **Speech synthesis voices** — no published entropy, despite being high-variance on Windows and explicitly targeted by Safari 26.
8. **WebGPU adapter info** — a 2025 ACM paper exists ("Unveiling Privacy Risks in WebGPU through Hardware-based Device Fingerprinting", DOI 10.1145/3734477.3734700) but no bit figure is verifiable in the open literature. A widely-repeated "14–16 bits" figure traces only to a secondary blog.
9. **Storage quota / `estimate()`** — no measurement.
10. **WebAssembly feature sets and timing** — Guri & Fibert give an FPR, not an entropy figure.
11. **Math/JS engine quirk families** — no modern measurement; the Mulazzani 2013 and Mowery 2011 work is pre-Spectre and pre-timing-coarsening.
12. **CSS feature support (`CSS.supports`)** — Unger 2013 is the last real study.
13. **Error stack traces as a signal** — used only as a _spoofing detector_, never measured as entropy.
14. **Key event codes / `getLayoutMap()`** — Google identified keyboard-layout ↔ language mutual information but published no bits.
15. **`navigator.connection`** — FP-Radar proved abuse in the wild; no entropy measurement.
16. **Behavioral signal entropy** — FP-Agent shows behavioural features are highly discriminative but reports classification performance, not bits.

---

## 9. Sources

### Primary measurements (peer-reviewed / published)

1. **Eckersley, P. (2010). "How Unique Is Your Web Browser?"** PETS 2010. https://coveryourtracks.eff.org/static/browser-uniqueness.pdf — 470,161 fingerprints; 83.6% unique (94.2% with Flash/Java); UA 10.0 / plugins 15.4 / fonts 13.9 bits; `H_M = 18.84`.
2. **Eckersley, P. (2010). "A Primer on Information Theory and Privacy."** EFF Deeplinks. https://www.eff.org/deeplinks/2010/01/primer-information-theory-and-privacy — the entropy-as-identifiability framing; 33 bits for a global population.
3. **Mowery, K. & Shacham, H. (2012). "Pixel Perfect: Fingerprinting Canvas in HTML5."** W2SP. 50 distinct renders from 300 canvas samples, largest cluster 172; 50 distinct WebGL renders from 270 samples.
4. **Acar, G. et al. (2014). "The Web Never Forgets."** ACM CCS. Canvas fingerprinting on 5,542 of top-100K sites; the canvas "signature" strings later reused by Google.
5. **Fifield, D. & Egelman, S. (2015). "Fingerprinting Web Users Through Font Metrics."** FC 2015. 125,000 glyphs, 1,000+ browsers; 43 characters uniquely identify 34%.
6. **Laperdrix, P., Rudametkin, W., Baudry, B. (2016). "Beauty and the Beast: Diverting Modern Web Browsers to Build Unique Browser Fingerprints."** IEEE S&P. AmIUnique dataset; 118,934 fingerprints; 89.4% desktop / 81% mobile unique. (PDF mirrors blocked by anti-bot at time of research; entropy figures taken from the 2019 survey's Table 3.)
7. **Cao, Y., Li, S., Wijmans, E. (2017). "Cross-Browser Fingerprinting via OS and Hardware Level Features."** NDSS. 31 WebGL rendering tasks; >99% of 1,903 devices uniquely identified; cross-browser stable.
8. **Gómez-Boix, A., Laperdrix, P., Baudry, B. (2018). "Hiding in the Crowd: An Analysis of the Effectiveness of Browser Fingerprinting at Large Scale."** WWW 2018. https://dl.acm.org/doi/abs/10.1145/3178876.3186097 — 2,067,942 fingerprints from a top-15 French site; 33.6% unique overall, 35.7% desktop, 18.5% mobile; per-attribute entropy table.
9. **Vastel, A., Laperdrix, P., Rudametkin, W., Rouvoy, R. (2018). "FP-Stalker: Tracking Browser Fingerprint Evolutions."** IEEE S&P. https://www.usenix.org/conference/usenixsecurity18/presentation/vastel (companion FP-Scanner paper at the same venue). 45.52% change in 1 day; 51.8 days average tracking; 26% >100 days.
10. **Vastel, A. et al. (2018). "FP-Scanner: The Privacy Implications of Browser Fingerprint Inconsistencies."** USENIX Security 18. https://www.usenix.org/conference/usenixsecurity18/presentation/vastel — detects countermeasures from inconsistencies and recovers original values.
11. **Datta, A. et al. (2019). "Taming the Shape Shifter: Detecting Anti-fingerprinting Browsers."** (cited via the 2019 survey). 26 tools evaluated; 24 provide marginal protection.
12. **Laperdrix, P., Bielova, N., Baudry, B., Avoine, G. (2019/2020). "Browser Fingerprinting: A Survey."** arXiv:1905.01051 / TWEB. https://arxiv.org/html/1905.01051v2 — **the source for the comparative entropy table (Table 3)** used throughout this report; defence taxonomy; adoption measurements.
13. **Andriamilanto, N., Allard, T., Le Guelvouit, G. (2021). "Guess Who? Large-Scale Data-Centric Study of the Adequacy of Browser Fingerprints for Web Authentication."** 4M+ fingerprints, 216 attributes; 81% unique; >90% of attributes unchanged after 6 months.
14. **Li, S. & Cao, Y. (2020). "Who Touched My Browser Fingerprint? A Large-Scale Measurement Study and Classification of Fingerprint Dynamics."** IMC 2020. https://dl.acm.org/doi/10.1145/3419394.3423614 — 7,246,618 fingerprints, 1,329,927 browser instances, 8 months; 960,853 dynamics; 32% clear cookies; FP-Stalker F1 86.1% → 75.9%; Browser ID 0.1% FP / 0.3% FN; three dynamics causes.
15. **Bahrami, P.N., Iqbal, U., Shafiq, Z. (2022). "FP-Radar: Longitudinal Measurement and Early Detection of Browser Fingerprinting."** PoPETS 2022. https://arxiv.org/abs/2112.01662 — detected abuse of Gamepad, Clipboard, Page Visibility, Network Information (`rtt`, `downlink`), `deviceMemory`, `paint`; time-to-detection up to 7 years before disclosure.
16. **Bacis, E., Bilogrevic, I., Busa-Fekete, R., Herath, A., Sartori, A., Syed, U. (2024). "Assessing Web Fingerprinting Risk."** WWW 2024. https://arxiv.org/html/2403.15607v1 — **the modern authoritative entropy study.** 5,383 surfaces / 161 APIs; >10M Chrome clients; Chow–Liu joint-entropy bound; ±1.5 bits @ 90%; bimodal session entropy at ~20 bits; language↔keyboard-layout mutual information; surface clusters.
17. **Venugopalan, H., Munir, S., Ahmed, S., Wang, T., King, S.T., Shafiq, Z. (2024/2025). "FP-Inconsistent: Measurement and Analysis of Fingerprint Inconsistencies in Evasive Bot Traffic."** https://arxiv.org/abs/2406.07647 — honey site, 20 bot services, ~500K requests; 52.93% evasion vs DataDome, 44.56% vs BotD; consistency rules cut evasion by 48.11% / 44.95%.
18. **Annamalai, M.S.M.S., Bilogrevic, I., De Cristofaro, E. (2025). "Beyond the Crawl: Unmasking Browser Fingerprinting in Real User Interactions."** WWW 2025. https://arxiv.org/abs/2502.01608 — 30 participants, 10 weeks, 3,000 top sites; **automated crawls miss 45% of the fingerprinting sites real users encounter.**
19. **Berke, A. et al. (2025). "How Unique is Whose Web Browser? The role of demographics in browser fingerprinting among US users."** PoPETS 2025(1). https://arxiv.org/abs/2410.06954 — 8,400 US participants with demographics; lower-income users more at risk; demographics inferable from fingerprint attributes.
20. **Wang, E., Shafiq, Z., Vekaria, Y. (2026). "FP-Agent: Fingerprinting AI Browsing Agents."** arXiv:2605.01247. https://arxiv.org/abs/2605.01247 — 7 AI browsing agents vs humans; browser fingerprints give limited discriminative power; **behavioural fingerprints separate them; FP-Agent detects 7/7, Cloudflare detects 1/7.**
21. **Guri, M. & Fibert, D. (2025). "Browser Fingerprinting Using WebAssembly."** arXiv:2506.00719. https://arxiv.org/abs/2506.00719 — Chrome vs Edge separable with UA fully spoofed, FPR <1%; validated across Intel/AMD/ARM, Windows/macOS/Android/iOS, VMware/KVM/VirtualBox.
22. **Bahrami, P.N., Cutler, D., Bilogrevic, I. (2025). "Byte by Byte: Unmasking Browser Fingerprinting at the Function Level Using V8 Bytecode Transformers."** arXiv:2509.09950. V8 bytecode-level detection, +4% page-load latency, robust to obfuscation.
23. **Sakib, A.H.M.N. et al. (2026). "To WASM or Not to WASM: Evaluation of Browser Fingerprinting Defenses Under WASM based Obfuscation."** RAID 2026. arXiv:2508.21219. Research detectors are moderately vulnerable to WASM obfuscation; **browser-extension and native-browser API-interception defences remain completely effective.**
24. **Tiwari, A., Prakash, J., Rahimov, A., Hammer, C. (2022). "Our fingerprints don't fade from the Apps we touch: Fingerprinting the Android WebView."** arXiv:2208.01968. >20,000 hybrid apps; account- and device-specific leakage; WebView does not always honour browser privacy policies.
25. **Das, A. et al. (2018).** Sensor-based fingerprinting on 3,695 of top-100K sites (cited via FP-Radar).
26. **Olejnik, L. et al. (2016, 2017).** Battery Status API privacy analysis; documented the removal/coarsening across browsers (cited via the 2019 survey).
27. **Sjösten, A. et al. (2017); Starov, O. & Nikiforakis, N. (2017); Gulyás, G.G. et al. (2018); Starov, O. et al. (2019).** Extension fingerprinting: 12,154/43,429 Chrome extensions detectable; 39.29% of 7,643 Chrome users unique from extensions; 485 probes suffice; 5.7% of 58,034 extensions had fingerprintable bloat (all cited via the 2019 survey).
28. **Al-Fannah, N.M. et al. (2018).** Top Majestic 10K crawl: 68.8% of sites performing fingerprinting under a broad 17-attribute definition; 284 distinct attributes observed.
29. **Englehardt, S. & Narayanan, A. (2016).** OpenWPM 1M-site crawl: 14,371 canvas, 3,250 canvas-font, 715 WebRTC, 67 AudioContext sites.
30. **Nikiforakis, N. et al. (2013, 2015).** Cookieless Monster; PriVaricator (WWW 2015) — the randomization-policy lineage that Brave's farbling descends from.
31. **Laperdrix, P. et al. (2017). "FPRandom."** ESSoS 2017. Noise in Canvas/AudioContext + JS property enumeration-order randomization.
32. **Wu, Y. et al. (2019). "UniGL."** Removes WebGL inter-device differences by redefining floating-point ops in GLSL — the uniformity alternative to randomization.
33. **Boda, K. et al. (2012).** Cross-browser fingerprinting from OS/hardware attributes (989 users).
34. **Bursztein, E. et al. (2016). "Picasso."** Google's canvas-based inorganic-traffic filtering, linked to reCAPTCHA.
35. **Mulazzani, M. et al. (2013); Mowery, K. et al. (2011); Schwarz, M. et al. (2019); Sánchez-Rola, I. et al. (2018); Nakibly, G. et al. (2015); Saito, T. et al. (2016, 2017).** JS-engine conformance, benchmarking, clock-skew, and CPU-feature fingerprinting (all via the 2019 survey).

### Vendor and standards sources

36. **Google Chrome / Privacy Sandbox — "Update on Plans for Privacy Sandbox Technologies," Oct 17 2025.** https://privacysandbox.com/news/update-on-plans-for-privacy-sandbox-technologies/ — **IP Protection, Topics, Protected Audience, Attribution Reporting, Related Website Sets, SelectURL, SDK Runtime, On-Device Personalization, Private Aggregation all retired; CHIPS, FedCM, Private State Tokens kept.**
37. **Google — IP Protection Overview.** https://privacysandbox.google.com/protections/ip-protection — two-hop proxy design, RSA blind signatures, coarse-geo IP geofeed, third-party-context only. (Last updated 2025-10-27; inconsistent with the retirement post.)
38. **Google — User-Agent reduction.** https://privacysandbox.google.com/protections/user-agent — reduced UA on all resource requests; low-entropy UA-CH defaults; `Accept-CH` / `Critical-CH`; bot-detection FAQ.
39. **Chromium — User-Agent Reduction.** https://www.chromium.org/updates/ua-reduction/ — platforms covered; **iOS and Android WebView excluded.**
40. **Chrome Enterprise — UserAgentReduction policy.** https://chromeenterprise.google/policies/user-agent-reduction/ — **reduced by default since Chrome 110; deprecation trial ended after Chrome 113.**
41. **Google — Bounce tracking mitigations.** https://privacysandbox.google.com/protections/bounce-tracking-mitigations — 45-day interaction rule; launched by default **October 2023** but only for users who opted into blocking third-party cookies; federated auth/SSO/payments out of scope.
42. **Google — Private State Tokens.** https://developer.chrome.com/docs/privacy-security/private-state-tokens and https://privacysandbox.google.com/protections/private-state-tokens — formerly Trust Tokens; kept explicitly for anti-fraud.
43. **WebKit — "Private Browsing 2.0," Jul 16 2024.** https://webkit.org/blog/15697/private-browsing-2-0/ — **the canonical vendor statement of fingerprinting policy.** Five fingerprinting types; three privacy problems; "unique per website + new fingerprint on every fresh start"; canvas/WebGL readback noise in drawn regions only; audio noise persistent per buffer, normally distributed, adaptive, also for AudioWorklets; screen/window metric fixing; Link Tracking Protection; ODoH; per-tab Private Relay egress IPs.
44. **WebKit — "WebKit Features in Safari 26.0," Sep 15 2025.** https://webkit.org/blog/17333/webkit-features-in-safari-26-0/ — **"prevents known fingerprinting scripts from reliably accessing web APIs… screen dimensions, hardware concurrency, the list of voices available through the SpeechSynthesis API, Apple Pay payment capabilities, web audio readback, 2D canvas and more"**; blocks long-lived script-written storage; blocks query-param/`document.referrer` reads; **iOS UA OS-version freeze**; WebGPU ships; Safe Font Parser in Lockdown Mode.
45. **WebKit — Tracking Prevention policy and documentation.** https://webkit.org/tracking-prevention/ and https://webkit.org/tracking-prevention-policy/
46. **Mozilla — "Firefox expands fingerprint protections," Nov 10 2025.** https://blog.mozilla.org/en/firefox/fingerprinting-protections/ — **65% → 35% (Phase 1) → 20% (Phase 2) trackable**; "reduce the amount of Firefox users trackable by fingerprinters by half"; shipped in Private Browsing + ETP Strict, working toward default-on.
47. **Mozilla Support — Firefox's protection against fingerprinting.** https://support.mozilla.org/en-US/kb/firefox-protection-against-fingerprinting
48. **Mozilla — Resist Fingerprinting (RFP).** https://support.mozilla.org/en-US/kb/resist-fingerprinting
49. **Mozilla — `nsRFPService.h` (source).** https://searchfox.org/firefox-main/source/toolkit/components/resistfingerprinting/nsRFPService.h — the standardized UA values per platform (Windows 10 / OS X 10.15 / Android 10 / Linux X11) referenced by Tor's docs.
50. **BleepingComputer — "Mozilla Firefox gets new anti-fingerprinting defenses," Nov 10 2025.** https://www.bleepingcomputer.com/news/security/mozilla-firefox-gets-new-anti-fingerprinting-defenses/ — the Phase 2 specifics: canvas readback-only noise; local fonts blocked except language allowlist; `maxTouchPoints` → 0/1/5; screen height − 48; `hardwareConcurrency` → 2.
51. **Tor Project — "How Tor Browser protects you against browser fingerprinting."** https://support.torproject.org/tor-browser/features/fingerprinting-protections/ — letterboxing to 200×100 buckets; standardized UA per platform; "Tor Browser does not let users select which OS they appear to be. This is intentional: any option to choose would only make users more unique"; canvas extraction blocking; NoScript; first-party isolation.
52. **Tor Project — Tor Browser Design Doc.** https://gitlab.torproject.org/tpo/applications/wiki/-/wikis/Design-Documents/Tor-Browser-Design-Doc
53. **Tor Project — May 2025 newsletter** (and TB 14.0/14.5 release notes). https://newsletter.torproject.org/archive/2025-05-29-may-2025-tor-news/ — **OS spoofing in HTTP headers removed; headers now consistent with JS `navigator` values; the stated reason is entropy from the mismatch.**
54. **Brave — "Fingerprint randomization," Mar 5 2020 (updated Jun 2 2020).** https://brave.com/privacy-updates/3-fingerprint-randomization/ — farbling lineage from PriVaricator and FPRandom; per-session per-eTLD+1 seed; blocks canvas/WebGL for third parties by default.
55. **Brave — "Brave improves protections against GPU fingerprinting," Aug 13 2026.** https://brave.com/privacy-updates/38-webgl-webgpu-fingerprinting-protections/ — v1.93: WebGL vendor/renderer collapsed to one generic string; **WebGPU adapter descriptors emptied**; **per-session, per-eTLD+1, per-storage-area noise injected into the WebGL extension list**; WebGPU extension randomization planned. Notes `ANGLE (Apple, ANGLE Metal Renderer: Apple M5 Max)` as a live example and that the debug extension was originally exposed for Google Maps.
56. **Brave — Fingerprinting Protections wiki.** https://github.com/brave/brave-browser/wiki/Fingerprinting-Protections
57. **FingerprintJS — "Audio fingerprinting: What it is + how it works with Web API," Jul 9 2025.** https://fingerprint.com/blog/audio-fingerprinting/ — **the single most useful public document on fingerprint stability.** 5.4 bits / 713 values / 18,500 devices (DarkwaveTechnologies); audio "slightly to uniqueness but highly stable"; **Brave fudge-factor recovery and the un-farbled fingerprint distance table**; Safari 17 noise and the multi-sample+rounding bypass; Tor disables Web Audio; per-device fingerprinting latency (5–50 ms).
58. **FingerprintJS — "Browser fingerprinting techniques: 6 top methods explained," Jul 4 2025.** https://fingerprint.com/blog/browser-fingerprinting-techniques/ — 100+ signals combined with server-side analysis and ML; "identifiers are stable for months or even years, even after browser updates, when the user uses a VPN, or is in incognito mode"; "accuracy improves when fingerprints evolve over time."
59. **FingerprintJS — "We analyzed 23 billion device identification events," Mar 10 2026.** https://fingerprint.com/blog/device-intelligence-report-2026/ — **the best available base-rate data.** 23B+ identification events, 7B+ browsers/devices. VPN ~1 in 5 (1 in 3 Chromium desktop; iOS Safari 10% → 15%); browser tampering 4.4% of desktop (up from 2.6%), 4.8% Chromium desktop; **VMs 13% of Chromium desktop; devtools open 6% of desktop**; rooted Android 0.12%, jailbroken iOS 0.03%, Android emulators 0.37%, app cloning 0.16%, iOS MitM 0.07%; 67% of traffic in lowest Suspect Score bucket, 85% below 10, <4% above 20.
60. **FingerprintJS — "Canvas fingerprinting: What it is and how it works," May 8 2024.** https://fingerprint.com/blog/canvas-fingerprinting/ — canvas construction details ("Cwm fjordbank glyphs vext quiz", anti-aliasing, hinting, fallback fonts).
61. **FingerprintJS — "Bypassing Safari 17 audio fingerprinting protection."** https://fingerprint.com/blog/bypassing-safari-17-audio-fingerprinting-protection/
62. **DataDome (Vastel, A.) — "How New Headless Chrome & the CDP Signal Are Impacting Bot Detection," Jun 13 2024.** https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/ — **the definitive CDP-detection writeup.** `Runtime.enable` requirement; `Error.stack` lazy formatting; the `--auto-open-devtools-for-tabs` countermeasure; `nodriver` / Selenium-Driverless bypasses; recommendation of behavioural + reputational + weak signals alongside JS fingerprinting.
63. **Castle (Vastel, A.) — "How to detect Headless Chrome bots instrumented with Puppeteer?" Mar 13 2025.** https://blog.castle.io/how-to-detect-headless-chrome-bots-instrumented-with-puppeteer-2/ — three-tier detection (UA substring, `navigator.webdriver`, CDP side effect); `--disable-blink-features=AutomationControlled`; devtools false positive; residential proxies; AI CAPTCHA farms.
64. **Device & Browser Info (Vastel, A.) — "How to detect (modified, headless) Chrome instrumented with Puppeteer (2024 edition)," Jun 2024.** https://deviceandbrowserinfo.com/learning_zone/articles/detecting-headless-chrome-puppeteer-2024
65. **Device & Browser Info (Vastel, A.) — "Privacy leak: detecting anti-canvas fingerprinting browser extensions," Jun 29 2024.** https://deviceandbrowserinfo.com/learning_zone/articles/privacy-leak-detecting-canvas-countermeasures — pixel-deviation detection; Canvas Blocker's `getImageData`/`toBlob`/`toDataURL` Proxy wrapping; **extension ID recovery from `e.stack`**; ~20K Canvas Blocker users.
66. **Device & Browser Info (Vastel, A.) — "The role of weak (fingerprinting) signals in bot and fraud detection."** https://deviceandbrowserinfo.com/learning_zone/articles/role-of-weak-fingerprinting-signals — **the language-quintuple coherence pattern** and `hasConsistentLanguagesSignals` / `hasStrictConsistentLanguagesSignals`; strong vs weak signals; GPU-vs-UA-platform example.
67. **IETF — RFC 8828, "WebRTC IP Address Handling Requirements."** https://www.rfc-editor.org/rfc/rfc8828.html
68. **IETF — "Using Multicast DNS to protect privacy when exposing ICE candidates."** https://www.ietf.org/archive/id/draft-ietf-mmusic-mdns-ice-candidates-02.html and https://rtcweb-wg.github.io/mdns-ice-candidates/ — `<uuid>.local` host candidates.
69. **IETF RTCWEB 103 slides — mDNS ICE candidates.** https://www.ietf.org/proceedings/103/slides/slides-103-rtcweb-mdns-ice-candidates-00.pdf — "Chrome, Firefox and Edge do expose default route host candidates by default; Safari does not expose any host candidates by default."
70. **MDN — `Performance.now()`.** https://developer.mozilla.org/en-US/docs/Web/API/Performance/now — **resolution 5 µs cross-origin isolated, 100 µs otherwise**; sleep-ticking divergence across OSes. (Last modified Aug 25, 2025.)
71. **MDN — `NavigatorUAData.getHighEntropyValues()`.** https://developer.mozilla.org/en-US/docs/Web/API/NavigatorUAData/getHighEntropyValues — the high-entropy hint list; `ch-ua-high-entropy-values` Permissions-Policy gating; `NotAllowedError`. (Last modified Jan 26, 2026.)
72. **MDN — Client Hints guide.** https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Client_hints — low- vs high-entropy definitions.
73. **EFF — Cover Your Tracks methodology.** https://coveryourtracks.eff.org/about — the collected attribute list; uniqueness score derivation.
74. **EFF — Cover Your Tracks "Learn."** https://coveryourtracks.eff.org/learn — **the clearest plain-language statement of the inconsistency paradox**: "we _don't recommend_ trying to change a single element of your fingerprint"; the fake-Chrome-on-Safari worked example.
75. **AmIUnique — FAQ and global statistics.** https://amiunique.org/faq — the collected attribute list (note: the statistics page is client-rendered and does not expose data to a fetcher).
76. **W3C TAG — Unsanctioned Web Tracking.** https://www.w3.org/2001/tag/doc/unsanctioned-tracking/ — cited by Google as the rationale for UA reduction.
77. **W3C Privacy Principles.** https://w3ctag.github.io/privacy-principles/ — the "recognition across contexts" concept referenced by the bounce-tracking explainer.
78. **PrivacyCG — Bounce Tracking Mitigations explainer.** https://github.com/privacycg/nav-tracking-mitigations/blob/main/explainers/bounce-tracking-mitigations.md
79. **ACM (2025) — "Unveiling Privacy Risks in WebGPU through Hardware-based Device Fingerprinting."** https://dl.acm.org/doi/10.1145/3734477.3734700 — existence confirmed; bit figures not verifiable in open literature.
80. **CreepJS.** https://github.com/abrahamjuliot/creepjs — the de facto open test suite for browser-signal consistency and spoofing detection.
81. **Cloudflare / bot.sannysoft / scrapfly automation detectors** — referenced for the standard headless test-suite contents (plugins stub, permissions contradiction, missing codecs, software WebGL renderers, `--headless=new` fixes).

### Deliberately excluded

Several search results during this research were AI-generated SEO content recycling unverifiable numbers (e.g. "audio entropy exceeds 17 bits per sample", "canvas fingerprints cover 47,000+ device configurations", "WebGL fingerprinting remains consistent for 94.2% of users", "WebGPU entropy 14–16 bits"). These are **not** cited above. Where a widely-repeated number had no traceable primary source, it is listed in §8 as a research gap instead.
