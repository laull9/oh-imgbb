//! repository 模块封装 SQLite 读写操作。

use anyhow::Result;
use chrono::Utc;
use imgbb::ibb_spider::{IbbAlbumDetail, IbbProfileReport};
use sqlx::{Row, SqlitePool};

use crate::db::models::{FavoriteInput, FavoriteRecord, ParseTabInput, ParseTabRecord};
use crate::settings::AppSettings;

const SETTINGS_KEY: &str = "app_settings";

/// CachedAlbumRecord 保存相册缓存和解析时间。
pub struct CachedAlbumRecord {
    pub detail: IbbAlbumDetail,
    pub parsed_at: String,
}

/// CachedProfileRecord 保存个人空间缓存和解析时间。
pub struct CachedProfileRecord {
    pub report: IbbProfileReport,
    pub parsed_at: String,
}

/// 返回当前 UTC 时间字符串。
fn now_string() -> String {
    Utc::now().to_rfc3339()
}

/// 确保设置表中存在默认设置。
pub async fn ensure_settings(pool: &SqlitePool, default_settings: &AppSettings) -> Result<()> {
    if load_settings(pool).await?.is_some() {
        return Ok(());
    }

    save_settings(pool, default_settings).await
}

/// 读取应用设置。
pub async fn load_settings(pool: &SqlitePool) -> Result<Option<AppSettings>> {
    let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
        .bind(SETTINGS_KEY)
        .fetch_optional(pool)
        .await?;

    row.map(|row| serde_json::from_str(row.get::<&str, _>("value")).map_err(Into::into))
        .transpose()
}

