# C4 — Shipped privacy/compliance debt (device-layer independent)

**Scope:** the compliance debt the repo has **today**, independent of any fingerprint/device layer. This is B9 finding 12's "irreducible minimum" expanded into a work order.
**Method:** working-tree inspection of the shipped surfaces on 12 October 2026 (every `file:line` below was read, not inferred), plus DNS checks for the contact route.
**Inputs:** `A5-privacy-legal.md` (full), `B9-second-order.md` F12, `B8-retention-exec.md` F9/F10/F13, `A8-ops-realism.md` F6/F7/F14, `R2-legal-privacy.md` §6.3/§6.7/§6.8/§7.1/§7.2/§9.
**Not legal advice.** R2's uncertainty register applies in full. The load-bearing unverified items for _this_ document: DPDP s.7 "legitimate uses" content; DPDP Rules commencement (substantive Rules 3, 5–16 commence **13 May 2027**); DSA Art 19's structural reading (Art 17 not exempted — analysis, not a regulator statement); whether the operator owns/uses `easyquran.app` for mail (DNS says no MX today).

**Out of scope:** everything device-key/`device_records`/edge-attestation. Cutting that layer deletes most of B7's artefact list; nothing in this document depends on it.

---

## 0. Ground truth (verified in the working tree)

| Fact                                                                                                                                                                                                                                                         | Evidence                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Privacy policy renders `Last updated 30 July 2026 · placeholder text, not legal advice`                                                                                                                                                                      | `web/messages/en.json:171-172`; `web/src/lib/i18n/legal-copy.ts:28-33`                                                                           |
| Policy has 8 sections: collect / accounts / device / hosting / notifications / children / choices / changes. No controller identity, no legal basis, no retention periods, no rights list, no transfers, no processor list, no device/fingerprint disclosure | `web/src/lib/i18n/privacy-copy.ts:39-80`; `web/messages/en.json:176-190`                                                                         |
| Policy claims "You can delete your account and its data at any time" — **no self-service deletion exists**; `/user/v1/admin/delete/{user_id}` is `ROLE_ADMIN`                                                                                                | `web/messages/en.json:180`, `:190`; `rust/backend/api/src/modules/user_v1/mod.rs:29`                                                             |
| Terms contact is `salam@easyquran.app`; FAQ corrections address is `corrections@easyquran.app`; site config domain is `easyquran.fyi`, contact email is a Gmail address                                                                                      | `web/messages/en.json:213`, `:152`; `web/src/lib/config/site.ts:3,11`                                                                            |
| **`easyquran.app` has no MX record** (A records only → GitHub Pages). `easyquran.fyi` has Cloudflare MX                                                                                                                                                      | `dig MX easyquran.app` → empty; `dig MX easyquran.fyi` → 3× `*.mx.cloudflare.net`                                                                |
| Consent defaults: `analytics: true`, `performance: true`, `advertising: false`                                                                                                                                                                               | `web/src/lib/stores/consent.svelte.ts:16`                                                                                                        |
| Analytics initialises on every boot **before** any consent check; `pageView` fires immediately; crash reporting sends `exception` events through the same pipeline                                                                                           | `web/src/lib/boot/analytics.ts:20-31`; `web/src/lib/boot/crash-reporting.ts:2-6`; `web/src/routes/+layout.svelte:98`                             |
| `setConsent` + `setAnalyticsCollectionEnabled` are only applied _after_ init, using the (default-true) flags                                                                                                                                                 | `web/src/lib/boot/analytics.ts:12-16`                                                                                                            |
| Firebase project `easyquran-fyi`, `measurementId G-ZYL6HY24W6` (Google Analytics for Firebase)                                                                                                                                                               | `web/src/lib/firebase/index.ts:5-13`                                                                                                             |
| Firebase Performance is **never initialised** — the `performance` consent flag controls nothing                                                                                                                                                              | `web/src/lib/boot/analytics.ts:3-7`                                                                                                              |
| A second, public consent surface ships in the marketing layout                                                                                                                                                                                               | `web/src/routes/(marketing)/+layout.svelte:41`; `web/src/lib/components/tweaks/Tweaks.svelte:349-370`                                            |
| Settings-file import writes consent flags verbatim, including `analytics: true`                                                                                                                                                                              | `web/src/lib/settings/settings-document.ts:226`                                                                                                  |
| `user_sessions` stores raw `device` (User-Agent) + `ip_address` per login, no expiry, CASCADE on user delete only                                                                                                                                            | `migration/src/m000001_init.rs:48-57`; `modules/auth_v1/controller.rs:182-186,203`                                                               |
| The only hourly sweep deletes from the tower `sessions` table, not `user_sessions`                                                                                                                                                                           | `src/main.rs:764-775`; `src/services/session_store.rs:52-60`                                                                                     |
| FCM `devices` (push tokens) has no expiry; pruned only when FCM returns UNREGISTERED or past the 20-device cap                                                                                                                                               | `migration/src/m000005_device_notification_oauth.rs:13-24`; `src/modules/device_v1/controller.rs:17`; `src/services/notification/mod.rs:114-120` |
| `/post/v1` and `/post/comment/v1` mounted unconditionally; posts in sitemap + RSS/Atom; admin hide/unhide/delete on comments with no reason, no notice, no appeal                                                                                            | `src/router.rs:115-123`; `modules/post_comment_v1/mod.rs:17-37`; `controller.rs:233-313`                                                         |
| `comment_flags.reason` is `Option<String>`; flags CASCADE with the comment                                                                                                                                                                                   | `m000001_init.rs:298-308`                                                                                                                        |
| `user_bans` exists but has **zero writers** — `create`/`revoke` are never called; only `get_active_ban` is read at login                                                                                                                                     | `src/services/auth.rs:530-547`; `db/sea_models/user_ban/actions.rs:11-31`; grep for `NewUserBan` outside the model dir → none                    |
| Mail classification: `is_transactional` = verification + password_reset only. Suppressed transactional recipients are dropped with `Ok` (anti-enumeration); non-transactional get `MailError::Suppressed`                                                    | `src/services/mail/provider.rs:17-22`; `src/services/mail/router.rs:241-258`                                                                     |
| No enforcement/ban mail template, no `List-Unsubscribe` header anywhere in the mail service                                                                                                                                                                  | `src/services/mail/templates/` (7 templates, none enforcement); grep `List-Unsubscribe` → 0 hits                                                 |
| Newsletter: double opt-in, unsubscribe requires the same token, token stored plaintext, subscriber rows never expire, no pending expiry                                                                                                                      | `modules/newsletter_v1/controller.rs:55-150,273-295`; `db/sea_models/newsletter_subscriber/model.rs:9-17`                                        |
| `email_suppression` rows never expire                                                                                                                                                                                                                        | `m000001_init.rs:221-231`                                                                                                                        |
| The only scheduled job publishes scheduled posts; the only retention reaper is rate-limit expiry                                                                                                                                                             | `src/services/scheduler.rs:11-13`; `src/services/rate_limit_store.rs:194`                                                                        |
| Migrations stop at m000011; no `expires_at` column exists outside `user_bans.expires_at` (which has no writers)                                                                                                                                              | `migration/src/lib.rs:12-13`                                                                                                                     |
| `audit_logs` has no writers; it carries `ip_address` + `user_agent` columns                                                                                                                                                                                  | `m000001_init.rs:252-264`; grep outside the model dir → none                                                                                     |
| `post_views` carries `ip_address` + `user_agent`; written only by the seed system, not by live traffic                                                                                                                                                       | `db/sea_models/post_view/model.rs:6-14`; writers are `seed_v1`/`services/seed/*` only                                                            |
| Billing is real infrastructure: 9 providers wired by env (Stripe, Paddle, PayPal, Razorpay, Lemon Squeezy, Polar, Airwallex, Mercado Pago, Revolut, Crypto)                                                                                                  | `src/main.rs:241-274`; `modules/billing_v1/mod.rs`                                                                                               |
| No `appeal`/`report_abuse`/`statement of reasons` string anywhere in web or API                                                                                                                                                                              | grep over `web/src` + `rust/backend/api/src` → 0 hits                                                                                            |
| `user` model has no date of birth, no age, no minor flag                                                                                                                                                                                                     | `db/sea_models/user/model.rs:10-33`                                                                                                              |
| No backup/restore section in the deploy docs                                                                                                                                                                                                                 | `deploy/README.md` (grep `backup` → 1 unrelated hit about backup codes)                                                                          |
| The repo itself calls the legal copy a placeholder                                                                                                                                                                                                           | `docs/remaining/feature-gap-catalogue.md:613-614`                                                                                                |

