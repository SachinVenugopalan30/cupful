# AGENTS.md

Guide for AI coding agents working on Cupful. `PLAN.md` is the full brief; this file is the short, current-state version. Read both before writing code.

## What this is

Self-hosted food/exercise/steps tracker. Rust (Axum + SQLx/SQLite) API serving an embedded SvelteKit (Svelte 5, static SPA) frontend, one binary, deployed with Docker + a Tailscale sidecar. License AGPL-3.0-or-later, DCO sign-off on every commit.

## Status

- **Done:** M0 (scaffold), M1 (accounts, profile, budget engine, write contract), M2 (AI providers, nutrition pipeline, barcode product lookup; live check on oMLX), M3 (Today screen; owner phone-checked). Since M3: saved products in estimates (phrase aliases, product servings as portions), editable kcal, coral/sage/sand palette. M4 (Activity, ingest tokens + Shortcut endpoint, Trends with LayerChart, seed-demo, Google Health v4 steps sync moved up from v0.2; owner connected Google live). M5 (CSV export, Fuel Ledger import, JSON backup/restore, Settings → Data, `docs/import-export.md` and `docs/backups.md`; backend, Vitest and Playwright suites green).
- **Done:** M6 (PWA manifest and app shell, service worker with the update prompt, per-user IndexedDB read cache, queue-first logged-data writes with the flush identity guard, Settings → Sync detail, `docs/offline.md`; offline and PWA e2e, Vitest, svelte-check and build green). Owner-verified on Android: WebAPK install, airplane-mode meal and weigh-in sync, Activity/Trends from a cold offline start. Scanner autofocus works in Chrome; Brave may ignore focus hints. The IndexedDB cache has no eviction yet.
- **Next:** M7 (release v0.1.0: invites, Tailscale identity mode, `docs/self-hosting.md`, tag and published image).
- Open shortcuts: `grep -rn "ponytail:" server web/src`. Deviations from the plan: `docs/decisions.md`.

## Layout

```
server/                 Rust crate `cupful`
  src/main.rs           CLI: serve (default), reset-password, import-foods, seed-demo, eval
  src/config.rs         env config (CUPFUL_*)
  src/db.rs             pool + SQLite pragmas + migrations
  src/assets.rs         rust-embed of web/build (debug builds read disk); security headers + CSP with hashes of index.html's inline scripts
  src/auth/             mod.rs (User/Admin extractors, setup/login/logout), password.rs (argon2 behind a 4-permit semaphore), session.rs (SQLite session store), limits.rs (RateLimiter, CSRF, client IP)
  src/profile.rs        profile + profile_versions REST
  src/budget.rs         budget math, pure functions
  src/sync/             POST /api/sync write contract: mod.rs (ops, shared validators), meals.rs (MealIn, write_meal_on, tombstones), days.rs (DayPatch, patch_day, Source)
  src/reads.rs          GET days / meals / budget / trends (one `day_budget` shared by budget and trends)
  src/google/           Google Health API v4 steps sync, 404 unless configured: mod.rs (status, disconnect, daily task), oauth.rs (PKCE, state in session, encrypted tokens), sync.rs (dailyRollUp → days)
  src/ingest.rs         ingest tokens REST (hashed at rest) + POST /api/ingest/activity (bearer token, `Access::Token`, no CSRF header)
  src/demo.rs           `seed-demo`: deterministic demo user and history
  src/time.rs           user-local "today" helpers
  src/cleanup.rs        daily cleanup task
  src/transfer/         GET /api/export.csv, POST /api/import/fuel-ledger, GET /api/backup, POST /api/restore: backup/{doc,export,validate,restore}.rs, csv/{export,parse,import}.rs
  src/products/         mod.rs (shared helpers, set_portions, item_logged), off.rs (Open Food Facts lookup), label.rs (label entry, private to the user), saved.rs (saved products)
  src/ai/mod.rs         Provider trait, ProviderError, for_user/by_id, daily cap, config
  src/ai/http.rs        HttpProvider: retries, timeouts, scrubbing, reply parsing (no redirects followed)
  src/ai/{openai_compatible,anthropic,gemini}.rs   per-adapter request body and reply text
  src/ai/settings.rs    admin provider REST, Test connection, env seeding
  src/crypto.rs         secrets at rest: key loading, check value, `Secret`
  src/nutrition/        estimate pipeline: mod.rs (types, estimate), prompt.rs, measures.rs (units, portions, oil rule), foods/{text,seed,usda,find}.rs, routes.rs, golden.rs
  src/error.rs          API error type; JsonBody/Query extractors that reject with JSON
  src/lib.rs            AppState, route table (`route_table()`), router
  migrations/           sqlx migrations (never edit a released one)
  .sqlx/                committed offline query metadata
  tests/                integration tests (oneshot against the Router, in-memory SQLite)
  tests/isolation.rs    user A vs user B for every `Access::User` route
  tests/products.rs     barcode lookups against wiremock
web/                    SvelteKit app (adapter-static, SPA fallback); routes: / (Today), /activity (phone tab), /trends, /settings, /login, /setup
  src/lib/api.ts        API client, types for every endpoint, ApiError, errorMessage(e, context)
  src/lib/sync/         uuidv7, op builders, sendOps (the only logged-data write path), scaleItems
  src/lib/{format,date,theme}.ts, session.svelte.ts   formatting, dates, theme preference, session/day state
  src/lib/styles/       tokens.css (design tokens, light-dark()), global.css (shared classes)
  src/lib/{today,products,settings,activity,trends}/   screen components (activity/: ActivitySection + form helpers; trends/: Charts.svelte (LayerChart, lazy-loaded), ChartCard, logic.ts; settings/TokensSection: ingest tokens) (products/PhraseLinks.svelte: phrases that resolve to a product) (props documented in each file's header)
  src/service-worker.ts  SvelteKit built-in service worker: app-shell precache, navigation fallback, cache-first immutable assets, SKIP_WAITING
  src/lib/offline/       IndexedDB offline layer: db (queue/cache/meta), queue (flush, identity guard), cache (allowlisted reads), overlay, session-cache, state, update prompt
  static/manifest.webmanifest, static/icons/   PWA manifest and install icons
  e2e/, playwright.config.ts   Playwright against the real server
docs/design.md          design direction, tokens, component rules
fixtures/                shared test vectors and samples (budget_cases.json; M5 import/backup fixtures)
data/staples.csv, data/aliases.csv, data/portions.csv   shipped food data and USDA portion weights (embedded, seeded at startup)
data/tools/            build_staples.py + staples_spec.txt: regenerate staples.csv from USDA downloads
data/golden/            golden estimate cases with recorded model replies
docs/providers.md       AI provider setup
docs/google-health.md   Google Cloud setup for the optional Google Health steps sync
docs/shortcuts.md       sending steps from iOS Shortcuts / Android with an ingest token
docs/import-export.md   CSV export and Fuel Ledger import: format, ids, merge semantics
docs/backups.md         JSON backup/restore and the nightly SQLite copy
docs/offline.md         offline scope, write queue and identity guard, read cache, PWA install
docs/decisions.md       one line per deviation/notable choice
```

