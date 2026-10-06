import argparse
from bisect import bisect_right
from collections import Counter
import json
from pathlib import Path


def summarize(report, source_rows):
    rows = report["rows"]
    comparable = [row for row in rows if row["comparable"]]
    shifts = []
    final_wraps = []
    investigations = []
    for row in comparable:
        reference = row["reference"]
        ours = row["ours"]
        reference_words = [word for word in reference["words"] if word["type"] == "word"]
        boundaries = [word["offset"] for word in reference_words]
        source = source_rows[row["key"]]
        for index, (local, remote) in enumerate(zip(ours["line_starts"], reference["line_starts"])):
            local_word = max(0, bisect_right(boundaries, local["offset"]) - 1)
            remote_word = max(0, bisect_right(boundaries, remote["offset"]) - 1)
            shift = local_word - remote_word
            if abs(shift) > 1:
                shifts.append({"key": row["key"], "viewport": row["viewport"], "line": index + 1,
                               "reference_word_shift": shift, "local_offset": local["offset"],
                               "reference_offset": remote["offset"]})
        if (not reference["marker_orphaned"] and len(ours["line_starts"]) > len(reference["line_starts"])):
            final_wraps.append({"key": row["key"], "viewport": row["viewport"],
                                "line_count_difference": row["line_count_difference"],
                                "local_final_cluster_intact": ours["final_word_with_marker"],
                                "source_boundary_classification": source["word_boundary_classification"]})
        if abs(row["line_count_difference"]) >= 2:
            tops = []
            for word in reference_words:
                if not tops or abs(word["top"] - tops[-1]) > 2:
                    tops.append(word["top"])
            word_difference = len(ours["line_starts"]) - len(tops)
            investigations.append({
                "key": row["key"], "viewport": row["viewport"], "column_px": reference["width"],
                "size_px": reference["size"], "raw_line_difference": row["line_count_difference"],
                "word_line_difference_excluding_reference_marker_only_line": word_difference,
                "reference_marker_orphaned": reference["marker_orphaned"],
                "source_boundary_classification": source["word_boundary_classification"],
                "source_boxes": source["word_boxes"], "reference_words": source["reference_words"],
                "local_final_cluster_intact": ours["final_word_with_marker"],
                "local_line_starts": ours["line_starts"], "reference_line_starts": reference["line_starts"],
                "assessment": "Tracked font/spacing comparison; exact source and column asserted by capture runner. Reference marker-only lines inflate raw count where recorded. Joined clusters retain documented source boundaries. No data edits or tolerance changes.",
            })
    return {
        "engine": report["engine"], "version": report.get("version"), "status": report["status"],
        "states": len(rows), "comparable_states": len(comparable),
        "identical_break_percent": 100 * sum(row["identical_breaks"] for row in comparable) / len(comparable),
        "mean_line_count_difference": sum(row["line_count_difference"] for row in comparable) / len(comparable),
        "line_count_distribution": dict(Counter(row["line_count_difference"] for row in comparable)),
        "shifts_beyond_one_reference_word": shifts, "additional_final_cluster_lines": final_wraps,
        "two_or_more_line_investigations": investigations,
        "unavailable_viewports": report.get("unavailable_viewports", []),
        "gating": False,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--review", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    args = parser.parse_args()
    reference = json.loads(args.reference.read_text())
    source_rows = {row["key"]: row for row in reference["results"]}
    summaries = []
    for engine in ["chromium", "webkit", "safari"]:
        filename = args.review / f"{engine}-layout-report.json"
        if filename.exists():
            report = json.loads(filename.read_text())
            if report["rows"]:
                summaries.append(summarize(report, source_rows))
    output = {"comparison_only": True, "engines": summaries, "owner_visual_scores": "pending"}
    (args.review / "line-parity-summary.json").write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps([{key: value for key, value in report.items() if not isinstance(value, list)} for report in summaries]))


if __name__ == "__main__":
    main()
