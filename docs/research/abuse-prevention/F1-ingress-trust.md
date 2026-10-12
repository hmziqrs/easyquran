# F1 — Ingress trust chain: deep audit and fix design

Auditor: fresh subagent, independent of E5 (`E5-fresh-eyes.md`) and C2 (`C2-contradictions.md`).
Input: working tree at `/Users/hmziq/os/easyquran` (HEAD `53d3d421`, Mon Oct 12 2026), read at
source: `rust/backend/api/src/middlewares/client_ip.rs`, `src/config/settings.rs`, `src/main.rs`,
`src/middlewares/rate_limit.rs`, `src/services/escalation.rs`, `src/services/turnstile.rs`,
`src/modules/admin_bans_v1/controller.rs`, `crates/rux-request-gate/src/ip.rs`,
`docker-compose.yml`, `docker-compose.local.yml`, `deploy/README.md`,
`web/server.ts`, `web/src/lib/server/quran-translation-page.ts`, `web/scripts/assert-headers.sh`,
`.env.example`, `docs/security.md`, `docs/quran-system.md`.
Dependency sources read: `client-ip-0.2.1`, `axum-client-ip-1.3.1` (both in the cargo registry).
External facts verified against primary sources: Traefik v3 docs + `forwarded_header.go`
(XHeadersSet, ipAllowList remote-addr semantics, `headers.customRequestHeaders`), Cloudflare docs
(HTTP headers page, Authenticated Origin Pulls page).

Cross-references: E5 §7 and #1 name this as the single most important weakness; C2 item 9 proves
nothing strips `X-Eq-Edge-*`. This report re-derives the trust chain from source, adds the
Docker-network lateral path (absent from both), the Traefik header-managing behaviour (verified
from Traefik source), the Cloudflare multi-tenant/Worker caveats (verified from CF docs), and a
concrete fix + negative test. Every finding below is marked **NEW**.

---

## 0. Verdict (one paragraph)

The app's entire abuse-control substrate — every per-IP rate limit, every ban, every escalation
decision, the Turnstile `remoteip`, and the future device token's IP evidence — is keyed on
`CF-Connecting-IP`, which is believed from **any TCP peer inside `TRUSTED_PROXY_CIDRS`**, whose
default is every RFC1918 range plus `fd00::/8`, i.e. **every container on the shared Dokploy
Docker network**. Nothing in any layer sets or strips the header: Traefik does not manage it
(it is not in Traefik's `XHeadersSet`), the api never rewrites it, the web tier never touches it.
The one control that would make the header unforgeable — restricting origin ingress to Cloudflare
ranges — exists only as prose in `deploy/README.md:377-409`, is owner-run, and its own negative
test is explicitly "the owner's sign-off; this script is the scaffolding" (`:407-409`). Two
consequences sharper than prior reports: (1) the firewall step, even if done perfectly, does **not**
close the Docker-network path, because the api's peer is always a private container IP and the
default CIDR set covers it; (2) an allowlist of CF ranges alone does **not** prove the request came
from _our_ zone (cross-zone relay; Worker subrequests can control `CF-Connecting-IP`; global
Authenticated Origin Pulls uses a cert shared by every Cloudflare account). The fix is a
two-layer proof — an authenticated transport (Tunnel or per-zone AOP) plus an application-level
edge marker that only our Cloudflare zone can add — enforced at runtime, gated at boot, and
proven by an automated negative test.

---

## 1. The trust chain as actually built

Resolution order in `resolve_client_ip` (`client_ip.rs:76-149`), which is the **innermost** layer
of the private-router stack (`main.rs:827-858`) and of the public Quran stack (`main.rs:897-927`):

```
request
  │  health route? /healthz, */health/ready → no identity, no limiter (client_ip.rs:24-26,78-80)
  ├─ X-EasyQuran-Internal-Token present?
  │     constant-time match vs InternalApiToken extension (client_ip.rs:82-106)
  │     match  → RequestIdentity::InternalService(WebSsr); NO peer check; NO CF header needed; return
  │     no match → warn, fall through
  ├─ peer gate (client_ip.rs:113-121): if IP_SOURCE is a header source AND peer ∉ TRUSTED_PROXY_CIDRS
  │     → 400 untrusted_proxy_peer
  ├─ ClientIp extractor reads CF-Connecting-IP (axum_client_ip CfConnectingIp)
  │     ok    → RequestIdentity::External(header_value)  ← believed verbatim (client_ip.rs:123-131)
  │     err   → prod: 400 missing_client_identity (client_ip.rs:133-140)
  │             non-prod: "unknown" bucket, no identity (client_ip.rs:141-147)
  └─ rate limiters key off the extension (rux-request-gate/src/ip.rs:70-80; rate_limit.rs:22-27)
```

Facts that matter, each verified:

| Fact                                                                             | Evidence                                                                                                                                                                                         |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Header believed whenever the peer is in the list                                 | `client_ip.rs:64-74,113-131`; pinned by test `cf_header_from_peer_inside_trusted_cidrs_is_believed` (`:500-522`)                                                                                 |
| Empty list = presence-only trust (worse)                                         | `client_ip.rs:70-74` (`!trusted.0.is_empty() &&`); test `:525-544`                                                                                                                               |
| Duplicate `CF-Connecting-IP` headers fail closed                                 | `client-ip-0.2.1/src/lib.rs:40` → `ip_from_single_header` → `of_single_header` (`:147-165`): two occurrences → `SingleHeaderRequired` → prod 400                                                 |
| Internal token checked **before** the peer gate, with no peer check              | `client_ip.rs:92-106`                                                                                                                                                                            |
| Bucket keys derive from the resolved identity                                    | `ip.rs:44-56`; `rate_limit.rs:57-63,68-79`; test keys `ratelimit:203.0.113.5/32:quran-v1`, `ratelimit:internal-webssr:quran-v1` (`rate_limit.rs:297-299`)                                        |
| Bans derive from the same identity                                               | `escalation.rs:125-154` (`active_ban`), `:199-284` (`on_first_block` → `set_block(quran-ban:{unit})`); `BanUnit::from_ip` → `/32` or `/64` (`admin_bans_v1/controller.rs:298-307`)               |
| Only external IPs can escalate                                                   | `ip.rs:52-56` (`is_external_ip`), `escalation.rs:72-78`                                                                                                                                          |
| Escalation is default OFF today                                                  | `settings.rs:349` (`QURAN_BAN_ESCALATION_ENABLED` default `false`)                                                                                                                               |
| No strip/overwrite middleware exists in the api                                  | `main.rs:827-858` layer list contains no header removal; `client_ip.rs` only reads                                                                                                               |
| Traefik middleware on the api router = `strip-api` only                          | `docker-compose.yml:189-192`; web router = compress only (`:108-110`)                                                                                                                            |
| Traefik does not manage `CF-Connecting-IP`                                       | Traefik `XHeadersSet` (forwarded_header.go) contains only `X-Forwarded-*`, `X-Scheme`, `X-Real-Ip`; `CF-Connecting-IP` is not a managed header, so it is proxied verbatim                        |
| Traefik _does_ sanitize `X-Forwarded-For`/`X-Real-Ip` when the peer is untrusted | forwarded_header.go `ServeHTTP`: `if !insecure && !isTrustedIP(remote) { DeleteXForwardedHeaders }`, then `X-Real-Ip = RemoteAddr` if absent; `forwardedHeaders.trustedIPs` defaults empty       |
| The app never reads XFF                                                          | `IP_SOURCE: CfConnectingIp` (`docker-compose.yml:136`); grep of `rust/backend/api/src` for `x-forwarded-for`/`XRealIp`/`TrueClientIp` outside settings = only a test header (`client_ip.rs:202`) |
| The web tier never reads CF headers for limits                                   | `web/server.ts:640-657`; `ADDRESS_HEADER` unset in compose → `server.requestIP()` = TCP peer (Traefik)                                                                                           |

---

## 2. Attack paths, precisely

Preconditions and gains per path. "Forged" always means an attacker-chosen `CF-Connecting-IP`.

### (a) Know the origin IP and reach it directly (bypassing Cloudflare)

**Precondition.** Origin IP discoverable (TLS-cert scan for the `easyquran.fyi` SAN, DNS history,
Shodan) **and** the host's 80/443 (Dokploy's Traefik) reachable from the internet — i.e. the
manual firewall/allowlist step in `deploy/README.md:377-392` is missing or incomplete. The repo
cannot show either side (see §4); the deploy doc admits the negative test is owner-run and
unverified (`:407-409`), and `docs/security.md:84` says the same ("none of this is verified yet").
**Gain.** Send `Host: easyquran.fyi` + `CF-Connecting-IP: <anything>` to Traefik. Traefik matches
the api router, forwards the header verbatim; the api's peer is Traefik (private, trusted) → the
header is believed → `RequestIdentity::External(<anything>)`. Full identity forgery: unlimited
buckets (rotate the forged IP per request), targeted 429s against any third-party IP (pre-fill
their fixed-window buckets), attacker-directed bans when W3a is on, ban evasion, complete WAF/bot
bypass, and unthrottled enumeration (`forgot_password` oracle), session-row inflation
(`/csrf/v1/generate`), mail bombing (register), and password-work saturation.

