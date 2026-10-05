import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter, ImageOps

from audit import ROOT


def ink(path):
    with Image.open(path) as image:
        return image.convert("L").point(lambda value: 255 if value < 128 else 0)


def count(image):
    return image.histogram()[255]


def analyze(output, engine):
    records = json.loads((output / f"{engine}-dom-images.json").read_text())
    results = []
    originals_checked = 0
    for record in records:
        images = record["images"]
        actual = ink(output / images["Actual"])
        private = ink(output / images[record["code"]])
        ordinary = ink(output / images[f"Except {record['code']}"])
        original_mismatch = 0
        if "Original" in images:
            original = ink(output / images["Original"])
            if original.size != actual.size:
                raise ValueError(f"Diagnostic changed original layout: {record['key']}")
            first = ImageChops.multiply(original, ImageOps.invert(actual.filter(ImageFilter.MaxFilter(5))))
            second = ImageChops.multiply(actual, ImageOps.invert(original.filter(ImageFilter.MaxFilter(5))))
            original_mismatch = count(first) + count(second)
            originals_checked += 1
        if actual.size != private.size or actual.size != ordinary.size:
            raise ValueError(f"DOM layout changed in diagnostic {record['key']}")
        reconstruction = ImageChops.lighter(private, ordinary)
        mismatch = ImageChops.difference(actual, reconstruction)
        outside = ImageChops.multiply(mismatch, ImageOps.invert(private.filter(ImageFilter.MaxFilter(5))))
        bounds = private.getbbox()
        boundary = bounds and min(bounds[0], bounds[1], private.width - bounds[2], private.height - bounds[3])
        results.append({"key": record["key"], "code": record["code"],
                        "private_pixels": count(private),
                        "boundary_clearance_px": boundary,
                        "original_geometry_mismatch": original_mismatch,
                        "overlap_pixels": count(ImageChops.multiply(private, ordinary)),
                        "outside_mask_mismatch": count(outside)})
    report = {"engine": engine, "cases": len(results), "original_font_comparisons": originals_checked,
              "original_raster_edge_tolerance_px": 2,
              "invisible": [row for row in results if row["private_pixels"] == 0],
              "geometry_mismatch": [row for row in results if row["outside_mask_mismatch"] > 0 or row["original_geometry_mismatch"] > 0],
              "boundary_candidates": [row for row in results if row["boundary_clearance_px"] is not None and row["boundary_clearance_px"] < 2],
              "overlap_candidates": [row for row in results if row["overlap_pixels"] >= 3],
              "results": results}
    (output / f"{engine}-dom-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: len(value) if isinstance(value, list) else value
                      for key, value in report.items() if key != "results"}, indent=2))
    if report["invisible"] or report["geometry_mismatch"]:
        raise ValueError("invalid DOM ink diagnostic; do not interpret overlap candidates")
    if report["overlap_candidates"] or report["boundary_candidates"]:
        raise ValueError("Private ink needs visual review before approval; inspect original DOM captures")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/indopak-deep")
    parser.add_argument("--engine", choices=["chromium", "webkit", "chromium-flow", "webkit-flow"], required=True)
    args = parser.parse_args()
    analyze(args.output, args.engine)


if __name__ == "__main__":
    main()
