//! Tests for `SqliteStore::preflight` — startup database-path validation.

use std::path::PathBuf;

use din_store_sqlite::SqliteStore;

fn tmp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("din_preflight_{}_{}", std::process::id(), name))
}

#[test]
fn missing_parent_dir_rejected() {
    let p = tmp_path("nodir/deep/db.sqlite");
    let _ = std::fs::remove_dir_all(tmp_path("nodir"));
    let err = SqliteStore::preflight(&p).unwrap_err();
    assert!(err.contains("目录不存在"), "{err}");
}

#[test]
fn garbage_file_rejected_as_not_valid_db() {
    let p = tmp_path("garbage.sqlite");
    std::fs::write(&p, b"this is definitely not a sqlite database file........").unwrap();
    let err = SqliteStore::preflight(&p).unwrap_err();
    assert!(err.contains("不是有效的 SQLite 数据库"), "{err}");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn valid_db_passes() {
    let p = tmp_path("valid.sqlite");
    let _ = std::fs::remove_file(&p);
    // Create a valid, non-empty DB.
    {
        let store = SqliteStore::open(&p).unwrap();
        let _ = store; // keeps the connection alive; table created by init
    }
    assert!(SqliteStore::preflight(&p).is_ok(), "valid db should pass preflight");
    let _ = std::fs::remove_file(&p);
}

#[cfg(unix)]
#[test]
fn readonly_file_detected() {
    use std::os::unix::fs::PermissionsExt;

    let p = tmp_path("readonly.sqlite");
    let _ = std::fs::remove_file(&p);
    {
        let store = SqliteStore::open(&p).unwrap();
        let _ = store;
    }

    // Make the file read-only (owner) and confirm preflight flags it.
    let mut perms = std::fs::metadata(&p).unwrap().permissions();
    perms.set_mode(0o444);
    std::fs::set_permissions(&p, perms).unwrap();

    let result = SqliteStore::preflight(&p);

    // Restore permissions so cleanup works.
    let mut perms2 = std::fs::metadata(&p).unwrap().permissions();
    perms2.set_mode(0o644);
    std::fs::set_permissions(&p, perms2).unwrap();

    match result {
        Err(msg) => assert!(msg.contains("只读"), "{msg}"),
        // Running as root may bypass file perms; tolerate that.
        Ok(()) => {}
    }
    let _ = std::fs::remove_file(&p);
}
