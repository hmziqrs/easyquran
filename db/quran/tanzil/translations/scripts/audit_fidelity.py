#!/usr/bin/env python3
"""
Independent fidelity + structure audit of the converted Tanzil SQLite DBs.

This is deliberately a SEPARATE implementation from the TS converter
(scripts/sql-to-sqlite.ts). It parses each phpMyAdmin MySQL dump itself,
detects the dump's escape mode by an oracle that does NOT consult SQLite
(the `index` column must come out as a strictly contiguous 1..N run), and then
compares the parsed verse text row-for-row against what is stored in the
SQLite output. Agreement between two independent parsers is real evidence of
fidelity; a mismatch is either a converter bug or a known source anomaly.

Usage:
    python3 scripts/audit_fidelity.py
"""
import sqlite3, os, sys, re

SQL_DIR = "./sql"
DB_DIR  = "./sqlite"

# --- MySQL escape decoding (backslash mode) ---
ESC = {'0':'\0','b':'\b','n':'\n','r':'\r','t':'\t','Z':'\x1A',
       '\\':'\\',"'":"'",'"':'"','`':'`'}

def scan_string(s, i, backslash):
    """s[i] == "'". Return (decoded, index_after_closing_quote)."""
    out = []
    i += 1
    n = len(s)
    while i < n:
        c = s[i]
        if c == '\\' and backslash:
            nx = s[i+1] if i+1 < n else ''
            out.append(ESC.get(nx, nx))  # unknown escape -> char verbatim (MySQL)
            i += 2
            continue
        if c == "'":
            if not backslash and i+1 < n and s[i+1] == "'":
                out.append("'"); i += 2; continue
            return ''.join(out), i+1
        out.append(c); i += 1
    return ''.join(out), i  # ran off end (malformed)

def parse_tuple(s, i, ncols, backslash):
    """s[i] == '('. Parse one VALUES tuple. Return (list_of_fields, idx_after)."""
    n = len(s)
    assert s[i] == '('
    i += 1
    fields = []
    while True:
        while i < n and s[i] in ' \t\r\n': i += 1
        if i >= n: return None, i
        c = s[i]
        if c == "'":
            val, i = scan_string(s, i, backslash)
            fields.append(val)
        elif c == '"':
            # double-quoted MySQL string (also has escapes in backslash mode)
            val, i = scan_string(s, i, backslash)  # same scanner works for "
            fields.append(val)
        else:
            # bare token (number / NULL / bool) up to , or )
            j = i
            while j < n and s[j] not in ',)' and s[j] not in ' \t\r\n':
                j += 1
            tok = s[i:j]
            while j < n and s[j] in ' \t\r\n': j += 1
            fields.append(None if tok.upper() == 'NULL' else tok)
            i = j
        while i < n and s[i] in ' \t\r\n': i += 1
        if i < n and s[i] == ',':
            i += 1; continue
        if i < n and s[i] == ')':
            i += 1
            return fields, i
        return None, i  # malformed

def parse_dump(text, backslash):
    """Return list of rows; each row = dict by column name from INSERT header."""
    # strip full-line comments so `()` etc. in comments can't fool the scanner
    lines = text.split('\n')
    kept = []
    for ln in lines:
        st = ln.lstrip(' \t')
        if st.startswith('--') or st.startswith('#'):
            continue
        kept.append(ln)
    s = '\n'.join(kept)
    rows = []
    n = len(s)
    for m in re.finditer(r'INSERT\s+INTO\b', s, re.I):
        p = m.end()
        # table name (optionally backticked; tolerate dotted/hyphenated)
        tm = re.match(r'\s*`?[\w.\-]+`?\s*', s[p:])
        if tm: p += tm.end()
        cols = None
        while p < n and s[p] in ' \t\r\n': p += 1
        if p < n and s[p] == '(':
            # column list
            q = s.find(')', p)
            if q != -1:
                cols = [c.strip().strip('`') for c in s[p+1:q].split(',')]
                p = q+1
        vm = re.search(r'VALUES\b', s[p:], re.I)
        if not vm: continue
        p += vm.end()
        while p < n:
            while p < n and s[p] in ' \t\r\n;': p += 1
            if p >= n or s[p] != '(': break
            flds, p2 = parse_tuple(s, p, len(cols) if cols else 4, backslash)
            if flds is None: break
            p = p2
            if cols:
                row = {cols[k]: flds[k] for k in range(min(len(cols), len(flds)))}
            else:
                row = {('index','sura','aya','text')[k]: flds[k] for k in range(min(4,len(flds)))}
            rows.append(row)
            while p < n and s[p] in ' \t\r\n': p += 1
            if p < n and s[p] == ',': p += 1; continue
            break  # end of this INSERT's value list
    return rows

def clean_index_run(rows):
    """True iff rows carry a strictly contiguous 1..N integer `index`."""
    idx = []
    for r in rows:
        v = r.get('index')
        try:
            idx.append(int(v))
        except (TypeError, ValueError):
            return False
    if not idx: return False
    idx.sort()
    return idx == list(range(1, len(idx)+1))