### (b) Reach it from a peer inside `TRUSTED_PROXY_CIDRS` (Docker-network lateral)

**Precondition.** Control of, or code execution in, **any container on `dokploy-network`** — the
external network this stack joins (`docker-compose.yml:195-198`) and which every Dokploy stack
shares — or host access / Docker socket. Target is trivial: Docker DNS resolves `api` and `web`
(`:168-170`, `:98-100`). Default CIDRs are `10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fd00::/8`
(`:143`), so the api's peer is always inside the list. **This path does not depend on the public
firewall at all** — the CF-range rule from `deploy/README.md` cannot close it, because the api's
peer is a private container either way. **Gain.** Identical to (a), from inside the perimeter, with
no internet exposure and no knowledge of the origin IP. Also reachable: `http://web:8080` (no IP
logic, no limiter).

### (c) Forge `CF-Connecting-IP` through Traefik (public client, origin only reachable via Traefik)

**Precondition.** Origin IP + Traefik 443/80 open to non-CF peers. Same capability as (a); the
distinction matters because (a) needs a firewall gap and (c) needs only that the host serves
anything on 443. **Traefik neither sets nor strips `CF-Connecting-IP`** (verified from Traefik
source: not in `XHeadersSet`; the only middleware in the repo is `stripprefix`, `:189-192`).
**Gain.** Same as (a). If the CF-range allowlist _were_ enforced at Traefik (or the firewall),
this path is closed for non-CF peers — but nothing in the repo enforces it, and the doc's own
procedure is manual.

### (d) Forge the internal SSR token

**Precondition.** Know/steal the 32-byte `INTERNAL_QURAN_API_TOKEN` — present in the api and web
container env (`docker-compose.yml:66`, `:119`) and in the host `.env`; exposed to any code
execution in the web container (it is read as `INTERNAL_QURAN_API_TOKEN` in
`quran-translation-page.ts:47` and only sent to the internal base, `:48-63`, so it is not leaked
outward by design). The token check runs **before** the peer gate and with **no peer check**
(`client_ip.rs:92-106`). **Gain.** `RequestIdentity::InternalService(WebSsr)`: skips the external
content limiter entirely (only the shared 600/min internal bucket, `rate_limit.rs:68-79`), needs
no CF header at all (bypasses the prod identity gate), and can never be IP-banned or escalated
(`ip.rs:52-56`). It also shares the web SSR bucket, so the holder can starve translated-reader SSR
for every user (E5 A2). It does not grant authenticated-user access. A leaked token therefore
defeats the entire ingress gate for the API surface.

### (e) SSRF or a compromised container

**Precondition.** Any SSR/SSRF in the web tier (or a future URL-fetching feature) or RCE in any
container on the shared network. **Gain.** Reach `http://api:8888` with a forged header (→ (b));
read the web container's env (→ (d)); possibly reach other Dokploy containers. The web tier
already holds the internal token, so a compromised web container is a full ingress bypass by
construction — the fix must not pretend otherwise (see §9).

### Attack-path table

| #   | Path                                    | Precondition                                     | Identity gained           | Immediate gains                                                                                           | Needs W3a on?                     |
| --- | --------------------------------------- | ------------------------------------------------ | ------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------- |
| a   | Direct to origin (Traefik on public IP) | origin IP + firewall gap                         | any external IP           | unlimited buckets, targeted 429s, ban evasion, WAF bypass, unthrottled enumeration/mail/session inflation | no for 429s; yes for durable bans |
| b   | Docker-network peer                     | any container on `dokploy-network` / host access | any external IP           | same as (a), no internet exposure, **firewall cannot help**                                               | same                              |
| c   | Forge header through Traefik            | origin IP + 443 open                             | any external IP           | same as (a)                                                                                               | same                              |
| d   | Internal token                          | token leak / web-container RCE                   | `InternalService(WebSsr)` | bypass external limiter + identity gate; starve web SSR; immune to bans                                   | n/a                               |
| e   | SSRF / RCE                              | web SSR SSRF or container RCE                    | (b)+(d)                   | lateral to other Dokploy containers                                                                       | n/a                               |

