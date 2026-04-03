pub struct Config {
    pub port: u16,
    pub bind_address: String,
    pub turn_secret: String,
    pub turn_urls: Vec<String>,
    pub stun_urls: Vec<String>,
    pub cors_origins: Vec<String>,
    pub room_assets_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env("PORT", "3000").parse().expect("PORT must be a number"),
            bind_address: env("BIND_ADDRESS", "0.0.0.0"),
            turn_secret: env("TURN_SECRET", ""),
            turn_urls: csv("TURN_URLS"),
            stun_urls: csv_or("STUN_URLS", "stun:stun.l.google.com:19302"),
            cors_origins: csv_or("CORS_ORIGINS", "*"),
            room_assets_dir: env("ROOM_ASSETS_DIR", "/app/room"),
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
