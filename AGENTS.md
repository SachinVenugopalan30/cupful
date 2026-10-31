# AGENTS.md

Guide for AI coding agents. Read this before writing code. The v0.1.0 brief was removed after release.

## What

Self-hosted food, exercise and steps tracker. Rust Axum + SQLx/SQLite API. Embedded SvelteKit 5 static SPA. One binary. Docker with an optional Tailscale sidecar. AGPL-3.0-or-later. Every commit needs DCO sign-off.

## Status

- M0 to M8 are done. M7 shipped v0.1.0: one-time invites, Tailscale identity mode, rewritten self-hosting docs, and the multi-arch image on GHCR. M8 refreshed the UI: the Mineral palette on the Pantry layout, self-hosted Manrope, new Today, Activity, Trends and Settings screens, server-side trend averages and meal mix, and new docs screenshots.
- Next: v0.2. Start with the known gaps below.
- Known gaps: the offline cache has no eviction. Brave may ignore camera focus hints.
- Shortcuts: `grep -rn "ponytail:" server web/src`. Decisions: `docs/decisions.md`.

## Layout

```
server/                 Rust crate `cupful`
  src/main.rs           CLI: serve, reset-password, import-foods, seed-demo, eval
  src/auth/             setup, login, sessions, passwords, limits, extractors
  src/sync/             POST /api/sync: meals, days, validators
  src/reads.rs          budget, meals, days, trends reads
  src/nutrition/        estimate pipeline, prompt, measures, food matching
  src/ai/               providers, HTTP hardening, admin settings
  src/products/         barcode lookup, label entry, saved products
  src/transfer/         CSV export/import, backup/restore
  src/google/           optional Google Health steps sync
  src/{budget,profile,ingest,demo,cleanup,time,error,crypto,assets,config,db}.rs
  migrations/           never edit a released migration
  .sqlx/                committed offline query metadata
  tests/                integration tests on the real router, in-memory SQLite
  tests/{isolation,products}/   route isolation and barcode wiremock suites
web/                    SvelteKit app: /, /activity, /trends, /settings, /login, /setup, /invite
  src/lib/api.ts        API barrel. `api/` holds types and endpoints by resource
  src/lib/sync/         op builders, uuidv7, sendOps
  src/lib/offline/      IndexedDB queue, cache, overlay, session cache, sync UI
  src/lib/{today,products,settings,activity,trends}/   screens and their logic
  static/fonts/         self-hosted Manrope woff2 and its OFL licence
  src/service-worker.ts PWA shell cache, navigation fallback, update prompt
  e2e/                  Playwright against the real server
docs/                   design, offline, import-export, backups, providers, google-health, shortcuts, decisions
data/                   staples, aliases, portions, USDA tools, golden estimates
fixtures/               shared test vectors and samples
```

## Commands

```sh
# backend, from server/
CUPFUL_SECURE_COOKIES=false cargo run -- serve
cargo run -- import-foods <FoodData_Central_*_food_json.json>
cargo run -- seed-demo --days 90 --seed 42 [--force]
cargo run -- eval --provider <id>
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
# after a migration or query! change
mkdir -p ../.cupful && export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db && sqlx database create && sqlx migrate run && cargo sqlx prepare -- --all-targets

# frontend, from web/
npm run dev        # :5173, proxies /api to :8080
npm test           # Vitest
npm run test:e2e   # Playwright; first run: npx playwright install chromium
npm run check
npm run build
```

## Rules

- Start every source file with `SPDX-License-Identifier: AGPL-3.0-or-later`.
- Keep source and test files under 500 lines. Split by behaviour. No `utils` buckets.
- TDD: write the failing test first. Backend tests use `tower::ServiceExt::oneshot` on the real router. Helpers live in `tests/common/mod.rs`.
- SQL uses `sqlx::query!` and `query_as!`. Refresh `.sqlx/` after a query change. FTS5 is the exception and uses runtime `query_as`.
- Logged data (meals, days) goes only through `POST /api/sync`. Import and restore are the only exceptions. Config and account data use plain REST, online only.
- Every mutating request needs `X-Cupful: 1`.
- Every user-owned route needs an isolation test. `tests/isolation/` fails when a route is missing. Admin routes must answer 403 for users and 401 for anonymous requests.
- Budget math lives only on the server (`budget.rs`). Use f64 and round on serialize.
- Dates are the user's local date, `YYYY-MM-DD`. Times are integer unix milliseconds in SQLite and RFC 3339 on the wire.
- New routes go in `routes()` in `lib.rs` with their `Access`. Use `error::JsonBody` and `error::Query` so errors are `{"error": ...}`.
- Outbound HTTP goes only to configured AI providers, Open Food Facts (unless `CUPFUL_PRODUCT_LOOKUP=off`), and Google when configured and connected. No telemetry.
- No new dependency for a few lines of code. Mark a deliberate shortcut with `// ponytail: <ceiling>, <upgrade path>`.
- Record a deviation or notable choice in `docs/decisions.md`: date, what, why.
- Frontend logic gets a Vitest test next to it. Screens use tokens and `global.css` classes. No CSS framework. Writes go through `sync.sendOps`. Keep `.ts` in `#lib` imports.
- Update this file and `CLAUDE.md` when a milestone completes or the layout, commands or rules change.
