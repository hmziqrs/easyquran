use ruxlog_types::PaginatedList;
use sea_orm::{DatabaseBackend, DbConn, DbErr, FromQueryResult, Statement, Value};

#[derive(Debug, FromQueryResult)]
struct CountRow {
    total: i64,
}

pub async fn paginate_query<T>(
    conn: &DbConn,
    data_sql: &str,
    params: Vec<Value>,
    page: u64,
    per_page: u64,
) -> Result<PaginatedList<T>, DbErr>
where
    T: FromQueryResult + Send + Sync,
{
    let current_page = if page == 0 { 1 } else { page };
    let page_size = if per_page == 0 { 1 } else { per_page };
    let limit: i64 = page_size as i64;
    let offset: i64 = (current_page.saturating_sub(1) * page_size) as i64;

    let count_sql =
        format!("SELECT CAST(COUNT(*) AS INTEGER) AS total FROM (\n{data_sql}\n) AS __ruxlog_inner");
    let count_stmt =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, count_sql, params.clone());
    let total = CountRow::find_by_statement(count_stmt)
        .one(conn)
        .await?
        .map(|row| row.total)
        .unwrap_or(0)
        .max(0) as u64;

    let paged_sql = format!("{data_sql}\nLIMIT ? OFFSET ?");
    let mut paged_params = params;
    paged_params.push(Value::BigInt(Some(limit)));
    paged_params.push(Value::BigInt(Some(offset)));
    let paged_stmt =
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, paged_sql, paged_params);
    let rows = T::find_by_statement(paged_stmt).all(conn).await?;

    Ok(PaginatedList::new(rows, total, current_page, page_size))
}
