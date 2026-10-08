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
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::{error::AppError, state::SharedState, user::require_user, util};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
    pub created_by: Option<String>,
    pub created_at: String,
    /// Display name of the uploader, from the users table.
    pub author: Option<String>,
}

const SELECT_ITEM: &str = "SELECT i.id, i.kind, i.title, i.description, i.body, i.url, \
    i.file_name, i.content_type, i.size, i.created_by, i.created_at, \
    COALESCE(u.display_name, u.username, i.created_by) AS author \
    FROM items i LEFT JOIN users u ON u.sub = i.created_by";

async fn fetch_item(state: &SharedState, id: &str) -> Result<Item, AppError> {
    sqlx::query_as::<_, Item>(&format!("{SELECT_ITEM} WHERE i.id = ?"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)
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
    let items = match params.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => {
            let like = format!("%{q}%");
            sqlx::query_as::<_, Item>(&format!(
                "{SELECT_ITEM} WHERE (i.title LIKE ? OR i.description LIKE ? OR i.body LIKE ?) \
                 ORDER BY i.created_at DESC"
            ))
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as::<_, Item>(&format!("{SELECT_ITEM} ORDER BY i.created_at DESC"))
                .fetch_all(&state.db)
                .await?
        }
    };
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
    // (original name, content type, size, path on disk)
    let mut uploaded: Option<(String, String, u64, PathBuf)> = None;

    let files_dir = state.cfg.data_dir.join("files");
    tokio::fs::create_dir_all(&files_dir).await?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("bad upload: {e}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let file_name = field.file_name().unwrap_or("file").to_string();
            let content_type = field
                .content_type()
                .map(str::to_string)
                .unwrap_or_else(|| "application/octet-stream".to_string());
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
            uploaded = Some((file_name, content_type, size, path));
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
                _ => {}
            }
        }
    }

    let kind = kind.ok_or_else(|| AppError::BadRequest("kind is required".to_string()))?;
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::BadRequest("title is required".to_string()))?;

    // A file sent for a non-file item is discarded.
    if kind != "file" {
        if let Some((_, _, _, path)) = uploaded.take() {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    let (body, url, file_name, file_path, content_type, size) = match kind.as_str() {
        "text" => {
            let body = body.ok_or_else(|| AppError::BadRequest("body is required".to_string()))?;
            (Some(body), None, None, None, None, None)
        }
        "link" => {
            let url = url.ok_or_else(|| AppError::BadRequest("url is required".to_string()))?;
            let parsed = url::Url::parse(&url)
                .map_err(|_| AppError::BadRequest("url is not valid".to_string()))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(AppError::BadRequest(
                    "url must start with http:// or https://".to_string(),
                ));
            }
            (None, Some(url), None, None, None, None)
        }
        "file" => {
            let (name, content_type, size, _) =
                uploaded.ok_or_else(|| AppError::BadRequest("file is required".to_string()))?;
            (
                None,
                None,
                Some(name),
                Some(id.clone()),
                Some(content_type),
                Some(size as i64),
            )
        }
        other => return Err(AppError::BadRequest(format!("unknown kind: {other}"))),
    };

    sqlx::query(
        "INSERT INTO items
           (id, kind, title, description, body, url, file_name, file_path, content_type, size, created_by, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&kind)
    .bind(&title)
    .bind(&description)
    .bind(&body)
    .bind(&url)
    .bind(&file_name)
    .bind(&file_path)
    .bind(&content_type)
    .bind(size)
    .bind(&user.sub)
    .bind(util::now())
    .execute(&state.db)
    .await?;

    Ok(Json(fetch_item(&state, &id).await?))
}

#[derive(Deserialize)]
pub struct UpdateItem {
    title: Option<String>,
    description: Option<String>,
    body: Option<String>,
    url: Option<String>,
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
        let parsed = url::Url::parse(&url)
            .map_err(|_| AppError::BadRequest("url is not valid".to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::BadRequest(
                "url must start with http:// or https://".to_string(),
            ));
        }
        sqlx::query("UPDATE items SET url = ? WHERE id = ?")
            .bind(url)
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

    sqlx::query("DELETE FROM items WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    if item.kind == "file" {
        if let Some(file_path) = item_id_path(&state, &item) {
            let _ = tokio::fs::remove_file(file_path).await;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct FileParams {
    inline: Option<bool>,
}

pub async fn get_item_file(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<FileParams>,
) -> Result<Response, AppError> {
    require_user(&state, &headers).await?;

    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
        ),
    >("SELECT kind, file_name, file_path, content_type, size FROM items WHERE id = ?")
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (kind, file_name, file_path, content_type, size) = row;

    if kind != "file" {
        return Err(AppError::BadRequest("item has no file".to_string()));
    }
    let file_name = file_name.unwrap_or_else(|| "download".to_string());
    let path = state
        .cfg
        .data_dir
        .join("files")
        .join(file_path.ok_or(AppError::NotFound)?);

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| AppError::NotFound)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = content_type.unwrap_or_else(|| {
        mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string()
    });
    let disposition = if params.inline.unwrap_or(false) {
        "inline"
    } else {
        "attachment"
    };

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

fn ensure_can_modify(user: &crate::user::User, item: &Item) -> Result<(), AppError> {
    if user.is_admin || item.created_by.as_deref() == Some(user.sub.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn item_id_path(state: &SharedState, item: &Item) -> Option<std::path::PathBuf> {
    // Only files have a stored path; the row's file_path mirrors the id.
    (item.kind == "file").then(|| state.cfg.data_dir.join("files").join(&item.id))
}