Additional gain common to a/b/c: **targeted 429 against any third-party IP on any route** by
pre-filling `ratelimit:{victim}:{route}` buckets (fixed-window counters are keyed on the resolved
identity, `rate_limit.rs:22-27,297-299`), and — once E4/E5 turn W3a on — **attacker-directed
durable bans** (`quran-ban:{victim}/32`, up to `QURAN_ACTIVE_BAN_MAX` default 2000,
`settings.rs:497`) which the export feed (`admin_bans_v1/controller.rs:162-181`) hands to an
out-of-repo edge adapter (`deploy/README.md:429-436`) that may enforce them at the CDN.

---

## 3. Defaults, verified

**Compose default** (`docker-compose.yml:143`):

```yaml
TRUSTED_PROXY_CIDRS: ${TRUSTED_PROXY_CIDRS:-10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fd00::/8}
```

**Settings default:** there is none — unset parses to an empty list
(`settings.rs:216-217` → `parse_trusted_proxy_cidrs` on `unwrap_or_default()`), and production
rejects empty (`:218-220`). So a production deployment with no explicit value gets the compose
default above; `.env.example` does not mention the variable at all, and `IP_SOURCE` is likewise
absent from `.env.example` (it is hardcoded in compose, `:136`).

**Is the default safe?** No, and the reason is structural: it is _broader than the proxy it is
meant to describe_. The compose comment (`:137-143`) justifies it as "the Docker networks the
Traefik proxy container can come from", but `10/8 ∪ 172.16/12 ∪ 192.168/16 ∪ fd00::/8` is every
private address in existence, including every other Dokploy app container on `dokploy-network`.
It is sufficient to boot and it does block public peers (they are not private), but it grants
"trusted proxy" status to anything on the host's Docker networks. The header is then believed
verbatim, so the trust set and the forge set are the same set.

**Production boot gates that exist today** (all in `Settings::from_env` → `main.rs:134-139`
prints and exits 1):

| Gate                                                                                          | Evidence                                      |
| --------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `APP_ENV` must be a known value; compose pins `production`                                    | `settings.rs:30-43`; `docker-compose.yml:132` |
| `IP_SOURCE` must parse; `ConnectInfo` rejected in prod                                        | `settings.rs:195-215`                         |
| `TRUSTED_PROXY_CIDRS` must be non-empty in prod                                               | `settings.rs:216-220`                         |
| `COOKIE_SECURE` must be true in prod                                                          | `settings.rs:221-224`                         |
| `INTERNAL_QURAN_API_TOKEN` set, not a shipped placeholder, ≥32 chars; internal/health rpm > 0 | `settings.rs:456,464-495`                     |
| `QURAN_BAN_ESCALATION_ENABLED=true` requires `QURAN_BAN_ALLOWLIST`                            | `settings.rs:349-363`                         |
| `WEB_AUTH_ENABLED=true` in prod requires real mail/provider creds                             | `settings.rs:548-...`                         |

**What is _not_ gated:** anything about the ingress path. There is no variable, no assertion, and
no test that requires the CF-range allowlist to exist, that the origin is not publicly reachable,
or that requests reaching the api actually traversed Cloudflare. `parse_trusted_proxy_cidrs`
(`settings.rs:237-250`) also accepts _any_ valid CIDR, including `0.0.0.0/0` and `::/0`, with no
range-sanity check.

---

## 4. Does the origin accept non-Cloudflare traffic? What the repo shows vs cannot show

**Shows (in-repo, verifiable):**

- The api service has **no `ports:` and no `expose:`** in `docker-compose.yml` (service block
  `:113-193`; network membership `:168-170`). It is reachable only on the external
  `dokploy-network` (`:195-198`), by Docker DNS name `api`, port 8888.
- The web service likewise has no published ports (`:40-111`).
- The api binds `0.0.0.0:8888` inside the container (`settings.rs:192`, `main.rs:934-944`) — fine,
  the container boundary is the control.
- `docker-compose.local.yml` publishes **loopback-only** for dev: web `127.0.0.1:8080:8080`
  (`:29-30`), api `127.0.0.1:8888:8888` (`:49-50`), with `APP_ENV: development` /
  `IP_SOURCE: ConnectInfo` (`:36-37`) — no prod impact.
- The only public listener in production is the **external** Dokploy Traefik (not in the repo).
  Its entrypoints are on 80/443; the routers here are `websecure` only (`:105-107`, `:185-187`).

**Cannot show (outside the repo):** whether the host firewall or Traefik itself restricts 80/443
to Cloudflare ranges; whether the origin IP is exposed by any direct DNS record; whether ACME
renewal depends on a publicly reachable port (relevant to §6). `deploy/README.md:377-392` states
the rule is required and owner-run; `:407-409` states the negative test is unverified;
`docs/security.md:84` confirms.

**Conclusion.** Internet attackers cannot reach the api unless they can reach Traefik on 443 with
a matching Host — that is the (a)/(c) path, and it hinges entirely on out-of-repo firewall state.
But **any peer on the Docker network can reach the api directly** — that is the (b) path, and it
is fully determined by in-repo config (shared external network + all-private default CIDRs).
So the documented manual step is necessary but not sufficient: fixing only the firewall leaves
(b) and (d) open.

---

## 5. The header-stripping problem, answered exactly

1. **Through Traefik (allowlist in place):** a forged `CF-Connecting-IP` from a _non-CF_ peer is
   impossible once the peer is restricted to CF ranges, because Traefik forwards the header
   verbatim and the only peers that can reach it are Cloudflare's. Cloudflare itself sets
   `CF-Connecting-IP` on edge→origin traffic ("This header will only be sent on the traffic from
   Cloudflare's edge to your origin web server", CF HTTP-headers docs); a client-supplied value
   does not survive a proxied request. **State it plainly: with a correct allowlist, this path is
   closed — but only this path.**
2. **Multi-tenant Cloudflare:** an allowlist of CF ranges proves "some Cloudflare edge", not "our
   zone". Another CF customer can proxy their own zone to our origin (SSL mode Full, crafted/
   preserved `Host`) and reach the api from a CF IP; their `CF-Connecting-IP` is their real client
   IP (not chosen), so they cannot spoof, but they gain a CF-edge path that bypasses _our_ zone's
   WAF/rate/cache rules. Worse, Cloudflare documents that **Worker subrequests to a destination
   that is not a Cloudflare-customer zone let the Worker control `CF-Connecting-IP` and
   `x-real-ip`** (CF HTTP-headers docs, "CF-Connecting-IP in Worker subrequests"). A free-plan
   Worker therefore has an arbitrary-IP primitive against our origin _if_ it can get a working
   transport to it (plain HTTP if routed, or a cert-valid path). And **global Authenticated Origin
   Pulls does not close this**: "this certificate is shared across all Cloudflare accounts and
   guarantees that the request is coming from the Cloudflare network" (CF AOP docs). The same docs
   state the correct control: "To authenticate that traffic originates from your own zone, use
   zone-level or per-hostname Authenticated Origin Pulls with your own certificate, or Cloudflare
   Tunnel."
