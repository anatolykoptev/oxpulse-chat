//! Partner edge-node registration.
//!
//! Single-use bootstrap tokens let a partner's edge-node self-provision
//! via `POST /api/partner/register`: admin issues a token with the
//! `partner-cli` binary, edge-node calls `install.sh --token=...`, the
//! installer hits this endpoint and receives reality + turn credentials.
//!
//! Concerns are split across submodules:
//! - `creds` — token hashing, random generation, reality env loader.
//! - `error` — `RegistrationError` variants + HTTP status mapping.
//! - `register` — SQL transaction + business rules for claiming a token.
//! - `handler` — HTTP surface (request shape, rate limit, IP extraction).

pub mod creds;
pub mod error;
pub mod handler;
pub mod rate_limit;
pub mod register;

pub use creds::{generate_raw_token, hash_token};
pub use error::RegistrationError;
pub use handler::handler;
pub use register::{register, RealityCreds, RegisterRequest, RegistrationOk};
