//! Generic row and key/value access over one connection, plus the JSON
//! field helpers the domain decoders share. Every domain model maps to a
//! [`RowRecord`] through [`RowModel`], and a write lands row and `dirty`
//! flag in one statement.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use super::StoreError;

/// One stored row.
#[derive(Debug, Clone, PartialEq)]
pub struct RowRecord {
    pub id: String,
    pub row: Value,
    pub stamps: Vec<Option<String>>,
    pub dirty: bool,
    pub rejected: bool,
}

/// A domain model stored as rows of one domain.
pub(crate) trait RowModel: Clone + PartialEq {
    const DOMAIN: &'static str;
    fn id(&self) -> &str;
    fn decode(record: &RowRecord) -> Option<Self>;
    fn encode(&self) -> RowRecord;
}

fn record_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<RowRecord> {
    let text: String = row.get(1)?;
    let stamps: String = row.get(2)?;
    Ok(RowRecord {
        id: row.get(0)?,
        row: serde_json::from_str(&text).unwrap_or(Value::Null),
        stamps: serde_json::from_str(&stamps).unwrap_or_default(),
        dirty: row.get::<_, i64>(3)? != 0,
        rejected: row.get::<_, i64>(4)? != 0,
    })
}

pub(crate) fn get(
    conn: &Connection,
    domain: &str,
    id: &str,
) -> Result<Option<RowRecord>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id, row, stamps, dirty, rejected FROM rows WHERE domain = ?1 AND id = ?2",
            params![domain, id],
            record_of,
        )
        .optional()?)
}

pub(crate) fn list(conn: &Connection, domain: &str) -> Result<Vec<RowRecord>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, row, stamps, dirty, rejected FROM rows WHERE domain = ?1 ORDER BY rowid",
    )?;
    let rows = stmt.query_map(params![domain], record_of)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(crate) fn put(conn: &Connection, domain: &str, record: &RowRecord) -> Result<(), StoreError> {
    let stamps = serde_json::to_string(&record.stamps).unwrap_or_else(|_| "[]".into());
    conn.prepare_cached(
        "INSERT INTO rows (domain, id, row, stamps, dirty, rejected) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (domain, id) DO UPDATE SET row = excluded.row, stamps = excluded.stamps,
         dirty = excluded.dirty, rejected = excluded.rejected",
    )?
    .execute(params![
        domain,
        record.id,
        record.row.to_string(),
        stamps,
        i64::from(record.dirty),
        i64::from(record.rejected)
    ])?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, domain: &str, id: &str) -> Result<(), StoreError> {
    conn.prepare_cached("DELETE FROM rows WHERE domain = ?1 AND id = ?2")?
        .execute(params![domain, id])?;
    Ok(())
}

pub(crate) fn delete_domain(conn: &Connection, domain: &str) -> Result<(), StoreError> {
    conn.execute("DELETE FROM rows WHERE domain = ?1", params![domain])?;
    Ok(())
}

/// Clean rows go; dirty rows wait for their account (180-day idle rule).
pub(crate) fn delete_clean(conn: &Connection) -> Result<(), StoreError> {
    conn.execute("DELETE FROM rows WHERE dirty = 0", [])?;
    Ok(())
}

pub(crate) fn count_dirty(conn: &Connection, domain: &str) -> Result<usize, StoreError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM rows WHERE domain = ?1 AND dirty = 1 AND rejected = 0",
        params![domain],
        |r| r.get(0),
    )?;
    Ok(usize::try_from(n).unwrap_or(0))
}

pub(crate) fn count_rejected(conn: &Connection, domain: &str) -> Result<usize, StoreError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM rows WHERE domain = ?1 AND rejected = 1",
        params![domain],
        |r| r.get(0),
    )?;
    Ok(usize::try_from(n).unwrap_or(0))
}

pub(crate) fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .prepare_cached("SELECT value FROM kv WHERE key = ?1")?
        .query_row(params![key], |r| r.get(0))
        .optional()?)
}

