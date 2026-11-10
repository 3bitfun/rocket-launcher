//! Native account authentication and offline session handling.
pub mod accounts;
pub(crate) mod browser;
#[cfg(windows)]
pub(crate) mod credential_store;
mod microsoft;
mod pkce;
mod services;
mod verified_cache;
