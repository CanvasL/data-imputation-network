//! SQLite-backed append-only event store — an adapter for
//! `din_core::store::EventStore`.
//!
//! The protocol core deliberately does not pick a storage technology; this
//! crate is one concrete pluggable implementation of the `EventStore`
//! port. Events are stored as JSON rows keyed by `event_id`.

use std::path::Path;

use din_core::event::Envelope;
use din_core::store::{EventStore, StoreError};
use rusqlite::{Connection, OpenFlags};

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Io(e.to_string()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(|e| StoreError::Io(e.to_string()))?;
        Self::init(conn)
    }

    /// Pre-flight check of a database path, run at node startup so the user
    /// gets a clear, actionable warning before the server starts instead of
    /// confusing mid-run SQLite errors.
    ///
    /// Checks, in order: parent directory exists & is writable → file is a
    /// valid SQLite database → file is actually writable (read-only probe).
    /// Returns a human-readable Chinese error with advice on failure.
    pub fn preflight(path: &Path) -> Result<(), String> {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        if !dir.exists() {
            return Err(format!(
                "数据库目录不存在：{}。请先创建该目录，或改用其他 --db 路径。",
                dir.display()
            ));
        }
        if !dir.is_dir() {
            return Err(format!("--db 的父路径不是目录：{}", dir.display()));
        }

        if path.exists() {
            // 文件已存在：必须以可读写方式打开，且是有效的 SQLite 数据库
            let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(|e| {
                    format!(
                        "无法以读写方式打开数据库 {}：{e}（文件可能存在或权限不足）",
                        path.display()
                    )
                })?;
            let check: String = conn
                .query_row("PRAGMA quick_check", [], |r| r.get(0))
                .map_err(|e| {
                    format!(
                        "数据库文件不是有效的 SQLite 数据库或已损坏：{}（{e}）。如需重新开始，请先停止节点再删除该文件。",
                        path.display()
                    )
                })?;
            if check != "ok" {
                return Err(format!(
                    "数据库完整性检查未通过：{}（{check}）",
                    path.display()
                ));
            }
            // 实际写探针：检测只读文件系统 / 权限
            conn.execute_batch(
                "BEGIN; \
                 CREATE TABLE IF NOT EXISTS _din_preflight_probe(x INTEGER); \
                 DROP TABLE IF EXISTS _din_preflight_probe; \
                 COMMIT;",
            )
            .map_err(|e| {
                let msg = e.to_string().to_lowercase();
                if msg.contains("readonly") {
                    format!(
                        "数据库文件当前只读（无法写入）：{}。请检查文件权限（chmod 修复），或换一个可写的 --db 路径。",
                        path.display()
                    )
                } else {
                    format!("数据库写入测试失败：{}：{e}", path.display())
                }
            })?;
        } else {
            // 文件还不存在：确认能在目录里创建它
            let probe = dir.join(".din_write_probe");
            match std::fs::write(&probe, b"") {
                Ok(_) => {
                    let _ = std::fs::remove_file(&probe);
                }
                Err(e) => {
                    return Err(format!(
                        "数据库目录不可写：{}（{e}）。请检查目录权限后重试。",
                        dir.display()
                    ));
                }
            }
        }
        Ok(())
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                event_id INTEGER PRIMARY KEY,
                data     TEXT NOT NULL
            );",
        )
        .map_err(|e| StoreError::Io(e.to_string()))?;
        Ok(Self { conn })
    }
}

impl EventStore for SqliteStore {
    fn append(&mut self, env: &Envelope) -> Result<(), StoreError> {
        let data =
            serde_json::to_string(env).map_err(|e| StoreError::Serialization(e.to_string()))?;
        // INSERT OR REPLACE => idempotent per event_id.
        self.conn
            .execute(
                "INSERT OR REPLACE INTO events (event_id, data) VALUES (?1, ?2)",
                rusqlite::params![env.event_id as i64, data],
            )
            .map_err(|e| {
                let msg = e.to_string().to_lowercase();
                if msg.contains("readonly") {
                    StoreError::Io(
                        "数据库处于只读状态（文件可能被替换或删除）。请停止节点后重启——数据仍在文件中，重启不会丢失。"
                            .to_string(),
                    )
                } else {
                    StoreError::Io(e.to_string())
                }
            })?;
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<Envelope>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data FROM events ORDER BY event_id ASC")
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| StoreError::Io(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let data = row.map_err(|e| StoreError::Io(e.to_string()))?;
            let env: Envelope =
                serde_json::from_str(&data).map_err(|e| StoreError::Serialization(e.to_string()))?;
            out.push(env);
        }
        Ok(out)
    }
}