---

## 1. Debt register

Ordered by **legal risk ÷ effort**. Effort: **S** ≤ half a day, **M** 1–2 days, **L** 3–5 days (solo dev, this repo's conventions).

| #       | Gap (what exists in code)                                                                                                                                                             | Legal hook                                                                                                                                                              | Minimum fix                                                                                                                                                       | Artifact                   | Effort             | Blocking                                        |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- | ------------------ | ----------------------------------------------- |
| **D1**  | Privacy policy is a marked placeholder: no controller, no lawful basis, no retention, no rights, no transfers, no processors, no disclosure of IP/UA/push-token/analytics-by-default  | GDPR Arts 13/14; DPDP Rules 2025 Rule 3 (itemised notice); CPPA §7012 if a "business"; ICO sole-purpose evidence                                                        | Paste the rewrite in §2; add the retention table in §3                                                                                                            | Policy text (i18n keys)    | **S** (text ready) | **YES**                                         |
| **D2**  | Firebase Analytics + crash reports run **by default** on every boot, including logged-out readers; `analytics: true` default; consent applied only after init                         | ePrivacy Art 5(3) + PECR reg 6 (UK); GDPR Art 6/13; EDPB Guidelines 2/2023                                                                                              | Default flags false; gate init + `pageView` + crash reports on consent                                                                                            | Code + UI copy             | **S**              | **YES**                                         |
| **D3**  | `user_sessions.device` (raw UA) + `ip_address` stored per login forever; undisclosed; only CASCADE removes them                                                                       | GDPR Art 5(1)(e), 13, 32; DSAR Art 15; DPDP Rule 3/8                                                                                                                    | Retention sweeper + clear UA/IP on revoke + disclose                                                                                                              | Retention job + policy     | **S**              | **YES**                                         |
| **D4**  | FCM `devices.token` never expires; no user-visible device list; survives logout                                                                                                       | GDPR Art 5(1)(e), 13(2)(f); Art 17                                                                                                                                      | 90-day inactivity sweeper + "Devices" list with revoke in settings                                                                                                | Retention job + UI         | **S**              | **YES**                                         |
| **D5**  | Policy promises account+data deletion "at any time"; deletion is admin-only                                                                                                           | GDPR Arts 17, 12(3); DPDP s.12(3); LGPD Art 18; CCPA §1798.105 if a business                                                                                            | Interim: reword the policy (S). Real: self-service `POST /user/v1/delete` + settings UI (M)                                                                       | Policy + API + UI          | **S** / M          | **YES**                                         |
| **D6**  | No DSAR intake, no erasure procedure, no receipts, no response template                                                                                                               | GDPR Arts 15/17/21(1)/12(3); DPDP s.11-12; LGPD Art 18                                                                                                                  | Documented SOP + monitored mailbox + a logged request table                                                                                                       | SOP + policy               | **M**              | **YES**                                         |
| **D7**  | Comment hide/unhide/delete restricts user content with no statement of reasons, no notice to the author, no appeal — **already happening**                                            | **DSA Art 17** (Section 2 — _not_ exempted by Art 19); Art 14 terms ground missing                                                                                      | Statement-of-reasons template + notice record + transactional mail (see §4)                                                                                       | Template + table + mail    | **M**              | **YES**                                         |
| **D8**  | No appeal/complaint path of any kind                                                                                                                                                  | DSA Art 20 (exempt while micro/small); GDPR Art 22(3) if any automated decision; LGPD Art 20                                                                            | Public tokenised appeal page + `POST /appeal/v1/submit`; model on Art 20 (6-month window, human decision)                                                         | API + page + SOP           | **M**              | **YES** (with D7)                               |
| **D9**  | No durable enforcement notice channel: `is_transactional` = verification+password_reset only; suppression silently drops; no enforcement template; no `NotificationKind::Enforcement` | DSA Art 17 (notice), Art 20(4); GDPR Art 12(1)                                                                                                                          | Add `enforcement_notice` template + transactional classification; record `notice_channel/sent_at/delivery_state`; treat suppression as "no contact details known" | Mail + table               | **S/M**            | **YES** (with D7)                               |
| **D10** | Newsletter: no `List-Unsubscribe` header; unsubscribe needs the token (lost-mail = stuck subscribed); token plaintext; pending rows never expire; confirmed rows kept forever         | ePrivacy Art 13; GDPR Art 21(2)-(3); CAN-SPAM; DPDP s.6(6) (withdrawal as easy as giving)                                                                               | One-click unsubscribe endpoint + `List-Unsubscribe`/`List-Unsubscribe-Post` headers + token hashing + pending expiry                                              | API + mail                 | **S/M**            | **YES** (API already live)                      |
| **D11** | The only contact address in the Terms is undeliverable: `easyquran.app` has **no MX**                                                                                                 | GDPR Art 13(1)(b) (contact details), Art 12(3) channel; DPDP Rule 3                                                                                                     | Replace with the working address on every surface; fix FAQ too                                                                                                    | Copy                       | **S**              | **YES**                                         |
| **D12** | No age field anywhere; IP/UA, push tokens, analytics apply identically to children                                                                                                    | DPDP s.9(3) (no tracking/behavioural monitoring of under-18s; no security exemption in the Fourth Schedule) + Rule 10; GDPR Art 8/Recital 38 + AADC; COPPA 16 CFR 312.2 | Age declaration at signup + minor flag; minor = no UA/IP retention, no push, no analytics, no comments; India = no account under 18                               | Schema + API + UI + policy | **M**              | **YES**                                         |
| **D13** | Consent integrity: settings-file import can set `analytics: true`; a second public consent surface ships in the marketing layout; the `performance` toggle controls nothing           | ePrivacy Art 4(11) (freely given, specific, informed); Art 5(1)(a)                                                                                                      | Force analytics off on import unless re-confirmed; single consent surface; drop or reword the dead toggle                                                         | Code + UI                  | **S**              | **YES** (cheap, high-visibility)                |
| **D14** | No retention mechanism exists at all; the only scheduled job publishes posts; migrations stop at m000011                                                                              | GDPR Art 5(1)(e), 5(2); DPDP Rule 8; COPPA "reasonably necessary"                                                                                                       | One `services/retention.rs` module registered beside `scheduler::start_scheduler`, 6 h tick, batched deletes, per-table config                                    | Job + tests + metric       | **M**              | **YES** (D3/D4/D10 depend on it)                |
| **D15** | No backup/restore exists, so "deleted" cannot be qualified and propagation is unbacked                                                                                                | GDPR Art 5(1)(e), 32; Art 33 runbook                                                                                                                                    | Nightly `VACUUM INTO` + off-host encrypted copy, 30-day cap, restore runbook                                                                                      | Ops doc + script           | **M**              | no (but any retention claim is soft until done) |
| **D16** | No ROPA, LIA, DPIA, processor DPAs, sub-processor list, or transfer basis; the lawful basis for account-level abuse prevention is undocumented                                        | GDPR Arts 30, 35, 28, 44, 5(2); DPDP Rule 13 (SDF analysis)                                                                                                             | Four short documents with owners + review dates (LIA is the load-bearing one)                                                                                     | Docs                       | **M**              | no (blocking before _growth_, not before ship)  |
| **D17** | `audit_logs` has no writer (no accountability trail for admin actions); `post_views` carries `ip_address`/`user_agent`; `rate_limit_state` keys hold IPs                              | GDPR Art 5(2), 32; Art 15 (DSAR bundle)                                                                                                                                 | Wire `audit_logs` for admin/content actions; keep `post_views` unwired and note it; declare rate-limit keys in the schedule                                       | Code + schedule            | **S/M**            | no                                              |
| **D18** | Terms lack the DSA Art 14 ground for abuse bans, an entity/governing-law line, and any content-moderation notice                                                                      | DSA Arts 14, 17 (the "contractual ground" Art 17 must cite)                                                                                                             | Add a moderation-and-enforcement clause + entity line                                                                                                             | Terms text                 | **S**              | no                                              |

### Register notes

- **D7 is not hypothetical.** The service stores recipient-provided information and disseminates it (sitemap, RSS/Atom), so it is a hosting service under DSA Art 2(1) today; `post_comment_v1` admin routes already restrict content visibility. Art 17 sits in Section 2 and is not covered by Art 19's micro/small exclusion. Every hide/delete without a statement of reasons is a live Art 17 exposure.
- **D8's exemption is time-limited.** Micro/small status under Recommendation 2003/361/EC is lost on growth, and billing is live with nine providers wired. Art 17 (D7) is owed regardless.
- **D14 is the keystone.** Every retention sentence in any policy — including the one in §2 below — is unenforceable until a job exists. Build it first; the schedule in §3 is the config.
- **D2's fix is the cheapest high-risk item in the register.** Two defaults and two guards.

---

## 2. Privacy-policy replacement text (paste-ready)

**Mechanics in this repo.** The body lives in i18n messages rendered by `web/src/lib/i18n/privacy-copy.ts` (`legal_privacy_*` keys; `messages/en.json` + `messages/ar.json`). Replace the eight `legal_privacy_*_heading/_body` pairs with the sections below (add keys as needed), keep the `id` list in `privacy-copy.ts:39-80` in the same order, and delete `legal_placeholder_note` usage from `legal-copy.ts:32` once a human has read it. Arabic must be a real translation, not machine output.

**Placeholders to fill before publishing (three):** `[CONTROLLER NAME]`, `[POSTAL ADDRESS]`, `[JURISDICTION]`. Everything else below matches what the code actually does; nothing in it describes a device fingerprint, because none exists.

> ### Privacy Policy
>
> **Last updated [DATE]**
>
> EasyQuran is a Qur'an reading app. This policy says what we collect, why, how long we keep it, and what you can do about it. It is short on purpose.
>
> **Who we are.** `[CONTROLLER NAME]`, `[POSTAL ADDRESS]`, `[JURISDICTION]` — an independent project, not a company group. Contact for anything in this policy: **privacy@easyquran.fyi** (or `[CONTROLLER EMAIL]`). We answer within 30 days, usually much sooner. If you are in the EU/UK and want to complain to a regulator, your local data protection authority can be found at edpb.europa.eu (EU) or ico.org.uk (UK); we would rather you came to us first.
>
> **The short version.** Reading needs no account and no analytics. If you turn analytics on, we get anonymous usage counts through Google Firebase. We store your IP address and browser string while you are signed in, for security. We store a push token only if you enable notifications. We do not sell or share your data, we do not run advertising, and we do not use any device fingerprint or cross-site tracking. We never infer or record your religious beliefs, and our security records are not linked to what you read or listen to.
>
> **1. What we collect, why, and on what legal basis**
>
> | What                                                                                                    | When                                                                    | Why                                                           | Legal basis                                                                      | Kept for                                                                      |
> | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
> | Session cookie (`ruxlog.sid`, httpOnly, SameSite=Lax)                                                   | Every visit                                                             | Keep you signed in, stop request forgery                      | Necessary for the service (GDPR Art 6(1)(b); ePrivacy "strictly necessary")      | 14 days after your last visit                                                 |
> | IP address and browser string                                                                           | When you sign in                                                        | Security: detect stolen sessions, abuse, and ban evasion      | Legitimate interests (Art 6(1)(f)) — our assessment is at easyquran.fyi/security | 90 days, then deleted                                                         |
> | Account details: name, email, hashed password (Argon2id), optional 2FA secret, linked sign-in providers | If you create an account                                                | Provide the account and sync                                  | Contract (Art 6(1)(b))                                                           | While your account exists; deleted within 30 days of account deletion         |
> | Push token and platform                                                                                 | Only if you enable notifications                                        | Deliver notifications to your device                          | Consent (Art 6(1)(a)) — you can withdraw it any time                             | 90 days after your device last registered                                     |
> | Anonymous usage analytics: screen views and feature events, sent to Google Firebase                     | Only if analytics is ON (it is **off** unless you turn it on)           | Understand which features are used, to improve them           | Consent (Art 6(1)(a))                                                            | 14 months (Google's default retention setting for this property)              |
> | Crash reports: the error message and file location of an uncaught error                                 | Only if analytics is ON                                                 | Fix bugs                                                      | Consent (Art 6(1)(a))                                                            | 14 months                                                                     |
> | Bookmarks, notes, reading position                                                                      | Stored in your browser; synced to your account only if you turn sync on | Provide the feature                                           | Contract (Art 6(1)(b))                                                           | Until you delete them                                                         |
> | Comments you post, and reports on comments                                                              | If you post                                                             | Publish your comment; moderate abuse                          | Contract + legitimate interests (safety of the community)                        | Until you delete it, or up to 12 months after we hide it                      |
> | Newsletter email address                                                                                | Only if you subscribe and confirm                                       | Send the newsletter                                           | Consent (Art 6(1)(a))                                                            | While subscribed + 12 months after you unsubscribe                            |
> | Email suppression record (your address, marked "do not mail")                                           | If a mail to you bounces or you unsubscribe                             | Never mail you again — a legal and deliverability requirement | Legal obligation + legitimate interests                                          | Kept while the service runs; contains nothing but your address and the reason |
> | Purchase records, if you buy a subscription                                                             | If you buy                                                              | Tax, accounting, and support                                  | Legal obligation (Art 6(1)(c))                                                   | As required by tax law (typically 7–10 years), then deleted                   |
>
> **2. What we do not do.** No advertising and no ad profiles. No sale or sharing of personal data. No device fingerprinting, no canvas/WebGL/AudioContext probes, no cross-site or cross-device tracking, no behavioural or biometric analysis of how you type, scroll, or recite. No precise location — the most we see is the country-level region our network provider attaches to a request. We do not infer, record, or use your religious beliefs for anything; the fact that you use a Qur'an app is not stored in any security or analytics record alongside anything that could identify you.
>
> **3. Who processes it for us.** We use a small number of providers, each under a data processing agreement, and we do not let them use your data for their own purposes:
>
> - **Cloudflare** — hosting, content delivery, DNS, and email routing (servers may be outside your country).
> - **Google (Firebase)** — analytics and push notification delivery, only as described above.
> - **Cloudflare R2** — storage for recitation audio and images.
> - **Our email provider** — transactional and newsletter mail.
> - **Our payment processor**, only if you buy something.
>
> Some of these providers process data in the United States. Transfers rely on the EU-US Data Privacy Framework and/or the European Commission's standard contractual clauses, with a transfer impact assessment on file.
>
> **4. Your rights.** You can ask for a copy of your data, correct it, delete it, or object to processing based on legitimate interests. Where you gave consent, you can withdraw it — analytics has a switch in Settings → Privacy, and withdrawing it does not affect anything else. Write to **privacy@easyquran.fyi** and we will act within 30 days. You can also delete your account and its synced data yourself from Settings → Account.
>
> Two honest limits. First, if you ask us to delete a security record while an active ban is under review, we may keep the minimum needed to enforce that ban and to defend the decision; we will tell you exactly what was kept and when it expires. Second, we take backups, and a deletion propagates out of them within 30 days.
>
> **5. Push notifications.** If you enable them, we store a token that identifies your browser or device so the message reaches the right place. You can turn notifications off in your browser or device settings, and the token is deleted automatically if it stops being used for 90 days. We never use the token to track you.
>
> **6. Moderation and enforcement.** Comments and posts are moderated. If we hide or remove something you posted, or restrict your account, we will tell you what was decided, why, and how to challenge it, and the notice will say whether a person reviewed the decision or whether it was automatic. Every restriction can be appealed, and the appeal is decided by a person. If you have been banned and cannot sign in, the ban notice contains a link that works without an account.
>
> **7. Children.** EasyQuran is a reader for everyone, and reading needs no account. We do not knowingly create accounts for children under 13 (or under 18 in India), we do not run analytics or push notifications for accounts declared as belonging to a minor, and we do not allow minors to post comments. We do not use age estimation or biometric age checks — we ask, and we trust the answer, which means a determined child can get around it. If you believe a child has given us personal data, write to us and we will delete it.
>
> **8. Security.** Passwords are hashed with Argon2id. Sessions use an httpOnly cookie. Access to production data is limited to the operator, and every administrative read is logged.
>
> **9. Changes.** If we change this policy we update the date at the top and, for anything substantive, tell you in the app before it takes effect.
>
> **10. Contact.** **privacy@easyquran.fyi** — or the address in section 1. We read everything.

**Two honesty rules for this text.** (1) The analytics row says "off unless you turn it on" — that is only true after the D2 fix lands. Until then, either land D2 or change that cell to "on by default; turn it off in Settings". Do not publish a policy that contradicts the shipped default. (2) The India itemised notice (DPDP Rule 3) is satisfied by the table in §1 _if_ the India-facing copy is translated and the withdrawal route is stated; if you decide not to serve accounts in India, say so in the India variant instead.

---

## 3. Retention schedule

**Current state:** exactly one retention mechanism exists (`rate_limit_store.rs:194`, expired rate-limit rows). Everything else is "forever" unless the parent row is deleted by CASCADE. `user_bans.expires_at` has no writer. The schedule below is the spec for `services/retention.rs` (§1 D14) plus three schema additions.

| Identifier (table.column)                          | Current retention                         | Proposed                                                                          | Mechanism to enforce                                                                                                                           |
| -------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Session cookie — tower `sessions` table            | 14 days inactivity; hourly sweep          | **keep 14 days**                                                                  | exists: `main.rs:764-775` → `session_store.rs:52-60`                                                                                           |
| `user_sessions.ip_address`, `user_sessions.device` | **forever** (CASCADE on user delete only) | **90 days** from `last_seen`; and set both to NULL on revoke                      | new sweeper: `DELETE FROM user_sessions WHERE last_seen < now-90d`; `UPDATE ... SET device=NULL, ip_address=NULL WHERE revoked_at IS NOT NULL` |
| `user_sessions` row itself                         | forever                                   | **90 days** after `last_seen` (revoked or not)                                    | same sweeper                                                                                                                                   |
| `devices.token` (FCM)                              | forever (until FCM says UNREGISTERED)     | **90 days** after `last_seen_at`                                                  | sweeper: `DELETE FROM devices WHERE last_seen_at < now-90d`; keep the existing UNREGISTERED prune                                              |
| `audit_logs.*` (incl. ip_address, user_agent)      | no writers                                | **12 months** from `created_at`                                                   | sweeper (only matters once writers are added)                                                                                                  |
| `rate_limit_state.*` (IP-keyed)                    | reaped on expiry                          | **keep as is** (expiry + 30 d)                                                    | exists: `rate_limit_store.rs:194`                                                                                                              |
| `comment_flags.*`                                  | forever (CASCADE with the comment)        | **12 months** after the comment is hidden/removed, or with the comment            | new `resolved_at` + sweeper, or delete with comment (CASCADE already)                                                                          |
| `post_comments` (hidden, not deleted)              | forever                                   | **12 months** after `hidden=1`, then purge content                                | sweeper on `hidden=1 AND updated_at < now-12mo`                                                                                                |
| `notifications` (in-app inbox)                     | forever                                   | **12 months**                                                                     | sweeper                                                                                                                                        |
| `post_views.ip_address`, `user_agent`              | seed-only writers                         | **never enable** for live traffic; if enabled, hash the IP and drop the UA        | policy + code review (no job needed)                                                                                                           |
| newsletter subscriber (confirmed)                  | forever                                   | **while subscribed + 12 months** after unsubscribe, then delete                   | sweeper on `status=Unsubscribed AND updated_at < now-12mo`                                                                                     |
| newsletter subscriber (pending, never confirmed)   | forever                                   | **30 days** from `created_at`                                                     | sweeper on `status=Pending AND created_at < now-30d`                                                                                           |
| `email_suppression`                                | forever                                   | **keep** — minimal, purpose = never contact again; document the basis             | no job; add a comment + the schedule entry                                                                                                     |
| `user_bans` (+ ban evidence, once written)         | no writers                                | **12 months** from `expires_at`, then delete the row; keep a count-only aggregate | new sweeper; Art 17(3)(e) restriction only under a per-matter hold                                                                             |
| billing: `subscriptions`, `payments`, `invoices`   | forever                                   | **tax-law period (7–10 years)**, then delete                                      | sweeper with a long class; processor holds the canonical copy                                                                                  |
| device-layer tables (`device_records`, etc.)       | n/a                                       | not in this document — conditional on the device-layer go/no-go                   | —                                                                                                                                              |

**Implementation notes (repo-specific):**

1. **Timestamps are TEXT with mixed formats.** `user_bans.expires_at` is chrono RFC3339 (`user_ban/actions.rs:12,18`) while `audit_logs.created_at` defaults to SQLite `CURRENT_TIMESTAMP` (`m000001_init.rs:261`). Comparing across storage classes in SQLite is always false — a retention job written against a TEXT column with a different format silently deletes nothing. Every new retention column must be **INTEGER epoch seconds** (the `rate_limit_state`/`sessions` convention) with a CHECK, and written by Rust only.
2. **Register the job next to the scheduler.** `src/main.rs:761` (`scheduler::start_scheduler`) is the pattern: spawn a task, 6-hour interval with 0–300 s jitter, batches of ≤500 rows, a short pause between batches, per-table errors non-fatal and logged, one OTel counter per table. Do not ride the rate-limit flush companion; the pool is size 1.
3. **Deletion is not physical erasure** on this build (no `SQLITE_SECURE_DELETE`; `auto_vacuum` default NONE). Either turn on `PRAGMA secure_delete=ON` and checkpoint the WAL after sweeps, or qualify every claim as "removed from the live database; backups within 30 days". The policy text above already uses the qualified form.
4. **Schema changes go through `migration/`** (m000012), per the backend AGENTS rules. `user_sessions` needs an `expires_at INTEGER` column and `devices` needs one only if you prefer expiry over a `last_seen` predicate; the sweeper can work with `last_seen` alone, which avoids the migration entirely. Prefer that.
5. **Add a boot invariant + test:** no `user_sessions` row older than the retention window; no `devices` row older than 90 days; a count logged at boot. A retention job with no verification is a claim, not a control.

---

## 4. Account-level abuse notice + appeal path

### 4.1 What is owed today vs what is contingent

| Duty                                                      | Applies today?                      | Why                                                                                                                                                                                                            |
| --------------------------------------------------------- | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **DSA Art 17** statement of reasons                       | **Yes, already**                    | The service stores and disseminates recipient content (`/post/v1`, `/post/comment/v1`, sitemap, RSS). Hosting service. `admin_hide`/`admin_delete` are live. Art 17 is Section 2 — **not** exempted by Art 19. |
| **DSA Art 20** internal complaint-handling                | Exempt while micro/small            | Art 19 excludes Section 3 (Arts 20–28). Verify and document status; the exemption is lost on growth.                                                                                                           |
| **DSA Art 20(6)** human supervision of platform decisions | Exempt while micro/small, but       | Build it anyway: it is the shape Art 22(3) requires.                                                                                                                                                           |
| **GDPR Art 22** automated decisions                       | **No** — nothing automated exists   | `user_bans` has zero writers; no automated suspension/ban exists today. The duty is _not to build it automated_: nothing above a rate limit should take effect without a person.                               |
| **GDPR Art 13(2)(f)/15(1)(h)** logic/significance info    | Only if Art 22 applies              | Not today. The moment scoring or auto-suspension is added, this and Art 22(3) both attach.                                                                                                                     |
| **LGPD Art 20** review right                              | If Brazilian users are restricted   | Broader than Art 22 — no "significant effect" requirement.                                                                                                                                                     |
| **DSA Art 14** contractual ground                         | Yes, as the ground Art 17 must cite | The Terms have no moderation/enforcement clause today.                                                                                                                                                         |

### 4.2 Notice template (statement of reasons)

> **Subject: We restricted [content / your account] on EasyQuran — what happened and how to challenge it**
>
> Hello [name],
>
> **What we did.** On [DATE] we [removed / hid] [a comment / a post] you posted on [DATE], / [suspended / terminated] your account. This applies [to that item / to the whole service] and lasts [until DATE / indefinitely].
>
> **Why.** [Plain-language reason, e.g.: "Your comment was removed because it contained a direct threat against another person."] We relied on [the rule in our Terms: "Acceptable use" / the law]. [If automated scoring contributed: "An automated system flagged this and a person reviewed it before we acted." / "No person reviewed this before it took effect; a person will review it within 72 hours and the restriction will be lifted if they disagree."]
>
> **What we looked at.** [The comment itself and the reports on it / the sign-in attempts and the session records for your account]. We do not use your reading history or your religious data for any decision.
>
> **How to challenge it.** Open [https://easyquran.fyi/appeal/TOKEN] — the link works without signing in, it is unique to this decision, and it stays valid for six months. You can also reply to this email. A person who was not involved in the original decision will read it. We answer within [14] days, and if your appeal succeeds we reverse the restriction and tell you.
>
> **How long we keep this record.** The decision record is kept for 12 months, then deleted.
>
> EasyQuran

### 4.3 Appeal page / endpoint requirements

- Public, unauthenticated, outside the signed-in app shell (a suspended user cannot sign in).
- `POST /appeal/v1/submit` — token + free-text; rate-limited per token and per IP; **uniform responses** that never reveal whether a case exists without a valid token (otherwise the endpoint is a ban oracle).
- Token: ≥128 bits, server-minted, stored **hashed**, single-purpose, revocable, expiring (≥ the appeal window), bound to the case so it survives the subject's deletion.
- Page hygiene: `X-Robots-Tag: noindex`, `Referrer-Policy: no-referrer`, **no analytics and no third-party scripts** on the page — an analytics beacon on a ban page exports "this person was banned".
- Mail: add `enforcement_notice` to `is_transactional` (`provider.rs:17-22`) so a suppression entry cannot silently swallow it; if the address is suppressed, treat it as "no contact details known" (DSA Art 17's conditional) and record that fact. Also add `NotificationKind::Enforcement` for the in-app copy.
- Solo-operator honesty: if you cannot staff a second reviewer, record the appeal as **self-reviewed** and say so in the notice. Two-person review is unreachable with one person; a documented single-reviewer appeal is better than a silent one.

### 4.4 Storage

New table `enforcement_notices` (or `enforcement_cases` if the device layer later wants cases):

```
id INTEGER PK
subject_type TEXT NOT NULL        -- 'user' | 'comment' | 'post'
subject_id INTEGER NOT NULL
decision TEXT NOT NULL            -- 'hide' | 'remove' | 'suspend' | 'terminate'
scope TEXT NOT NULL               -- 'content' | 'account' | 'service'
starts_at INTEGER NOT NULL
ends_at INTEGER                   -- NULL = permanent (needs two-person rule or documented solo exception)
reason_code TEXT NOT NULL
reason_text TEXT NOT NULL         -- the plain-language sentence shown to the user
facts TEXT NOT NULL               -- what was relied on, in plain language
automated_means INTEGER NOT NULL DEFAULT 0
contract_ground TEXT NOT NULL     -- e.g. 'terms:acceptable-use'
decided_by INTEGER                -- reviewer user id (NULL only for the interim automated case)
decided_at INTEGER NOT NULL
second_reviewer_id INTEGER
notice_channel TEXT               -- 'email' | 'in_app' | 'none'
notice_sent_at INTEGER
notice_delivery_state TEXT        -- 'sent' | 'suppressed' | 'no_contact_details'
appeal_token_hash TEXT
appeal_window_ends_at INTEGER
appeal_submitted_at INTEGER
appeal_outcome TEXT               -- 'upheld' | 'reversed' | 'partially_reversed'
appeal_decided_at INTEGER
appeal_reviewer_id INTEGER
retention_class TEXT NOT NULL DEFAULT 'enforcement_12mo'
```

Wire the writer into `post_comment_v1` admin hide/delete and the new `user_bans` admin path. `user_bans` already carries `reason`, `banned_by`, `expires_at`, `revoked_at`, `revoked_by` — build on it (`A8` F1), do not invent a second ban table. Add the missing admin routes (`/admin/users/bans` list/create/revoke) because `user_bans` currently has no writer at all.

### 4.5 Micro/small exemption status — the honest answer

- The operator is a **sole natural person** (`web/src/lib/server/owner.ts:23-32` fallback profile; no company entity appears anywhere in the repo). Headcount is 1, so the **staff** limb of Recommendation 2003/361/EC is met by a wide margin.
- The turnover/balance-sheet limb (≤ €10m for "small") is **unverifiable from the repo** and must be answered by the operator. Billing is live infrastructure — nine providers are wired in `src/main.rs:241-274` (Stripe, Paddle, PayPal, Razorpay, Lemon Squeezy, Polar, Airwallex, Mercado Pago, Revolut, Crypto) — but provider wiring is not revenue. **Assume commercial until told otherwise.**
- Consequence: **Art 17 (notice) is owed regardless of the answer.** Art 20 (complaint system) and Art 21 (out-of-court settlement) are exempt only while micro/small; a paid plan that crosses €10m or 50 staff ends the exemption. Write the assessment down with a review trigger ("re-check annually and on any change in turnover, headcount, or paid-plan activity") — that one page is the whole defence if the exemption is later questioned.
- Being commercial also flips other questions: the CCPA "business" thresholds, the CPPA §7012 notice-at-collection duty, and (potentially) tax/consumer-law duties for paid plans. Those are outside this document's scope but must be re-answered once the operator confirms revenue.

---

## 5. Minors posture for the shipped system

**What ships today:** no age field (`user/model.rs:10-33`), no minor flag, no age gate; the children's policy section is one sentence (`en.json:188`). Every child who creates an account gets the same treatment as an adult: raw UA + IP in `user_sessions`, an FCM push token, Firebase Analytics (on by default), comment posting, and newsletter subscription.

**The duties that actually attach:**

| Regime                      | Duty                                                                                                                                                                                                                                                                                                                | Does it bite today?                                                                                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **India — DPDP s.9(3)**     | No tracking or behavioural monitoring of children (under 18); no targeted advertising. Fourth Schedule exemptions cover health, education, crèches, school transport — **no security/anti-abuse exemption**. Rule 10 verifiable parental consent commences 13 May 2027 and cannot be satisfied by self-declaration. | **Yes.** IP/UA per-login records, push tokens, and analytics are all tracking of a child's activity. This is the strongest constraint in the whole register. |
| **GDPR Art 8 + Recital 38** | Art 8 gates consent-based ISS only — our bases are contract/LI, so parental consent is not directly triggered. But Art 6(1)(f) is expressly weakened "in particular where the data subject is a child", and children are a WP248 DPIA criterion.                                                                    | Partly — child-specific balancing + DPIA required.                                                                                                           |
| **UK — DUAA s.81 + AADC**   | Children's higher protection in design; the AADC applies to services likely to be accessed by children. Profiling children requires a DPIA and a best-interests assessment.                                                                                                                                         | **Yes.** A Qur'an reader is accessed by children by definition.                                                                                              |
| **US — COPPA, under 13**    | Persistent identifiers are PI; the "support for internal operations" exception (16 CFR 312.2) covers security/technical functioning but **not** product analytics or profiling. 2025 amendments: retention only "as long as reasonably necessary".                                                                  | **Yes, for under-13s** if analytics or profiling runs on them.                                                                                               |

**Minimum change (one migration + one guard + copy):**

1. **Self-declared date of birth at signup**, stored as `users.birth_year` (or `is_minor` computed and stored), with a hard rule: **India — no account for a self-declared under-18.** Show the message and offer the fully anonymous reader instead.
2. **When the account is a minor:** (a) do not store `user_sessions.device`/`ip_address` — store the platform class only; (b) no push tokens (reject `/device/v1/register` for minors); (c) analytics forced off for that account and the toggle hidden; (d) comments disabled; (e) newsletter subscription disabled.
3. **Analytics for a minor is never on**, regardless of the toggle, and the toggle is not shown. A child cannot consent to analytics under any of these regimes.
4. **Add a COPPA-shaped children's section** to the policy (already drafted in §2 item 7) naming the persistent identifiers, the purpose, the retention, and the no-disclosure rule.
5. **No age estimation, no biometric age assurance.** Both are themselves profiling (and, for face-based methods, biometric processing) and are disproportionate for a free reader.

**The honest limits, in writing (put this in the minors memo, not the marketing copy):**

- Self-declaration is not verification. A determined child can lie; Rule 10's verifiable-parental-consent standard is unreachable without an age-assurance vendor or a DigiLocker-style token, and neither is proportionate here.
- What the posture buys you is _proportionality_: you have minimised the data collected from minors, excluded them from analytics, push, and publishing, and documented why. That is a defensible reasonable-effort position for a free reader; it is not a guarantee.
- If the project later adds anything that increases child risk — public profiles, streaks with notifications, recitation recording, social features — the analysis changes and the minors posture must be re-run before that feature ships.

---

## 6. The consent fix (exact change)

**What the default should be: everything off.** Analytics and crash reporting collect personal data (client identifier, IP processed by Google, in-app events) and write to storage in the terminal. Under ePrivacy Art 5(3) / PECR reg 6 they are not "strictly necessary" for a reader that works without them, so they require prior consent in the EU/UK. "On by default, off in Settings" is not consent; it is the pattern DPAs fine. India has no legitimate-interest basis for this at all. Off-by-default is also the honest reading of the policy text in §2.

**Code changes, in order:**

1. `web/src/lib/stores/consent.svelte.ts:16`
   ```ts
   const DEFAULT_FLAGS: ConsentFlags = { analytics: false, performance: false, advertising: false };
   ```
   `decodeConsent` (lines 19-27) then yields `false` for any flag the user has never touched, and preserves an explicit `true` from a stored doc — that is a prior opt-in and may stand.
2. `web/src/lib/boot/analytics.ts` — gate the whole pipeline, not just the settings:
   ```ts
   void (async () => {
     if (!consent.analytics) return;              // nothing loads, nothing fires
     ...
   })();
   ```
   and inside `applyConsent` (lines 12-16), when `consent.analytics` goes false, call `setAnalyticsCollectionEnabled(false)` and stop firing `pageView`. Keep the `easyquran:consent` listener so turning it on mid-session initialises the SDK lazily.
3. `web/src/lib/boot/crash-reporting.ts:2-6` — crash reports ride the same analytics pipeline (`logException` → `track("exception")`), so gate `startCrashReporting()` on `consent.analytics` as well, or (better) split crash reports into their own toggle so the choice is specific rather than bundled.
4. `web/src/routes/+layout.svelte:98` — `startAnalytics()`/`startCrashReporting()` now no-op when denied; no other change needed.
5. `web/src/routes/(application)/settings/_components/PrivacySection.svelte:83-107` — the UI must say what the toggle does and who receives the data. Suggested labels: _"Anonymous usage analytics — off. When on, screen views and feature events go to Google Firebase. No ads, no profile, never sold."_ and _"Crash reports — off. When on, the error and file location of an app crash go to Google Firebase so we can fix it."_ Remove the `location.reload()` (line 44) once the pipeline is lazy — it exists only because the old code initialised eagerly.
6. **Delete the dead `performance` toggle** (`PrivacySection.svelte:99-107`, `Tweaks.svelte:360-370`). Firebase Performance is never initialised (`boot/analytics.ts:3-7`), so the control changes nothing. A privacy switch that controls nothing is worse than no switch.
7. **Single consent surface.** `web/src/routes/(marketing)/+layout.svelte:41` renders `MarketingTweaks`, which contains a second pair of analytics/performance toggles (`Tweaks.svelte:349-370`) on the public marketing site — i.e. for logged-out visitors, the exact audience the consent gate exists for. Remove the consent toggles from that panel.
8. **Consent import.** `web/src/lib/settings/settings-document.ts:226` writes `doc.privacy` straight into the store, so importing a settings file can switch analytics **on** without a fresh affirmative act. Force `analytics: false` (and `performance: false`) on import and let the user re-enable it in Settings; consent must not be grantable by a file the user did not write today.
9. **India:** if you keep analytics available at all, the India-facing variant needs a Rule 3 notice (itemised purpose + withdrawal route) before the toggle, and the default stays off. The clean answer remains: no analytics for Indian users.

---

## 7. Blocking vs nice-to-have

**BLOCKING — fix before any new feature ships.** Each of these is either a live unlawful processing activity, a false statement in a published policy, or a legal duty already owed on a surface that is already live.

| #   | Item                                                                                                                              | Why it blocks                                                                                                                                                        |
| --- | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1  | **D2 — analytics off by default + gated; crash reports gated; dead toggle removed; consent import fixed; single consent surface** | Personal data is being processed without a valid basis in the EU/UK right now, for every visitor, including logged-out ones. Cheapest high-risk fix in the register. |
| B2  | **D1 — real privacy policy published**                                                                                            | Art 13/14, DPDP Rule 3, §7012. A page marked "not legal advice" is not a policy; every other fix below is invisible without it.                                      |
| B3  | **D14 + D3 + D4 — retention job + `user_sessions` UA/IP + FCM token retention**                                                   | Art 5(1)(e). "Forever" is the default today, and a policy cannot state a period that no mechanism enforces.                                                          |
| B4  | **D5 — make the deletion claim true**                                                                                             | The policy currently promises something the code cannot do. Interim: reword. Real: self-service deletion.                                                            |
| B5  | **D11 — fix the contact address**                                                                                                 | The only contact route in the Terms bounces (no MX). Art 13(1)(b) and the DSAR channel both depend on it.                                                            |
| B6  | **D7 + D9 — statement of reasons + notice channel for content moderation**                                                        | Already owed under DSA Art 17 because comments are live and moderated. Add the Terms clause (D18) so the notice has a contractual ground to cite.                    |
| B7  | **D6 — DSAR intake path**                                                                                                         | A rights request today has no route, no owner, and no log. Even a manual SOP + monitored mailbox clears the blocking bar.                                            |
| B8  | **D12 — minors posture**                                                                                                          | India s.9(3) is a hard prohibition, not a balancing test, and it is being breached for every minor who signs in.                                                     |
| B9  | **D10 — newsletter unsubscribe integrity**                                                                                        | The newsletter API is live. No `List-Unsubscribe` header and a token-bound unsubscribe = a withdrawal route that can fail.                                           |
| B10 | **D13 — the remaining consent-integrity items**                                                                                   | Same class as B1; do it in the same pass.                                                                                                                            |

**NICE-TO-HAVE — do it, but nothing breaks the moment it is delayed.** D8's full appeal UX beyond the minimum (the notice + a working page is the blocking part; the SLA, the log dashboards, the out-of-court-settlement line are not). D15 backups. D16 ROPA/LIA/DPIA/DPAs — these become blocking before _growth_ (new jurisdictions, paid marketing, or any new sensitive feature), not before the next commit. D17 audit-log wiring. D18's Terms polish beyond the moderation clause. Retention verification tests and metrics.

**One sequencing rule.** B3's retention job must land before B2's policy is published, or the policy will state periods the system does not honour — the exact failure mode this audit exists to stop.

---

## 8. Findings (all NEW relative to A5/A8/B8/B9)

### 1. The Terms' only contact address is undeliverable — `easyquran.app` has no MX record

**Severity: High.**
Evidence: `web/messages/en.json:213` (`salam@easyquran.app`), `:152` (`corrections@easyquran.app`); `web/src/lib/config/site.ts:3,11` (domain `easyquran.fyi`, contact email a Gmail address); DNS today: `dig MX easyquran.app` → empty, A records point at GitHub Pages; `easyquran.fyi` → Cloudflare MX.
Why it matters: Art 13(1)(b) requires a usable contact route; the DSAR/erasure channel and the DSA Art 17 redress route both depend on it. A user who tries the published address gets a bounce, which is the worst possible evidence in a complaint.
Fix: publish one working address everywhere (`privacy@easyquran.fyi` for rights, plus the operator address in `/contact`), remove the `.app` addresses from Terms and FAQ, and verify delivery with an actual send.

### 2. Firebase Analytics and crash reporting run by default on every page load, before and without consent

**Severity: High.**
Evidence: `web/src/lib/stores/consent.svelte.ts:16` (`analytics: true`), `web/src/lib/boot/analytics.ts:20-31` (init → `pageView` on boot), `web/src/lib/boot/crash-reporting.ts:2-6` (exceptions via `logException` → `track`), `web/src/routes/+layout.svelte:98` (both started unconditionally), `web/src/lib/boot/analytics.ts:12-16` (consent applied only after init, using the default-true flags).
Why it matters: ePrivacy Art 5(3)/PECR reg 6 require prior consent for non-essential storage/access; analytics on by default is the classic DPA fine. India has no basis for it at all.
Fix: §6 — default false, gate init/`pageView`/crash reports, remove the dead toggle, single surface, block consent-by-import.

### 3. Raw User-Agent and client IP are stored per login with no retention and no disclosure

**Severity: High.**
Evidence: `migration/src/m000001_init.rs:48-57` (`user_sessions.device`, `ip_address`, no expiry); `src/modules/auth_v1/controller.rs:182-186,203`; `src/main.rs:764-775` sweeps the tower `sessions` table only (`services/session_store.rs:52-60`); policy's only abuse sentence is about CDN logs (`en.json:184`).
Why it matters: Art 5(1)(e) — indefinite storage of a raw UA plus IP per session is the single largest retention exposure in the repo, and it silently contradicts any future "we minimise" claim.
Fix: sweeper at 90 days + NULL both columns on revoke (§3), disclose in the policy table (§2), include in the DSAR bundle.

### 4. The policy promises self-service account deletion that does not exist

**Severity: High.**
Evidence: `web/messages/en.json:180` ("You can delete your account and its data at any time"), `:190`; `rust/backend/api/src/modules/user_v1/mod.rs:29` (`/admin/delete/{user_id}`, `ROLE_ADMIN` only); no delete-account UI anywhere in `web/src` (grep).
Why it matters: Art 17 + Art 12(3), DPDP s.12(3). A published promise that the code cannot fulfil converts a rights request into a complaint.
Fix: interim reword to "email us and we will delete it within 30 days"; real fix = authenticated `POST /user/v1/delete` + a Settings → Account control + a receipt row.

### 5. Comment hide/delete restricts user content with no statement of reasons, no author notice, and no appeal

**Severity: High.**
Evidence: `src/modules/post_comment_v1/mod.rs:17-37` (admin hide/unhide/delete routes); `controller.rs:233-313` (hide and delete return `{"message": "..."}`, no reason, no notification); `db/sea_models/comment_flag/model.rs` (`reason` optional, no resolution state); `src/router.rs:115-123` (posts and comments mounted unconditionally); grep for `appeal`/`report_abuse`/`statement of reasons` → 0 hits.
Why it matters: the service is a hosting service under DSA Art 2(1); Art 17 sits in Section 2 and is not exempted by Art 19's micro/small exclusion. This duty is owed today, not when the device layer ships.
Fix: §4 — `enforcement_notices` table, the notice template, `enforcement_notice` added to the transactional set, and the Terms clause that Art 17 must cite.

### 6. FCM push tokens never expire and there is no user-visible way to revoke them

**Severity: High.**
Evidence: `migration/src/m000005_device_notification_oauth.rs:13-24` (no expiry column); `db/sea_models/device/actions.rs` (upsert refreshes `last_seen_at`); `modules/device_v1/controller.rs:17` (cap of 20, eviction only past the cap); `services/notification/mod.rs:114-120` (prune only on FCM UNREGISTERED); no device list in `web/src` settings.
Why it matters: a push token is a persistent identifier that survives logout; Art 5(1)(e) and Art 13(2)(f) both apply, and a user cannot see or delete it.
Fix: 90-day inactivity sweeper (§3) + a "Notifications & devices" list in Settings with a per-device revoke that calls the existing `/device/v1/delete`.

### 7. Consent can be granted by importing a settings file, and a second consent surface ships on the public marketing site

**Severity: Medium.**
Evidence: `web/src/lib/settings/settings-document.ts:226` (`stores.consent.set({ ...doc.privacy })` — a file can set `analytics: true`); `web/src/routes/(marketing)/+layout.svelte:41` renders `MarketingTweaks` on public pages; `web/src/lib/components/tweaks/Tweaks.svelte:349-370` (analytics + performance toggles); `web/src/lib/boot/analytics.ts:3-7` (the performance flag controls nothing).
Why it matters: consent must be freely given by a specific, informed act (Art 4(11)); a file import is not an act, and a toggle that does nothing is a transparency defect.
Fix: force analytics/performance false on import; remove consent toggles from the marketing panel; delete or reword the dead toggle.

### 8. No retention mechanism exists anywhere; the only scheduled job publishes posts

**Severity: High.**
Evidence: `src/services/scheduler.rs:11-13` (publishes scheduled posts); `src/main.rs:761` (the single job registration) and `:764-775` (session sweep); `migration/src/lib.rs:12-13` (migrations stop at m000011); `m000001_init.rs` — no `expires_at` on `user_sessions`, `devices`, `audit_logs`, `email_suppression`; `rate_limit_store.rs:194` is the only reaper.
Why it matters: every retention period in any policy, including §2 and §3 of this document, is unenforceable until this exists. Art 5(1)(e)/5(2).
Fix: `services/retention.rs` registered beside the scheduler — 6-hour tick, jitter, ≤500-row batches, per-table errors non-fatal, INTEGER-epoch columns only, one counter per table, boot invariant; then the §3 schedule as its config.

### 9. Newsletter opt-out can fail and the API sends no unsubscribe header

**Severity: Medium-High.**
Evidence: `modules/newsletter_v1/controller.rs:128-150` (unsubscribe requires `email` + `token`), `:273-295` (confirm uses the same token), `:55-124` (subscribe re-issues the token, status → Pending); `db/sea_models/newsletter_subscriber/model.rs:13` (token stored plaintext); grep `List-Unsubscribe` in `services/mail/` → 0 hits; `email_suppression` rows never expire (`m000001_init.rs:221-231`).
Why it matters: withdrawal must be as easy as giving (DPDP s.6(6)); Art 21(2)-(3) requires a free, working objection route; CAN-SPAM requires an opt-out in every message. A subscriber who lost the confirmation email cannot unsubscribe at all, and the admin-authored newsletter body may contain no unsubscribe link.
Fix: one-click unsubscribe endpoint keyed on a hashed token + `List-Unsubscribe`/`List-Unsubscribe-Post` headers on every newsletter send, 30-day expiry for unconfirmed rows, 12-month post-unsubscribe retention, and an explicit "unsubscribe" footer enforced server-side.

### 10. No age data exists anywhere; children receive the same tracking, push tokens, analytics, and comment rights as adults

**Severity: High.**
Evidence: `db/sea_models/user/model.rs:10-33` (no DOB, no minor flag); `web/messages/en.json:188` (one-sentence children's section); `user_sessions` IP/UA (`m000001_init.rs:48-57`), FCM tokens (`m000005:13-24`), analytics default on (`consent.svelte.ts:16`), comment posting open to any account (`post_comment_v1/mod.rs:10-14`).
Why it matters: DPDP s.9(3) prohibits tracking and behavioural monitoring of under-18s with **no security exemption in the Fourth Schedule**; the UK AADC and COPPA's internal-operations limits point the same way.
Fix: §5 — self-declared DOB at signup, `is_minor` enforcement (no UA/IP, no push, no analytics, no comments, no newsletter), no accounts for self-declared under-18s in India, COPPA-shaped children's section, and a written minors memo stating the self-declaration limitation.

---

_End of C4. The five-item irreducible minimum from B9 F12 maps to B1-B9 here: (1) real policy → B2/B4/B5; (2) documented basis + LIA → D16; (3) notice + appeal → B6; (4) retention for already-stored identifiers → B3; (5) DSAR/erasure procedure → B7. Everything else in this document is either cheaper than those five or follows from them._
