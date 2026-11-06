# Bulk Quran data

Where bulk Quran data is published, and the limits that apply to it.

## Paths

| Path | What it is |
| --- | --- |
| `/.well-known/quran-bulk.json` | Machine-readable index: offline pack paths, per-address limits, integrity rule. |
| `/llms.txt` | Plain-text site index. Links to the bulk index in its "Bulk data" section. |
| `/offline/manifest.json` | Current offline pack pointer. Served with `no-cache` revalidation. |
| `/offline/pack.<id>.json` | Offline pack. Hashed name, default cache mode, per-address budget. |

`/offline/` is disallowed in `robots.txt`. Packs are per-device downloads, not crawl targets.

## Limits (per address)

- 20 pack downloads a day.
- Burst of 5 per minute.
- 100 MB of pack bytes a day.

Over any limit the server returns `429` with `Retry-After` and `Cache-Control: no-store`.
Counters live in the web process, so a restart resets the daily allowance. Addresses come
from `cf-connecting-ip` only when `x-eq-ingress-marker` matches `INGRESS_EDGE_MARKER`.
Otherwise the socket peer is used.

## Integrity

Quran bytes are served from the app's baked id maps. Bulk files publish no checksums, and
the service does not verify one at runtime. SHA-256 is a manual audit step only, run by the
owner (`just quran-audit`, `bun run audit:arabic`, `bun run verify`).

## Owner actions

- Cloudflare Cache Rule for `/offline/*` is not set (O14). Until it is, every pack download reaches origin.
- Translated-reader SSR ceilings (`WEB_SSR_*`) stay off until measured (O4).
