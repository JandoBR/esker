use std::path::PathBuf;

use anyhow::{bail, Context};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub data_dir: PathBuf,
    pub static_dir: PathBuf,
    pub public_url: String,
    pub oidc: OidcConfig,
    pub admin_subs: Vec<String>,
    pub cookie_secure: bool,
    /// 0 means no limit.
    pub max_upload_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub group_claim: String,
    pub admin_group: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = PathBuf::from(env_or("BOARD_DATA_DIR", "./data"));
        let static_dir = PathBuf::from(env_or("BOARD_STATIC_DIR", "./web/dist"));
        let bind_addr = env_or("BOARD_BIND", "0.0.0.0:8080");

        let missing = |name: &str| anyhow::anyhow!("{name} is not set");
        let issuer = std::env::var("OIDC_ISSUER").map_err(|_| missing("OIDC_ISSUER"))?;
        let client_id = std::env::var("OIDC_CLIENT_ID").map_err(|_| missing("OIDC_CLIENT_ID"))?;
        let client_secret =
            std::env::var("OIDC_CLIENT_SECRET").map_err(|_| missing("OIDC_CLIENT_SECRET"))?;
        let public_url =
            std::env::var("BOARD_PUBLIC_URL").map_err(|_| missing("BOARD_PUBLIC_URL"))?;

        let admin_subs = std::env::var("BOARD_ADMIN_SUBS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let cookie_secure = parse_bool(&env_or("BOARD_COOKIE_SECURE", "true"))
            .context("BOARD_COOKIE_SECURE must be true or false")?;
        let max_upload_bytes = env_or("BOARD_MAX_UPLOAD_BYTES", "0")
            .parse::<usize>()
            .context("BOARD_MAX_UPLOAD_BYTES must be a number of bytes (0 = unlimited)")?;

        if public_url.ends_with('/') {
            bail!("BOARD_PUBLIC_URL must not end with a slash");
        }

        Ok(Config {
            bind_addr,
            data_dir,
            static_dir,
            public_url,
            oidc: OidcConfig {
                issuer,
                client_id,
                client_secret,
                group_claim: env_or("OIDC_GROUP_CLAIM", "groups"),
                admin_group: std::env::var("OIDC_ADMIN_GROUP")
                    .ok()
                    .filter(|s| !s.is_empty()),
            },
            admin_subs,
            cookie_secure,
            max_upload_bytes,
        })
    }

    /// The URL the browser is sent to after signing in.
    pub fn redirect_uri(&self) -> String {
        format!("{}/api/auth/callback", self.public_url)
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => bail!("invalid boolean: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_bool;

    #[test]
    fn bools() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("ON").unwrap());
        assert!(!parse_bool("0").unwrap());
        assert!(parse_bool("nope").is_err());
    }
}
