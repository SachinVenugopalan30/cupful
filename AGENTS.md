# AGENTS.md

Guide for AI coding agents working on Cupful. `PLAN.md` is the full brief; this file is the short, current-state version. Read both before writing code.

## What this is

Self-hosted food/exercise/steps tracker. Rust (Axum + SQLx/SQLite) API serving an embedded SvelteKit (Svelte 5, static SPA) frontend, one binary, deployed with Docker + a Tailscale sidecar. License AGPL-3.0-or-later, DCO sign-off on every commit.

## Status

- **Done:** M0 (scaffold), M1 (accounts, profile, budget engine, write contract).
- **Next:** M2 — AI providers, nutrition pipeline and barcode product lookup (PLAN.md §8, §14).
- Open shortcuts: `grep -rn "ponytail:" server web/src`. Deviations from the plan: `docs/decisions.md`.

## Layout

```
server/                 Rust crate `cupful`
  src/main.rs           CLI: serve (default), reset-password
  src/config.rs         env config (CUPFUL_*)
  src/db.rs             pool + SQLite pragmas + migrations
  src/assets.rs         rust-embed of web/build (debug builds read disk)
  src/auth.rs           setup, login, logout, sessions store, rate limit, CSRF, client IP
  src/profile.rs        profile + profile_versions REST
  src/budget.rs         budget math, pure functions
  src/sync.rs           POST /api/sync write contract
  src/reads.rs          GET days / meals / budget
  src/time.rs           user-local "today" helpers
  src/cleanup.rs        daily cleanup task
  src/error.rs          API error type; JsonBody/Query extractors that reject with JSON
  src/lib.rs            AppState, route table (`route_table()`), router
  migrations/           sqlx migrations (never edit a released one)
  .sqlx/                committed offline query metadata
  tests/                integration tests (oneshot against the Router, in-memory SQLite)
  tests/isolation.rs    user A vs user B for every `Access::User` route
web/                    SvelteKit app (adapter-static, SPA fallback)
fixtures/budget_cases.json   shared budget test vectors
docs/decisions.md       one line per deviation/notable choice
```

## Commands

```sh
# backend (from server/)
CUPFUL_SECURE_COOKIES=false cargo run -- serve
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
# after a migration or query! change:
mkdir -p ../.cupful && export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db && sqlx database create && sqlx migrate run && cargo sqlx prepare -- --all-targets

# frontend (from web/)
npm run dev        # :5173, proxies /api -> :8080
npm run check
npm run build
```

## Conventions

- Every source file starts with `SPDX-License-Identifier: AGPL-3.0-or-later`.
- TDD: failing test first. Backend integration tests use `tower::ServiceExt::oneshot` on the real router with in-memory SQLite (`tests/common/mod.rs` has helpers).
- SQL through `sqlx::query!`/`query_as!` macros so it's compile-checked; refresh `.sqlx/` after changes.
- Logged data (meals, days) is written **only** through `POST /api/sync`. Config/account data uses plain REST, online only.
- Every mutating request needs header `X-Cupful: 1` (CSRF).
- Every user-owned route needs an isolation test; `tests/isolation.rs` fails if a route is not listed.
- Budget is computed only on the server (`budget.rs`); f64 throughout, round on serialize.
- Dates are the user's local date (profile timezone), TEXT `YYYY-MM-DD`. Times are INTEGER unix milliseconds in the DB; RFC 3339 on the wire.
- New routes go in `routes()` in `lib.rs` with their `Access`. Request bodies use `error::JsonBody`, queries `error::Query`, so errors are always `{"error": ...}`.
- Keep it small: no new dependency for what a few lines do. Mark deliberate shortcuts with `// ponytail: <ceiling>, <upgrade path>`.
- Record deviations from `PLAN.md` in `docs/decisions.md` as one line: date, what, why.
- No telemetry, ever.

## Keeping this file current

Update this file and `CLAUDE.md` in the same commit whenever a milestone completes or the layout, commands or conventions change. Stale agent docs are a bug.
