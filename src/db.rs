use duckdb::Connection;
use tracing::info;

pub fn init(path: &str) -> Connection {
    let db = Connection::open(path)
        .unwrap_or_else(|e| panic!("Failed to open DuckDB at {path}: {e}"));
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS turns (
            session_id         VARCHAR NOT NULL,
            turn               INTEGER NOT NULL,
            user_message       TEXT    NOT NULL,
            assistant_response TEXT    NOT NULL,
            created_at         VARCHAR NOT NULL
        );",
    )
    .expect("Failed to create turns table");
    info!("DuckDB open at {path}");
    db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_turns_table() {
        let db = init(":memory:");
        db.execute(
            "INSERT INTO turns VALUES (?, ?, ?, ?, ?)",
            duckdb::params!["sess-1", 1i32, "hello world", "some response", "2024-01-01T00:00:00Z"],
        )
        .expect("insert should succeed");

        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))
            .expect("count query");
        assert_eq!(count, 1);
    }

    #[test]
    fn init_table_creation_is_idempotent() {
        // Two calls to init on separate in-memory DBs should both succeed
        let _db1 = init(":memory:");
        let _db2 = init(":memory:");
    }

    #[test]
    fn session_grouping_query_works() {
        let db = init(":memory:");
        for i in 1i32..=3 {
            db.execute(
                "INSERT INTO turns VALUES (?, ?, ?, ?, ?)",
                duckdb::params![
                    "sess-1", i,
                    format!("user msg {i}"),
                    format!("assistant resp {i}"),
                    format!("2024-01-0{i}T00:00:00Z")
                ],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO turns VALUES (?, ?, ?, ?, ?)",
            duckdb::params!["sess-2", 1i32, "other msg", "other resp", "2024-02-01T00:00:00Z"],
        )
        .unwrap();

        let session_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM (SELECT session_id FROM turns GROUP BY session_id)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(session_count, 2);

        let turn_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM turns WHERE session_id = 'sess-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(turn_count, 3);
    }

    #[test]
    fn turns_ordered_by_turn_number() {
        let db = init(":memory:");
        // Insert out of order
        for i in [3i32, 1, 2] {
            db.execute(
                "INSERT INTO turns VALUES (?, ?, ?, ?, ?)",
                duckdb::params!["sess-x", i, format!("msg {i}"), format!("resp {i}"), format!("2024-01-0{i}T00:00:00Z")],
            )
            .unwrap();
        }
        let mut stmt = db
            .prepare("SELECT turn FROM turns WHERE session_id = 'sess-x' ORDER BY turn ASC")
            .unwrap();
        let turns: Vec<i32> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(turns, vec![1, 2, 3]);
    }
}
