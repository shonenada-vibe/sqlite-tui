use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, types::ValueRef};

pub const PAGE_SIZE: usize = 100;
pub const QUERY_LIMIT: usize = 1_000;

pub struct Database {
    connection: Connection,
    timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct Object {
    pub name: String,
    pub kind: String,
    pub schema: String,
}

#[derive(Default, Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
    pub affected: Option<u64>,
    pub elapsed: Duration,
}

impl Database {
    pub fn open(path: &Path, read_only: bool, create: bool, timeout: Duration) -> Result<Self> {
        let mut flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let connection = Connection::open_with_flags(path, flags).with_context(|| {
            format!(
                "Could not open {} (use --create for a new database)",
                path.display()
            )
        })?;
        Self::configure(connection, timeout)
    }

    pub fn demo(timeout: Duration) -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(include_str!("demo.sql"))?;
        Self::configure(connection, timeout)
    }

    fn configure(connection: Connection, timeout: Duration) -> Result<Self> {
        connection.busy_timeout(timeout.min(Duration::from_secs(2)))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        // Validate the file before switching the terminal into raw mode.
        connection.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))?;
        Ok(Self {
            connection,
            timeout,
        })
    }

    pub fn objects(&self) -> Result<Vec<Object>> {
        let mut stmt = self.connection.prepare(
            "SELECT name, type, coalesce(sql, '') FROM main.sqlite_schema
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
             ORDER BY name COLLATE NOCASE",
        )?;
        Ok(stmt
            .query_map([], |row| {
                Ok(Object {
                    name: row.get(0)?,
                    kind: row.get(1)?,
                    schema: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn browse(&self, name: &str, page: usize) -> Result<QueryResult> {
        self.run(
            &format!(
                "SELECT * FROM main.{} LIMIT {} OFFSET {}",
                quote_identifier(name),
                PAGE_SIZE + 1,
                page.saturating_mul(PAGE_SIZE)
            ),
            PAGE_SIZE,
        )
    }

    pub fn query(&self, sql: &str) -> Result<QueryResult> {
        if sql.trim().is_empty() {
            bail!("Enter a SQL statement first");
        }
        self.run(sql, QUERY_LIMIT)
    }

    pub fn in_transaction(&self) -> bool {
        !self.connection.is_autocommit()
    }

    fn run(&self, sql: &str, limit: usize) -> Result<QueryResult> {
        let start = Instant::now();
        let timeout = self.timeout;
        self.connection
            .progress_handler(1_000, Some(move || start.elapsed() >= timeout))?;
        let result = self.run_inner(sql, limit);
        self.connection.progress_handler(0, None::<fn() -> bool>)?;
        result
            .map(|mut result| {
                result.elapsed = start.elapsed();
                result
            })
            .context("SQL failed (long-running statements are stopped by the query timeout)")
    }

    fn run_inner(&self, sql: &str, limit: usize) -> Result<QueryResult> {
        let mut stmt = self.connection.prepare(sql)?;
        let read_only = stmt.readonly();
        let columns = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let before = self.connection.total_changes();
        if columns.is_empty() {
            stmt.execute([])?;
            return Ok(QueryResult {
                affected: Some(self.connection.total_changes() - before),
                ..Default::default()
            });
        }
        let mut rows = stmt.query([])?;
        let mut result = QueryResult {
            columns,
            ..Default::default()
        };
        while let Some(row) = rows.next()? {
            if result.rows.len() < limit {
                result.rows.push(
                    (0..result.columns.len())
                        .map(|i| row.get_ref(i).map(format_value))
                        .collect::<rusqlite::Result<_>>()?,
                );
            } else {
                result.truncated = true;
                // Writes with RETURNING must reach SQLITE_DONE to complete.
                if read_only {
                    break;
                }
            }
        }
        drop(rows);
        if !read_only {
            result.affected = Some(self.connection.total_changes() - before);
        }
        Ok(result)
    }
}

pub fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn format_value(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned(),
        ValueRef::Blob(v) => format!("<BLOB · {} bytes>", v.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Database {
        Database::demo(Duration::from_secs(2)).unwrap()
    }

    #[test]
    fn browses_demo_and_schema() {
        let db = db();
        assert!(
            db.objects()
                .unwrap()
                .iter()
                .any(|o| o.name == "customers" && o.schema.contains("CREATE TABLE"))
        );
        let result = db.browse("orders", 0).unwrap();
        assert_eq!(result.rows.len(), PAGE_SIZE);
        assert!(result.truncated);
        assert!(!db.browse("orders", 2).unwrap().truncated);
    }

    #[test]
    fn quotes_unusual_identifiers() {
        let db = db();
        db.query("CREATE TABLE \"a\"\"b; --\" (value)").unwrap();
        db.query("INSERT INTO \"a\"\"b; --\" VALUES (42)").unwrap();
        assert_eq!(db.browse("a\"b; --", 0).unwrap().rows[0][0], "42");
    }

    #[test]
    fn handles_values_and_empty_results() {
        let result = db().query("SELECT NULL, 42, 1.5, '你好', x'ff00'").unwrap();
        assert_eq!(
            result.rows[0],
            ["NULL", "42", "1.5", "你好", "<BLOB · 2 bytes>"]
        );
        let empty = db().query("SELECT 1 AS value WHERE 0").unwrap();
        assert_eq!(empty.columns, ["value"]);
        assert!(empty.rows.is_empty());
    }

    #[test]
    fn truncates_reads_but_completes_returning_writes() {
        let db = db();
        db.query("CREATE TABLE numbers (n)").unwrap();
        let result = db.query("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1200) INSERT INTO numbers SELECT x FROM n RETURNING n").unwrap();
        assert!(result.truncated);
        assert_eq!(result.rows.len(), QUERY_LIMIT);
        assert_eq!(result.affected, Some(1200));
        assert_eq!(
            db.query("SELECT count(*) FROM numbers").unwrap().rows[0][0],
            "1200"
        );
    }

    #[test]
    fn rejects_multiple_statements_without_executing_them() {
        let db = db();
        assert!(db.query("DELETE FROM orders; SELECT 1").is_err());
        assert!(db.query("SELECT 1; DELETE FROM orders").is_err());
        assert_eq!(
            db.query("SELECT count(*) FROM orders").unwrap().rows[0][0],
            "240"
        );
    }

    #[test]
    fn supports_transactions_and_recovers_after_errors() {
        let db = db();
        db.query("BEGIN").unwrap();
        assert!(db.in_transaction());
        db.query("DELETE FROM orders").unwrap();
        db.query("ROLLBACK").unwrap();
        assert!(!db.in_transaction());
        assert!(db.query("not sql").is_err());
        assert_eq!(
            db.query("SELECT count(*) FROM orders").unwrap().rows[0][0],
            "240"
        );
    }

    #[test]
    fn interrupts_expensive_queries_and_recovers() {
        let db = Database::demo(Duration::from_millis(1)).unwrap();
        assert!(db.query("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT sum(x) FROM n").is_err());
        assert_eq!(db.query("SELECT 1").unwrap().rows.len(), 1);
    }

    #[test]
    fn only_creates_files_when_requested_and_enforces_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let timeout = Duration::from_secs(2);
        assert!(Database::open(&path, false, false, timeout).is_err());
        assert!(!path.exists());
        let db = Database::open(&path, false, true, timeout).unwrap();
        assert!(db.objects().unwrap().is_empty());
        db.query("CREATE TABLE t (value)").unwrap();
        db.query("INSERT INTO t VALUES ('saved')").unwrap();
        drop(db);
        let read_only = Database::open(&path, true, false, timeout).unwrap();
        assert_eq!(read_only.browse("t", 0).unwrap().rows[0][0], "saved");
        assert!(read_only.query("DELETE FROM t").is_err());
        assert_eq!(read_only.browse("t", 0).unwrap().rows.len(), 1);
    }

    #[test]
    fn closing_a_connection_rolls_back_an_open_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let timeout = Duration::from_secs(2);
        let db = Database::open(&path, false, true, timeout).unwrap();
        db.query("CREATE TABLE t (value)").unwrap();
        db.query("BEGIN").unwrap();
        db.query("INSERT INTO t VALUES (1)").unwrap();
        drop(db);
        assert!(
            Database::open(&path, false, false, timeout)
                .unwrap()
                .browse("t", 0)
                .unwrap()
                .rows
                .is_empty()
        );
    }
}
