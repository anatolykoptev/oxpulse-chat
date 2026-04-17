#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnServerCfg {
    pub url: String,
    pub region: String,
    pub priority: i32,
}

pub struct Config {
    pub port: u16,
    pub bind_address: String,
    pub turn_secret: String,
    pub turn_urls: Vec<String>,
    /// Structured TURN server list with region/priority metadata.
    /// Parsed from `TURN_SERVERS` env (see `parse_turn_servers`).
    /// Consumed by `TurnPool` in `main.rs` to drive the probe loop.
    pub turn_servers: Vec<TurnServerCfg>,
    pub turn_probe_interval_secs: u64,
    pub turn_unhealthy_after_fails: u32,
    pub stun_urls: Vec<String>,
    pub cors_origins: Vec<String>,
    pub room_assets_dir: String,
    pub database_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env("PORT", "3000").parse().expect("PORT must be a number"),
            bind_address: env("BIND_ADDRESS", "0.0.0.0"),
            turn_secret: env("TURN_SECRET", ""),
            turn_urls: csv("TURN_URLS"),
            turn_servers: parse_turn_servers(&env("TURN_SERVERS", "")),
            turn_probe_interval_secs: env("TURN_PROBE_INTERVAL_SECS", "30")
                .parse()
                .expect("TURN_PROBE_INTERVAL_SECS must be a number"),
            turn_unhealthy_after_fails: env("TURN_UNHEALTHY_AFTER_FAILS", "3")
                .parse()
                .expect("TURN_UNHEALTHY_AFTER_FAILS must be a number"),
            stun_urls: csv_or("STUN_URLS", "stun:stun.l.google.com:19302"),
            cors_origins: csv_or("CORS_ORIGINS", "*"),
            room_assets_dir: env("ROOM_ASSETS_DIR", "/app/room"),
            database_url: std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty()),
        }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn csv(key: &str) -> Vec<String> {
    std::env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn csv_or(key: &str, default: &str) -> Vec<String> {
    let val = std::env::var(key).unwrap_or_else(|_| default.to_string());
    val.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse `TURN_SERVERS` env into structured `TurnServerCfg` entries.
///
/// Format: comma-separated list of `region:priority:url` triples. The
/// `url` segment itself contains colons (`turn:host:3478`), so we only
/// split on the FIRST two colons and treat the rest as `url` verbatim.
///
/// Malformed chunks are logged via `tracing::warn!` and skipped — the
/// server still starts if one entry is broken.
pub fn parse_turn_servers(s: &str) -> Vec<TurnServerCfg> {
    s.split(',')
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .filter_map(|chunk| {
            let Some((region, rest)) = chunk.split_once(':') else {
                tracing::warn!(
                    entry = chunk,
                    "TURN_SERVERS entry missing region:priority:url separator — skipped"
                );
                return None;
            };
            let Some((priority_str, url)) = rest.split_once(':') else {
                tracing::warn!(
                    entry = chunk,
                    "TURN_SERVERS entry missing priority:url separator — skipped"
                );
                return None;
            };
            let priority = match priority_str.parse::<i32>() {
                Ok(p) => p,
                Err(_) => {
                    tracing::warn!(
                        entry = chunk,
                        priority = priority_str,
                        "TURN_SERVERS entry has non-numeric priority — skipped"
                    );
                    return None;
                }
            };
            if region.is_empty() || url.is_empty() {
                tracing::warn!(
                    entry = chunk,
                    "TURN_SERVERS entry has empty region or url — skipped"
                );
                return None;
            }
            Some(TurnServerCfg {
                url: url.to_string(),
                region: region.to_string(),
                priority,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_triple() {
        let v = parse_turn_servers("ru-msk:0:turn:host1:3478");
        assert_eq!(
            v,
            vec![TurnServerCfg {
                region: "ru-msk".into(),
                priority: 0,
                url: "turn:host1:3478".into(),
            }]
        );
    }

    #[test]
    fn parses_multiple_with_transport_query() {
        let v = parse_turn_servers(
            "ru-msk:0:turn:host1:3478?transport=udp,de-fra:1:turn:host2:3478?transport=tcp",
        );
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].region, "ru-msk");
        assert_eq!(v[0].priority, 0);
        assert_eq!(v[0].url, "turn:host1:3478?transport=udp");
        assert_eq!(v[1].region, "de-fra");
        assert_eq!(v[1].priority, 1);
        assert_eq!(v[1].url, "turn:host2:3478?transport=tcp");
    }

    #[test]
    fn skips_malformed_entries_and_keeps_good_ones() {
        let v = parse_turn_servers(
            "valid:0:turn:x:1,broken-no-priority,another:notanumber:turn:y:2,good:5:turn:z:3",
        );
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].region, "valid");
        assert_eq!(v[1].region, "good");
        assert_eq!(v[1].priority, 5);
    }

    #[test]
    fn empty_string_returns_empty_vec() {
        assert!(parse_turn_servers("").is_empty());
    }
}
