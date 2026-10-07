import argparse
import json
import math
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter, ImageOps

from audit import ROOT
from native_capture import stitch_native_capture

EDGE_TOLERANCE_CSS_PX = 2


def ink(path, box=None):
    with Image.open(path) as image:
        if box:
            image = image.crop(box)
        return image.convert("L").point(lambda value: 255 if value < 128 else 0)


def fill_edges(path):
    with Image.open(path) as image:
        rgba = image.convert("RGBA")
    width, height = rgba.size

    def black(box):
        return rgba.crop(box).getcolors(1) == [(abs(box[2] - box[0]) * abs(box[3] - box[1]), (0, 0, 0, 255))]

    top = 0
    while top < height and black((0, top, width, top + 1)):
        top += 1
    bottom = height
    while bottom > top and black((0, bottom - 1, width, bottom)):
        bottom -= 1
    left = 0
    while left < width and black((left, top, left + 1, bottom)):
        left += 1
    right = width
    while right > left and black((right - 1, top, right, bottom)):
        right -= 1
    return (left, top, right, bottom), (top + height - bottom + left + width - right)


def count(image):
    return image.histogram()[255]


def dilated_ink(image, window):
    if window < 1 or window % 2 == 0:
        raise ValueError("Ink dilation window must be positive and odd")
    if window > 21:
        return image.filter(ImageFilter.MaxFilter(window))
    return image.filter(ImageFilter.BoxBlur(window // 2)).point(lambda value: 255 if value else 0)


def analyze(output, engine):
    records = json.loads((output / f"{engine}-dom-images.json").read_text())
    native_viewport = None
    if engine.startswith("android"):
        browser = json.loads((output / f"{engine}-browser-report.json").read_text())
        native_viewport = browser["device"]["viewport"]
    results = []
    originals_checked = 0
    scales = set()
    for record in records:
        for kind, capture in record.get("native_tiles", {}).items():
            stitch_native_capture(output, record["images"][kind], capture)
        scale = record.get("scale", 1)
        scales.add(scale)
        edge = round(EDGE_TOLERANCE_CSS_PX * scale)
        window = 2 * edge + 1
        images = record["images"]
        if native_viewport and not record.get("native_tiles"):
            with Image.open(output / images["Actual"]) as image:
                if image.height > math.ceil(native_viewport[1] * scale):
                    raise ValueError("Native capture exceeds viewport; recapture with real scroll tiles")
        fills = {fill_edges(output / name) for name in images.values()}
        if len(fills) != 1:
            raise ValueError(f"Capture fill differs between diagnostic images: {record['key']}")
        box, fill_lines = fills.pop()
        if fill_lines > 2:
            raise ValueError(f"Unexpected capture fill ({fill_lines} lines): {record['key']}")
        actual = ink(output / images["Actual"], box)
        private = ink(output / images[record["code"]], box)
        ordinary = ink(output / images[f"Except {record['code']}"], box)
        original_mismatch = 0
        if "Original" in images:
            original = ink(output / images["Original"], box)
            if original.size != actual.size:
                raise ValueError(f"Diagnostic changed original layout: {record['key']}")
            first = ImageChops.multiply(original, ImageOps.invert(dilated_ink(actual, window)))
            second = ImageChops.multiply(actual, ImageOps.invert(dilated_ink(original, window)))
            original_mismatch = count(first) + count(second)
            originals_checked += 1
        if actual.size != private.size or actual.size != ordinary.size:
            raise ValueError(f"DOM layout changed in diagnostic {record['key']}")
        reconstruction = ImageChops.lighter(private, ordinary)
        mismatch = ImageChops.difference(actual, reconstruction)
        outside = ImageChops.multiply(mismatch, ImageOps.invert(dilated_ink(private, window)))
        bounds = private.getbbox()
        boundary = bounds and min(bounds[0], bounds[1], private.width - bounds[2], private.height - bounds[3])
        occurrences = []
        if not record.get("occurrences"):
            raise ValueError(f"Missing occurrence scopes: {record['key']}")
        for occurrence in record["occurrences"]:
            left, top, right, bottom = occurrence["scope"]
            region = (max(0, math.floor(left * scale) - edge - box[0]),
                      max(0, math.floor(top * scale) - edge - box[1]),
                      min(private.width, math.ceil(right * scale) + edge - box[0]),
                      min(private.height, math.ceil(bottom * scale) + edge - box[1]))
            if region[2] <= region[0] or region[3] <= region[1]:
                raise ValueError(f"Invalid occurrence scope: {record['key']}:{occurrence['index']}")
            occurrences.append({"index": occurrence["index"], "private_pixels": count(private.crop(region)),
                                "scope": region, "same_code_in_scope": occurrence["same_code_in_scope"]})
        results.append({"key": record["key"], "code": record["code"], "scale": scale,
                        "boundary_threshold_px": edge, "capture_fill_lines_cropped": fill_lines,
                        "private_pixels": count(private),
                        "boundary_clearance_px": boundary,
                        "original_geometry_mismatch": original_mismatch,
                        "overlap_pixels": count(ImageChops.multiply(private, ordinary)),
                        "outside_mask_mismatch": count(outside), "occurrences": occurrences,
                        "ring_check": record.get("ring_check"), "end_rows": record.get("end_rows", [])})
    report = {"engine": engine, "cases": len(results), "original_font_comparisons": originals_checked,
              "original_raster_edge_tolerance_px": EDGE_TOLERANCE_CSS_PX,
              "raster_edge_tolerance_unit": "css_px",
              "capture_scales": sorted(scales),
              "occurrences_checked": sum(len(row["occurrences"]) for row in results),
              "invisible_occurrences": [{"key": row["key"], "code": row["code"], **occurrence}
                                        for row in results for occurrence in row["occurrences"]
                                        if occurrence["private_pixels"] == 0],
              "ambiguous_occurrence_scopes": [{"key": row["key"], "code": row["code"], **occurrence}
                                              for row in results for occurrence in row["occurrences"]
                                              if occurrence["same_code_in_scope"] > 1],
              "captures_with_fill_cropped": sum(1 for row in results if row["capture_fill_lines_cropped"]),
              "invisible": [row for row in results if row["private_pixels"] == 0],
              "geometry_mismatch": [row for row in results if row["outside_mask_mismatch"] > 0 or row["original_geometry_mismatch"] > 0],
              "boundary_candidates": [row for row in results if row["boundary_clearance_px"] is not None and row["boundary_clearance_px"] < row["boundary_threshold_px"]],
              "overlap_candidates": [row for row in results if row["overlap_pixels"] >= 3],
              "results": results}
    (output / f"{engine}-dom-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: len(value) if isinstance(value, list) and key != "capture_scales" else value
                      for key, value in report.items() if key != "results"}, indent=2))
    if report["invisible"] or report["invisible_occurrences"] or report["geometry_mismatch"]:
        raise ValueError("invalid DOM ink diagnostic; do not interpret overlap candidates")
    if report["overlap_candidates"] or report["boundary_candidates"]:
        raise ValueError("Private ink needs visual review before approval; inspect original DOM captures")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/indopak-deep")
    parser.add_argument("--engine", required=True)
    args = parser.parse_args()
    analyze(args.output, args.engine)


if __name__ == "__main__":
    main()
