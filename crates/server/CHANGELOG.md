# Changelog

## [0.5.0](https://github.com/anatolykoptev/oxpulse-chat/compare/v0.4.0...v0.5.0) (2026-04-19)


### Features

* **analytics:** pipe ox_vid cookie into call_events.visitor_id ([d08a537](https://github.com/anatolykoptev/oxpulse-chat/commit/d08a537536cf5f718b41e95203c09c41409e2de8))
* **visitor:** server-side anonymous identity cookie (ox_vid) ([44a62ae](https://github.com/anatolykoptev/oxpulse-chat/commit/44a62aebbbf201c56c8b34036e1a84a9fdc23f23))


### Bug Fixes

* **migrate:** register 20260419_visitor_id.sql in embedded migrations ([4723eb1](https://github.com/anatolykoptev/oxpulse-chat/commit/4723eb127a4c830252050223c9e80c11f695e0bb))

## [0.4.0](https://github.com/anatolykoptev/oxpulse-chat/compare/v0.3.0...v0.4.0) (2026-04-18)


### Features

* **server:** ML-KEM-768 post-quantum VLESS encryption support (Harvest Now, Decrypt Later defense) ([#24](https://github.com/anatolykoptev/oxpulse-chat/issues/24)) ([5285e1d](https://github.com/anatolykoptev/oxpulse-chat/commit/5285e1d3a2edcede8c45cdb0e1c7646594d20be5))
* **server:** SNI rotation + partner-edge DoH for DNS-poison resistance ([#21](https://github.com/anatolykoptev/oxpulse-chat/issues/21)) ([78c9027](https://github.com/anatolykoptev/oxpulse-chat/commit/78c9027c7727f19883e8343f495e4c33c41c3329))

## [0.3.0](https://github.com/anatolykoptev/oxpulse-chat/compare/v0.2.0...v0.3.0) (2026-04-18)


### Features

* **branding:** unify product name to OxPulse, add partner co-brand credit ([a5fe2b0](https://github.com/anatolykoptev/oxpulse-chat/commit/a5fe2b080b75a01c1140987562e40b75392319b6))
* **server:** /metrics endpoint with Prometheus text format + token auth ([#7](https://github.com/anatolykoptev/oxpulse-chat/issues/7)) ([8110bb0](https://github.com/anatolykoptev/oxpulse-chat/commit/8110bb020d60ede614e5b7beea23a367c7b587d9))
* **server:** geo-hint from client headers reorders TurnPool (Task 2.5) ([#16](https://github.com/anatolykoptev/oxpulse-chat/issues/16)) ([61b4ef3](https://github.com/anatolykoptev/oxpulse-chat/commit/61b4ef3098791012cc8851501b9010c737f0b3d0))
* **server:** partner registration API + admin CLI ([5ed5932](https://github.com/anatolykoptev/oxpulse-chat/commit/5ed59321effadba87b57650478d37ea785e7bd2f))
* **server:** per-IP rate limit on /api/turn-credentials + /api/event (Task 4.1) ([#17](https://github.com/anatolykoptev/oxpulse-chat/issues/17)) ([d6e8bef](https://github.com/anatolykoptev/oxpulse-chat/commit/d6e8bef8910b42f4ce65377d94f682805beb69a9))
* **server:** server-decided iceTransportPolicy in /api/turn-credentials (Task 4.3) ([#20](https://github.com/anatolykoptev/oxpulse-chat/issues/20)) ([9a90443](https://github.com/anatolykoptev/oxpulse-chat/commit/9a90443fe5a306189bb2a57dae9dae9fb8d09224))
* **server:** SIGHUP hot-reload of TURN server list via ArcSwap (Task 2.6) ([#19](https://github.com/anatolykoptev/oxpulse-chat/issues/19)) ([82e1163](https://github.com/anatolykoptev/oxpulse-chat/commit/82e1163ba7ece11cab36043eff7d9fcfa4d394ff))
* **server:** wire Prometheus metrics into all hot paths including 3 new SLO metrics ([#15](https://github.com/anatolykoptev/oxpulse-chat/issues/15)) ([2af3151](https://github.com/anatolykoptev/oxpulse-chat/commit/2af31514f7b0fb1f342b612bb62a8a6772c96bd9))
* **server:** wire TurnPool probe loop + healthy-only /api/turn-credentials ([f6a39ce](https://github.com/anatolykoptev/oxpulse-chat/commit/f6a39ce610a7cd36add8cc026ea8296c00d8cea9))
* **signaling:** room-id entropy guard + per-IP join rate limit (Task 4.2) ([#11](https://github.com/anatolykoptev/oxpulse-chat/issues/11)) ([4af0f3c](https://github.com/anatolykoptev/oxpulse-chat/commit/4af0f3c94667a30d7e7861ef7dc744541ed8c5e8))


### Bug Fixes

* **branding:** use proper PITER.NOW / Питер Сегодня partner wordmarks ([975c02e](https://github.com/anatolykoptev/oxpulse-chat/commit/975c02e15d8c22c162900b02372704f6f62d349a))
* **migrate:** remove stray semicolon in partner_nodes comment ([4486d4f](https://github.com/anatolykoptev/oxpulse-chat/commit/4486d4f7d16715921fecde3ea8094a01a9fc9c58))
* **partner-registry:** require PARTNER_BACKEND_ENDPOINT, drop broken default ([#6](https://github.com/anatolykoptev/oxpulse-chat/issues/6)) ([69f8c2b](https://github.com/anatolykoptev/oxpulse-chat/commit/69f8c2b1cbf0224b86cd5c53e893eaeee84806f0))

## [0.2.0](https://github.com/anatolykoptev/oxpulse-chat/compare/v0.1.0...v0.2.0) (2026-04-17)


### Features

* add /api/event endpoint for analytics ingestion ([237fca5](https://github.com/anatolykoptev/oxpulse-chat/commit/237fca5955943cbc8f304b211cd3bc3b66ae924c))
* add optional PostgreSQL for analytics ([b3f3b7d](https://github.com/anatolykoptev/oxpulse-chat/commit/b3f3b7d6e174742610efe932459b159961c5d756))
* add source field to call_events (hostname per domain) ([f36ddae](https://github.com/anatolykoptev/oxpulse-chat/commit/f36ddaee03324adb74cef3344d8274c8e0e3b684))
* **partners:** add piter and rvpn co-brand configs + placeholder assets ([ef9bd64](https://github.com/anatolykoptev/oxpulse-chat/commit/ef9bd6493d064ea526b112af5350cbf775bd630a))
* **server,web:** domain rotation + connectivity fallback ([2ba6b4f](https://github.com/anatolykoptev/oxpulse-chat/commit/2ba6b4fe8fe9ff8800fff2c3965b802360c4906a))
* **server:** branding resolver + /api/branding endpoint ([752b0fc](https://github.com/anatolykoptev/oxpulse-chat/commit/752b0fc7d0c8a5b82faaaf40e34bb67e5bec92cc))
* **server:** prefer X-Oxpulse-Host over X-Forwarded-Host ([3b74257](https://github.com/anatolykoptev/oxpulse-chat/commit/3b742571d4fed29754bf03989c80d52a33fc3e50))
* **server:** STUN binding-request health probe ([2aa2fab](https://github.com/anatolykoptev/oxpulse-chat/commit/2aa2fab8ca08718cff15983a0bfa48e210bd7a9e))
* **server:** TurnPool skeleton with config parsing ([5b56f26](https://github.com/anatolykoptev/oxpulse-chat/commit/5b56f26f81d679758eac7193d6a37dc7870c31d4))
* **web:** runtime branding — template placeholders + client store ([00be3fb](https://github.com/anatolykoptev/oxpulse-chat/commit/00be3fbfcee94a8a13a55681a959a06d681cf0fa))


### Bug Fixes

* add pool: None to test AppState (CI fix) ([67d3bca](https://github.com/anatolykoptev/oxpulse-chat/commit/67d3bca653e5adb3e0674000b386396c6adde9f8))
* **router:** route / through spa_fallback for branding template substitution ([78dc4fc](https://github.com/anatolykoptev/oxpulse-chat/commit/78dc4fc96465261a82b84d74189f3087f79ac304))
* **server:** don't panic on missing index.html in build_router ([c5dfda7](https://github.com/anatolykoptev/oxpulse-chat/commit/c5dfda797e1eed0c303fe4e942b7d6ce4f0b9791))
* **server:** SPA fallback returns 200 and analytics persists data field ([651d175](https://github.com/anatolykoptev/oxpulse-chat/commit/651d1750d8234f462b411597d2773ec0373c5afc))
* split migration SQL into individual statements for sqlx ([1511ad0](https://github.com/anatolykoptev/oxpulse-chat/commit/1511ad01c6cc8e35461f7f40774e121f25a87ae3))
