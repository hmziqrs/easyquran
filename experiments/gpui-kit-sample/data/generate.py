#!/usr/bin/env python3
"""Regenerate data/quran-sample.json from the local immutable Quran DBs.

Read-only against the DBs. Urdu text is NFKC-normalized because the
translation source encodes lam-alef as the isolated presentation form
U+FEFB, which every text stack renders as a broken mid-word ligature.
"""
import json
import sqlite3
import unicodedata
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
AR = REPO / "db/quran/arabic"
UR = REPO / "db/quran/translations/sqlite/ur.junagarhi.sqlite"
OUT = Path(__file__).resolve().parent / "quran-sample.json"

NAMES = {1: ("Al-Fatihah", "الفاتحة"), 2: ("Al-Baqarah", "البقرة")}
DATASETS = {
    "uthmani": AR / "quran-uthmani.sqlite",
    "simple": AR / "quran-simple-clean.sqlite",
    "indopak": AR / "quran-indopak.sqlite",
}


def rows(db, surah, limit=None):
    q = "SELECT aya, text FROM quran_text WHERE sura=? ORDER BY aya"
    if limit:
        q += f" LIMIT {limit}"
    con = sqlite3.connect(db)
    return {a: t for a, t in con.execute(q, (surah,))}


def main():
    urdu = {n: rows(UR, n, None if n == 1 else 10) for n in (1, 2)}
    arabic = {}
    for ds, db in DATASETS.items():
        surahs = []
        for num, limit in ((1, None), (2, 10)):
            a = rows(db, num, limit)
            en, ar = NAMES[num]
            surahs.append({
                "number": num,
                "name_en": en,
                "name_ar": ar,
                "ayahs": [{"n": n, "text": a[n]} for n in sorted(a)],
            })
        arabic[ds] = {"surahs": surahs}
    out = {
        "translations": {
            "ur": {
                str(n): [
                    {"n": k, "text": unicodedata.normalize("NFKC", t)}
                    for k, t in sorted(v.items())
                ]
                for n, v in urdu.items()
            }
        },
        "arabic": arabic,
    }
    OUT.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"wrote {OUT} ({sum(len(s['ayahs']) for d in arabic.values() for s in d['surahs'])} ayahs)")


if __name__ == "__main__":
    main()
