#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod handlers;
#[cfg(feature = "server")]
mod store;

#[cfg(feature = "server")]
pub use config::McpClientsConfig;
#[cfg(feature = "server")]
pub use handlers::{
    approve, auth_server, authorize, gen_access_token, protected_resource, validate_access_token,
};
#[cfg(feature = "server")]
pub use store::McpOAuthStore;
