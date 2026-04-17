# Partner Branding Configs

Each `.json` file in this directory defines per-host branding for one partner.
The files are bundled at compile time via `include_dir!` in
`crates/server/src/branding/mod.rs` and loaded on startup.

## What a partner config does

OxPulse runs partner mirrors at custom domains (e.g. `call.piter.now`,
`call.rvpn.online`). When a request arrives, the server checks the `Host`
header against all partner `domains` lists and returns the matching branding.
The SPA then applies colors, logos, copy, and OG meta from that config.

Lexicographic sort order matters: `oxpulse.json` is index 0 and serves
as the fallback for any unknown host. Do not add a file that sorts
before `oxpulse.json` (i.e. starts with a character below `o` ASCII 111)
unless you also update `mod.rs`.

## Fields

| Field | Type | Description |
|-------|------|-------------|
| `partner_id` | string | Short identifier, matches filename (e.g. `"piter"`) |
| `domains` | string[] | Exact hostnames that map to this config (port-stripped, lowercased) |
| `display_name` | string | Human-readable name shown in page title and OG site_name |
| `description` | string | Used in `<meta name="description">` and OG description |
| `logo.light` | string | URL to light-theme logo, served from `/partners/{id}/logo-light.svg` |
| `logo.dark` | string | URL to dark-theme logo |
| `favicon` | string | URL to partner favicon |
| `og_image` | string | URL to OG image (1200x630); real PNG expected before go-live |
| `colors.primary` | string | Hex color, used for UI accents (buttons, links) |
| `colors.secondary` | string | Hex color, used for backgrounds / navbar |
| `colors.accent` | string\|null | Optional third accent color |
| `copy.hero_title_ru` | string | Russian hero heading |
| `copy.hero_title_en` | string | English hero heading |
| `affiliate` | object\|null | VPN CTA block: `vpn_cta_url`, `vpn_cta_text_ru`, `vpn_cta_text_en` |
| `legal` | object\|null | `partner_entity`, `partner_country`, `partner_contact` |

## Asset path conventions

Static assets are served from `web/static/partners/{partner_id}/`:

```
web/static/partners/
  piter/
    logo-light.svg   (200×60, white background)
    logo-dark.svg    (200×60, dark background)
    favicon.svg      (32×32, single letter)
    og-image.png     (1200×630, real PNG for go-live; SVG placeholder until Task 9)
  rvpn/
    ...
```

JSON paths reference `/partners/{id}/filename` which maps directly to these files.

## Schema template

See `template.json` for a fully-populated example with all fields.

## How to add a new partner (5 steps)

1. Copy `template.json` to `{partner_id}.json` (ensure filename sorts after `oxpulse.json`)
2. Fill in all fields — `partner_id` must match the filename stem
3. Add placeholder SVGs to `web/static/partners/{partner_id}/` (see piter/rvpn for reference)
4. Run `cargo build -p oxpulse-chat && cargo test -p oxpulse-chat` — must be green
5. Run `cd web && npm run build` and verify `assets/room/partners/{partner_id}/` is present

## Reference

Design doc: `~/docs/superpowers/specs/2026-04-17-oxpulse-partner-mirror-design.md`
