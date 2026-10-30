#!/usr/bin/env python3
"""Regenerate reference/index.html from gpui-kit-sample data + fonts.

One combo per URL: index.html?font=<arabic id>&urdu=<urdu id>&db=<dataset>
"""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
data = json.loads((HERE / "../../gpui-kit-sample/data/quran-sample.json").read_text())

FONTS = [
    ("amiri", "Amiri", None),
    ("scheherazade-new", "Scheherazade New", None),
    ("noto-naskh-arabic", "Noto Naskh Arabic", None),
    ("kfgqpc-hafs", "KFGQPC Uthmanic Hafs", None),
    ("kfgqpc-taha-v1", "KFGQPC Uthman Taha Naskh", 400),
    ("kfgqpc-taha-v1-bold", "KFGQPC Uthman Taha Naskh", 700),
]
URDU_FONTS = [
    ("noto-nastaliq-urdu", "Noto Nastaliq Urdu", 2.5),
    ("gulzar", "Gulzar", 2.5),
    ("awami-nastaliq", "Awami Nastaliq", 2.6),
    ("mehr-nastaliq-web", "Mehr Nastaliq Web", 2.9),
    ("lateef", "Lateef", 2.2),
]
FONT_FACES = """
@font-face { font-family: "Amiri"; src: url("../../gpui-kit-sample/fonts/amiri.ttf"); }
@font-face { font-family: "Scheherazade New"; src: url("../../gpui-kit-sample/fonts/scheherazade-new.ttf"); }
@font-face { font-family: "Noto Naskh Arabic"; src: url("../../gpui-kit-sample/fonts/noto-naskh-arabic.ttf"); }
@font-face { font-family: "KFGQPC Uthmanic Hafs"; src: url("../../gpui-kit-sample/fonts/kfgqpc-hafs.ttf"); }
@font-face { font-family: "KFGQPC Uthman Taha Naskh"; src: url("../../gpui-kit-sample/fonts/kfgqpc-taha-v1.ttf"); font-weight: 400; }
@font-face { font-family: "KFGQPC Uthman Taha Naskh"; src: url("../../gpui-kit-sample/fonts/kfgqpc-taha-v1-bold.ttf"); font-weight: 700; }
@font-face { font-family: "Mehr Nastaliq Web"; src: url("../../gpui-kit-sample/fonts/mehr-nastaliq.ttf"); }
@font-face { font-family: "Noto Nastaliq Urdu"; src: url("NotoNastaliqUrdu.ttf"); }
@font-face { font-family: "Gulzar"; src: url("../../flutter-quran-sample/fonts/Gulzar-Regular.ttf"); }
@font-face { font-family: "Awami Nastaliq"; src: url("../../flutter-quran-sample/fonts/AwamiNastaliq-Regular.ttf"); }
@font-face { font-family: "Lateef"; src: url("../../flutter-quran-sample/fonts/Lateef-Regular.ttf"); }
"""

TEMPLATE = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
__FONT_FACES__
body { font-family: -apple-system, sans-serif; margin: 0; background: #fff; }
.toolbar { padding: 12px 16px; border-bottom: 1px solid #ddd; font-size: 14px; color: #444; }
.surah-header { text-align: center; padding: 20px 0; background: #f5f5f5; border-bottom: 1px solid #ddd; }
.surah-header .en { font-size: 15px; font-weight: 600; }
.surah-header .ar { font-family: "KFGQPC Uthmanic Hafs"; font-size: 28px; color: #00695c; margin-top: 4px; }
.ayah { padding: 16px 32px; border-bottom: 1px solid #ddd; }
.ayah .ar { font-size: 30px; line-height: 1.9; direction: rtl; text-align: right; color: #111; }
.ayah .ur { font-size: 20px; direction: rtl; text-align: right; color: #666; margin-top: 8px; }
.ayah .marker { font-family: "KFGQPC Uthmanic Hafs"; font-size: 16px; color: #00695c; margin-top: 8px; direction: rtl; text-align: right; }
</style>
</head>
<body>
<div class="toolbar" id="toolbar"></div>
<div id="content"></div>
<script>
const DATA = __DATA__;
const FONTS = __FONTS__;
const URDU_FONTS = __URDU__;
const params = new URLSearchParams(location.search);
const fontId = params.get("font") || "kfgqpc-hafs";
const urduId = params.get("urdu") || "noto-nastaliq-urdu";
const dataset = params.get("db") || "uthmani";
const font = FONTS.find(f => f.id === fontId);
const urdu = URDU_FONTS.find(f => f.id === urduId);
document.getElementById("toolbar").textContent = `browser reference \u2014 ${font.id} / ${font.family} \u00d7 ${urdu.id} \u00d7 ${dataset}`;
const content = document.getElementById("content");
for (const surah of DATA.arabic[dataset].surahs) {
  const header = document.createElement("div");
  header.className = "surah-header";
  header.innerHTML = `<div class="en">${surah.name_en} \u2014 Surah ${surah.number}</div><div class="ar">${surah.name_ar}</div>`;
  content.appendChild(header);
  const urList = (DATA.translations.ur[String(surah.number)] || []);
  for (const ayah of surah.ayahs) {
    const urEntry = urList.find(u => u.n === ayah.n);
    const digits = String(ayah.n).split("").map(c => String.fromCharCode(0x0660 + Number(c))).join("");
    const row = document.createElement("div");
    row.className = "ayah";
    const ar = document.createElement("div");
    ar.className = "ar";
    ar.style.fontFamily = `"${font.family}"`;
    if (font.weight) ar.style.fontWeight = font.weight;
    ar.textContent = ayah.text;
    const ur = document.createElement("div");
    ur.className = "ur";
    ur.style.fontFamily = `"${urdu.family}"`;
    ur.style.lineHeight = urdu.height;
    ur.textContent = urEntry ? urEntry.text : "";
    const marker = document.createElement("div");
    marker.className = "marker";
    marker.textContent = `\uFD3F${digits}\uFD3E`;
    row.append(ar, ur, marker);
    content.appendChild(row);
  }
}
</script>
</body>
</html>
"""


def main():
    html = TEMPLATE
    html = html.replace("__FONT_FACES__", FONT_FACES)
    html = html.replace("__DATA__", json.dumps(data, ensure_ascii=False))
    html = html.replace("__FONTS__", json.dumps(
        [{"id": fid, "family": fam, "weight": w} for fid, fam, w in FONTS]))
    html = html.replace("__URDU__", json.dumps(
        [{"id": fid, "family": fam, "height": h} for fid, fam, h in URDU_FONTS]))
    (HERE / "index.html").write_text(html, encoding="utf-8")
    print("wrote index.html")


if __name__ == "__main__":
    main()