3. **`X-Forwarded-For`:** Traefik manages it. With the default `forwardedHeaders.trustedIPs` empty,
   Traefik deletes client-supplied `X-Forwarded-*`/`X-Real-Ip` on requests from untrusted peers and
   then sets `X-Real-Ip` to the _peer_ address (the CF edge IP), appending the peer to XFF at proxy
   time (Go ReverseProxy behaviour). So downstream XFF is **not** a client-controllable value in
   this topology, but it is also **not** the client IP — it is the CF edge IP. The app ignores XFF
   entirely (`IP_SOURCE=CfConnectingIp`), and the web tier ignores it too (`ADDRESS_HEADER` unset →
   `server.requestIP()` = Traefik peer, `web/server.ts:640-657`). Residual: any future consumer
   that trusts XFF (e.g. setting `ADDRESS_HEADER=x-forwarded-for`) inherits a value that is only
   as good as Traefik's sanitisation and never carries the real client in this chain. Ban that
   pattern (§7).
4. **`True-Client-IP`:** Enterprise-only at CF, and CF only adds it when the Managed Transform is
   enabled; the docs explicitly warn that in a stacked-CDN setup its value "can be spoofed to any
   value" if you do not add it. Nothing in the repo reads it; the api's `IP_SOURCE` parser would
   accept it (`settings.rs:202`) — a footgun (§6.2).
5. **Duplicates:** two `CF-Connecting-IP` header lines fail closed at the extractor
   (`client-ip-0.2.1` `of_single_header`, `:147-165`), so header-duplication smuggling cannot pick
   the second value. Worth pinning in the negative test.

---

## 6. The fix

Two layers, both required, each independently verifiable. Names: `INGRESS_TRUST_MODE`,
`INGRESS_EDGE_MARKER`, `INGRESS_ALLOWED_PEER_CIDRS`.

### 6.0 The core insight

In the CF→Traefik→api topology the api's peer is **always** a private container IP; you cannot
reconstruct the client IP from the peer, and Traefik cannot substitute the peer into
`CF-Connecting-IP` (its `customRequestHeaders` sets literal values only). The genuine client IP
exists only in CF's header. Therefore the fix must make **the CF→origin channel** provable:

- **Transport proof (network):** Cloudflare Tunnel (no public origin ingress at all) or per-zone/
  per-hostname Authenticated Origin Pulls with an uploaded certificate (not the global shared one).
- **Application proof (edge marker):** a per-deployment secret header that **only our Cloudflare
  zone adds**, via a Cloudflare Transform Rule (`Modify Request Header → Set`, overwrite), so that
  any request reaching the api from anywhere else — a direct attacker, a rogue container, another
  CF customer's relay, or a Worker subrequest from another account — is missing it and is rejected.
  The marker is what closes the cross-zone/Worker hole that neither an allowlist nor global AOP
  closes.

The marker alone would be "static secret at CF + static secret at origin"; combined with the
transport proof it is defense in depth: a marker leak still cannot reach the origin past AOP/Tunnel,
and an AOP/Tunnel gap still cannot pass the marker gate.

### 6.1 (a) Tunnel vs AOP — which, and exact surfaces

**Recommended: Cloudflare Tunnel.** It removes the origin's public ingress for this app entirely
(the "attacker knows the origin IP" precondition stops existing), authenticates the path by
construction (the tunnel credential is tied to our account), and avoids the ACME/clientAuth
interaction risk of AOP (below). Exact surface:

- `docker-compose.yml`: new service `cloudflared`
  (`image: cloudflare/cloudflared:latest`, `command: tunnel --no-autoupdate run --token ${TUNNEL_TOKEN}`,
  `restart: unless-stopped`, `networks: [proxy]`, no published ports), token from `.env`
  (secret, Dokploy environment tab).
- Cloudflare dashboard: named tunnel + public hostnames `easyquran.fyi → https://traefik:443`
  with `originServerName: easyquran.fyi` (keep Traefik in the path so `strip-api` and the existing
  routers are unchanged), plus the Transform Rule that sets `X-Eq-Ingress-Marker`.
- Traefik labels: `ipallowlist` on both routers that **denies public peers** (sourceRange = the
  Docker private ranges) — with the tunnel, CF edge IPs never connect to Traefik, so the list must
  be the private peer set, not the CF ranges; the marker carries the trust. This also means the
  broad default is acceptable _as a deny-public filter_ once the marker is mandatory — but it must
  be paired with the marker requirement, never used alone.
- DNS: ensure no record points directly at the origin IP; keep the proxied A/AAAA.

**Alternative: per-zone Authenticated Origin Pulls** (if the public origin must stay):

- Cloudflare: upload **your own** certificate (zone-level or per-hostname AOP) and enable it —
  explicitly _not_ global AOP, whose cert is shared across all accounts (CF docs).
- Traefik (Dokploy-managed, outside this repo): a dynamic-config `TLSOptions` object with
  `clientAuth: { clientAuthType: RequireAndVerifyClientCert, caFiles: [/etc/traefik/origin-pull-ca.pem] }`,
  mounted into the `dokploy-traefik` container, and referenced from the routers'
  `traefik.http.routers.{api,web}.tls.options` label. Optionally add Traefik's
  `passTLSClientCert` middleware to forward the client-cert subject for logging.
- Then the Traefik `ipAllowList` sourceRange = current CF IPv4 **and IPv6** ranges
  (`https://www.cloudflare.com/ips-v4` + `ips-v6`), attached before `strip-api`.
- Rollout check: verify AOP does not break ACME renewal (Traefik's default TLS-ALPN-01 challenge
  runs on the same entrypoint; if `clientAuth` is applied at the entrypoint/default TLS store
  rather than per-router, renewal handshakes without a client cert will fail — test renewal, or
  move to DNS-01 / pre-provisioned certs before enabling).

### 6.2 (b) Boot-time production assertion (follows the existing gate style)

Add `IngressSettings::from_env()` to `Settings::from_env` (`settings.rs:884-914`), returning
`Err` in production so `main.rs:134-139` prints and exits 1. Rules:

1. `INGRESS_TRUST_MODE` ∈ {`cloudflare_tunnel`, `authenticated_origin_pull`} — required in
   production; anything else (including unset) is a boot error. In non-production, default `none`.
2. `INGRESS_EDGE_MARKER` — required in production; ≥32 chars; not in a new
   `KNOWN_INGRESS_MARKER_PLACEHOLDERS` (mirror `KNOWN_INTERNAL_TOKEN_PLACEHOLDERS`,
   `settings.rs:456`); must differ from `INTERNAL_QURAN_API_TOKEN` and `COOKIE_KEY`; error text
   tells the operator to generate `openssl rand -hex 32`.
