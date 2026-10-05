import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter, ImageOps

from audit import ROOT

# Production-font/diagnostic raster comparisons tolerate this much ink-edge drift, in CSS
# pixels. Native captures (Safari, devices) arrive at devicePixelRatio scale, so the window
# scales with the recorded capture scale; the CSS-pixel value itself stays frozen.
EDGE_TOLERANCE_CSS_PX = 2


def ink(path, box=None):
    with Image.open(path) as image:
        if box:
            image = image.crop(box)
        return image.convert("L").point(lambda value: 255 if value < 128 else 0)


def fill_edges(path):
    """Edge rows/columns of pure opaque black that Safari adds when it rounds an element box
    up to whole device pixels. Audit captures force a white background, so page content never
    produces a full pure-black edge line."""
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


def analyze(output, engine):
    records = json.loads((output / f"{engine}-dom-images.json").read_text())
    results = []
    originals_checked = 0
    scales = set()
    for record in records:
        scale = record.get("scale", 1)
        scales.add(scale)
        edge = round(EDGE_TOLERANCE_CSS_PX * scale)
        window = 2 * edge + 1
        images = record["images"]
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
            first = ImageChops.multiply(original, ImageOps.invert(actual.filter(ImageFilter.MaxFilter(window))))
            second = ImageChops.multiply(actual, ImageOps.invert(original.filter(ImageFilter.MaxFilter(window))))
            original_mismatch = count(first) + count(second)
            originals_checked += 1
        if actual.size != private.size or actual.size != ordinary.size:
            raise ValueError(f"DOM layout changed in diagnostic {record['key']}")
        reconstruction = ImageChops.lighter(private, ordinary)
        mismatch = ImageChops.difference(actual, reconstruction)
        outside = ImageChops.multiply(mismatch, ImageOps.invert(private.filter(ImageFilter.MaxFilter(window))))
        bounds = private.getbbox()
        boundary = bounds and min(bounds[0], bounds[1], private.width - bounds[2], private.height - bounds[3])
        results.append({"key": record["key"], "code": record["code"], "scale": scale,
                        "boundary_threshold_px": edge, "capture_fill_lines_cropped": fill_lines,
                        "private_pixels": count(private),
                        "boundary_clearance_px": boundary,
                        "original_geometry_mismatch": original_mismatch,
                        "overlap_pixels": count(ImageChops.multiply(private, ordinary)),
                        "outside_mask_mismatch": count(outside)})
    report = {"engine": engine, "cases": len(results), "original_font_comparisons": originals_checked,
              "original_raster_edge_tolerance_px": EDGE_TOLERANCE_CSS_PX,
              "raster_edge_tolerance_unit": "css_px",
              "capture_scales": sorted(scales),
              "captures_with_fill_cropped": sum(1 for row in results if row["capture_fill_lines_cropped"]),
              "invisible": [row for row in results if row["private_pixels"] == 0],
              "geometry_mismatch": [row for row in results if row["outside_mask_mismatch"] > 0 or row["original_geometry_mismatch"] > 0],
              "boundary_candidates": [row for row in results if row["boundary_clearance_px"] is not None and row["boundary_clearance_px"] < row["boundary_threshold_px"]],
              "overlap_candidates": [row for row in results if row["overlap_pixels"] >= 3],
              "results": results}
    (output / f"{engine}-dom-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: len(value) if isinstance(value, list) and key != "capture_scales" else value
                      for key, value in report.items() if key != "results"}, indent=2))
    if report["invisible"] or report["geometry_mismatch"]:
        raise ValueError("invalid DOM ink diagnostic; do not interpret overlap candidates")
    if report["overlap_candidates"] or report["boundary_candidates"]:
        raise ValueError("Private ink needs visual review before approval; inspect original DOM captures")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/indopak-deep")
    parser.add_argument("--engine", choices=["chromium", "webkit", "chromium-flow", "webkit-flow", "safari", "safari-flow"], required=True)
    args = parser.parse_args()
    analyze(args.output, args.engine)


if __name__ == "__main__":
    main()
