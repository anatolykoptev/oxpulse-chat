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
/// Server name selection: prefers plural `PARTNER_REALITY_SERVER_NAMES`
/// (comma-separated) and picks one deterministically from `node_id`
/// so each partner node gets a stable but differentiated SNI — reduces
/// clustering signal for DPI targeting by (IP, SNI) pair. Falls back
/// to singular `PARTNER_REALITY_SERVER_NAME` for backward compat,
/// then to `"www.samsung.com"` default.
///
/// `PARTNER_REALITY_UUID` is REQUIRED (not auto-generated) because
/// per-node xray-reality hot-reload is a deferred follow-up: until that
/// lands, every edge-node must share the single UUID configured on the
/// central xray server. A silently-generated UUID would register the
/// node successfully but its VLESS tunnel would fail at runtime.
/// Partial reality creds — everything except `reality_server_name` which
/// requires a `node_id` to pick deterministically. Used for pre-tx
/// fail-fast validation; the final pick happens post-commit via
/// [`assemble_reality_creds`] once the DB-returned node_id is known.
pub struct PartialReality {
    pub reality_uuid: String,
    pub reality_public_key: String,
    pub reality_short_id: String,
    pub server_names: Vec<String>,
}

pub fn load_reality_from_env() -> Result<PartialReality, RegistrationError> {
    let reality_uuid = std::env::var("PARTNER_REALITY_UUID")
        .map_err(|_| RegistrationError::RealityNotConfigured)?;
    let reality_public_key = std::env::var("PARTNER_REALITY_PUBLIC_KEY")
        .map_err(|_| RegistrationError::RealityNotConfigured)?;
    let reality_short_id = std::env::var("PARTNER_REALITY_SHORT_ID")
        .map_err(|_| RegistrationError::RealityNotConfigured)?;

    let server_names: Vec<String> = match std::env::var("PARTNER_REALITY_SERVER_NAMES") {
        Ok(s) if !s.trim().is_empty() => s
            .split(",")
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect(),
        _ => vec![
            std::env::var("PARTNER_REALITY_SERVER_NAME").unwrap_or_else(|_| "www.samsung.com".into()),
        ],
    };

    if reality_uuid.is_empty()
        || reality_public_key.is_empty()
        || reality_short_id.is_empty()
        || server_names.is_empty()
    {
        return Err(RegistrationError::RealityNotConfigured);
    }

    Ok(PartialReality {
        reality_uuid,
        reality_public_key,
        reality_short_id,
        server_names,
    })
}

/// Finalize [`RealityCreds`] by picking one `server_name` from the partial
/// list, keyed off the node_id so the same node always gets the same SNI
/// (stability) but different nodes spread across the list (diversification).
pub fn assemble_reality_creds(partial: PartialReality, node_id: &str) -> RealityCreds {
    let reality_server_name = pick_server_name(&partial.server_names, node_id);
    RealityCreds {
        reality_uuid: partial.reality_uuid,
        reality_public_key: partial.reality_public_key,
        reality_short_id: partial.reality_short_id,
        reality_server_name,
    }
}

/// Deterministic pick: sha256(node_id) first byte modulo list length.
/// Same node_id always yields the same server name; different node_ids
/// spread evenly across the list.
fn pick_server_name(names: &[String], node_id: &str) -> String {
    let mut h = Sha256::new();
    h.update(node_id.as_bytes());
    let digest = h.finalize();
    let idx = (digest[0] as usize) % names.len();
    names[idx].clone()
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

#[cfg(test)]
mod server_name_tests {
    use super::*;

    #[test]
    fn pick_is_deterministic() {
        let names = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let id = "rvpn-abc123";
        assert_eq!(pick_server_name(&names, id), pick_server_name(&names, id));
    }

    #[test]
    fn pick_spreads_across_list() {
        let names: Vec<String> = (0..5).map(|i| format!("sni-{i}")).collect();
        let picks: std::collections::HashSet<String> = (0..200)
            .map(|i| pick_server_name(&names, &format!("node-{i}")))
            .collect();
        // 200 random ids spread across 5 names should hit every bucket.
        assert_eq!(picks.len(), 5, "expected all 5 SNIs used at least once");
    }

    #[test]
    fn assemble_overrides_only_server_name() {
        let partial = PartialReality {
            reality_uuid: "u".into(),
            reality_public_key: "p".into(),
            reality_short_id: "s".into(),
            server_names: vec!["one".into(), "two".into(), "three".into()],
        };
        let creds = assemble_reality_creds(partial, "node-x");
        assert_eq!(creds.reality_uuid, "u");
        assert_eq!(creds.reality_public_key, "p");
        assert_eq!(creds.reality_short_id, "s");
        assert!(["one", "two", "three"].contains(&creds.reality_server_name.as_str()));
    }

    #[test]
    fn pick_single_name_list() {
        let names = vec!["only-sni".to_string()];
        for id in ["a", "b", "c", "xyz"] {
            assert_eq!(pick_server_name(&names, id), "only-sni");
        }
    }
}