3. `INGRESS_ALLOWED_PEER_CIDRS` — required non-empty in production; this is the same list the
   Traefik `ipallowlist` label uses, so the declaration and the enforcement surface are one value.
4. `IP_SOURCE` must be exactly `CfConnectingIp` in production (extend the existing rejection at
   `settings.rs:208-215`, which today only rejects `ConnectInfo`, to reject `XRealIp`,
   `TrueClientIp`, `RightmostXForwardedFor`, etc. — in this topology those resolve to the edge
   peer or to nothing).
5. `TRUSTED_PROXY_CIDRS` must not contain `0.0.0.0/0` or `::/0` (extend
   `parse_trusted_proxy_cidrs`, `settings.rs:237-250`), and the boot log should print the parsed
   list plus the mode (extend the existing "Ingress contract configured" log,
   `main.rs:145-150`).

Honest limitation, stated in the error text and the README: the gate proves _configuration
presence_, not that Traefik/CF are actually configured. The runtime marker check (§6.3) is the
part that is enforced on every request; the scheduled canary (§6.5) is the part that proves the
invariant in production.

### 6.3 (c) Runtime: marker gate + header overwrite/strip (new layer in the api)

In `resolve_client_ip` (`client_ip.rs:76-149`), in order:

1. Health routes: unchanged (`:24-26,78-80`).
2. Internal token: unchanged (`:92-106`) — it is a stronger proof than the marker; but add a
   counter/log for internal-token use from a peer that is not the expected web container
   (monitoring, §6.5), and rotate the token on a schedule.
3. **New, before the peer gate:** if the marker is configured (non-empty extension, i.e. prod),
   require `x-eq-ingress-marker` present and constant-time equal to it; missing/mismatch →
   `400 missing_edge_marker` (same shape as `reject_missing_external_identity`,
   `client_ip.rs:28-38`), log `warn` + increment `ingress_marker_missing_total`. Mandatory control
   → reject, matching the house style (mandatory identity gates reject; optional attestation
   degrades — C2 item 4).
4. Peer gate and CF-header resolution: unchanged (`:113-140`).
5. **New, after successful external resolution:** overwrite `cf-connecting-ip` with the verified
   IP's canonical string (so the handlers' `ClientIp` extractor, e.g.
   `auth_v1/controller.rs:125,461`, still works and can never see a forged value), and remove
   `x-forwarded-for`, `x-real-ip`, `true-client-ip`, `forwarded`, and `x-eq-ingress-marker` from
   the request before `next.run`. This is the strip/overwrite defense-in-depth: after this point
   no downstream code can read a client-supplied IP header or the marker.
6. Also add a tiny outermost check (or fold into 3): if a client-supplied `x-eq-ingress-marker`
   is present and differs from the configured value, count it (`ingress_marker_spoof_total`) —
   active probing signal.

Wiring: layer a new `EdgeIngressMarker(Arc<str>)` extension next to `TrustedProxyCidrs` and
`InternalApiToken` in both stacks (`main.rs:832-835` private, `:917-920` public), and include the
marker value in `AppState`/settings for the middleware. No change to `axum_client_ip` or the
`IP_SOURCE` enum.

### 6.4 (d) Is a signed edge header scheme needed? No — say it plainly

If Tunnel or per-zone AOP is in place, the CF→origin channel is authenticated and
`CF-Connecting-IP` cannot be chosen by anyone outside our account, so an HMAC/Ed25519 signed edge
header adds nothing for IP trust. The earlier reports already cut the `X-Eq-Edge` scheme
(C2 item 3) because it has no consumer; the only fields it could add (JA4, bot score) require a
Worker and Enterprise/Bot-Management data, and a Worker would itself become the client-IP source.
Recommendation: **do not build edge signing.** Keep the static marker (which is not a signature,
just an unforgeable-by-outsiders path proof) and revisit signing only when a concrete consumer for
edge-derived fields exists. If a Worker is ever added, it must `set` (never append) and the
stripping rule from C2 item 9 must land first.

### 6.5 (e) Monitoring that proves the invariant in production

1. **Counters** (OpenTelemetry surface already exists, `utils/telemetry.rs:339-375`; `http_metrics`
   shows the pattern): `ingress_marker_missing_total`, `ingress_marker_mismatch_total`,
   `ingress_marker_spoof_total`, `untrusted_proxy_peer_total`, `missing_client_identity_total`
   (the latter two already `warn`-logged at `client_ip.rs:115-119,134-138`; promote to counters).
   Steady state in production: all zero. Alert: any nonzero `ingress_marker_missing_total` over a
   5-minute window (it means something reached the api without traversing our zone — a rogue
   container, a stale probe, or a config drift after a redeploy), and any nonzero spoof counter.
2. **Scheduled external canary** — the test `deploy/README.md:394-409` admits is missing: a cron
   (GitHub Action / external monitor) from a non-Cloudflare network that hits
   `https://<origin-ip>/api/quran/sources/...` with `Host: easyquran.fyi` and a forged
   `CF-Connecting-IP`, and asserts the response is 400/403 (not 200). Page on 2xx. This is the only
   signal that proves the _transport_ half (firewall/AOP/Tunnel) rather than the marker half.
3. **Boot log**: extend `main.rs:145-150` to print `ingress_mode`, `edge_marker_set`,
   `allowed_peer_cidrs`, `trusted_proxy_cidrs`, `ip_source`; alert if the values change between
   deploys (config-drift detection).
4. **Ban-side signal**: count `quran-ban:*` keys created and include the triggering identity's
   peer/marker status in the log line (`escalation.rs:271,280` already logs unit + scope) so an
   operator can tell a real abuser from a marker-less forgery attempt.

### 6.6 Files to change

