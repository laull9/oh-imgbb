//! db 模块负责 SQLite 连接和表结构初始化。

use std::path::Path;

use anyhow::Result;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};

pub mod models;
pub mod repository;

/// 创建 SQLite 连接池。
pub async fn connect(db_path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?)
}

/// 初始化首版需要的数据库表。
pub async fn init_schema(pool: &SqlitePool) -> Result<()> {
    let statements = [
        r#"
        CREATE TABLE IF NOT EXISTS settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS favorites (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          kind TEXT NOT NULL,
          title TEXT NOT NULL,
          url TEXT NOT NULL UNIQUE,
          cover_url TEXT,
          local_cover_path TEXT,
          note TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS album_cache (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          album_url TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          author_url TEXT,
          cover_url TEXT,
          image_count INTEGER NOT NULL,
          raw_json TEXT NOT NULL,
          parsed_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS image_cache (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          album_url TEXT NOT NULL,
          image_url TEXT NOT NULL,
          thumbnail_url TEXT,
          local_thumbnail_path TEXT,
          filename TEXT NOT NULL,
          sort_index INTEGER NOT NULL,
          selected INTEGER NOT NULL DEFAULT 0,
          UNIQUE(album_url, image_url)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS profile_cache (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          profile_url TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          album_count INTEGER NOT NULL,
          raw_json TEXT NOT NULL,
          parsed_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS download_tasks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          target_kind TEXT NOT NULL,
          target_url TEXT NOT NULL,
          status TEXT NOT NULL,
          total_items INTEGER NOT NULL DEFAULT 0,
          finished_items INTEGER NOT NULL DEFAULT 0,
          error_message TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
    ];

    for statement in statements {
        sqlx::query(statement).execute(pool).await?;
    }

    Ok(())
}
