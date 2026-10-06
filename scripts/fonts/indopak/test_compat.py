import copy
import json
import math
import re
import tempfile
import unittest
import unicodedata
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.basePen import BasePen
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable

from audit import is_private, missing_coverage
from build import HERE, ROOT, STEM, add_mappings, build, checked_entries, obtain_input, read_json
from validate import shape, shaping_font
import uharfbuzz as hb


FONT = ROOT / f"web/static/fonts/{STEM}.woff2"
RENDERER = ROOT / "web/src/lib/quran/view/indopak.ts"
SIZE_ADJUST = 1.25


class OutlinePoints(BasePen):
    def __init__(self, glyphs):
        super().__init__(glyphs)
        self.points = []

    def _moveTo(self, point):
        self.points.append(point)

    def _lineTo(self, point):
        self.points.append(point)

    def _qCurveToOne(self, control, end):
        start = self._getCurrentPoint()
        for step in range(1, 65):
            t = step / 64
            self.points.append(tuple((1 - t) ** 2 * start[axis] +
                                     2 * (1 - t) * t * control[axis] + t ** 2 * end[axis]
                                     for axis in range(2)))

    def _closePath(self):
        pass


class CompatibilityTest(unittest.TestCase):
    def setUp(self):
        self.font = TTFont(FONT)
        self.manifest = read_json(HERE / "mapping.json")
        self.inventory = read_json(HERE / "inventory.json")

    def tearDown(self):
        self.font.close()

    def test_all_private_ranges_including_supplementary_boundaries(self):
        for codepoint in [0xE000, 0xF8FF, 0xF0000, 0xFFFFD, 0x100000, 0x10FFFD]:
            self.assertTrue(is_private(codepoint))
        for codepoint in [0xDFFF, 0xF900, 0xFFFFE, 0x10FFFE]:
            self.assertFalse(is_private(codepoint))

    def test_unresolved_duplicate_and_absent_targets_are_rejected(self):
        checked_entries(self.manifest, self.font, self.inventory)
        for field, value, message in [
            ("status", "unresolved", "unresolved mapping"),
            ("target_glyph", "nonexistent", "absent target glyph"),
            ("occurrences", 999, "inventory mismatch"),
            ("contexts", [], "context inventory mismatch"),
        ]:
            invalid = copy.deepcopy(self.manifest)
            invalid["entries"][0][field] = value
            with self.assertRaisesRegex(ValueError, message):
                checked_entries(invalid, self.font, self.inventory)
        invalid = copy.deepcopy(self.manifest)
        invalid["entries"].append(invalid["entries"][0])
        with self.assertRaisesRegex(ValueError, "duplicate/conflicting"):
            checked_entries(invalid, self.font, self.inventory)

    def test_production_build_refuses_unapproved_renderer_before_download(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            cache = Path(directory) / "cache"
            invalid = copy.deepcopy(self.manifest)
            invalid["production_approved"] = False
            invalid["renderer_blocker"] = "unreviewed rendering"
            manifest = Path(directory) / "mapping.json"
            manifest.write_text(json.dumps(invalid))
            with self.assertRaisesRegex(ValueError, "production blocked"):
                build(cache, output, manifest, preview=False)
            self.assertFalse(output.exists())
            self.assertFalse(cache.exists())

    def test_cached_font_checksum_is_verified_before_use(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            (cache / "font.ttf").write_bytes(b"corrupted font")
            with self.assertRaisesRegex(ValueError, "cached input checksum mismatch"):
                obtain_input({"filename": "font.ttf", "sha256": "0" * 64}, cache)

    def test_cmap_conflicts_are_rejected_and_variation_sequences_untouched(self):
        variation = CmapSubtable.newSubtable(14)
        variation.platformID = 0
        variation.platEncID = 5
        variation.language = 0
        variation.uvsDict = {0xFE00: [(0x0627, None)]}
        self.font["cmap"].tables.append(variation)
        before = copy.deepcopy(variation.uvsDict)
        add_mappings(self.font, {0xF0000: "uni0656"})
        self.assertEqual(variation.uvsDict, before)
        self.assertEqual(self.font.getBestCmap()[0xF0000], "uni0656")
        for table in self.font["cmap"].tables:
            if table.format == 4:
                self.assertNotIn(0xF0000, table.cmap)
        with self.assertRaisesRegex(ValueError, "conflicting existing cmap"):
            add_mappings(self.font, {0x0627: "uni0656"})

    def test_final_packaged_coverage_metrics_and_layout(self):
        self.assertEqual(missing_coverage(FONT, self.inventory)["missing_visible"], [])
        self.assertEqual(self.inventory["private_occurrences"], 1383)
        self.assertEqual(self.inventory["verse_count"], 6236)
        cmap = self.font.getBestCmap()
        classes = self.font["GDEF"].table.GlyphClassDef.classDefs
        for entry in self.manifest["entries"]:
            glyph = cmap[int(entry["codepoint"][2:], 16)]
            advance = self.font["hmtx"][glyph][0]
            if entry["source_behavior"] == "mark":
                self.assertEqual(classes[glyph], 3)
                self.assertEqual(advance, 0)
            else:
                self.assertEqual(classes[glyph], 1)
                self.assertGreater(advance, 0)
            self.assertNotEqual(glyph, ".notdef")
        self.assertEqual(self.font["hmtx"][cmap[0x2003]][0], self.font["head"].unitsPerEm)
        for tag in ["GDEF", "GSUB", "GPOS"]:
            self.font[tag].compile(self.font)

    def test_alias_does_not_change_unicode_properties_or_clusters(self):
        self.assertEqual(unicodedata.category("\uE004"), "Co")
        self.assertEqual(unicodedata.bidirectional("\uE004"), "L")
        self.assertEqual(unicodedata.combining("\uE004"), 0)
        self.assertEqual(unicodedata.bidirectional("\u0657"), "NSM")
        font, shaper = shaping_font(FONT)
        try:
            original = shape(shaper, "ء\uE004ا")
            diagnostic = shape(shaper, "ء\u0657ا")
            self.assertNotEqual([item[0] for item in original], [item[0] for item in diagnostic])
            self.assertNotEqual([item[1] for item in original], [item[1] for item in diagnostic])
        finally:
            font.close()

    def test_standalone_private_marks_have_separate_ink_lanes(self):
        cmap = self.font.getBestCmap()
        outlines = self.font["glyf"]
        per_mille = self.font["head"].unitsPerEm / 1000
        optional = outlines[cmap[0xE021]]
        for pause in [0x06D9, 0x0615, 0x06DA, 0x06DB, 0xE01E]:
            self.assertLess(optional.xMax + 30 * per_mille, outlines[cmap[pause]].xMin)
            self.assertGreater(optional.yMin, outlines[cmap[pause]].yMax + 10 * per_mille)
        inverted = outlines[cmap[0xE004]]
        hamza = outlines[cmap[0x0621]]
        gap = inverted.yMin - hamza.yMax
        self.assertGreater(gap, 40 * per_mille)
        self.assertLess(gap, 150 * per_mille)

    def test_spacing_pause_signs_share_one_raised_lane(self):
        cmap = self.font.getBestCmap()
        outlines = self.font["glyf"]
        bottoms = [outlines[cmap[code]].yMin for code in [0xE01A, 0xE01B, 0xE01C, 0xE01E, 0xE01F]]
        self.assertGreaterEqual(min(bottoms), 1000)
        self.assertLess(max(bottoms) - min(bottoms), 0.1 * self.font["head"].unitsPerEm)

    def test_ayah_sign_is_a_plain_ring_enclosing_its_digits(self):
        glyph_set = self.font.getGlyphSet()
        for name in ["uni06DD", "uni06DD.2", "uni06DD.3"]:
            self.assertEqual(self.font["glyf"][name].numberOfContours, 2)
            pen = BoundsPen(glyph_set)
            glyph_set[name].draw(pen)
            x_min, y_min, x_max, y_max = pen.bounds
            self.assertAlmostEqual(x_max - x_min, y_max - y_min, delta=4)

    def test_all_ayah_numbers_have_digit_ink_inside_the_ring(self):
        font, shaper = shaping_font(FONT)
        try:
            glyphs = font.getGlyphSet()
            outlines = {}
            for name in font.getGlyphOrder():
                if name.startswith("uni06F") and name.endswith((".medium", ".small")):
                    pen = OutlinePoints(glyphs)
                    glyphs[name].draw(pen)
                    outlines[name] = pen.points
            for number in range(1, 287):
                buffer = hb.Buffer()
                buffer.add_str("\u06dd" + "".join(chr(0x06F0 + int(digit)) for digit in str(number)))
                buffer.direction = "ltr"
                buffer.script = "arab"
                buffer.language = "ur"
                hb.shape(shaper, buffer)
                advance = 0
                center = None
                inner = 0
                for info, position in zip(buffer.glyph_infos, buffer.glyph_positions):
                    name = font.getGlyphName(info.codepoint)
                    if name.startswith("uni06DD"):
                        pen = BoundsPen(glyphs)
                        glyphs[name].draw(pen)
                        left, bottom, right, top = pen.bounds
                        center = ((left + right) / 2 + advance + position.x_offset,
                                  (bottom + top) / 2 + position.y_offset)
                        inner = (right - left) / 2 - 110
                    else:
                        self.assertIsNotNone(center)
                        radius = max(math.hypot(x + advance + position.x_offset - center[0],
                                                y + position.y_offset - center[1])
                                     for x, y in outlines[name])
                        self.assertGreater(inner - radius, 20, msg=f"{number}: {name}")
                    advance += position.x_advance
        finally:
            font.close()

    def test_renderer_ink_table_matches_packaged_marks(self):
        source = RENDERER.read_text(encoding="utf-8")
        table = dict(re.findall(r"\[0x([0-9a-f]{4}), \[(-?[0-9.]+, -?[0-9.]+)\]\]", source))
        self.assertEqual(len(table), 9)
        cmap = self.font.getBestCmap()
        glyph_set = self.font.getGlyphSet()
        upm = self.font["head"].unitsPerEm
        for code, extents in table.items():
            pen = BoundsPen(glyph_set)
            glyph_set[cmap[int(code, 16)]].draw(pen)
            expected = [pen.bounds[0] / upm * SIZE_ADJUST, pen.bounds[2] / upm * SIZE_ADJUST]
            actual = [float(value) for value in extents.split(",")]
            for want, got in zip(expected, actual):
                self.assertAlmostEqual(want, got, delta=0.001, msg=code)


if __name__ == "__main__":
    unittest.main()