| File                                            | Change                                                                                                                                                      |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust/backend/api/src/config/settings.rs`       | new `IngressSettings` + gates (§6.2); reject default routes in `parse_trusted_proxy_cidrs`; extend the prod `IP_SOURCE` rejection; placeholder list         |
| `rust/backend/api/src/middlewares/client_ip.rs` | marker gate + header overwrite/strip + counters (§6.3)                                                                                                      |
| `rust/backend/api/src/main.rs`                  | layer `EdgeIngressMarker` in both stacks; extend the boot log                                                                                               |
| `docker-compose.yml`                            | `INGRESS_TRUST_MODE`, `INGRESS_EDGE_MARKER`, `INGRESS_ALLOWED_PEER_CIDRS`; Traefik `ipallowlist` middleware on both routers; optional `cloudflared` service |
| `web/scripts/assert-headers.sh`                 | make the CF-range reminder (`:222-223`) a hard check under `EXPECT_INGRESS_LOCKED=1`; add a marker-less direct request that must fail                       |
| `deploy/README.md`                              | rewrite `:377-409` into the two supported topologies, the CF Transform Rule steps, the marker rotation procedure, and exactly which test proves which half  |
| `rust/backend/api/tests/ingress_trust.rs` (new) | the negative test (§7)                                                                                                                                      |

---

## 7. The automated negative test

Lives at `rust/backend/api/tests/ingress_trust.rs` — a separate binary because `APP_ENV` is
process-wide, exactly the rationale documented at `tests/quran_prod_gate.rs:1-8`. It builds the
real middleware stack with `tower::ServiceExt::oneshot` and the repo's extension pattern
(`client_ip.rs` tests `:465-476` show `TrustedProxyCidrs` + `ConnectInfo` injection).

What it sets up: a router with `resolve_client_ip`, `ClientIpSource::CfConnectingIp`,
`TrustedProxyCidrs(["10.0.0.0/8"])`, `InternalApiToken`, the new `EdgeIngressMarker`, a
`ConnectInfo` peer of `10.1.2.3` (a "Traefik" peer inside the trusted set), and the quran content
limiter with an `InMemoryStore` (max 1 so one request is observable), optionally with an enabled
`EscalationEngine`.

What it asserts:

1. Forged `CF-Connecting-IP` + trusted peer + **no marker** (prod) → `400`, and the store contains
   **no** key mentioning the forged IP — the forged identity never reached the limiter.
2. Same with a **wrong marker** → `400`, no bucket.
3. Positive control: same forged IP + **correct marker** → `200`, and exactly one bucket keyed by
   the header value (`ratelimit:203.0.113.77/32:quran-v1`) — proving the identity still flows when
   the path proof is present.
4. Escalation: flood the forged identity 25× with the escalation engine enabled and the suspicious
   4xx threshold configured; assert `store.ban_status("quran-ban:198.51.100.9/32")` is `None` —
   a forged IP cannot direct a ban.
5. Peer outside `TRUSTED_PROXY_CIDRS` + header → `400 untrusted_proxy_peer` (pins existing
   behaviour, `client_ip.rs:478-498`).
6. Two `cf-connecting-ip` header lines → `400` (pins the `client-ip` single-header behaviour).
7. Header hygiene: a handler that echoes the request `HeaderMap` sees `cf-connecting-ip` equal to
   the verified IP and no `x-forwarded-for` / `x-real-ip` / `true-client-ip` / marker header.

Boot-gate tests go in the existing `settings.rs` test module (the `TEST_ENV_MUTEX` pattern,
`:1240-1330`): production without `INGRESS_TRUST_MODE` → `Err`; with `cloudflare_tunnel` and a
placeholder marker → `Err`; with a valid marker and `0.0.0.0/0` in `TRUSTED_PROXY_CIDRS` → `Err`;
with valid config → `Ok`.

Sketch:

```rust
// tests/ingress_trust.rs — production-pinned negative test for the ingress trust
// chain. Separate binary: APP_ENV is process-wide (same reason as quran_prod_gate.rs).
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use axum::{body::Body, http::{Request, StatusCode}, middleware, routing::get, Extension, Router};
use tower::ServiceExt;
use rux_request_gate::{InMemoryStore, RateLimitStore, RateLimitLayer, PathKey, Escalation};
use ruxlog::middlewares::client_ip::{resolve_client_ip, EdgeIngressMarker, InternalApiToken, TrustedProxyCidrs};
use ruxlog::services::escalation::EscalationEngine;
use ruxlog::config::settings::EscalationConfig;

static PROD: OnceLock<()> = OnceLock::new();
const MARKER: &str = "test-ingress-marker-0123456789abcdef";

fn app(store: Arc<InMemoryStore>, escalation: Option<Arc<dyn Escalation>>) -> Router {
    PROD.get_or_init(|| std::env::set_var("APP_ENV", "production"));
    let content = RateLimitLayer::builder(store, 1, 60)
        .path_key(PathKey::Fixed("quran-v1"))
        .applies(|req| !matches!(
            req.extensions().get::<rux_request_gate::RequestIdentity>(),
            Some(rux_request_gate::RequestIdentity::InternalService(_))))
        .with_escalation(escalation)
        .build();
    Router::new()
        .route("/quran/v1/sources", get(|| async { "ok" }))
        .layer(content)
        .layer(middleware::from_fn(resolve_client_ip))
        .layer(Extension(axum_client_ip::ClientIpSource::CfConnectingIp))
        .layer(Extension(TrustedProxyCidrs(Arc::new(vec!["10.0.0.0/8".parse().unwrap()]))))
        .layer(Extension(InternalApiToken(Arc::from(""))))
        .layer(Extension(EdgeIngressMarker(Arc::from(MARKER))))
}

fn forged_req(ip: &str, marker: Option<&str>) -> Request<Body> {
    let mut req = Request::builder()
        .uri("/quran/v1/sources")
        .header("cf-connecting-ip", ip)
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(axum::extract::ConnectInfo(
        "10.1.2.3:40000".parse::<SocketAddr>().unwrap()));
    if let Some(m) = marker {
        req.headers_mut().insert("x-eq-ingress-marker", m.parse().unwrap());
    }
    req
}

#[tokio::test]
async fn forged_cf_ip_without_marker_is_rejected_and_creates_no_bucket() {
    let store = Arc::new(InMemoryStore::default());
    let app = app(store.clone(), None);
    let denied = app.clone().oneshot(forged_req("203.0.113.77", None)).await.unwrap();
    assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
    assert!(store.snapshot().iter().all(|s| !s.key.contains("203.0.113.77")));

    let allowed = app.clone().oneshot(forged_req("203.0.113.77", Some(MARKER))).await.unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    assert!(store.snapshot().iter().any(|s| s.key == "ratelimit:203.0.113.77/32:quran-v1"));
}

#[tokio::test]
async fn forged_ip_cannot_direct_a_ban() {
    let store = Arc::new(InMemoryStore::default());
    let cfg = EscalationConfig { enabled: true, ..EscalationConfig::disabled(2_000) };
    let engine: Arc<dyn Escalation> =
        Arc::new(EscalationEngine::new(cfg, store.clone()));
    let app = app(store.clone(), Some(engine));
    for _ in 0..25 {
        let _ = app.clone().oneshot(forged_req("198.51.100.9", None)).await.unwrap();
    }
    assert!(store.ban_status("quran-ban:198.51.100.9/32").await.unwrap().is_none());
}
```

(`EscalationConfig::disabled` is `#[allow(dead_code)]` today, `settings.rs:330-346`; the sketch can
construct the struct literally instead if the helper stays private.)

---

## 8. The web tier: what it can trust if a limiter is added

