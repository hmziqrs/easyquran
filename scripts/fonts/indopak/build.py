import argparse
import hashlib
from importlib.metadata import version
import json
import math
from pathlib import Path
import urllib.request

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
OUTPUT = ROOT / "web/static/fonts"
STEM = "indopak-reader-compat-v4"
FAMILY = "IndoPak Reader Compat"
VERSION = "4.000"

# Construction geometry in base-font units (Lateef, 2048/em), tuned against browser ink
# measurements. Spacing pause signs keep their outline and gain side bearings; signs the base
# draws at baseline height (qif/waqfa) are lifted to the shared pause-sign lane.
SIGN_SIDEBEARING = 123
PAUSE_LANE_BOTTOM = 1000
E004_OFFSET = (239, -353)
E021_SCALE = 0.5
E021_OFFSET = (-800, 1390)
# Ayah ring: Lateef's "Simplified A" enclosure geometry with a heavier stroke, closer to the
# circular markers of printed IndoPak mushafs (and Quran.com's reference reader).
AYAH_RING_STROKE = 110
AYAH_ENCLOSURES = ("uni06DD", "uni06DD.2", "uni06DD.3")
AYAH_DIGIT_SCALES = {"medium": 0.8, "small": 0.75}


def require_versions():
    for requirement in (HERE / "requirements.txt").read_text().splitlines():
        package, expected = requirement.split("==")
        actual = version(package)
        if actual != expected:
            raise ValueError(f"dependency version mismatch: {package} {actual}, expected {expected}")


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def obtain_input(spec, cache):
    path = cache / spec["filename"]
    if not path.exists():
        cache.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(spec["url"], timeout=60) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != spec["sha256"]:
            raise ValueError(f"upstream checksum mismatch: {spec['filename']}")
        path.write_bytes(data)
    if hashlib.sha256(path.read_bytes()).hexdigest() != spec["sha256"]:
        raise ValueError(f"cached input checksum mismatch: {path}")
    return path


def checked_entries(manifest, font, inventory):
    entries = manifest["entries"]
    private = {item["codepoint"]: item["count"] for item in inventory["codepoints"]
               if item["category"] == "Co"}
    private_contexts = {item["codepoint"]: item["contexts"] for item in inventory["codepoints"]
                        if item["category"] == "Co"}
    seen = set()
    targets = set()
    for entry in entries:
        code = entry["codepoint"]
        if code in seen:
            raise ValueError(f"duplicate/conflicting assignment: {code}")
        seen.add(code)
        if entry["status"] != "verified":
            raise ValueError(f"unresolved mapping: {code}")
        if entry["occurrences"] != private.get(code):
            raise ValueError(f"inventory mismatch: {code}")
        if entry["contexts"] != private_contexts.get(code):
            raise ValueError(f"context inventory mismatch: {code}")
        glyph = entry["target_glyph"]
        if glyph == ".notdef" or glyph not in font.getGlyphOrder():
            raise ValueError(f"absent target glyph: {code} -> {glyph}")
        if glyph in targets:
            raise ValueError(f"distinct symbols share a target: {glyph}")
        targets.add(glyph)
        if font.getBestCmap().get(int(entry["unicode_equivalent"][2:], 16)) != glyph:
            raise ValueError(f"incorrect Unicode target: {code}")
        if entry["source_behavior"] not in {"mark", "spacing"}:
            raise ValueError(f"unreviewed source behavior: {code}")
    if seen != set(private):
        raise ValueError("private repertoire differs from mapping manifest")
    if len(seen) != manifest["expected_private_codepoints"]:
        raise ValueError("private codepoint total differs from manifest")
    if sum(private.values()) != manifest["expected_private_occurrences"]:
        raise ValueError("private occurrence total differs from manifest")
    return entries


def bounds(font, name):
    pen = BoundsPen(font.getGlyphSet())
    font.getGlyphSet()[name].draw(pen)
    return pen.bounds


def add_component(font, name, source, transform, advance, glyph_class):
    order = [*font.getGlyphOrder(), name]
    pen = TTGlyphPen(font.getGlyphSet())
    pen.addComponent(source, transform)
    glyph = pen.glyph()
    glyph.recalcBounds(font["glyf"])
    font["glyf"][name] = glyph
    font["hmtx"][name] = (advance, glyph.xMin)
    font.setGlyphOrder(order)
    font["GDEF"].table.GlyphClassDef.classDefs[name] = glyph_class


