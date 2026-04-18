# OxPulse Chat Analytics + Admin Dashboard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Privacy-preserving analytics (anonymous device_id, no PII) with a Go admin dashboard for the OxPulse Chat ecosystem. Measures viral coefficient: "did the person who received a call create their own room?"

**Architecture:** Two components: (1) Lightweight client-side tracker in SvelteKit sends anonymous events to Rust backend → PostgreSQL. (2) Go admin dashboard (HTMX + Go templates, same pattern as go-nerv) reads from the same DB and serves a real-time metrics UI. Dashboard runs as a separate binary on port 8908.

**Tech Stack:** Rust/Axum (event ingestion), Go (admin dashboard), PostgreSQL (storage), HTMX + Go templates (UI), Chart.js (graphs)

---

## File Structure

### Rust side (oxpulse-chat — event collection)

```
crates/server/src/
├── analytics.rs          # POST /api/event — event ingestion handler
├── config.rs             # +DATABASE_URL env var
├── router.rs             # +route for /api/event
├── main.rs               # +PostgreSQL pool init, +migrations
└── migrate.rs            # Embedded SQL migrations

crates/server/migrations/
└── 001_analytics.sql     # visitor_events table

web/src/lib/
└── tracker.ts            # Client-side anonymous event tracker
```

### Go side (oxpulse-admin — new binary)

```
~/src/oxpulse-admin/
├── cmd/main.go                  # Entry point
├── internal/
│   ├── admin/
│   │   ├── handler.go           # Routes, Handler struct
│   │   ├── auth.go              # HMAC session (copy from go-nerv)
│   │   ├── page_overview.go     # Dashboard overview page
│   │   ├── page_calls.go        # Call analytics page
│   │   ├── page_viral.go        # Viral coefficient page
│   │   ├── page_devices.go      # Device breakdown page
│   │   ├── store.go             # PostgreSQL queries
│   │   ├── templates.go         # Template loading
│   │   └── templates/
│   │       ├── layout.html      # Main layout (sidebar, dark theme)
│   │       ├── login.html       # Login form
│   │       ├── overview.html    # Dashboard with charts
│   │       ├── calls.html       # Call metrics
│   │       ├── viral.html       # Viral funnel
│   │       └── devices.html     # Device breakdown
│   │   └── static/
│   │       ├── htmx.min.js
│   │       ├── chart.min.js     # Chart.js (lightweight)
│   │       └── admin.js         # Sidebar + chart init
│   └── config/
│       └── config.go            # Env-based config
├── go.mod
├── go.sum
├── Makefile
└── Dockerfile
```

---

### Task 1: Add PostgreSQL to oxpulse-chat and create analytics table

**Files:**
- Create: `~/src/oxpulse-chat/crates/server/migrations/001_analytics.sql`
- Create: `~/src/oxpulse-chat/crates/server/src/migrate.rs`
- Modify: `~/src/oxpulse-chat/crates/server/Cargo.toml` — add `sqlx`
- Modify: `~/src/oxpulse-chat/crates/server/src/config.rs` — add `database_url`
- Modify: `~/src/oxpulse-chat/crates/server/src/router.rs` — add pool to AppState
- Modify: `~/src/oxpulse-chat/crates/server/src/main.rs` — init pool, run migrations

- [ ] **Step 1: Add sqlx dependency**

In `crates/server/Cargo.toml`, add to `[dependencies]`:
```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 2: Create migration SQL**

```sql
-- crates/server/migrations/001_analytics.sql