Today `web/server.ts` has **no rate limiter at all**, and its client address is not the client:
`ADDRESS_HEADER` is unset in compose, so `getClientAddress` (`web/server.ts:640-657`) returns
`server.requestIP()` — the TCP peer, i.e. Traefik. If E6 adds a web-origin limiter:

- The only trustworthy client IP is `CF-Connecting-IP`, and only after the same ingress
  verification the api has (Tunnel/AOP transport + the marker requirement). A web limiter keyed on
  the header without the marker gate is forgeable by exactly the (a)/(b)/(c) paths above.
- It must **never** be keyed on `X-Forwarded-For`. Setting `ADDRESS_HEADER=x-forwarded-for` would
  hand the key to whatever Traefik's sanitisation leaves; in this chain XFF is the CF edge peer,
  not the client, and CF appends client-supplied XFF entries before Traefik sanitises them.
- **Dependency, stated as an ordering constraint:** the web limiter (E6) must not ship before
  F1 lands, and it must share the marker check (either the web tier re-implements the marker gate
  or it calls the api's `/csrf`-style verification — simplest is the same middleware logic in
  `web/server.ts`, requiring `x-eq-ingress-marker` for limiter keying). Also note the web
  container holds `INTERNAL_QURAN_API_TOKEN`, so a compromised web tier can always bypass the
  api's external limits; that is a separate blast-radius item, not something the ingress fix
  solves.

---

## 9. Attacking my own fix — residual risk after Tunnel + allowlist + marker + boot gate

| Residual                                   | Reality                                                                                                                                                                         | Mitigation                                                                                                                                                    |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Compromised Cloudflare account             | Attacker can edit the Transform Rule (learn/change the marker), disable AOP, repoint the tunnel, add hostnames. The marker is only as strong as the CF account.                 | Hardware-key 2FA on the CF account, scoped API tokens, audit log review, alert on Transform Rule / AOP / tunnel changes; marker rotation procedure documented |
| Leaked `TUNNEL_TOKEN`                      | A second connector can join the tunnel and reach origin services with arbitrary headers (the connector is attacker-controlled).                                                 | Marker requirement: the leaked token does not reveal the marker; rotate the token; alert on connector count changes                                           |
| Leaked marker                              | Attacker can forge identity from anywhere that can reach the api (Docker network or, in the AOP topology, a CF-range peer).                                                     | Transport proof (Tunnel/AOP) is the backstop; rotate the marker on suspicion; spoof counter detects probing                                                   |
| Rogue container on the same Docker network | Can reach `api:8888` and DoS it (connections, ports), but cannot forge identity (no marker).                                                                                    | Dedicated ingress network, drop `dokploy-network` membership where possible, resource limits; DoS remains an availability risk                                |
| Host-level access / Docker socket          | Read all env (marker, internal token, `COOKIE_KEY`, DB), exec into containers.                                                                                                  | Out of scope; standard host hardening                                                                                                                         |
| Compromised web container                  | Holds the internal token → full internal identity; also the future marker if the web limiter needs it.                                                                          | Token rotation, least-privilege env, treat web RCE as full compromise                                                                                         |
| IPv6                                       | Default `fd00::/8` is broad; the CF allowlist must include CF's IPv6 ranges (compose default does not list them); bans collapse IPv6 to `/64` so a hostile /64 rotates cheaply. | Include `ips-v6` in `INGRESS_ALLOWED_PEER_CIDRS`; consider /48 grouping at the ban layer (separate decision)                                                  |
| ACME vs AOP                                | `RequireAndVerifyClientCert` on the wrong TLS scope can break TLS-ALPN-01 renewal.                                                                                              | Test renewal after enabling; prefer Tunnel (no clientAuth), or DNS-01                                                                                         |
| Cloudflare itself                          | Compromised edge sees and can alter everything.                                                                                                                                 | Accepted (alternative is self-hosted TLS + own WAF); the marker limits cross-zone abuse but not a fully compromised edge                                      |

---

## 10. Ordering vs E4 / login limiter / E6 / device token

**Correct order:**

