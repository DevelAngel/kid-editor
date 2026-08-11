#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::*;

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::authenticated_client;