CREATE TABLE IF NOT EXISTS call_events (
    id UUID PRIMARY KEY,
    device_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    room_id TEXT,
    data JSONB NOT NULL DEFAULT '{}',
    country TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_events_device ON call_events (device_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_type ON call_events (event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_room ON call_events (room_id, created_at DESC) WHERE room_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_country ON call_events (country, created_at DESC) WHERE country != '';
```

Event types:
- `room_created` — user created a new room (data: {})
- `room_joined` — user joined an existing room (data: {via: "link"})
- `call_connected` — WebRTC connected (data: {audio_only: bool})
- `call_ended` — call ended (data: {duration_secs: u64})
- `page_view` — landing page visited (data: {referrer: ""})

- [ ] **Step 3: Create migrate.rs**

```rust
// crates/server/src/migrate.rs
use sqlx::PgPool;

const MIGRATION: &str = include_str!("../migrations/001_analytics.sql");

pub async fn run(pool: &PgPool) {
    sqlx::query(MIGRATION)
        .execute(pool)
        .await
        .expect("migration failed");
    tracing::info!("migrations applied");
}
```

- [ ] **Step 4: Add database_url to config.rs**

Add to `Config` struct:
```rust
pub database_url: Option<String>,
```

In `from_env()`:
```rust
database_url: std::env::var("DATABASE_URL").ok(),
```

- [ ] **Step 5: Add pool to AppState**

In `router.rs`, add to `AppState`:
```rust
pub pool: Option<sqlx::PgPool>,
```

- [ ] **Step 6: Init pool in main.rs**

After config loading, before building router:
```rust
let pool = if let Some(ref db_url) = cfg.database_url {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(db_url)
        .await
        .expect("failed to connect to database");
    oxpulse_chat::migrate::run(&pool).await;
    Some(pool)
} else {
    tracing::warn!("DATABASE_URL not set — analytics disabled");
    None
};
```

Add `pool` to AppState construction. Update `lib.rs` to expose `pub mod migrate;`.

- [ ] **Step 7: Verify it compiles with pool as Option**

```bash
cd ~/src/oxpulse-chat && cargo check --workspace
```

Pool is Optional — app still works without a database (call-only mode).

- [ ] **Step 8: Commit**

```bash
git add crates/server/ && git commit -m "feat: add PostgreSQL support for analytics (optional)"
```

---

### Task 2: Event ingestion endpoint

**Files:**
- Create: `~/src/oxpulse-chat/crates/server/src/analytics.rs`
- Modify: `~/src/oxpulse-chat/crates/server/src/router.rs` — add route
- Modify: `~/src/oxpulse-chat/crates/server/src/lib.rs` — add module

- [ ] **Step 1: Create analytics.rs**

```rust
// crates/server/src/analytics.rs
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::router::AppState;

#[derive(Deserialize)]
pub struct EventBatch {
    #[serde(rename = "did")]
    pub device_id: String,
    pub events: Vec<Event>,
}

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "r")]
    pub room_id: Option<String>,
    #[serde(rename = "d", default)]
    pub data: serde_json::Value,
}

pub async fn ingest(
    State(state): State<AppState>,
    Json(batch): Json<EventBatch>,
) -> StatusCode {
    let pool = match &state.pool {
        Some(p) => p,
        None => return StatusCode::NO_CONTENT,
    };

    if batch.device_id.is_empty() || batch.device_id.len() > 64 {
        return StatusCode::BAD_REQUEST;
    }
    if batch.events.is_empty() || batch.events.len() > 20 {
        return StatusCode::BAD_REQUEST;
    }

    for event in &batch.events {
        let id = uuid::Uuid::new_v4();
        let _ = sqlx::query(
            "INSERT INTO call_events (id, device_id, event_type, room_id, data, created_at) \
             VALUES ($1, $2, $3, $4, $5, now())"
        )
        .bind(id)
        .bind(&batch.device_id)
        .bind(&event.event_type)
        .bind(&event.room_id)
        .bind(&event.data)
        .execute(pool)
        .await;
    }

    StatusCode::NO_CONTENT
}
```

- [ ] **Step 2: Add route in router.rs**

Add to `build_router()`, before `.with_state(state)`:
```rust
.route("/api/event", post(crate::analytics::ingest))
```

- [ ] **Step 3: Add module to lib.rs**

```rust
pub mod analytics;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p oxpulse-chat
```

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/ && git commit -m "feat: POST /api/event — anonymous event ingestion"
```

---

### Task 3: Client-side anonymous tracker

