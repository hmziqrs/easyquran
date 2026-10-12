# R2 — Cryptographically Sound Session Binding, Risk-Based Authentication, and Device Identity Standards

**Scope:** the "trustworthy" half of a fingerprinting/device-identity system — standards that _prove_ something about a device or session rather than merely observing it.
**Date:** October 2026. **Method:** primary standard texts (RFC/W3C/NIST), vendor engineering docs, and 2025–2026 security reporting. All 2026 browser-status claims are as-of Q3/Q4 2026 and must be re-checked before implementation.
**Consumer:** SvelteKit (browser) + axum (server) cookie-session web app, global audience, no native app.

---

## 1. DPoP — RFC 9449 (OAuth 2.0 Demonstrating Proof of Possession)

### 1.1 What it is and what it binds

RFC 9449 (published September 2023; https://www.rfc-editor.org/rfc/rfc9449) sender-constrains **OAuth access tokens and refresh tokens** to a client-held asymmetric key pair. It binds:

- the **token** → a public key, via the `cnf: { jkt: "<JWK-SHA-256 thumbprint>" }` confirmation claim (RFC 7800 claim, RFC 7638 thumbprint);
- the **proof** → one specific HTTP request, via `htm` (method), `htu` (URI, no query/fragment), `iat`, `jti` (≥96 bits of pseudorandom data or a v4 UUID), and, for protected-resource calls, `ath` (base64url SHA-256 of the access token);
- optionally the **authorization code** → the same key, via the `dpop_jkt` authorization-request parameter (§10), which closes the code-injection gap end-to-end;
- optionally the **proof's lifetime** → a server-chosen value, via the `nonce` claim (§8).

Key mechanics:

- Request header `DPoP: <proof JWT>`; the access token is sent with the `Authorization: DPoP` scheme, **not** `Bearer`.
- Any server receiving a bound token must confirm `cnf.jkt` matches the proof's `jwk` thumbprint and that `ath` matches the presented token.
- Supported algorithms per RFC 9449 §5 include ES256/ES384/ES512, EdDSA, PS256/PS384/PS512, RS256/RS384/RS512. Auth0 (Early Access + Enterprise Connections, Apr 2026) accepts ES256/384/512 and Ed25519 and **silently disables DPoP** on unsupported algorithms — a real operational trap.

### 1.2 Nonce and replay handling (server-side)

Three distinct mechanisms, often confused:

1. **Time window.** Servers MUST accept proofs only for a short window after creation — RFC 9449 §11.1 says "on the order of seconds or minutes." Clock skew is tolerated ("seconds or minutes" of future `iat`).
2. **`jti` replay cache.** §11.1: store each proof's `jti` keyed by target URI for the acceptance window, reject duplicates. This is a strong single-use guarantee but "may not always be feasible in practice, e.g., when multiple servers behind a single endpoint have no shared state." Mitigations: hash the `jti` before storing, reject oversized `jti` values (memory-exhaustion guard).
3. **Server nonce** (§8 authorization server, §9 resource server). Server returns `DPoP-Nonce: <value>`; clients must include it in `nonce`. Error signalling: `400 { "error": "use_dpop_nonce" }` at the token endpoint; `401 WWW-Authenticate: DPoP` at a resource server. Nonces must be unpredictable, and §11.3 forbids a **nonce downgrade** (a server that has issued a nonce MUST reject proofs lacking it). The design is deliberately stateless-ish: the client keeps one nonce, the server keeps a window of recent nonces, and mismatches are self-correcting through the error path. §11.2 notes the nonce exists mainly because XSS can pre-generate future-dated proofs.

Operational note that bites browser clients: `DPoP-Nonce` is not a CORS-safelisted response header, so a browser-based client needs `Access-Control-Expose-Headers: DPoP-Nonce` (explicit in RFC 9449 errata).

### 1.3 Adoption, 2025–2026

Real, but concentrated in the OAuth server/API world, not in browsers:

| Product             | Status                                                                                                                                                                                                                                                   |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Keycloak**        | Official (non-preview) from **26.4, Oct 2025**; `Require DPoP bound tokens` per-client; option to bind only refresh tokens for public clients; `dpop_jkt` support; enforcement across Admin/Account/UserInfo/Logout; token exchange carries the binding. |
| **Auth0**           | SPA SDK (`auth0-spa-js` ≥2.4.0) with `useDpop: true`; DPoP for **Enterprise Connections** Early Access (Apr 2026); server validation via `express-oauth2-jwt-bearer` and the new `auth0-springboot-api` starter (beta, `dpopMode: ALLOWED \| REQUIRED`). |
| **Okta**            | DPoP-bound tokens on resource servers; nonce rotation policy published (nonce renewed every 24 h, old value valid 3 days).                                                                                                                               |
| **Spring Security** | Resource-server DPoP validation (`ath`, `cnf.jkt`).                                                                                                                                                                                                      |
| **FAPI 2.0**        | Mandates sender-constrained tokens; DPoP is the recommended method where mTLS is unavailable.                                                                                                                                                            |
| **RFC 9700**        | "Use sender-constrained tokens (DPoP or mTLS) where possible."                                                                                                                                                                                           |

### 1.4 Is it practical for a normal web app? (the crux)

**No — not for cookie-based auth, and not for the session at all.** Two independent facts:

1. **No browser implements DPoP.** DPoP is an _application-level_ OAuth binding. Chrome/Safari/Firefox do not generate proofs and will never attach a `DPoP` header to a request for you. Every proof is produced by your JavaScript calling WebCrypto. The FIDO Alliance's DBSC/DPoP white paper (May 2025) frames DPoP as the browser's _counterpart_ to DBSC precisely because DBSC is the browser-integrated mechanism and DPoP is the OAuth/token mechanism.
2. **WebCrypto key storage does not survive XSS.** The persistent industry finding (InfoQ, "The DPoP Storage Paradox", 2026) is that a non-extractable `CryptoKey` in IndexedDB is _not_ a boundary: an XSS payload retrieves the key handle and calls `crypto.subtle.sign()` as a **signing oracle**, minting valid proofs for arbitrary requests without ever extracting the private key. Firefox has historically also failed to persist `CryptoKey` objects in IndexedDB, so the "portable non-extractable key" story is browser-dependent. The industry consensus mitigation is the **BFF (Backend-for-Frontend) pattern**: keep tokens and keys server-side, hand the browser only an `HttpOnly` session cookie.

Consequences for a SvelteKit + axum app:

- If the browser authenticates with an **HttpOnly cookie** (the shape described in the brief), DPoP adds **nothing** to the browser↔server hop. There is no standard "DPoP-bound cookie," and inventing one means signing a proof per request from JS, which XSS can forge — plus it forces a signature per request and breaks HTTP caching and form POSTs.
- DPoP is the right tool only where an OAuth **token** is handed to a client you don't fully control: mobile apps, CLI/M2M, or third-party API access. Even for a SPA, the safe pattern is BFF + cookie, so DPoP is then a server-to-server concern (axum as the client to an IdP).
- If we ever issue tokens directly to the browser, the RFC 9700 / current guidance path is PKCE + short-lived tokens + DPoP-with-BFF, never tokens in `localStorage`.
- Rust tooling exists if we need it: `dpop-verifier` (ES256/P-256, EdDSA/Ed25519, manual `htm`/`htu`/`iat`/`ath` checks, pluggable replay store, DPoP-Nonce support) — small but a reasonable starting point; otherwise use a vetted JWT/JOSE stack and implement §4.3 checks explicitly.

**Verdict:** DPoP = high value on the OAuth plane, zero value for a cookie session, and _net negative_ if it pushes tokens into browser JS.

---

## 2. Device Bound Session Credentials (DBSC)

### 2.1 Status timeline

| Date        | Event                                                                                                                                                                                                                 |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Apr 2024    | Google announces DBSC concept ("Fighting cookie theft using device bound sessions").                                                                                                                                  |
| Apr 2025    | Origin trial in **Chrome 135** (developer.chrome.com/blog/dbsc-origin-trial).                                                                                                                                         |
| Jun 2025    | W3C WebAppSec incubation; FIDO Alliance white paper positions DBSC as complementary to passkeys.                                                                                                                      |
| 21 Aug 2025 | **W3C First Public Working Draft**: https://www.w3.org/TR/2025/WD-dbsc-1-20250821/                                                                                                                                    |
| Feb 2026    | Chromium **Intent to Ship** (Windows-only initially, Finch-controlled, WPT suite).                                                                                                                                    |
| Apr 2026    | **GA in Chrome 146 for Windows** (blog.google "Protecting Cookies with DBSC"); Edge 147 lists DBSC as released (Apr 2026).                                                                                            |
| May 2026    | **Google Workspace** DBSC GA and **on by default**, gradual rollout from 25 May 2026; _no admin control to disable_, audit-log visibility; works with context-aware access.                                           |
| Oct 2026    | Chrome: Windows ✅ TPM, macOS rollout from **Chrome 147** ✅ Secure Enclave; Edge ✅ (TPM/Secure Enclave); Linux/Android/iOS ✅ none. Mozilla position **negative**; WebKit **under discussion / no implementation**. |

### 2.2 How the binding works

Web API + HTTP protocol (all site-scoped like cookies; keys deleted with site data):

1. **Registration.** Server responds with `Sec-Session-Registration: (ES256);path="/refresh";challenge="..."`. The browser generates a key pair, stores the private key via OS hardware APIs (TPM on Windows, Secure Enclave on macOS), and registers with the server. Server binds the public key to the session and issues a **short-lived** auth cookie.
2. **Refresh (proof of possession).** When the short-lived cookie expires, Chrome calls the server-declared refresh `path` asynchronously. If challenged (`Sec-Session-Challenge`), the browser returns `Sec-Session-Response: <signed proof>`; the server verifies and issues a fresh short-lived cookie. Chrome defers the original request until the refresh lands, so the app sees cookies as "always present."
3. **Hardening.** The refresh endpoint should require the `Sec-Secure-Session-Id` header (proves browser-initiated, not cross-site), a narrow CORS policy, and `X-Frame-Options`/`Cross-Origin-Resource-Policy` refusal. Cross-origin/federated binding is arranged via `/.well-known/device-bound-sessions`.
4. **Deliberate non-features.** No attestation of the key's hardware, no device identifier, no cross-session correlation, no cross-site tracking.

### 2.3 What it defends against — and what it does not

**Defends:** cookie exfiltration + **replay from another machine**. Stealing the cookie buys the attacker only the remaining lifetime of a short-lived cookie; refresh requires a non-exportable key.

**Does not defend (this matters more than the marketing):**

- **On-device code execution.** The FIDO white paper is explicit: DBSC "is not a replacement for client-side malware protection"; in-browser malware has full visibility into both the session cookie and the short-lived `auth_cookie`. NIST's own framing agrees (see §5).
- **The live authenticated browser.** SpecterOps' "Return of the Cookie Monster" (13 Aug 2026; CSA research note 17 Aug 2026, Chrome 147.0.7727.102 / Edge 147.0.3912.98) activates Chrome's DevTools Protocol _inside the running browser_ via a message-queue-injected stub, then calls `Storage.getCookies` and issues requests through the browser's own authenticated context. **DBSC's key is never extracted** — the browser itself is the credential. Google's pivot away from disk-cookie theft to remote debugging was already visible in 2025.
- Phishing of the login event, credential stuffing, XSS, and account recovery flows.
- Replay within the short-cookie window (Mozilla's objection #1: "it leaves a window in which an attacker can use a stolen session cookie").

Mozilla's recorded objections (standards-positions#912, position: **negative**): the replay window exists; the separate reauthentication flow is a bespoke protocol that doesn't match existing cookie handling; and Mozilla does not want TPM-backed credentials or device attestation to become an _expectation_ on the web. Google's reply in the same thread: per-request signatures were explored and judged infeasible, and PQC will make them harder; DBSC explicitly carries no attestation.

### 2.4 What a non-Google browser does in 2026

Nothing. Safari and Firefox fall back to the current bearer-cookie model. Implementation guidance (FIDO white paper) is explicit: servers must work with a **mixed population** — use DBSC when the browser accepts `Sec-Session-Registration`, tolerate unbound cookies otherwise; optionally enforce DBSC-only for tightly controlled cohorts (enterprise-registered devices). So DBSC is an _opportunistic upgrade for a subset of Chrome/Edge traffic_, not a security guarantee for the audience.

**Deployment cost for us:** two endpoints (register, refresh) + header handling, flat cookie-semantics preserved on the front end. Low code cost, high payoff **only** for Chrome/Edge desktop on Windows/macOS.

---

## 3. WebAuthn / passkeys as device anchors

### 3.1 `userVerification`, UP/UV

- `userVerification: 'preferred' | 'required' | 'discouraged'` on both create and get. `required` fails locally if no UV method is available; `preferred` can return UV=false (e.g. macOS without Touch ID).
- The authenticator data returns **UP** (user present) and **UV** (user verified) as _signed_ flags, so a server that verified the signature can trust them.
- Policy (this is what makes UV a risk input): require UV (`'required'`) for step-up and high-value actions; `'preferred'` for ordinary sign-in so users without biometrics aren't locked out; **server must inspect the UV flag** and treat UV=false as single-factor. NIST formalizes exactly this: "Verifiers SHALL indicate that UV is preferred and SHALL inspect responses to confirm the value of the UV flag… If the user is not verified, agencies SHALL treat the authenticator as a single-factor cryptographic authenticator" (SP 800-63B-4, Appendix B).

### 3.2 Resident keys / discoverable credentials

`residentKey: 'required' | 'preferred' | 'discouraged'` (plus legacy `requireResidentKey`). Discoverable credentials are what let a site offer **usernameless** sign-in and conditional UI autofill. Consumer platform passkeys (iCloud Keychain, Google Password Manager, Windows Hello) are discoverable by default. Requirement to note: discoverable credentials created on a platform are effectively **synced** on that platform's credential manager (see §3.3), so "resident" ≠ "device-bound."

### 3.3 `backupEligible` (BE) / `backupState` (BS) — the sync question

WebAuthn L3 adds two flags in the authenticator data:

- **BE (backup eligible)** — the credential _may_ be copied into a sync fabric (exportable key material). A **device-bound** credential has BE=0.
- **BS (backup state)** — the credential _has been_ backed up/synced at some point. BE=1 does **not** imply BS=1 ("can be synced" ≠ "has been synced").

How to read them:

- BE=0, BS=0 → device-bound: hardware-bound key (Secure Enclave/TPM/passkey hardware), strongest available anchor, cannot be cloned by the provider.
- BE=1, BS=0 → sync-capable, currently only on this device.
- BE=1, BS=1 → synced/backed-up: the private key exists in a cloud fabric and can appear on other devices of the same account.

Caveats:

- The flags are **provider-asserted**. A malicious or compromised password manager can lie about them; treat them as policy hints, not proofs.
- NIST's rule set (SP 800-63B-4 App. B): "This flag is necessary to distinguish authenticators that are device-bound from those that may be cloned to more than one device… Verifiers **MAY** use this flag to establish policies that restrict the use of syncable authenticators." For **BS**: "Verifiers MAY use this flag to establish restrictions on authenticators that are synced… Agencies **SHOULD NOT** condition acceptance based on this flag for public-facing applications due to user experience concerns." Syncing _inherently_ means the key is exportable, which "violates the non-exportability requirements of AAL3" — so **syncable authenticators SHALL NOT be used at AAL3** (SP 800-63B-4 §2.3).
- Assume syncable authenticators may be **shared** between users (NIST App. B "Sharing"), because public-facing RPs have no visibility into the sync fabric's sharing model.

Practical policy for a global app: accept synced passkeys for login (they're phishing-resistant, replay-resistant, and the overwhelming majority of consumer passkeys), but do **not** treat any passkey as a hardware device anchor unless BE=0 (e.g. a security key, Windows Hello with hardware key, or a platform authenticator whose provider reports non-exportable). For forcing a true device anchor, either require an AAL3-shaped path (hardware key) or pair the passkey with DBSC.

### 3.4 Cross-device (hybrid) authentication — implications

Hybrid transport (CTAP 2.2 / caBLE) lets a phone sign for a desktop: the desktop shows a QR code, the phone scans it, the two establish an encrypted BLE channel with a **proximity check**, and a normal WebAuthn ceremony runs over that tunnel.

Consequences for device identity:

- The **credential lives on the phone**; the **session lives on the desktop**. Any device identifier you derive must be attributed to the _browser doing the request_, not to the authenticator, or your "device" model silently merges two machines. This is a feature for usability and a hazard for device-based risk models.
- The proximity check authenticates the _tunnel_, not the person: a remote attacker cannot relink a scanned QR challenge, but there is no signal about which human is present beyond local UV on the phone.
- Treat hybrid ceremonies as a distinct `amr`/transport value (`hybrid`, `internal`, `usb`, `nfc`, `ble`) stored in the session/device record, and score them accordingly (a passkey assertion over hybrid from a brand-new desktop is _not_ evidence about that desktop's hardware).

### 3.5 Signal APIs (WebAuthn L3)

Three static methods plus capability detection:

- `PublicKeyCredential.getClientCapabilities()` — feature detection without user-agent sniffing.
- `PublicKeyCredential.signalUnknownCredential({ rpId, credentialId })` — call after a login fails because the credential is gone. Safe pre-auth; leaks no credential count.
- `PublicKeyCredential.signalAllAcceptedCredentials({ rpId, userId, allAcceptedCredentialIds })` — call after sign-in and after deletion so the provider hides stale passkeys (hides, does not delete; restorable). **Warning:** an incomplete list hides valid passkeys and can lock users out; an empty list hides everything.
- `PublicKeyCredential.signalCurrentUserDetails({ rpId, userId, name, displayName })` — keeps provider-side labels in sync after a username/email change.

Support (per Chrome's docs, cross-checked with the browser-compat badges): Chrome **132+** (desktop + Android) and Edge 132+; **Safari 26** lists support; **Firefox is not supported** and has not published a position. The "Chrome desktop only, Google Password Manager only" restriction noted in the 2024 Intent to Ship has since widened, but third-party extension providers "decide whether to reflect the signal," so coverage is uneven.

---

## 4. Token Binding, mTLS, OAuth 2.1, RFC 9700

### 4.1 Token Binding — dead, and why

Token Binding = RFC 8471 (protocol), RFC 8472 (TLS extension), RFC 8473 (over HTTP, `Sec-Token-Binding` header). It binds a token to a _TLS connection_ via an exported-key material ("token binding ID"), which was strictly stronger in one respect than DPoP: the binding material never leaves the TLS stack, so even the application couldn't leak it.

Why it died:

- **It required ecosystem-wide deployment.** The binding is negotiated in the TLS handshake, so every hop — browser, CDN, load balancer, reverse proxy, origin — must participate. Any middlebox that terminates TLS breaks the binding or forces fallbacks.
- **Almost no browser adopted it.** Chrome shipped support in 65 (2018) and **removed Token Binding entirely in Chrome 130 (2024)**, citing insufficient adoption; the FIDO white paper notes "only one major browser currently offers support." There was no Safari/Firefox path, no server-side incentive, and privacy concerns about a long-lived cross-site correlator.
- The spec itself is candid about limits: RFC 8471's security considerations note that a malicious provider "can remove the binding or substitute a Token Binding ID of their choice without detection."

**Verdict: treat RFC 8471/8472/8473 as historical.** Do not design around it; do not cite it as a live option. Learn the actual lesson: _sender-constraining mechanisms only get deployed when they can be adopted incrementally, at the application layer, without breaking the rest of the stack_ — which is exactly DPoP's design and exactly DBSC's design.

### 4.2 mTLS-bound tokens (RFC 8705)

`OAuth 2.0 Mutual-TLS Client Authentication and Certificate-Bound Access Tokens`: the client authenticates with a certificate; the token carries a certificate thumbprint (`cnf.x5t#S256`); resource servers verify the presented cert matches.

- Strengths: strong, battle-tested, TLS-native, formal-analysis-friendly; mandated/approved in FAPI 2.0; no per-request application-layer signature.
- Costs: PKI + certificate issuance/rotation/revocation lifecycle; client-cert provisioning is hostile to browsers and consumer devices; proxy/CDN and re-termination issues (you must enforce the cert at the TLS edge and forward the thumbprint securely); poor fit for public clients.

### 4.3 DPoP vs mTLS — selection

| Dimension                               | DPoP (RFC 9449)                                                     | mTLS (RFC 8705)                               |
| --------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------- |
| Layer                                   | Application (JWT per request)                                       | Transport (client cert)                       |
| Works for public clients / mobile / SPA | Yes                                                                 | Poorly                                        |
| Works for backend M2M                   | Yes                                                                 | Yes — strong fit                              |
| Infrastructure required                 | JOSE + replay store + nonce                                         | PKI/CA, cert lifecycle, TLS edge enforcement  |
| Per-request cost                        | One signature + one JWT verify                                      | TLS-level; amortized over connection          |
| Proxy/CDN friendliness                  | Good (+nonce via headers, CORS exposure)                            | Harder (re-termination breaks the binding)    |
| Regulatory traction                     | FAPI 2.0 recommended where mTLS unavailable                         | FAPI 2.0, open banking, enterprise APIs       |
| Key custody risk                        | Private key on client (XSS = oracle; browser has no native support) | Private key in OS/keystore (often TPM-backed) |

Rule of thumb: **mTLS for controlled infrastructure and machine clients; DPoP for public/native/mobile clients and API tokens**; both only where tokens actually cross a boundary. For a BFF-shaped cookie app, neither touches the browser session.

### 4.4 RFC 9700 (OAuth 2.0 Security BCP, Jan 2025) and OAuth 2.1

RFC 9700 supersedes RFC 6819, updates RFCs 6749/6750, and is the normative security baseline. Relevant requirements/recommendations:

- **PKCE for all Authorization Code flows** (not just public clients).
- **Exact redirect-URI matching**; no wildcards; no fragment-based token delivery.
- **Implicit flow and Resource Owner Password Credentials deprecated/forbidden.**
- **Sender-constrained tokens preferred** (DPoP or mTLS) — "use sender-constrained tokens where possible."
- **Pushed Authorization Requests (PAR)** for high-security deployments.
- **Mix-up attack defenses** and issuer identification requirements in discovery.
- Keys: modern implementations (Auth0, Okta, Keycloak) enforce these by default; Spotify's Nov 2025 cutover to mandatory PKCE broke apps that had been relying on legacy behavior — a reminder that these are now enforced in practice.

**OAuth 2.1 status:** still an Internet-Draft in 2026 (`draft-ietf-oauth-v2-1-15` as of mid-2026) — it consolidates RFC 6749 + RFC 7636 (PKCE) + RFC 9700 and deletes the dangerous options. Cite it as a draft, never as "RFC xxxx."

---

## 5. Risk-Based Authentication and NIST SP 800-63B-4

### 5.1 The standard signal set

Grouped by what they actually tell you:

1. **Device signals.** New/known device (from a server-side device record, not a cookie alone); device age and usage history; device reputation (has this device been used in prior abuse?); whether the device is the _current_ device for other sessions.
2. **Network/geo.** IP reputation (abuse/malware/proxy/VPN/Tor/datacenter/cloud ASN); ASN change from the established device profile; geolocation and geovelocity; implausible-travel (two logins whose distance/time ratio is physically impossible); impossible-travel false positives are constant (corporate VPNs, CGNAT, mobile roaming, IPv6 rotation, iCloud Private Relay) — always pair with another dimension.
3. **Credential/account state.** Password/email/phone change recency; MFA reset; recovery activity; recent failed attempts; whether the session was created by a credential-stuffing-favourable path.
4. **Behavioural.** Typing cadence, pointer kinematics, navigation cadence, request velocity, scripted-timing regularity; headless/automation tells (webdriver flags, missing APIs, CDP artefacts, UA/TLS/JA4 inconsistencies).
5. **Session characteristics.** `amr`, authentication age (auth_time), AAL of the session, session age/idle time, whether elevation happened.

### 5.2 Step-up flows

Standard shape: signals → composite risk score → three-way decision **Allow / Step-up / Block**. Common production tuning targets are roughly 70% allow / 25% step-up / 5% block, though these are deployment-specific. Step-up options in order of preference: fresh WebAuthn assertion with `userVerification: 'required'` → TOTP/OTP → email/out-of-band. Requirements: bind the step-up to the _current_ session (do not let the elevated state leak into other sessions or devices); record the elevated `amr` and timestamp for auditability; expire the elevation (e.g. 15 minutes for high-risk actions); and never let step-up silently downgrade (if the user fails, deny rather than fall back to a weaker method).

### 5.3 What NIST SP 800-63B-4 (July 2025) actually says

This revision is unusually relevant: it _names_ DBSC, blesses proof-of-possession sessions, and adds session monitoring.

- **AAL overview (SP 800-63B-4 §2):** "At all AALs, indicators of potential fraud, including applicable indicators described in Sec. 5.3, MAY be used to lower the risk of misauthentication. For example, authentication from an unexpected geolocation or IP address block (e.g., a cloud service) might prompt the use of additional risk-based controls." CSPs/verifiers "SHALL assess their use of indicators of potential fraud for efficacy and to identify and mitigate potential negative impacts"; they "SHALL include fraud indicators in the authentication privacy risk assessment"; and using fraud indicators "does not impact or change the AAL of a transaction or substitute for an authentication factor."
- **Session bindings (§5.1):** a session secret SHALL be shared between subscriber software and the service; continuity of authenticated sessions "SHALL be based on the possession of a session secret… or possession of the secret SHALL be proven using a cryptographic mechanism." Bearer session secrets "SHOULD NOT be persistent"; **but** "Some technologies (e.g., the emerging device bound session credentials specification [DBSC]) mitigate the risk of theft of session secrets by using cryptographic protocols that prove the possession of a session secret rather than using them as bearer tokens… Session secrets used with such proof of possession techniques MAY persist. However, RPs and CSPs SHALL ensure that the session lifetime limits described in Sec. 2.2.3 are enforced even when knowledge of the session secret is demonstrated." This is the strongest standards endorsement of DBSC-shaped designs that exists today.
- **Session secret requirements (§5.1):** established during/immediately after authentication; ≥64 bits from an approved RBG; erased/invalidated on logout; transferred over an authenticated protected channel or derived from the mutually authenticated channel; timeouts per AAL; unavailable to intermediaries; SHOULD NOT be stored in insecure locations such as HTML5 Local Storage. Cookies SHOULD use `__Host-` prefix, `Path=/`, `SameSite=Lax/Strict`, HttpOnly, and contain only an opaque identifier with no cleartext PII.
- **Reauthentication (§5.2 + §2.1.3/2.2.3/2.3.3):** AAL1 ≤30 days overall (inactivity optional); **AAL2 ≤24 h overall, ≤1 h inactivity**, with single-factor reauth permitted after inactivity when the overall limit has not been exceeded; AAL3 ≤12 h overall, ≤15 min inactivity. Crucially: "The limits MAY also be extended when higher security session maintenance technologies (e.g., device-bound mechanisms) are used."
- **Session monitoring / continuous authentication (§5.3, new):** "MAY be performed by the RP in coordination with the CSP or IdP"; characteristics that MAY be evaluated: usage patterns/velocity/timing; behavioural biometrics (e.g. typing cadence); **device and browser characteristics (e.g. accepted languages)**; geolocation; **IP address characteristics (e.g. whether the IP is in a block known for abuse)**. On suspected fraud the RP SHOULD "reauthenticate, terminate the session, or notify appropriate support personnel." Privacy: "Collection, the storage of expected subscriber characteristics, and the processing of session characteristics SHALL be included in the privacy risk assessment described in Sec. 7." (This is the normative hook that forces your fingerprinting design to be a privacy-assessment artefact, not just an engineering choice.)
- **Appendix B (Syncable Authenticators, normative):** BE distinguishes device-bound from cloneable; BE "is necessary to distinguish authenticators that are device-bound from those that may be cloned"; cloning/exportability is what disqualifies AAL3; verifiers MAY restrict syncable authenticators by policy; SHOULD NOT condition on BS for public-facing apps; when the subscriber provides the authenticator, the verifier "SHOULD leverage additional risk indicators or trust signals… to increase confidence in the authentication event"; assume syncable authenticators may be shared.

Interpretation for our threat model: **NIST explicitly allows device/geolocation/IP signals as _risk inputs_ with a mandatory privacy assessment, expects a stable server-side session secret, permits PoP-bound sessions to persist, and treats device-bound (BE=0) credentials as the only AAL3-grade anchors.** It does not bless "fingerprint and silently track" and it does not bless attestation-gating.

---

## 6. Account-Takeover Detection in Practice

### 6.1 Which signals actually catch ATO early

Ranked by signal-to-noise in published attacker-behaviour data:

1. **Credential-stuffing pattern, cross-account.** Many usernames tried from one device/IP/ASN; or one username tried from many IPs with similar fingerprints. Detecting the _shape_ (success/failure distributions, velocity, sequence) beats detecting any single request. Kasada's report is built on exactly this: 22 credential-stuffing groups, "slow-and-steady credential testing to avoid detection," and 65% of ATO attacks using advanced automation (solver services, CAPTCHA farms, residential proxies).
2. **New device + successful login from a new network** at the same time. Either alone is weak; together they're the canonical ATO tripwire. Requires a server-side device record that survives cookie clears (see §8).
3. **ASN change and datacenter/proxy origin.** Residential-proxy services exist precisely to defeat IP reputation, so ASN/proxy signals are a _hint_ — treat "datacenter ASN + new device + password login + no passkey" as a compound rule rather than a single block.
4. **Headless / automation tells.** Inconsistent UA ↔ client hints, missing/false `navigator.webdriver`, CDP artefacts, impossible timing, non-browser TLS (JA4) under a browser UA.
5. **The persistence pivot: credential change followed by email change.** This is where the damage happens. Attackers who land an ATO immediately change the account email (or add a second one), reset recovery codes, and create long-lived API keys. Detection rule: any email change, password change, MFA reset, or session-minting API key creation within N minutes of a _new-device_ login or elevated-auth session should be held (delay + notify + require fresh UV step-up), and the old email should be notified independently of the session.
6. **Session-level anomalies.** Session created without a fresh authentication event, token replay (same token family from two networks), session used from two ASNs concurrently, or a session surviving a password reset.
7. **MFA fatigue / push bombing.** Repeated denied pushes preceding a success; correlate push attempts and step-up failures per account.

### 6.2 Published numbers (what's actually documented)

- **ATO attacks +250% in 2024**; **22 credential-stuffing groups targeted 1,000+ major organizations**; **65% of observed ATO attacks used sophisticated automation** (62% sophisticated + 3% highly sophisticated: CAPTCHA bypasses, solver services, residential proxies); **85% of companies that were breached had bot-detection in place and still lost**. Kasada 2025 ATO Attack Trends (report + 4-takeaways blog). Case data: a major retailer saw a **32×** increase in bot-driven login attempts on Black Friday, with **72%** of total traffic from malicious bots; travel/hospitality saw **+40%** ATO incidents during holiday booking.
- **6.2 million customer accounts compromised across 1,027 large organizations in 2024** (Kasada 2025 report, cited in secondary reporting).
- **Detection latency remains the real problem:** IBM's Cost of a Data Breach puts average breach detection at **194 days** (2024 figure, cited in Kasada's write-up). That is the number to quote when someone claims a detection rate.
- **Distinguish "attack volume" from "detection rate."** Public "ATO detection rate" figures are vendor-benchmark marketing — algorithm, threshold, and population are never disclosed. The defensible published findings are (a) attacks are growing, (b) existing bot defenses are bypassed, (c) detection is slow. Build our own labelled dataset (step-up outcomes, confirmed compromises) and measure precision/recall for _our_ thresholds instead of importing someone's percentage.
- Note the structural implication of the Kasada data: **pre-auth bot defense alone is insufficient** (85% failure rate), so ATO work has to include post-auth signals (new-device correlation, credential/email change holds) and step-up, not just CAPTCHA-style gates.

---

## 7. Client Attestation on the Open Web in 2026

### 7.1 Privacy Pass / Private Access Tokens (PAT) / PACT

- **Privacy Pass** is standardized as an IETF series: **RFC 9576** (architecture), **RFC 9577** (HTTP authentication scheme — `WWW-Authenticate: PrivateToken` challenge, `Authorization: PrivateToken token=...` redemption), **RFC 9578** (issuance protocols), on RFC 9497 (VOPRF) and RFC 9474 (RSA blind signatures). Four roles: Client, Attester, Issuer, Origin; blind issuance gives unlinkability between issuance and redemption.
- **Private Access Tokens (PAT)** — the Apple/Cloudflare deployment (2022): attester = Apple App Attest hardware, issuer = Cloudflare, client = Safari on iOS 16 / macOS 13. Cloudflare's Managed Challenge asks for the token automatically, and "any customer using this feature… automatically takes advantage" without site changes. **Coverage has stayed Apple-shaped**: PAT shipped "in Safari and worked in exactly one browser"; Google's Trust Tokens / Private State Tokens "never landed multi-vendor support." Chrome/Android never produced PATs. Cloudflare's Turnstile-based attester is the accessible fallback.
- **PACT (Private Access Control Tokens)** — announced **22 June 2026** by Cloudflare with **Chrome, Edge, Firefox and Shopify**; to be submitted for standardization. Cryptographically the same blinded-token core, extended with **Anonymous Credit Tokens** (credential-with-state; `draft-schlesinger-cfrg-act-01`, CFRG, Feb 2026) so an origin can adjust a credential's trust when its perception of the holder changes. First user-visible surface: **Edge Canary "Automatic Verification"** (Aug 2026) over Chromium's existing `PrivateStateTokens` plumbing. **Apple is absent.**
- **What PACT does not do** (important for a global audience):
  - No token ⇒ no information. Unattested Linux desktops, older/no TPM machines, corporate-locked-down devices, containers, Tor — and _also_ the residential-proxy stealth automation that defeats IP reputation. From the origin's view, a legitimate Linux user and a stealth scraper are identical without a token.
  - Adversarial agents and credential-stuffing frameworks will never request a token; they run on the unattested surface by design.
  - Issuer trust/governance is unresolved: no public issuer-list policy, no cross-browser attestation forum, no per-issuance transparency log is possible without breaking the unlinkability that the protocol exists to provide.
  - Cloudflare's own Radar puts automated requests at **58% of HTTP traffic (June 2026)** — PACT authenticates the well-behaved half of the internet; the problem traffic sits outside it. Cloudflare launched PACT alongside **Precursor** (client-side behavioural runtime detection), which tells you what they think PACT alone is worth.

**Practical rule:** accept a valid PrivateToken/PAT from a trusted issuer as **one strong positive signal**, never as the sole gate. Right now, on a plain website for a global audience, that signal reaches a minority slice (Apple devices via Cloudflare; Edge Canary; eventually Chrome/Firefox via PACT). It is not a general device attestation.

### 7.2 Turnstile (the pragmatic baseline)

Cloudflare Turnstile = widget + server-side `siteverify` call, no device attestation, privacy-preserving by design (it can be the _attester_ behind Privacy Pass). Scale: about **3 billion verifications on a typical weekday**, with **>23,000 new widgets in a recent week**. Bypass economics: solver services, AI/human CAPTCHA farms, and residential proxies defeat challenges at scale (Kasada: 65% of ATO attacks use advanced automation; 85% of breached companies had bot detection). Turnstile is therefore a **friction/cost raiser and a signal source**, not a device identity: it tells you "a human-ish client solved this," and it works for every browser.

### 7.3 Apple App Attest / DeviceCheck

- iOS/iPadOS **14+**, app-only (`DCAppAttestService`), Secure Enclave-backed attestation key; server validates the attestation object; assertions are signed per request; `DeviceCheck` provides two persistent per-device bits (`DCDevice`).
- Limits: no website surface (app only); Apple's own guidance is to keep `attestKey()` calls "within double digits per second" for a given app and warns that undocumented rate limits/throttling can appear; Secure Enclave availability bounds device coverage; the device "identifier" is app-scoped (per `teamIdentifier`/bundle); the whole thing only exists if you ship a native iOS app.
- **2026 note:** App Attest gained a **fraud metric** (WWDC26 session 201) — useful as a risk input, still app-only.

### 7.4 Google Play Integrity

- Android **app-only** (or Android WebView via the WebView Media Integrity API). Verdicts cover app recognition, device integrity (MEETS_DEVICE_INTEGRITY etc.), and account licensing. Classically requires Google Play services + a certified device; **hardware-backed attestation is being phased in for the stronger verdicts**, which excludes unlocked/custom ROMs (GrapheneOS has objected publicly) and many "uncertified" devices.
- **SafetyNet Attestation is dead** — all calls stopped working **31 January 2025**; Play Integrity is the replacement with no alternative path through Google's tooling.
- Same structural limit as App Attest: not available to a plain website, and the population it can flag is precisely the population some legitimate users occupy.

### 7.5 Web Environment Integrity (WEI) — what happened

- Prototype in Chromium from **May to November 2023**; proposed as an API returning a third-party-signed "this client is authentic" token.
- Backlash was immediate and broad: Mozilla opposed ("mechanisms that attempt to restrict these choices are harmful to the openness of the Web"), Vivaldi called it "simply dangerous," the FSF called it "an all-out attack on the free Internet," Brave refused to implement it, the EFF opposed it, and the W3C stated WEI "has no standing at W3C."
- **Google abandoned it on 2 November 2023** and removed the prototype, replacing it with the **Android WebView Media Integrity API** — confined to apps/WebViews, not the open web. Explicitly an "abandoned API proposal."

### 7.6 State of play, and what actually works on a plain website for a global audience

The pattern from 2023→2026 is consistent: **general-purpose browser attestation keeps failing** (WEI killed, Token Binding removed, Private State Tokens never multi-vendor, PAT Apple-only for four years), while **application-scoped, incremental mechanisms keep succeeding** (DPoP for tokens, DBSC for cookies, passkeys for logins).

What you can deploy on a plain global website today:

1. **Turnstile (or equivalent) + rate limiting + risk scoring** — works everywhere, bypassable, cheap.
2. **WebAuthn/passkeys** — works in every current major browser, phishing-resistant, and with BE/BS gives you an _honest_ device-bound signal.
3. **Server-side device records + step-up** — zero browser support needed, the actual backbone of RBA.
4. **Accept PAT/PACT tokens as an extra positive signal** when they arrive; do not require them.
5. **DBSC** where the browser supports it (Chrome/Edge desktop, Windows/macOS), with graceful fallback.

What you cannot do: attest a device on the open web in a way that covers a global audience, including Linux, browsers without a secure element, corporate-managed machines, and privacy tools. Trying is how products lock out legitimate users and _still_ miss the bots.

---

## 8. Signal Design Patterns

### 8.1 Raw vs derived identifiers

Never store raw fingerprint components (UA string, IP, canvas hash, font list, screen geometry). Store a **derived, keyed digest** and keep the raw value, if needed at all, in short-lived logs with its own retention policy.

Requirements for the derived form:

- **Deterministic** for legitimate correlation (same input → same digest under the same key).
- **Not enumerable** by an attacker who has the database but not the key. Low-entropy inputs (phone numbers, IPs, screen sizes) are trivially enumerable without a key; a shared or date-derived public salt adds no protection.
- **Domain-separated** per identifier class, so the same raw string used as two different classes cannot accidentally join.
- **Canonicalized before hashing** — E.164 for phones, lowercase for emails, strip ports/brackets from IPv6, normalized numeric rounding for device metrics.

The construction that meets all four: `HMAC-SHA-256(pepper, class || version || normalized_value)` with the pepper held in the platform secret store, never in the database. Properties: equality correlation survives, rainbow tables and candidate testing die, and a leaked digest column discloses only equality relationships. Ship a per-row **hash version stamp** so schema changes and legacy unkeyed digests are self-describing and excludable from privacy claims.

### 8.2 Keyed HMAC with a rotating secret

Rotation is a real requirement (compromise, hygiene, policy) and it collides head-on with determinism: rotate the pepper and every future digest changes, orphaning every stored one. Two workable patterns:

1. **Dual-key read window (simplest, widely used).** On rotation, demote the old key to a "previous pepper" slot; new writes use the new key immediately; equality-matching reads compute candidate digests under **both** keys until every row written under the old key has aged out or been rewritten; then delete the old key and close the window. Steady-state cost is unchanged (one HMAC per lookup); only the window pays double. Cache the imported key by key value, not as a singleton, so rotation takes effect immediately in warm workers.
2. **Indirection/mapping layer (stability across arbitrary rotation).** Store `hash_output → stable_pseudonym` (the pseudonym being a random value/UUID). On rotation, recompute the hash output, look it up, and replace the mapping row with the new key while keeping the same pseudonym. The stable identifier never changes even though the key does; erasure simply deletes the mapping row, which breaks the linkage for downstream systems without touching them. (This is the mechanism described in US Patent 12158973, "Techniques related to stable pseudonymous identifiers" — useful as a published design reference, not as a license to copy blindly.)

Also worth knowing: **RFC 9415** (§7.3, "Category #3: Uniqueness, Stable within Context") is the IETF's own guidance on derived stable identifiers — `next_id = F(CONTEXT, secret_key)` where F is a PRF/HMAC (HMAC-SHA-256 is named as an acceptable option), the key must be secret and of reasonable length, and must remain stable for the context or the stability property is lost. This is the standard text to cite when someone proposes hashing identifiers with a public salt.

### 8.3 Rotation / rekey schedules for an external device correlator

There is a working draft that does almost exactly what a device-id design needs, in another domain: **`draft-seralathan-radext-persistent-devid-01`** defines a Chargeable-Device-Identity `CDI = "cdi:" + Base64(HMAC-SHA-256(server_secret, device_id || epoch))`, carried in RADIUS `Class`, with an epoch boundary configurable by operators (daily/weekly/monthly; **weekly recommended**). Properties to copy:

- Only the server(s) holding the secret can reverse; intermediate systems can do equality comparison **within an epoch** and nothing else.
- The identifier **rotates at epoch boundaries**, bounding linkability, while remaining stable _within_ an epoch so correlation still works.
- Explicit guidance against predictability: the device identifier must be CSPRNG-generated, never sequential.

The same shape works for a device correlator on the web: the durable internal key is a per-device server-side row (`devices.id`, ULID); the value you hand to analytics, logs, and third parties is `HMAC(pepper_epoch, devices.id)`, rotated weekly with a dual-key read window.

Two traps, both documented in the privacy-engineering literature:

- **A public epoch label is not a key.** `HMAC(u, epoch_number)` is invertible by anyone who can enumerate the user space — and device/account populations are enumerable. Per-epoch keys must be random and stored.
- **Rotation is only as good as the channels that survive it.** Derivation, emission ordering, a session/sequence counter spanning the boundary, a device fingerprint, or a schema difference each re-link the "rotated" identifiers. If you rotate the device id but keep emitting the same canvas digest, you rotated nothing.

### 8.4 Per-purpose key separation

- One key per purpose/class: `device`, `ip`, `email`, `behaviour`, `abuse-correlator`. Never one global key for everything, unless the business requirement is genuinely cross-class correlation and you accept the blast radius.
- Bind purpose + version into the MAC input: `HMAC(pepper, "device:v2:" || normalized_value)`.
- Keep _identification_ keys (HMAC) distinct from _authentication_ secrets (Argon2id for credentials, HKDF-derived AES-GCM keys for cookies). A pepper leaked from the fingerprint table must not help anyone authenticate.
- Tenant isolation is a design fork: per-tenant peppers give isolation but make cross-tenant correlation structurally impossible; a shared pepper gives abuse correlation but concentrates risk in one secret. Choose explicitly and write down why.

### 8.5 Keeping a stable identifier while honoring deletion requests

This is the part most designs get wrong.

- **Keyed digests are pseudonymization, not encryption.** If you delete the mapping table row (and any retained raw input), the subject's records can no longer be located by identifier — the linkage is gone even though digests remain in logs. This satisfies deletion _for identification purposes_; document that residual digests are not re-identifiable without the key.
- **If you rotate salts and then delete the salts, you can never find a subject's records again to delete them.** Decide before launch whether the design supports erasure: if it does, retained epoch keys (or the mapping table) are themselves a re-identification capability and must be stored under the same controls as raw PII, with a documented expiry, and deleted on the retention boundary rather than kept "just in case."
- **Decouple the stable internal key from the exposed value.** Keep `devices.id` (opaque, account-scoped) as the internal stable identifier; expose only the rotating HMAC. Deletion = delete the device row; the exposed values become unlinkable to the account once the epoch keys age out and the row is gone.
- **Version every digest.** Version stamps are what make rotation, deletion, and honest privacy claims auditable ("version 1 legacy digests are enumerable and are excluded from privacy claims"). Retrofitting a version column after the fact means the history can't be re-keyed — you can only age it out.
- Do not over-claim: the sharpest audit question is "for every stored digest, what is the keyspace of its input, and where does the key live?" If the answer is "small" and "there is no key," the control is marketing.

---

## 9. Recommended Architecture (SvelteKit + axum cookie session)

### 9.1 Layer map

| Layer                                   | Choice                                                                                                                                                                                                                                                                | Standard / rationale                                                                |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| **L1 Transport & origin**               | HTTPS-only, HSTS, `__Host-` cookie prefix, `Secure; HttpOnly; SameSite=Lax` (Strict on sensitive routes), Origin/CSRF check on state-changing POST/PUT, tight CORS                                                                                                    | NIST 63B-4 §5.1 cookie requirements                                                 |
| **L2 Session**                          | Server-side session row (`sid`, opaque 256-bit random, ≥64-bit minimum), rotated on login and privilege change; `auth_time`, `amr`, `aal`, `device_id`; idle 1 h / absolute 24 h (extend only if PoP lands)                                                           | NIST 63B-4 §5.1/§5.2 (AAL2 shapes; extension permitted for device-bound mechanisms) |
| **L3 Device record**                    | `devices(id ULID, user_id, first_seen, last_seen, label, fp_digest, fp_version, bind_state, risk_state, revoked_at)`; only keyed digests stored; exposed correlator = `HMAC(epoch_pepper, devices.id)` rotating weekly with dual-key read                             | RFC 9415 §7.3; CDI draft pattern                                                    |
| **L4 Risk scoring + step-up**           | axum middleware computes score → Allow / Step-up / Deny; step-up = fresh WebAuthn assertion with `userVerification: 'required'` (TOTP fallback); elevation stored with expiry; no silent downgrade                                                                    | NIST 63B-4 §2 (fraud indicators), §5.3 (session monitoring)                         |
| **L5 Passkeys**                         | WebAuthn, `residentKey: 'preferred'`, UV `'preferred'` for login / `'required'` for step-up; read BE/BS; treat BE=1 as _not_ device-bound; Signal API calls on every sign-in and on credential deletion                                                               | WebAuthn L3; NIST App. B                                                            |
| **L6 Proof-of-possession (session)**    | **DBSC** registration + refresh endpoints, opportunistic only, graceful fallback; no attestation, no key-extraction assumptions                                                                                                                                       | W3C DBSC FPWD; NIST §5.1 explicitly blesses PoP session secrets                     |
| **L7 OAuth plane (API/mobile, if any)** | DPoP for public/native clients; mTLS (RFC 8705) for service-to-service; PKCE everywhere, exact redirect URIs, no implicit/ROPC, PAR if we ever federate                                                                                                               | RFC 9449, RFC 8705, RFC 9700, OAuth 2.1 draft                                       |
| **L8 Bot/fraud defense**                | Turnstile at signup/login; accept `PrivateToken`/PACT as a positive signal when present; per-IP + per-account + per-ASN rate limits; credential-stuffing shape detection                                                                                              | Privacy Pass RFC 9576/77/78; Kasada ATO findings                                    |
| **L9 Monitoring & IR**                  | Audit every session create, step-up, device add/remove, credential change, email change; notify the _old_ email on account-key changes; hold + re-verify after "new device → credential/email change"; session list UI with revoke; global sign-out on password reset | ATO pivot data (§6.1)                                                               |

### 9.2 Which standard at which layer — and which to avoid

**Use:**

- Cookie session + server-side session record as the primary session primitive (no browser dependency, works for 100% of users).
- `__Host-` + `SameSite` + `HttpOnly` and a rotating opaque `sid`.
- Device record with keyed HMAC correlation, per-purpose keys, versioned digests, dual-key rotation.
- WebAuthn passkeys as the login and step-up anchor; UV inspected server-side.
- Risk-based step-up with post-auth signals (new device, ASN change, credential/email change holds).
- DBSC opportunistically where supported.
- Turnstile plus PrivateToken acceptance as _signals_.

**Use with explicit limits:**

- DPoP: only on the OAuth plane, only with a BFF; never push tokens into browser JS to gain a DPoP story.
- BE/BS flags: policy hints only; assume providers can misreport and that sync fabric sharing exists.
- Hybrid passkey transport: record it as a distinct `amr`; do not attribute the credential's device to the browser's device.
- PACT: ingest when it arrives, never require it.

**Avoid:**

- Token Binding (RFC 8471/2/3) — removed from Chrome 130; ecosystem-wide deployment model is the reason it died.
- WEI-style browser attestation requirements — abandoned; the surviving variant is app-scoped.
- Gating on App Attest / Play Integrity for a website — not available, and it excludes legitimate users.
- Raw fingerprint storage, client-side-only device ids (`localStorage`/cookie-only), unkeyed hashes of low-entropy values, public salts, and derived-from-epoch-label keys.
- Reusing one secret across identification and authentication.
- Inferring AAL/step-up decisions from fraud indicators (NIST: indicators do not change AAL and do not substitute for an authentication factor).
- Treating device binding as malware protection: on-device code execution (XSS, CDP, infostealer with equivalent privileges) defeats every binding in this document, including DBSC.

### 9.3 Notes specific to our codebase

- `db/` is untracked/immutable and the Quran databases are never hashed or versioned; the device-id pepper and mapping tables live in the normal application store (migrations under `rust/backend/api/migration/`), never under `db/`.
- The no-SHA-256-over-Quran-data rule is orthogonal to device-signal HMACs, but keep them visibly separate: identifiers use HMAC-SHA-256 with a purpose/version prefix and a server-held pepper; Quran corpus reads never hash.
- SvelteKit front end: the device correlator and risk inputs are computed/collected server-side (axum) on the session record, not shipped as fingerprinting JS. Client contributions, if any, go through the existing typed API surface with the same CSRF/origin protections as other mutations, and are treated as untrusted hints.
- Hotkeys/UI work: revocation and step-up surfaces (session list, revoke-all) should be reachable via the existing TanStack Hotkeys registry, not new listeners.

---

## 10. Mechanism Comparison Table

| Mechanism                                                   | Binds what                                                                        | Browser support 2026                                                                                        | Attacker cost to bypass                                                                                                                                  | Complexity                                         |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| **Cookie session (baseline)**                               | Nothing (bearer)                                                                  | Universal                                                                                                   | Steal cookie (infostealer, XSS, CDP) — low                                                                                                               | Trivial                                            |
| **Server-side device record + keyed-HMAC id + RBA/step-up** | Nothing cryptographic; correlates and gates                                       | Universal (server-side)                                                                                     | Needs to look like a known device _and_ network _and_ behave plausibly; costs solver/proxy infra + fresh UV step-up                                      | Medium (data model + scoring + UX)                 |
| **DBSC**                                                    | Session cookie ↔ hardware key (TPM/Secure Enclave) on Windows/macOS Chrome + Edge | Chrome Windows GA (146, Apr 2026), macOS from 147; Edge 147; **Firefox negative; Safari no implementation** | Cookie replay from a second device is dead; **on-device code execution/CDP is not** (SpecterOps Aug 2026); replay within the short-cookie window remains | Medium-low (2 endpoints + headers, fallback logic) |
| **WebAuthn passkey (BE=1)**                                 | Login ↔ credential in a sync fabric                                               | Universal                                                                                                   | Phishing-resistant; costs AitM proxy + realistic UI; not device-bound (may exist elsewhere)                                                              | Medium (RP library, account recovery)              |
| **WebAuthn passkey (BE=0, hardware)**                       | Login ↔ hardware-bound key                                                        | Universal for external keys; platform variants narrower                                                     | Must possess the physical device; very high                                                                                                              | Medium-high (provisioning, recovery)               |
| **WebAuthn UV flag**                                        | Assertion ↔ local user verification                                               | Universal                                                                                                   | Needs the device's local unlock factor; proxy cannot fake it                                                                                             | Low (one parameter + server check)                 |
| **Signal API**                                              | Credential lifecycle ↔ provider store                                             | Chrome/Edge 132+, Safari 26; **Firefox no**                                                                 | N/A (hygiene)                                                                                                                                            | Low (2–3 calls)                                    |
| **Hybrid/cross-device**                                     | Phone credential ↔ desktop session via BLE proximity                              | Universal (platform-dependent)                                                                              | Remote QR relay blocked by proximity; social-engineering of the phone user not blocked                                                                   | Low client-side, but attribution caveats           |
| **DPoP**                                                    | OAuth access/refresh token ↔ client key                                           | **No native browser support**; JS/WebCrypto only → XSS becomes a signing oracle                             | Token alone is useless; attacker needs client key _or_ XSS in the client origin                                                                          | Medium (JOSE, jti store, nonce plumbing)           |
| **mTLS-bound token**                                        | Token ↔ client certificate                                                        | N/A for browsers; excellent for backends                                                                    | Needs the client cert/private key (usually TPM/keystore-backed)                                                                                          | High (PKI lifecycle, TLS edge)                     |
| **Token Binding**                                           | Token ↔ TLS connection                                                            | **Dead** (Chrome removed v130, 2024)                                                                        | N/A — do not deploy                                                                                                                                      | N/A                                                |
| **Private Access Token (PAT)**                              | Request ↔ Apple-attested device (Safari/iOS/macOS only)                           | Safari on iOS 16+/macOS 13+ via Cloudflare; **not Chrome/Android**                                          | Residential proxies + unattested devices are simply not covered                                                                                          | Low if Cloudflare fronts you                       |
| **PACT / PrivateToken**                                     | Request ↔ issuer-signed (blinded) token                                           | Announced Jun 2026 (Chrome/Edge/Firefox/Shopify; Apple absent); Edge Canary Aug 2026                        | Untouched by design on the unattested surface; governance unresolved                                                                                     | Low to accept, no build                            |
| **Turnstile / challenge**                                   | Session ↔ human-ish client proof                                                  | Universal                                                                                                   | Solver services, CAPTCHA farms, residential proxies (65% of ATO already)                                                                                 | Low                                                |
| **App Attest / DeviceCheck**                                | App request ↔ Secure Enclave-attested app install                                 | iOS apps only (14+); rate-limited by Apple                                                                  | Needs a genuine install on a real device + bypassing app integrity                                                                                       | Medium (native app required)                       |
| **Play Integrity**                                          | App request ↔ genuine app + certified Android device                              | Android apps/WebViews only; SafetyNet dead Jan 2025; hardware attestation being phased in                   | Needs genuine app + certified device; custom ROMs excluded                                                                                               | Medium (native app required)                       |
| **WEI**                                                     | Browser environment ↔ third-party attester                                        | **Abandoned Nov 2023**; successor is app/WebView-scoped                                                     | N/A                                                                                                                                                      | N/A                                                |

**Cross-cutting:** every device-binding mechanism in the table is defeated by code execution on the bound device (XSS in the origin, infostealer with browser-equivalent privileges, CDP injection into the live browser). Device binding raises the cost of _credential portability_, not of endpoint compromise. Plan for both.

---

## 11. Ranking by Real-World Deployability (SvelteKit + axum, 2026)

1. **Server-side session record + device record + keyed-HMAC device correlator + RBA with step-up.** Zero browser dependency, works for 100% of users, directly aligned with NIST §5.1/§5.3, and it is the layer that catches the actual ATO pattern (new device + credential/email change). Build this first.
2. **WebAuthn passkeys with UV policy and BE/BS handling.** Universally supported, phishing-resistant, and the only way to get a _real_ device anchor (BE=0). Add the Signal API calls opportunistically (Chrome/Edge/Safari; Firefox ignores them harmlessly).
3. **Turnstile + rate limiting + PrivateToken acceptance at ingress.** Immediate, global, low cost. Treat as friction and one positive signal, never as the gate.
4. **DBSC registration + refresh endpoints, opportunistic.** Two endpoints, header handling, plain-cookie fallback. Covers Chrome/Edge desktop Windows (+macOS rolling out); add an enterprise-style "require DBSC" policy flag for cohorts that can meet it. Do not promise DBSC-grade security to Safari/Firefox users.
5. **DPoP / mTLS — only if and when tokens cross a boundary.** If we add a mobile client or third-party API: DPoP for public clients, mTLS (RFC 8705) for service-to-service, RFC 9700 rules throughout. For the current cookie-session web app: no browser-facing benefit, and risk if it tempts us into moving tokens into JS.
6. **PACT — watch and accept.** No engineering until tokens actually arrive in production traffic; ingest as a positive signal when they do.

**Do not deploy:** Token Binding (removed from Chrome, never adopted), WEI (abandoned), any browser-attestation requirement (breaks global coverage and does not stop the bots that live on the unattested surface), raw fingerprint storage, or unkeyed hashes of low-entropy identifiers.

---

## 12. Sources

**Standards (primary)**

1. RFC 9449 — OAuth 2.0 Demonstrating Proof of Possession (DPoP). https://www.rfc-editor.org/rfc/rfc9449 ; https://datatracker.ietf.org/doc/html/rfc9449 (incl. inline errata on `DPoP-Nonce` CORS exposure)
2. RFC 8471 / 8472 / 8473 — Token Binding Protocol v1.0 / TLS extension / over HTTP. https://www.rfc-editor.org/info/rfc8471 ; https://www.rfc-editor.org/info/rfc8473
3. RFC 8705 — OAuth 2.0 Mutual-TLS Client Authentication and Certificate-Bound Access Tokens. https://datatracker.ietf.org/doc/html/rfc8705
4. RFC 9700 — Best Current Practice for OAuth 2.0 Security. https://www.rfc-editor.org/info/rfc9700 ; summary: https://oauth.net/2/oauth-best-practice/
5. OAuth 2.1 (draft-ietf-oauth-v2-1-15, no RFC as of mid-2026). https://datatracker.ietf.org/doc/draft-ietf-oauth-v2-1/
6. RFC 9576 — The Privacy Pass Architecture; RFC 9577 — HTTP Authentication Scheme; RFC 9578 — Issuance Protocols. https://www.rfc-editor.org/info/rfc9576
7. RFC 9415 — On the Generation of Transient Numeric Identifiers (§7.3 keyed stable identifiers). https://datatracker.ietf.org/doc/rfc9415/
8. W3C — Device Bound Session Credentials, First Public Working Draft 21 Aug 2025. https://www.w3.org/TR/2025/WD-dbsc-1-20250821/
9. W3C — WebAuthn Level 3. https://www.w3.org/TR/webauthn-3/
10. NIST SP 800-63B-4 — Digital Identity Guidelines: Authentication and Lifecycle Management (July 2025). PDF: https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-63B-4.pdf ; §2 AAL: https://pages.nist.gov/800-63-4/sp800-63b/aal/ ; §5 Session (bindings, timeouts, monitoring): https://pages.nist.gov/800-63-4/sp800-63b/session/ ; Appendix B Syncable Authenticators: https://pages.nist.gov/800-63-4/sp800-63b/syncable/
11. NIST SP 800-63B Supplement 1 — Incorporating Syncable Authenticators (2024). https://csrc.nist.gov/pubs/sp/800/63/b/sup/final
12. draft-seralathan-radext-persistent-devid-01 — Chargeable-Device-Identity rotating correlator (`HMAC-SHA-256(secret, device_id || epoch)`). https://datatracker.ietf.org/doc/html/draft-seralathan-radext-persistent-devid-01
13. draft-schlesinger-cfrg-act-01 — Anonymous Credit Tokens (credential-with-state, basis for PACT). Referenced in Mozilla Hacks and Prosopo (below).

**DBSC** 14. Chrome for Developers — Origin trial: Device Bound Session Credentials in Chrome (Chrome 135). https://developer.chrome.com/blog/dbsc-origin-trial 15. Google Security Blog — Protecting Cookies with Device Bound Session Credentials (GA, Chrome 146, Apr 2026; federated/cross-origin future work). https://blog.google/security/protecting-cookies-with-device-bound-session-credentials/ 16. Google Workspace Updates — DBSC GA in Chrome for Windows, on by default, no admin disable (28 May 2026). https://workspaceupdates.googleblog.com/2026/05/prevent-account-takeovers-with-DBSC-now-generally-available-in-the-Chrome-browser-for-Windows.html 17. Google Workspace Admin Help — Prevent cookie theft with session binding. https://support.google.com/a/answer/15956470 18. Chrome Platform Status — DBSC (Firefox: negative; Safari: no signal). https://chromestatus.com/feature/5140168270413824 ; https://chromestatuslite.com/feature/5140168270413824 19. Corbado — DBSC explained, browser support matrix checked 3 Oct 2026. https://www.corbado.com/blog/device-bound-session-credentials-dbsc 20. Mozilla standards-positions #912 — proposed position: negative (replay window; bespoke reauth protocol; no attestation expectation). https://github.com/mozilla/standards-positions/issues/912 21. FIDO Alliance White Paper — DBSC/DPoP as Complementary Technologies to FIDO Authentication (May 2025). https://fidoalliance.org/white-paper-dbsc-dpop-as-complementary-technologies-to-fido-authentication/ 22. SpecterOps — "Return of the Cookie Monster" (CDP injection defeats cookie/DbSC protections, Aug 2026). https://specterops.io/blog/2026/08/13/chrome-devtools-protocol-cookie-theft/ ; CSA research note: https://labs.cloudsecurityalliance.org/research/csa-research-note-chrome-devtools-session-hijacking-20260817/ 23. KnowBe4 — "I Love Device-Bound Session Credentials, But They Are Still Phishable/Hackable." https://blog.knowbe4.com/device-bound-session-credentials-phishable-hackable 24. The Hacker News — Google Rolls Out DBSC in Chrome 146 (Apr 2026). https://thehackernews.com/2026/04/google-rolls-out-dbsc-in-chrome-146-to.html

**DPoP / OAuth operations** 25. WorkOS — DPoP (RFC 9449) explained. https://workos.com/blog/dpop-rfc-9449-explained 26. InfoQ — The DPoP Storage Paradox: Why Browser-Based Proof-of-Possession Remains an Unsolved Problem (non-extractable CryptoKey as an XSS signing oracle; BFF as the standard mitigation). https://www.infoq.com/articles/dpop-key-storage-unsolved-problem/ 27. Keycloak — Official Support for DPoP in Keycloak 26.4 (Oct 2025). https://www.keycloak.org/2025/10/dpop-support-26-4 ; configuration guide: https://www.keycloak.org/securing-apps/dpop 28. Auth0 — Implementing DPoP with Auth0 (SDK 2.4.0+, `useDpop`). https://auth0.com/blog/implementing-dpop-with-auth0/ ; Enterprise Connections DPoP (Apr 2026, algorithm support + silent-disable caveat): https://auth0.com/docs/authenticate/enterprise-connections/enable-dpop-enterprise-connections 29. Okta — Configure DPoP for resource servers (nonce lifetime: 24 h renewal, 3-day overlap). https://developer.okta.com/docs/guides/dpop/oktaresourceserver/main/ 30. Spring Security — OAuth 2.0 DPoP-bound Access Tokens. https://docs.spring.io/spring-security/reference/servlet/oauth2/resource-server/dpop-tokens.html 31. WorkOS — Sender-constrained tokens: why mTLS and DPoP exist, and what killed Token Binding. https://workos.com/blog/mtls-dpop-token-binding-sender-constrained-oauth 32. `dpop-verifier` crate (Rust DPoP proof verifier). https://lib.rs/crates/dpop-verifier ; https://github.com/Ukonhattu/dpop-verifier

**WebAuthn / passkeys** 33. web.dev — userVerification deep dive (UP/UV, preferred/required/discouraged, server checks). https://web.dev/articles/webauthn-user-verification 34. web.dev — WebAuthn feature detection with `getClientCapabilities()`. https://web.dev/articles/webauthn-client-capabilities 35. Chrome for Developers — Keep passkeys consistent with your server with the Signal API (methods, browser support badges, hidden-vs-deleted semantics). https://developer.chrome.com/docs/identity/webauthn-signal-api 36. MDN — `PublicKeyCredential.signalUnknownCredential()` / `signalAllAcceptedCredentials()`. https://developer.mozilla.org/en-US/docs/Web/API/PublicKeyCredential/signalUnknownCredential_static 37. Cobrado — Device-bound vs synced passkeys (BE/BS semantics). https://www.corbado.com/blog/device-bound-synced-passkeys ; WebAuthn transports (internal/hybrid): https://www.corbado.com/blog/webauthn-transports-internal-hybrid 38. MojoAuth — Telling a synced passkey from a device-bound one (BE/BS flags). https://mojoauth.com/blog/webauthn-be-bs-flags-synced-vs-device-bound-passkey 39. Yubico — High-assurance hybrid flows (proximity, caBLE). https://developers.yubico.com/WebAuthn/Concepts/Hybrid_Flows/High_assurance_hybrid_flows/ 40. FIDO Passkey Central — Cross-device sign-in design guidance. https://www.passkeycentral.org/design-guidelines/optional-patterns/cross-device-sign-in

**Attestation** 41. Cloudflare — Private Access Tokens: eliminating CAPTCHAs on iPhones and Macs with open standards (attester/issuer roles; Apple-only client). https://blog.cloudflare.com/eliminating-captchas-on-iphones-and-macs-using-new-standard/ 42. Cloudflare — Privacy Pass: upgrading to the latest protocol version (roles, `WWW-Authenticate: PrivateToken`, Turnstile attester, Silk extension, replay API). https://blog.cloudflare.com/privacy-pass-standard/ 43. Cloudflare press release — PACT with Chrome, Edge, Firefox, Shopify (22 June 2026). https://www.cloudflare.com/press/press-releases/2026/cloudflare-collaborates-with-leading-browsers-to-develop-a-privacy-first-protocol-for-the-global-internet/ 44. Prosopo — Private Access Control Tokens (PACT): the Privacy Pass successor (what PACT does and does not cover; 58% automated traffic; Apple gap; issuer governance). https://prosopo.io/blog/private-access-control-tokens-pact/ 45. Cloudflare blog — Turnstile Spin (≈3B verifications/day; adoption scale). https://blog.cloudflare.com/turnstile-spin/ 46. Apple — Establishing your app's integrity (App Attest / DeviceCheck). https://developer.apple.com/documentation/devicecheck/establishing-your-app-s-integrity ; rate-limit guidance: https://developer.apple.com/forums/thread/759285 ; WWDC26 App Attest session (fraud metric): https://developer.apple.com/videos/play/wwdc2026/201/ 47. Approov — Limitations of Apple DeviceCheck and App Attest. https://approov.io/blog/limitations-of-apple-devicecheck-and-apple-app-attest 48. Android Developers — Play Integrity API / overview. https://developer.android.com/google/play/integrity ; https://developer.android.com/google/play/integrity/overview ; SafetyNet shutdown notice (31 Jan 2025): https://groups.google.com/g/safetynet-api-clients/c/qcMXTCpHReg 49. Wikipedia — Web Environment Integrity (abandoned 2 Nov 2023; WebView Media Integrity successor; Mozilla/Vivaldi/FSF/Brave/EFF/W3C positions). https://en.wikipedia.org/wiki/Web_Environment_Integrity 50. Brave — "Web Environment Integrity": Locking Down the Web. https://brave.com/web-standards-at-brave/9-web-environment-integrity/ ; FSF: https://www.fsf.org/blogs/community/web-environment-integrity-is-an-all-out-attack-on-the-free-internet ; W3C: https://www.w3.org/blog/2023/web-environment-integrity-has-no-standing-at-w3c/

**ATO / risk** 51. Kasada — 2025 Account Takeover Attack Trends (report landing). https://www.kasada.io/resources/account-takeover-attack-trends ; four data-driven takeaways (+250% ATO in 2024; 22 groups; 1,000+ orgs; 65% sophisticated automation; 85% had bot detection; 32× Black Friday spike; 72% malicious traffic; 194-day detection). https://www.kasada.io/blog/4-takeaways-2025-account-takeover-trends 52. Security Boulevard — MSP guide to ATOs (6.2M accounts compromised across 1,027 organizations, 2024, citing Kasada). https://securityboulevard.com/2025/11/from-compromise-to-control-an-msp-guide-to-account-takeovers/ 53. Cisco Duo — Risk-Based Authentication (signal set, device registration risk, impossible travel). https://duo.com/docs/risk-based-auth ; https://duo.com/learn/risk-based-authentication 54. Ping Identity — Impossible travel 101. https://www.pingidentity.com/en/resources/cybersecurity-fundamentals/detect-risk/impossible-travel-101.html 55. Fingerprint — Impossible travel detection (pairing with device intelligence; false-positive handling). https://fingerprint.com/blog/impossible-travel-detection/ 56. Deepstrike — Account takeover statistics 2026 (Verizon DBIR 2025: credential abuse as initial access in 22%). https://deepstrike.io/blog/account-takeover-statistics 57. Gupta Deepak (CIAM Compass) — Adaptive risk-based authentication (signals → score → allow/step-up/block; ~70/25/5 tuning target). https://guptadeepak.com/ciam-compass/guides/adaptive-risk-based-authentication/

**Signal design** 58. PasskeyBridge — Keyed identifier hashing: HMAC, pepper, rotation (dual-key rotation window; version stamps; domain separation; pepper custody). https://passkeybridge.io/blog/plain-sha256-not-privacy-control-keyed-identifier-hashing 59. Spatial Privacy — Rotating pseudonyms without leaking the join key (per-epoch random salt vs derived salt; five re-linkage channels; salt retention vs erasure tension). https://www.spatial-privacy.org/trajectory-mobility-data-privacy/mix-zones-and-path-confusion/rotating-pseudonyms-without-leaking-the-join-key/ 60. US Patent 12158973 — Techniques related to stable pseudonymous identifiers (mapping-table indirection that keeps a pseudonym stable across key rotation; erasure by deleting the mapping row). https://patents.google.com/patent/US12158973
