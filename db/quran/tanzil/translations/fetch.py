#!/usr/bin/env python3
"""
fetch.py — mirror every Quran translation listed on https://tanzil.net/trans/

For each translation it stores the MySQL-dump (SQL) form — the same format the
Arabic text already uses under db/quran/tanzil/arabic/sql — and emits an
index.json describing every translation (language, name, translator, direction,
last update, verse count, file checksums, source URLs) so the dataset is easy
to consume directly.

  python3 fetch.py            # download missing files + (re)build index.json
  python3 fetch.py --force    # re-download every file
  python3 fetch.py --limit 5  # operate on the first 5 translations (smoke test)

The page at /trans/ is the source of truth for the id list and the display
fields (language / name / translator); each SQL dump's header is the source of
truth for `lastUpdate` and is cross-checked against the page. Downloads are
concurrent with retries and a polite delay.
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import hashlib
import html as H
import json
import re
import sys
import time
import urllib.request
from pathlib import Path

BASE = "https://tanzil.net/trans"
ROOT = Path(__file__).resolve().parent
SQLDIR = ROOT / "sql"
INDEX = ROOT / "index.json"
PAGE_CACHE = ROOT / ".trans_page.html"

# Language codes (the prefix of a tanzil id) whose script is right-to-left.
RTL_CODES = {"ar", "fa", "ur", "ps", "sd", "ug", "ku", "dv"}
UA = "easyquran-data/1.0 (+https://easyquran.app; tanzil mirror)"


def log(msg: str) -> None:
    print(msg, flush=True)


# ---------------------------------------------------------------------------
# 1. discover the translation list from the /trans/ page
# ---------------------------------------------------------------------------

def fetch_page() -> str:
    if PAGE_CACHE.exists():
        return PAGE_CACHE.read_text(encoding="utf-8")
    req = urllib.request.Request(f"{BASE}/", headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read().decode("utf-8")
    PAGE_CACHE.write_text(data, encoding="utf-8")
    return data


def parse_page(html: str) -> list[dict]:
    """Return [{id, language, name, translator}, ...] in page order."""
    rows = re.findall(r"<tr[^>]*>.*?</tr>", html, re.S)

    def strip(t: str) -> str:
        return H.unescape(re.sub(r"<[^>]+>", "", t)).strip()

    out, seen = [], set()
    for r in rows:
        cells = re.split(r"<td[^>]*>", r)[1:]  # drop the "<tr>" prefix
        if len(cells) < 4:
            continue
        language, name, translator = (strip(c) for c in cells[:3])
        m = re.search(r'href="/trans/([a-z]{2,4}\.[a-z0-9-]+)"', cells[3])
        if not m:
            continue
        tid = m.group(1)
        if tid in seen:
            continue
        seen.add(tid)
        # translator cells carry "*" / "†" markers for biography links — drop them
        translator = re.sub(r"\s*[*†]+\s*", "", translator).strip()
        name = re.sub(r"\s*[*†]+\s*$", "", name).strip()
        out.append({"id": tid, "language": language, "name": name, "translator": translator})
    return out


# ---------------------------------------------------------------------------
# 2. download one SQL dump (with retries)
# ---------------------------------------------------------------------------

def download_one(item: dict, force: bool) -> tuple[str, str]:
    tid = item["id"]
    dest = SQLDIR / f"{tid}.sql"
    if dest.exists() and not force and dest.stat().st_size > 1000:
        return tid, "cached"
    url = f"{BASE}/{tid}?type=sql"
    last = ""
    for attempt in range(1, 5):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=90) as r:
                data = r.read()
            head = data[:400].lower()
            if len(data) < 1000 or b"file not found" in data[:400] or b"<!doctype html" in head:
                raise RuntimeError(f"bad response ({len(data)} bytes)")
            dest.write_bytes(data)
            time.sleep(0.2)  # be polite
            return tid, "ok"
        except Exception as e:  # noqa: BLE001
            last = f"{type(e).__name__}: {e}"
            time.sleep(1.5 * attempt)
    return tid, f"FAIL: {last}"


def download_all(items: list[dict], force: bool) -> dict[str, str]:
    SQLDIR.mkdir(parents=True, exist_ok=True)
    results: dict[str, str] = {}
    with cf.ThreadPoolExecutor(max_workers=6) as ex:
        futs = {ex.submit(download_one, it, force): it["id"] for it in items}
        done = 0
        for fut in cf.as_completed(futs):
            tid, status = fut.result()
            results[tid] = status
            done += 1
            flag = "✓" if status in ("ok", "cached") else "✗"
            if done % 15 == 0 or status.startswith("FAIL"):
                log(f"  [{done}/{len(items)}] {flag} {tid} — {status}")
    return results


# ---------------------------------------------------------------------------
# 3. parse a downloaded SQL dump
# ---------------------------------------------------------------------------

HEADER_RE = re.compile(r"^#\s*(Name|Translator|Language|ID|Last Update|Source)\s*:\s*(.+)$")
ROW_RE = re.compile(r"\(\d+,\s*\d+,\s*\d+,\s*'")


def parse_sql(path: Path) -> dict:
    raw = path.read_bytes()
    text = raw.decode("utf-8")  # raises if not valid utf-8 — that's what we want
    hdr: dict[str, str] = {}
    for line in text.splitlines():
        m = HEADER_RE.match(line)
        if m:
            hdr[m.group(1)] = m.group(2).strip()
    aya_count = len(ROW_RE.findall(text))
    return {
        "header": hdr,
        "ayaCount": aya_count,
        "sizeBytes": len(raw),
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


def build_index(items: list[dict]) -> dict:
    translations = []
    for it in items:
        tid = it["id"]
        code = tid.split(".", 1)[0]
        sql_path = SQLDIR / f"{tid}.sql"
        parsed = parse_sql(sql_path) if sql_path.exists() else None
        hdr = parsed["header"] if parsed else {}

        # page = display source of truth; SQL header cross-checks language
        page_lang = it["language"]
        sql_lang = hdr.get("Language", "")
        mismatch = bool(sql_lang and page_lang and sql_lang.lower() != page_lang.lower())

        rec = {
            "id": tid,
            "language": page_lang,
            "languageCode": code,
            "direction": "rtl" if code in RTL_CODES else "ltr",
            "name": hdr.get("Name") or it["name"],
            "nameNative": it["name"] if it["name"] != hdr.get("Name") else None,
            "translator": hdr.get("Translator") or it["translator"],
            "lastUpdate": hdr.get("Last Update"),
            "ayaCount": parsed["ayaCount"] if parsed else None,
            "file": {
                "sql": f"sql/{tid}.sql",
                "sizeBytes": parsed["sizeBytes"] if parsed else None,
                "sha256": parsed["sha256"] if parsed else None,
            },
            "urls": {
                "tanzil": f"{BASE}/{tid}",
                "download": f"{BASE}/{tid}?type=sql",
                "browse": f"https://tanzil.net/#trans/{tid}/1:1",
                "changelog": f"{BASE}/log/{tid}",
            },
        }
        rec = {k: v for k, v in rec.items() if v is not None}
        if mismatch:
            rec["languageMismatch"] = {"page": page_lang, "sql": sql_lang}
        translations.append(rec)
    return {
        "source": "Tanzil.net",
        "sourceUrl": f"{BASE}/",
        "format": "sql (MySQL phpMyAdmin dump)",
        "license": "Tanzil Terms of Use — non-commercial use; attribution + backlink to tanzil.net required when redistributing more than three translations.",
        "note": "name = SQL-header name (authoritative); nameNative = display name from the /trans/ page (often the original script).",
        "count": len(translations),
        "translations": translations,
    }


# ---------------------------------------------------------------------------

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--force", action="store_true", help="re-download even if cached")
    ap.add_argument("--limit", type=int, default=0, help="only process the first N translations")
    ap.add_argument("--no-index", action="store_true", help="skip writing index.json")
    args = ap.parse_args()

    log("→ fetching translation list from tanzil.net/trans/ ...")
    items = parse_page(fetch_page())
    log(f"  discovered {len(items)} translations")
    if args.limit:
        items = items[: args.limit]
        log(f"  --limit: processing {len(items)}")

    log("→ downloading SQL dumps ...")
    results = download_all(items, args.force)
    failed = {t: s for t, s in results.items() if s.startswith("FAIL")}
    cached = sum(1 for s in results.values() if s == "cached")
    ok = sum(1 for s in results.values() if s == "ok")
    log(f"  ok={ok} cached={cached} failed={len(failed)}")
    if failed:
        for t, s in failed.items():
            log(f"  FAIL {t}: {s}")

    if args.no_index:
        return 1 if failed else 0

    log("→ building index.json ...")
    # always build the index across the FULL discovered set when not limited
    all_items = items if not args.limit else parse_page(fetch_page())
    index = build_index(all_items)
    INDEX.write_text(json.dumps(index, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    log(f"  wrote {INDEX} ({index['count']} translations)")

    # quick sanity tally
    counts = [t["ayaCount"] for t in index["translations"] if t.get("ayaCount")]
    if counts:
        log(f"  aya counts: min={min(counts)} max={max(counts)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
