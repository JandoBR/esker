use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio_util::compat::FuturesAsyncWriteCompatExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::{error::AppError, state::SharedState, user::require_user, util};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct FileMeta {
    pub id: String,
    pub file_name: String,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    /// When the item disappears; NULL means never.
    pub expires_at: Option<String>,
    /// Display name of the uploader, from the users table.
    pub author: Option<String>,
    /// Attachments, for a file item. Filled in after the row is read.
    #[sqlx(skip)]
    pub files: Vec<FileMeta>,
}

const SELECT_ITEM: &str = "SELECT i.id, i.kind, i.title, i.description, i.body, i.url, \
    i.created_by, i.created_at, i.expires_at, \
    COALESCE(u.display_name, u.username, i.created_by) AS author \
    FROM items i LEFT JOIN users u ON u.sub = i.created_by";

async fn load_files(state: &SharedState, item_id: &str) -> Result<Vec<FileMeta>, AppError> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, file_name, content_type, size FROM files WHERE item_id = ? ORDER BY created_at",
    )
    .bind(item_id)
    .fetch_all(&state.db)
    .await?)
}

async fn fetch_item(state: &SharedState, id: &str) -> Result<Item, AppError> {
    let mut item = sqlx::query_as::<_, Item>(&format!(
        "{SELECT_ITEM} WHERE i.id = ? AND (i.expires_at IS NULL OR i.expires_at > ?)"
    ))
    .bind(id)
    .bind(util::now())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    item.files = load_files(state, id).await?;
    Ok(item)
}

#[derive(sqlx::FromRow)]
struct FileRow {
    item_id: String,
    id: String,
    file_name: String,
    content_type: Option<String>,
    size: Option<i64>,
}