**Files:**
- Create: `~/src/oxpulse-chat/web/src/lib/tracker.ts`
- Modify: `~/src/oxpulse-chat/web/src/lib/useCall.svelte.ts` — track call events
- Modify: `~/src/oxpulse-chat/web/src/routes/+page.svelte` — track page view + room creation
- Modify: `~/src/oxpulse-chat/web/src/routes/[roomId]/+page.svelte` — track room join

- [ ] **Step 1: Create tracker.ts**

```typescript
// web/src/lib/tracker.ts

/** Anonymous device ID — random UUID stored in localStorage. No fingerprinting. */
function getDeviceId(): string {
  const key = 'ox_did';
  let id = localStorage.getItem(key);
  if (!id) {
    id = crypto.randomUUID();
    localStorage.setItem(key, id);
  }
  return id;
}

let queue: Array<{ e: string; r?: string; d?: Record<string, unknown> }> = [];
let timer: ReturnType<typeof setTimeout> | null = null;

export function track(eventType: string, roomId?: string, data?: Record<string, unknown>) {
  queue.push({ e: eventType, r: roomId, d: data });
  if (!timer) {
    timer = setTimeout(flush, 2000);
  }
}

function flush() {
  timer = null;
  if (queue.length === 0) return;

  const payload = JSON.stringify({ did: getDeviceId(), events: queue });
  queue = [];

  if (navigator.sendBeacon) {
    navigator.sendBeacon('/api/event', new Blob([payload], { type: 'application/json' }));
  } else {
    fetch('/api/event', { method: 'POST', body: payload, headers: { 'Content-Type': 'application/json' }, keepalive: true }).catch(() => {});
  }
}

// Flush on page unload
if (typeof window !== 'undefined') {
  window.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') flush();
  });
}
```

- [ ] **Step 2: Track events in useCall.svelte.ts**

At the top of the file, add import:
```typescript
import { track } from './tracker';
```

In `init()`, after `status = 'waiting'`:
```typescript
track('room_joined', opts.roomId, { via: document.referrer ? 'link' : 'direct' });
```

In the `onConnectionState` callback, inside `if (state === 'connected')`:
```typescript
track('call_connected', opts.roomId, { audio_only: !videoEnabled });
```

In `hangup()`, before `status = 'ended'`:
```typescript
if (elapsed > 0) track('call_ended', opts.roomId, { duration_secs: elapsed });
```

- [ ] **Step 3: Track page view + room creation on landing page**

In `web/src/routes/+page.svelte`, add import:
```typescript
import { track } from '$lib/tracker';
```

In the `$effect` block (after `mounted = true`):
```typescript
track('page_view', undefined, { referrer: document.referrer || '' });
```

In `createRoom()`, before `goto`:
```typescript
track('room_created', roomId);
```

- [ ] **Step 4: Track room join on call page**

In `web/src/routes/[roomId]/+page.svelte`, add import and track when the page loads. Check the file — the room_joined event is already fired from useCall, so no double-tracking needed here.

- [ ] **Step 5: Build frontend**

```bash
cd ~/src/oxpulse-chat/web && npm run build
```

- [ ] **Step 6: Commit**

```bash
cd ~/src/oxpulse-chat && git add web/src/ && git commit -m "feat: anonymous client-side tracker (device_id + 5 event types)"
```

---

### Task 4: Go admin dashboard — scaffold

**Files:**
- Create: `~/src/oxpulse-admin/` (new project)
- Create: All files listed in the Go side file structure above

- [ ] **Step 1: Init Go module**

```bash
mkdir -p ~/src/oxpulse-admin/{cmd,internal/{admin/templates,admin/static,config}}
cd ~/src/oxpulse-admin
go mod init github.com/anatolykoptev/oxpulse-admin
```

- [ ] **Step 2: Create config.go**

```go
// internal/config/config.go
package config

import "os"

type Config struct {
    Port        string
    DatabaseURL string
    Username    string
    Password    string
    HMACSecret  string
}

func Load() *Config {
    return &Config{
        Port:        envOr("PORT", "8908"),
        DatabaseURL: envOr("DATABASE_URL", ""),
        Username:    envOr("ADMIN_USER", "admin"),
        Password:    envOr("ADMIN_PASS", ""),
        HMACSecret:  envOr("HMAC_SECRET", "change-me"),
    }
}

func envOr(key, def string) string {
    if v := os.Getenv(key); v != "" {
        return v
    }
    return def
}
```

