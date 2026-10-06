import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageOps

from audit import ROOT
from dom_ink_analysis import EDGE_TOLERANCE_CSS_PX, count, dilated_ink, fill_edges, ink


def analyze(output, engine):
    records = json.loads((output / f"{engine}-ring-images.json").read_text())
    pairs = [(record["size"], number["number"]) for record in records for number in record["numbers"]]
    expected = {(size, number) for size in [22, 24, 33, 48, 56] for number in range(1, 287)}
    if len(pairs) != len(set(pairs)) or set(pairs) != expected:
        raise ValueError("Ring capture must cover every number exactly once at each size")
    results = []
    for record in records:
        fills = {fill_edges(output / filename) for filename in record["images"].values()}
        if len(fills) != 1:
            raise ValueError("Ring capture fill differs between diagnostic images")
        capture_box, fill_lines = fills.pop()
        if fill_lines > 2:
            raise ValueError(f"Unexpected ring capture fill ({fill_lines} lines)")
        images = {kind: ink(output / filename, capture_box) for kind, filename in record["images"].items()}
        if len({image.size for image in images.values()}) != 1:
            raise ValueError("Ring diagnostic image sizes differ")
        scale = record["scale"]
        edge = round(EDGE_TOLERANCE_CSS_PX * scale)
        window = 2 * edge + 1
        with Image.open(output / record["images"]["Original"]) as original_image:
            original_size = original_image.size
        if abs(original_size[0] - record["width"] * scale) > scale + 1 or abs(original_size[1] - record["height"] * scale) > scale + 1:
            raise ValueError("Ring grid screenshot is clipped")
        for number in record["numbers"]:
            box = tuple(round(value * scale) - capture_box[index % 2] for index, value in enumerate(number["box"]))
            original, actual, ring, digits = [images[kind].crop(box) for kind in ["Original", "Actual", "Ring", "Digits"]]
            interior = ring.copy()
            ImageDraw.floodfill(interior, (0, 0), 128)
            interior = interior.point(lambda value: 255 if value == 0 else 0)
            outside = count(ImageChops.multiply(digits, ImageOps.invert(interior)))
            first = ImageChops.multiply(original, ImageOps.invert(dilated_ink(actual, window)))
            second = ImageChops.multiply(actual, ImageOps.invert(dilated_ink(original, window)))
            reconstructed = ImageChops.lighter(ring, digits)
            mismatch = ImageChops.difference(actual, reconstructed)
            reconstruction_error = count(ImageChops.multiply(mismatch, ImageOps.invert(dilated_ink(reconstructed, window))))
            results.append({"size": record["size"], "number": number["number"], "scale": scale,
                            "capture_fill_lines_cropped": fill_lines,
                            "ring_pixels": count(ring), "digit_pixels": count(digits),
                            "outside_pixels": outside, "original_geometry_mismatch": count(first) + count(second),
                            "reconstruction_mismatch": reconstruction_error})
    failures = [row for row in results if not row["ring_pixels"] or not row["digit_pixels"] or row["outside_pixels"] or row["original_geometry_mismatch"] or row["reconstruction_mismatch"]]
    report = {"engine": engine, "method": "Actual DOM captures with unchanged-geometry ring/digit colour-font masks",
              "cases": len(results), "numbers": [1, 286], "sizes": sorted({row["size"] for row in results}),
              "edge_tolerance_css_px": EDGE_TOLERANCE_CSS_PX, "failures": failures, "results": results}
    (output / f"{engine}-ring-report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "results"}, indent=2))
    if failures or len(results) != 286 * 5:
        raise ValueError("Ring DOM containment or capture validation failed")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/indopak-deep")
    parser.add_argument("--engine", required=True)
    args = parser.parse_args()
    analyze(args.output, args.engine)


if __name__ == "__main__":
    main()
