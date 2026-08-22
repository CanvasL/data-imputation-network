//! din-node entry point: CLI + HTTP server startup.

use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use din_node::{AppState, router};
use din_store_sqlite::SqliteStore;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "din-node", version, about = "Data Imputation Network — lightweight node")]
struct Args {
    /// Path to the SQLite database file
    #[arg(long, default_value = "din-node.db")]
    db: String,

    /// Listen address, e.g. 127.0.0.1:8080
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt().init();
    let args = Args::parse();

    // Pre-flight the database path so a broken/permission-locked DB produces
    // a clear, actionable message instead of confusing mid-run SQLite errors.
    let db_path = PathBuf::from(&args.db);
    if let Err(msg) = SqliteStore::preflight(&db_path) {
        eprintln!("[din-node] 数据库路径异常，拒绝启动：");
        eprintln!("  {msg}");
        eprintln!("  提示：确认没有其他节点占用该文件；如文件损坏想重新开始，请先停止所有节点再删除它。");
        std::process::exit(2);
    }

    let store = SqliteStore::open(&args.db)?;
    let state = AppState::new(store)?;
    let app = router(state);

    let listener = TcpListener::bind(&args.addr).await?;
    tracing::info!("din-node listening on http://{}", args.addr);
    axum::serve(listener, app).await?;
    Ok(())
}
