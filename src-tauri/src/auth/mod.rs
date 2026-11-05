//! Native authentication building blocks. No session is issued by this module yet.
#![allow(dead_code)]
#[cfg(windows)]
mod credential_store;
mod microsoft;
mod pkce;
mod services;
mod verified_cache;
