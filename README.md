# OxPulse Chat

Encrypted 1-on-1 video calls. No account. No tracking. Open source.

## Features

- **Instant calls** — create a room, share the link, start talking
- **Encrypted** — DTLS-SRTP with E2EE verification (emoji codes)
- **Works in Russia** — VLESS/Reality tunnel bypasses DPI
- **No account required** — just click and call
- **Self-hosted** — single binary, <30MB Docker image
- **Zero JS dependencies** — native WebRTC, no external libs

## Quick Start

### Docker (recommended)

```bash
docker run -p 3000:3000 \
  -e TURN_SECRET=your-secret \
  -e TURN_URLS=turns:your-turn-server:5349 \
  ghcr.io/oxpulse-hq/oxpulse-chat:latest
```

Open http://localhost:3000

### From source

```bash
git clone https://github.com/oxpulse-hq/oxpulse-chat
cd oxpulse-chat
make build-web
cargo run --release
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| PORT | 3000 | HTTP listen port |
| BIND_ADDRESS | 0.0.0.0 | Bind address |
| TURN_SECRET | — | Shared secret for coturn |
| TURN_URLS | — | TURN server URLs (comma-separated) |
| STUN_URLS | stun:stun.l.google.com:19302 | STUN URLs |
| CORS_ORIGINS | * | Allowed CORS origins |
| ROOM_ASSETS_DIR | /app/room | Path to SvelteKit build |

## Architecture

```
oxpulse-chat (Rust, ~500 LOC)
├── signaling    WebSocket relay (DashMap rooms, max 2 peers)
├── turn         TURN credential generation (HMAC-SHA1, RFC 5766)
└── server       Axum HTTP/WS, static file serving

web/ (SvelteKit 5)
├── Landing page (create/join room)
└── Call room (WebRTC, Perfect Negotiation pattern)
```

### Running a TURN relay

See [`deploy/turn-node/README.md`](deploy/turn-node/README.md) for the one-command installer that provisions a production-grade coturn partner node on Debian/Ubuntu or RHEL-family VMs.

## License

AGPL-3.0 — see [LICENSE](LICENSE)