1. **F1 (this fix) — first.** Every IP-keyed control that follows consumes `RequestIdentity`;
   shipping them on a forgeable identity converts weaknesses into weapons (E4's ban write path on
   a forgeable IP lets an attacker ban third parties; E6's limiter is a forgeable 429 cannon).
2. **CI gates restored (E5 #2) — same window, parallel.** It is what keeps F1 from being undone by
   the next commit; it has no dependency on F1 and F1 should not ship without it.
3. **E4 (ban contract / ban write path + web enforcement)** — after F1, because a ban write path
   must never exist on a forgeable unit. Also decide, before shipping E4, whether the edge export
   adapter is real (`deploy/README.md:429-436`) and how it authenticates the feed.
4. **Login limiter redesign (E5 #4)** — technically independent of IP trust (the throttle is keyed
   on the email, `auth_v1/controller.rs:133-140`), so it may land in parallel. But its compensating
   per-IP brake is nominal until F1, and it is itself the most effective abuse tool today; do not
   let it be the reason F1 slips.
5. **E6 (reader-path/web limiter)** — strictly after F1 **and** after the web tier has the marker
   gate (§8). A web limiter on `CF-Connecting-IP` without the marker check is forgeable.
6. **Device token — last.** Its entire evidentiary value is the IP it is correlated with (E5 §5,
   C2 §7). Building it before F1 records attacker-chosen IPs as evidence; building it after F1 is
   the first point at which the correlation means anything.

---

## 11. Findings (all **NEW**)

### F1-1 — The ingress identity is forgeable by design; default trust set covers every Docker peer

**Severity: Critical.**
Evidence: `docker-compose.yml:143` (default `10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fd00::/8`);
`client_ip.rs:64-74,113-131` (peer gate + verbatim belief); `docker-compose.yml:168-170,195-198`
(api on the shared external `dokploy-network`); `rate_limit.rs:57-63,68-79` and `escalation.rs:125-154,199-284`
(every decision keyed on the resolved identity); pinned by tests `client_ip.rs:500-522`.
Fix: §6 — transport proof (Tunnel or per-zone AOP) + mandatory edge marker + boot gate + runtime
header overwrite; narrow `TRUSTED_PROXY_CIDRS` to the actual ingress network.

### F1-2 — Nothing sets or strips `CF-Connecting-IP` in any layer

**Severity: Critical.**
Evidence: `main.rs:827-858` (no header-removal middleware); `docker-compose.yml:189-192`
(only `stripprefix`); Traefik `XHeadersSet` excludes `CF-Connecting-IP` (forwarded_header.go), so
it is proxied verbatim; `web/server.ts` never mutates it.
Fix: edge marker + api-side overwrite/removal of `cf-connecting-ip`/`x-forwarded-for`/`x-real-ip`/
`true-client-ip`/`forwarded` after identity resolution (§6.3).

### F1-3 — The required CF-range origin restriction is documentation only, with an owner-run test

**Severity: High.**
Evidence: `deploy/README.md:377-392` ("MUST deny public origin ingress", owner-run procedure),
`:394-409` ("The live CF-range negative test … is the owner's sign-off; this script is the
scaffolding"); `web/scripts/assert-headers.sh:9-15,222-223` (warn only); repo contains no Traefik
allowlist, no `wrangler.toml`, no `cloudflared` config, no CF zone config (grep for
`ipAllowList|wrangler|cloudflared|origin.pull|mTLS` → only comments/docs; `.wrangler` appears only
in ignore files); `docs/security.md:84` confirms unverified.
Fix: ship the allowlist as code — Traefik `ipallowlist` labels in `docker-compose.yml` (§6.1) or
Tunnel — and turn the assert script's reminder into a hard check; add the canary (§6.5).

### F1-4 — The firewall step alone cannot close the Docker-network path

**Severity: High.**
Evidence: the api's peer through Traefik is the Traefik container (private, in the default set);
any container on `dokploy-network` can hit `http://api:8888` directly with a forged header
(`docker-compose.yml:168-170`); `client_ip.rs:113-131` believes it. This is independent of any
host firewall.
Fix: same as F1-1; additionally give api/web a dedicated ingress network rather than the shared
Dokploy network where possible.

### F1-5 — No production boot assertion ties the deployment to a verified ingress

**Severity: High.**
Evidence: `settings.rs:191-233` (the only prod HTTP gates: `IP_SOURCE≠ConnectInfo`,
`TRUSTED_PROXY_CIDRS` non-empty, `COOKIE_SECURE`); nothing requires an allowlist/marker/topology;
`main.rs:134-139` is the established fail-closed exit path to extend.
Fix: `IngressSettings::from_env` per §6.2.

### F1-6 — `TRUSTED_PROXY_CIDRS` accepts any CIDR, including default routes

**Severity: High.**
Evidence: `settings.rs:216-220,237-250` — parse-only; `0.0.0.0/0` or `::/0` would pass and make
every internet peer a trusted proxy.
Fix: reject default routes (and, ideally, any prefix broader than the ingress network) in
`parse_trusted_proxy_cidrs`; add a test.

### F1-7 — The internal SSR token grants identity with no peer check and no CF header

**Severity: High.**
Evidence: `client_ip.rs:82-106` (token branch precedes the peer gate; no peer check); the token
lives in both containers' env (`docker-compose.yml:66`) and the host `.env`; blast radius:
`rate_limit.rs:68-79` (internal bucket, non-escalating, no ban identity), and it starves the
shared web SSR bucket. Existing protections: placeholder rejection and ≥32 chars
(`settings.rs:456,464-495`), constant-time compare (`client_ip.rs:94-96`).
Fix: keep the token but treat a leak as full ingress compromise — add use-from-unexpected-peer
monitoring, schedule rotation, and document it in the ingress contract; the marker must not be
required on the internal path (the token is the stronger proof).

### F1-8 — An allowlist of Cloudflare ranges does not prove _our_ zone (relay, Worker, global AOP)

**Severity: Medium-High.**
Evidence: CF HTTP-headers docs (Worker subrequests to non-CF-customer destinations let the Worker
control `CF-Connecting-IP`/`x-real-ip`); CF AOP docs ("global … shared across all accounts …
guarantees only that the request is coming from the Cloudflare network"; "use zone-level or
per-hostname AOP with your own certificate, or Cloudflare Tunnel"); `client_ip.rs:113-131` trusts
whatever the header says.
Fix: per-zone/per-hostname AOP or Tunnel **plus** the CF-zone Transform Rule marker (§6.0-6.1).

### F1-9 — `IP_SOURCE` still allows header sources that are wrong for this topology

**Severity: Medium.**
Evidence: `settings.rs:195-205` accepts `XRealIp`, `TrueClientIp`, `RightmostXForwardedFor`,
`RightmostForwarded`, `CloudFrontViewerAddress`; prod rejects only `ConnectInfo` (`:208-215`).
In this chain Traefik sets `X-Real-Ip` from the peer (CF edge IP) and sanitises XFF; `True-Client-IP`
is Enterprise-only and spoofable in stacked-CDN setups (CF docs). `docs/quran-system.md:275-276`
documents the intended source.
Fix: in production, require exactly `CfConnectingIp`; reject the rest with a clear error.

### F1-10 — No monitoring signal for the invariant; the negative test is owner-run

**Severity: Medium.**
Evidence: `client_ip.rs:115-119,134-138` (warn logs only, no counters); `deploy/README.md:394-409`
(owner-run); `web/scripts/assert-headers.sh:222-223` (warn).
Fix: counters + alerts + scheduled non-CF canary + boot-log drift detection (§6.5).

### F1-11 — Every per-IP and ban decision consumes the forgeable value

**Severity: Medium.**
Evidence: `rate_limit.rs:22-27,57-63` (fixed-window buckets per resolved IP; forged requests
pre-fill a victim's bucket → targeted 429s even with W3a off); `escalation.rs:199-284` +
`settings.rs:328-378` (Temp after 5 / Long after 20 qualifying windows, Long = 7 days);
`admin_bans_v1/controller.rs:41-70,162-181` (ban rows and export feed);
`turnstile.rs:58-73` + `auth_v1/controller.rs:461-474` (captcha `remoteip` = the same value).
Fix: F1-1/F1-2; no separate code change needed for Turnstile (it becomes trustworthy once the
identity is), but note it in the ordering.

### F1-12 — Web tier has no IP logic; a future limiter has no trustworthy source yet

**Severity: Low-Medium.**
Evidence: `web/server.ts:594-598,640-657` (`ADDRESS_HEADER` unset → TCP peer); no limiter anywhere
in `server.ts`; compose web env has no `ADDRESS_HEADER`/`XFF_DEPTH` (`docker-compose.yml:52-78`).
Fix: §8 — E6 must share the marker-gated identity and must not use XFF.

---

## Appendix — what this audit could not verify from the repo

1. Whether the live origin accepts non-CF ingress (host firewall / Dokploy Traefik config lives
   outside the repo; `deploy/README.md:377-409` says manual; `docs/security.md:84` says unverified).
2. Whether the origin IP is exposed by any direct DNS record or historical record.
3. Whether Dokploy's Traefik supports the exact label/version surface assumed here (the compose
   uses `ipAllowList` v3 syntax; Dokploy's Traefik version is not pinned in the repo).
4. Whether CF Transform Rules / AOP / Tunnel are available on the deployed plan (all are available
   on Free except per-hostname AOP details; not verifiable here).
5. Whether ACME renewal uses TLS-ALPN (Traefik default) or HTTP-01/DNS-01 — affects the AOP
   rollout check only.
