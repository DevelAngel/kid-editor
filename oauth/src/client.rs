use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client as HttpClient;
use rmcp::transport::auth::{AuthClient, ClientCredentialsConfig, OAuthState};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

/// Builds an [`AuthClient`] authenticated against `mcp_url` via the OAuth
/// 2.0 Client Credentials grant (SEP-1046).
///
/// The returned client discovers the upstream's OAuth metadata, exchanges
/// `client_id`/`client_secret` for an access token, and transparently
/// caches and refreshes that token on subsequent requests - all handled by
/// the `AuthorizationManager` inside `AuthClient`, so callers don't need to
/// track expiry themselves.
pub async fn authenticated_client(
    mcp_url: &Url,
    client_id: &str,
    client_secret: &SecretString,
) -> Result<AuthClient<HttpClient>> {
    // Built explicitly here (instead of letting `OAuthState::new` build its
    // own internally) so a broken TLS setup (e.g. no system CA certificates
    // loaded) surfaces as a concrete "failed to create http client" error
    // instead of the opaque "Internal error: builder error" that
    // `OAuthState::new` would otherwise report.
    let http_client = HttpClient::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("failed to create http client for OAuth communication")?;

    let mut state = OAuthState::new(mcp_url.as_str(), Some(http_client.clone()))
        .await
        .with_context(|| format!("failed to initialize OAuth state for {mcp_url}"))?;
    state
        .authenticate_client_credentials(ClientCredentialsConfig::ClientSecret {
            client_id: client_id.to_owned(),
            client_secret: client_secret.expose_secret().to_owned(),
            scopes: vec![],
            resource: Some(mcp_url.to_string()),
        })
        .await
        .with_context(|| format!("OAuth client credentials authentication failed for {mcp_url}"))?;

    // `into_authorization_manager` only returns `None` for states that
    // can't hold one; `authenticate_client_credentials` just transitioned
    // us into `Authorized`, which always does.
    let auth_manager = state
        .into_authorization_manager()
        .context("failed to get OAuth authorization manager")?;

    // Reuse the same (already-configured) http client for actual MCP
    // traffic instead of building a second, unconfigured one via
    // `HttpClient::default()` - `reqwest::Client` is cheap to clone
    // (internally Arc-wrapped), so this keeps timeout/connection-pool
    // settings consistent between OAuth calls and MCP requests.
    Ok(AuthClient::new(http_client, auth_manager))
}
