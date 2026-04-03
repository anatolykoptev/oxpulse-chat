# OxPulse Chat — Marketing Plan

## Positioning

**One-liner:** Encrypted video calls that work everywhere — no account, no tracking, open source.

**For Russia:** "Звонки, которые работают когда Telegram не работает."

**For global:** "The open-source Zoom alternative that respects your privacy."

**Category:** Privacy-first communication tool (Signal meets Zoom meets Briar).

## Target Audiences

| Audience | Pain point | Hook |
|----------|-----------|------|
| **Russian users** | Zoom/Meet/FaceTime blocked or unstable | "Works in Russia, invisible to DPI" |
| **Privacy-conscious devs** | Zoom tracks everything, closed source | "Open source, self-hosted, no telemetry" |
| **Self-hosters** | No good lightweight alternative to Jitsi (500MB+) | "30MB Docker image, single binary" |
| **Activists / journalists** | Need censorship-resistant calls | "VLESS tunnel + offline mesh (roadmap)" |
| **Small businesses (B2B)** | Intercom/Zendesk too expensive | → Conversion to OxPulse Business widget |

## Brand Identity

- **Name:** OxPulse Chat
- **Domain:** oxpulse.chat
- **Tone:** Technical, honest, no hype. "We don't track you. Here's the source code."
- **Visual:** Dark theme, gold accent (#C9A96E), minimal
- **Logo:** TBD (need design)

## Growth Strategy

### Channel 1: Viral Loop (Zoom playbook)

Every call = brand exposure. The link `oxpulse.chat/ABCD-1234` introduces new users.

**Actions:**
- [ ] "Powered by OxPulse" subtle footer on call screen
- [ ] OG meta tags with branded preview when sharing links
- [ ] Room link format is memorable: `oxpulse.chat/BEAR-7042`
- [ ] After call ended → "Create your own room" CTA

**Metric:** % of call recipients who later create their own room.

### Channel 2: Open Source Community

GitHub is a distribution channel, not just a repo.

**Actions:**
- [ ] README with GIF demo (first impression = everything)
- [ ] "Show HN" post on Hacker News (timing: weekday 9-11am PST)
- [ ] r/selfhosted post ("30MB encrypted video calls, self-hosted")
- [ ] r/privacy post ("open-source Zoom alternative with E2EE")
- [ ] r/rust post ("500 LOC Rust signaling server, Axum + DashMap")
- [ ] dev.to / Habr article: "Building a Zoom alternative in Rust"
- [ ] Product Hunt launch (after Phase 2 with accounts)
- [ ] awesome-selfhosted PR
- [ ] awesome-rust PR

**Metric:** GitHub stars, forks, Docker pulls.

### Channel 3: Russia / Anti-Censorship Angle

Unique positioning no competitor has.

**Actions:**
- [ ] Habr article: "Как мы сделали видеозвонки, невидимые для ТСПУ"
- [ ] Russian tech Telegram channels (vc.ru, Habr, tproger)
- [ ] IT communities: Хабр Q&A, Pikabu tech
- [ ] Keywords: "видеозвонки без блокировок", "замена Zoom в России"
- [ ] Blog post on VLESS/Reality architecture (technical credibility)

**Metric:** Russian traffic share, Telegram channel subscribers.

### Channel 4: Content / SEO

Long-term organic traffic.

**Topics:**
- "Best open source video call tools 2026"
- "How to self-host video calls"
- "WebRTC Perfect Negotiation explained"
- "TURN server setup guide (coturn)"
- "How to bypass DPI for video calls"
- "Rust for real-time applications"

**Platform:** Blog on oxpulse.chat/blog (Phase 2+) or Medium/dev.to initially.

### Channel 5: B2B Conversion Funnel

Free calls → paid widget.

```
User discovers OxPulse via call link
  → Creates account (Phase 2)
  → Uses regularly for calls
  → Has a business / website
  → Sees "OxPulse Business" in settings or footer
  → Tries embeddable widget (free tier?)
  → Converts to paid plan
```

**Actions:**
- [ ] "OxPulse for Business" link in app footer
- [ ] Landing page: oxpulse.chat/business (after Phase 2)
- [ ] Case study: "How reklama.piter.now uses OxPulse widget"

## Launch Plan

### Week 1: Soft Launch (NOW)

- [x] Deploy to oxpulse.chat
- [x] GitHub repo public with README
- [x] ROADMAP published
- [ ] Buy oxpulse.chat domain — DONE
- [ ] Share with 5-10 people for testing
- [ ] Fix any UX issues found

### Week 2: Community Launch

- [ ] Record GIF demo (create room → call → screen share)
- [ ] Update README with GIF
- [ ] Post to Hacker News ("Show HN: OxPulse — open source encrypted video calls in 500 lines of Rust")
- [ ] Post to r/selfhosted, r/privacy, r/rust
- [ ] Habr article (Russian)

### Week 3-4: Iterate

- [ ] Respond to GitHub issues / feedback
- [ ] Fix top-reported bugs
- [ ] Start Phase 2 (accounts) based on feedback
- [ ] awesome-selfhosted PR

### Month 2: Product Hunt + Phase 2

- [ ] Phase 2 shipped (accounts, contacts, personal rooms)
- [ ] Product Hunt launch
- [ ] dev.to article series
- [ ] Twitter @oxpulse active (technical content, updates)

### Month 3: Growth

- [ ] Target: 1K GitHub stars
- [ ] Target: 100 DAU (daily active users)
- [ ] Target: 10 self-hosted deployments
- [ ] Phase 3 (encrypted chat) in progress
- [ ] First B2B conversion attempts

## Messaging by Audience

### For Hacker News

> **Show HN: OxPulse — Open source encrypted video calls in 500 lines of Rust**
>
> I built a lightweight alternative to Zoom/Jitsi. 30MB Docker image, zero JS dependencies for WebRTC, AGPL-3.0. Works in Russia through VLESS/Reality tunnel (invisible to DPI). No account required — create a room, share the link, start talking.
>
> Stack: Rust (Axum), SvelteKit 5, coturn for TURN relay.
>
> Roadmap: encrypted chat (Double Ratchet), offline P2P via Bluetooth, mesh networking.

### For r/selfhosted

> **OxPulse Chat — self-hosted encrypted video calls (30MB, single binary)**
>
> docker run -p 3000:3000 oxpulse-chat
>
> That's it. No database, no Redis, no complex setup. 1-on-1 video calls with E2EE verification. Open source (AGPL-3.0).

### For Russian audience (Habr)

> **OxPulse — видеозвонки, которые работают в России**
>
> Open-source альтернатива Zoom на Rust. 30 МБ Docker-образ. Работает через VLESS/Reality — невидим для ТСПУ. Без регистрации — создай комнату, скинь ссылку, звони.

## KPIs

| Metric | Month 1 | Month 3 | Month 6 |
|--------|---------|---------|---------|
| GitHub stars | 200 | 1,000 | 3,000 |
| Daily active calls | 10 | 100 | 500 |
| Registered users | — | 500 | 5,000 |
| Self-hosted instances | 5 | 20 | 100 |
| B2B widget trials | — | — | 10 |

## Budget

**$0.** Everything is organic: open source, content, community. No paid ads until product-market fit is proven.

Infrastructure costs: $0 extra (runs on existing krolik-server + Hostiman).
