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


def code_differences(local, reference):
    return [{"kind": kind, "local_index": start, "reference_index": ref_start,
             "local_codes": [f"U+{ord(c):04X}" for c in local[start:end]],
             "reference_codes": [f"U+{ord(c):04X}" for c in reference[ref_start:ref_end]]}
            for kind, start, end, ref_start, ref_end in
            difflib.SequenceMatcher(None, local, reference, autojunk=False).get_opcodes()
            if kind != "equal"]


def letters(text):
    return "".join(c for c in text if unicodedata.category(c) == "Lo")


SERVED_ENCODING = {0xE003: "\u0656", 0xE004: "\u0657", 0xE01A: "\u0617",
                   0xE01B: "\u06EA", 0xE01C: "\u06D7", 0xE01E: "\u06EB",
                   0xE01F: "\u06E5", 0xE021: "\u06EC", 0xE022: "\u06E0"}
SERVED_SIGNS = set(SERVED_ENCODING.values()) | {chr(c) for c in range(0x06D6, 0x06DD)} | {"\u0614", "\u0615"}


def encoded_signs(text, legacy=False):
    encoded = "".join(SERVED_ENCODING.get(ord(c), c) for c in text) if legacy else text
    return "".join(c for c in encoded if c in SERVED_SIGNS)


def boundary_offsets(words):
    offset = 0
    result = set()
    for word in words[:-1]:
        offset += len(word)
        result.add(offset)
    return result


def difference_causes(differences):
    codes = {code for difference in differences
             for side in ["local_codes", "reference_codes"] for code in difference[side]}
    causes = []
    if "U+06DE" in codes:
        causes.append("reference_rub_el_hizb")
    if "U+E022" in codes:
        causes.append("ruku_attachment_or_presence")
    if "U+E021" in codes:
        causes.append("optional_ayah_attachment_or_order")
    if any(unicodedata.category(chr(int(code[2:], 16))).startswith("L") for code in codes):
        causes.append("letter_difference")
    if any(code.startswith("U+E0") for code in codes):
        causes.append("private_sign_difference")
    if any(unicodedata.category(chr(int(code[2:], 16))).startswith("M") for code in codes):
        causes.append("mark_or_standard_pause_difference")
    return causes


def compare_observed_pages(verses, observed_reference):
    observed = json.loads((observed_reference / "verses.json").read_text())
    provenance = json.loads((observed_reference / "live-report.json").read_text())
    current = {verse["key"]: verse for verse in verses}
    fields = ("location", "legacy", "display", "type", "css")
    keys = [verse["key"] for verse in observed]
    if not keys or len(keys) != len(set(keys)):
        raise ValueError("Observed pages must contain unique verse keys")
    mismatches = []
    for verse in observed:
        key = verse["key"]
        if key not in current:
            mismatches.append(key)
            continue
        first = [[word.get(field, "") for field in fields] for word in verse["words"]]
        second = [[word.get(field, "") for field in fields] for word in current[key]["words"]]
        if first != second:
            mismatches.append(key)
    if mismatches or provenance["failures"] or provenance["pages_checked"] != len(keys):
        raise ValueError(f"Current proxy differs from observed website pages: {mismatches}")
    return {"pages_checked": len(keys), "keys": keys, "fields": list(fields),
            "mismatches": mismatches, "provenance": provenance}


