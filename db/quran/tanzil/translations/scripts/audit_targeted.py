#!/usr/bin/env python3
"""Targeted confirmation of the known source anomalies + the single mismatch."""
import sqlite3, sys
sys.path.insert(0, 'scripts')
from audit_fidelity import parse_dump, read_sqlite, clean_index_run

def dict_from_rows(rows):
    d = {}
    for r in rows:
        d[(int(r['sura']), int(r['aya']))] = r.get('text')
    return d

def sqlite_dict(tid):
    d = {}
    for idx,su,ay,tx in read_sqlite(f"./sqlite/{tid}.sqlite"):
        d[(su,ay)] = tx
    return d

print("### 1. ku.asan: does STANDARD mode reproduce SQLite exactly? ###")
txt = open('./sql/ku.asan.sql', encoding='utf-8', errors='replace').read()
std = parse_dump(txt, backslash=False)
bs  = parse_dump(txt, backslash=True)
print(f"  backslash parse: {len(bs)} rows, clean_index={clean_index_run(bs)}")
print(f"  standard  parse: {len(std)} rows, clean_index={clean_index_run(std)}")
sdb = sqlite_dict('ku.asan')
for label, rows in [('backslash', bs), ('standard', std)]:
    src = dict_from_rows(rows)
    diff = [k for k in sdb if k in src and sdb[k] != src[k]]
    only_db  = [k for k in sdb if k not in src]
    only_src = [k for k in src if k not in sdb]
    print(f"  [{label}] text_diff={len(diff)} only_db={len(only_db)} only_src={len(only_src)}")
# show the contested char at 46:14 in source (raw bytes around it)
print("  --- ku.asan 46:14 ---")
sv = sdb[(46,14)]
BS = chr(0x5C)  # literal backslash, kept out of the f-string expression
print(f"  sqlite   has literal backslash? {BS in sv}   repr head: {sv[:60]!r}")

print()
print("### 2. fa.safavi 80:39 — known genuinely-empty verse ###")
sdb = sqlite_dict('fa.safavi')
print(f"  sqlite 80:39 text == '' ? {sdb.get((80,39)) == ''!r}   value={sdb.get((80,39))!r}")
# count empty-text verses across fa.safavi
empties = [k for k,v in sdb.items() if v == '']
print(f"  total empty-text verses in fa.safavi sqlite: {len(empties)}  e.g. {empties[:8]}")

print()
print("### 3. de.zaidan — known ~83 C1 control-char mojibake ###")
sdb = sqlite_dict('de.zaidan')
c1 = 0
files_with = 0
sample = None
for k,v in sdb.items():
    if v is None: continue
    bad = [ch for ch in v if 0x80 <= ord(ch) <= 0x9F]  # C1 range
    if bad:
        c1 += len(bad); files_with += 1
        if sample is None: sample = (k, v[:50])
print(f"  C1 control chars in de.zaidan sqlite: {c1} across {files_with} verses")
print(f"  sample verse: {sample}")

print()
print("### 4. ko.korean — known \\xC2 mojibake ###")
sdb = sqlite_dict('ko.korean')
xc2 = 0; verses_with = 0; sample=None
for k,v in sdb.items():
    if v is None: continue
    if '\xc2' in v:
        xc2 += v.count('\xc2'); verses_with += 1
        if sample is None: sample=(k, v[:50])
print(f"  \\xc2 bytes in ko.korean sqlite: {xc2} across {verses_with} verses")
print(f"  sample verse: {sample}")

print()
print("### 5. sq.mehdiu — known stray literal backslash (standard-mode) ###")
sdb = sqlite_dict('sq.mehdiu')
has_bs = [k for k,v in sdb.items() if v and '\\' in v]
print(f"  verses in sq.mehdiu sqlite containing literal backslash: {len(has_bs)}  {has_bs[:5]}")

print()
print("### 6. Full re-fidelity on ku.asan in STANDARD mode == 0 mismatch confirms converter correct ###")
