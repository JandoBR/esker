use axum::http::{HeaderMap, HeaderValue};
use base64::Engine;
use sha2::{Digest, Sha256};

pub const SESSION_COOKIE: &str = "board_session";
pub const SESSION_DAYS: i64 = 30;

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn is_expired(timestamp: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|t| t < chrono::Utc::now())
        .unwrap_or(true)
}

pub fn random_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Read a cookie value from the request, without pulling in a cookie crate.
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        if let Some((key, value)) = part.trim().split_once('=') {
            if key == name {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn session_cookie(token: &str, secure: bool) -> HeaderValue {
    let mut value = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        SESSION_DAYS * 24 * 60 * 60
    );
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("cookie is valid")
}

pub fn clear_cookie(secure: bool) -> HeaderValue {
    let mut value = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("cookie is valid")
}

/// Decode the payload of a JWT without verifying it. Only ever call this on a
/// token whose signature has already been checked by the OIDC library.
pub fn decode_jwt_payload(jwt: &str) -> anyhow::Result<serde_json::Value> {
    let payload = jwt
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("malformed JWT"))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// A Content-Disposition value safe for arbitrary filenames: an ASCII fallback
/// plus the RFC 5987 UTF-8 form.
pub fn content_disposition(disposition: &str, filename: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    let ascii: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii() && c != '"' && c != '\\' && !c.is_control() {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded = utf8_percent_encode(filename, NON_ALPHANUMERIC).to_string();
    format!("{disposition}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}
