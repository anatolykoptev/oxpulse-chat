# OxPulse Chat — Marketing Strategy

## Two Markets, One Product

### Market 1: Russia — "Telegram заблокировали, как звонить за границу?"

**Ситуация:** РКН блокирует Telegram, Zoom нестабилен, FaceTime/WhatsApp через VPN — лагает. Миллионы людей в России не могут нормально позвонить родственникам и друзьям за границу. Это не техническая проблема — это бытовая. Мама не может позвонить сыну в Берлин.

**Что мы решаем:** Видеозвонки, которые невидимы для ТСПУ. Не нужен VPN, не нужен аккаунт. Открыл ссылку — позвонил. Трафик маскируется под обычный TLS к samsung.com. Медиа не покидает Россию (coturn на российском сервере).

**Боль:** Конкретная, массовая, ежедневная. Не "privacy-conscious", а "мне нужно поговорить с семьёй".

### Market 2: US Immigration — "Нужна связь, которую не мониторят"

**Ситуация:** Иммигранты в США (undocumented, asylum seekers, visa overstays) нуждаются в безопасной связи. Обычные мессенджеры привязаны к номеру телефона, логируют метаданные, отвечают на запросы ICE/DHS. Адвокаты, правозащитные организации, диаспорные комьюнити — всем нужен канал связи без привязки к личности.

**Что мы решаем:** Звонки без регистрации, без номера телефона, без метаданных. Open source — можно проверить что нет бэкдоров. E2EE. Будущий mesh — работает даже без интернета.

**Боль:** Страх депортации + потребность в связи с семьёй и юристами.

## Positioning

**Universal:** "Encrypted calls that just work. No account. No tracking."

**Russia:** "Звонки, которые работают когда Telegram не работает."

**US Immigration:** "Calls without a phone number. No logs. No metadata."

## Brand Identity

- **Name:** OxPulse Chat
- **Domain:** oxpulse.chat
- **Tone:** Simple, human, no tech jargon for end users. Technical depth for devs.
- **Visual:** Dark theme, gold accent (#C9A96E), minimal
- **Logo:** TBD

## Growth Strategy

### Strategy 1: Russia — Word of Mouth + Telegram Channels

The product solves a daily pain. People will share it naturally.

**Distribution:**
- [ ] Telegram channels about блокировках/VPN (antizapret, roskomsvoboda communities)
- [ ] Telegram channels диаспоры (русские в Германии, Израиле, США, Грузии)
- [ ] Мамские форумы и чаты (бабушка не может позвонить внукам — это боль)
- [ ] Habr/vc.ru: технический разбор "как обойти ТСПУ для видеозвонков"
- [ ] SEO (Yandex): "видеозвонки без VPN", "как позвонить за границу из России"
- [ ] YouTube: короткое видео "как позвонить если заблокировали Telegram"

**Key insight:** Не нужно объяснять что такое E2EE или VLESS. Нужно объяснить: "скинь ссылку маме — она нажмёт и вы поговорите".

**Viral mechanic:** Человек в России скидывает ссылку родственнику за границей. Родственник тоже видит OxPulse. Теперь двое знают про продукт. Один из них скинет ещё кому-то.

### Strategy 2: US Immigration — NGOs + Legal Aid Networks

Иммигрантские комьюнити не сидят на HackerNews. Они в WhatsApp группах, в церквях, в юридических клиниках.

**Distribution:**
- [ ] Партнёрства с immigration legal aid organizations (RAICES, CLINIC, local legal aid)
- [ ] Материалы для immigration lawyers: "recommend this to your clients"
- [ ] Diaspora communities: Facebook groups, WhatsApp groups, community centers
- [ ] Spanish-language landing page (Phase 2 — largest undocumented population)
- [ ] Flyers/QR codes for physical distribution in legal clinics
- [ ] Digital security training organizations (EFF, Access Now)

**Key insight:** Доверие через организации, не через рекламу. Если адвокат говорит "используйте это" — люди используют.

### Strategy 3: Open Source / Tech Community

Secondary market but drives credibility and self-hosted adoption.

**Distribution:**
- [ ] HN: "Show HN" (focus on Rust + 500 LOC + anti-censorship angle)
- [ ] Reddit: r/selfhosted, r/privacy, r/rust
- [ ] awesome-selfhosted, awesome-rust PRs
- [ ] Habr technical article (architecture deep dive)

### Strategy 4: Viral Loop (Built into Product)

Every call = brand exposure.

- [ ] Link format: `oxpulse.chat/BEAR-7042` (memorable, shareable)
- [ ] OG meta tags with branded preview image
- [ ] After call ended → "Create your own room" CTA
- [ ] "Powered by OxPulse" subtle footer

### Strategy 5: B2B Conversion (Long-term)

```
Free calls → Daily usage → Account → Sees "OxPulse Business" → Widget trial → Paid
```

- [ ] "OxPulse for Business" link in footer (after Phase 2)
- [ ] Case study with reklama.piter.now

## Launch Plan

### Week 1: Soft Launch (NOW)

- [x] Deploy to oxpulse.chat
- [x] GitHub repo with README + ROADMAP
- [x] Domain oxpulse.chat purchased, SSL active
- [ ] Test with 5-10 people (cross-border calls Russia↔abroad)
- [ ] Fix UX issues found in testing
- [ ] Record GIF demo: create room → share link → call starts

### Week 2: Russia Launch

- [ ] Post in Telegram channels диаспоры (русские за границей)
- [ ] Post in anti-censorship communities
- [ ] Habr article: "Видеозвонки, невидимые для ТСПУ — как мы это сделали"
- [ ] Short video for YouTube/Telegram: "Как позвонить из России за границу"
- [ ] Ask 5 people to try calling their relatives and give feedback

### Week 3-4: Tech Community

- [ ] Show HN post
- [ ] r/selfhosted, r/privacy, r/rust
- [ ] awesome-selfhosted PR
- [ ] Respond to GitHub issues

### Month 2: Phase 2 + Immigration Market

- [ ] Phase 2 shipped (accounts, personal rooms)
- [ ] Contact 3 immigration legal aid organizations
- [ ] Spanish-language basics on landing page
- [ ] Product Hunt launch
- [ ] @oxpulse Twitter active

### Month 3: Scale

- [ ] Phase 3 (encrypted chat) in progress
- [ ] First partnerships with NGOs
- [ ] Content: blog posts, technical articles
- [ ] First B2B conversion attempts

## KPIs

| Metric | Month 1 | Month 3 | Month 6 |
|--------|---------|---------|---------|
| Daily active calls | 20 | 200 | 1,000 |
| Unique users (weekly) | 50 | 500 | 5,000 |
| GitHub stars | 100 | 1,000 | 3,000 |
| Russia→abroad calls | 10 | 100 | 500 |
| Self-hosted instances | 3 | 15 | 50 |
| NGO partnerships | 0 | 2 | 5 |

## Budget

**$0.** Organic only. No paid ads until product-market fit proven.

Infrastructure: $0 extra (existing krolik-server + Hostiman).
Domain: ~$10/year (oxpulse.chat).

## Key Risks

| Risk | Mitigation |
|------|-----------|
| RKN blocks VLESS/Reality | Fallback protocols (WebSocket, gRPC), domain fronting |
| Low adoption (no network effect yet) | Focus on cross-border use case (caller sends link to recipient) |
| Competitors copy the idea | Speed + open source community + mesh (hard to replicate) |
| Legal pressure in Russia | Server outside Russia, no Russian entity, AGPL protects code |
| Immigration market is hard to reach digitally | Partner with NGOs who have direct access |
