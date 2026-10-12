# R2 — Commercial Device Fingerprinting / Identity-Anti-Fraud Products

Research report. Compiled Oct 2026. All claims attributed to vendor pages, docs, engineering blogs, research posts, regulator pages, or press as cited. Vendor accuracy/FP numbers are **marketing claims unless a methodology is published**; treat accordingly. Where a URL is a vendor page, it is the vendor's own claim, not an independent verification.

Environment note: search tooling was intermittent; most facts below come from direct fetches of primary sources (vendor docs, blogs, regulator pages, repo READMEs) plus Google News RSS where noted.

---

## 1. Fingerprint (fingerprint.com, formerly FingerprintJS Pro)

**Product shape.** Two-tier: `FingerprintJS` (Apache-licensed OSS, client-side hash) and `Fingerprint` / "Pro" (server-side identification + Smart Signals).

- Pro vs OSS comparison: https://fingerprint.com/github/
- OSS repo: https://github.com/fingerprintjs/fingerprintjs/

**Signals.**

- OSS: browser attributes only (screen, OS, device name, canvas, audio, fonts) — computed and hashed in-browser. ID lifetime "several weeks", collisions "common", ID origin = client (per https://fingerprint.com/github/).
- Pro: "100+ device, network, and behavioral signals" (https://fingerprint.com/blog/open-source-vs-fingerprint-pro-accuracy/). Raw Device Attributes (Enterprise plan) include font preferences, emoji rendering, font list, device_memory, timezone, canvas winding/geometry/text hashes, WebGL extensions/parameters, screen res, touch support, cookies/storage availability, audio hash, plugins, keyboard layout hash, battery level, IndexedDB, math hash, font hash — full reference at https://docs.fingerprint.com/docs/smart-signals-overview
- Smart Signals (browser + mobile): Suspect Score (weighted combination), Velocity Signals (5m/1h/24h counts of distinct IPs, countries, linked IDs per visitorID/linkedID/IP), IP Geolocation + ASN/datacenter, Proxy Detection (incl. residential-proxy ML score, `proxy_confidence`, provider e.g. BrightData), Tor/IP blocklist, High-Activity Device (top 2% by visit count per app), Raw Device Attributes, Proximity Detection (geolocation → coarse "proximity ID" via discrete global grid; 0-10 m to 1 km+ precision bands), Browser Bot Detection (`good`/`bad`, e.g. `bot_type: selenium`), Incognito, VPN (timezone_mismatch, public_vpn, os_mismatch via "proprietary network signature", relay, ml_prediction; explicit caveat: os_mismatch accurate for Windows, fails to capture OS in 10–15% of cases), Browser Tamper Detection (statistical anomaly score + antidetect-browser ML model; explicitly names AdsPower, DolphinAnty, OctoBrowser, GoLogin), Virtual Machine, Privacy-Focused Settings, Developer Tools Detection (detects CDP-controlled Chrome). Mobile: emulator, root/jailbreak, cloned app, factory reset, Frida, geolocation spoofing, MitM, tampered request, active call, iOS simulator.
- Source: https://docs.fingerprint.com/docs/smart-signals-overview

**Collection layer.** Browser JS agent (small, auto-updating CDN) + server-side API. Server does "fuzzy matching" and ML to dedupe/recover IDs; also uses "cryptographically signed cookies and local storage" plus TLS/IPv4/IPv6 server-side signals (https://fingerprint.com/github/). A customer can proxy the agent through their own subdomain to defeat ad blockers (docs: protecting the JS agent from adblockers; integrations exist for Cloudflare/Akamai/Fastly/CloudFront/Azure proxies).

**Identifier stability / accuracy claims.**

- Graph on https://fingerprint.com/github/: 99.5% correctly identified at 30 days, 99.0% @60d, 98.5% @90d, 98.0% @120d; "months/years" lifetime; "rare" collisions.
- Accuracy definition: "how many returning visitors we successfully identify as returning visitors" — https://fingerprint.com/resources/frequently-asked-questions-faqs/
- Public methodology for accuracy experiments (control = deterministic IDs, test = deterministic removed): https://docs.fingerprint.com/docs/identification-accuracy-and-confidence. Confidence score = 1 − falsePositiveProbability; new/deterministic matches score 1.0.
- No published false-positive rate for Smart Signals beyond per-signal confidence/ML thresholds (e.g., tampering actionable >0.8; proxy ML >0.6).

**Storage model.** Customer-selected data region (US Richmond VA / EU Frankfurt / AP Mumbai); retention 7 days (Free), 30 days (Pro Plus), 90 days (Enterprise); GDPR/CCPA/SOC 2 Type II; no PII beyond IP (pseudonymizable), URL (hashable on request). Pricing page: https://fingerprint.com/pricing/

**Pricing.** Free: 1,000 API calls/mo. Pro Plus: $99/mo for 20K calls, then $4 per 1,000; Android calls free up to 500K/mo. Enterprise custom (99.9% SLA, more signals). 5 RPS cap on self-serve. Surge/DDoS protection and WAF filtering on billing. Source: https://fingerprint.com/pricing/

**Uptime/latency.** 99.9% SLA claim; status page shows a 99.944% trailing-year figure for US Identification with several 2025–2026 degradations and a July 2026 "Bot Detection False Positives" incident: https://status.fingerprint.com

**Breach/leak history.** No public breach of Fingerprint itself surfaced in this research. Fingerprint's own marketing/blog is the main accuracy source (self-reported). No independent academic benchmark found.

**Criticism.** EFF-class critique applies to any fingerprinting (see §5). Fingerprint's own privacy position: "primarily designed for fraud detection, which generally does not require consent" — a claim regulators dispute in the EU/UK context (see §5). Their blog openly markets detecting privacy browsers ("privacy settings" signal) and incognito, which privacy advocates read as anti-user.

---

## 2. Castle

**Product shape.** Device intelligence + risk rules engine for account abuse (ATO, fake accounts, multi-accounting, promo abuse, SMS pumping).

- https://castle.io/product/devices ; docs https://docs.castle.io/docs/device-fingerprinting

**Signals.** Device fingerprint, user agent, IP geolocation/proxy/Tor/datacenter, carrier, system uptime, incognito, adblocker detection, headless-browser signatures (Puppeteer/Playwright/Selenium), mouse/keyboard behavioral analysis, bot score 0–100. Rules engine + analytics; backend SDK.

**Collection layer.** Client JS ("stealth mode" deliberately circumvents ad blockers and privacy plugins) + server-side API. "Backend SDK responds in 150 milliseconds, regardless of scale."

**Claims.** "Up to 99.5% accuracy" identifying returning users, "0.001% collision rate", "100% coverage" vs ad blockers. Self-published. https://castle.io/product/devices

**Notable research.** Castle Research publishes fingerprint-harvesting ecosystem analysis (April 2026): 811 bot/fraud-adjacent sites measured, 678 reachable; 12.5% loaded fingerprinting-related scripts; evidence of harvesting structured to mirror commercial anti-bot vendors (PerimeterX, Incapsula, Akamai, Adyen, hCaptcha); operators willing to pay up to $5 per 1,000 real fingerprints; Bablosoft `clientsafe.js` still live on third-party sites (Group-IB previously found it injected into 115+ e-commerce sites). https://castle.io/research/fingerprint-harvesting-in-the-bot-ecosystem — This is one of the best public sources on attacker replay economics.

**Storage/breach.** SOC 2 Type II, GDPR-ready; no public breach. Status page: https://status.castle.io/

**Pricing.** Free tier + transparent-ish usage pricing (https://castle.io/pricing); exact per-event price not fetched.

---

## 3. Sift

**Product shape.** Digital trust & safety platform: payment fraud, account takeover, fake accounts. 700+ brands claimed.

- https://sift.com/platform/account-defense/ ; docs https://developers.sift.com/docs

**Signals.** "16,000+ fraud signals" per the Sift Score API post (https://sift.com/blog/sift-score-api-for-internal-models/); device intelligence + behavioral patterns + network intelligence ("1T+ annual events"). Device ID + IP analysis explained in https://sift.com/blog/device-ip-analysis/ (specificity-vs-persistence tradeoff; IP as one signal among many).

**Collection layer.** Server-side API + SDKs (client SDK for device/behavioral data; Sift Score returned to customer rules or used inside Sift workflows).

**Identifier stability.** No published accuracy %. Sift frames device ID as probabilistic and warns about specificity/persistence tradeoff explicitly (honest positioning; contrast with Fingerprint's stability graph).

**Storage/breach.** No public breach found. Status: https://status.sift.com/

**Pricing.** Not public; enterprise sales.

---

## 4. Sardine

**Product shape.** Agentic fraud platform: onboarding, ATO, payment fraud, policy abuse; consortium data.

- https://www.sardine.ai/device-and-behavior

**Signals.** Device fingerprint (hardware/software/sensors/environment), emulator/root/jailbreak detection, tap pressure, SIM slots, device motion, "impossible location" checks (GPS vs IP vs telemetry), remote-access-tool detection, bot/automation detection (claims to outperform Google Invisible reCAPTCHA), deepfake/spoofed-camera detection, copy/paste-in-sensitive-field monitoring, "Patented True Piercing™" for VPN/proxy/residential-IP de-obfuscation. "1,000+ risk features" derived from device+behavior signals; "6.5B+ profiled devices" consortium.

**Collection layer.** One lightweight SDK for device + behavior; ML scoring server-side; consortium enrichment cross-merchant.

**Claims.** "Persistent device identity across resets, reinstalls, and incognito sessions." No published FP rate; a Prezzee case study cites +70% approval rate. Source: https://www.sardine.ai/device-and-behavior

**Pricing.** Not public. Status: https://status.sardine.ai/

---

## 5. Incognia (network/geolocation-first — "device + location")

**Product shape.** Cross-device user recognition from device signals + precise location behavior + tamper detection. Strong in food delivery, ride-sharing, marketplaces, fintech.

- https://www.incognia.com/device-intelligence ; https://www.incognia.com/incognia-id

**Signals.** Proprietary device fingerprint + location behavior; tamper detection list: device model, root/jailbreak, emulator, GPS spoofing, factory reset; app tampering (debugging mode, code injection, data mismatch, app cloner, Frida); web/browser tampering (incognito, geolocation tampering, privacy browsers, IP-to-location mapping, VPN/proxy, bot detection). AI/Transformer models for browser ID ("85+ integrated signals", "200 ms", "0.0002% false-positive rate"). Mobile: "Over 99.9% accurate user recognition", "0.0001% false-positive rate", "17x more accurate than Face ID" (vendor marketing), "99.999% accurate user identification" in results table.

**Collection layer.** Mobile SDK + browser SDK + server. Location data is the differentiator: repeated visits to home/work anchor a location fingerprint that persists across device ID resets (reinstall-proof), per customer testimonials on the page and Incognia's patent/press history.

**Critique angle.** Most privacy-sensitive of the group (precise location + cross-device linking). Delivery-app TOS enforcement use (ban evasion) is explicit: https://www.incognia.com/ride-sharing

**Pricing.** https://www.incognia.com/pricing (not fetched in detail); enterprise sales.

---

## 6. SEON

**Product shape.** Fraud prevention + KYC/AML; device intelligence as one module.

- https://seon.io/products/device-intelligence-solution/ ; explainer https://seon.io/resources/device-fingerprinting/

**Signals.** "1,100+ real-time signals." Cookie hash, browser hash, device hash (True Device ID); hardware anomalies (0x0 screen, 0% battery, overheating, screen mirroring, unsecured keyguard); remote-access/screen-sharing detection; "on-call and carrier status analysis" (detects user on a phone call during account edits); residential-proxy detection; cross-account link analysis via graphs; velocity/relationship signals.

**Collection layer.** Lightweight JS collector or mobile SDK + server-side risk engine; persistent identifiers stored server-side.

**Claims.** Persistence claim: "does not matter if a user clears cookies... factory resets their device. The persistent identifiers in a true device fingerprint ensure the same device ID propagates through" (SVP Product quote on page). No published FP rate. GDPR positioning: legitimate interest (Art. 6(1)(f)) for fraud prevention, no consent (same argument Fingerprint makes; regulators dispute for tracking, accept more readily for fraud — see §5).

**Breach.** No public breach found. Status: https://status.seon.io/ ; trust center: https://trust.seon.io/

**Pricing.** https://seon.io/pricing/ (tiered; not fetched).

---

## 7. IPQualityScore (IPQS)

**Product shape.** Cheapest, most self-serve: IP/email/phone/device fraud scoring APIs.

- Device API docs: https://www.ipqualityscore.com/documentation/device-fingerprint-api/overview
- Product: https://www.ipqualityscore.com/device-fingerprinting

**Signals.** JS tag collects: `device_id` (hash), `guid` + `guid_confidence`, `canvas_hash`, `webgl_hash`, `ssl_fingerprint` (TLS fingerprint), `fraud_chance` 0–100, is_crawler, proxy/vpn/tor, connection_type, ISP, timezone, recent_abuse, bot_status, geo, OS/browser/model/brand, `high_risk_device`, `active_vpn`, `active_tor`. Also emulator/GPS-spoof detection, mobile SDK (iOS/Android/Flutter/Unity), Fraud Fusion consortium.

**Collection layer.** Client JS tag (`Startup.Init()` pattern) + REST API; mobile SDK.

**Claims.** "Hundreds of data points scored"; "millions of patterns"; no published FP rate. `guid_confidence` is the closest thing to a confidence score.

**Pricing.** Free 1,000 lookups/mo; entry paid ~$99/mo for 5,000 (per third-party summaries e.g. https://apis.io/plans/ipqualityscore/ipqualityscore-plans-pricing/ and https://www.ipqualityscore.com/plans). One of the few with an on-prem IP reputation DB option.

**Breach.** No public breach found.

---

## 8. Kount / Equifax

**Product shape.** Kount (Kount 360) was a payments-fraud device fingerprinting pioneer; **Equifax acquired Kount for $640M, deal closed Feb 2021** (https://investor.equifax.com press release "Equifax Closes Acquisition of Kount", Feb 11, 2021; price per PYMNTS/IBS Intelligence coverage Jan 2021). Now sold as Equifax Identity & Fraud products: https://www.equifax.com/business/identity-fraud/

**Signals / mechanism.** Classic Kount: device fingerprint built from browser/device attributes, **anchored by a persistent first-party cookie** ("Kount's persistent cookie") plus network/velocity/behavior, producing a Kount Score. Current Equifax positioning adds synthetic identity risk, credit-abuse scores, identity proofing, behavioral insights, orchestration. Equifax claims: 22.4M US applications screened in 2024, 4% synthetic; 96.4M losses deterred from confirmed synthetic identities in 2025 (vendor stats).

**Collection layer.** Merchant-integrated JS + server-side; deep payment-processor integrations (card networks, AVS/CVV, order data). Enterprise-first; historically the "boring but embedded" incumbent.

**Breach history.** Kount itself: none public. **Parent-company context: Equifax 2017 breach (147M consumers)** — the corporate owner has the worst breach record of any vendor here, relevant when a merchant asks "who holds my device-risk data": https://www.equifaxsecurity2017.com/

**Pricing.** Enterprise, quote-based. Kount support docs are behind login (https://support.kount.com/).

---

## 9. Bot management layer

### 9.1 Cloudflare Bot Management

- Docs: https://developers.cloudflare.com/bots/concepts/bot-detection-engines/ ; bot score: https://developers.cloudflare.com/bots/concepts/bot-score/
- Engines: **Heuristics** (all requests; known malicious fingerprints), **JavaScript Detections** (headless detection via invisible JS injection; optional), **Machine Learning** (Business/Enterprise; "majority of detections"; supervised model outputs Bot Score 1–99 as P(human)), **Anomaly Detection** (Enterprise, unsupervised; being deprecated). `__cf_bm` cookie smooths scores across a session. Heuristics give deterministic detections score 1; ML produces most 2–99.
- Accuracy disclosure: Cloudflare publishes per-heuristic FP rates. Golang-fingerprint heuristic: 0.01% FP before adding network context; **0.0001% FP (1 in 1M) after narrowing to abusive networks** — https://blog.cloudflare.com/bots-heuristics/. Bot Feedback Loop (Enterprise) lets customers report FP/FN to train the next model: https://developers.cloudflare.com/bots/concepts/feedback-loop
- ML v8 targets residential-proxy bots: https://blog.cloudflare.com/residential-proxy-bot-detection-using-machine-learning/
- Pricing: Enterprise (Bot Management); Turnstile/Managed Challenge available lower.

### 9.2 DataDome

- Product: https://datadome.co/products/bot-protection/ ; pricing: https://datadome.co/pricing/
- Layers: server-side + client-side signals ("getting rid of client-side signals (~30-40% of all signals) is detrimental" — https://datadome.co/bot-management-protection/why-client-side-signals-are-a-must-have-for-detecting-sophisticated-attacks/), **TLS fingerprinting** (JA3-style; models score fingerprints by %-bots, IP quality, OS/browser consistency — https://datadome.co/engineering/how-tls-fingerprinting-reinforces-datadomes-protection/), HTTP/2, behavioral, Device Check (invisible challenge), optional CAPTCHA for <0.01% of requests, 1000+ models, 5T signals/day.
- Claims: **<0.01% false-positive rate** ("1 in 10,000 legitimate requests"), **<2 ms** mitigation, 35+ PoPs, 99.9% availability. CEO blog argues zero-FP claims are impossible and publishes the metric discipline: https://datadome.co/bot-management-protection/why-measuring-false-positives-and-negatives-is-the-only-way-assess-ai-bot-protection
- Pricing (rare transparency): Essentials $3,830/mo (<100M req), Advanced $8,670/mo (<200M), Premium $10,160/mo (<300M), Enterprise from $13,270/mo (>300M).
- CDP research (key for evasion section): https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/

### 9.3 Kasada

- Product: https://www.kasada.io/bot-defense ; adversarial techniques: https://www.kasada.io/adversarial-techniques
- Architecture: invisible client-side sensors ("hundreds of sensors"), client validation (tamper checks on the payload), **Proof of Execution / proof-of-work in a highly obfuscated polymorphic VM** (Polyform), server-side anomaly detection "<2ms", rapid sensor rollout "in minutes across entire customer base". No CAPTCHAs by design; blocks with 403/429.
- Published self-test table (https://www.kasada.io/blog/mastery-of-the-puppets-advanced-bot-detection): vs a 560-node botnet — WAF only: 5% detection, 100K hits/hr, 24K accounts/day; PoW only: 99.5%, 500 hits/hr; PoW+fingerprint: 100%, 0 hits.
- Adversary economics (their page): solver services **$0.80–$3.50 per 1,000 solves**; headless setups $20–$500/mo; residential proxies; CAPTCHA bypass ~$0.50/1K; ML poisoning; de-obfuscation.
- Scale/funding: ~$300M valuation (AFR Dec 2025); $20M raise (SecurityWeek Feb 2026).
- Bypass notes: tokens `x-kpsdk-ct` + single-use `x-kpsdk-cd`; escalation of PoW difficulty over time; Ticketmaster-class deployments remain hard (https://www.browserless.io/blog/kasada-bypass, https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf).

### 9.4 HUMAN Security (White Ops + PerimeterX + Distil)

- Buyers guide: https://www.humansecurity.com/bot-management-buyers-guide
- Scale claims: **2,500+ signals per interaction**, 400+ algorithms/adaptive ML models, 20T+ interactions/week, 3B unique devices. Multilayer: ML + behavioral + fingerprinting + threat intel; Human Challenge (CAPTCHA alternative); Precheck; Threat Tracker; AgenticTrust for AI agents. Ad-fraud side was first MRC-accredited for pre- and post-bid IVT detection (https://www.humansecurity.com/newsroom/white-ops-is-the-first-company-to-receive-mrc-accreditation-for-both-pre-bid-and-post-bid-invalid-traffic-detection-and-mitigation-across-desktop-mobile/).
- Docs list concrete bot characteristics incl. "Signal Manipulation" (tampering with sensor output): https://docs.humansecurity.com/applications/sightline-bot-characteristics
- Pricing: enterprise-only (third-party summary: https://wafplanet.com/waf/human-security/).

### 9.5 Akamai Bot Manager

- Product (archived fetch, since akamai.com 403s datacenter fetches): https://web.archive.org/web/20251231213510/https://www.akamai.com/products/bot-manager
- Detection: AI models for behavior + browser fingerprinting + "cleanest data based on billions of bot requests and logins daily"; script injection into monitored pages for behavioral anomaly detection; **Bot Score 0 (human) → 100 (bot)**, visible from first request, increasing with request count; three response segments (Cautious/Strict/Aggressive); patented tech; visibility into **40B+ bots/day**. TLS fingerprinting research: https://www.akamai.com/blog/security/bots-tampering-with-tls-to-avoid-detection (Akamai correlates tampered-TLS bot traffic across customers).
- Third-party reverse engineering: `_abck` cookie + `sensor_data` payload (https://scrapfly.io/blog/posts/akamai-bot-manager-understanding-abck-cookies-and-sensor-data); 5-layer chain IP rep → TCP/IP (p0f) → JA3/JA4 → sensor payload → behavior (https://voidmob.com/blog/how-to-bypass-akamai-bot-detection-2026).
- Pricing: enterprise.

### 9.6 AWS WAF Bot Control

- Feature: https://aws.amazon.com/waf/features/bot-control/ ; rules: https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-bot.html ; pricing: https://aws.amazon.com/waf/pricing/
- Two managed tiers: **Common** (static request analysis, self-identifying bots, verified-bot handling) and **Targeted** (behavior-based detection for evasive bots; adds targeted signals). Actions: allow/block/count/challenge/CAPTCHA, scope-down statements. Fraud Control rule groups: Account Takeover Prevention + Account Creation Fraud Prevention (login/signup abuse, credential stuffing).
- Pricing: $5/WebACL/mo + $1/rule or rule group/mo + $0.60/M requests; **Bot Control $10/mo + $1/M requests (first 10M free)**; **Targeted $10/M requests (first 1M free)**; CAPTCHA attempts $0.40/1,000; Fraud Control $10/mo per rule group + tiered request fees ($1,000/M for first 2M down to $50/M at high volume). One of the few vendors with fully public per-request economics.

### 9.7 Arkose Labs

- Platform: https://www.arkoselabs.com/ ; Device ID: https://www.arkoselabs.com/arkose-device-id
- Layers: adaptive risk-based challenges (Arkose MatchKey: per-round key image — the "correct answer" is defined by a key image shown each round, so class-based solvers can't generalize; 1,250+ variants claimed; 109 languages), **Proof of Work** (crypto tasks scaled by request volume, marketed as "invisible security" and threat-intel generator: https://www.arkoselabs.com/blog/proof-of-work-invisible-security-visible-results), detection funnel + Arkose Titan platform, Device ID (AI similarity + deterministic, `arkose_id`, `confidence_score` shown as 99.99 in sample payload, `device_first_seen`, risk score/band/insights, stateful + stateless IDs; 100M+ devices/day analyzed). Claims 41% ATO reduction (case study quote) and 99.4% credential-stuffing stop (Zilch case study).
- Cost model: per-challenge/session, enterprise; historically "solves" cost attackers $ per challenge (see Kasada's solver-market pricing for the buy side).

---

## 10. Anti-detect browsers and bypass tooling

### 10.1 Commercial anti-detect browsers

- **Multilogin** (Mimic/Stealthfox engines): spoofs Canvas, WebGL, AudioContext, fonts, WebRTC, timezone, UA, screen, hardware/OS-level signals; REST API + Puppeteer/Selenium; built-in residential proxies; Android cloud phones billed $0.011/min (https://multilogin.com/pricing, https://multilogin.com/mobile/cloud-phone/). Independent-ish benchmark (Oct 2026, published by competitor GoLogin, includes their own fails): **Multilogin failed Iphey** (inconsistent fingerprint, MX 90) though it passed Pixelscan + CreepJS; 7.4 s launch. https://gologin.com/blog/anti-fingerprinting-browser/
- **GoLogin** (Orbita engine): basic spoofing set — WebRTC, Canvas, Timezone, UA, Fonts, Resolution (per Multilogin's comparison: https://multilogin.com/blog/adspower-vs-gologin/). Passed all three checkers in the Oct 2026 test (Chrome 152, 2 versions behind stable; "like headless" 31% vs 25% for others).
- **AdsPower** (SunBrowser/FlowerBrowser): "20+ options and 50+ customizable parameters", full fingerprint customization + JS masking + RPA (https://www.adspower.com/antidetect-browser). **Failed Pixelscan (timezone spoofed: America/Anchorage on a South Carolina proxy) and Iphey (MX 70)** in Oct 2026 test — default timezone mismatch is exactly the kind of inconsistency fraud systems flag.
- **Kameleo**: mobile browser emulation on desktop; failed to start in Oct 2026 test; previously failed location checks. https://gologin.com/blog/kameleo-browser-alternative-review/
- Other named in the same test: Octo Browser, Hidemium, 1Browser, MoreLogin, GeeLark, Dolphin Anty, NstBrowser, Incogniton. Incogniton was the worst: proxy + timezone flagged, and **CreepJS caught audio fingerprint spoofing** plus a Russian system TTS voice leaking in a US profile.
- General 2026 anti-detect leaks documented there: US IP + Alaska timezone; US profile + Russian system voice; Spanish language on US proxy; engine versions 142–154 (outdated engine = smaller, more suspicious crowd); TLS/HTTP2 handshake vs claimed browser mismatch; "trust scores replace hard blocks" (slow decay instead of instant bans).

### 10.2 Open-source / framework bypass

Mechanisms, leaks and what each tool concedes (rebrowser-patches, Patchright, Camoufox, undetected-chromedriver, nodriver/Selenium-Driverless): R2-evasion-counters §1.2 and §4.2. Facts this survey adds:

- **rebrowser-patches** claims "undetectable by Cloudflare or DataDome" in its own tests; last fully-tested versions Puppeteer 24.8.1, Playwright 1.52.0 (April/May 2025).
- **Patchright**: an independent benchmark (ianlpaterson.com, 2026, "7 tools, 651 verdicts") is more mixed than its README — Camoufox and Patchright tied; rebrowser patches changed no outcomes on the tested targets — https://ianlpaterson.com/blog/anti-detect-browser-benchmark-patchright-nodriver-curl-cffi/
- **Camoufox** (https://camoufox.com/stealth/) samples market-share-realistic fingerprints via BrowserForge and ports HumanCursor motion. Self-criticism: a year without maintenance; "performance down due to base Firefox version and newly discovered fingerprint inconsistencies"; anti-bot vendors "test Camoufox over and over to find even 1 unique inconsistency."
- **undetected-chromedriver** is now largely superseded by nodriver/Selenium-Driverless.
- **puppeteer-extra-plugin-stealth**: the original evasion collection; DataDome's detection post: https://datadome.co/bot-management-protection/detecting-headless-chrome-puppeteer-extra-plugin-stealth/. Kasada prices stealth-plugin setups at $20–$500/mo in their adversary catalog.

### 10.3 The harvesting/replay economy (the real bypass)

From Castle's research and F5/Group-IB history:

- **Genesis Marketplace** sold ~323,000 compromised browser environments bundled with fingerprints + a Chromium extension to replay victim device attributes, paired with residential proxies (F5 Labs; Europol takedown).
- **Bablosoft**: BrowserAutomationStudio + FingerprintSwitcher + **PerfectCanvas** (render canvas on a real remote machine, inject output locally to defeat canvas antifraud) + `clientsafe.js` harvesting (§2).
- Market price for real fingerprints: §2; forum advice: deploy a collector on a high-traffic site (or buy ads) to harvest clean fingerprints.
- Castle's recommendation: treat fingerprint data as untrusted client input; add controlled variability in client logic/payload structure; validate cross-context consistency; combine with behavioral/network/server-side telemetry.

---

## 11. Privacy-tech critiques and enforcement

**EFF.** Cover Your Tracks (ex-Panopticlick) documents that browser fingerprinting is "difficult to detect and extremely difficult to thwart", that deleting cookies doesn't help, and that realistic protection is Tor Browser or blocking + NoScript; unique browsers enable dossiers without cookies. https://coveryourtracks.eff.org/about

**Brave.** Long-running fingerprinting defenses: randomization/farbling (2020: "Fingerprint randomization", "Fingerprinting defenses 2.0"), language randomization (2022), GPU/WebGL vendor de-identification + extension noise (Privacy Update #38, Aug 2026: https://brave.com/privacy-updates/38-webgl-webgpu-fingerprinting-protections/). Note the **simplification**: Jan 2024 Brave sunset its Strict fingerprinting mode (#28) arguing randomization made it redundant. Critiques exist: the 2025 paper "Breaking the Shield: Analyzing and Attacking Canvas Fingerprinting Defenses in the Wild" claims statistical analysis defeats farbling and nine extensions "with little inconvenience" (summary: https://www.brside.com/blog/brave-firefox-safari-only-two-survived-this-fingerprinting-test).

**Mozilla.** Firefox expanded fingerprinting protections (Phase 2, announced Nov 10, 2025, Tom Ritter): drawing on a global analysis of real fingerprintable browsers, Mozilla claims the share of Firefox users trackable by fingerprinters is **halved**; initially Private Browsing + ETP Strict, later default. Firefox 145 added protections against font, hardware (cores, touch points, dock/taskbar dimensions) leaks. https://blog.mozilla.org/en/firefox/fingerprinting-protections ; feature page https://www.firefox.com/en-US/features/block-fingerprinting

**CNIL (France).** Position: fingerprinting is a tracking technology subject to consent rules. The 2021 position paper (courtesy translation) states fingerprinting "aims to uniquely identify a user... to track them in a similar way to cookies" and that profiling/targeted-advertising operations based on terminal access "require the prior consent of the user... regardless of the technique used." https://www.cnil.fr/en/alternatives-third-party-cookies-what-consequences-regarding-consent
Recent enforcement sample (not fingerprinting-specific but the same consent/legit-interest boundary): **CNIL fined a loyalty program operator €3.5M (decision Dec 30, 2025)** for transferring member emails/phones to a social network for ad targeting without valid consent, plus cookies placed before choice and not deleted on refusal. https://www.cnil.fr/en/transfer-data-social-network-advertising-purposes-cnil-imposed-fine-eu35-million

**ICO (UK).** Sept 11, 2025: the ICO's storage-and-access rules apply to any storage/access on a device, **"like device fingerprinting, where they involve storage or access"**; the "strictly necessary" exemption is judged from the user's perspective and does not cover ad revenue; if PECR requires consent you cannot fall back on legitimate interests. https://ico.org.uk/about-the-ico/media-centre/news-and-blogs/2025/09/fact-vs-fiction-ico-debunks-myths-on-storage-and-access-technologies/
ICO online-tracking strategy 2025 + a 2026 development: "Google Activates IP Fingerprinting for EU Ads as ICO Warns Publishers on Consent" (Tech Times, June 19, 2026 — headline-level; not independently verified here).

**FTC (US).** No action against a fingerprinting vendor per se; the FTC has been aggressive on **location data brokers**, which is the adjacent data economy fingerprint vendors feed: first-of-kind settlement with **X-Mode/Outlogic** banning sale of sensitive location data (Jan 2024; TechCrunch/Reuters/The Record coverage), proposed order against **Gravy Analytics/Venntel** (Dec 3-4, 2024; DataGuidance/The Record/NBC), and the ongoing **FTC v. Kochava** litigation. One of the brokers, Gravy Analytics, was then breached in Jan 2025 ("17TB allegedly stolen"; reported to Norwegian authorities — NBC News, The Record, Malwarebytes). Lesson for fingerprint vendors: the liability and breach risk sits in the data-retention model, not just the collection method.

**Lawsuits naming fingerprinting vendors.** No vendor-specific class action against Fingerprint/Castle/Sift etc. surfaced in this research (searches of news + class-action trackers). What exists is the broad 2023–2024 wave of web-tracking/privacy class actions against **deployers** (pixels, session replay, chatbots) under state wiretap laws and BIPA for actual biometrics — overview: https://www.wilmerhale.com/en/insights/blogs/wilmerhale-privacy-and-cybersecurity-law/20250225-year-in-review-2024-web-tracking-litigation-and-enforcement

---

## 12. Comparison table

| Vendor                                                             | Primary use                     | Key signals                                                                                                      | Collection layer                                                   | Accuracy claim (self)                              | FP rate (self)                                   | Pricing model                                     |
| ------------------------------------------------------------------ | ------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | -------------------------------------------------- | ------------------------------------------------ | ------------------------------------------------- |
| **Fingerprint**                                                    | Device ID + fraud signals       | 100+ browser/network/behavioral; 20+ Smart Signals (bot, VPN, proxy, incognito, tamper, VM, velocity, proximity) | Browser JS + server-side ML/fuzzy matching; cookie+storage anchors | 99.5% @30d; 98% @120d (graph)                      | None published (per-signal confidence only)      | Free 1K/mo; $99/mo 20K, $4/1K overage; enterprise |
| **Castle**                                                         | ATO/fake accounts               | Fingerprint, bot score 0-100, IP/proxy/Tor, incognito, adblock, headless, mouse/keyboard                         | Client JS (stealth, adblock-proof) + API (150 ms)                  | "Up to 99.5%"                                      | "0.001% collision rate"                          | Free tier + usage pricing                         |
| **Sift**                                                           | Payments + ATO                  | 16,000+ signals, device ID, behavioral, network (1T events/yr)                                                   | SDK/API + server                                                   | None published                                     | None published                                   | Enterprise quote                                  |
| **Sardine**                                                        | Fintech onboarding/ATO/payments | Device+behavior SDK, emulator/root, True Piercing proxy piercing, deepfake, consortium                           | Single SDK + server ML + consortium                                | None published                                     | None published                                   | Enterprise quote                                  |
| **Incognia**                                                       | Cross-device ID via location    | Device + precise location behavior + tamper (GPS spoof, Frida, clone, factory reset); 85+ web signals            | Mobile/browser SDK + server (AI/Transformer)                       | 99.9%+ user recognition; 17x Face ID (mktg)        | 0.0001% (mobile), 0.0002% (browser)              | Enterprise quote                                  |
| **SEON**                                                           | Fraud + KYC/AML                 | 1,100+ signals: device hash, emulator anomalies, remote access, on-call, residential proxy, graph links          | JS collector/SDK + server                                          | "True Device ID" persists resets                   | None published                                   | Tiered self-serve                                 |
| **IPQS**                                                           | Cheap fraud scoring             | canvas/webgl hash, TLS (`ssl_fingerprint`), guid_confidence, fraud_chance, VPN/proxy/Tor, emulator               | JS tag + REST + mobile SDK                                         | "Hundreds of data points"                          | None published                                   | Free 1K; ~$99/mo entry                            |
| **Kount/Equifax**                                                  | Payments fraud (legacy)         | Device fingerprint + persistent cookie, velocity, order data; now synthetic-ID/credit scores                     | JS + server; processor integrations                                | Vendor stats on synthetic IDs                      | None published                                   | Enterprise quote                                  |
| **Cloudflare**                                                     | Edge bot mgmt                   | Heuristics, JS detections, ML Bot Score 1-99, `__cf_bm`                                                          | Edge (all requests) + client JS                                    | Per-heuristic FP published                         | Golang heuristic 0.0001% after network narrowing | Enterprise; Turnstile lower tiers                 |
| **DataDome**                                                       | Bot + account fraud             | 5T signals/day, TLS/JA3, HTTP/2, client+server, Device Check, 1000+ models                                       | Edge PoPs + client JS                                              | —                                                  | **<0.01%** (1 in 10K)                            | $3,830–$13,270+/mo                                |
| **Kasada**                                                         | Bot defense                     | Hidden sensors, client validation, PoW VM (Polyform), server anomaly <2ms                                        | Client JS + server                                                 | 99.5–100% vs own test botnet                       | Claims no CAPTCHA friction                       | Enterprise                                        |
| **HUMAN**                                                          | Bot/fraud/ad fraud              | 2,500+ signals/interaction, 400+ models, 20T interactions/wk                                                     | Client JS + edge/server                                            | MRC-accredited IVT (ad side)                       | None published                                   | Enterprise                                        |
| **Akamai**                                                         | Bot mgmt at edge                | Bot Score 0-100, script sensor, TLS/p0f, 40B+ bots/day                                                           | Edge + injected JS                                                 | —                                                  | None published                                   | Enterprise                                        |
| **AWS WAF Bot Control**                                            | AWS-native bot mgmt             | Static + behavior (Targeted), verified bots, Fraud Control for ATO/signup                                        | Edge (CloudFront/ALB/APIGW)                                        | —                                                  | None published                                   | $10/mo + $1–$10/M req; CAPTCHA $0.40/1K           |
| **Arkose**                                                         | Challenge-based bot/ATO         | MatchKey per-round key images, Proof of Work, Device ID (arkose_id, risk score)                                  | Client challenge + server                                          | 41% ATO drop (case study)                          | "No false positives" (marketing)                 | Per-challenge enterprise                          |
| **Anti-detect browsers** (Multilogin, GoLogin, AdsPower, Kameleo…) | Evasion                         | Canvas/WebGL/audio/fonts/WebRTC/timezone/UA spoofing; cloud phones                                               | Modified Chromium/Firefox builds                                   | "Undetectable" (vendor)                            | Oct 2026 test: 3/12 passed 3 checkers            | $4.50–$59+/mo; cloud phone per-minute             |
| **OSS bypass** (Patchright, Camoufox, rebrowser, uc)               | Evasion                         | Runtime.enable leak fix, native-level spoofing, isolated worlds                                                  | Patched drivers/browsers                                           | Patchright: passes many checkers; benchmark: mixed | —                                                | Free                                              |

---

## 13. What to borrow / what to avoid — for a self-hosted, security-only, privacy-respecting fingerprinting system

### Borrow (high value, low risk)

1. **Hybrid client + server architecture, server-side matching.** Fingerprint's core insight: collect raw signals client-side, but do dedupe/merging/recovery server-side with fuzzy matching + confidence scoring. A self-hosted system should never let the client compute the final ID. (docs.fingerprint.com/docs/identification-accuracy-and-confidence)
2. **Explicit confidence score per identification**, defined as 1 − false-positive probability, and _published methodology_ (control set = deterministic IDs; test set = deterministic stripped). Copy this measurement design — it's the only way to know your own FP/FN.
3. **Signal taxonomy worth copying:** velocity aggregates (distinct IPs/countries/linked accounts per ID at 5m/1h/24h), incognito flag, timezone-vs-IP mismatch, public-VPN/datacenter-ASN/Tor flags, high-activity device (top-2% threshold computed per deployment), headless/bot flag, tampering/anomaly score vs population. These are cheap and high-signal.
4. **TLS/JA3-JA4 + HTTP/2 fingerprinting at the edge** (DataDome/Akamai model): server-observable, no client cost, catches UA/TLS inconsistency. Pair with IP/ASN reputation. Not available on this stack: Traefik terminates TLS so the app never sees a ClientHello, and Cloudflare exposes JA4 only with Enterprise Bot Management (R2-network-transport).
5. **Behavioural signal separation:** treat bot score and fraud risk as separate outputs (Fingerprint and DataDome both do); don't conflate "automated" with "malicious".
6. **Cross-context consistency checks:** compare main-world vs worker/iframe values; detect overwritten getters (`getOwnPropertyDescriptor`, `toString()` not `[native code]`); detect `Runtime.enable`/CDP artifacts (DataDome's Error.stack technique; rebrowser's leak list). This catches anti-detect tooling cheaply.
7. **Treat client data as untrusted and expect replay** (Castle): rotate/vary payload schemas, add server-side corroboration (TLS, timing, navigation graph), never make a single client signal authoritative.
8. **Feedback loop + FP/FN accounting** (Cloudflare's Bot Feedback Loop; DataDome's published FP metric). Make false positives a first-class, measured, published number. Refuse to ship "zero FP" claims.
9. **Data minimization defaults:** region-pinned storage, short retention (7–30 days), IP pseudonymization/hashing, no cross-site identifiers, no PII in payloads. Fingerprint's plan matrix (retention tiers, region choice, URL hashing) is a reasonable template even if you never match its scale.
10. **Transparent, published rate limits/billing and public status page** (Cloudflare/AWS/DataDome/Fingerprint all do). Trust is a product feature.

### Avoid (or explicitly design out)

1. **"Stealth" collection that bypasses ad blockers/blocklists** (Castle's "never blocked" claim). For a privacy-respecting system, that's the wrong side of the line and creates regulatory exposure under PECR/CNIL/ICO rules. If a user blocks your script, degrade gracefully.
2. **Privacy-hostile signals as first-class flags:** incognito detection, "privacy-focused browser" flags, private-mode penalties. They punish exactly the users with the best security posture. Use only as weak, non-punitive context — or not at all.
3. **Legitimate-interest-by-default for anything beyond strict fraud/security.** Fingerprint and SEON assert consent isn't needed for fraud detection; ICO's Sept 2025 statement and CNIL's position say terminal access requires consent where PECR/ePrivacy applies and that "strictly necessary" is user-centric and narrow. If you self-host for security only, scope strictly to security, document it, and don't reuse data for analytics/ads.
4. **Cross-consortium data sharing** (Sardine, IPQS Fraud Fusion). Network effects are strong but the privacy/legal surface is huge and it changes the product from "security" to "tracking."
5. **Precise-location signals** (Incognia-style) unless the product genuinely requires them; the FTC's location-data enforcement wave shows where that ends.
6. **Persistent cookies + fingerprint hybrid without expiry discipline.** Kount's classic model (persistent first-party cookie anchored fingerprint) is durable but is exactly what browser privacy changes (Safari ITP, Firefox TCP, Chrome privacy work) erode; design for cookie loss and for ID recovery from server-side matching instead.
7. **Overclaiming.** "99.5% accuracy", "0.001% collision", "no false positives", "100% detection" are unverifiable marketing; publish methodology + raw numbers instead. The 2026 anti-detect benchmark showing 3/12 "undetectable" browsers passing checks is a good reminder that "undetectable" is always temporary.
8. **Single-signal authority.** Every major vendor's postmortem-style content (Cloudflare's Golang heuristic FP story; DataDome's "zero FP is impossible") argues the same thing: layer, calibrate, and measure.

### Minimal viable self-hosted architecture (suggested by the survey)

- **Edge/server:** TLS (JA4) + HTTP/2 fingerprint (needs an edge that exposes it — see Borrow #4), header order, IP/ASN reputation, velocity counters (per IP / per account / per ID), all computed server-side with no client involvement.
- **Client (only on security-relevant pages: login, signup, payment, password reset):** small script, no cross-site storage, collects ~30–60 stable signals (canvas/webgl hashes, fonts, screen, timezone, languages, hardware concurrency/memory, plugins, touch, storage availability) + consistency checks (main world vs worker, getter tampering, CDP artifacts, `navigator.webdriver`, headless markers).
- **Server-side matcher:** deterministic anchors (signed first-party cookie) with fuzzy fallback; per-identification confidence score; explicit merge/split audit trail; short retention.
- **Outputs:** `device_id` (rotating, deployment-scoped), `confidence`, `bot_score` (separate), risk flags as _weak_ signals, never a black-box "fraud score" without per-signal attribution.
- **Ops:** FP/FN feedback loop, published FP rate, public status page, retention controls, no third-party sharing.

---

## 14. Concrete evasion techniques attackers use (with sources)

Detection-side detail for automation, rotation and spoofing lives in R2-evasion-counters (§1–§4, master table §9). This list keeps only what the vendor survey adds:

1. **CDP `Runtime.enable` detection → patched drivers** (R2-evasion-counters §4.1–§4.2). DataDome's counter is layering behaviour + reputation + weak signals rather than one detector.
2. **`navigator.webdriver` + command-flag removal** (R2-evasion-counters §4.2, Patchright row).
3. **Headless fingerprint convergence.** New headless Chrome shares its codebase with headful; once UA is set and webdriver removed, few JS inconsistencies remain — hence the pivot to CDP/behavioural detection (DataDome post in §9.2).
4. **Fingerprint harvesting + replay** — §10.3.
5. **Vendor-aligned payload forgery.** Harvesting scripts replicate the signal structures/challenge logic of PerimeterX, Incapsula, Akamai, Adyen, hCaptcha so replayed telemetry looks vendor-consistent (Castle report, §2).
6. **Solver services / CAPTCHA farms** — §9.3.
7. **Residential proxy networks + infected devices** (R2-evasion-counters §2). Cloudflare built ML v8 for residential-proxy bots (§9.1); Fingerprint exposes `proxy_type: residential` + provider (§1).
8. **TLS/HTTP2 fingerprint spoofing and mismatch.** Bots tamper with TLS (Akamai: https://www.akamai.com/blog/security/bots-tampering-with-tls-to-avoid-detection); defenders catch the _inconsistency_ (TLS says iOS, UA says Chrome) — DataDome's TLS post documents a 2.1M-request attack blocked this way.
9. **PoW farms / VM de-obfuscation** — §9.3 (Kasada tokens) and §9.5 (Akamai `_abck`/`sensor_data`); real-browser farms replay the tokens, which is why both vendors keep escalating.
10. **Anti-detect browser inconsistencies** — §10.1; Fingerprint's Tamper Detection names AdsPower/DolphinAnty/OctoBrowser/GoLogin (§1).
11. **ML poisoning + de-obfuscation** — Kasada adversary page (§9.3).
12. **Privacy-tool abuse.** VPN/proxy/Tor/incognito/privacy browsers launder identity; Fingerprint monetises detecting them (§1); treat as weak signals — legitimate privacy users are the false positives.
13. **Ban evasion via factory reset / reinstall.** Mobile factory reset changes device IDs; countermeasures are location anchoring (Incognia, §5) or hardware-level persistence (SEON "True Device ID", §6). iOS/Android Keychain/Keystore reuse is the common trick; Fingerprint exposes "Factory Reset" and "Cloned App" signals.
14. **Fingerprint randomization as user defence (and its limits)** — §11 (Brave farbling, Firefox RFP/ETP-Strict, the 2025 "Breaking the Shield" paper).

## 15. Gaps / open questions

- **No independent, methodologically transparent FP-rate publication from the fraud-signal vendors** (Fingerprint, Castle, Sift, Sardine, Incognia, SEON, IPQS, Kount). Only bot-management vendors publish (Cloudflare per-heuristic, DataDome <0.01%). Accuracy numbers are self-reported.
- **No public breach history found for the fingerprinting vendors themselves** — the adjacent location-data broker ecosystem (Gravy Analytics breach Jan 2025, FTC actions) carries the demonstrated risk.
- **Enterprise pricing opacity:** Sift, Sardine, Incognia, SEON (partial), Castle (partial), HUMAN, Akamai, Kasada, Arkose are all quote-based. DataDome, AWS, Fingerprint, IPQS, Cloudflare (Enterprise tiers) are the exceptions.
- **Regulatory direction of travel:** ICO (Sept 2025) and CNIL (2021 position + 2025–2026 consent fines) treat fingerprinting as consent-requiring when used for tracking; fraud/security use is more defensible but not automatic. US federal action is concentrated on location brokers. Any self-hosted system should be built to the stricter standard.
