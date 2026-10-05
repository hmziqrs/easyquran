import argparse
from collections import Counter
import json
from pathlib import Path
import re
import unicodedata

from fontTools.colorLib.builder import buildCOLR, buildCPAL
from fontTools.ttLib import TTFont

from audit import DATABASE, ROOT, is_private, read_verses
from build import STEM


FONT = ROOT / f"web/static/fonts/{STEM}.ttf"


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
    kinds = ["Actual", "Base", *[f"{code:04X}" for code in codes],
             *[f"Except {code:04X}" for code in codes]]
    for kind in kinds:
        with TTFont(font_path) as font:
            hidden = {}
            for name in font.getGlyphOrder():
                except_code = kind.startswith("Except ")
                target = None
                if kind not in {"Actual", "Base"}:
                    target = code_glyphs[int(kind.split()[-1], 16)]
                keep = kind not in {"Actual", "Base"} and not except_code and name == target
                hidden_glyph = not keep and ((kind == "Base" and name in private_glyphs) or
                      (except_code and name == target) or
                      (kind not in {"Actual", "Base"} and not except_code))
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/indopak-deep")
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--font", type=Path, default=FONT)
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
    (args.output / "corpus-report.json").write_text(json.dumps(report, indent=2) + "\n",
                                                   encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