/// One query for every attachment, grouped onto the items it belongs to.
async fn attach_files(state: &SharedState, items: &mut [Item]) -> Result<(), AppError> {
    let rows = sqlx::query_as::<_, FileRow>(
        "SELECT item_id, id, file_name, content_type, size FROM files ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?;
    let mut grouped: HashMap<String, Vec<FileMeta>> = HashMap::new();
    for row in rows {
        grouped.entry(row.item_id).or_default().push(FileMeta {
            id: row.id,
            file_name: row.file_name,
            content_type: row.content_type,
            size: row.size,
        });
    }
    for item in items.iter_mut() {
        if let Some(files) = grouped.remove(&item.id) {
            item.files = files;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct ListParams {
    q: Option<String>,
}

pub async fn list_items(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<Item>>, AppError> {
    require_user(&state, &headers).await?;
    let mut items = match params.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let like = format!("%{q}%");
            sqlx::query_as::<_, Item>(&format!(
                "{SELECT_ITEM} WHERE (i.expires_at IS NULL OR i.expires_at > ?) \
                 AND (i.title LIKE ? OR i.description LIKE ? OR i.body LIKE ?) \
                 ORDER BY i.created_at DESC"
            ))
            .bind(util::now())
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as::<_, Item>(&format!(
                "{SELECT_ITEM} WHERE (i.expires_at IS NULL OR i.expires_at > ?) \
                 ORDER BY i.created_at DESC"
            ))
            .bind(util::now())
            .fetch_all(&state.db)
            .await?
        }
    };
    attach_files(&state, &mut items).await?;
    Ok(Json(items))
}

pub async fn get_item(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Item>, AppError> {
    require_user(&state, &headers).await?;
    Ok(Json(fetch_item(&state, &id).await?))
}

/// An uploaded file, streamed to disk before its row exists.
struct Uploaded {
    id: String,
    file_name: String,
    content_type: Option<String>,
    size: i64,
    path: PathBuf,
}

/// Stream one multipart file field to disk, returning its metadata.
async fn receive_file(
    state: &SharedState,
    files_dir: &std::path::Path,
    field: axum::extract::multipart::Field<'_>,
) -> Result<Uploaded, AppError> {
    let file_name = field.file_name().unwrap_or("file").to_string();
    let content_type = field.content_type().map(str::to_string);
    let id = Uuid::new_v4().to_string();
    let path = files_dir.join(&id);
    let mut file = tokio::fs::File::create(&path).await?;
    let mut field = field;
    let mut size: u64 = 0;
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| AppError::BadRequest(format!("bad upload: {e}")))?
    {
        size += chunk.len() as u64;
        if state.cfg.max_upload_bytes > 0 && size > state.cfg.max_upload_bytes as u64 {
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;
            return Err(AppError::BadRequest("file is too large".to_string()));
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(Uploaded {
        id,
        file_name,
        content_type,
        size: size as i64,
        path,
    })
}

async fn insert_file(state: &SharedState, item_id: &str, file: &Uploaded) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO files (id, item_id, file_name, content_type, size, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&file.id)
    .bind(item_id)
    .bind(&file.file_name)
    .bind(&file.content_type)
    .bind(file.size)
    .bind(util::now())
    .execute(&state.db)
    .await?;
    Ok(())
}

pub async fn create_item(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Item>, AppError> {
    let user = require_user(&state, &headers).await?;

    let id = Uuid::new_v4().to_string();
    let mut kind: Option<String> = None;
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut body: Option<String> = None;
    let mut url: Option<String> = None;
    let mut ttl_seconds: Option<i64> = None;
    let mut uploads: Vec<Uploaded> = Vec::new();

    let files_dir = state.cfg.data_dir.join("files");
    tokio::fs::create_dir_all(&files_dir).await?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("bad upload: {e}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            uploads.push(receive_file(&state, &files_dir, field).await?);
        } else {
            let value = field
                .text()
                .await
                .map_err(|e| AppError::BadRequest(format!("bad field: {e}")))?;
            match name.as_str() {
                "kind" => kind = Some(value),
                "title" => title = Some(value),
                "description" => description = Some(value).filter(|v| !v.trim().is_empty()),
                "body" => body = Some(value),
                "url" => url = Some(value),
                "ttl_seconds" => ttl_seconds = value.trim().parse().ok(),
                _ => {}
            }
        }
    }

    let kind = kind.ok_or_else(|| AppError::BadRequest("kind is required".to_string()))?;
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::BadRequest("title is required".to_string()))?;

    // Files sent for a non-file item are discarded.
    if kind != "file" {
        for upload in uploads.drain(..) {
            let _ = tokio::fs::remove_file(upload.path).await;
        }
    }

    let (body, url) = match kind.as_str() {
        "text" => {
            let body = body.ok_or_else(|| AppError::BadRequest("body is required".to_string()))?;
            (Some(body), None)
        }
        "link" => {
            let url = url.ok_or_else(|| AppError::BadRequest("url is required".to_string()))?;
            validate_url(&url)?;
            (None, Some(url))
        }
        "file" => {
            if uploads.is_empty() {
                return Err(AppError::BadRequest(
                    "at least one file is required".to_string(),
                ));
            }
            (None, None)
        }
        other => return Err(AppError::BadRequest(format!("unknown kind: {other}"))),
    };

    let expires_at = ttl_seconds
        .filter(|seconds| *seconds > 0)
        .map(|seconds| (chrono::Utc::now() + chrono::Duration::seconds(seconds)).to_rfc3339());

    sqlx::query(
        "INSERT INTO items
           (id, kind, title, description, body, url, created_by, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&kind)
    .bind(&title)
    .bind(&description)
    .bind(&body)
    .bind(&url)
    .bind(&user.sub)
    .bind(util::now())
    .bind(&expires_at)
    .execute(&state.db)
    .await?;

    for upload in &uploads {
        insert_file(&state, &id, upload).await?;
    }

    Ok(Json(fetch_item(&state, &id).await?))
}

#[derive(Deserialize)]
pub struct UpdateItem {
    title: Option<String>,
    description: Option<String>,
    body: Option<String>,
    url: Option<String>,
    /// Seconds until it expires; 0 (or omitted) leaves it unchanged, a
    /// negative value clears the expiry.
    expires_in: Option<i64>,
}

pub async fn update_item(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<UpdateItem>,
) -> Result<Json<Item>, AppError> {
    let user = require_user(&state, &headers).await?;
    let item = fetch_item(&state, &id).await?;
    ensure_can_modify(&user, &item)?;

    if let Some(title) = patch.title {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(AppError::BadRequest("title must not be empty".to_string()));
        }
        sqlx::query("UPDATE items SET title = ? WHERE id = ?")
            .bind(title)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(description) = patch.description {
        let description = description.trim().to_string();
        let description = (!description.is_empty()).then_some(description);
        sqlx::query("UPDATE items SET description = ? WHERE id = ?")
            .bind(description)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(body) = patch.body {
        sqlx::query("UPDATE items SET body = ? WHERE id = ?")
            .bind(body)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(url) = patch.url {
        validate_url(&url)?;
        sqlx::query("UPDATE items SET url = ? WHERE id = ?")
            .bind(url)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    if let Some(seconds) = patch.expires_in.filter(|s| *s != 0) {
        let expires_at = if seconds > 0 {
            Some((chrono::Utc::now() + chrono::Duration::seconds(seconds)).to_rfc3339())
        } else {
            None
        };
        sqlx::query("UPDATE items SET expires_at = ? WHERE id = ?")
            .bind(expires_at)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }

    Ok(Json(fetch_item(&state, &id).await?))
}

pub async fn delete_item(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let user = require_user(&state, &headers).await?;
    let item = fetch_item(&state, &id).await?;
    ensure_can_modify(&user, &item)?;

    for file in &item.files {
        let _ = tokio::fs::remove_file(state.cfg.data_dir.join("files").join(&file.id)).await;
    }
    sqlx::query("DELETE FROM items WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Add one or more attachments to an existing file item.
pub async fn add_item_files(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<Item>, AppError> {
    let user = require_user(&state, &headers).await?;
    let item = fetch_item(&state, &id).await?;
    ensure_can_modify(&user, &item)?;
    if item.kind != "file" {
        return Err(AppError::BadRequest(
            "only file items can have attachments".to_string(),
        ));
    }

    let files_dir = state.cfg.data_dir.join("files");
    tokio::fs::create_dir_all(&files_dir).await?;

    let mut added = 0;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("bad upload: {e}")))?
    {
        if field.name() == Some("file") {
            let upload = receive_file(&state, &files_dir, field).await?;
            insert_file(&state, &id, &upload).await?;
            added += 1;
        }
    }
    if added == 0 {
        return Err(AppError::BadRequest("no files provided".to_string()));
    }
    Ok(Json(fetch_item(&state, &id).await?))
}

pub async fn delete_item_file(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((id, file_id)): Path<(String, String)>,
) -> Result<Json<Item>, AppError> {
    let user = require_user(&state, &headers).await?;
    let item = fetch_item(&state, &id).await?;
    ensure_can_modify(&user, &item)?;

    let result = sqlx::query("DELETE FROM files WHERE id = ? AND item_id = ?")
        .bind(&file_id)
        .bind(&id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let _ = tokio::fs::remove_file(state.cfg.data_dir.join("files").join(&file_id)).await;
    Ok(Json(fetch_item(&state, &id).await?))
}

#[derive(Deserialize)]
pub struct FileParams {
    inline: Option<bool>,
}

pub async fn get_item_file(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((id, file_id)): Path<(String, String)>,
    Query(params): Query<FileParams>,
) -> Result<Response, AppError> {
    require_user(&state, &headers).await?;
    serve_by_file_id(&state, &id, &file_id, params.inline.unwrap_or(false)).await
}

/// Every attachment of an item as one zip: the "download all" endpoint.
pub async fn get_item_archive(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    require_user(&state, &headers).await?;
    let item = fetch_item(&state, &id).await?;
    if item.files.is_empty() {
        return Err(AppError::NotFound);
    }

    // The zip is written while the response is read, so a large item never has
    // to fit in memory or in a temporary file.
    let (writer, reader) = tokio::io::duplex(64 * 1024);
    let files_dir = state.cfg.data_dir.join("files");
    let files = item.files.clone();
    let title = item.title.clone();
    tokio::spawn(async move {
        if let Err(error) = write_zip(writer, files_dir, files).await {
            tracing::warn!("archive of \"{title}\": {error}");
        }
    });

    let mut response = Response::new(Body::from_stream(ReaderStream::new(reader)));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    if let Ok(value) = HeaderValue::from_str(&util::content_disposition(
        "attachment",
        &archive_name(&item.title),
    )) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

async fn write_zip(
    writer: tokio::io::DuplexStream,
    files_dir: PathBuf,
    files: Vec<FileMeta>,
) -> anyhow::Result<()> {
    let mut zip = async_zip::tokio::write::ZipFileWriter::with_tokio(writer);
    let mut names = HashSet::new();
    for file in files {
        let entry = async_zip::ZipEntryBuilder::new(
            entry_name(&file.file_name, &mut names).into(),
            compression_for(&file),
        )
        .attribute_compatibility(async_zip::AttributeCompatibility::Unix)
        .unix_permissions(0o644)
        .last_modification_date(chrono::Utc::now().into());
        let mut source = tokio::fs::File::open(files_dir.join(&file.id)).await?;
        let mut sink = zip.write_entry_stream(entry).await?.compat_write();
        tokio::io::copy(&mut source, &mut sink).await?;
        sink.into_inner().close().await?;
    }
    zip.close().await?;
    Ok(())
}

/// Images, video and archives are already compressed, and big files are not
/// worth the CPU: those are stored, everything else is deflated.
fn compression_for(file: &FileMeta) -> async_zip::Compression {
    let compressed = file.content_type.as_deref().is_some_and(|kind| {
        kind.starts_with("image/")
            || kind.starts_with("video/")
            || kind.starts_with("audio/")
            || kind.contains("zip")
            || kind.contains("gzip")
    });
    let too_big = file.size.is_some_and(|size| size > 16 * 1024 * 1024);
    if compressed || too_big {
        async_zip::Compression::Stored
    } else {
        async_zip::Compression::Deflate
    }
}

/// A flat, unique name inside the archive. Uploads keep their own name; two
/// files with the same one get a numeric suffix.
fn entry_name(raw: &str, used: &mut HashSet<String>) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("").trim();
    let base = if base.is_empty() || base == "." || base == ".." {
        "file"
    } else {
        base
    };
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    let path = std::path::Path::new(base);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let extension = path.extension().and_then(|s| s.to_str());
    for n in 2.. {
        let candidate = match extension {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("there is always another name")
}

fn archive_name(title: &str) -> String {
    let stem: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim();
    let stem = if stem.is_empty() { "files" } else { stem };
    format!("{stem}.zip")
}

/// The old single-file URL, kept working: serves the item's first attachment.
pub async fn get_item_file_legacy(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<FileParams>,
) -> Result<Response, AppError> {
    require_user(&state, &headers).await?;
    let file_id: Option<String> = sqlx::query_scalar(
        "SELECT f.id FROM files f JOIN items i ON i.id = f.item_id \
         WHERE f.item_id = ? AND (i.expires_at IS NULL OR i.expires_at > ?) \
         ORDER BY f.created_at LIMIT 1",
    )
    .bind(&id)
    .bind(util::now())
    .fetch_optional(&state.db)
    .await?;
    let file_id = file_id.ok_or(AppError::NotFound)?;
    serve_by_file_id(&state, &id, &file_id, params.inline.unwrap_or(false)).await
}

async fn serve_by_file_id(
    state: &SharedState,
    item_id: &str,
    file_id: &str,
    inline: bool,
) -> Result<Response, AppError> {
    let row = sqlx::query_as::<_, (String, Option<String>, Option<i64>)>(
        "SELECT f.file_name, f.content_type, f.size FROM files f \
         JOIN items i ON i.id = f.item_id \
         WHERE f.id = ? AND f.item_id = ? AND (i.expires_at IS NULL OR i.expires_at > ?)",
    )
    .bind(file_id)
    .bind(item_id)
    .bind(util::now())
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (file_name, content_type, size) = row;

    let path = state.cfg.data_dir.join("files").join(file_id);
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| AppError::NotFound)?;
    let body = Body::from_stream(ReaderStream::new(file));

    let content_type = content_type.unwrap_or_else(|| {
        mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string()
    });
    let disposition = if inline { "inline" } else { "attachment" };

    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    if let Ok(value) = HeaderValue::from_str(&util::content_disposition(disposition, &file_name)) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    if let Some(size) = size {
        if let Ok(value) = HeaderValue::from_str(&size.to_string()) {
            headers.insert(header::CONTENT_LENGTH, value);
        }
    }
    Ok(response.into_response())
}

fn validate_url(url: &str) -> Result<(), AppError> {
    let parsed =
        url::Url::parse(url).map_err(|_| AppError::BadRequest("url is not valid".to_string()))?;
    if matches!(parsed.scheme(), "http" | "https") {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "url must start with http:// or https://".to_string(),
        ))
    }
}

fn ensure_can_modify(user: &crate::user::User, item: &Item) -> Result<(), AppError> {
    if user.is_admin || item.created_by.as_deref() == Some(user.sub.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

/// Delete items past their expiry, along with their files. Reads already hide
/// expired items; this reclaims the rows and the disk.
pub async fn cleanup_expired(state: &SharedState) -> Result<(), AppError> {
    let now = util::now();
    let expired = sqlx::query_scalar::<_, String>(
        "SELECT id FROM items WHERE expires_at IS NOT NULL AND expires_at <= ?",
    )
    .bind(&now)
    .fetch_all(&state.db)
    .await?;

    for item_id in &expired {
        for file in load_files(state, item_id).await? {
            let _ = tokio::fs::remove_file(state.cfg.data_dir.join("files").join(&file.id)).await;
        }
        sqlx::query("DELETE FROM items WHERE id = ?")
            .bind(item_id)
            .execute(&state.db)
            .await?;
    }
    if !expired.is_empty() {
        tracing::info!("removed {} expired item(s)", expired.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{archive_name, entry_name};
    use std::collections::HashSet;

    #[test]
    fn entry_names_are_flat_and_unique() {
        let mut used = HashSet::new();
        assert_eq!(entry_name("../../etc/passwd", &mut used), "passwd");
        assert_eq!(entry_name("shot.png", &mut used), "shot.png");
        assert_eq!(entry_name("shot.png", &mut used), "shot (2).png");
        assert_eq!(entry_name("shot.png", &mut used), "shot (3).png");
        assert_eq!(entry_name("", &mut used), "file");
        assert_eq!(entry_name("..", &mut used), "file (2)");
    }

    #[test]
    fn archive_names_are_safe() {
        assert_eq!(archive_name("Screenshots"), "Screenshots.zip");
        assert_eq!(archive_name(" a/b:c "), "a-b-c.zip");
        assert_eq!(archive_name("   "), "files.zip");
    }
}
