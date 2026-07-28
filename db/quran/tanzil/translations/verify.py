#!/usr/bin/env python3
"""
verify.py — integrity checks for the mirrored tanzil translations.

Confirms every translation in index.json has a downloaded SQL dump, that each
dump is valid UTF-8 with a sane verse count, and that the recorded checksums
and metadata are consistent. Exits non-zero on any hard problem.
"""

from __future__ import annotations

import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INDEX = ROOT / "index.json"
MIN_AYAS = 6000  # every full-Quran translation should be near 6236


def main() -> int:
    errors, warnings = [], []
    index = json.loads(INDEX.read_text(encoding="utf-8"))
    trans = index["translations"]
    print(f"index.json: {len(trans)} translations (header count={index['count']})")
    if len(trans) != index["count"]:
        errors.append("index count field != actual entries")

    aya_dist = Counter()
    bad_encoding, bad_sha, missing, short = [], [], [], []

    for t in trans:
        tid = t["id"]
        rel = t["file"]["sql"]
        p = ROOT / rel
        if not p.exists():
            missing.append(tid)
            continue
        raw = p.read_bytes()
        # utf-8 check
        try:
            raw.decode("utf-8")
        except UnicodeDecodeError as e:
            bad_encoding.append(f"{tid}: {e}")
        # checksum check
        if hashlib.sha256(raw).hexdigest() != t["file"]["sha256"]:
            bad_sha.append(tid)
        ac = t.get("ayaCount")
        aya_dist[ac] += 1
        if ac is not None and ac < MIN_AYAS:
            short.append(f"{tid} ({ac})")
        # required display fields present
        for f in ("language", "name", "translator", "direction", "languageCode"):
            if not t.get(f):
                warnings.append(f"{tid}: missing {f}")

    print(f"\nfiles present: {len(trans) - len(missing)}/{len(trans)}")
    print(f"aya-count distribution: {dict(sorted(aya_dist.items(), key=lambda kv: (kv[0] is None, kv[0])))}")
    if short:
        warnings.append(f"translations with < {MIN_AYAS} ayas: " + ", ".join(short))

    for label, items in (("MISSING", missing), ("BAD ENCODING", bad_encoding),
                         ("BAD SHA256", bad_sha)):
        if items:
            errors.append(f"{label}: {items}")

    # group by language for a quick human overview
    by_lang = Counter(t["language"] for t in trans)
    print(f"\nlanguages: {len(by_lang)}")
    for lang, n in by_lang.most_common():
        print(f"  {lang:14} {n}")

    rtl = [t["language"] for t in trans if t["direction"] == "rtl"]
    print(f"\nrtl languages: {sorted(set(rtl))}")

    print("\nWARNINGS:" + ("" if not warnings else ""))
    for w in warnings:
        print(f"  ! {w}")
    print("ERRORS:" + ("" if not errors else ""))
    for e in errors:
        print(f"  ✗ {e}")

    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
