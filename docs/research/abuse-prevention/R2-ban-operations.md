# R2 — Ban Operations: how real platforms do linking, evasion, enforcement, appeals, and false positives

Scope: the operating model a self-hosted fingerprinting system must fit into — for a Quran reader used by families, madrasas/schools, and shared phones.
Date: 2026-10-12. All claims cite a source; vendor claims are labelled as vendor claims; figures I could not verify against a primary source are labelled _unverified_.

Hard constraint carried from the repo: **no SHA-256 over Quran data in any automated path**. Everything below that mentions hashing concerns _abuse/device evidence_, never Quran DB identity. Do not let an evidence-hash column leak into the Quran catalogue path.

---

## 1. Ban models: what each one actually achieves

| Model                                                | Mechanism                                                                 | What it achieves                                                     | Collateral damage                                                                                                                                                                                                                                                                                                                                                                                                                              | Real examples                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Content takedown / demotion** (shadowban-adjacent) | Remove post, or reduce distribution without notifying                     | Stops a specific harm; no account loss                               | Opaque; users can't tell they're throttled; research calls it "light, insidious" censorship                                                                                                                                                                                                                                                                                                                                                    | Meta: Pages "removed from recommendations… distribution reduced" ([Meta, Restricting accounts](https://transparency.meta.com/enforcement/taking-action/restricting-accounts/)); shadowbanning analysed in [Springer BISE 2024](https://link.springer.com/article/10.1007/s12599-024-00905-3) and [Information Systems Research 2024](https://pubsonline.informs.org/doi/full/10.1287/isre.2024.1140) |
| **Feature restriction / timeout**                    | Can't post, comment, DM, or create for N days                             | Proportional, cheap, reversible                                      | Annoying for benign users; easy to evade by switching accounts                                                                                                                                                                                                                                                                                                                                                                                 | Meta strike ladder (below)                                                                                                                                                                                                                                                                                                                                                                           |
| **Hard account ban**                                 | Account disabled, content gone                                            | Removes identity and history; strongest account-level tool           | Loses a real user's data; if wrong, expensive to undo; motivates evasion                                                                                                                                                                                                                                                                                                                                                                       | Meta "10 or more strikes → 30-day restriction", then disable                                                                                                                                                                                                                                                                                                                                         |
| **IP ban**                                           | Block address/range                                                       | Blocks naive evasion and scripted signups                            | Breaks shared networks: schools, cafés, CGNAT, families. Wikipedia therefore uses _soft_ (anon-only) IP blocks by default and grants per-user [IP block exemption](https://en.wikipedia.org/wiki/Wikipedia:IP_block_exemption) for "editors in good standing whose editing is disrupted by unrelated blocks" — 666 accounts held the flag at time of reading                                                                                   | Wikipedia: [Blocking IP addresses](https://en.wikipedia.org/wiki/Wikipedia:Blocking_IP_addresses), [IP block exemption](https://en.wikipedia.org/wiki/Wikipedia:IP_block_exemption)                                                                                                                                                                                                                  |
| **Device ban / fingerprint ban**                     | Persist a device token; block or challenge on match                       | Highest recapture of naive evaders; survives new accounts            | False positives on shared hardware are structurally likely; a device ban punishes every person who touches that phone. Vendor-side narrative: "device ID is one of the most common ways to recognise a returning user — but it's also one of the easiest to sidestep" ([Incognia](https://www.incognia.com/blog/ban-evasion)); enforcement patterns in [Fingerprint docs](https://docs.fingerprint.com/docs/ban-enforcement-use-case-tutorial) | Education platform with minors using exactly this pattern: [Schoolhouse case study](https://fingerprint.com/case-studies/schoolhouse-prevents-ban-evasion/)                                                                                                                                                                                                                                          |
| **Payment-instrument ban**                           | Block card/IBAN/phone billing                                             | Very effective where money is the motive                             | Needs a payment method at all; chargebacks/refund duty                                                                                                                                                                                                                                                                                                                                                                                         | Meta advertiser strike tolerance: reportedly **8–32 fraud strikes before an account is banned** (NYT via [VAB/PPC Land](https://ppc.land/facebook-banned-3-5-billion-fake-accounts-in-2025-vab-analysis-finds/))                                                                                                                                                                                     |
| **Network/entity-level ban**                         | Disable accounts "owned by the same person or entity", or linked networks | Kills clusters, not just one head                                    | Guilt by association; punishes roommates, coworkers, families                                                                                                                                                                                                                                                                                                                                                                                  | Meta Account Integrity: bans accounts "Owned by the same person or entity as an account that has been disabled", "Created or repurposed to evade a previous account or entity removal", and "Close linkage with a network of accounts… that violate or evade our policies" ([Meta](https://transparency.meta.com/policies/community-standards/account-integrity/))                                   |
| **Legal escalation / referral**                      | Report to NCMEC/IC3/police; preserve evidence                             | Only path for criminal content; creates deterrence and a paper trail | Slow, no feedback, resource-heavy; legal exposure if mishandled                                                                                                                                                                                                                                                                                                                                                                                | 18 U.S.C. §2258A (NCMEC reporting), §2703(f) (preservation) — see §8                                                                                                                                                                                                                                                                                                                                 |

**Scale context.** Facebook actioned ~3.5B fake accounts in 2025 and >38B over eight years; 2019 peaked at 6.5B, equal to 83% of world population by the VAB comparison ([PPC Land/VAB](https://ppc.land/facebook-banned-3-5-billion-fake-accounts-in-2025-vab-analysis-finds/)). Meta states ~4% of >3B monthly users are fake (≈140M) ([Social Media Today](https://www.socialmediatoday.com/news/meta-outlines-latest-data-on-content-removals-and-fake-accounts/807727/)). Meta's published enforcement _precision_ is for **content removal**, not account bans: **>90% on Facebook, >87% on Instagram** ([Meta Q3 2025 integrity report](https://transparency.meta.com/reports/integrity-reports-q3-2025/)). Nobody publishes a device-ban false-positive rate. That asymmetry is the single most important fact in this report: **platforms measure removal precision, not ban precision.**

---

## 2. Ban evasion detection: how a new account gets linked to a banned one

Published evidence, strongest first:

1. **Niverthi, Verma & Kumar, "Characterizing, Detecting, and Predicting Online Ban Evasion", ACM WebConf 2022** ([arXiv:2202.05257](https://arxiv.org/abs/2202.05257)). First data-driven study of ban evasion: 8,551 verified (parent, child) evasion pairs on Wikipedia. Child accounts resemble banned parents in **username similarity, edited pages, added content, and psycholinguistic attributes**. Classifiers: predict future evaders **AUC 0.78**, early detection of evasion **AUC 0.85**, matching child→parent **MRR 0.97**. Note what carries the signal: _behaviour and content_, not device. Device adds recall; behaviour adds precision.
2. **Sockpuppet detection literature.** Wikipedia sockpuppet corpora and ML detectors: Solorio, Hasan & Mizan, _Sockpuppet Detection in Wikipedia_ (LREC 2014); Sakib & Spezzano, ASONAM 2022 ([doi](https://doi.org/10.1109/ASONAM55673.2022.10068604)); Baamer & Boicu 2025 ([doi](https://doi.org/10.5220/0013210100003944)). Method families: stylometry, writing-style similarity, temporal co-activity, graph/dual-view learning.
3. **Coordinated inauthentic behaviour (CIB/CoIA).** Meta introduced the concept in 2018; the detection literature is now a survey-level field: [Detection and Characterization of Coordinated Online Behavior: A Survey (arXiv:2408.01257)](https://arxiv.org/abs/2408.01257); cross-platform similarity networks for the 2024 US election ([arXiv:2410.22716](https://arxiv.org/abs/2410.22716)); TikTok video-first CIB ([arXiv:2505.10867](https://arxiv.org/abs/2505.10867)); coordination in likes ([arXiv:2305.07350](https://arxiv.org/abs/2305.07350)). Core technique: build a **similarity graph over shared behaviours** (synchronised posting, caption reuse, hashtag-sequence overlap, content reuse) and find dense subgraphs. Signals are behavioural, not hardware.
4. **Stack Exchange's operating doctrine.** Abuse is defined functionally: _"if the second account allows you to do something on the site that your normal account would be prevented from doing, it is abuse"_; there are explicit legitimate uses; first offence = warning, second = suspension, increasing duration; the detection mechanics are deliberately **not disclosed** ("lest it give users ammunition to evade detection") ([Meta SE 57682](https://meta.stackexchange.com/questions/57682/how-should-sockpuppets-be-handled-on-stack-exchange), [MSO 381860](https://meta.stackoverflow.com/questions/381860/what-are-the-methods-of-detecting-vote-fraud-sockpuppetry-user-cross-voting-w)).
5. **Vendor stack (claims, not evidence).** Fingerprint: "more than 20 Smart Signals… bot detection, browser tampering, VPN detection"; ban enforcement = keep banned visitor IDs, match new sessions ([blog](https://fingerprint.com/blog/how-to-detect-ban-evasion/), [docs](https://docs.fingerprint.com/docs/ban-enforcement-use-case-tutorial)). Incognia: device ID alone is easy to sidestep; obfuscation, OS version changes, screen-resolution changes, and account-takeover-with-stolen-credentials all defeat naive matching ([Incognia](https://www.incognia.com/blog/ban-evasion)). An academic treatment of game-side evasion detection: [Bachelor's thesis, "Anti-Cheat Ban Evasion Detection" (2025)](https://github.com/Alainx277/ban-evasion-detection).
6. **Measurement gap.** Fraud vendors themselves admit platforms rarely measure re-registration after enforcement: "no single team owns the metric… the result is an absence of data rather than a deliberate gap" ([SEON](https://seon.io/resources/ban-evasion-on-marketplaces-why-platforms-dont-measure-it-and-what-its-costing-them/)).

**Linkage signal families to implement** (each is independently weak; require ≥2 families for anything punitive):

| Family          | Examples                                                                                 | Strength                                            | Failure mode                                                                                       |
| --------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Device          | visitor ID, canvas/WebGL/audio hash, font set, screen, timezone, storage-quota probe     | High for naive evasion                              | Shared phone/library PC; browser updates; iOS/Android reset                                        |
| Network         | IP, ASN, VPN/datacenter flag, TLS/JA4-ish shape                                          | Low alone                                           | Family wifi, madrasa NAT, mobile CGNAT, VPN                                                        |
| Behavioural     | session timing, navigation order, scroll/reading cadence, request pacing, retry patterns | Medium-high                                         | Needs volume; can drift for legitimate users                                                       |
| Content         | username similarity, duplicated text, same-reply templates                               | High (MRR 0.97 in the Wikipedia study)              | Quoted Quran text is _identical by design_ — never treat shared religious text as a linking signal |
| Graph           | invite graph, same org/school code, mutual contacts, referral chains                     | Medium                                              | Families share; classes share                                                                      |
| Payment/contact | phone, email pattern, billing                                                            | High where present                                  | Often absent for a free reader                                                                     |
| Historical      | parent-account fingerprints retained at ban time                                         | High (this is the whole point of "evidence freeze") | Retention law and privacy                                                                          |

**Design rule:** the fingerprint proves _device reuse_, not _person reuse_. Treat it as a prior, and require behavioural or content corroboration before escalation beyond a challenge.

---

## 3. Multi-account detection: legitimate reasons, and how not to punish them

Legitimate multi-account use is normal and documented by mature platforms:

- **Wikipedia** lists valid alternative accounts: **security on public computers**, privacy (contribution under a pseudonym), doppelgänger (anti-impersonation), **clean start**, compromised account, education (a separate account for classwork), mobile/2FA clients, bots, testing ([WP:Sockpuppetry § Legitimate uses](https://en.wikipedia.org/wiki/Wikipedia:Sockpuppetry#Legitimate_uses)).
- **Shared computers and networks are expected**: Wikipedia warns that "if two or more editors use the same computer or network connection… their accounts risk being accused of sockpuppetry", advises declaration, and notes that **closely connected users may be treated as a single user if they edit with the same objectives** ([WP:Sockpuppetry § Sharing an IP address](https://en.wikipedia.org/wiki/Wikipedia:Sockpuppetry#Sharing_an_IP_address)).
- **Meta** explicitly permits owning multiple accounts; only accounts _created or repurposed to evade_ a removal, _owned by the same entity as a disabled account_, or _used to contact someone who blocked you_ are banned ([Meta Account Integrity](https://transparency.meta.com/policies/community-standards/account-integrity/)).
- **Instagram** historically supports up to five accounts per app install; **Google/Apple/Netflix** all ship multi-account and "household" models. (Well-known product facts; no primary citation needed beyond the Meta policy above.)

**Distinguishing "multiple accounts" from "evasion" — a workable test:**

1. **Functional test (adopt from Stack Exchange):** does the second account let the user do something their first account is prevented from doing? If no → not evasion.
2. **Causal test:** was the second account created _after_ a ban, on a device/fingerprint _retained from that ban_? If no retained parent match → not evasion.
3. **Corroboration test:** does the second account reproduce the parent's _violating behaviour_ (same spam text, same harassment target, same fraud pattern) within a short window? If no → friction, not ban.
4. **Continuity test:** does the account look like a person continuing their own life (same surah/bookmark trail, same reading hours, same language) or like a machine replaying a script?

**Shared-device reality for a Quran app.** A family phone, a madrasa PC lab, or an internet café is one fingerprint with N humans. Three mitigations, all cheap:

- **Organisation-linked device registration.** Let a madrasa register a device/network as a shared kiosk; the token is then scoped to the _organisation_, and per-account decisions on that token need corroboration. Wikipedia's IP block exemption is the same idea, per-user instead of per-device ([WP:IPBE](https://en.wikipedia.org/wiki/Wikipedia:IP_block_exemption)).
- **Never let a device signal alone remove content or an account.** Device signals may _challenge_ (CAPTCHA, email verify, cooldown) freely; they may only _ban_ with corroboration.
- **Grace for shared-network noise:** suppress network-only signals when the same ASN/IP hosts many _legitimate_ accounts (a madrasa IP with 40 daily active readers is evidence of a school, not of a farm). Compute an `ip_legit_share` metric and require stronger evidence as it rises.

---

## 4. False positives: published rates, famous failures, and the mitigating architecture

**What is actually published.**

- **Content-removal precision:** Meta reports >90% (Facebook) and >87% (Instagram); <0.1% of all content removed incorrectly ([Meta Q3 2025](https://transparency.meta.com/reports/integrity-reports-q3-2025/)). Meta also reported a ~50% reduction in enforcement mistakes in the US in Q1 2025 ([Social Media Today](https://www.socialmediatoday.com/news/meta-transparency-reports-q1-2025-community-notes/749361/)).
- **Account-ban precision:** no platform publishes it. **No vendor publishes a device-fingerprint ban false-positive rate either** — Fingerprint, Incognia, SEON and peers publish detection claims, not FP rates. Treat any "99% accurate" fingerprinting claim as unverified marketing.
- **Anti-cheat mass false bans are a documented genre.** VAC bans are permanent and false positives are reviewed internally ([Valve Anti-Cheat](https://en.wikipedia.org/wiki/Valve_Anti-Cheat)); detectors use statistical thresholds specifically to bound false positives ([Cheating in online games](https://en.wikipedia.org/wiki/Cheating_in_online_games)); and hardware/licence-based bans have produced mass false positives that had to be corrected afterwards — the _Product key_ article documents exactly that pattern, citing World of Warcraft ([Product key](https://en.wikipedia.org/wiki/Product_key)). (I could not reach a primary incident report during this research; search engines were rate-limiting, so the specific 2023–2026 ban-wave incidents are _unverified here_.)
- **Fraud/AML false positives.** The widely repeated figure that the overwhelming majority of AML alerts are false positives comes from vendor/industry surveys I could not verify against a primary source in this session — mark as _unverified, directionally safe_. The structural point stands: every high-recall abuse system trades precision for coverage, and payment fraud is the canonical example of a domain where false positives are a known, budgeted cost.

**Cost of a wrongful ban** (what you are actually buying when you tune recall up):

- Support load and trust loss for every reversed ban; for a Quran app the affected users are disproportionately **children, teachers, and non-technical parents**.
- The ban itself is the trigger for evasion: banning a benign user on a shared phone trains a family to create throwaway accounts.
- Escalation to "the app that bans Muslims" narratives is a real reputational risk for a religious app; a single viral false-ban thread costs more than a year of spam.

**Mitigating architecture (adopt all six):**

1. **Risk tiers, not booleans.** Score → tier → action (§7).
2. **Manual review queue** with SLA, reviewer identity, and a decision log.
3. **Evidence snapshots** frozen at proposal time (§6), so a reviewer months later sees the same data the machine saw.
4. **Appeal SLA** with a human, not a re-run of the same model (§5).
5. **Self-service unban for low-tier actions** (cooldown expiry, challenge pass, "this is my family's shared phone" flow) — no ticket needed.
6. **Monitor on ban-_proposal_ precision, not ban count.** Track: proposals → upheld / overturned / expired-without-review. A dashboard that rewards raw ban count is a false-positive factory.

---

## 5. Appeal and due process

**Legal obligations that apply if you serve EU users (and they are a good default everywhere):**

- **GDPR Art 22** — a data subject has the right not to be subject to a decision **based solely on automated processing** producing legal or "similarly significantly" affecting effects. ToS enforcement can rely on Art 22(2)(a) (contract performance) or (c) (consent), but Art 22(3) then requires **suitable safeguards: human intervention, the right to express a point of view, and the right to contest the decision** ([Art 22 text](https://gdpr-info.eu/art-22-gdpr/)). A permanent account ban from a fingerprint score, with no human in the loop, is exactly the case this article exists for.
- **DSA Art 17 — statement of reasons.** For any suspension/termination of an account, the platform must give a clear, specific statement including: the measure and its **duration and territorial scope**, **the facts and circumstances relied on**, **whether automated means were used**, the **contractual or legal ground**, and **how to seek redress** ([Art 17](https://www.eu-digital-services-act.com/Digital_Services_Act_Article_17.html)).
- **DSA Art 20 — internal complaint handling.** At least **six months** of access to a free, electronic complaint channel; complaints handled "in a timely, non-discriminatory, diligent and non-arbitrary manner"; if the complaint shows the conduct "does not warrant the measure taken", the platform **must reverse the decision without undue delay**; and the decision must be taken **under the supervision of appropriately qualified staff, and not solely on the basis of automated means** ([Art 20](https://www.eu-digital-services-act.com/Digital_Services_Act_Article_20.html)). Art 21 adds out-of-court dispute settlement (DSA articles index: [eu-digital-services-act.com](https://www.eu-digital-services-act.com/Digital_Services_Act_Articles.html)).
- **DSA transparency database.** Statements of reasons are submitted to the EU's public database ([transparency.dsa.ec.europa.eu/statement](https://transparency.dsa.ec.europa.eu/statement)) — a model for your own machine-readable enforcement log.
- **Voluntary standard worth adopting in full:** the **Santa Clara Principles 2.0** (Numbers / Notice / Appeal) — publish totals actioned, **appeals, successful appeals resulting in reinstatement, appeals of automated flags, and proactive reinstatements**; notice must be "in a durable form… accessible even if a user's account is suspended"; appeals must be reviewed by **someone not involved in the original decision**, who understands the language and cultural context; users may **present additional information** ([santaclaraprinciples.org](https://www.santaclaraprinciples.org/)).

**Practical appeal flow for a small team:**

1. **Notice at action time** — what happened, which policy, duration, and a stable URL/deep link to appeal that works _while suspended_ (durable notice; Santa Clara § Notice).
2. **One form, three fields:** "what were you doing", "who else uses this device", free-text. Optional upload.
3. **Triage:** auto-reverse anything where the action was device-signal-only or where the account had no prior violation (low tier). Route everything else to a human queue.
4. **Human review by a different person** than the one who proposed it; if only one operator exists, review at a fixed later time with a written second-look checklist and record that fact (Art 20(6) requires supervision by qualified staff; a single-operator shop should document compensating controls).
5. **Decision + reason**, in the user's language, with the out-of-court/regulator path if EU.
6. **Log everything** (case ID, timestamps, reviewer, inputs shown, decision, reason code) — this is what makes DSA Art 20 compliance auditable and what makes your own precision metrics possible.
7. **Target SLA:** first response ≤ 72 h; final ≤ 7 days; expedite harassment/impersonation and anything involving a minor.

**Presenting evidence without leaking detection logic** (do this, verbatim pattern):

> "This account was created on 12 Oct 2026 from a device and behaviour pattern previously associated with an account banned on 3 Oct 2026 for spamming. We matched (a) device, (b) content pattern, (c) timing. If this device is shared (school, family, café), tell us who else uses it and we will re-review."

Say **categories**, never weights, thresholds, feature names, or model versions. Keep the full evidence vector internal under the case ID; log the redaction. Stack Exchange states the doctrine plainly: detection mechanics are deliberately undisclosed "lest it give users ammunition to evade detection" ([MSO 381860](https://meta.stackoverflow.com/questions/381860/what-are-the-methods-of-detecting-vote-fraud-sockpuppetry-user-cross-voting-w)) — but that is a reason to withhold _methods_, not to withhold _reasons_. DSA Art 17 requires the reasons.

---

## 6. Evidence and retention ("evidence freeze")

**What to capture at the moment of a ban proposal (the freeze):**

| Field                                                                                    | Why                                                                                |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `case_id` (ULID), `subject_account_id`, `parent_account_id` if linked                    | Joins everything; supports appeals and LE requests                                 |
| Fingerprint snapshot: `visitor_id`, signal vector **as evaluated**, per-signal pass/fail | The vector changes within hours; without a snapshot the decision is unreproducible |
| Raw-but-keyed identifiers: IP, ASN, user agent, timezone, screen, language               | Needed for LE/preservation and for appeal context                                  |
| `score`, `tier`, **policy version**, **model/threshold version**, `rules_fired[]`        | Reproducibility; proves which rule generation acted                                |
| Matched evidence: duplicate content hashes, timing deltas, graph edges                   | Shows corroboration existed                                                        |
| Actor: proposer (automated/manual), reviewer ID, timestamps                              | Art 20(6) supervision; appeal independence                                         |
| Notice text sent + language                                                              | Art 17 statement-of-reasons compliance                                             |
| Retention class and expiry                                                               | So the freeze doesn't become a hoard                                               |

**Hashing at rest.** Store identifiers as **keyed HMAC (per-purpose key, e.g. `hmac_sha256(device_key, visitor_id)`)** rather than raw values, so you can still match a returning device without holding a raw fingerprint database. Keep the key in the app's secret store, rotate with a version prefix, and keep an unkeyed copy only inside the frozen case record where law-enforcement preservation needs it. Plain unsalted SHA-256 of a device signal is not protection — device signal spaces are small and enumerable. **This is not the banned "Quran SHA": the repo rule forbids SHA-256 over Quran data in automated paths; abuse-evidence hashing is a different dataset and must stay out of the Quran catalogue/ETag path.**

**Retention schedule (align with the two statutes that actually bind):**

| Data                                   | Retention                                                                  | Basis                                                                                                                                                         |
| -------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Raw signal snapshots (IP/UA/vector)    | 90 days rolling, then drop the raw values, keep the keyed IDs              | GDPR Art 5(1)(c)+(e) minimisation/storage limitation                                                                                                          |
| Decision records + freeze (ban ledger) | 12 months; extend only while a preservation request or open appeal is live | 18 U.S.C. §2703(f): preserve 90 days, extendable 90 days on renewed request ([law.cornell.edu](https://www.law.cornell.edu/uscode/text/18/2703))              |
| CSAM-related evidence                  | 1 year minimum, secure, access-limited, NIST-CSF-aligned storage           | 18 U.S.C. §2258A(h) — a CyberTipline report is itself a preservation request for 1 year ([law.cornell.edu](https://www.law.cornell.edu/uscode/text/18/2258A)) |
| Aggregate metrics (no identifiers)     | Keep indefinitely                                                          | Not personal data if properly aggregated                                                                                                                      |

**Anti-hoard rules:** every freeze has an expiry; deletion is a job with a metric, not a hope; "evidence freeze" ≠ "keep forever". Log who viewed a freeze record (access log on the evidence store) — that log is what you show a regulator or an auditor.

---

## 7. Automated vs human: deterministic rules, scored tiers, hysteresis, safe degradation

**Deterministic (hard rules, no score, no discretion).** These must fire identically every time and must be testable:

- Known-CSAM / child-exploitation material (hash-match against an industry list) → remove, report to NCMEC, freeze, ban. No scoring.
- Account created from a device token **already permanently banned for CSAM or credible threats to a minor** → block creation.
- Legal-order removals; age-gate violations; explicit ban on re-registration after a court order.
- Self-referential guard: any rule whose action is "delete account data" needs a human confirmation step unless it is a statutory takedown.

**Scored (risk tiers, ML or weighted rules).** Everything else — spam, scraping, mass account creation, harassment, ban-evasion suspicion.

**Policy-engine pattern** (small-team version):

```
signal → normalise → feature store → score(rule_version, weights) → tier
tier → action matrix → (notice, duration, review_required, appealable)
```

Non-negotiables: the score function is **versioned** and the version is stored with every decision; the action matrix lives in one file (so it is reviewable and diffable); actions are **pure functions of (tier, prior history)** so you can replay history and answer "what would the new thresholds have done?"

**Thresholds with hysteresis.** Ban at `score ≥ 80`; only auto-restore below `score ≤ 40`; between 40 and 80 hold the current state and never oscillate. Two thresholds, never one. Record both. Without hysteresis you get account flapping, which is worse than either action.

**Safe degradation ladder (never jump to ban):**

| Level        | Action                                                    | Duration      | Human review                            | Appeal                          |
| ------------ | --------------------------------------------------------- | ------------- | --------------------------------------- | ------------------------------- |
| L0 observe   | Log only                                                  | —             | No                                      | n/a                             |
| L1 throttle  | Rate limit / slow-mode                                    | minutes–hours | No                                      | auto-expire                     |
| L2 challenge | CAPTCHA / email verify / "confirm you're human"           | one session   | No                                      | auto                            |
| L3 cooldown  | Read-only for N hours; writes blocked                     | 1–24 h        | No                                      | auto-expire + one-click contest |
| L4 hold      | Actions queued, account visible, no new content published | ≤ 72 h        | **Yes**, before any content loss        | yes                             |
| L5 suspend   | Temporary account suspension with notice                  | 3–30 days     | Yes                                     | yes                             |
| L6 ban       | Permanent, evidence frozen, parent linkage recorded       | indefinite    | Yes, plus second look before finalising | yes, ≥ 6 months window          |
| L7 legal     | Ban + NCMEC/LE report + preservation                      | per statute   | Yes                                     | constrained by law              |

Meta's own ladder is a useful sanity check that even at planetary scale enforcement is graduated: 1 strike = warning; 2–6 = feature restrictions; 7 = 1-day content block; 8 = 3-day; 9 = 7-day; 10+ = 30-day; then disable ([Meta](https://transparency.meta.com/enforcement/taking-action/restricting-accounts/)).

**Ban only what you can defend in a review.** If the evidence would not survive being read aloud to the affected user's parent or teacher, it is an L3/L4 action, not an L6.

---

## 8. Legal/criminal escalation path

**The two statutes that matter for a small operator (US-facing):**

- **18 U.S.C. §2703(f) — preservation.** On request of a governmental entity, a provider "shall take all necessary steps to preserve records and other evidence in its possession pending the issuance of a court order or other process", for **90 days, extendable by another 90 on renewal** ([text](https://www.law.cornell.edu/uscode/text/18/2703)). Preservation is a _hold_, not a disclosure: you preserve and keep quiet unless a warrant/subpoena/court order compels disclosure (§2703(a)–(c)); during a challenge to foreign legal process you must preserve but are "not obligated to produce" (§2703(h)(4)). Disclosure to LE without valid process is a separate risk (§2702).
- **18 U.S.C. §2258A — NCMEC reporting.** Providers must report apparent CSAM violations to the CyberTipline **as soon as reasonably possible**; a completed report is treated as a preservation request for **1 year**; preserved material must be kept in a **secure location with access limited to what is necessary**; storage must be consistent with **NIST's Cybersecurity Framework**; knowing and willful failure to report carries fines up to **$600,000 (small providers, first failure)** / **$850,000 (subsequent)** ([text](https://www.law.cornell.edu/uscode/text/18/2258A)).

**Standard preservation-letter workflow (write this down before you need it):**

1. **Intake:** any email/letter claiming to be LE goes to one inbox with a case ID; verify sender domain and, if unsure, call the agency's published number.
2. **Classify:** (a) preservation request — no legal process needed, comply immediately; (b) subpoena — may cover subscriber records only, check scope; (c) court order/warrant — contents; (d) NCMEC/emergency (imminent risk of death/serious injury) — §2702(b)(8)/(c)(4) permit voluntary disclosure.
3. **Freeze:** run the evidence freeze, mark the records `legal_hold=true`, suspend the retention job for those rows.
4. **Acknowledge in writing** with the case ID and the preservation window end date; do not promise what you cannot do.
5. **Do not disclose** unless the process authorises it; log every disclosure with date, recipient, scope.
6. **Calendar** the 90-day expiry and the 90-day renewal.
7. **Respond narrowly:** minimum data, structured, with an index; never hand over an entire database.

**Evidence integrity for a small operator, realistically:**

- Append-only decision/evidence store (no in-place edits), with `created_at`, `actor`, and a hash chain over records (`prev_hash`) — this is cheap and gives you tamper-evidence without a chain-of-custody bureaucracy.
- One documented pipeline: export → checksum → secure transfer → receipt. Write the SOP once.
- Clock hygiene: NTP, UTC timestamps, timezone recorded.
- Access control: who can read the evidence store; log reads.

**Limits of a small operator — say this plainly in the policy:** you cannot staff a 24/7 law-enforcement desk, you cannot process MLAT requests, and you cannot verify foreign police credentials beyond published lists. Do: publish a **law-enforcement guidelines page** modelled on Reddit's ([redditinc.com/policies/guideline-for-law-enforcement](https://redditinc.com/policies/guideline-for-law-enforcement)), give a single contact address, honour preservation requests, report CSAM to NCMEC (mandatory), refer other crimes to local police/IC3 without promising action, and keep a lawyer-on-retainer for the day a warrant arrives. Everything else is theatre.

---

## 9. KPIs: what to measure so you know the system works

| Metric                                 | Definition                                                                            | Realistic target                                                                               | Rationale / benchmark                                                                                                                                                                                                                   |
| -------------------------------------- | ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Ban-proposal precision**             | share of proposals upheld after review (or not overturned on appeal)                  | **≥ 95%** for permanent bans; ≥ 90% for suspensions                                            | Meta's _content_ precision is >90% FB / >87% IG ([Meta](https://transparency.meta.com/reports/integrity-reports-q3-2025/)); account bans should be held to a higher bar because the harm is concentrated on one person                  |
| **Appeal overturn rate**               | reversed ÷ appeals filed                                                              | **< 5%** (yellow flag at 10%, red at 15%)                                                      | An overturn rate above ~10% means thresholds are too loose or reviewers are rubber-stamping                                                                                                                                             |
| **Proactive reinstatement rate**       | wrongly banned accounts restored **without** an appeal                                | **> 0 and trending up** — a system that never self-corrects is not measuring                   | Santa Clara § Numbers explicitly asks for proactive reinstatements                                                                                                                                                                      |
| **Evasion recapture rate**             | of new accounts linked to a banned identity, share re-caught within 30 days           | **40–70%** within 30 days for naive evaders; **< 20%** is a sign your linkage signals are weak | Wikipedia linkage study: MRR 0.97 for matching child→parent given the pair is in scope ([arXiv:2202.05257](https://arxiv.org/abs/2202.05257)); device ID alone is sidesteppable ([Incognia](https://www.incognia.com/blog/ban-evasion)) |
| **Repeat-offence rate after ban**      | share of banned users (identity-linked) who re-offend on a new account within 90 days | **< 20%**; measure, do not assume                                                              | Reddit Great Ban: 15.6% of affected users left Reddit, those who stayed cut toxicity 6.6%, but **5% increased toxicity by >70%** ([arXiv:2401.11254](https://arxiv.org/abs/2401.11254))                                                 |
| **Abuse per active user**              | violating events ÷ 1,000 DAU (by category), weekly                                    | trend flat or down; set a baseline before enforcing                                            | Meta reports prevalence by policy area rather than raw counts ([Meta CSER](https://transparency.meta.com/reports/community-standards-enforcement/))                                                                                     |
| **Time-to-detect (TTD)**               | first violating event → action                                                        | **< 24 h** for severe; **< 7 days** for evasion linkage                                        | Severity-weighted; publish the severe target                                                                                                                                                                                            |
| **Time-to-review (TTR)**               | proposal → human decision                                                             | **≤ 72 h**; ≤ 24 h if any minor is affected                                                    | Appeal SLA ≤ 7 days final (DSA Art 20 "without undue delay")                                                                                                                                                                            |
| **False-positive rate (hard actions)** | benign accounts permanently banned ÷ accounts banned                                  | **< 0.1%**                                                                                     | No platform publishes this; pick the bar you can defend in public                                                                                                                                                                       |
| **Evidence-freeze coverage**           | bans with a complete frozen snapshot ÷ bans                                           | **100%**                                                                                       | Without it, appeals and LE requests are unanswerable                                                                                                                                                                                    |
| **Retention compliance**               | expired freezes actually deleted ÷ expired                                            | **100%**, monitored as a job                                                                   | GDPR Art 5(1)(e)                                                                                                                                                                                                                        |
| **Reviewer load**                      | minutes per case; queue depth                                                         | < 20 min/case; queue < 50                                                                      | A queue you cannot clear becomes a rubber stamp                                                                                                                                                                                         |

**Dashboard rule:** the headline number is **precision**, not bans. A daily digest of "bans issued" creates the exact incentive that produces mass false bans.

---

## Graduated enforcement ladder (signal → action → duration → review)

| #   | Trigger signal (any one)                                                    | Action                                                                                 | Duration     | Review                           | Notes                                                            |
| --- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------ | -------------------------------- | ---------------------------------------------------------------- |
| 0   | nothing                                                                     | serve content                                                                          | —            | none                             | Default. Most traffic.                                           |
| 1   | new device + new account + datacenter/VPN IP                                | observe + score                                                                        | session      | none                             | Never friction a first-time reader                               |
| 2   | device token seen with ≥3 accounts, all benign                              | silent observation, no action                                                          | rolling 30 d | none                             | Family/madrasa pattern; this is _evidence of sharing_, not abuse |
| 3   | spam-shaped content (links, ads) from a young account                       | **L1** rate limit + content downrank                                                   | 1–24 h       | none                             | Content-level first, account-level later                         |
| 4   | device match to a banned token, **no** behavioural corroboration            | **L2** challenge (CAPTCHA / email verify)                                              | one session  | none                             | Challenge is cheap; a ban here is how false positives happen     |
| 5   | device match **+** behavioural similarity (timing/navigation)               | **L3** cooldown, read-only                                                             | 24 h         | none                             | Auto-expires; one-click "this is a shared device"                |
| 6   | device match **+** duplicated violating content (content hash / stylometry) | **L4** hold, queue for review                                                          | ≤ 72 h       | **required**                     | Freeze evidence now; notice now                                  |
| 7   | confirmed evasion: parent match + corroboration + prior ban                 | **L5** temporary suspension of the new account; strike recorded on the linked identity | 7–30 d       | required                         | Do **not** disable the shared device token                       |
| 8   | repeat evasion (2nd confirmed) or severe abuse (harassment, threats)        | **L6** permanent ban of the new account; parent linkage retained                       | indefinite   | required + second look           | Appeal window ≥ 6 months (DSA Art 20)                            |
| 9   | CSAM / credible threat to a minor / court order                             | **L7** immediate ban, evidence freeze, NCMEC or LE report, retention per statute       | per statute  | required (post-hoc if emergency) | Deterministic; no scoring                                        |
| 10  | any L4+ action on a **shared/kiosk-registered device**                      | step down one level and require a second corroborating family                          | as above     | required                         | Madrasa/school protection                                        |
| 11  | account claims shared device in appeal, claim verified by usage pattern     | reverse to L3 or clear; annotate device token as `shared`                              | —            | required                         | Feeds §3 mitigation; improves future precision                   |
| 12  | appeal upheld against the system (overturn)                                 | reverse, log `overturn`, review threshold that fired                                   | —            | required                         | Overturn rate feeds the precision KPI                            |

Hysteresis: L6 entered at `score ≥ 80`; never auto-restore above `score 40`; all tier transitions logged with policy version.

---

## What a small team can realistically run (no T&S department)

**Run (cheap, high value):**

1. A **policy engine** with versioned rules and a single action matrix — one file, reviewable in a PR.
2. **Hysteresis thresholds** and the L0–L7 ladder above; the ladder is the product.
3. **Evidence freeze on every L4+** with keyed-HMAC identifiers and a hash chain.
4. **Two-person review** for permanent bans (or a documented delayed second look if you are one person).
5. **Appeal form + durable notice** that works while suspended; 72 h first response, 7 days final.
6. **One dashboard with precision, overturn rate, TTD, TTR, recapture rate** — no "bans issued" headline.
7. **A law-enforcement guidelines page** + one contact + the preservation SOP + NCMEC reporting path.
8. **Retention job with a metric**; deletion is a feature.

**Do not attempt:**

- A 24/7 moderation desk, ML training pipelines, stylometry research, or real-time graph analytics.
- Cross-platform identity graphs, phone-number/email reputation vendors, or payment-instrument bans (no payment method in a free reader).
- Publishing "we catch 99% of evaders" claims — nobody publishes ban precision, and you would be the first, which is a liability not a boast.

**Explicitly for this app:** because it serves families, madrasas and shared phones, the device signal must be **advisory**, the network signal must be **weakly weighted**, and the shared-device exemption path (§3) must exist from day one. The realistic posture is _"recapture naive evasion, accept that a determined evader on a different phone is uncatchable, and never let a fingerprint alone cost a real reader their account."_

---

## Sources

Platform policy & transparency

1. Meta — Account Integrity policy: https://transparency.meta.com/policies/community-standards/account-integrity/
2. Meta — Restricting accounts (strike ladder): https://transparency.meta.com/enforcement/taking-action/restricting-accounts/
3. Meta — Community Standards Enforcement Report, Fake Accounts: https://transparency.meta.com/reports/community-standards-enforcement/fake-accounts/
4. Meta — Integrity Reports Q3 2025 (precision >90% FB / >87% IG): https://transparency.meta.com/reports/integrity-reports-q3-2025/
5. Meta — Transparency reports index: https://transparency.meta.com/reports/
6. Meta — Oversight Board case list: https://transparency.meta.com/oversight/oversight-board-cases/
7. Oversight Board — https://www.oversightboard.com/ (binding decisions; 250+ recommendations, 60% implemented/in progress)
8. Social Media Today — Meta Q3 2025 data (4% fake accounts ≈140M; enforcement precision): https://www.socialmediatoday.com/news/meta-outlines-latest-data-on-content-removals-and-fake-accounts/807727/
9. Social Media Today — Meta Q1 2025, ~50% reduction in enforcement mistakes (US): https://www.socialmediatoday.com/news/meta-transparency-reports-q1-2025-community-notes/749361/
10. PPC Land / VAB — 3.5B fake accounts in 2025, 38B over 8 years, 8–32 advertiser fraud strikes: https://ppc.land/facebook-banned-3-5-billion-fake-accounts-in-2025-vab-analysis-finds/
11. Mobile Ecosystem Forum — Meta moderation trends 2017–2025: https://mobileecosystemforum.com/2025/10/13/trends-in-metas-content-moderation-2017-2025-spam-fake-accounts-and-community-standards/
12. Reddit — Guidelines for law enforcement: https://redditinc.com/policies/guideline-for-law-enforcement
13. Stack Exchange — handling sockpuppets (functional abuse test, escalation): https://meta.stackexchange.com/questions/57682/how-should-sockpuppets-be-handled-on-stack-exchange
14. Stack Overflow — detection methods kept secret; handling practice: https://meta.stackoverflow.com/questions/381860/what-are-the-methods-of-detecting-vote-fraud-sockpuppetry-user-cross-voting-w
15. Stack Overflow — is there a mechanism to detect sock-puppets: https://meta.stackoverflow.com/questions/259949/is-there-a-mechanism-to-detect-sock-puppets
16. Wikipedia — Sockpuppetry § Legitimate uses: https://en.wikipedia.org/wiki/Wikipedia:Sockpuppetry#Legitimate_uses
17. Wikipedia — Sockpuppetry § Sharing an IP address: https://en.wikipedia.org/wiki/Wikipedia:Sockpuppetry#Sharing_an_IP_address
18. Wikipedia — IP block exemption (shared-network remedy): https://en.wikipedia.org/wiki/Wikipedia:IP_block_exemption
19. Wikipedia — Blocking IP addresses: https://en.wikipedia.org/wiki/Wikipedia:Blocking_IP_addresses

Academic 20. Niverthi, Verma, Kumar — Characterizing, Detecting, and Predicting Online Ban Evasion, ACM WebConf 2022: https://arxiv.org/abs/2202.05257 21. Cima, Trujillo, Avvenuti, Cresci — The Great Ban: Efficacy and Unintended Consequences of a Massive Deplatforming Operation on Reddit (WebSci 2024): https://arxiv.org/abs/2401.11254 22. Horta Ribeiro, Jhaver, Cluet i Martinell, Reignier-Tayar, West — Deplatforming Norm-Violating Influencers… (2024): https://arxiv.org/abs/2401.01253 23. Mekacher, Falkenberg, Baronchelli — The Systemic Impact of Deplatforming on Social Media (PNAS Nexus 2023): https://arxiv.org/abs/2303.11147 24. Monti, Cinelli, Valensise, Quattrociocchi, Starnini — Online conspiracy communities are more resilient to deplatforming (2023): https://arxiv.org/abs/2303.12115 25. Vu, Hutchings, Anderson — No Easy Way Out (Kiwi Farms; IEEE S&P 2024): https://arxiv.org/abs/2304.07037 26. Jhaver et al. — Evaluating the Effectiveness of Deplatforming as a Moderation Strategy on Twitter (2021): https://dl.acm.org/doi/10.1145/3479525 27. Mannocci et al. — Detection and Characterization of Coordinated Online Behavior: A Survey: https://arxiv.org/abs/2408.01257 28. Cinus, Minici, Luceri, Ferrara — Exposing Cross-Platform Coordinated Inauthentic Activity (2024 US election): https://arxiv.org/abs/2410.22716 29. Coordinated Inauthentic Behavior on TikTok (2025): https://arxiv.org/abs/2505.10867 30. Detecting Coordinated Inauthentic Behavior in Likes (2023): https://arxiv.org/abs/2305.07350 31. Sakib & Spezzano — Automated Detection of Sockpuppet Accounts in Wikipedia (ASONAM 2022): https://doi.org/10.1109/ASONAM55673.2022.10068604 32. Baamer & Boicu — Sockpuppet Detection in Wikipedia Using ML and Voting Classifiers (2025): https://doi.org/10.5220/0013210100003944 33. Solorio, Hasan, Mizan — Sockpuppet Detection in Wikipedia: A Corpus of Real-World Deceptive Writing (LREC 2014) 34. Shadowbanning — Business & Information Systems Engineering (2024): https://link.springer.com/article/10.1007/s12599-024-00905-3 35. Content Moderation with Shadowbanning — Information Systems Research (2024): https://pubsonline.informs.org/doi/full/10.1287/isre.2024.1140

Legal / due process 36. GDPR Art 22 (automated individual decision-making): https://gdpr-info.eu/art-22-gdpr/ 37. DSA Art 17 (statement of reasons): https://www.eu-digital-services-act.com/Digital_Services_Act_Article_17.html 38. DSA Art 20 (internal complaint-handling): https://www.eu-digital-services-act.com/Digital_Services_Act_Article_20.html 39. DSA article index (incl. Art 21 out-of-court settlement): https://www.eu-digital-services-act.com/Digital_Services_Act_Articles.html 40. DSA Transparency Database — statements of reasons: https://transparency.dsa.ec.europa.eu/statement 41. Santa Clara Principles 2.0 (Numbers / Notice / Appeal): https://www.santaclaraprinciples.org/ 42. 18 U.S.C. §2703 (incl. (f) preservation, 90+90 days): https://www.law.cornell.edu/uscode/text/18/2703 43. 18 U.S.C. §2258A (NCMEC reporting, 1-year preservation, NIST CSF, fines): https://www.law.cornell.edu/uscode/text/18/2258A

Vendor / practitioner (claims, not evidence) 44. Fingerprint — Strategies to Detect Ban Evasion (2025): https://fingerprint.com/blog/how-to-detect-ban-evasion/ 45. Fingerprint — Ban Enforcement tutorial: https://docs.fingerprint.com/docs/ban-enforcement-use-case-tutorial 46. Fingerprint — Schoolhouse case study (education, minors, ban evasion): https://fingerprint.com/case-studies/schoolhouse-prevents-ban-evasion/ 47. Incognia — Ban Evasion: how platforms shut out repeat offenders: https://www.incognia.com/blog/ban-evasion 48. SEON — Ban Evasion on Marketplaces: platforms don't measure it: https://seon.io/resources/ban-evasion-on-marketplaces-why-platforms-dont-measure-it-and-what-its-costing-them/ 49. Alainx277 — Anti-Cheat Ban Evasion Detection (Bachelor's thesis, 2025): https://github.com/Alainx277/ban-evasion-detection

Anti-cheat false positives (documented genre) 50. Wikipedia — Valve Anti-Cheat (permanent bans; internal review of confirmed false positives): https://en.wikipedia.org/wiki/Valve_Anti-Cheat 51. Wikipedia — Cheating in online games (statistical thresholds to reduce false positives): https://en.wikipedia.org/wiki/Cheating_in_online_games 52. Wikipedia — Product key (mass false positives, corrected afterwards; World of Warcraft example): https://en.wikipedia.org/wiki/Product_key

Unverified in this session (flagged, do not quote as fact)

- Any specific device-fingerprint ban false-positive rate (none published by vendors or platforms).
- AML/fraud alert false-positive percentages (industry surveys; primary sources not reached — search engines rate-limited during research).
- Specific 2023–2026 anti-cheat ban-wave incidents and their reversal counts.
- Oversight Board reversal percentage (the Board's binding-decisions model is documented; a specific rate should be read from its transparency reports: https://www.oversightboard.com/transparency-reports/).