def best_parse(text):
    b = parse_dump(text, True)
    if clean_index_run(b): return b, 'backslash'
    s = parse_dump(text, False)
    if clean_index_run(s): return s, 'standard'
    # neither clean: return whichever has more rows (flagged later)
    return (b if len(b) >= len(s) else s), ('backslash?' if len(b)>=len(s) else 'standard?')

def read_sqlite(path):
    con = sqlite3.connect(path)
    cur = con.execute('SELECT "index", sura, aya, text FROM quran_text')
    rows = cur.fetchall()
    con.close()
    return rows

def main():
    ids = sorted(os.path.splitext(os.path.basename(f))[0]
                 for f in os.listdir(DB_DIR) if f.endswith('.sqlite'))
    n_files = len(ids)
    struct_bad = []
    parse_fail = []
    total_compared = 0
    total_mismatch_rows = 0
    per_file_mismatch = []
    mode_counts = {}
    integrity_bad = []

    for tid in ids:
        dbp = os.path.join(DB_DIR, tid + '.sqlite')
        sqlp = os.path.join(SQL_DIR, tid + '.sql')
        # integrity + structure
        con = sqlite3.connect(dbp)
        ic = con.execute('PRAGMA integrity_check').fetchone()[0]
        tabs = [r[0] for r in con.execute("SELECT name FROM sqlite_master WHERE type='table'")]
        has_tbl = 'quran_text' in tabs
        cols = []
        rowcount = 0
        if has_tbl:
            cols = [r[1] for r in con.execute('PRAGMA table_info(quran_text)')]
            rowcount = con.execute('SELECT COUNT(*) FROM quran_text').fetchone()[0]
        con.close()
        if ic != 'ok':
            integrity_bad.append((tid, ic))
        if not has_tbl or cols[:4] != ['index','sura','aya','text']:
            struct_bad.append((tid, has_tbl, cols))
        if not os.path.exists(sqlp):
            parse_fail.append((tid, 'no source sql'))
            continue
        with open(sqlp, 'r', encoding='utf-8', errors='replace') as f:
            text = f.read()
        rows, mode = best_parse(text)
        mode_counts[mode] = mode_counts.get(mode, 0) + 1
        if mode.endswith('?'):
            parse_fail.append((tid, mode, len(rows)))
        # build source dict on (sura,aya)
        def to_int(x):
            try: return int(x)
            except: return x
        src = {}
        for r in rows:
            key = (to_int(r.get('sura')), to_int(r.get('aya')))
            src[key] = r.get('text')
        # sqlite dict
        srows = read_sqlite(dbp)
        sdb = {}
        for idx, su, ay, tx in srows:
            sdb[(su, ay)] = tx
        # compare
        only_db = [k for k in sdb if k not in src]
        only_src = [k for k in src if k not in sdb]
        diff = []
        for k in sdb:
            if k in src and sdb[k] != src[k]:
                diff.append(k)
        total_compared += len(sdb)
        if only_db or only_src or diff:
            total_mismatch_rows += len(only_db)+len(only_src)+len(diff)
            per_file_mismatch.append((tid, mode, len(only_db), len(only_src),
                                      len(diff), only_db[:5], only_src[:5], diff[:5]))

    print("="*70)
    print("INDEPENDENT FIDELITY + STRUCTURE AUDIT")
    print("="*70)
    print(f"files scanned                 : {n_files}")
    print(f"PRAGMA integrity_check != ok  : {len(integrity_bad)}  {integrity_bad[:10]}")
    print(f"structure bad (table/cols)    : {len(struct_bad)}  {struct_bad[:10]}")
    print(f"parse mode distribution       : {mode_counts}")
    print(f"files w/ non-clean parse      : {len(parse_fail)}  {parse_fail[:10]}")
    print(f"total verses compared (sqlite): {total_compared}")
    print(f"files w/ ANY row mismatch     : {len(per_file_mismatch)}")
    print(f"total mismatch rows (all kinds): {total_mismatch_rows}")
    print("-"*70)
    # detail on mismatches, truncated
    for (tid, mode, odb, osrc, d, odbk, osrck, diffk) in per_file_mismatch:
        print(f"  [{tid}] mode={mode} only_in_db={odb} only_in_src={osrc} text_diff={d}")
        if odbk: print(f"      only_in_db keys e.g.   : {odbk}")
        if osrck: print(f"      only_in_src keys e.g.  : {osrck}")
        if diffk:
            # show a short sample of the differing text
            for k in diffk[:3]:
                sv = None; pv = None
                try:
                    con = sqlite3.connect(os.path.join(DB_DIR, tid+'.sqlite'))
                    sv = con.execute('SELECT text FROM quran_text WHERE sura=? AND aya=?', k).fetchone()
                    con.close()
                except Exception: pass
                print(f"      diff @ sura={k[0]} aya={k[1]}")
                print(f"          sqlite[:80] : {(sv[0] if sv else None)!r}")
    print("="*70)

if __name__ == '__main__':
    main()
