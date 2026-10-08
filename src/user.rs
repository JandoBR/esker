use crate::util;

// A logged-in user, as stored in the sessions/users tables.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct User {
    pub sub: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_admin: bool,
}

/// The user for this request, or None when there is no valid session.
pub async fn current_user(
    state: &crate::state::AppState,
    headers: &axum::http::HeaderMap,
) -> Result<Option<User>, crate::error::AppError> {
    let Some(token) = util::cookie(headers, util::SESSION_COOKIE) else {
        return Ok(None);
    };
    let hash = util::sha256_hex(&token);
    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
            String,
        ),
    >(
        "SELECT u.sub, u.username, u.display_name, u.email, u.is_admin, s.expires_at
           FROM sessions s JOIN users u ON u.sub = s.user_sub
          WHERE s.token_hash = ?",
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await?;

    let Some((sub, username, display_name, email, is_admin, expires_at)) = row else {
        return Ok(None);
    };
    if util::is_expired(&expires_at) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&hash)
            .execute(&state.db)
            .await?;
        return Ok(None);
    }
    Ok(Some(User {
        sub,
        username,
        display_name,
        email,
        is_admin,
    }))
}

/// The user for this request, or a 401.
pub async fn require_user(
    state: &crate::state::AppState,
    headers: &axum::http::HeaderMap,
) -> Result<User, crate::error::AppError> {
    current_user(state, headers)
        .await?
        .ok_or(crate::error::AppError::Unauthorized)
}
