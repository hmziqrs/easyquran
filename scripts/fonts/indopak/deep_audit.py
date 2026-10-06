import argparse
from collections import Counter
import json
from pathlib import Path
import re
import unicodedata

from fontTools.colorLib.builder import buildCOLR, buildCPAL
from fontTools.ttLib import TTFont

from audit import DATABASE, ROOT, is_private, read_verses
from build import AYAH_ENCLOSURES, AYAH_RING_STROKE, STEM


FONT = ROOT / f"web/static/fonts/{STEM}.ttf"
UPSTREAM = ROOT / ".cache/fonts/indopak/Lateef-SemiBold.ttf"
SIZE_ADJUST = 1.25
REFERENCE_KEYS = ("1:1", "112:1")


def excerpts(rows):
    occurrences = []
    for sura, aya, text in rows:
        boundaries = [match.span() for match in re.finditer(r"\S+", text)]
        for index, character in enumerate(text):
            if not is_private(ord(character)):
                continue
            word_index = next(i for i, (start, end) in enumerate(boundaries)
                              if start <= index < end)
            start = boundaries[max(0, word_index - 1)][0]
            end = boundaries[min(len(boundaries) - 1, word_index + 1)][1]
            occurrences.append({"key": f"{sura}:{aya}", "index": index,
                                "code": f"{ord(character):04X}",
                                "excerpt": text[start:end], "offset": index - start,
                                "terminal": not text[index + 1:] or
                                all(unicodedata.category(c)[0] in {"M", "Z"}
                                    or unicodedata.category(c) == "Cf"
                                    for c in text[index + 1:])})
    return occurrences


def renamed(font, family):
    values = {1: family, 4: family, 6: family.replace(" ", ""), 16: family}
    for record in font["name"].names:
        if record.nameID in values:
            record.string = values[record.nameID].encode(record.getEncoding())
    return font


def rendering_state(font):
    outlines = []
    for name in font.getGlyphOrder():
        glyph = font["glyf"][name]
        coordinates, ends, flags = glyph.getCoordinates(font["glyf"])
        instructions = b""
        if hasattr(glyph, "program"):
            instructions = glyph.program.getBytecode()
        outlines.append((tuple(coordinates), tuple(ends), tuple(flag & 1 for flag in flags), instructions))
    return (font.getGlyphOrder(), outlines, font["hmtx"].metrics,
            font["head"].unitsPerEm,
            (font["hhea"].ascent, font["hhea"].descent, font["hhea"].lineGap),
            {tag: font[tag].compile(font) for tag in ["cmap", "GDEF", "GSUB", "GPOS", "OS/2"]})


def diagnostics(output, codes, font_path):
    with TTFont(font_path) as original:
        code_glyphs = {code: original.getBestCmap()[code] for code in codes}
        private_glyphs = set(code_glyphs.values())
        original_state = rendering_state(original)
    kinds = ["Actual", "Base", "Ring", "Digits", *[f"{code:04X}" for code in codes],
             *[f"Except {code:04X}" for code in codes]]
    for kind in kinds:
        with TTFont(font_path) as font:
            hidden = {}
            for name in font.getGlyphOrder():
                except_code = kind.startswith("Except ")
                target = None
                if kind not in {"Actual", "Base", "Ring", "Digits"}:
                    target = code_glyphs[int(kind.split()[-1], 16)]
                keep = kind not in {"Actual", "Base", "Ring", "Digits"} and not except_code and name == target
                hidden_glyph = not keep and ((kind == "Base" and name in private_glyphs) or
                      (except_code and name == target) or
                      (kind == "Ring" and name not in AYAH_ENCLOSURES) or
                      (kind == "Digits" and name in AYAH_ENCLOSURES) or
                      (kind not in {"Actual", "Base", "Ring", "Digits"} and not except_code))
                palette = 0xFFFF
                if hidden_glyph:
                    palette = 0
                hidden[name] = [(name, palette)]
            font["COLR"] = buildCOLR(hidden, version=0, glyphMap=font.getReverseGlyphMap())
            font["CPAL"] = buildCPAL([[(0, 0, 0, 0)]])
            renamed(font, f"IndoPak Audit {kind}")
            font.flavor = "woff2"
            packaged = output / f"audit-{kind.lower().replace(' ', '-')}.woff2"
            font.save(packaged)
            with TTFont(packaged) as check:
                if rendering_state(check) != original_state:
                    raise ValueError(f"diagnostic geometry, hinting or layout changed: {kind}")


