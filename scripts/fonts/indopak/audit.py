import argparse
from collections import Counter
import json
from pathlib import Path
import sqlite3
import unicodedata

from fontTools.ttLib import TTFont


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
DATABASE = ROOT / "db/quran/arabic/quran-indopak.sqlite"


def is_private(codepoint):
    return (0xE000 <= codepoint <= 0xF8FF or
            0xF0000 <= codepoint <= 0xFFFFD or
            0x100000 <= codepoint <= 0x10FFFD)


def read_verses(path):
    with sqlite3.connect(path.resolve().as_uri() + "?mode=ro&immutable=1", uri=True) as database:
        return database.execute('SELECT sura, aya, text FROM quran_text ORDER BY "index"').fetchall()


def context_signature(text, index):
    previous = text[index - 1] if index else ""
    following = text[index + 1] if index + 1 < len(text) else ""
    start = index
    while start > 0 and unicodedata.category(text[start - 1]).startswith("M"):
        start -= 1
    end = index + 1
    while end < len(text) and unicodedata.category(text[end]).startswith("M"):
        end += 1
    before_marks = "".join(f"{ord(char):04X}," for char in text[start:index])
    after_marks = "".join(f"{ord(char):04X}," for char in text[index + 1:end])
    private = "".join(f"{ord(char):04X}," for char in text[max(0, index - 2):index + 3]
                      if is_private(ord(char)))
    return "|".join([unicodedata.category(previous) if previous else "start",
                     unicodedata.category(following) if following else "end",
                     before_marks, after_marks, private])


def inventory(rows):
    counts = Counter(char for _, _, text in rows for char in text)
    items = []
    for char, count in sorted(counts.items()):
        codepoint = ord(char)
        locations = [f"{sura}:{aya}" for sura, aya, text in rows if char in text]
        fallback = "PRIVATE USE" if is_private(codepoint) else "UNNAMED"
        item = {"codepoint": f"U+{codepoint:04X}", "count": count,
                "name": unicodedata.name(char, fallback),
                "category": unicodedata.category(char),
                "combining": unicodedata.combining(char),
                "bidi": unicodedata.bidirectional(char),
                "representative_verse_keys": locations[:4]}
        if is_private(codepoint):
            contexts = {}
            for sura, aya, text in rows:
                for index, current in enumerate(text):
                    if current != char:
                        continue
                    signature = context_signature(text, index)
                    if signature not in contexts:
                        contexts[signature] = {"signature": signature,
                                               "verse_key": f"{sura}:{aya}",
                                               "character_index": index, "count": 0}
                    contexts[signature]["count"] += 1
            item["contexts"] = list(contexts.values())
        items.append(item)
    private_counts = [count for char, count in counts.items() if is_private(ord(char))]
    return {"database_id": "quran-indopak", "source_profile_id": "indopak-naveed-7d3c21e0",
            "verse_count": len(rows), "character_count": sum(counts.values()),
            "distinct_codepoints": len(items), "private_codepoint_count": len(private_counts),
            "private_occurrences": sum(private_counts), "codepoints": items}


def missing_coverage(path, corpus):
    with TTFont(path) as font:
        cmap = font.getBestCmap()
        visible = []
        controls = []
        for item in corpus["codepoints"]:
            if cmap.get(int(item["codepoint"][2:], 16)) not in {None, ".notdef"}:
                continue
            if item["category"] == "Cf":
                controls.append(item["codepoint"])
            else:
                visible.append({"codepoint": item["codepoint"], "name": item["name"],
                                "count": item["count"]})
        return {"font": str(path), "missing_visible": visible, "unmapped_controls": controls}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--database", type=Path, default=DATABASE)
    parser.add_argument("--write-inventory", type=Path)
    parser.add_argument("--font", type=Path, action="append", default=[])
    args = parser.parse_args()
    corpus = inventory(read_verses(args.database))
    if args.write_inventory:
        args.write_inventory.write_text(json.dumps(corpus, ensure_ascii=False, indent=2) + "\n",
                                        encoding="utf-8")
    else:
        expected = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
        if corpus != expected:
            raise ValueError("corpus inventory changed; review data identity, never modify the DB")
    print(json.dumps({key: value for key, value in corpus.items() if key != "codepoints"}))
    failed = False
    for path in args.font:
        report = missing_coverage(path, corpus)
        print(json.dumps(report, ensure_ascii=False))
        failed = failed or bool(report["missing_visible"])
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
