//! Token primitives + reality credential loading from env.
//!
//! Raw tokens exist only in the admin's clipboard until the first
//! successful registration — the DB stores sha256 hex hashes.
//!
//! Reality creds are pulled from env (`PARTNER_REALITY_*`) and returned
//! verbatim. Per-node xray-reality hot-reload is a deferred follow-up;
//! all nodes currently share the same reality public key.

use rand::RngCore;
use sha2::{Digest, Sha256};

use super::error::RegistrationError;
use super::register::RealityCreds;

/// sha256 hex of `raw` — the DB stores the hash, not the token itself.
pub fn hash_token(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    format!("{:x}", h.finalize())
}

/// Generate a new 32-byte random token, prefixed with `ptkn_`.
/// Used by `partner-cli issue-token`.
pub fn generate_raw_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    format!("ptkn_{hex}")
}

/// Short hex string used as node-id suffix so the same partner can
/// register multiple edges without nameclash.
pub fn short_random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Load reality credentials from env vars. Fails with
/// `RealityNotConfigured` if any of `PARTNER_REALITY_UUID`,
/// `PARTNER_REALITY_PUBLIC_KEY`, or `PARTNER_REALITY_SHORT_ID` is
/// missing or empty.
///
/// `PARTNER_REALITY_UUID` is REQUIRED (not auto-generated) because
/// per-node xray-reality hot-reload is a deferred follow-up: until that
/// lands, every edge-node must share the single UUID configured on the
/// central xray server. A silently-generated UUID would register the
/// node successfully but its VLESS tunnel would fail at runtime.
pub fn load_reality_from_env() -> Result<RealityCreds, RegistrationError> {
    let reality_uuid = std::env::var("PARTNER_REALITY_UUID")
        .map_err(|_| RegistrationError::RealityNotConfigured)?;
    let reality_public_key = std::env::var("PARTNER_REALITY_PUBLIC_KEY")
        .map_err(|_| RegistrationError::RealityNotConfigured)?;
    let reality_short_id = std::env::var("PARTNER_REALITY_SHORT_ID")
        .map_err(|_| RegistrationError::RealityNotConfigured)?;
    let reality_server_name =
        std::env::var("PARTNER_REALITY_SERVER_NAME").unwrap_or_else(|_| "www.samsung.com".into());
    if reality_uuid.is_empty() || reality_public_key.is_empty() || reality_short_id.is_empty() {
        return Err(RegistrationError::RealityNotConfigured);
    }
    Ok(RealityCreds {
        reality_uuid,
        reality_public_key,
        reality_short_id,
        reality_server_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_token_is_stable_and_hex() {
        let h = hash_token("hello");
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(h.len(), 64);
    }

    /// Golden-value parity test — must match partner-cli's hash_token.
    /// If this fails, the two implementations drifted. Fix in lockstep.
    #[test]
    fn hash_token_matches_cli_reference() {
        assert_eq!(
            hash_token("test-token-fixed"),
            "f227298136580b1377d03ef38f996e39bc442f9d1afd48069ea842af5d54cd97"
        );
    }

    #[test]
    fn generated_tokens_are_unique_and_prefixed() {
        let a = generate_raw_token();
        let b = generate_raw_token();
        assert_ne!(a, b);
        assert!(a.starts_with("ptkn_"));
        assert_eq!(a.len(), "ptkn_".len() + 64);
    }

    #[test]
    fn short_random_hex_has_expected_length() {
        assert_eq!(short_random_hex(6).len(), 12);
        assert_ne!(short_random_hex(8), short_random_hex(8));
    }
}
