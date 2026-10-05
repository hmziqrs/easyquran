import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import urllib.request

from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable
from fontTools.varLib.instancer import instantiateVariableFont


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
OUTPUT = ROOT / "web/static/fonts"
STEM = "indopak-reader-compat-v2"
FAMILY = "IndoPak Reader Compat"


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
        add_component(font, name, source, (1, 0, 0, 1, 122, -377), 0, 3)
        return name
    if entry["source_behavior"] == "spacing":
        glyph = font["glyf"][source]
        advance = glyph.xMax - glyph.xMin + 60
        add_component(font, name, source, (1, 0, 0, 1, 30 - glyph.xMin, 0), advance, 1)
        return name
    if entry["codepoint"] == "U+E021":
        add_component(font, name, source, (0.5, 0, 0, 0.5, -250, 820), 0, 3)
        return name
    raise ValueError(f"no reviewed construction: {entry['codepoint']}")


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
    values = {1: FAMILY, 2: "Regular", 3: f"{STEM};Regular;2.000",
              4: f"{FAMILY} Regular", 5: "Version 2.000; private encoding compatibility",
              6: "IndoPakReaderCompat-Regular", 16: FAMILY, 17: "Regular",
              18: f"{FAMILY} Regular", 21: FAMILY, 22: "Regular"}
    for record in font["name"].names:
        if record.nameID in values:
            record.string = values[record.nameID].encode(record.getEncoding())
    for name_id, value in values.items():
        font["name"].setName(value, name_id, 3, 1, 0x409)
    if "DSIG" in font:
        del font["DSIG"]


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
    font = instantiateVariableFont(font, {"wght": 400}, inplace=True)
    font.recalcTimestamp = False
    inventory = read_json(HERE / "inventory.json")
    original = dict(font.getBestCmap())
    entries = checked_entries(manifest, font, inventory)
    mappings = {int(entry["codepoint"][2:], 16): make_glyph(font, entry) for entry in entries}
    pen = TTGlyphPen(None)
    order = [*font.getGlyphOrder(), "compat.emspace"]
    font["glyf"]["compat.emspace"] = pen.glyph()
    font["hmtx"]["compat.emspace"] = (font["head"].unitsPerEm, 0)
    font.setGlyphOrder(order)
    mappings[0x2003] = "compat.emspace"
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
