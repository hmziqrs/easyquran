# H1 — Audit-loop exhaustion test (round-10 decision)

Auditor: exhaustion tester (round 9 → 10 gate). Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`
(working tree @ `53d3d421`, read-only). Method: `head`/`grep` over all 43 reports + 2 design docs to build a
subsystem map; then **code reads** to adjudicate every candidate gap the map produced; then the strict test:
_"could an audit of this change the plan (add/remove/reorder a workstream, or change a decision), or only
add detail to an already-decided area?"_

`NEW` = no prior report states this adjudication (grep-verified per item, not per topic).

---

## 1. Coverage matrix — subsystem | coverage (report(s)) | depth

Depth legend: **DEEP** = code read, findings with `file:line`; **MED** = code read for part of the surface;
**VERIFIED-LOW** = triaged shallow + independently code-verified low/zero reach (F6 or this session);
**SHALLOW** = mentioned/triaged only.

| Subsystem                                    | Coverage                                                                                 | Depth                                |
| -------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------ |
| auth / session                               | E5, F2, E4, C1, D1, A3, B1, G2                                                           | DEEP                                 |
| rate limiting (gate crate, keys, escalation) | B3 §7, B4, C1 §2, D1 #9, E5 #7, F4 F6/F10, G2 F3, G3                                     | DEEP                                 |
| bans (contract, evasion, appeal)             | A2, A8, B5, B9, C1, D2, E4, F3 §2, R2-ban                                                | DEEP                                 |
| ingress / IP trust                           | F1 (full trust-chain + fix design), G4                                                   | DEEP                                 |
| crypto / keys                                | A9, B8 §6, B10, C2                                                                       | DEEP                                 |
| device token (Option B)                      | B1, C1 §4, F5, G3 N1/N10                                                                 | DEEP                                 |
| fingerprint science / design                 | A1, A2, B2, B4, B5, B9, R2×4                                                             | DEEP                                 |
| retention / deletion execution               | A5, A10, B8, B9, C4, D2, E1, F3, G2                                                      | DEEP                                 |
| legal / privacy                              | A5, C4, R2-legal, B10, D2, E1                                                            | DEEP                                 |
| ops / incident response                      | A8, D5 §2, E1 (design + 4 attacks), F4, G4                                               | DEEP                                 |
| observability / kill switches                | F4 (8 numbers, alerting, self-heal), D3, E6 F9                                           | DEEP                                 |
| mail (oracle, caps, suppression, webhook)    | E2, D5 1a–1f, G3 §6–8, E1                                                                | DEEP                                 |
| billing                                      | E3, D5 #2                                                                                | DEEP                                 |
| media pipeline                               | G1 (12 findings), F3 §2.4                                                                | DEEP                                 |
| sync / bookmarks                             | G2 §1–2 (12 findings)                                                                    | DEEP                                 |
| comments / posts lifecycle                   | G2 §3–7, F3 §2.2, E1 F4, D1 #13, G2 F7–F13                                               | DEEP                                 |
| notifications / push / FCM                   | G3 §3–5, F4 F5, D5 #3, C4 D4/D6                                                          | DEEP                                 |
| search (quran + blog)                        | B3 §1.4 (quran, ≤50 hits, 30/min), D5 #4 (blog, `search_v1/controller.rs:27-52`), B4 R09 | DEEP (both surfaces read)            |
| seed / TUI                                   | D5 #5, F3 §5                                                                             | DEEP                                 |
| admin surface                                | F3 (capability + destruction + ACL + audit + rate), D5 #10, A8 §2                        | DEEP                                 |
| deploy / CI / rollout                        | B6, B7, D4, E5 #2, F1 §6, G4 §7                                                          | DEEP                                 |
| tests / guards                               | A6, B7, D4, C5 §5                                                                        | DEEP                                 |
| frontend / SSR / disk cache / SW             | A4, B1, B3, E6, F6 §3 (SW verified this session)                                         | DEEP                                 |
| i18n / paraglide                             | F6 §3                                                                                    | VERIFIED-LOW                         |
| deps / supply chain                          | B7 §2 (deny.toml + gate), F6 §3 (patches)                                                | DEEP (gate) / VERIFIED-LOW (patches) |
| quran delivery / content API                 | B3 §1.1–1.8, §4.4, §7; E6 Q1–Q7 + P0.1–P0.10; D5 #6/#7                                   | DEEP                                 |
| offline pack                                 | B3 F3, E6 F8, D5 #7                                                                      | DEEP                                 |
| R2 / artifacts / `_quran` gateway            | B3 F4/F5/F6/F11, E6 P0.5/P0.7, D5 #6                                                     | DEEP                                 |
| OAuth (5 providers)                          | D2 §4.5, D5 #8, F2 §3, F6 §3                                                             | DEEP                                 |
| passkeys / WebAuthn                          | F2 §2–3, E5 #7, R2-session-binding §3                                                    | DEEP                                 |
| Turnstile                                    | A9, C1 F5, E2 §1, F6 §3 (code verified)                                                  | DEEP                                 |
| CSP (API + web nonce + prerender hashes)     | E5 F9, A4 §2, B10 §1.10, F6 §3 (code verified)                                           | DEEP                                 |
| env / secrets / `app_constants`              | F6 §2.3 (code + DB proof), A9 §1.3, F1-7, D4 §8                                          | DEEP                                 |
| DB / schema / migrations                     | A10, B6 §4, B8 §1, E4 §1, F3 §2, G1 §1, G2 §7                                            | DEEP                                 |
| performance / cost                           | A7, E6, G1 #11, G2 F2, G3 N3                                                             | DEEP                                 |
| false positives                              | A2, B5, C3, G4 §1.3                                                                      | DEEP                                 |
| shared devices                               | B5 (10 findings), A2, C1 F7, G4 item 5                                                   | DEEP                                 |
| minors                                       | A5 §8, C4 #10, B5 §7.4                                                                   | DEEP                                 |
| India                                        | A5 F5/F8, B10 §1.15, R2-legal §6/§12                                                     | DEEP                                 |
| entity graph                                 | A8 §6, B9 #11                                                                            | DEEP                                 |
| measurement                                  | C3, D3, F4 §1, D2 §10                                                                    | DEEP                                 |
| Swagger / OpenAPI surface                    | E5 F9 (passing mention), F3 (verification cmd only)                                      | SHALLOW — candidate §3.2             |

Module-level check (nothing is unread): all 31 `src/modules/*` (incl. `category_v1`, `tag_v1`, `feed_v1`,
`analytics_v1` → D5 #10 / F3 §1.5; `search_v1` → D5 #4), all 29 `src/services/*` (incl. `session_store`,
`code_hash`, `email_alias` → C1 F3), all 9 `src/middlewares/*` (incl. `client_ip` → F1, `static_csrf` →
E5 A3), all 5 crates (`rux-request-gate` 23 reports, `rux-auth` 18, `rux-fcm` 3, `rux-webhook-crypto` 6,
`rux-provider-core` read via E3 §3/E2 §3), and every web route family (`(application)` reader/search/
bookmarks/settings → B3/E6/G2; `(auth)` → D2/D5; `(marketing)` → C4/E1; `design/*` → noindex+dev-guarded,
static; `firebase-config.js`, `llms*`, sitemap → B3/E6/F6).

---

## 2. Named-item verification (the seven the task requires)

| #   | Item                                       | Verdict                                               | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| --- | ------------------------------------------ | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `/api/quran/**` own authorization + limits | **Genuinely covered**                                 | B3 §1.4 enumerates all 29 GET routes (+ search = 30) with per-request yields + caps (`RESPONSE_CAP=300`, search ≤50 hits at 30/min) and the limit keys (§7.1/§7.3); authorization is "public by design" and verified (`quran_v1/mod.rs:11-53`, `cors.rs:20` `allow_credentials(false)` — B3 §3); edge contract E6 Q5/F7; fan-out E6 F4; licensed-translation residual B3 §4.4/F5 + E6 P0.8/F10; internal SSR token F1-7 + E6 F5. Nothing in this path lacks a report.                                                                                                                                                                                                 |
| 2   | Search as a data-extraction multiplier     | **Audited; not a multiplier**                         | D5 #4 read `search_v1/controller.rs:27-52` + validator + router: published posts only, ≤100/page, page ≤500, 30/min, triple `LIKE` scan (DoS-class, low value). Quran search read by B3 (≤50 hits, 30/60 s; `main.rs:899-901`) and given a rule (B4 R09). Bulk corpus is cheaper than scraping (B3 §1.5, E6 F10).                                                                                                                                                                                                                                                                                                                                                     |
| 3   | OAuth callback admission (D2 F7)           | **Deep enough to act**                                | D2 §4.5 verified each of the 5 providers' email-verification handling with `file:line`, the absence of Turnstile (grep: zero hits), the only limit (60/min nest ≈ 86 400 accounts/day/IP/provider), and the exact fix (per-IP/day + verification-identity cap; **no Turnstile** — breaks the redirect flow) or "state the residual". Reinforced by D5 #8, F2 §3 (callback amplification), F6 §3 (PKCE S256 + state TTL 600 s + single-use nonces verified). G4 item 7 sequences it.                                                                                                                                                                                   |
| 4   | paraglide / i18n surface                   | **F6 LOW holds — code-verified**                      | `hooks.server.ts:491-494`: `isUiLocale(locale)` gate before any catalog use; catalogs are repo JSON; `web/package.json` runs `i18n:check` (`check-i18n.mjs` + compile + namespaces) on `prebuild`/`precheck`/`prelint`/`pretest` (lines 12–33 verified); no user string reaches a template name; Svelte escapes. No mechanism found → LOW confirmed.                                                                                                                                                                                                                                                                                                                  |
| 5   | CSP nonce mechanism                        | **F6 "sound" claim supported by code, not assertion** | `freshNonce()` = `randomBytes(16).toString("base64")` per request (`hooks.server.ts:41-43`); disk cache stores `%csp-nonce%` placeholder and substitutes per request (`:364`, `:386`); `buildCsp` adds `'nonce-…'` only when present (`:59-101`); prerendered pages use a **boot-scanned sha256 map** (`web/server.ts` `scanPageScriptHashes`, `~:166-203` — F6's `server.ts` is `web/server.ts`, not `src/`) and 304s deliberately omit CSP (`hasBody` + comment, `~:261-270`). Residual: hash map keyed on exact pathname → trailing-slash variant falls back to hash-less CSP = fail-closed (JS-dead), not a hole.                                                 |
| 6   | Dependency supply chain / deny.toml policy | **Genuinely covered**                                 | B7 §2: `deny.toml` diff is comment-only (policy unchanged: `yanked="deny"`, `unmaintained="all"`, `unsound="all"`, sources deny, licence allowlist, `RUSTSEC-2023-0071` ignore with reason); the gate script still passes when run (B7 ran it + its 6 unit tests); only **execution** was removed from CI. Patches: 58 lines, benign and documented (single-letter-key regex; `$state.raw` upstream fix). **NEW (this session):** every GitHub Action in `.github/workflows/*` is SHA-pinned and permissions are minimal (`contents: read`; `packages: write` only on publish) — no report states this; it is a negative finding.                                     |
| 7   | `.env` / secrets incl. `app_constants`     | **F6's hole is the complete finding for this class**  | Verified `acl_service.rs:85-135`: denylist needles + `_KEY/_SECRET/_TOKEN/_PASSWORD` suffixes; `R2_ACCESS_KEY_ID` misses every needle (has `AWS_ACCESS_KEY_ID`, not `R2_…`) and ends `_ID` → imported (dev DB proof in F6). Swept every env name in `.env.example` + `docker-compose.yml`: all other secrets hit a needle (`COOKIE_KEY`, `FIELD_ENC_KEY`, `*_CLIENT_SECRET`, `*_TOKEN`, `SMTP_PASSWORD`, `DATABASE_URL`, `S3_SECRET_KEY`, `PUBLIC_TURNSTILE_SITE_KEY` ends `_KEY`); `CLOUDFLARE_ACCOUNT_ID` misses but is not a credential. Host-side `.env` is gitignored; key-leak blast radius is A9 §1.3; backups are B8 §7. **Nothing more plan-changing here.** |

---

## 3. Candidate gaps — strict plan-changing verdict

| #   | Candidate                                                                                                                                                                                                                                                                                              | Why it surfaced                                             | Plan-changing?                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 3.1 | **UGC output sanitization** — `utils/sanitize.rs` (ammonia) is in **no report**; it cleans only `paragraph.text`, `list` items and `raw.html` (`_ => {}` skips header/quote/embed/table/…); comment content is a raw `String` with no output sanitizer                                                 | Module sweep (sanitize.rs unread by any report)             | **No.** Posts require `ROLE_AUTHOR` (`post_v1/mod.rs:53`), and the web app has **zero** post/comment routes (`find web/src/routes -iname '*post*' -o -iname '*comment*'` → 0; only one `{@html}` in the whole app, for JSON-LD), so there is no in-repo HTML consumer; `feed_v1` XML-escapes and emits a text summary. A finding here would add one hardening line to the already-decided comment/post workstream (D2 §4.4), not add/remove/reorder a workstream. Consumer-side rendering (blog.hmziq.rs) is outside this repo's plan. |
| 3.2 | **Swagger UI / OpenAPI on the shipped build** — `openapi` is a default feature (`Cargo.toml:9-11`); `SwaggerUi` merged at `/api/docs` (`router.rs:179-185`), reachable publicly as `/api/api/docs` after Traefik's strip-prefix (`docker-compose.yml:191`); `/quran/openapi.json` is documented (§6.1) | Only F3's verification commands and E5 F9's passing mention | **No.** Public, unmetered, no data, no user input, no abuse lever; it is the same class as the already-found "unmetered public nests" detail (D5 #10). Detail only.                                                                                                                                                                                                                                                                                                                                                                    |
| 3.3 | Round-9 falsification (verify G1/G2/G3 byte math and reach; G2 states its per-row bytes are ±30 % estimates)                                                                                                                                                                                           | G2 §12 self-declared residual                               | **No.** It is a verification pass, not a new-surface audit; it can refine fix priority inside WS5/WS4a, not change the plan's shape.                                                                                                                                                                                                                                                                                                                                                                                                   |
| 3.4 | Owner decisions O1–O18 (C5 §2) + executing the C3/D3 measurement                                                                                                                                                                                                                                       | C5 §2 lists them as open                                    | **No — and not audit work.** These are policy/measurement actions for the owner (India mechanism, ban key, retention swap, artefact owner, base-rate measurement). No audit resolves them.                                                                                                                                                                                                                                                                                                                                             |
| 3.5 | F6's remaining LOW triage (SW cache, Turnstile, deps patches, `.env`, `db/` perms)                                                                                                                                                                                                                     | F6 §3 caveat: "no mechanism found, not proofs of absence"   | **No.** Independently re-verified by code this session for CSP, i18n, SW (`isCacheable` rejects `private`/`no-store` + `/api/` bypassed, `service-worker.ts:123-128,613-623`), Turnstile (fails closed in prod on unset secret/transport; only `success` parsed, `turnstile.rs:38-88`), patches, env (see §2.6/§2.7). None has a plan-changing mechanism.                                                                                                                                                                              |

Negative findings verified this session (no report states them; recorded so round 10 does not re-litigate):
session fixation is handled — login/2FA/passkey rotate the tower session id (`cycle_id`; `services/auth.rs:305,
710, 862`, binding replace tested); Firebase web config at `/firebase-config.js` is the standard public web
config (no Firestore/RTDB in use — `grep firestore web/src/lib/firebase` → 0); CI actions SHA-pinned (§2.6);
`/design/*` is prerendered but `noindex, nofollow` and dev-gated, static, no data.

---

## 4. Verdict — **(a) EXHAUSTED**

Another round of audit agents would add detail, not plan changes. Evidence:

1. **Subsystem coverage is complete at the audit's own unit of work.** All 31 API modules, 29 services, 9
   middlewares, 5 crates, and every web route family map to at least one report that read its code; the map
   is §1. There is no unread module, service, middleware, crate, or route family left to target.
2. **The named round-9 gap set was the last one, and it produced plan-changing findings — which is why the
   loop continued.** F6 named four areas; G1/G2/G3 deep-audited all four (36 findings), and G4 pre-mortemed
   the plan. After them, no report names a further area as under-covered, and my independent sweep found
   only the two detail-only candidates in §3.1–3.2.
3. **Every item the task flagged for verification resolves to "genuinely covered" or "code-verified low"**
   (§2): quran content API, search, OAuth admission, i18n, CSP nonce, deny.toml, `.env`/`app_constants`.
4. **The strict test eliminates every candidate.** Each remaining candidate either adds a line to an
   already-decided workstream (§3.1), duplicates an already-found detail (§3.2), is a verification pass
   (§3.3), or is an owner decision/measurement (§3.4). None adds, removes, or reorders a workstream, and
   none changes a decision.
5. **The remaining open items are not audit questions.** C5 §2's O1–O18 are policy choices (India, ban key,
   retention swap, artefact owner) and the C3/D3 measurement is an owner action. A round 10 would re-derive
   them, not resolve them.

Honest residual (stated, not hidden): coverage is not proof of absence. A new bug can still exist inside a
deep-audited subsystem — the loop breaks because there is no targeted, high-yield subsystem left, and the
marginal candidate set is detail-only. If the owner wants one more pass for assurance rather than for plan
input, the highest-yield target is §3.1 (UGC sanitizer block-type coverage) — but its outcome is a hardening
line, not a plan change.

**Next action is execution, not audit:** write the plan (C6 shape, G4 §7 edits, D2 §12 ordering), decide
O1/O8/O13/O17, and run the C3/D3 measurement. All three are already specified.

---

## Appendix — verification commands (all read-only, this session)

```bash
# named-item checks
sed -n '59,101p;355,395p;485,500p' web/src/hooks.server.ts        # nonce, placeholder, isUiLocale
sed -n '160,215p;255,290p' web/server.ts                          # prerendered sha256 scan; 304 omit CSP
sed -n '1,40p' web/src/lib/server/preview-headers.ts              # preview-server hash helper
grep -n "i18n:check" web/package.json                              # prebuild/precheck/prelint/pretest
sed -n '40,140p' rust/backend/api/src/services/acl_service.rs      # app_constants filter
sed -n '30,95p'  rust/backend/api/src/services/turnstile.rs        # fail-closed + success-only
sed -n '120,130p;610,625p' web/src/service-worker.ts               # isCacheable + /api bypass
grep -n "uses:\|permissions:" .github/workflows/images.yml         # SHA pins + minimal perms

# candidate checks
head -60 rust/backend/api/src/utils/sanitize.rs                    # ammonia block-type coverage
grep -rn "ammonia\|editorjs" *.md                                  # 0 hits → unaudited mechanism
sed -n '9,15p;179,185p' rust/backend/api/Cargo.toml rust/backend/api/src/router.rs  # openapi default
sed -n '184,192p' docker-compose.yml                               # strip-api → /api/api/docs reachable
grep -n "cycle_id" rust/backend/api/src/services/auth.rs           # fixation handled
grep -rn "firestore\|database()" web/src/lib/firebase/             # 0 hits → no rules surface
```
