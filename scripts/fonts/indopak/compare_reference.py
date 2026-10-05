import argparse
from collections import Counter
import difflib
import json
from pathlib import Path
import unicodedata

from audit import DATABASE, HERE, ROOT, is_private, read_verses


def comparison_text(text):
    return "".join(character for character in text
                   if not character.isspace() and unicodedata.category(character) != "Cf")


def private_sequence(text):
    return [f"U+{ord(character):04X}" for character in text if is_private(ord(character))]


def compare(reference):
    originals = {f"{sura}:{aya}": text for sura, aya, text in read_verses(DATABASE)}
    verses = json.loads((reference / "verses.json").read_text())
    live = json.loads((reference / "live-report.json").read_text())
    manifest = json.loads((HERE / "mapping.json").read_text())
    mismatches = []
    private_mismatches = []
    checked_private = Counter()
    keys = set()
    for verse in verses:
        key = verse["key"]
        if key in keys:
            raise ValueError(f"duplicate reference verse: {key}")
        keys.add(key)
        local = originals[key]
        legacy = " ".join(word["legacy"] for word in verse["words"] if word["type"] == "word")
        local_private = private_sequence(local)
        if local_private != private_sequence(legacy):
            private_mismatches.append(key)
        checked_private.update(local_private)
        first = comparison_text(local)
        second = comparison_text(legacy)
        if first == second:
            continue
        differences = []
        for kind, start, end, ref_start, ref_end in difflib.SequenceMatcher(None, first, second, autojunk=False).get_opcodes():
            if kind != "equal":
                differences.append({"kind": kind, "local_index": start,
                                    "local_codes": [f"U+{ord(c):04X}" for c in first[start:end]],
                                    "reference_index": ref_start,
                                    "reference_codes": [f"U+{ord(c):04X}" for c in second[ref_start:ref_end]]})
        mismatches.append({"key": key, "differences": differences})
    contexts = [context for entry in manifest["entries"] for context in entry["contexts"]]
    missing_contexts = [context["signature"] for context in contexts if context["verse_key"] not in keys]
    if private_mismatches or missing_contexts or live["failures"]:
        raise ValueError("reference audit incomplete or private-code sequences disagree")
    examples = []
    for key in ["16:6", "73:17", "79:27", "51:54", "26:51", "43:15"]:
        verse = next(item for item in verses if item["key"] == key)
        last = [word for word in verse["words"] if word["type"] == "word"][-1]
        ornament = next(word for word in verse["words"] if word["type"] == "end")
        examples.append({"key": key,
                         "legacy_final_word_private": private_sequence(last["legacy"]),
                         "served_final_word_codes": [f"U+{ord(c):04X}" for c in last["display"] if unicodedata.category(c) in {"Co", "Mn"}],
                         "served_ornament_codes": [f"U+{ord(c):04X}" for c in ornament["display"]]})
    return {"database_id": "quran-indopak", "observed_at": live["observed_at"],
            "font_style_observed_at": live.get("font_style_observed_at", live["observed_at"]),
            "pages_checked": len(verses), "reference_build_ids": live["build_ids"],
            "font_url": live["font_url"], "font_bytes": live["font_bytes"],
            "observed_word": live["observed"]["word"],
            "context_classes_checked": len(contexts), "private_occurrences_checked": sum(checked_private.values()),
            "private_counts": dict(sorted(checked_private.items())),
            "private_code_sequences_identical": True,
            "comparison_only_whitespace_and_format_removal": True,
            "legacy_strings_matching_after_comparison": len(verses) - len(mismatches),
            "legacy_differences": mismatches, "served_pipeline_examples": examples,
            "limitations": [f"{len(verses)} verse pages, not a full Quran.com corpus comparison",
                            "Legacy textIndopak field differs from current served word.text pipeline",
                            "Source differences require editorial review; no DB edits or reader normalization",
                            "No restricted reference font assets copied into repository or build"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", type=Path, default=Path("/tmp/easyquran-deep-reference"))
    parser.add_argument("--report", type=Path, default=ROOT / ".cache/indopak-deep/reference-report.json")
    args = parser.parse_args()
    report = compare(args.reference)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items()
                      if key not in {"legacy_differences", "served_pipeline_examples"}}, indent=2))


if __name__ == "__main__":
    main()
