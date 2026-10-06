import argparse
import io
import json
from pathlib import Path

from fontTools.ttLib import TTFont
import uharfbuzz as hb

from audit import DATABASE, HERE, ROOT, read_verses
from build import STEM, read_json


def shaping_font(path):
    font = TTFont(path)
    font.flavor = None
    buffer = io.BytesIO()
    font.save(buffer)
    return font, hb.Font(hb.Face(buffer.getvalue()))


def shape(font, text):
    buffer = hb.Buffer()
    buffer.add_str(text)
    buffer.direction = "rtl"
    buffer.script = "arab"
    buffer.language = "ar"
    hb.shape(font, buffer)
    return [(info.codepoint, info.cluster, position.x_advance, position.y_advance,
             position.x_offset, position.y_offset)
            for info, position in zip(buffer.glyph_infos, buffer.glyph_positions)]


def validate(database, ttf, woff2, upstream):
    rows = read_verses(database)
    manifest = read_json(HERE / "mapping.json")
    tt, tt_shaper = shaping_font(ttf)
    web, web_shaper = shaping_font(woff2)
    original, original_shaper = shaping_font(upstream)
    private = {int(entry["codepoint"][2:], 16) for entry in manifest["entries"]}
    covered_private = set()
    controls_checked = 0
    for sura, aya, text in rows:
        trace = shape(tt_shaper, text)
        if any(item[0] == 0 for item in trace):
            raise ValueError(f".notdef in corpus shaping at {sura}:{aya}")
        if trace != shape(web_shaper, text):
            raise ValueError(f"TTF/WOFF2 shaping disagreement at {sura}:{aya}")
        covered_private.update(ord(char) for char in text if ord(char) in private)
        # Runs with private codes, U+2003 or the U+FE8E presentation-form alias are changed
        # deliberately; every other run must shape exactly like the upstream base font.
        if not any(ord(char) in private or ord(char) in {0x2003, 0xFE8E} for char in text):
            current_names = [(tt.getGlyphName(item[0]), *item[1:]) for item in trace]
            original_names = [(original.getGlyphName(item[0]), *item[1:])
                              for item in shape(original_shaper, text)]
            if current_names != original_names:
                raise ValueError(f"unaffected run changed at {sura}:{aya}")
            controls_checked += 1
    if covered_private != private:
        raise ValueError("not every private code was exercised")
    for text in ["سَلَامٌ", "بِسْمِ اللّٰهِ", "(١٢٣) ٤٥٦", "۱۲۳ (٤٥٦)"]:
        if any(item[0] == 0 for item in shape(tt_shaper, text)):
            raise ValueError("missing glyph in punctuation/digit controls")
    original.close()
    web.close()
    tt.close()
    return {"verses_shaped": len(rows), "private_codes_exercised": len(private),
            "unaffected_runs_identical": controls_checked, "notdef_count": 0,
            "ttf_woff2_traces_identical": True, "harfbuzz": hb.version_string(),
            "limitation": "Forced Arabic/RTL HarfBuzz runs do not model browser bidi itemization.",
            "production_approved": manifest["production_approved"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--database", type=Path, default=DATABASE)
    parser.add_argument("--ttf", type=Path, default=ROOT / f"web/static/fonts/{STEM}.ttf")
    parser.add_argument("--woff2", type=Path, default=ROOT / f"web/static/fonts/{STEM}.woff2")
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report = validate(args.database, args.ttf, args.woff2, args.upstream)
    body = json.dumps(report, indent=2) + "\n"
    if args.report:
        args.report.write_text(body, encoding="utf-8")
    print(body)


if __name__ == "__main__":
    main()
