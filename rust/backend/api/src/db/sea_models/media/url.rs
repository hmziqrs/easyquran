use sea_orm::sea_query::{Expr, SimpleExpr};

pub fn build_public_file_url(public_url: &str, bucket: Option<&str>, object_key: &str) -> String {
    let base = public_url.trim_end_matches('/');
    let key = object_key.trim_start_matches('/');

    match bucket.map(str::trim).filter(|b| !b.is_empty()) {
        Some(bucket) => format!("{}/{}/{}", base, bucket.trim_matches('/'), key),
        None => format!("{}/{}", base, key),
    }
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn public_file_url_expr(public_url: &str, table_alias: &str) -> SimpleExpr {
    // Literal (not bind params): custom SQL fragments can't share placeholders across drivers (? vs $1); value is escaped via escape_sql_literal above.
    let base = escape_sql_literal(public_url.trim_end_matches('/'));
    let alias = escape_sql_literal(table_alias);

    Expr::cust(format!(
        "CASE \
            WHEN \"{alias}\".\"bucket\" IS NULL OR \"{alias}\".\"bucket\" = '' \
            THEN CONCAT('{base}', '/', \"{alias}\".\"object_key\") \
            ELSE CONCAT('{base}', '/', \"{alias}\".\"bucket\", '/', \"{alias}\".\"object_key\") \
        END"
    ))
}
