import argparse
import html
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = json.loads((args.output / "chromium-layout-report.json").read_text())
    captures = report["captures"]
    font_path = Path("/System/Library/Fonts/Supplemental/Arial.ttf")
    font = ImageFont.truetype(str(font_path), 26) if font_path.exists() else ImageFont.load_default()
    sheets = []
    for start in range(0, len(captures), 12):
        group = captures[start:start + 12]
        opened = [[Image.open(args.output / row[kind]).convert("RGB")
                   for kind in ["reference", "v4", "v3"]] for row in group]
        column = max(image.width for images in opened for image in images)
        heights = [max(image.height for image in images) + 62 for images in opened]
        sheet = Image.new("RGB", (column * 3 + 64, sum(heights) + 70), "white")
        draw = ImageDraw.Draw(sheet)
        for index, title in enumerate(["Quran.com", "v4 (Lateef)", "v3 (historical 104f049)"]):
            draw.text((16 + index * (column + 16), 18), title, fill="black", font=font)
        top = 70
        for row, images, height in zip(group, opened, heights):
            draw.rectangle((0, top, sheet.width, top + 42), fill="#e5e7eb")
            draw.text((16, top + 6), f"{row['key']}  |  {row['size']}px  |  column {row['width']:g}px  |  DPR {row['dpr']}", fill="black", font=font)
            for index, image in enumerate(images):
                sheet.paste(image, (16 + index * (column + 16), top + 48))
            top += height
        filename = f"sheet-{start // 12 + 1:02d}.png"
        sheet.save(args.output / filename)
        sheets.append({"file": filename, "keys": [row["key"] for row in group]})
    categories = ["Letterforms", "Stroke weight and colour", "Inline sign placement", "End-sign stacking", "Ring and digits", "Word spacing", "Overall line rhythm"]
    score_rows = "".join(
        f"<tr><th>{html.escape(category)}</th><td><input type=number min=1 max=5 step=1 data-field='v4-{index}'></td><td><input type=number min=1 max=5 step=1 data-field='v3-{index}'></td><td><input data-field='notes-{index}'></td></tr>"
        for index, category in enumerate(categories)
    )
    cards = "".join(
        f"<article><h2>Sheet {index + 1}: {html.escape(', '.join(sheet['keys']))}</h2><a href='{sheet['file']}'><img loading=lazy src='{sheet['file']}' alt='Quran.com, v4 and v3 comparison for {html.escape(', '.join(sheet['keys']))}'></a></article>"
        for index, sheet in enumerate(sheets)
    )
    failures = html.escape(json.dumps(report["failures"]))
    page = f"""<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>IndoPak v4 review</title>
<style>body{{font:16px/1.5 system-ui;margin:32px;max-width:1600px}}h1,h2{{line-height:1.2}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:8px;text-align:left}}input{{max-width:100%;width:90%;font:inherit}}img{{width:100%;height:auto}}article{{margin-block:48px}}button{{font:inherit;padding:10px 16px;margin:16px 0}}.warning{{background:#fff3cd;padding:16px}}</style>
<h1>IndoPak v4 review</h1><p class=warning>Human review pending. Blank scores are unreviewed. These sheets establish comparison evidence; they do not confer recitation or printed-edition approval.</p>
<p>{len(captures)} verse comparisons; {len(sheets)} sheets of up to 12. Actual Quran.com single-ayah page; 26px, light theme, 2× DPR. v4 uses 125% size-adjust. v3 uses renderer, parser and ornament from commit 104f049, with import paths and font-family alias adapted for isolated mounting.</p>
<p>Original immutable text stays unchanged. Quran.com served letters/sign encoding can differ. Colours remain each site's theme. Click sheet for full resolution. Capture failures: {failures}.</p>
<p><a href=chromium-layout-report.json>Capture and line-break report</a> · <a href=webkit-layout-report.json>WebKit line-break report</a></p>
<p><label>Reviewer name <input data-field=reviewer></label></p><p><label>Role / qualification <input data-field=qualification></label></p><p><label>Printed edition, if used <input data-field=edition></label></p>
<table><thead><tr><th>Category</th><th>v4 (1–5)</th><th>v3 (1–5)</th><th>Notes / follow-up</th></tr></thead><tbody>{score_rows}</tbody></table><button id=export>Export scores</button><p id=status>No scores submitted.</p>{cards}
<script>
const storageKey='easyquran-indopak-v4-review-20261006';
let saved={{}};try{{saved=JSON.parse(localStorage.getItem(storageKey)||'{{}}')}}catch{{}}
const fields=[...document.querySelectorAll('[data-field]')];
function values(){{return Object.fromEntries(fields.map(field=>[field.dataset.field,field.value]));}}
for(const field of fields){{field.value=saved[field.dataset.field]||'';field.addEventListener('input',()=>localStorage.setItem(storageKey,JSON.stringify(values())));}}
document.querySelector('#export').addEventListener('click',()=>{{
const scores=values();const categories={json.dumps(categories)};const results=categories.map((category,index)=>({{category,v4:scores['v4-'+index]?Number(scores['v4-'+index]):null,v3:scores['v3-'+index]?Number(scores['v3-'+index]):null,notes:scores['notes-'+index]}}));
const complete=results.every(row=>row.v4!==null&&row.v3!==null&&row.v4>=1&&row.v4<=5&&row.v3>=1&&row.v3<=5);
const acceptable=complete&&results.every(row=>row.v4>=3&&row.v4>=row.v3);
const data={{date:new Date().toISOString(),reviewer:scores.reviewer,qualification:scores.qualification,printed_edition:scores.edition,complete,acceptable,results}};
const url=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)+'\\n'],{{type:'application/json'}}));const link=document.createElement('a');link.href=url;link.download='indopak-v4-human-scores.json';link.click();URL.revokeObjectURL(url);document.querySelector('#status').textContent=complete?'Scores exported.':'Incomplete scores exported; review remains open.';
}});
</script></html>"""
    (args.output / "index.html").write_text(page)
    (args.output / "sheets.json").write_text(json.dumps({
        "status": "human_review_pending", "verses": len(captures),
        "sheets": sheets, "categories": categories, "scores": None,
    }, indent=2) + "\n")
    print(json.dumps({"verses": len(captures), "sheets": len(sheets), "status": "human_review_pending"}))


if __name__ == "__main__":
    main()