def reference_report(path, codes):
    with TTFont(path) as font:
        cmap = font.getBestCmap()
        result = {"family": font["name"].getDebugName(1),
                  "version": font["name"].getDebugName(5), "symbols": []}
        for code in codes:
            name = cmap.get(code)
            glyph = font["glyf"][name]
            glyph.recalcBounds(font["glyf"])
            result["symbols"].append({
                "code": f"{code:04X}", "glyph": name,
                "advance": font["hmtx"][name][0],
                "bounds": [glyph.xMin, glyph.yMin, glyph.xMax, glyph.yMax],
                "cmap_aliases": [f"{cp:04X}" for cp, glyph_name in cmap.items()
                                 if glyph_name == name and cp != code],
            })
        return result


def font_reference(output, rows, font_path, upstream_path):
    import uharfbuzz as hb

    from validate import shape, shaping_font

    texts = {f"{sura}:{aya}": text for sura, aya, text in rows}
    with TTFont(upstream_path) as source:
        renamed(source, "IndoPak Audit Upstream")
        source.flavor = "woff2"
        source.save(output / "audit-upstream.woff2")
    fonts = {}
    shapers = {}
    for label, path in [("upstream", upstream_path), ("packaged", font_path)]:
        font, shaper = shaping_font(path)
        fonts[label] = {"path": str(Path(path).resolve().relative_to(ROOT)),
                        "family": font["name"].getDebugName(1),
                        "units_per_em": font["head"].unitsPerEm}
        shapers[label] = (font["head"].unitsPerEm, shaper)
    words = []
    for key in REFERENCE_KEYS:
        for word in texts[key].split():
            if any(is_private(ord(c)) or unicodedata.category(c) == "Cf" for c in word):
                continue
            if not any(unicodedata.category(c).startswith("L") for c in word):
                continue
            advances = {label: sum(glyph[2] for glyph in shape(shaper, word)) / upem
                        for label, (upem, shaper) in shapers.items()}
            words.append({"key": key, "text": word,
                          "upstream_advance_em": round(advances["upstream"], 6),
                          "packaged_advance_em": round(advances["packaged"], 6),
                          "advances_identical": advances["upstream"] == advances["packaged"]})
    report = {"size_adjust": SIZE_ADJUST,
              "ring_stroke_em": AYAH_RING_STROKE / fonts["packaged"]["units_per_em"] * SIZE_ADJUST,
              "harfbuzz": hb.version_string(),
              "shaping": {"direction": "rtl", "script": "arab", "language": "ar"},
              "fonts": fonts, "words": words}
    with TTFont(font_path) as packaged:
        from build import bounds
        signs = [0x0614, 0x0615, *range(0x06D6, 0x06DD), 0xE01A, 0xE01B, 0xE01C, 0xE01E, 0xE01F, 0xE021, 0xE022]
        report["single_signs"] = {f"{code:04X}": [value / packaged["head"].unitsPerEm * SIZE_ADJUST
                                                  for value in bounds(packaged, packaged.getBestCmap()[code])]
                                  for code in signs}
        report["ayah_enclosures"] = [[value / packaged["head"].unitsPerEm * SIZE_ADJUST
                                      for value in bounds(packaged,name)] for name in AYAH_ENCLOSURES]
    (output / "font-reference.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/indopak-deep")
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--font", type=Path, default=FONT)
    parser.add_argument("--upstream", type=Path, default=UPSTREAM,
                        help="pinned upstream Lateef TTF for the size-adjust reference")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = read_verses(DATABASE)
    occurrences = excerpts(rows)
    counts = Counter(item["code"] for item in occurrences)
    codes = sorted(int(code, 16) for code in counts)
    diagnostics(args.output, codes, args.font)
    (args.output / "occurrences.json").write_text(
        json.dumps(occurrences, ensure_ascii=False), encoding="utf-8")
    report = {"database_id": "quran-indopak", "database_open_mode": "ro&immutable=1",
              "verses": len(rows), "private_verses": len({item["key"] for item in occurrences}),
              "occurrences": len(occurrences), "symbols": dict(sorted(counts.items())),
              "exact_excerpts": len({item["excerpt"] for item in occurrences}),
              "terminal_counts": dict(sorted(Counter(item["code"] for item in occurrences
                                                       if item["terminal"]).items())),
              "diagnostic_fonts": "Local OFL font only; original outlines, cmap, advances and layout retained. Transparent COLR layers isolate ink.",
              "diagnostic_geometry_hinting_and_layout_verified": True
              }
    if args.reference:
        report["reference"] = reference_report(args.reference, codes)
    if args.upstream.exists():
        reference = font_reference(args.output, rows, args.font, args.upstream)
        report["font_reference_words"] = len(reference["words"])
    else:
        report["font_reference_words"] = None
        print(f"warning: {args.upstream} missing; size-adjust reference not written")
    (args.output / "corpus-report.json").write_text(json.dumps(report, indent=2) + "\n",
                                                   encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
