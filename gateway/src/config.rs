use anyhow::{Result, anyhow};
use secrecy::SecretString;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use url::Url;

/// A single upstream MCP server this gateway aggregates tools from, as read
/// from the TOML file.
///
/// `name` becomes the tool-name prefix used to route calls back to this
/// upstream (e.g. `name = "editor"` exposes its tools as `editor_*`), so it
/// must be unique across all configured upstreams and forms a valid tool
/// name prefix (ASCII letters, digits, underscore).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UpstreamConfig {
    pub name: String,
    pub mcp_url: Url,
    pub client_id: String,
    pub secret: SecretString,
}

/// Top-level shape of the TOML file
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamsConfig {
    #[serde(rename = "upstream", default)]
    pub upstreams: Vec<UpstreamConfig>,
}

impl UpstreamsConfig {
    pub fn load(file: &Path) -> Result<Self> {
        let config = fs::read_to_string(file)?;
        let config: Self = toml::from_str(&config)?;

        if config.upstreams.is_empty() {
            return Err(anyhow!(
                "{} lists no upstreams: the gateway would expose no tools",
                file.display()
            ));
        }

        let mut names: Vec<&str> = config.upstreams.iter().map(|u| u.name.as_str()).collect();
        names.sort_unstable();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(anyhow!(
                "{} lists duplicate upstream names: each `name` must be unique",
                file.display()
            ));
        }

        tracing::info!(
            "{} upstream(s) loaded from {}",
            config.upstreams.len(),
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
        let raw = include_str!("../../gateway.example.toml");
        let config: UpstreamsConfig = toml::from_str(raw).unwrap();
        assert_eq!(config.upstreams.len(), 2);
        assert_eq!(config.upstreams[0].name, "editor");
        assert_eq!(config.upstreams[1].name, "tasks");
    }
}
