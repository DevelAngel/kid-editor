use anyhow::{Result, anyhow};
use secrecy::SecretString;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use url::Url;

/// A single OAuth client allowed to authenticate against the MCP server,
/// as read from the TOML file.
///
/// `redirect_uri` is required for clients that use the `authorization_code` grant
/// (they're redirected back to it after user approval), but has no meaning
/// for machine clients that only use the `client_credentials` grant
/// - those can leave it unset.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct McpClientConfig {
    pub client_id: String,
    pub secret: SecretString,
    #[serde(default)]
    pub redirect_uri: Option<Url>,
}

/// Top-level shape of the TOML file
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpClientsConfig {
    #[serde(rename = "client", default)]
    pub clients: Vec<McpClientConfig>,
}

impl McpClientsConfig {
    pub fn load(file: &Path) -> Result<Self> {
        let config = fs::read_to_string(file)?;
        let config: Self = toml::from_str(&config)?;

        if config.clients.is_empty() {
            return Err(anyhow!(
                "{} lists no clients: OAuth is disabled, no client can authenticate",
                file.display()
            ));
        }

        if tracing::enabled!(tracing::Level::DEBUG) {
            config.clients.iter().for_each(|c| {
                let client_id = &c.client_id;
                let uri = c
                    .redirect_uri
                    .as_ref()
                    .map(|uri| format!(" with uri {uri}"))
                    .unwrap_or_default();
                tracing::debug!("{client_id}{uri}");
            });
        }
        tracing::info!(
            "{} MCP client(s) loaded from {}",
            config.clients.len(),
            file.display()
        );

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_file_parses() {
        let raw = include_str!("../../../mcp-clients.example.toml");
        let config: McpClientsConfig = toml::from_str(raw).unwrap();
        assert_eq!(config.clients.len(), 3);
        assert_eq!(config.clients[0].client_id, "mcp-inspector");
        assert_eq!(config.clients[1].client_id, "example.ai");
        assert_eq!(config.clients[2].client_id, "kid-agent");
    }
}
