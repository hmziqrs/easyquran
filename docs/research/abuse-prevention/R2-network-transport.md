# R2 — Network- and Transport-Layer Fingerprinting

What is observable at each hop, and what is realistically implementable behind **Cloudflare + Traefik + a Rust (axum/hyper) origin**.

Compiled 2026-10-12. All claims are cited inline; a source list is at the end.

---

## 0. Preamble: the honest verdict for this stack

Three facts dominate everything below.

1. **A fingerprint is only computable where the relevant bytes are visible.** The TLS ClientHello is only visible to the entity that terminates TLS. The TCP/IP handshake is only visible to the entity that terminates TCP. Behind Cloudflare, that entity is Cloudflare — not Traefik, not the origin. Traefik and axum see a _Cloudflare-to-origin_ connection; fingerprinting it identifies Cloudflare's egress pool, not the user.
2. **Cloudflare exposes fingerprints as data, not as raw protocol.** Cloudflare computes JA3/JA4 itself and surfaces them (a) to Workers as `request.cf.botManagement.ja3Hash` / `.ja4` and JA4 signal aggregates, (b) as Ruleset Engine fields, (c) as Logpush fields — but **only for Enterprise customers who bought Bot Management** ([Cloudflare docs: JA3/JA4 fingerprint](https://developers.cloudflare.com/bots/concepts/ja3-ja4-fingerprint/), [Bot Management variables](https://developers.cloudflare.com/bots/reference/bot-management-variables/)). Free/Pro/Business get coarse bot groupings only ([Bot scores](https://developers.cloudflare.com/bots/concepts/bot-score/)).
3. **Origin-side HTTP fingerprinting behind Cloudflare is mostly self-fingerprinting.** Cloudflare rewrites/overwrites headers (`accept-encoding` is forced to `br, gzip`; `connection: keep-alive`; invalid-name headers are dropped) and issues the origin request from its own edge ([Cloudflare HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)). Header order and casing at the origin therefore describe Cloudflare, not the eyeball. CloudFront only offers `CloudFront-Viewer-Header-Order` because it captures the viewer request _at the edge_ ([CloudFront request headers](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/adding-cloudfront-headers.html)).

**Feasible today, unmodified stack:** Cloudflare-provided JA4/JA3 (Ent. BM), bot score, JA4 signals, detection IDs/tags, verified-bot + corporate-proxy flags; all-plans `request.cf` transport/geo metadata (`tlsCipher`, `tlsVersion`, `tlsClientCiphersSha1`, `tlsClientExtensionsSha1(Le)`, `tlsClientHelloLength`, `tlsClientRandom`, `clientTcpRtt`, `clientQuicRtt`, `edgeL4.deliveryRate`, `httpProtocol`, `requestPriority`, `asn`, geo, mTLS); Logpush (all plans) and Instant Logs (Business+); IP intelligence from third-party data; Worker-signed edge headers to the origin.

**Not feasible behind Cloudflare, period:** eyeball TCP/IP fingerprinting (p0f/JA4T/TTL/MSS/timestamp skew), eyeball HTTP/2 SETTINGS/pseudo-header order, eyeball HTTP/3 transport parameters, raw ClientHello bytes/extension order, and origin-side JA4H-style header-order fingerprinting of the eyeball. Traefik cannot compute JA3/JA4 at all (plugin model has no TLS hook).

---

## 1. TLS fingerprinting

### 1.1 Where in a stack the ClientHello is even visible

The ClientHello is plaintext but only on the wire segment **before the TLS terminator**. Everything after that is inside a TLS record. Concretely:

- Terminator = Cloudflare: Cloudflare sees it; Traefik and origin see a _second_ ClientHello (Cloudflare's own) when Cloudflare opens the origin connection. Session reuse means origin connections often skip the handshake entirely.
- Terminator = Traefik (Cloudflare in DNS-only mode, or no CDN): Traefik's Go TLS stack parses the ClientHello, but **Traefik exposes nothing from it** — the plugin API is Yaegi-Go/WASM middleware operating on HTTP requests, with no TLS listener hook ([Traefik: Extend Traefik](https://doc.traefik.io/traefik/extend/extend-traefik/)).
- Terminator = origin (nginx/Envoy/HAProxy/Caddy/rustls): full visibility, see §4.
- Terminator = QUIC (HTTP/3): the ClientHello rides inside the QUIC Initial packet's CRYPTO frame, decryptable with the initial keys; same fingerprint methods apply with protocol marker `q` ([JA4 spec](https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md), [Cloudflare: JA4 signals](https://blog.cloudflare.com/ja4-signals/)).

### 1.2 JA3 / JA3S

JA3 = MD5 over the string `TLSVersion,Ciphers,Extensions,EllipticCurves,ECPointFormats`, decimal values, extension order preserved. JA3S is the server-side counterpart from ServerHello ([Salesforce JA3](https://github.com/salesforce/ja3); summary in [Fastly: state of TLS fingerprinting](https://www.fastly.com/blog/the-state-of-tls-fingerprinting-whats-working-what-isnt-and-whats-next)).

Known failure modes (all documented in the Fastly piece and the Cloudflare JA4 launch post):

- **Extension-order randomization.** Chromium (Feb 2023) randomizes ClientHello extension order, which shreds JA3's most distinctive field ([Chromium feature 5124606246518784](https://chromestatus.com/feature/5124606246518784); [Cloudflare JA4 signals](https://blog.cloudflare.com/ja4-signals/)).
- **Cipher stunting**: randomizing cipher order; JA4 sorts ciphers to blunt this ([FoxIO README Q&A](https://github.com/FoxIO-LLC/ja4)).
- **Implementation divergence.** Fastly showed `ja3er.com` produced a different hash from their implementation because of an unregistered BoringSSL extension, and a second mismatch caused by `x448` being encoded as `1035` instead of `30`; they recommend not trusting that database ([Fastly](https://www.fastly.com/blog/the-state-of-tls-fingerprinting-whats-working-what-isnt-and-whats-next)).

### 1.3 The JA4 family

Authoritative construction (verbatim from the [JA4 spec](https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md)):

```
(QUIC="q", DTLS="d", TLS over TCP="t")
(2 char TLS version: 13/12/11/10/s3/s2/d1/d2/d3/00)
(SNI="d" or no SNI="i")
(2 char count of ciphers)        <- GREASE excluded, capped at 99
(2 char count of extensions)     <- GREASE excluded, capped at 99
(first and last characters of first ALPN value, e.g. h2, 00 if absent)
_
sha256(hex ciphers sorted in hex order)[:12]
_
sha256(hex extensions sorted in hex order "_" sig_algorithms)[:12]
```

Example: `t13d1516h2_8daaf6152771_b186095e22b6`. Version is taken from `supported_versions` when present, GREASE ignored; the handshake-layer version is ignored. The reference implementation also emits `JA4_o` (original, unsorted order) and `JA4_r`/`JA4_ro` (raw, unhashed) variants, and handles per-stream counters ([ja4.py](https://github.com/FoxIO-LLC/ja4/blob/main/python/ja4.py)).

The rest of the suite ([FoxIO README](https://github.com/FoxIO-LLC/ja4), [FoxIO JA4 blog](https://foxio.io/blog/ja4-network-fingerprinting), [JA4T blog](https://foxio.io/blog/ja4t-tcp-fingerprinting)):

| Fingerprint  | Input                               | Shape / notes                                                                                                                                                                                                                              |
| ------------ | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| JA4S         | ServerHello                         | `{t                                                                                                                                                                                                                                        | q}{ver}{ext_count}{alpn}_{single_cipher}_{ext_hashes}`; not sorted; depends on the client hello it answered |
| JA4H         | HTTP request                        | method+version+cookie/referer presence, 2-digit header count (Cookie/Referer excluded, >99→99), 4-char language, then cookie-field hash and a per-user hash ([JA4H](https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4H.md)) |
| JA4X         | X.509 cert generation style         | issuer/subject/RDN-OID/extensions hashes; catches Sliver/Cobalt Strike even with randomized certs                                                                                                                                          |
| JA4L / JA4LS | Latency + TTL from handshake timing | `{half-RTT_µs}_{TTL}`; distance estimate + OS guess; works on encrypted traffic                                                                                                                                                            |
| JA4T / JA4TS | TCP SYN / SYN-ACK                   | `{window}_{option-kinds}_{MSS}_{window-scale}` e.g. `64240_2-1-3-1-1-4_1460_8`; MSS reveals tunnels/VPN/carrier                                                                                                                            |
| JA4TScan     | Active SYN scan                     | adds retransmission timing                                                                                                                                                                                                                 |
| JA4SSH       | SSH session (rolling)               | `c36s36_c51s80_c69s0` style; reverse-shell detection                                                                                                                                                                                       |

GREASE handling is consistent across implementations: RFC 8701 GREASE values must be ignored in version/cipher/extension processing ([RFC 8701](https://www.rfc-editor.org/rfc/rfc8701.html)); Envoy's implementation literally filters with `JA4Fingerprinter::isNotGrease` ([tls_inspector.cc](https://github.com/envoyproxy/envoy/blob/main/source/extensions/filters/listener/tls_inspector/tls_inspector.cc)); HAProxy exposes a filter option `1` documented as "exclude GREASE (RFC8701) values" ([HAProxy config manual](https://docs.haproxy.org/3.2/configuration.html)).

Also relevant: JA4's `b` section is the cipher hash; on the curated JA4DB dataset, `ja4_b`, `cipher_count` and `ext_count` were the most influential features for bot classification (§5).

### 1.4 ECH / ESNI impact

- ECH is now **RFC 9849** (Proposed Standard, March 2026) — it encrypts ClientHelloInner under an HPKE key fetched via DNS HTTPS/SVCB records; the wire carries a decoy ClientHelloOuter plus an `encrypted_client_hello` extension ([RFC 9849](https://datatracker.ietf.org/doc/rfc9849/); [Cloudflare ECH explainer](https://blog.cloudflare.com/encrypted-client-hello/)).
- Consequence for fingerprinting: an on-path observer sees only ClientHelloOuter — deliberately padded and greased, so its JA4 is a _decoy_ fingerprint shared across an anonymity set, and the `ech_outer_extensions` compression means inner extensions may be referenced rather than repeated ([RFC 9849 §5.1](https://www.rfc-editor.org/rfc/rfc9849.html)).
- A terminator that _supports_ ECH (shared mode) decrypts and therefore still sees ClientHelloInner; Cloudflare is a client-facing/backend server in shared mode. What Cloudflare's JA4 is computed over for an ECH connection is not documented publicly — treat JA4 for ECH clients as unreliable and verify empirically.
- ECH visibility flags you _can_ get: nginx 1.29.4+ has `$ssl_ech_status` (`SUCCESS`/`FAILED`/`GREASE`/`NOT_TRIED`/`BACKEND`) and `$ssl_ech_outer_server_name` ([nginx ssl module](https://nginx.org/en/docs/http/ngx_http_ssl_module.html)).
- Cloudflare may also respond with `ech_required`/retry configs, so a single connection may not carry application data at all.

### 1.5 Session resumption effects

- Cloudflare: "With TLS Session Resumption … subsequent connections will be streamlined. This results in no further fingerprint calculation." JA3/JA4 may therefore be **null on resumed connections** — plan for missing fields ([Cloudflare JA3/JA4 docs](https://developers.cloudflare.com/bots/concepts/ja3-ja4-fingerprint/)).
- CloudFront surfaces the state explicitly in `CloudFront-Viewer-TLS` as `fullHandshake | sessionResumed | connectionReused` ([CloudFront headers](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/adding-cloudfront-headers.html)).
- nginx exposes `$ssl_session_reused` (`r`/`.`) ([nginx ssl module](https://nginx.org/en/docs/http/ngx_http_ssl_module.html)); HAProxy `ssl_fc_is_resumed`, `ssl_fc_has_early`, and TLS 1.3 early-data secrets/`ssl_fc_has_early` ([HAProxy manual](https://docs.haproxy.org/3.2/configuration.html)).
- Practical rule: **never treat a missing JA3/JA4 as "clean".** Resumption, non-TLS, Worker subrequests, O2O internals, and skipped Bot Management all produce empty values (Cloudflare doc, same link).

### 1.6 Other ClientHello fields worth logging

ALPN (Cloudflare JA4 `a` section and nginx `$ssl_alpn_protocol`), supported_groups/`named_groups` (rustls exposes them, see §8), signature_algorithms (nginx `$ssl_sigalgs`, 1.31.2+; HAProxy `ssl_fc_sigalgs_bin`), key-exchange group actually negotiated (Cloudflare Logpush `ClientTLSKeyExchangeGroup`, e.g. `X25519MLKEM768`), TLS key exchange group to origin (`OriginTLSKeyExchangeGroup`), and cipher (`tlsCipher`).

---

## 2. HTTP/2 and HTTP/3 fingerprinting

### 2.1 HTTP/2: the Akamai method

The canonical description is Akamai's Black Hat EU 2017 whitepaper by Ory Segal, Aharon Fridman and Elad Shuster, which defined the format still used today ([whitepaper PDF](https://www.blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf), referenced by [Finch](https://github.com/0x4D31/finch)):

```
S[;] | WU | P[,]
```

- `S` = SETTINGS parameters as `key:value`, in the order sent (e.g. `1:65536;4:131072;5:16384`).
- `WU` = WINDOW_UPDATE increment (`0` if absent).
- `P` = PRIORITY tuples `StreamID:Exclusive:DependentID:Weight`, comma-separated (`0` if absent).
- Example Chrome: `1:65536;3:1000;4:6291456|15663105|0`; OkHttp: `4:16777216|16711681|0`.

The whitepaper notes the RFC itself predicts this: "the value of settings, the manner in which flow-control windows are managed, the way priorities are allocated to streams …" (RFC 7540 §10.8). Modern implementations add a fourth field for **pseudo-header order** (e.g. `|m,a,s,p`), and browser-impersonation libraries treat that as part of the fingerprint ([httpcloak](https://github.com/sardanioss/httpcloak) describes "Akamai fingerprint" alongside header order; [pingly](https://github.com/0x676e67/pingly) parses "Akamai HTTP/2" fingerprints plus "ordered HTTP/1 fields, HTTP/2 frames, QPACK fields, and QUIC transport parameters").

**Exposure problem.** HTTP/2 frame-level data is only available to the HTTP/2 terminator. At a Rust origin using `hyper`/`h2`, the peer's SETTINGS are not exposed: `h2::server::Builder` configures _our_ SETTINGS, and `Connection` only reports our acknowledged limits ([h2 server source](https://github.com/hyperium/h2/blob/master/src/server.rs)); hyper 1.x's `src/ext` contains only `h1_reason_phrase`, `informational` — no HTTP/2 extension access. Behind Cloudflare, this is moot: Cloudflare terminates HTTP/2 and does not expose SETTINGS fingerprints to customers.

### 2.2 HTTP/3 / QUIC

- JA4 marks QUIC with `q` and works on the QUIC-encapsulated ClientHello; the QUIC handshake is also the input for JA4L's UDP variant ([JA4 spec](https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md), [FoxIO blog](https://foxio.io/blog/ja4-network-fingerprinting)).
- Beyond TLS-in-QUIC, the distinctive surface is **transport parameters** (idle timeout, max UDP payload, initial max data/streams, active connection ID limit, GREASE transport parameters), packet-number length, spin bit, and 0-RTT/early-data behaviour. Tooling that parses these: pingly ("QUIC transport parameters", ordered), Finch (experimental "Parse QUIC Initial packets to generate JA4 fingerprints"), curl_cffi/node-libcurl-style impersonators ([Finch README](https://github.com/0x4D31/finch), [pingly](https://github.com/0x676e67/pingly)).
- Historical QUIC fingerprinting lineage: Salesforce's **CYU hash** for gQUIC, implemented in Zeek/Suricata ([Fastly](https://www.fastly.com/blog/the-state-of-tls-fingerprinting-whats-working-what-isnt-and-whats-next)); QUIC's own privacy considerations (correlation via settings, flow control, priorities) mirror HTTP/2's.
- 0-RTT caveat: early data is replayable; servers log/limit it (`$ssl_early_data`, `ssl_fc_has_early`) and 0-RTT sessions also skip parts of the handshake that fingerprints rely on.
- Cloudflare, behind its own edge: `request.cf.httpProtocol` ("HTTP/2", "HTTP/3"), `clientQuicRtt` (ms, QUIC only), `clientTcpRtt` (TCP only), `edgeL4.deliveryRate`, and `requestPriority` ("weight=192;exclusive=0;group=3;group-weight=127") — the latter is the browser-requested priority, i.e. an HTTP/2/3 priority signal available on **all plans** ([Workers Request docs](https://developers.cloudflare.com/workers/runtime-apis/request/)). No transport-parameter fingerprint is exposed.

---

## 3. TCP/IP fingerprinting, and why CDNs destroy it

### 3.1 What the methods look at

- **p0f v3** fingerprints the client SYN and server SYN-ACK: TCP option layout/order, MSS, window size + scale, IP initial TTL (via `observed+distance`), IPv6 extension headers, quirks in must-be-zero fields, ACK/URG anomalies, timestamp progression, MTU classification, plus HTTP header order/absence for app fingerprinting. It also does **NAT/proxy detection** by watching for signature changes, timestamp jumps, TTL/MTU changes, and source-port decreases ([p0f README](https://lcamtuf.coredump.cx/p0f3/README)).
- **nmap** OS detection actively sends up to 16 probes and tests ISN GCD/rate/predictability, IP-ID algorithms, TCP timestamps (`TS`, with 2 Hz/100 Hz/200 Hz special cases), option strings, window sizes, DF bit, ECN, and ICMP error handling ([nmap OS detection methods](https://nmap.org/book/osdetect-methods.html)).
- **JA4T/JA4TS/JA4TScan** are the modern passive/active equivalents; JA4T's MSS component identifies tunnels/VPNs and even mobile carriers (each carrier sets a different MSS), and an iPhone on iCloud Private Relay shows a completely different JA4T ([JA4T blog](https://foxio.io/blog/ja4t-tcp-fingerprinting)).
- **TCP timestamp skew** as a device identifier comes from Kohno et al., "Remote Physical Device Fingerprinting" (IEEE S&P 2005) — clock skew estimation from timestamps allows tracking a device across IP changes ([DOI 10.1109/SP.2005.16](https://doi.org/10.1109/SP.2005.16)).

### 3.2 Why the CDN kills it

Cloudflare terminates TCP at the edge. The origin sees a fresh TCP connection from a Cloudflare edge IP, with Cloudflare's own window/MSS/options/timestamps. There is **no customer-facing field for eyeball TCP parameters** — the only transport-ish client signals in `request.cf` are `clientTcpRtt`/`clientQuicRtt` and `edgeL4.deliveryRate` ([Workers Request docs](https://developers.cloudflare.com/workers/runtime-apis/request/)). JA4T at the edge is not offered to customers (Cloudflare's public JA4 material covers JA3/JA4/JA4 signals only). Anything TCP/IP-level you compute at the origin is a fingerprint of Cloudflare's egress — stable, shared, and useless for client identity. The same is true for Traefik if it sits behind Cloudflare.

The only ways to get JA4T-class data behind a CDN you don't control:

1. Run your own TCP path for that traffic (e.g. a non-proxied hostname / Spectrum-style TCP app), or
2. Terminate TCP yourself on a non-CDN host and only use the CDN for cacheable static traffic, or
3. Buy the signal from the CDN (not currently offered by Cloudflare).

Also note proxy/VPN side effects documented by FoxIO: a proxy presents the _proxy's_ TCP fingerprint; iCloud Private Relay changes it entirely; VPNs change MSS and sometimes window size.

---

## 4. Availability per component (docs-backed)

### 4.1 Cloudflare (eyeball side)

| Item                                                                                                                                                                   | Availability                                                                                   | Source                                                                                                                                                                                                                                                                                                   |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| JA3 hash (`JA3Hash` log field; `cf.bot_management.ja3_hash`; `request.cf.botManagement.ja3Hash`)                                                                       | Enterprise + Bot Management                                                                    | [JA3/JA4 docs](https://developers.cloudflare.com/bots/concepts/ja3-ja4-fingerprint/), [Bot Mgmt variables](https://developers.cloudflare.com/bots/reference/bot-management-variables/), [Logpush http_requests](https://developers.cloudflare.com/logs/logpush/logpush-job/datasets/zone/http_requests/) |
| JA4 (`JA4` log field; `cf.bot_management.ja4`; `request.cf.botManagement.ja4`)                                                                                         | Enterprise + Bot Management                                                                    | same                                                                                                                                                                                                                                                                                                     |
| JA4 Signals (`JA4Signals` log object; `request.cf.botManagement.ja4Signals`)                                                                                           | Enterprise + Bot Management                                                                    | [JA4 signals blog](https://blog.cloudflare.com/ja4-signals/)                                                                                                                                                                                                                                             |
| `tlsClientCiphersSha1`, `tlsClientExtensionsSha1`, `tlsClientExtensionsSha1Le`                                                                                         | **All plans**                                                                                  | [Workers Request docs](https://developers.cloudflare.com/workers/runtime-apis/request/)                                                                                                                                                                                                                  |
| `tlsCipher`, `tlsVersion`, `tlsClientHelloLength`, `tlsClientRandom`, `tlsClientAuth` (mTLS)                                                                           | All plans                                                                                      | same                                                                                                                                                                                                                                                                                                     |
| `clientTcpRtt`, `clientQuicRtt`, `edgeL4.deliveryRate`, `httpProtocol`, `requestPriority`                                                                              | All plans                                                                                      | same                                                                                                                                                                                                                                                                                                     |
| `asn`, `asOrganization`, `colo`, geo (country/city/lat/long/region/timezone)                                                                                           | All plans                                                                                      | same                                                                                                                                                                                                                                                                                                     |
| Bot score / detection IDs / tags / verified bot / static resource / corporate proxy / signed agent                                                                     | Bot Management (score+IDs/tags); corporate proxy Enterprise-only; signed agents = Web Bot Auth | [Bot Mgmt variables](https://developers.cloudflare.com/bots/reference/bot-management-variables/), [Bot scores](https://developers.cloudflare.com/bots/concepts/bot-score/)                                                                                                                               |
| Headers to origin (`CF-Connecting-IP`, `CF-IPCountry`, `CF-Visitor`, `Cf-Ray`, `X-Forwarded-For/Proto`, `True-Client-IP` Ent., `CF-Connecting-IPv6`, `CF-Pseudo-IPv4`) | All plans (True-Client-IP Ent.)                                                                | [HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)                                                                                                                                                                                                                   |
| Logpush (all fields incl. `ClientSrcPort`, `ClientTCPRTTMs`, `ClientTLSKeyExchangeGroup`, `EdgeTimeToFirstByteMs`)                                                     | All plans; near-real-time batches                                                              | [Logpush](https://developers.cloudflare.com/logs/logpush/), [dataset](https://developers.cloudflare.com/logs/logpush/logpush-job/datasets/zone/http_requests/)                                                                                                                                           |
| Instant Logs (WebSocket live stream of HTTP Requests fields)                                                                                                           | Business + Enterprise; 1 session/zone, 60 min max                                              | [Instant Logs](https://developers.cloudflare.com/logs/instant-logs/)                                                                                                                                                                                                                                     |
| HTTP/2 SETTINGS / pseudo-header order, HTTP/3 transport params, TCP SYN fingerprint                                                                                    | **Not exposed**                                                                                | no documented field; see §2, §3                                                                                                                                                                                                                                                                          |

Important caveat on the SHA-1 fields: they are base64 SHA-1 digests of the client cipher list and extension list (big- and little-endian variants), not JA3/JA4 and not composable into either (JA3 is an MD5 over a concatenated decimal string; JA4 needs extension _count_ and sorted lists plus signature algorithms). Treat them as stable equality keys / change detectors, and note they are computed on the ClientHello Cloudflare received — absent on resumed sessions.

### 4.2 Traefik (origin side)

- No JA3/JA4, no ClientHello exposure. Documented surface: `forwardedHeaders.trustedIPs/insecure/connection`, PROXY protocol (v1/v2) acceptance, `http2.maxConcurrentStreams`/table sizes (server-side only), HTTP/3 enablement, TLS options, and the `PassTLSClientCert` middleware (mTLS client certs only) ([EntryPoints](https://doc.traefik.io/traefik/reference/install-configuration/entrypoints/), [PassTLSClientCert](https://doc.traefik.io/traefik/reference/routing-configuration/http/middlewares/passtlsclientcert/)).
- Plugins are Yaegi-Go (middleware + provider) or WASM/http-wasm (middleware only) — no TLS-layer hook, so a plugin cannot see the ClientHello ([Extend Traefik](https://doc.traefik.io/traefik/extend/extend-traefik/)). A repo search for Traefik JA3 tooling returns nothing on GitHub (`api.github.com/search/repositories?q=traefik+ja3` → 0 items).
- TCP routers support TLS **passthrough**, which pushes termination to the origin — but then Traefik cannot route on HTTP attributes, and behind Cloudflare the passthrough still terminates at Cloudflare.

### 4.3 nginx

- Stock variables: `$ssl_ciphers` (client cipher list), `$ssl_curves`, `$ssl_sigalgs` (1.31.2+), `$ssl_alpn_protocol`, `$ssl_early_data`, `$ssl_curve`, `$ssl_protocol`, `$ssl_server_name`, `$ssl_session_reused`, `$ssl_ech_status`, `$ssl_ech_outer_server_name`, client-cert variables ([nginx ssl module](https://nginx.org/en/docs/http/ngx_http_ssl_module.html)). No extension list/count → **JA3/JA4 are not computable from stock nginx variables**.
- FoxIO's module patches nginx (1.26+) to save the raw ClientHello and exposes `$http_ssl_ja4`, `$http_ssl_ja4one` (excludes dynamic extensions), `$http_ssl_ja4h`, and `$http_ssl_ja4t` (requires a second patch + `tcp_save_syn on;` and Linux `TCP_SAVE_SYN`; note: behind a TCP proxy it fingerprints the proxy) ([ja4-nginx-module](https://github.com/FoxIO-LLC/ja4-nginx-module)).

### 4.4 Envoy

- `envoy.filters.listener.tls_inspector` has `enable_ja3_fingerprinting` and `enable_ja4_fingerprinting` ([tls_inspector.proto](https://github.com/envoyproxy/envoy/blob/main/api/envoy/extensions/filters/listener/tls_inspector/v3/tls_inspector.proto)); the filter computes them in the BoringSSL select-certificate callback and stores them on the connection socket (`setJA3Hash`, `setJA4Hash`) ([tls_inspector.cc](https://github.com/envoyproxy/envoy/blob/main/source/extensions/filters/listener/tls_inspector/tls_inspector.cc)).
- The documented listener-filter outputs are SNI/ALPN for filter-chain matching, stats, and dynamic metadata on failure; there is no documented request header or access-log formatter for the JA3/JA4 hash, so surfacing it to an app requires a custom filter or a log/telemetry consumer.

### 4.5 HAProxy

- No dedicated `ssl_fc_ja3` fetch, but the config manual ships a complete JA3 recipe:
  ```
  http-request set-header X-SSL-JA3 %[ssl_fc_protocol_hello_id],\
    %[ssl_fc_cipherlist_bin(1),be2dec(-,2)],\
    %[ssl_fc_extlist_bin(1),be2dec(-,2)],\
    %[ssl_fc_eclist_bin(1),be2dec(-,2)],\
    %[ssl_fc_ecformats_bin,be2dec(-,1)]
  acl is_malware req.fhdr(x-ssl-ja3),digest(md5),hex -f /path/to/malware-ja3.lst
  ```
  Filter option `1` excludes GREASE. Requires `tune.ssl.capture-buffer-size`. JA4-relevant fetches also exist: `ssl_fc_sigalgs_bin`, `ssl_fc_supported_versions_bin`, `ssl_fc_alpn`, `ssl_fc_is_resumed` ([HAProxy 3.2 manual](https://docs.haproxy.org/3.2/configuration.html)). Community JA4/JA4H plugins exist ([OXL](https://github.com/O-X-L/haproxy-ja4-fingerprint)).

### 4.6 Caddy

- Core placeholders cover TLS connection state (`{http.request.tls.*}`: version, cipher_suite, proto, server_name, client certificate fields); no JA3/JA4 in core ([Caddy placeholders](https://caddyserver.com/docs/extending-caddy/placeholders)).
- Community modules: `rushiiMachine/caddy-ja3` (40★) and `josuebrunel/caddy-ja3ja4` (2026) ([GitHub API search](https://api.github.com/search/repositories?q=caddy+ja4+OR+caddy+ja3)). Cloudflare's Web Bot Auth repo also ships a Caddy plugin for signature verification ([Cloudflare Web Bot Auth](https://blog.cloudflare.com/web-bot-auth/)).

### 4.7 Rust ecosystem (details in §8)

`rustls` exposes a parsed ClientHello subset but not raw bytes or extension lists; `hyper`/`h2` expose no peer HTTP/2 settings; JA3/JA4 must be computed by a parser that sees the raw ClientHello bytes (`tls-parser`, `pingly`, `ja4`, `huginn-net-tls`, `prosopo-ja4`, `rama`).

---

## 5. TLS+HTTP fingerprint based bot detection in practice

### 5.1 What production systems actually model

Cloudflare (the best-documented production system):

- **Heuristics**: pattern matching against a DB of known malicious fingerprints; deterministic hits get score 1 (occasionally 29 while overlapping traffic is assessed).
- **Machine learning**: supervised, CatBoost; "accounts for the majority of all detections"; inputs are "headers, session characteristics, and browser signals"; output is P(human) mapped to 1–99. In 2023 Cloudflare said ML produced the final Bot Score for **over 72% of all HTTP requests**, at 46M req/s average. Features moved from single-request to **inter-request aggregates** in sliding windows (e.g. "number of unique user agents associated with certain request attributes") — this is exactly the JA4 Signals concept ([Bot scores](https://developers.cloudflare.com/bots/concepts/bot-score/), [scalable ML blog](https://blog.cloudflare.com/scalable-machine-learning-at-cloudflare/), [JA4 signals blog](https://blog.cloudflare.com/ja4-signals/)).
- **JA4 Signals** (per-JA4, 1-hour window, computed globally) include `browser_ratio_1h`, `cache_ratio_1h`, `h2h3_ratio_1h`, `heuristic_ratio_1h`, `uas_rank_1h`, `paths_rank_1h`, `reqs_rank_1h`, `ips_rank_1h`, `reqs_quantile_1h`, `ips_quantile_1h`. Cloudflare's stated motivation: a bare hash caused customers to block new browser releases; context fixes that. They analyse >15M unique JA4s/day ([JA4 signals blog](https://blog.cloudflare.com/ja4-signals/)).
- **Other engines**: JavaScript Detections (headless browsers; invisible JS injection), Anomaly Detection (unsupervised, now deprecated for new customers), Cloudflare Service (Zero Trust FP avoidance), verified-bot validation (rDNS + ASN + public lists + internal ML), `__cf_bm` cookie to smooth per-session scoring ([Bot scores](https://developers.cloudflare.com/bots/concepts/bot-score/)).

### 5.2 Accuracy and false positives

- Academic (curated data): "When Handshakes Tell the Truth" (arXiv Feb 2026) trains XGBoost/CatBoost on JA4DB fingerprints; CatBoost reached **AUC 0.998, F1 0.9734, accuracy 0.9863**, with `ja4_b`, `cipher_count`, `ext_count` most influential. Caveat: JA4DB is a labelled fingerprint database, not live adversarial traffic, and the authors plan HTTP/3 extension ([arXiv:2602.09606](https://arxiv.org/abs/2602.09606)).
- Web agents (live honeysites, June 2026): six LLM-based web agents were all distinguishable from humans and from each other using multi-layer (network+HTTP+browser) fingerprinting; several bypassed CAPTCHA/PoW/robots.txt and Cloudflare's free solutions; **stealth/anti-detection tooling often increased detectability** ([arXiv:2606.30119](https://arxiv.org/abs/2606.30119)).
- Cloudflare's own FP history: native mobile app traffic was systematically false-positive because "human" training labels came from CAPTCHA solvers that mobile apps cannot run; one food-delivery customer saw Android FPs drop to ~0% after retraining, a Web3 platform went from **28.7%–40.7% FPs on edge-case mobile traffic to ~0%** ([mobile bots blog](https://blog.cloudflare.com/machine-learning-mobile-traffic-bots/)).
- Documented systemic FP triggers: **missing/empty User-Agent → immediate score 1**, hitting Zero Trust/WARP and corporate-proxy environments; mitigation is `cf.bot_management.corporate_proxy` or IP allowlists. Bot scores aren't computed for requests handled by other features (redirects, some rules), and score 0 means "not evaluated", not "safe" ([Bot scores](https://developers.cloudflare.com/bots/concepts/bot-score/)).
- FP _structural_ risks: fingerprint spoofing is cheap for popular libraries (`curl_cffi`, `tls-client`, `wreq`, `koon`, `warpsock`, and JA3/JA4 "parroting" via patched rustls in `ja-tools` all advertise browser-identical TLS/h2/h3 fingerprints — see §8/§9 tooling), and one fingerprint can cover a whole legitimate class (Cobalt Strike's JARM equals Java's, so blocking it blocks all Java TLS servers) ([Fastly](https://www.fastly.com/blog/the-state-of-tls-fingerprinting-whats-working-what-isnt-and-whats-next)).
- Rule of thumb from the literature and vendors: use fingerprints as **one feature among several**, require multiple corroborating signals, and never hard-block a fingerprint without an allowlist path.

---

## 6. IP intelligence

### 6.1 ASN classification (datacenter vs residential vs mobile)

- **RIR delegated statistics** are the free ground truth for ASN→org→country and IP→ASN: `registry|cc|type|start|value|date|status` lines, including `asn` rows; e.g. RIPE NCC's file is ~167k lines, updated daily ([RIPE delegated stats](https://ftp.ripe.net/pub/stats/ripencc/delegated-ripencc-latest); ASN registry at [IANA](https://www.iana.org/assignments/as-numbers/as-numbers.xhtml); sibling files at ARIN/APNIC/AFRINIC/LACNIC).
- Caveats: registry country is the _allocation_ country, not the user's; allocations get transferred/hijacked; the file's `status` (allocated/assigned/available) is not a business classification. Datacenter vs residential must come from enrichment vendors or heuristics (reverse DNS, hosting ASN lists, TLS/JA4T MSS).
- Cloudflare gives `asn` + `asOrganization` to all plans; CloudFront gives `CloudFront-Viewer-ASN`; IPinfo sells ASN/company/carrier datasets; IPinfo's `ip-carrier` product and FoxIO's JA4T MSS table both address mobile-carrier identification.

### 6.2 VPN / proxy / Tor / residential-proxy detection

- **IPinfo Privacy Detection**: evidence-based (they claim active measurement of exit-node behaviour via their ProbeNet methodology), returns `anonymous.name`, `last_seen`, `percent_days_seen`, `is_proxy/is_relay/is_tor/is_vpn`, plus `is_hosting`/`is_anonymous`. Claims ">99%" accuracy for VPN/proxy, daily updates, API + MMDB/CSV/JSON/Parquet; 44M entries in the privacy DB ([IPinfo privacy detection](https://ipinfo.io/data/proxy-vpn-detection), [IPinfo residential proxy](https://ipinfo.io/data/residential-proxy)).
- **Residential proxies are a separate problem**: they borrow real ISP IPs, so ownership data and standard privacy detection miss them. IPinfo ships a dedicated 64M-entry dataset with `service`, `last_seen`, `percent_days_seen`; Spur similarly markets residential-proxy and VPN detection with a free client-side "Monocle" layer plus Cloudflare/CloudFront/Fastly integrations, a Context API, and bulk data feeds ([IPinfo residential proxy](https://ipinfo.io/data/residential-proxy), [Spur docs](https://docs.spur.us/), [Spur Monocle × Cloudflare](https://docs.spur.us/monocle/integrations/cloudflare)).
- Tor: exit lists are public; Cloudflare already emits `ClientIPClass: tor` in Logpush and `CF-IPCountry: T1` for Tor clients ([Logpush dataset](https://developers.cloudflare.com/logs/logpush/logpush-job/datasets/zone/http_requests/), [HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)).
- Terms/provenance: IPinfo's data is commercial and governed by their ToS/AUP/DPA ([IPinfo terms](https://ipinfo.io/terms-of-service)); Spur is commercial (free Monocle tier); MaxMind GeoLite2 requires an account/license key and is explicitly not warranted for commercial-grade use, with attribution requirements in the GeoLite2 EULA ([GeoLite2 EULA](https://www.maxmind.com/en/geolite2/eula), [GeoLite2 data](https://dev.maxmind.com/geoip/geolite2-free-geolocation-data)).
- **Cloudflare Radar** is free and API-accessible but aggregated/anonymized (global/regional/ASN trends, attack data, URL scanner) and licensed **CC BY-NC 4.0** — non-commercial; it is not a per-IP reputation feed ([Radar docs](https://developers.cloudflare.com/radar/)).

### 6.3 Geolocation accuracy (don't over-trust it)

MaxMind's own accuracy comparison (IPs correctly located within 50 km) shows large variance by country: **US 60%**, Canada 74%, UK 67%, Germany 71%, France 50%, Japan 51%, Hong Kong 29% (71% with no city-level data at all), Finland 35%. MaxMind explicitly says GeoLite's limitations make it unsuitable for commercial-grade applications ([MaxMind accuracy tool](https://www.maxmind.com/en/geoip2-city-accuracy-comparison)). VPN/proxy/residential-proxy IPs break geo entirely, and mobile IPs geolocate to carrier gateways.

### 6.4 IPv6 privacy churn, CGNAT, and shared-IP bans

- **IPv6 privacy addresses**: RFC 8981 (obsoletes RFC 4941) specifies temporary addresses that rotate and are not stable per host; a single subscriber normally controls a /64 (or larger). Consequences: per-address rate limits are trivially evaded; per-/64 limits are needed; prefix-level bans hit every device on the connection; device-level continuity via IP is unreliable ([RFC 8981](https://www.rfc-editor.org/rfc/rfc8981.html)). Cloudflare mitigates a related issue by offering `CF-Connecting-IPv6` and pseudo-IPv4 ([HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)).
- **CGNAT**: RFC 6888 (with RFC 6598 shared address space) documents carrier NAT where many subscribers share one public IP; per-IP bans and rate limits create collateral damage across unrelated users, and "one user = one IP" assumptions fail ([RFC 6888](https://www.rfc-editor.org/rfc/rfc6888.html)). Practical mitigations: group on **IP + source port** where available (Cloudflare Logpush exposes `ClientSrcPort`), lengthen windows, escalate to challenges rather than bans, and use account/cookie-level identity rather than IP.
- NAT detection is also a fingerprinting feature: p0f flags IP sharing via timestamp jumps/TTL/MTU changes and source-port decreases ([p0f README](https://lcamtuf.coredump.cx/p0f3/README)).

---

## 7. When TLS terminates at a CDN you don't control: what remains

Enumerated fallback signals, with what they can and cannot tell you.

1. **CDN-injected client-identity headers.** Cloudflare: `CF-Connecting-IP`, `X-Forwarded-For` (appends proxies), `X-Forwarded-Proto`, `CF-IPCountry` (`T1` for Tor, `XX` unknown), `Cf-Ray` (colo), `True-Client-IP` (Enterprise), `CF-Connecting-IPv6`, `CF-Pseudo-IPv4`, `CF-Visitor`, `CF-Worker`, `CF-Connecting-O2O`. Cloudflare passes all client headers through except invalid names, and strips `x-real-ip` when no Worker subrequest is involved. `X-Forwarded-For`/`True-Client-IP` are spoofable if the origin is reachable outside Cloudflare ([HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)).
2. **Cloudflare `request.cf` (Workers, all plans).** TLS state (`tlsVersion`, `tlsCipher`, `tlsClientHelloLength`, `tlsClientRandom`, `tlsClientCiphersSha1`, `tlsClientExtensionsSha1`, `tlsClientExtensionsSha1Le`), transport (`clientTcpRtt`, `clientQuicRtt`, `edgeL4.deliveryRate`, `httpProtocol`, `requestPriority`), network (`asn`, `asOrganization`, `colo`), geo, mTLS (`tlsClientAuth`), `clientAcceptEncoding`, `hostMetadata` ([Workers Request docs](https://developers.cloudflare.com/workers/runtime-apis/request/)). This is the richest per-request source available without Bot Management.
3. **Cloudflare Bot Management fields (Enterprise).** `ja3Hash`, `ja4`, `ja4Signals`, `score`, `verifiedBot`, `staticResource`, `detectionIds`, `tags`, `signedAgent`, `corporate_proxy`, `verified_bot_category` — available in Workers, Ruleset Engine, and Logpush ([Bot Mgmt variables](https://developers.cloudflare.com/bots/reference/bot-management-variables/)).
4. **Logpush → consumer pipeline (all plans, async).** Batches "as quickly as possible … potentially more than once per minute", max 4 jobs/zone, **no backfill** if a job fails ([Logpush](https://developers.cloudflare.com/logs/logpush/)). Rich per-request fields incl. `ClientSrcPort`, `ClientTCPRTTMs`, `ClientTLSKeyExchangeGroup`, `ClientMTLSAuthCertFingerprint`, `EdgeTimeToFirstByteMs`, `OriginResponseDurationMs` ([dataset](https://developers.cloudflare.com/logs/logpush/logpush-job/datasets/zone/http_requests/)). Use for enrichment/analytics, not inline blocking.
5. **Instant Logs (Business+)**: WebSocket live stream of the same HTTP Requests fields; one session per zone, 60-minute cap, stops if the socket is idle >5 min ([Instant Logs](https://developers.cloudflare.com/logs/instant-logs/)). Closest thing to "real-time edge telemetry" available to customers, still not inline.
6. **Analytics GraphQL API / Security Events / Bot Analytics**: JA3/JA4 usable in analytics dashboards and queries ([JA3/JA4 docs](https://developers.cloudflare.com/bots/concepts/ja3-ja4-fingerprint/)).
7. **Origin-side HTTP-layer fingerprinting — with a hard caveat.** You can compute JA4H-style features (method, HTTP version, header count, `Accept-Language`, cookie field names, header order, casing on HTTP/1.1) at Traefik/axum, but behind Cloudflare these describe **Cloudflare's** request to the origin. Cloudflare overwrites `accept-encoding` and `connection`, may drop headers with invalid names, and adds its own headers ([HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)). Worker `Headers` are normalized too, so you cannot recover the eyeball's header order or casing. If eyeball header order matters, you need an edge platform that exposes it — CloudFront does (`CloudFront-Viewer-Header-Order`, `-Header-Count`) ([CloudFront headers](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/adding-cloudfront-headers.html)).
8. **Signed client identity instead of fingerprints.** mTLS (Cloudflare `tlsClientAuth`, Logpush `ClientMTLSAuthCertFingerprint`) gives a real client identity when clients can hold certs; Cloudflare's Authenticated Origin Pulls secures the CF→origin leg; Web Bot Auth (RFC 9421 signatures + `Signature-Agent`, plus a proposed `req mTLS` TLS flag) is Cloudflare's standards-based answer for legitimate bots/agents ([Web Bot Auth](https://blog.cloudflare.com/web-bot-auth/), [RFC 9421](https://www.rfc-editor.org/rfc/rfc9421.html)).
9. **Coarse timing signals.** `clientTcpRtt` / `clientQuicRtt` / `edgeL4.deliveryRate` can distinguish broad classes (satellite/mobile/datacenter-adjacent latency) but are noisy and easily manipulated by proximity to the POP.
10. **Precedent from other CDNs** (for what a CDN _can_ forward): CloudFront `CloudFront-Viewer-JA3-Fingerprint`, `-JA4-Fingerprint`, `-TLS` (with `fullHandshake|sessionResumed|connectionReused`), `-Header-Order`, `-Header-Count`, `-ASN`, `-Address` (ip:port) — note these are origin-request-policy headers, not cache-policy headers ([CloudFront headers](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/adding-cloudfront-headers.html)). Vercel sends `x-vercel-ja4-digest` and `x-vercel-ja3-digest` to Functions **on all plans** ([Vercel TLS fingerprints](https://vercel.com/docs/security/tls-fingerprints)). Fastly exposes JA3 via VCL/Compute (`tls-client-ja3-md5`) and offers JA4 ([Fastly](https://www.fastly.com/blog/the-state-of-tls-fingerprinting-whats-working-what-isnt-and-whats-next), [FoxIO support table](https://github.com/FoxIO-LLC/ja4)). ngrok exposes `conn.tls.ja4_fingerprint`. AWS WAF, Google Cloud Armor and Akamai expose JA4 in rules ([FoxIO table](https://github.com/FoxIO-LLC/ja4)).

**What is definitively lost behind Cloudflare:** eyeball TCP/IP fingerprint (JA4T, TTL, MSS, window scale, TCP timestamps/skew, IP options), eyeball HTTP/2 frame fingerprint (SETTINGS order/values, WINDOW_UPDATE, PRIORITY, pseudo-header order), eyeball HTTP/3 transport parameters, raw ClientHello bytes and extension order, and eyeball HTTP header order/casing. Cloudflare never publishes these to customers, and nothing in Traefik or axum can recover them.

---

## 8. Reproducible implementation notes

### 8.1 Rust APIs that actually expose ClientHello information

`rustls` (server side):

- `rustls::server::Acceptor` reads the handshake before a `ServerConfig` exists; `accept()` returns `Accepted`, and `Accepted::client_hello()` yields a `ClientHello` (the docs' example uses exactly this to pick a config per ClientHello) ([Acceptor docs](https://docs.rs/rustls/latest/rustls/server/struct.Acceptor.html), [Accepted docs](https://docs.rs/rustls/latest/rustls/server/struct.Accepted.html)).
- `ClientHello` exposes: `server_name()`, `signature_schemes()`, `alpn()`, `cipher_suites()`, `server_cert_types()`, `client_cert_types()`, `certificate_authorities()`, `named_groups()`. There is **no** accessor for the raw ClientHello bytes, the TLS version, the extension list, or extension order ([ClientHello docs](https://docs.rs/rustls/latest/rustls/server/struct.ClientHello.html)).
- Therefore: **JA3 cannot be computed from rustls alone** (needs ordered extensions, curves, EC point formats) and **JA4 cannot either** (needs extension count, sorted extension list, signature algorithms, version). You need a parser over the raw bytes. Options:
  - Tee the TCP stream: read into a buffer and feed the same bytes to `Acceptor::read_tls` and to your parser (rustls consumes the bytes, so capture first).
  - Use `tokio-rustls`'s `TlsAcceptor`/`LazyConfigAcceptor` equivalents for the same "read hello, then choose config" flow in async code.
  - Crates that parse ClientHello from bytes: `tls-parser` (rusticata), [`ja4`](https://crates.io/crates/ja4) (no_std JA4), [`pingly`](https://crates.io/crates/pingly) (`ClientHello::parse(&bytes)` → `.ja3().hash`, `.ja4().fingerprint`; also HTTP/1/2/3, QPACK, QUIC transport params, Akamai h2), `huginn-net-tls`, `prosopo-ja4`, `ja3`, `ja3-rustls` (works with rustls types), [`rama`](https://github.com/plabayo/rama) (JA4 + JA4H per FoxIO's support table). Cloudflare itself built an internal Rust `client-hello-parser` crate (zero-alloc, fuzzed, ~500 ns per short ClientHello) and open-sourced the _technique_ via `mmap-sync`, not the crate ([Cloudflare JA4 signals blog](https://blog.cloudflare.com/ja4-signals/), [Cloudflare ML blog](https://blog.cloudflare.com/scalable-machine-learning-at-cloudflare/)).
  - Client-side (for testing/impersonation): `ja-tools` patches rustls to override the ClientHello and check JA3/JA4 ([ja-tools](https://github.com/XOR-op/ja-tools)).

`hyper`/`h2` (origin side):

- No peer SETTINGS exposure. `h2::server::Builder` sets _our_ SETTINGS; `Connection` exposes our acknowledged limits ([h2 server.rs](https://github.com/hyperium/h2/blob/master/src/server.rs)). hyper 1.x `src/ext` has only `h1_reason_phrase` and `informational` ([hyper src/ext](https://github.com/hyperium/hyper/tree/master/src/ext)).
- What you _can_ do in axum: read `http::HeaderMap` (iteration preserves received order for the non-pseudo headers hyper passes through), `Method`, `Version`, `OriginalUri`, and header casing on HTTP/1.1. Pseudo-header order is consumed by hyper and unavailable. Combined with cookies and `Accept-Language`, this is enough for a JA4H-style _self_ fingerprint (of Cloudflare's request), or a real client fingerprint if the origin is directly exposed.

### 8.2 Design: "capture at edge, forward to origin as a signed header"

This pattern is exactly what CloudFront, Vercel and Fastly already do (§7). For a Cloudflare-fronted app the implementation is a Worker (or a Transform Rule, for simple static values).

**Step 1 — collect.** In the Worker, read `request.cf` fields you are entitled to:

```
tlsVersion, tlsCipher, tlsClientHelloLength, tlsClientRandom,
tlsClientCiphersSha1, tlsClientExtensionsSha1, tlsClientExtensionsSha1Le,
httpProtocol, requestPriority, clientTcpRtt, clientQuicRtt,
edgeL4.deliveryRate, asn, asOrganization, colo, country,
botManagement?.ja3Hash, botManagement?.ja4, botManagement?.score,
botManagement?.verifiedBot, botManagement?.detectionIds, botManagement?.tags
```

These are read-only and may be absent (resumed sessions, non-TLS, skipped Bot Management, subrequests) — the design must tolerate missing values.

**Step 2 — name the headers.** Use a vendor prefix that cannot collide with Cloudflare's own and is easy to strip, e.g. `X-Eq-Edge-*`:

```
X-Eq-Edge-JA4, X-Eq-Edge-JA3, X-Eq-Edge-TLS-Version, X-Eq-Edge-TLS-Cipher,
X-Eq-Edge-TLS-Hello-Len, X-Eq-Edge-TLS-Cipher-SHA1, X-Eq-Edge-TLS-Ext-SHA1,
X-Eq-Edge-TLS-Ext-SHA1-LE, X-Eq-Edge-Proto, X-Eq-Edge-Prio, X-Eq-Edge-RTT,
X-Eq-Edge-ASN, X-Eq-Edge-AS-Org, X-Eq-Edge-Colo, X-Eq-Edge-Country,
X-Eq-Edge-Bot-Score, X-Eq-Edge-Bot-Src, X-Eq-Edge-TS, X-Eq-Edge-Kid, X-Eq-Edge-Sig
```

**Step 3 — sign.** Canonical string over the values, not the whole request body:

```
canonical = "v1\n" + method + "\n" + url.pathname + "\n" +
            join(sorted(name + ":" + value for headers in the set), "\n") + "\n" +
            ts
sig = hex(HMAC-SHA256(secret_kid, canonical))
```

Send `X-Eq-Edge-TS: <unix-seconds>`, `X-Eq-Edge-Kid: v1` (or a rotating key id), `X-Eq-Edge-Sig: v1=<hex>`. In the Worker use WebCrypto (`crypto.subtle.importKey`/`sign`) — available in Workers ([Web Crypto docs](https://developers.cloudflare.com/workers/runtime-apis/web-crypto/)). In Rust verify with `hmac`+`sha2` (or `ring`/`aws-lc-rs`) and compare in constant time (`subtle::ConstantTimeEq`).

**Step 4 — replay window.** Enforce `|now - X-Eq-Edge-TS| <= 30 s` (tune 5–60 s). That alone bounds replay; for stronger guarantees keep a short-lived set of `(kid, ts, sig)` or a nonce header in a fast store (Workers KV is too slow for inline; Redis/`moka` in the axum process is fine). Rotate the HMAC secret on a schedule with overlapping key ids, and reject unknown `kid`s.

**Step 5 — enforce at the origin, and lock the origin down.** Critical details:

- **Strip inbound `X-Eq-Edge-*` from client requests.** Cloudflare forwards almost all client headers; without stripping, any client can forge these values.
- **Require a valid signature** (or a valid mTLS/Authenticated-Origin-Pull identity) before trusting any header; a missing signature means the request did not come through the Worker pipeline.
- **Make the origin unreachable except through Cloudflare**: Authenticated Origin Pulls, Cloudflare Tunnel, or a strict firewall allowlist of Cloudflare egress IPs; otherwise `CF-Connecting-IP`-style headers and even the Worker path can be bypassed ([HTTP headers docs](https://developers.cloudflare.com/fundamentals/reference/http-headers/)).
- Keep the header count small; don't put large blobs in headers.
- Log both raw values and a derived `edge_fp_version` so you can re-derive when Cloudflare changes field semantics.

**Step 6 — consumer/pipeline option.** If you need the full Logpush dataset (e.g. `ClientSrcPort`, `ClientMTLSAuthCertFingerprint`, `ClientTLSKeyExchangeGroup`, origin timings), push Logpush → R2/S3/Kafka and join asynchronously; treat it as a 1-minute-scale enrichment path, not an inline decision ([Logpush](https://developers.cloudflare.com/logs/logpush/)).

**What this design cannot give you**, no matter how it's wired: JA4T/TCP fingerprint, HTTP/2 SETTINGS, HTTP/3 transport params, eyeball header order/casing, or a JA4/JA3 hash without Bot Management. If those are hard requirements, the architecture must change (own the TLS/TCP termination for that traffic, or move to a platform that exposes them — CloudFront/Vercel/Fastly/ngrok/self-hosted).

---

## Signal → availability table

Legend: **✅** directly exposed · **🟡** derived/partial/indirect · **🔒** Enterprise + Bot Management only · **✖** not exposed · **n/a** meaningless at that hop.

| Signal                                           | Cloudflare (eyeball)                                                            | Traefik (origin-side)                                     | Rust origin (axum/hyper, no CDN)                 | Notes                                            |
| ------------------------------------------------ | ------------------------------------------------------------------------------- | --------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------------ |
| Raw TLS ClientHello bytes                        | ✖ (internal only)                                                               | ✖                                                         | ✅ (tee socket / acceptor)                       | Traefik has no TLS hook                          |
| JA3                                              | 🔒 (`JA3Hash`, `cf.bot_management.ja3_hash`)                                    | ✖                                                         | ✅ via raw parser                                | missing on resumed sessions                      |
| JA3S                                             | ✖                                                                               | ✖                                                         | ✅                                               | server-side, low uniqueness alone                |
| JA4                                              | 🔒 (`JA4`, `cf.bot_management.ja4`)                                             | ✖                                                         | ✅ via raw parser                                | `q`-prefix for QUIC                              |
| JA4S / JA4X / JA4L / JA4SSH                      | ✖                                                                               | ✖                                                         | ✅ if you own the bytes/tap                      | JA4X needs cert visibility (TLS1.2 only on wire) |
| JA4H                                             | 🟡 (your own logs + JA4H recipe; Cloudflare's copy is of the CF→origin request) | 🟡                                                        | ✅                                               | eyeball header order not recoverable behind CF   |
| JA4T / JA4TS / JA4TScan                          | ✖                                                                               | ✖                                                         | ✅ only with SYN capture (`TCP_SAVE_SYN`) or tap | CDN terminates TCP                               |
| TLS version, cipher (negotiated)                 | ✅ (`tlsVersion`, `tlsCipher`)                                                  | ✅ (`{http.request.tls.*}` in Caddy; TLS opts in Traefik) | ✅                                               |                                                  |
| Client cipher list                               | 🟡 SHA-1 digest (`tlsClientCiphersSha1`)                                        | ✖                                                         | ✅                                               | digest ≠ JA3 input                               |
| Client extension list / order / count            | ✖ (only `tlsClientExtensionsSha1(Le)` digest)                                   | ✖                                                         | ✅                                               |                                                  |
| ALPN                                             | 🟡 via JA4 `a` section (BM)                                                     | ✖                                                         | ✅                                               |                                                  |
| Supported groups / curves                        | ✖                                                                               | ✖                                                         | ✅ (rustls `named_groups`)                       |                                                  |
| Signature algorithms                             | ✖                                                                               | ✖                                                         | ✅                                               |                                                  |
| SNI                                              | 🟡 (used internally; not a `request.cf` field)                                  | ✅ (SNI available for routing)                            | ✅                                               |                                                  |
| ClientHello length / random                      | ✅                                                                              | ✖                                                         | ✅                                               |                                                  |
| ECH status                                       | ✖ (undocumented)                                                                | ✖                                                         | ✅ (nginx `$ssl_ech_status`; rustls side)        | RFC 9849                                         |
| Session resumption / 0-RTT                       | 🟡 (fingerprints vanish; `clientQuicRtt` only for QUIC)                         | ✖                                                         | ✅ (`$ssl_session_reused`, rustls)               |                                                  |
| HTTP/2 SETTINGS order+values                     | ✖                                                                               | ✖                                                         | ✖ (hyper/h2 don't expose peer settings)          | use Finch/pingly if self-hosted                  |
| HTTP/2 pseudo-header order                       | ✖                                                                               | ✖                                                         | ✖ (consumed by hyper)                            |                                                  |
| HTTP/1.x or h2 header order/casing (eyeball)     | ✖                                                                               | ✖                                                         | 🟡 (`HeaderMap` order; casing only HTTP/1.1)     | CF normalizes/overwrites                         |
| HTTP/3 transport parameters, pkt-num len, GREASE | ✖                                                                               | ✖                                                         | ✖ unless you run your own QUIC stack             |                                                  |
| Client TCP RTT                                   | ✅ (`clientTcpRtt`, `clientQuicRtt`)                                            | ✖                                                         | 🟡                                               | coarse                                           |
| Delivery rate                                    | ✅ (`edgeL4.deliveryRate`)                                                      | ✖                                                         | ✖                                                |                                                  |
| Client source port                               | 🟡 Logpush `ClientSrcPort` only                                                 | 🟡 (own connection)                                       | 🟡                                               | CGNAT grouping                                   |
| TCP TTL/MSS/window/options/timestamps            | ✖                                                                               | ✖                                                         | ✅ (own listener)                                | CDN destroys                                     |
| ASN / org                                        | ✅ (`asn`, `asOrganization`)                                                    | ✖                                                         | 🟡 (needs lookup)                                |                                                  |
| Geo (country/city/lat/long)                      | ✅ (all plans)                                                                  | ✖                                                         | 🟡                                               | accuracy caveats (§6.3)                          |
| VPN/proxy/Tor flag                               | 🟡 (`ClientIPClass: tor`; `CF-IPCountry: T1`; corporate proxy 🔒)               | ✖                                                         | 🟡 (third-party data)                            |                                                  |
| Residential-proxy flag                           | ✖                                                                               | ✖                                                         | 🟡 (IPinfo/Spur datasets)                        | separate dataset                                 |
| Bot score / detection IDs / tags                 | 🔒                                                                              | ✖                                                         | ✖                                                | Enterprise + BM                                  |
| Verified bot / signed agent (Web Bot Auth)       | 🔒 (signedAgent; verifiedBot)                                                   | ✖                                                         | 🟡 (verify RFC 9421 yourself)                    |                                                  |
| mTLS client cert fingerprint                     | 🟡 `tlsClientAuth` (all plans) + Logpush `ClientMTLSAuthCertFingerprint`        | 🟡 `PassTLSClientCert` middleware                         | ✅                                               | real identity, not a fingerprint                 |
| JA4 inter-request signals                        | 🔒                                                                              | ✖                                                         | ✖                                                | 1-hour global window                             |
| Logpush pipeline (rich per-request fields)       | ✅ all plans, ~1 min batches                                                    | n/a                                                       | n/a                                              | no backfill, 4 jobs/zone                         |
| Instant Logs (live WebSocket)                    | ✅ Business+                                                                    | n/a                                                       | n/a                                              | 60-min sessions                                  |
| Edge→origin header forwarding                    | ✅ (CF headers; Worker-signed custom headers)                                   | ✅ (X-Forwarded-*)                                        | ✅                                               | must be signed + origin locked down              |

---

## Sources

Fingerprinting standards and methods

- FoxIO JA4+ repo (methods, licensing, support table): https://github.com/FoxIO-LLC/ja4
- JA4 spec: https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md
- JA4H spec: https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4H.md
- JA4 reference implementation (Python): https://github.com/FoxIO-LLC/ja4/blob/main/python/ja4.py
- FoxIO JA4+ blog: https://foxio.io/blog/ja4-network-fingerprinting
- FoxIO JA4T/JA4TS/JA4TScan blog: https://foxio.io/blog/ja4t-tcp-fingerprinting
- Salesforce JA3: https://github.com/salesforce/ja3
- Fastly, "What is TLS Fingerprinting?" (JA3 pitfalls, JARM caveat, CYU): https://www.fastly.com/blog/the-state-of-tls-fingerprinting-whats-working-what-isnt-and-whats-next
- Akamai HTTP/2 fingerprinting whitepaper (Black Hat EU 2017): https://www.blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf
- RFC 8701 (GREASE): https://www.rfc-editor.org/rfc/rfc8701.html
- RFC 9849 (Encrypted Client Hello): https://datatracker.ietf.org/doc/rfc9849/
- Cloudflare, "Good-bye ESNI, hello ECH": https://blog.cloudflare.com/encrypted-client-hello/
- Chromium extension-order randomization: https://chromestatus.com/feature/5124606246518784
- p0f v3 README: https://lcamtuf.coredump.cx/p0f3/README
- nmap OS detection methods: https://nmap.org/book/osdetect-methods.html
- Kohno et al., Remote Physical Device Fingerprinting: https://doi.org/10.1109/SP.2005.16

Cloudflare (availability, fields, product behaviour)

- JA3/JA4 fingerprint (Enterprise + Bot Management; resumption caveat): https://developers.cloudflare.com/bots/concepts/ja3-ja4-fingerprint/
- Bot Management variables (`cf.bot_management.ja3_hash`/`ja4`, Workers, corporate proxy): https://developers.cloudflare.com/bots/reference/bot-management-variables/
- Bot scores (engines, ML, missing-UA FP, `__cf_bm`): https://developers.cloudflare.com/bots/concepts/bot-score/
- Bot Management plans: https://developers.cloudflare.com/bots/plans/
- Logpush http_requests fields (`JA3Hash`, `JA4`, `JA4Signals`, `ClientSrcPort`, `ClientTLSKeyExchangeGroup`, `ClientMTLSAuthCertFingerprint`): https://developers.cloudflare.com/logs/logpush/logpush-job/datasets/zone/http_requests/
- Logpush overview (batch cadence, 4 jobs/zone, no backfill): https://developers.cloudflare.com/logs/logpush/
- Instant Logs (Business+, WebSocket, limits): https://developers.cloudflare.com/logs/instant-logs/
- Workers Request / `request.cf` fields: https://developers.cloudflare.com/workers/runtime-apis/request/
- Workers Web Crypto: https://developers.cloudflare.com/workers/runtime-apis/web-crypto/
- Cloudflare HTTP request/response headers: https://developers.cloudflare.com/fundamentals/reference/http-headers/
- JA4 signals blog (inter-request features, 15M JA4s/day): https://blog.cloudflare.com/ja4-signals/
- Scalable ML at Cloudflare (CatBoost, 72% of scores, BLISS latency): https://blog.cloudflare.com/scalable-machine-learning-at-cloudflare/
- Mobile bot ML (FP rates 28.7–40.7% → ~0): https://blog.cloudflare.com/machine-learning-mobile-traffic-bots/
- Web Bot Auth (RFC 9421 signatures, mTLS flag): https://blog.cloudflare.com/web-bot-auth/
- Cloudflare Radar (free API, CC BY-NC 4.0): https://developers.cloudflare.com/radar/

Proxies / servers

- Traefik EntryPoints (forwarded headers, PROXY protocol, http2 settings): https://doc.traefik.io/traefik/reference/install-configuration/entrypoints/
- Traefik plugin systems (no TLS hook): https://doc.traefik.io/traefik/extend/extend-traefik/
- Traefik PassTLSClientCert: https://doc.traefik.io/traefik/reference/routing-configuration/http/middlewares/passtlsclientcert/
- nginx ssl module variables (`$ssl_ciphers`, `$ssl_curves`, `$ssl_sigalgs`, `$ssl_ech_status`, `$ssl_session_reused`): https://nginx.org/en/docs/http/ngx_http_ssl_module.html
- FoxIO nginx module (JA4/JA4H/JA4T; ClientHello patch; TCP_SAVE_SYN): https://github.com/FoxIO-LLC/ja4-nginx-module
- Envoy TLS Inspector docs: https://www.envoyproxy.io/docs/envoy/latest/configuration/listeners/listener_filters/tls_inspector
- Envoy tls_inspector proto (enable_ja3/ja4_fingerprinting): https://github.com/envoyproxy/envoy/blob/main/api/envoy/extensions/filters/listener/tls_inspector/v3/tls_inspector.proto
- Envoy tls_inspector source (JA3/JA4 computation, GREASE filter): https://github.com/envoyproxy/envoy/blob/main/source/extensions/filters/listener/tls_inspector/tls_inspector.cc
- HAProxy 3.2 configuration manual (JA3 recipe, `ssl_fc_*`, GREASE filter option): https://docs.haproxy.org/3.2/configuration.html
- Caddy placeholders: https://caddyserver.com/docs/extending-caddy/placeholders
- Caddy JA3/JA4 community modules: https://github.com/rushiiMachine/caddy-ja3 , https://github.com/josuebrunel/caddy-ja3ja4
- CloudFront request headers (Viewer-JA3/JA4/TLS/Header-Order/ASN/Address): https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/adding-cloudfront-headers.html
- Vercel TLS fingerprints (`x-vercel-ja4-digest`, `x-vercel-ja3-digest`, all plans): https://vercel.com/docs/security/tls-fingerprints

Rust

- rustls `ClientHello`: https://docs.rs/rustls/latest/rustls/server/struct.ClientHello.html
- rustls `Acceptor`: https://docs.rs/rustls/latest/rustls/server/struct.Acceptor.html
- rustls `Accepted`: https://docs.rs/rustls/latest/rustls/server/struct.Accepted.html
- h2 server source (settings are ours, not the peer's): https://github.com/hyperium/h2/blob/master/src/server.rs
- hyper `src/ext` (no HTTP/2 extension): https://github.com/hyperium/hyper/tree/master/src/ext
- pingly (TLS/HTTP/1/2/3, QUIC transport params, Akamai h2): https://github.com/0x676e67/pingly
- `ja4` crate: https://crates.io/crates/ja4 ; `tls-parser`: https://crates.io/crates/tls-parser ; `huginn-net-tls`: https://crates.io/crates/huginn-net-tls ; `prosopo-ja4`: https://crates.io/crates/prosopo-ja4
- rama (JA4/JA4H): https://github.com/plabayo/rama
- ja-tools (patched rustls, ClientHello override, JA3/JA4): https://github.com/XOR-op/ja-tools
- Finch (fingerprint-aware reverse proxy: JA3, JA4, JA4H, Akamai h2, experimental QUIC): https://github.com/0x4D31/finch
- FATT (pcap fingerprints): https://github.com/0x4D31/fatt

Bot-detection research

- Jarad & Bicakci, "When Handshakes Tell the Truth: Detecting Web Bad Bots via TLS Fingerprints" (JA4 + CatBoost, AUC 0.998): https://arxiv.org/abs/2602.09606
- Fayolle et al., "On the Internet, Nobody Knows You're an LLM Bot" (multi-layer fingerprinting; stealth increases detectability): https://arxiv.org/abs/2606.30119
- "Positional-Unigram Byte Models for Generalized TLS Fingerprinting": https://arxiv.org/abs/2405.07848
- "Capabilities and Limitations of TLS Fingerprinting in Server-Side Network Traffic Analysis" (2026 thesis): https://monami.hs-mittweida.de/frontdoor/index/index/docId/16771

IP intelligence / networking

- RIPE NCC delegated stats: https://ftp.ripe.net/pub/stats/ripencc/delegated-ripencc-latest
- IANA AS number registry: https://www.iana.org/assignments/as-numbers/as-numbers.xhtml
- MaxMind accuracy comparison (country-level 50 km): https://www.maxmind.com/en/geoip2-city-accuracy-comparison
- MaxMind GeoLite2 EULA: https://www.maxmind.com/en/geolite2/eula
- MaxMind GeoLite2 data/limits: https://dev.maxmind.com/geoip/geolite2-free-geolocation-data
- IPinfo privacy detection (VPN/proxy/Tor/relay/hosting): https://ipinfo.io/data/proxy-vpn-detection
- IPinfo residential proxy detection: https://ipinfo.io/data/residential-proxy
- IPinfo terms: https://ipinfo.io/terms-of-service
- Spur docs (Context API, Monocle, feeds; Cloudflare integration): https://docs.spur.us/ , https://docs.spur.us/monocle/integrations/cloudflare
- RFC 8981 (IPv6 temporary addresses): https://www.rfc-editor.org/rfc/rfc8981.html
- RFC 6888 (CGNAT requirements): https://www.rfc-editor.org/rfc/rfc6888.html
- RFC 9421 (HTTP Message Signatures): https://www.rfc-editor.org/rfc/rfc9421.html

Impersonation tooling referenced for FP discussion

- curl_cffi-style node bindings: https://github.com/Ossianaa/node-libcurl
- koon: https://github.com/scrape-hub/koon ; warpsock: https://github.com/jaredboynton/warpsock ; httpcloak: https://github.com/sardanioss/httpcloak ; wreq: https://github.com/0x676e67/wreq