## Commands

```sh
# backend (from server/)
CUPFUL_SECURE_COOKIES=false cargo run -- serve
cargo run -- import-foods <FoodData_Central_*_food_json.json>   # Foundation or SR Legacy, unzipped
cargo run -- seed-demo --days 90 --seed 42 [--force]          # user `demo`, random password printed once; --force replaces only that user
cargo run -- eval --provider <id>                             # golden set against a live provider
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
# after a migration or query! change:
mkdir -p ../.cupful && export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db && sqlx database create && sqlx migrate run && cargo sqlx prepare -- --all-targets

# frontend (from web/)
npm run dev        # :5173, proxies /api -> :8080
npm test           # Vitest
npm run test:e2e   # Playwright (builds web, starts the Rust server; offline specs spawn their own); once: npx playwright install chromium
npm run check
npm run build
```

## Conventions

- Every source file starts with `SPDX-License-Identifier: AGPL-3.0-or-later`.
- TDD: failing test first. Backend integration tests use `tower::ServiceExt::oneshot` on the real router with in-memory SQLite (`tests/common/mod.rs` has helpers).
- SQL through `sqlx::query!`/`query_as!` macros so it's compile-checked; refresh `.sqlx/` after changes. Exception: FTS5 queries use runtime `query_as` (sqlx's describe crashes on virtual tables).
- JSON Schemas sent to AI providers: `additionalProperties: false` and full `required` on every object.
- Logged data (meals, days) is written **only** through `POST /api/sync`, except the import and restore endpoints that PLAN.md §6 lists under configuration writes. Config/account data uses plain REST, online only.
- Every mutating request needs header `X-Cupful: 1` (CSRF).
- Every user-owned route needs an isolation test; `tests/isolation.rs` fails if a route is not listed. `Access::Admin` routes use the `auth::Admin` extractor and are checked for 403/401 there too.
- Budget is computed only on the server (`budget.rs`); f64 throughout, round on serialize.
- Dates are the user's local date (profile timezone), TEXT `YYYY-MM-DD`. Times are INTEGER unix milliseconds in the DB; RFC 3339 on the wire.
- New routes go in `routes()` in `lib.rs` with their `Access`. Request bodies use `error::JsonBody`, queries `error::Query`, so errors are always `{"error": ...}`.
- Keep it small: no new dependency for what a few lines do. Mark deliberate shortcuts with `// ponytail: <ceiling>, <upgrade path>`.
- Record deviations from `PLAN.md` in `docs/decisions.md` as one line: date, what, why.
- Frontend logic gets a Vitest test (`*.test.ts` next to the code). Screens use tokens and `global.css` classes; no CSS framework or UI kit. Logged-data writes go only through `sync.sendOps`. `#lib` imports of TS files keep the `.ts` extension.
- Outbound HTTP goes only to configured AI providers, (unless `CUPFUL_PRODUCT_LOOKUP=off`) Open Food Facts, and, only when `CUPFUL_GOOGLE_CLIENT_ID`/`_SECRET` are set and a user connected it, Google (`accounts.google.com`, `oauth2.googleapis.com`, `health.googleapis.com`). Tests point base URLs at `wiremock`; `common::app()` has product lookup off. No telemetry, ever.

## Keeping this file current

Update this file and `CLAUDE.md` in the same commit whenever a milestone completes or the layout, commands or conventions change. Stale agent docs are a bug.