def make_glyph(font, entry):
    source = entry["target_glyph"]
    if entry["implementation"] == "reuse":
        return source
    name = "compat." + entry["codepoint"][2:]
    if name in font.getGlyphOrder():
        raise ValueError(f"duplicate generated glyph: {name}")
    if entry["codepoint"] == "U+E004":
        add_component(font, name, source, (1, 0, 0, 1, *E004_OFFSET), 0, 3)
        return name
    if entry["source_behavior"] == "spacing":
        x_min, y_min, x_max, _ = bounds(font, source)
        lift = max(0, PAUSE_LANE_BOTTOM - y_min)
        advance = round(x_max - x_min) + 2 * SIGN_SIDEBEARING
        transform = (1, 0, 0, 1, round(SIGN_SIDEBEARING - x_min), round(lift))
        add_component(font, name, source, transform, advance, 1)
        return name
    if entry["codepoint"] == "U+E021":
        transform = (E021_SCALE, 0, 0, E021_SCALE, *E021_OFFSET)
        add_component(font, name, source, transform, 0, 3)
        return name
    raise ValueError(f"no reviewed construction: {entry['codepoint']}")


def single_substitute(font, feature, glyph):
    table = font["GSUB"].table
    for record in table.FeatureList.FeatureRecord:
        if record.FeatureTag != feature:
            continue
        for index in record.Feature.LookupListIndex:
            for subtable in table.LookupList.Lookup[index].SubTable:
                mapping = getattr(getattr(subtable, "ExtSubTable", subtable), "mapping", None)
                if mapping and glyph in mapping:
                    return mapping[glyph]
    raise ValueError(f"no {feature} substitute for {glyph}")


def ring(center, outer, inner):
    pen = TTGlyphPen(None)
    for radius, clockwise in [(outer, True), (inner, False)]:
        steps = 16
        angles = [2 * math.pi * index / steps for index in range(steps)]
        if clockwise:
            angles = [-angle for angle in angles]
        control = radius / math.cos(math.pi / steps)
        points = []
        for index, angle in enumerate(angles):
            half = angle + (angles[1] - angles[0]) / 2
            points.append((round(center[0] + control * math.cos(half)),
                           round(center[1] + control * math.sin(half))))
        pen.qCurveTo(*points, None)
        pen.closePath()
    return pen.glyph()


def simplify_ayah_marker(font):
    for name in AYAH_ENCLOSURES:
        x_min, y_min, x_max, y_max = bounds(font, name.replace("uni06DD", "uni06DD.alt"))
        center = ((x_min + x_max) / 2, (y_min + y_max) / 2)
        outer = min(x_max - x_min, y_max - y_min) / 2
        glyph = ring(center, outer, outer - AYAH_RING_STROKE)
        glyph.recalcBounds(font["glyf"])
        font["glyf"][name] = glyph
        font["hmtx"][name] = (font["hmtx"][name][0], glyph.xMin)
    for name in font.getGlyphOrder():
        if not name.startswith("uni06F") or not name.endswith((".medium", ".small")):
            continue
        x_min, y_min, x_max, y_max = bounds(font, name)
        center_x = (x_min + x_max) / 2
        center_y = (y_min + y_max) / 2
        scale = AYAH_DIGIT_SCALES[name.rsplit(".", 1)[1]]
        pen = TTGlyphPen(None)
        transform = (scale, 0, 0, scale, center_x * (1 - scale), center_y * (1 - scale))
        font.getGlyphSet()[name].draw(TransformPen(pen, transform))
        glyph = pen.glyph()
        glyph.recalcBounds(font["glyf"])
        font["glyf"][name] = glyph
        font["hmtx"][name] = (font["hmtx"][name][0], glyph.xMin)


def add_mappings(font, mappings):
    original = dict(font.getBestCmap())
    full = {**original, **mappings}
    for table in font["cmap"].tables:
        if not table.isUnicode() or table.format == 14:
            continue
        for codepoint, glyph in mappings.items():
            if codepoint in table.cmap and table.cmap[codepoint] != glyph:
                raise ValueError(f"conflicting existing cmap assignment: U+{codepoint:04X}")
            if table.format == 12 or (table.format == 4 and codepoint <= 0xFFFF):
                table.cmap[codepoint] = glyph
    if any(codepoint > 0xFFFF for codepoint in mappings):
        for platform, encoding in [(0, 4), (3, 10)]:
            if any(table.format == 12 and table.platformID == platform and
                   table.platEncID == encoding for table in font["cmap"].tables):
                continue
            table = CmapSubtable.newSubtable(12)
            table.platformID = platform
            table.platEncID = encoding
            table.language = 0
            table.cmap = dict(full)
            font["cmap"].tables.append(table)
    for codepoint, glyph in original.items():
        if font.getBestCmap().get(codepoint) != glyph:
            raise ValueError(f"original cmap changed: U+{codepoint:04X}")


