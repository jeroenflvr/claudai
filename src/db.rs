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