- [ ] **Step 3: Copy auth.go from go-nerv**

Copy `$HOME/src/go-nerv/internal/admin/auth.go` into `internal/admin/auth.go`.

Change the package constants at the top. The file is self-contained (HMAC session, requireAuth middleware, login/logout handlers). No changes needed except package name should already be `admin`.

- [ ] **Step 4: Create templates.go**

```go
// internal/admin/templates.go
package admin

import (
    "embed"
    "html/template"
    "io"
    "net/http"
)

//go:embed templates/*.html
var templateFS embed.FS

//go:embed static/*
var staticFS embed.FS

var templates *template.Template

func init() {
    templates = template.Must(template.ParseFS(templateFS, "templates/*.html"))
}

func Render(w io.Writer, name string, data any) error {
    return templates.ExecuteTemplate(w, name, data)
}

func isHTMX(r *http.Request) bool {
    return r.Header.Get("HX-Request") == "true"
}
```

- [ ] **Step 5: Create handler.go with routes**

```go
// internal/admin/handler.go
package admin

import (
    "net/http"
    "time"

    "github.com/jackc/pgx/v5/pgxpool"
)

const (
    sessionCookie = "oxadmin_session"
    sessionTTL    = 24 * time.Hour
)

type Handler struct {
    pool     *pgxpool.Pool
    username string
    password string
    hmacKey  []byte
}

func New(pool *pgxpool.Pool, username, password, hmacSecret string) *Handler {
    return &Handler{
        pool:     pool,
        username: username,
        password: password,
        hmacKey:  []byte(hmacSecret),
    }
}

func (h *Handler) RegisterRoutes(mux *http.ServeMux) {
    mux.HandleFunc("GET /admin/login", h.handleLogin)
    mux.HandleFunc("POST /admin/login", h.handleLogin)
    mux.HandleFunc("GET /admin/logout", h.handleLogout)

    mux.Handle("GET /admin/static/", http.StripPrefix("/admin/static/",
        http.FileServerFS(staticFS)))

    mux.HandleFunc("GET /admin/", h.requireAuth(h.handleOverview))
    mux.HandleFunc("GET /admin/calls", h.requireAuth(h.handleCalls))
    mux.HandleFunc("GET /admin/viral", h.requireAuth(h.handleViral))
    mux.HandleFunc("GET /admin/devices", h.requireAuth(h.handleDevices))
}
```

- [ ] **Step 6: Create store.go with analytics queries**

