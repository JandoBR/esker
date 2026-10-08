use std::sync::Arc;

use openidconnect::core::CoreProviderMetadata;
use openidconnect::IssuerUrl;
use sqlx::SqlitePool;
use tokio::sync::OnceCell;

use crate::config::Config;

pub struct AppState {
    pub cfg: Config,
    pub db: SqlitePool,
    pub http: openidconnect::reqwest::Client,
    /// Provider discovery, fetched on first use so the service starts even when
    /// Pocket ID is briefly unreachable.
    pub metadata: OnceCell<CoreProviderMetadata>,
}

impl AppState {
    pub async fn provider_metadata(&self) -> anyhow::Result<&CoreProviderMetadata> {
        self.metadata
            .get_or_try_init(|| async {
                let metadata = CoreProviderMetadata::discover_async(
                    IssuerUrl::new(self.cfg.oidc.issuer.clone())?,
                    &self.http,
                )
                .await?;
                Ok(metadata)
            })
            .await
    }
}

pub type SharedState = Arc<AppState>;
