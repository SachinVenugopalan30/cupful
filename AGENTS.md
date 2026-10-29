# AGENTS.md

Guide for AI coding agents working on Cupful. `PLAN.md` is the full brief; this file is the short, current-state version. Read both before writing code.

## What this is

Self-hosted food/exercise/steps tracker. Rust (Axum + SQLx/SQLite) API serving an embedded SvelteKit (Svelte 5, static SPA) frontend, one binary, deployed with Docker + a Tailscale sidecar. License AGPL-3.0-or-later, DCO sign-off on every commit.

## Status

- **Done:** M0 (scaffold), M1 (accounts, profile, budget engine, write contract), M2 (AI providers, nutrition pipeline, barcode product lookup; live check on oMLX).
- **In progress:** M3 — Today screen built, polished and merged (budget card, composer + review, meal list, barcode scanning, settings). Waiting on the owner's phone check over the tailnet (PLAN.md §14 M3 done-criterion).
- Open shortcuts: `grep -rn "ponytail:" server web/src`. Deviations from the plan: `docs/decisions.md`.

## Layout

```
server/                 Rust crate `cupful`
  src/main.rs           CLI: serve (default), reset-password, import-foods, eval
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
  src/products.rs       barcode lookup (Open Food Facts), label entry, saved products
  src/ai/mod.rs         Provider trait, for_user/by_id, daily cap, HTTP driver (retries, timeouts, scrubbing)
  src/ai/{openai_compatible,anthropic,gemini}.rs   per-adapter request body and reply text
  src/ai/settings.rs    admin provider REST, Test connection, env seeding
  src/crypto.rs         secrets at rest: key loading, check value, `Secret`
  src/nutrition/        estimate pipeline: mod.rs (prompt, validation, measures, oil rule), foods.rs (normalize, seeding, USDA import, matching), routes.rs, golden.rs
  src/error.rs          API error type; JsonBody/Query extractors that reject with JSON
  src/lib.rs            AppState, route table (`route_table()`), router
  migrations/           sqlx migrations (never edit a released one)
  .sqlx/                committed offline query metadata
  tests/                integration tests (oneshot against the Router, in-memory SQLite)
  tests/isolation.rs    user A vs user B for every `Access::User` route
  tests/products.rs     barcode lookups against wiremock
web/                    SvelteKit app (adapter-static, SPA fallback)
  src/lib/api.ts        API client, types for every endpoint, ApiError, errorMessage(e, context)
  src/lib/sync/         uuidv7, op builders, sendOps (the only logged-data write path), scaleItems
  src/lib/{format,date,theme}.ts, session.svelte.ts   formatting, dates, theme preference, session/day state
  src/lib/styles/       tokens.css (design tokens, light-dark()), global.css (shared classes)
  src/lib/{today,products,settings}/   screen components (props documented in each file's header)
  e2e/, playwright.config.ts   Playwright against the real server
docs/design.md          design direction, tokens, component rules
fixtures/budget_cases.json   shared budget test vectors
data/staples.csv, data/aliases.csv, data/portions.csv   shipped food data and USDA portion weights (embedded, seeded at startup)
data/tools/            build_staples.py + staples_spec.txt: regenerate staples.csv from USDA downloads
data/golden/            golden estimate cases with recorded model replies
docs/providers.md       AI provider setup
docs/decisions.md       one line per deviation/notable choice
```

## Commands

```sh
# backend (from server/)
CUPFUL_SECURE_COOKIES=false cargo run -- serve
cargo run -- import-foods <FoodData_Central_*_food_json.json>   # Foundation or SR Legacy, unzipped
cargo run -- eval --provider <id>                             # golden set against a live provider
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
# after a migration or query! change:
mkdir -p ../.cupful && export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db && sqlx database create && sqlx migrate run && cargo sqlx prepare -- --all-targets

# frontend (from web/)
npm run dev        # :5173, proxies /api -> :8080
npm test           # Vitest
npm run test:e2e   # Playwright (builds web, starts the Rust server); once: npx playwright install chromium
npm run check
npm run build
```

## Conventions

- Every source file starts with `SPDX-License-Identifier: AGPL-3.0-or-later`.
- TDD: failing test first. Backend integration tests use `tower::ServiceExt::oneshot` on the real router with in-memory SQLite (`tests/common/mod.rs` has helpers).
- SQL through `sqlx::query!`/`query_as!` macros so it's compile-checked; refresh `.sqlx/` after changes. Exception: FTS5 queries use runtime `query_as` (sqlx's describe crashes on virtual tables).
- JSON Schemas sent to AI providers: `additionalProperties: false` and full `required` on every object.
- Logged data (meals, days) is written **only** through `POST /api/sync`. Config/account data uses plain REST, online only.
- Every mutating request needs header `X-Cupful: 1` (CSRF).
- Every user-owned route needs an isolation test; `tests/isolation.rs` fails if a route is not listed. `Access::Admin` routes use the `auth::Admin` extractor and are checked for 403/401 there too.
- Budget is computed only on the server (`budget.rs`); f64 throughout, round on serialize.
- Dates are the user's local date (profile timezone), TEXT `YYYY-MM-DD`. Times are INTEGER unix milliseconds in the DB; RFC 3339 on the wire.
- New routes go in `routes()` in `lib.rs` with their `Access`. Request bodies use `error::JsonBody`, queries `error::Query`, so errors are always `{"error": ...}`.
- Keep it small: no new dependency for what a few lines do. Mark deliberate shortcuts with `// ponytail: <ceiling>, <upgrade path>`.
- Record deviations from `PLAN.md` in `docs/decisions.md` as one line: date, what, why.
- Frontend logic gets a Vitest test (`*.test.ts` next to the code). Screens use tokens and `global.css` classes; no CSS framework or UI kit. Logged-data writes go only through `sync.sendOps`. `#lib` imports of TS files keep the `.ts` extension.
- Outbound HTTP goes only to configured AI providers and (unless `CUPFUL_PRODUCT_LOOKUP=off`) Open Food Facts. Tests point base URLs at `wiremock`; `common::app()` has product lookup off. No telemetry, ever.

## Keeping this file current

Update this file and `CLAUDE.md` in the same commit whenever a milestone completes or the layout, commands or conventions change. Stale agent docs are a bug.