```go
// internal/admin/store.go
package admin

import (
    "context"
    "time"

    "github.com/jackc/pgx/v5/pgxpool"
)

type OverviewStats struct {
    TotalCalls     int
    TotalDevices   int
    TotalRooms     int
    CallsToday     int
    AvgDurationSec float64
}

type DailyStat struct {
    Date  time.Time
    Count int
}

type ViralFunnel struct {
    PageViews    int // visited landing
    RoomsCreated int // created a room
    CallsStarted int // call_connected
    Repeat       int // devices with 2+ room_created
}

func getOverview(ctx context.Context, pool *pgxpool.Pool) (*OverviewStats, error) {
    s := &OverviewStats{}
    row := pool.QueryRow(ctx, `
        SELECT
            count(*) FILTER (WHERE event_type = 'call_connected'),
            count(DISTINCT device_id),
            count(DISTINCT room_id) FILTER (WHERE room_id IS NOT NULL),
            count(*) FILTER (WHERE event_type = 'call_connected' AND created_at > now() - interval '24 hours'),
            coalesce(avg((data->>'duration_secs')::int) FILTER (WHERE event_type = 'call_ended' AND data->>'duration_secs' IS NOT NULL), 0)
        FROM call_events
    `)
    err := row.Scan(&s.TotalCalls, &s.TotalDevices, &s.TotalRooms, &s.CallsToday, &s.AvgDurationSec)
    return s, err
}

func getDailyStats(ctx context.Context, pool *pgxpool.Pool, eventType string, days int) ([]DailyStat, error) {
    rows, err := pool.Query(ctx, `
        SELECT date_trunc('day', created_at)::date AS d, count(*)
        FROM call_events
        WHERE event_type = $1 AND created_at > now() - make_interval(days => $2)
        GROUP BY d ORDER BY d
    `, eventType, days)
    if err != nil {
        return nil, err
    }
    defer rows.Close()
    var stats []DailyStat
    for rows.Next() {
        var s DailyStat
        if err := rows.Scan(&s.Date, &s.Count); err != nil {
            return nil, err
        }
        stats = append(stats, s)
    }
    return stats, nil
}

func getViralFunnel(ctx context.Context, pool *pgxpool.Pool, days int) (*ViralFunnel, error) {
    f := &ViralFunnel{}
    row := pool.QueryRow(ctx, `
        SELECT
            count(*) FILTER (WHERE event_type = 'page_view'),
            count(*) FILTER (WHERE event_type = 'room_created'),
            count(*) FILTER (WHERE event_type = 'call_connected'),
            (SELECT count(*) FROM (
                SELECT device_id FROM call_events
                WHERE event_type = 'room_created' AND created_at > now() - make_interval(days => $1)
                GROUP BY device_id HAVING count(*) >= 2
            ) sub)
        FROM call_events
        WHERE created_at > now() - make_interval(days => $1)
    `, days)
    err := row.Scan(&f.PageViews, &f.RoomsCreated, &f.CallsStarted, &f.Repeat)
    return f, err
}
```

- [ ] **Step 7: Create page handlers (stubs)**

```go
// internal/admin/page_overview.go
package admin

import "net/http"

func (h *Handler) handleOverview(w http.ResponseWriter, r *http.Request) {
    stats, err := getOverview(r.Context(), h.pool)
    if err != nil {
        http.Error(w, err.Error(), 500)
        return
    }
    daily, _ := getDailyStats(r.Context(), h.pool, "call_connected", 30)
    _ = Render(w, "overview", map[string]any{
        "Stats": stats, "Daily": daily, "Active": "overview",
    })
}
```

```go
// internal/admin/page_calls.go
package admin

import "net/http"

func (h *Handler) handleCalls(w http.ResponseWriter, r *http.Request) {
    daily, _ := getDailyStats(r.Context(), h.pool, "call_connected", 30)
    ended, _ := getDailyStats(r.Context(), h.pool, "call_ended", 30)
    _ = Render(w, "calls", map[string]any{
        "Connected": daily, "Ended": ended, "Active": "calls",
    })
}
```

```go
// internal/admin/page_viral.go
package admin

import "net/http"

func (h *Handler) handleViral(w http.ResponseWriter, r *http.Request) {
    funnel, _ := getViralFunnel(r.Context(), h.pool, 30)
    _ = Render(w, "viral", map[string]any{
        "Funnel": funnel, "Active": "viral",
    })
}
```

```go
// internal/admin/page_devices.go
package admin

import "net/http"

func (h *Handler) handleDevices(w http.ResponseWriter, r *http.Request) {
    // Top devices by activity
    rows, _ := h.pool.Query(r.Context(), `
        SELECT device_id,
            count(*) FILTER (WHERE event_type = 'room_created') AS rooms,
            count(*) FILTER (WHERE event_type = 'call_connected') AS calls,
            min(created_at) AS first_seen,
            max(created_at) AS last_seen
        FROM call_events
        GROUP BY device_id
        ORDER BY calls DESC
        LIMIT 50
    `)
    defer rows.Close()

    type Device struct {
        ID        string
        Rooms     int
        Calls     int
        FirstSeen string
        LastSeen  string
    }
    var devices []Device
    for rows.Next() {
        var d Device
        var first, last interface{}
        _ = rows.Scan(&d.ID, &d.Rooms, &d.Calls, &first, &last)
        d.ID = d.ID[:8] + "..." // truncate for privacy
        devices = append(devices, d)
    }
    _ = Render(w, "devices", map[string]any{
        "Devices": devices, "Active": "devices",
    })
}
```