pub(crate) fn kv_json(conn: &Connection, key: &str) -> Result<Option<Value>, StoreError> {
    Ok(kv_get(conn, key)?.and_then(|text| serde_json::from_str(&text).ok()))
}

pub(crate) fn kv_set(conn: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
    conn.prepare_cached(
        "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )?
    .execute(params![key, value])?;
    Ok(())
}

pub(crate) fn kv_set_json(conn: &Connection, key: &str, value: &Value) -> Result<(), StoreError> {
    kv_set(conn, key, &value.to_string())
}

pub(crate) fn kv_remove(conn: &Connection, key: &str) -> Result<(), StoreError> {
    conn.prepare_cached("DELETE FROM kv WHERE key = ?1")?
        .execute(params![key])?;
    Ok(())
}

/// Every decodable row of a model's domain, in insertion order.
pub(crate) fn load_all<T: RowModel>(conn: &Connection) -> Result<Vec<T>, StoreError> {
    Ok(list(conn, T::DOMAIN)?
        .iter()
        .filter_map(T::decode)
        .collect())
}

pub(crate) fn save<T: RowModel>(conn: &Connection, row: &T) -> Result<(), StoreError> {
    put(conn, T::DOMAIN, &row.encode())
}

/// Writes what changed between `before` and `after` (ids missing from `after` are deleted).
pub(crate) fn save_diff<T: RowModel>(
    conn: &Connection,
    before: &[T],
    after: &[T],
) -> Result<(), StoreError> {
    for row in after {
        let unchanged = before.iter().any(|old| old.id() == row.id() && old == row);
        if !unchanged {
            save(conn, row)?;
        }
    }
    for old in before {
        if !after.iter().any(|row| row.id() == old.id()) {
            delete(conn, T::DOMAIN, old.id())?;
        }
    }
    Ok(())
}

/// Runs `f` in one transaction on `conn`.
pub(crate) fn in_tx<T>(
    conn: &Connection,
    f: impl FnOnce(&Connection) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let tx = conn.unchecked_transaction()?;
    let out = f(&tx)?;
    tx.commit()?;
    Ok(out)
}

// --- JSON field helpers (the decoders' boundary parsers) ---

pub(crate) fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// A non-empty string, else `None` (absent, null and empty all mean none).
pub(crate) fn opt_str(v: &Value, key: &str) -> Option<String> {
    str_of(v, key).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// A number within `[min, max]`.
pub(crate) fn num_in(v: &Value, key: &str, min: f64, max: f64) -> Option<f64> {
    let n = v.get(key)?.as_f64()?;
    (n.is_finite() && n >= min && n <= max).then_some(n)
}

/// An integer within `[min, max]`.
pub(crate) fn int_in(v: &Value, key: &str, min: i64, max: i64) -> Option<i64> {
    let n = num_in(v, key, min as f64, max as f64)?;
    (n.fract() == 0.0).then_some(n as i64)
}

pub(crate) fn bool_true(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool) == Some(true)
}

/// The object's fields not in `known`.
pub(crate) fn extras_of(v: &Value, known: &[&str]) -> Map<String, Value> {
    v.as_object()
        .map(|o| {
            o.iter()
                .filter(|(k, _)| !known.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn opt_json<T: Into<Value>>(v: Option<T>) -> Value {
    v.map_or(Value::Null, Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_round_trip_with_their_flags() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::migrate(&conn, false).unwrap();
        let rec = RowRecord {
            id: "a".into(),
            row: json!({"x": 1, "future": {"keep": true}}),
            stamps: vec![Some("s".into()), None],
            dirty: true,
            rejected: false,
        };
        put(&conn, "d", &rec).unwrap();
        assert_eq!(get(&conn, "d", "a").unwrap(), Some(rec.clone()));
        assert_eq!(count_dirty(&conn, "d").unwrap(), 1);
        delete_clean(&conn).unwrap();
        assert_eq!(list(&conn, "d").unwrap().len(), 1);
        kv_set(&conn, "k", "v").unwrap();
        assert_eq!(kv_get(&conn, "k").unwrap().as_deref(), Some("v"));
    }
}
