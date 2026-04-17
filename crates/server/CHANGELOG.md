# Changelog

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