- [ ] **Step 8: Create main.go**

```go
// cmd/main.go
package main

import (
    "context"
    "log"
    "net/http"
    "os"
    "os/signal"
    "time"

    "github.com/anatolykoptev/oxpulse-admin/internal/admin"
    "github.com/anatolykoptev/oxpulse-admin/internal/config"
    "github.com/jackc/pgx/v5/pgxpool"
)

func main() {
    cfg := config.Load()

    pool, err := pgxpool.New(context.Background(), cfg.DatabaseURL)
    if err != nil {
        log.Fatalf("db: %v", err)
    }
    defer pool.Close()

    h := admin.New(pool, cfg.Username, cfg.Password, cfg.HMACSecret)
    mux := http.NewServeMux()
    h.RegisterRoutes(mux)

    mux.HandleFunc("GET /", func(w http.ResponseWriter, r *http.Request) {
        http.Redirect(w, r, "/admin/", http.StatusFound)
    })

    srv := &http.Server{Addr: ":" + cfg.Port, Handler: mux}

    go func() {
        log.Printf("oxpulse-admin on :%s", cfg.Port)
        if err := srv.ListenAndServe(); err != http.ErrServerClosed {
            log.Fatal(err)
        }
    }()

    quit := make(chan os.Signal, 1)
    signal.Notify(quit, os.Interrupt)
    <-quit

    ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
    defer cancel()
    _ = srv.Shutdown(ctx)
}
```

- [ ] **Step 9: Add go dependencies**

```bash
cd ~/src/oxpulse-admin
go get github.com/jackc/pgx/v5
go mod tidy
```

- [ ] **Step 10: Commit**

```bash
cd ~/src/oxpulse-admin && git init && git add -A
git commit -m "init: oxpulse-admin dashboard scaffold (Go + HTMX)"
```

---

### Task 5: Dashboard HTML templates

**Files:**
- Create: `~/src/oxpulse-admin/internal/admin/templates/layout.html`
- Create: `~/src/oxpulse-admin/internal/admin/templates/login.html`
- Create: `~/src/oxpulse-admin/internal/admin/templates/overview.html`
- Create: `~/src/oxpulse-admin/internal/admin/templates/calls.html`
- Create: `~/src/oxpulse-admin/internal/admin/templates/viral.html`
- Create: `~/src/oxpulse-admin/internal/admin/templates/devices.html`
- Create: `~/src/oxpulse-admin/internal/admin/static/admin.js`
- Copy: `htmx.min.js` from go-nerv static

Design: same dark theme as go-nerv (CSS variables, sidebar, no framework). Use Chart.js for line charts (calls over time, viral funnel).

**Key pages:**

1. **Overview** — 4 stat cards (total calls, unique devices, rooms created, avg duration) + 30-day call chart
2. **Calls** — daily calls chart, call duration distribution
3. **Viral** — funnel: page_view → room_created → call_connected → repeat users. This is THE metric.
4. **Devices** — top 50 devices by activity (truncated IDs), rooms created, calls made

Layout pattern: copy from `$HOME/src/go-nerv/internal/admin/templates/layout.html` and adapt sidebar links.

Template approach: use `{{define "layout"}}...{{template "content" .}}...{{end}}` pattern from go-nerv.

- [ ] **Step 1: Copy layout.html from go-nerv and adapt**

Sidebar items:
```html
<a href="/admin/" class="nav-item {{if eq .Active "overview"}}active{{end}}">Overview</a>
<a href="/admin/calls" class="nav-item {{if eq .Active "calls"}}active{{end}}">Calls</a>
<a href="/admin/viral" class="nav-item {{if eq .Active "viral"}}active{{end}}">Viral</a>
<a href="/admin/devices" class="nav-item {{if eq .Active "devices"}}active{{end}}">Devices</a>
```

