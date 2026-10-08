use axum::{
    extract::{Query, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use chrono::Duration;
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient},
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    state::SharedState,
    user::{require_user, User},
    util,
};

fn other(e: impl std::fmt::Display) -> AppError {
    AppError::Other(anyhow::anyhow!("{e}"))
}

pub async fn login(State(state): State<SharedState>) -> Result<Redirect, AppError> {
    let metadata = state.provider_metadata().await?.clone();
    let redirect = RedirectUrl::new(state.cfg.redirect_uri())
        .map_err(|e| other(format!("invalid redirect url: {e}")))?;
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(state.cfg.oidc.client_id.clone()),
        Some(ClientSecret::new(state.cfg.oidc.client_secret.clone())),
    )
    .set_redirect_uri(redirect);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("groups".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    sqlx::query(
        "INSERT INTO login_states (state, nonce, pkce_verifier, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(csrf.secret())
    .bind(nonce.secret())
    .bind(pkce_verifier.secret())
    .bind(util::now())
    .execute(&state.db)
    .await?;

    let cutoff = (chrono::Utc::now() - Duration::hours(1)).to_rfc3339();
    sqlx::query("DELETE FROM login_states WHERE created_at < ?")
        .bind(cutoff)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to(auth_url.as_str()))
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn callback(
    State(state): State<SharedState>,
    Query(params): Query<CallbackParams>,
) -> Result<Response, AppError> {
    if let Some(error) = params.error {
        let detail = params.error_description.unwrap_or_default();
        return Err(AppError::BadRequest(
            format!("sign-in failed: {error} {detail}")
                .trim()
                .to_string(),
        ));
    }
    let code = params
        .code
        .ok_or_else(|| AppError::BadRequest("missing code".to_string()))?;
    let csrf = params
        .state
        .ok_or_else(|| AppError::BadRequest("missing state".to_string()))?;

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT nonce, pkce_verifier FROM login_states WHERE state = ?",
    )
    .bind(&csrf)
    .fetch_optional(&state.db)
    .await?;
    let Some((nonce, verifier)) = row else {
        return Err(AppError::BadRequest(
            "unknown or expired sign-in attempt".to_string(),
        ));
    };
    sqlx::query("DELETE FROM login_states WHERE state = ?")
        .bind(&csrf)
        .execute(&state.db)
        .await?;

    let metadata = state.provider_metadata().await?.clone();
    let redirect = RedirectUrl::new(state.cfg.redirect_uri())
        .map_err(|e| other(format!("invalid redirect url: {e}")))?;
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(state.cfg.oidc.client_id.clone()),
        Some(ClientSecret::new(state.cfg.oidc.client_secret.clone())),
    )
    .set_redirect_uri(redirect);

    let nonce = Nonce::new(nonce);
    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|e| other(format!("token request: {e}")))?
        .set_pkce_verifier(PkceCodeVerifier::new(verifier))
        .request_async(&state.http)
        .await
        .map_err(|e| other(format!("token exchange failed: {e}")))?;

    let id_token = token_response
        .id_token()
        .ok_or_else(|| AppError::BadRequest("provider returned no id_token".to_string()))?;
    let verifier = client.id_token_verifier();
    let claims = id_token
        .claims(&verifier, &nonce)
        .map_err(|e| other(format!("id_token verification failed: {e}")))?;

    let sub = claims.subject().as_str().to_string();
    // The signature is verified above; read the extra group claim from the
    // same token's payload.
    let raw = util::decode_jwt_payload(&id_token.to_string())?;
    let groups: Vec<String> = raw
        .get(&state.cfg.oidc.group_claim)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let username = raw
        .get("preferred_username")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let display_name = raw.get("name").and_then(|v| v.as_str()).map(str::to_string);
    let email = claims.email().map(|e| e.to_string());

    let is_admin = state
        .cfg
        .oidc
        .admin_group
        .as_ref()
        .is_some_and(|group| groups.iter().any(|g| g == group))
        || state.cfg.admin_subs.iter().any(|s| s == &sub);

    let now = util::now();
    sqlx::query(
        "INSERT INTO users (sub, username, display_name, email, is_admin, created_at, last_login_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(sub) DO UPDATE SET
             username = excluded.username,
             display_name = excluded.display_name,
             email = excluded.email,
             is_admin = excluded.is_admin,
             last_login_at = excluded.last_login_at",
    )
    .bind(&sub)
    .bind(&username)
    .bind(&display_name)
    .bind(&email)
    .bind(is_admin)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;

    let token = util::random_token();
    let expires = (chrono::Utc::now() + Duration::days(util::SESSION_DAYS)).to_rfc3339();
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_sub, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(util::sha256_hex(&token))
    .bind(&sub)
    .bind(util::now())
    .bind(expires)
    .execute(&state.db)
    .await?;

    let mut response = Redirect::to("/").into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        util::session_cookie(&token, state.cfg.cookie_secure),
    );
    Ok(response)
}

pub async fn logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if let Some(token) = util::cookie(&headers, util::SESSION_COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(util::sha256_hex(&token))
            .execute(&state.db)
            .await?;
    }
    let mut response = Redirect::to("/").into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, util::clear_cookie(state.cfg.cookie_secure));
    Ok(response)
}

pub async fn me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<User>, AppError> {
    Ok(Json(require_user(&state, &headers).await?))
}
