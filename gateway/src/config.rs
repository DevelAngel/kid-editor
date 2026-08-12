use anyhow::{Result, anyhow};
use derive_more::{Deref, Display, From};
use indexmap::IndexMap;
use secrecy::SecretString;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use url::Url;

/// The name of an upstream MCP server, and the tool-name prefix
/// (`{name}_`) this gateway exposes its tools under - so it must form a
/// valid tool name prefix (ASCII letters, digits, underscore) and be
/// unique across all configured upstreams (enforced by
/// [`UpstreamsConfig::load`], which keys upstreams by this type).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deref, Display, From, Deserialize)]
#[serde(transparent)]
pub struct UpstreamName(String);

/// A single upstream MCP server this gateway aggregates tools from, as read
/// from the TOML file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UpstreamConfig {
    pub name: UpstreamName,
    pub mcp_url: Url,
    pub client_id: String,
    pub secret: SecretString,
}

/// Raw shape of the TOML file - a plain list, since TOML has no way to
/// express `[[upstream]]` as a map keyed by `name`. [`UpstreamsConfig::load`]
/// converts this into the de-duplicated, name-keyed [`UpstreamsConfig`].
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUpstreamsConfig {
    #[serde(rename = "upstream", default)]
    upstreams: Vec<UpstreamConfig>,
}

/// Upstreams keyed by their (unique) [`UpstreamName`], in the order they
/// appeared in the TOML file.
#[derive(Clone, Debug, Default)]
pub struct UpstreamsConfig {
    pub upstreams: IndexMap<UpstreamName, UpstreamConfig>,
}

impl UpstreamsConfig {
    pub fn load(file: &Path) -> Result<Self> {
        let raw = fs::read_to_string(file)?;
        let raw: RawUpstreamsConfig = toml::from_str(&raw)?;

        if raw.upstreams.is_empty() {
            return Err(anyhow!(
                "{} lists no upstreams: the gateway would expose no tools",
                file.display()
            ));
        }

        let mut upstreams = IndexMap::with_capacity(raw.upstreams.len());
        for upstream in raw.upstreams {
            if let Some(previous) = upstreams.insert(upstream.name.clone(), upstream) {
                return Err(anyhow!(
                    "{} lists duplicate upstream name '{}': each `name` must be unique",
                    file.display(),
                    previous.name
                ));
            }
        }

        tracing::info!(
            "{} upstream(s) loaded from {}",
            upstreams.len(),
            file.display()
        );

        Ok(Self { upstreams })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_file_parses() {
        let raw = include_str!("../../gateway.example.toml");
        let config: RawUpstreamsConfig = toml::from_str(raw).unwrap();
        assert_eq!(config.upstreams.len(), 2);
        assert_eq!(
            config.upstreams[0].name,
            UpstreamName::from("editor".to_owned())
        );
        assert_eq!(
            config.upstreams[1].name,
            UpstreamName::from("tasks".to_owned())
        );
    }

    #[test]
    fn load_rejects_duplicate_names() {
        let dir = assert_fs::TempDir::new().unwrap();
        let file = dir.path().join("gateway.toml");
        std::fs::write(
            &file,
            indoc::indoc! {r#"
                [[upstream]]
                name = "editor"
                mcp-url = "http://localhost:9300/mcp"
                client-id = "gateway"
                secret = "s1"

                [[upstream]]
                name = "editor"
                mcp-url = "http://localhost:9301/mcp"
                client-id = "gateway"
                secret = "s2"
            "#},
        )
        .unwrap();

        let error = UpstreamsConfig::load(&file).unwrap_err();
        assert!(error.to_string().contains("duplicate upstream name"));
    }
}