- [ ] **Step 2: Create login.html**

Copy from go-nerv, change title to "OxPulse Admin".

- [ ] **Step 3: Create overview.html**

4 stat cards + Chart.js line chart for daily calls:
```html
{{define "content"}}
<div class="stats-grid">
  <div class="stat-card"><div class="stat-value">{{.Stats.TotalCalls}}</div><div class="stat-label">Total Calls</div></div>
  <div class="stat-card"><div class="stat-value">{{.Stats.TotalDevices}}</div><div class="stat-label">Unique Devices</div></div>
  <div class="stat-card"><div class="stat-value">{{.Stats.TotalRooms}}</div><div class="stat-label">Rooms Created</div></div>
  <div class="stat-card"><div class="stat-value">{{.Stats.CallsToday}}</div><div class="stat-label">Calls Today</div></div>
</div>
<div class="chart-container">
  <canvas id="dailyChart"></canvas>
</div>
<script>
  initLineChart('dailyChart', [{{range .Daily}}'{{.Date.Format "Jan 02"}}',{{end}}], [{{range .Daily}}{{.Count}},{{end}}], 'Daily Calls');
</script>
{{end}}
```

- [ ] **Step 4: Create viral.html**

Funnel visualization:
```html
{{define "content"}}
<h2>Viral Funnel (30 days)</h2>
<div class="funnel">
  <div class="funnel-step"><span class="funnel-count">{{.Funnel.PageViews}}</span><span class="funnel-label">Page Views</span></div>
  <div class="funnel-arrow">→</div>
  <div class="funnel-step"><span class="funnel-count">{{.Funnel.RoomsCreated}}</span><span class="funnel-label">Rooms Created</span></div>
  <div class="funnel-arrow">→</div>
  <div class="funnel-step"><span class="funnel-count">{{.Funnel.CallsStarted}}</span><span class="funnel-label">Calls Connected</span></div>
  <div class="funnel-arrow">→</div>
  <div class="funnel-step highlight"><span class="funnel-count">{{.Funnel.Repeat}}</span><span class="funnel-label">Repeat Creators</span></div>
</div>
<p class="viral-note">Repeat Creators = devices that created 2+ rooms. This is the viral metric.</p>
{{end}}
```

- [ ] **Step 5: Create calls.html and devices.html**

Calls: Chart.js chart with connected + ended overlaid.
Devices: HTML table with truncated device_id, rooms, calls, first/last seen.

- [ ] **Step 6: Create admin.js with Chart.js helper**

```javascript
function initLineChart(id, labels, data, label) {
  new Chart(document.getElementById(id), {
    type: 'line',
    data: { labels, datasets: [{ label, data, borderColor: '#C9A96E', tension: 0.3, fill: false }] },
    options: { responsive: true, scales: { y: { beginAtZero: true } }, plugins: { legend: { display: false } } }
  });
}
```

- [ ] **Step 7: Download htmx.min.js and chart.min.js**

```bash
curl -o internal/admin/static/htmx.min.js https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js
curl -o internal/admin/static/chart.min.js https://cdn.jsdelivr.net/npm/chart.js@4/dist/chart.umd.min.js
```

- [ ] **Step 8: Verify build**

```bash
cd ~/src/oxpulse-admin && go build ./cmd/...
```

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: dashboard templates — overview, calls, viral funnel, devices"
```

---

### Task 6: Deploy admin dashboard

**Files:**
- Create: `~/src/oxpulse-admin/Dockerfile`
- Modify: `$OPERATOR_DEPLOY/compose/apps.yml` — add oxpulse-admin service

- [ ] **Step 1: Create Dockerfile**

```dockerfile
FROM golang:1.24-alpine AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /binary ./cmd

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /binary /usr/local/bin/oxpulse-admin
EXPOSE 8908
CMD ["oxpulse-admin"]
```

- [ ] **Step 2: Add to docker-compose**

```yaml
  oxpulse-admin:
    build:
      context: $HOME/src/oxpulse-admin
      dockerfile: Dockerfile
    container_name: oxpulse-admin
    restart: unless-stopped
    labels:
      dozor.group: "apps"
    ports:
      - "127.0.0.1:8908:8908"
    environment:
      - PORT=8908
      - DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/oxpulse
      - ADMIN_USER=${OXPULSE_ADMIN_USER:-admin}
      - ADMIN_PASS=${OXPULSE_ADMIN_API_KEY}
      - HMAC_SECRET=${OXPULSE_ADMIN_HMAC_SECRET:-change-me}
    networks:
      - backend
    mem_limit: 32M
    cap_drop:
      - ALL