/// 保存应用设置。
pub async fn save_settings(pool: &SqlitePool, settings: &AppSettings) -> Result<()> {
    let value = serde_json::to_string(settings)?;
    let now = now_string();

    sqlx::query(
        r#"
        INSERT INTO settings (key, value, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
    )
    .bind(SETTINGS_KEY)
    .bind(value)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

/// 读取相册缓存。
pub async fn load_album_cache(
    pool: &SqlitePool,
    album_url: &str,
) -> Result<Option<CachedAlbumRecord>> {
    let row = sqlx::query("SELECT raw_json, parsed_at FROM album_cache WHERE album_url = ?")
        .bind(album_url)
        .fetch_optional(pool)
        .await?;

    row.map(|row| {
        Ok(CachedAlbumRecord {
            detail: serde_json::from_str(row.get::<&str, _>("raw_json"))?,
            parsed_at: row.get("parsed_at"),
        })
    })
    .transpose()
}

/// 保存相册缓存和图片索引。
pub async fn save_album_cache(pool: &SqlitePool, detail: &IbbAlbumDetail) -> Result<String> {
    let now = now_string();
    let raw_json = serde_json::to_string(detail)?;
    let cover_url = detail
        .images
        .first()
        .and_then(|image| image.thumbnail_url.clone())
        .or_else(|| detail.images.first().map(|image| image.image_url.clone()));

    sqlx::query(
        r#"
        INSERT INTO album_cache
          (album_url, title, author_url, cover_url, image_count, raw_json, parsed_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(album_url) DO UPDATE SET
          title = excluded.title,
          author_url = excluded.author_url,
          cover_url = excluded.cover_url,
          image_count = excluded.image_count,
          raw_json = excluded.raw_json,
          parsed_at = excluded.parsed_at
        "#,
    )
    .bind(&detail.url)
    .bind(&detail.title)
    .bind(&detail.author_url)
    .bind(&cover_url)
    .bind(detail.images.len() as i64)
    .bind(raw_json)
    .bind(&now)
    .execute(pool)
    .await?;

    for image in &detail.images {
        sqlx::query(
            r#"
            INSERT INTO image_cache
              (album_url, image_url, thumbnail_url, local_thumbnail_path, filename, sort_index)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(album_url, image_url) DO UPDATE SET
              thumbnail_url = excluded.thumbnail_url,
              local_thumbnail_path = excluded.local_thumbnail_path,
              filename = excluded.filename,
              sort_index = excluded.sort_index
            "#,
        )
        .bind(&detail.url)
        .bind(&image.image_url)
        .bind(&image.thumbnail_url)
        .bind(&image.local_thumbnail_path)
        .bind(&image.filename)
        .bind(image.sort_index as i64)
        .execute(pool)
        .await?;
    }

    Ok(now)
}

/// 读取个人空间缓存。
pub async fn load_profile_cache(
    pool: &SqlitePool,
    profile_url: &str,
) -> Result<Option<CachedProfileRecord>> {
    let row = sqlx::query("SELECT raw_json, parsed_at FROM profile_cache WHERE profile_url = ?")
        .bind(profile_url)
        .fetch_optional(pool)
        .await?;

    row.map(|row| {
        Ok(CachedProfileRecord {
            report: serde_json::from_str(row.get::<&str, _>("raw_json"))?,
            parsed_at: row.get("parsed_at"),
        })
    })
    .transpose()
}

/// 保存个人空间缓存。
pub async fn save_profile_cache(
    pool: &SqlitePool,
    profile_url: &str,
    report: &IbbProfileReport,
) -> Result<String> {
    let now = now_string();
    let raw_json = serde_json::to_string(report)?;

    sqlx::query(
        r#"
        INSERT INTO profile_cache (profile_url, title, album_count, raw_json, parsed_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(profile_url) DO UPDATE SET
          title = excluded.title,
          album_count = excluded.album_count,
          raw_json = excluded.raw_json,
          parsed_at = excluded.parsed_at
        "#,
    )
    .bind(profile_url)
    .bind(profile_url)
    .bind(report.albums.len() as i64)
    .bind(raw_json)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(now)
}

/// 读取收藏列表。
pub async fn list_favorites(pool: &SqlitePool, kind: Option<&str>) -> Result<Vec<FavoriteRecord>> {
    let rows = if let Some(kind) = kind {
        sqlx::query("SELECT * FROM favorites WHERE kind = ? ORDER BY updated_at DESC")
            .bind(kind)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM favorites ORDER BY updated_at DESC")
            .fetch_all(pool)
            .await?
    };

    Ok(rows
        .into_iter()
        .map(|row| FavoriteRecord {
            id: row.get("id"),
            kind: row.get("kind"),
            title: row.get("title"),
            url: row.get("url"),
            cover_url: row.get("cover_url"),
            local_cover_path: row.get("local_cover_path"),
            note: row.get("note"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

/// 保存或更新收藏。
pub async fn save_favorite(pool: &SqlitePool, favorite: &FavoriteInput) -> Result<()> {
    let now = now_string();

    sqlx::query(
        r#"
        INSERT INTO favorites (kind, title, url, cover_url, note, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(url) DO UPDATE SET
          kind = excluded.kind,
          title = excluded.title,
          cover_url = excluded.cover_url,
          note = excluded.note,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(&favorite.kind)
    .bind(&favorite.title)
    .bind(&favorite.url)
    .bind(&favorite.cover_url)
    .bind(&favorite.note)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

/// 删除收藏记录。
pub async fn remove_favorite(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM favorites WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

/// 读取可恢复的解析标签页列表。
pub async fn list_parse_tabs(pool: &SqlitePool) -> Result<Vec<ParseTabRecord>> {
    let rows = sqlx::query("SELECT * FROM parse_tabs ORDER BY sort_index ASC, updated_at ASC")
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| ParseTabRecord {
            tab_key: row.get("tab_key"),
            kind: row.get("kind"),
            title: row.get("title"),
            url: row.get("url"),
            sort_index: row.get("sort_index"),
            active: row.get::<i64, _>("active") != 0,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

/// 保存或更新解析标签页。
pub async fn save_parse_tab(pool: &SqlitePool, tab: &ParseTabInput) -> Result<()> {
    let now = now_string();
    if tab.active {
        clear_active_parse_tab(pool).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO parse_tabs
          (tab_key, kind, title, url, sort_index, active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(tab_key) DO UPDATE SET
          kind = excluded.kind,
          title = excluded.title,
          url = excluded.url,
          sort_index = excluded.sort_index,
          active = excluded.active,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(&tab.tab_key)
    .bind(&tab.kind)
    .bind(&tab.title)
    .bind(&tab.url)
    .bind(tab.sort_index)
    .bind(if tab.active { 1_i64 } else { 0_i64 })
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

/// 删除解析标签页。
pub async fn remove_parse_tab(pool: &SqlitePool, tab_key: &str) -> Result<()> {
    sqlx::query("DELETE FROM parse_tabs WHERE tab_key = ?")
        .bind(tab_key)
        .execute(pool)
        .await?;

    Ok(())
}

/// 设置当前激活的解析标签页。
pub async fn set_active_parse_tab(pool: &SqlitePool, tab_key: Option<&str>) -> Result<()> {
    clear_active_parse_tab(pool).await?;
    if let Some(tab_key) = tab_key {
        sqlx::query("UPDATE parse_tabs SET active = 1, updated_at = ? WHERE tab_key = ?")
            .bind(now_string())
            .bind(tab_key)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// 清空解析标签页激活状态。
async fn clear_active_parse_tab(pool: &SqlitePool) -> Result<()> {
    sqlx::query("UPDATE parse_tabs SET active = 0")
        .execute(pool)
        .await?;

    Ok(())
}
