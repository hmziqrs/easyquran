import copy
import tempfile
import unittest
import unicodedata
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable

from audit import is_private, missing_coverage
from build import HERE, ROOT, STEM, add_mappings, build, checked_entries, obtain_input, read_json
from validate import shape, shaping_font


FONT = ROOT / f"web/static/fonts/{STEM}.woff2"


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
            with self.assertRaisesRegex(ValueError, "production blocked"):
                build(cache, output, HERE / "mapping.json", preview=False)
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
            self.assertEqual([item[0] for item in original], [item[0] for item in diagnostic])
            self.assertNotEqual([item[1] for item in original], [item[1] for item in diagnostic])
            self.assertFalse(self.manifest["production_approved"])
        finally:
            font.close()


if __name__ == "__main__":
    unittest.main()