```

Port 8908 (next free).

- [ ] **Step 3: Add DATABASE_URL to oxpulse-chat docker-compose**

Add to the existing oxpulse-chat service environment:
```yaml
      - DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/oxpulse
```

This enables analytics storage in the shared oxpulse database.

- [ ] **Step 4: Build and deploy both**

```bash
cd $OPERATOR_DEPLOY
docker compose build oxpulse-chat oxpulse-admin
docker compose up -d --no-deps --force-recreate oxpulse-chat oxpulse-admin
```

- [ ] **Step 5: Add Caddy route for admin**

```
admin.oxpulse.chat {
    reverse_proxy localhost:8908
}
```

Or use a path-based route on the same domain (simpler for now):
Add to Caddyfile under oxpulse.chat block:
```
handle /admin/* {
    reverse_proxy localhost:8908
}
```

- [ ] **Step 6: Verify health**

```bash
curl -s http://127.0.0.1:8908/admin/login | head -5  # should show HTML
curl -s http://127.0.0.1:8907/api/health              # should show "ok"
```

- [ ] **Step 7: Update CLAUDE.md ports**

Add `8908 | oxpulse-admin`.

- [ ] **Step 8: Commit deploy changes**

```bash
cd $OPERATOR_DEPLOY && git add compose/apps.yml
git commit -m "deploy: add oxpulse-admin dashboard on port 8908"
```

---

### Task 7: Update ROADMAP and rebuild frontend

**Files:**
- Modify: `~/src/oxpulse-chat/docs/ROADMAP.md` — add analytics to Phase 1
- Modify: `~/src/oxpulse-chat/docs/MARKETING.md` — add metrics section

- [ ] **Step 1: Add to ROADMAP under Phase 1**

Add new section after "Tests":
```markdown
### Analytics (Privacy-Preserving)
- [x] Anonymous device_id (random UUID in localStorage, no fingerprinting)
- [x] 5 event types: page_view, room_created, room_joined, call_connected, call_ended
- [x] Batch transport (sendBeacon, max 20 events/req)
- [x] PostgreSQL storage (optional — app works without DB)
- [x] Admin dashboard (Go + HTMX, dark theme)
- [x] Viral funnel metric: page_view → room_created → call_connected → repeat creators
- [x] Call analytics: daily calls, duration, unique devices
- [x] Device activity: top devices by rooms/calls (truncated IDs, no PII)
```

- [ ] **Step 2: Add to MARKETING.md**

Update the metrics section:
```markdown
## Метрики (что мы считаем анонимно)

| Метрика | Как | Зачем |
|---------|-----|-------|
| Viral coefficient | Devices с 2+ room_created | Продукт растёт сам? |
| Daily active calls | call_connected за день | Продукт используют? |
| Avg call duration | call_ended.duration_secs | Качество звонков |
| Unique devices | Distinct device_id | Сколько людей |
| Funnel | page_view → room → call → repeat | Где теряем |

**Не собираем:** IP, email, телефон, browser fingerprint, геолокацию, содержание звонков.
```

- [ ] **Step 3: Build and deploy frontend**

```bash
cd ~/src/oxpulse-chat/web && npm run build
cd $OPERATOR_DEPLOY && docker compose build oxpulse-chat && docker compose up -d --no-deps --force-recreate oxpulse-chat
```

- [ ] **Step 4: Commit all**

```bash
cd ~/src/oxpulse-chat && git add -A
git commit -m "feat: privacy-preserving analytics + admin dashboard"
git push origin main
```