def compare_full(reference, segmentation, observed_reference=None):
    originals = {f"{sura}:{aya}": text for sura, aya, text in read_verses(DATABASE)}
    verses = json.loads((reference / "verses.json").read_text())
    provenance = json.loads((reference / "api-report.json").read_text())
    boxes = {verse["key"]: verse["boxes"] for verse in json.loads(segmentation.read_text())["verses"]}
    known = {"4:142", "12:1", "2:10", "7:206", "2:219", "2:233", "2:243", "3:171", "9:111", "13:5"}
    keys = [verse["key"] for verse in verses]
    if len(keys) != 6236 or len(set(keys)) != 6236 or set(keys) != set(originals):
        raise ValueError("Reference must cover all 6,236 unique verse keys")
    rows = []
    for verse in verses:
        key = verse["key"]
        local = originals[key]
        words = [word for word in verse["words"] if word["type"] == "word"]
        legacy = " ".join(word["legacy"] for word in words)
        display = " ".join(word["display"] for word in words)
        served = " ".join(word["display"] for word in verse["words"])
        encoded_local, encoded_served = encoded_signs(local, True), encoded_signs(served)
        served_class = "identical_encoding_sequence"
        if encoded_local != encoded_served:
            served_class = "served_sign_pipeline_difference"
            if Counter(encoded_local) == Counter(encoded_served):
                served_class = "served_encoding_order_only"
        served_differences = code_differences(encoded_local, encoded_served)
        local_private = private_sequence(local)
        private_match = local_private == private_sequence(legacy)
        first, second = comparison_text(local), comparison_text(legacy)
        verse_legacy = comparison_text(verse["legacy"])
        differences = code_differences(first, second)
        classification = "identical"
        if differences:
            classification = "new_source_difference"
            if key in known:
                classification = "previously_recorded_source_difference"
            if first == verse_legacy:
                classification = "word_pipeline_difference"
        local_words = [letters(box["text"]) for box in boxes[key] if letters(box["text"])]
        reference_words = [letters(word["legacy"]) for word in words if letters(word["legacy"])]
        boundary_class = "identical"
        if local_words != reference_words:
            boundary_class = "source_letters_differ"
            if "".join(local_words) == "".join(reference_words):
                local_offsets = boundary_offsets(local_words)
                reference_offsets = boundary_offsets(reference_words)
                boundary_class = "different_source_word_boundaries"
                if local_offsets < reference_offsets:
                    boundary_class = "source_joined_clusters"
                if reference_offsets < local_offsets:
                    boundary_class = "reference_joined_clusters"
        rows.append({"key": key, "private_sequence_match": private_match,
                     "legacy_verse_match": first == verse_legacy,
                     "private_occurrences": len(local_private), "legacy_classification": classification,
                     "legacy_differences": differences, "legacy_difference_causes": difference_causes(differences),
                     "api_display_matches_legacy": display == legacy,
                     "served_encoding_classification": served_class,
                     "served_encoding_differences": served_differences,
                     "served_encoding_local_codes": [f"U+{ord(c):04X}" for c in encoded_local],
                     "served_encoding_reference_codes": [f"U+{ord(c):04X}" for c in encoded_served],
                     "word_boxes": len(local_words), "reference_words": len(reference_words),
                     "word_boundary_classification": boundary_class,
                     "word_boundary_evidence": {
                         "local_letters": local_words, "reference_letters": reference_words,
                         "local_only_break_offsets": sorted(boundary_offsets(local_words) - boundary_offsets(reference_words)),
                         "reference_only_break_offsets": sorted(boundary_offsets(reference_words) - boundary_offsets(local_words)),
                         "letters_differences": code_differences("".join(local_words), "".join(reference_words))
                     } if boundary_class != "identical" else None})
    report = {"database_id": "quran-indopak", "provenance": provenance,
              "verses_checked": len(rows), "private_occurrences_checked": sum(row["private_occurrences"] for row in rows),
              "private_mismatches": [row["key"] for row in rows if not row["private_sequence_match"]],
              "legacy_verse_mismatches": [row["key"] for row in rows if not row["legacy_verse_match"]],
              "source_classifications": dict(Counter(row["legacy_classification"] for row in rows)),
              "served_encoding_classifications": dict(Counter(row["served_encoding_classification"] for row in rows)),
              "served_encoding_table": {f"U+{code:04X}": f"U+{ord(value):04X}" for code, value in SERVED_ENCODING.items()},
              "served_encoding_table_scope": "Observed font-specific rendering assignments, not Unicode equivalence; ordinary shared aliases included in sequence comparisons",
              "served_legacy_differences_are_source_review_items": True,
              "word_boundary_classifications": dict(Counter(row["word_boundary_classification"] for row in rows)),
              "new_source_differences": [row for row in rows if row["legacy_classification"] == "new_source_difference"],
              "reference_contains_current_served_words": "Current website word.text" in provenance.get("served_text_pipeline", ""),
              "comparison_only_whitespace_format_removal_and_letters_filter": True,
              "database_untouched": True, "results": rows}
    if observed_reference:
        report["observed_website_pages"] = compare_observed_pages(verses, observed_reference)
    return report


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
    parser.add_argument("--segmentation", type=Path)
    parser.add_argument("--observed-reference", type=Path)
    args = parser.parse_args()
    if args.segmentation:
        report = compare_full(args.reference, args.segmentation, args.observed_reference)
    else:
        if args.observed_reference:
            parser.error("--observed-reference requires --segmentation")
        report = compare(args.reference)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items()
                      if key not in {"legacy_differences", "served_pipeline_examples", "results", "provenance", "new_source_differences"}}, indent=2))


if __name__ == "__main__":
    main()
