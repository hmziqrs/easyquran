import argparse
import json
from pathlib import Path

from fontTools.ttLib import TTFont
import uharfbuzz as hb

from audit import ROOT
from build import STEM


FONT = ROOT / f"web/static/fonts/{STEM}.ttf"


def shaper(path):
    blob = Path(path).read_bytes()
    return hb.Font(hb.Face(blob))


def shape(font, text, direction, language):
    buffer = hb.Buffer()
    buffer.add_str(text)
    buffer.direction = direction
    buffer.script = "arab"
    buffer.language = language
    hb.shape(font, buffer)
    return buffer.glyph_infos, buffer.glyph_positions


def probe(path, font, codes):
    result = {}
    with TTFont(path) as tt:
        order = tt.getGlyphOrder()
        glyf = tt["glyf"]
        classes = {}
        if "GDEF" in tt and tt["GDEF"].table.GlyphClassDef:
            classes = tt["GDEF"].table.GlyphClassDef.classDefs
        for code in codes:
            infos, positions = shape(font, chr(int(code, 16)), "ltr", "ar")
            glyphs = []
            for info, position in zip(infos, positions):
                name = order[info.codepoint]
                glyph = glyf[name]
                glyph.recalcBounds(glyf)
                bounds = None
                if glyph.numberOfContours:
                    bounds = [glyph.xMin, glyph.xMax]
                glyphs.append({"glyph": name, "advance": position.x_advance,
                               "x_offset": position.x_offset, "ink_x": bounds,
                               "gdef_class": classes.get(name)})
            result[code] = glyphs
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--font", type=Path, default=FONT)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    request = json.loads(args.input.read_text(encoding="utf-8"))
    font = shaper(args.font)
    advances = []
    notdef = []
    for index, run in enumerate(request["runs"]):
        infos, positions = shape(font, run["text"], run["direction"], run["language"])
        advances.append(sum(position.x_advance for position in positions))
        if any(info.codepoint == 0 for info in infos):
            notdef.append(index)
    with TTFont(args.font) as tt:
        upem = tt["head"].unitsPerEm
    report = {"font": str(args.font.relative_to(ROOT)), "upem": upem, "advances": advances,
              "notdef": notdef, "probe": probe(args.font, font, request.get("probe", []))}
    args.output.write_text(json.dumps(report) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