def rename(font):
    """Our own names; the base's Reserved Font Names stay out of every name record we set."""
    values = {1: FAMILY, 2: "Regular", 3: f"{STEM};Regular;{VERSION}",
              4: f"{FAMILY} Regular", 5: f"Version {VERSION}; private encoding compatibility",
              6: "IndoPakReaderCompat-Regular", 10: "IndoPak Quran reader font with compatibility mappings",
              16: FAMILY, 17: "Regular", 18: f"{FAMILY} Regular", 21: FAMILY, 22: "Regular"}
    for record in font["name"].names:
        if record.nameID in values:
            record.string = values[record.nameID].encode(record.getEncoding())
    for name_id, value in values.items():
        font["name"].setName(value, name_id, 3, 1, 0x409)
    if "DSIG" in font:
        del font["DSIG"]
    font["OS/2"].usWeightClass = 400


def validate_package(path, mappings, inventory, original):
    with TTFont(path) as packaged:
        if "glyf" not in packaged or "CFF " in packaged:
            raise ValueError("expected real TrueType outlines")
        cmap = packaged.getBestCmap()
        for codepoint, glyph in {**original, **mappings}.items():
            if cmap.get(codepoint) != glyph:
                raise ValueError(f"packaged cmap mismatch: U+{codepoint:04X}")
        for item in inventory["codepoints"]:
            codepoint = int(item["codepoint"][2:], 16)
            if item["category"] == "Cf":
                continue
            if cmap.get(codepoint) in {None, ".notdef"}:
                raise ValueError(f"packaged corpus coverage missing: {item['codepoint']}")
        for table in packaged["cmap"].tables:
            if not table.isUnicode() or table.format == 14:
                continue
            for codepoint, glyph in mappings.items():
                if table.format in {4, 12} and codepoint <= 0xFFFF:
                    if table.cmap.get(codepoint) != glyph:
                        raise ValueError("packaged Unicode subtables disagree")
        if packaged["name"].getDebugName(1) != FAMILY:
            raise ValueError("packaged font name mismatch")
        for tag in ["GDEF", "GSUB", "GPOS"]:
            packaged[tag].compile(packaged)


def build(cache, output, manifest_path, preview):
    manifest = read_json(manifest_path)
    if not preview and not manifest["production_approved"]:
        raise ValueError("production blocked: " + manifest["renderer_blocker"])
    require_versions()
    upstream = read_json(HERE / "upstream.json")
    path = obtain_input(upstream["build_inputs"][0], cache)
    license_path = obtain_input(upstream["build_inputs"][1], cache)
    font = TTFont(path, recalcTimestamp=False)
    if "fvar" in font or "glyf" not in font:
        raise ValueError("expected a static TrueType base font")
    inventory = read_json(HERE / "inventory.json")
    original = dict(font.getBestCmap())
    entries = checked_entries(manifest, font, inventory)
    mappings = {int(entry["codepoint"][2:], 16): make_glyph(font, entry) for entry in entries}
    if 0x2003 not in original:
        pen = TTGlyphPen(None)
        order = [*font.getGlyphOrder(), "compat.emspace"]
        font["glyf"]["compat.emspace"] = pen.glyph()
        font["hmtx"]["compat.emspace"] = (font["head"].unitsPerEm, 0)
        font.setGlyphOrder(order)
        mappings[0x2003] = "compat.emspace"
    if 0xFE8E not in original:
        mappings[0xFE8E] = single_substitute(font, "fina", original[0x0627])
    simplify_ayah_marker(font)
    add_mappings(font, mappings)
    rename(font)
    output.mkdir(parents=True, exist_ok=True)
    paths = []
    for suffix, flavor in [("ttf", None), ("woff2", "woff2")]:
        packaged_path = output / f"{STEM}.{suffix}"
        font.flavor = flavor
        font.save(packaged_path)
        validate_package(packaged_path, mappings, inventory, original)
        paths.append(packaged_path)
    (output / f"{STEM}.OFL.txt").write_bytes(license_path.read_bytes())
    (output / f"{STEM}.FONTLOG.txt").write_bytes((HERE / "FONTLOG.txt").read_bytes())
    return paths


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--preview", action="store_true")
    parser.add_argument("--cache", type=Path, default=ROOT / ".cache/fonts/indopak")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--manifest", type=Path, default=HERE / "mapping.json")
    args = parser.parse_args()
    for path in build(args.cache, args.output, args.manifest, args.preview):
        print(f"{path} ({path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
