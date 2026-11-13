# AGENTS.md

Guide for AI coding agents. Read this before writing code. The v0.1.0 brief was removed after release.

## What

Self-hosted food, exercise and steps tracker. Rust Axum + SQLx/SQLite API. Embedded SvelteKit 5 static SPA. One binary. Docker with an optional Tailscale sidecar. AGPL-3.0-or-later. Every commit needs DCO sign-off.

## Status

- M0 to M8 are done. M7 shipped v0.1.0: one-time invites, Tailscale identity mode, rewritten self-hosting docs, and the multi-arch image on GHCR. M8 refreshed the UI: the Mineral palette on the Pantry layout, self-hosted Manrope, new Today, Activity, Trends and Settings screens, server-side trend averages and meal mix, and new docs screenshots.
- After M8: v0.2.0 shipped meal history as a repeat library (`GET /api/meals/history`, Trends "Eaten most", the recent-meals sheet, review-first repeat and copy-to-today) and the calmer motion scale with the shared sheet exit lifecycle.
- v0.2.1 fixed PWA reachability on Android and Tailscale: real requests and a `/api/health` probe drive the online state, not `navigator.onLine`. v0.2.2 stopped the offline bar flashing during a boot-time blip.
- v0.3.0 shipped the optional fixed daily calorie goal: the goal is stored on each profile version, manual mode keeps the goal every day and shows the gap to maintenance, and backups carry it.
- v0.3.1 added week-strip tooltips for steps, gym minutes, trained parts and workouts, plus a live GitHub star link backed by a cached public endpoint.
- v0.3.2 softened the light theme panels to `#f8fafa` and kept input outlines at 3:1 contrast.
- After v0.3.2: Review builds multi-product meals (optional name, appended products, inline scan or label entry, one save), Today offers the usual meal for the current slot as a one-tap chip, product rows use household units, and saved products are offered on exact AI item matches.
- Releases use an explicit version-bump pull request; `main` is protected, and `release.yml` publishes both architectures, tags, and creates the GitHub release after merge.
- Ember jug redesign (branch `ember-jug-redesign`): graphite and ember palette, Bricolage Grotesque and Instrument Sans, one motion scale (`--dur-*`, `#lib/motion`), the jug budget on Today, a designed desktop layout (top bar, two-column Today, grids on Activity and Trends, Settings section list), and one jug logo across app, favicon, PWA icons and README. The offline cache now evicts day reads beyond the 60 newest dates.
- Next: v1.0. Release checklist: backup format version with a v0.3.2 restore test and upgrade notes, a security review, an axe accessibility check in Playwright, and the Brave camera note in the docs.
- Known gaps: Brave may ignore camera focus hints.
- Shortcuts: `grep -rn "ponytail:" server web/src`. Decisions: `docs/decisions.md`.

## Layout

```
server/                 Rust crate `cupful`
  src/main.rs           CLI: serve, reset-password, import-foods, seed-demo, eval
  src/auth/             setup, login, sessions, passwords, limits, extractors
  src/sync/             POST /api/sync: meals, days, validators
  src/reads.rs          budget, meals, days, trends reads
  src/history.rs        meal history grouping, recent-meals endpoint, top meals
  src/nutrition/        estimate pipeline, prompt, measures, food matching
  src/ai/               providers, HTTP hardening, admin settings
  src/products/         barcode lookup, label entry, saved products with household portions
  src/transfer/         CSV export/import, backup/restore
  src/google/           optional Google Health sync: steps (sync.rs), workouts + gym minutes + gym types (workouts.rs)
  src/{budget,profile,ingest,demo,cleanup,time,error,crypto,assets,config,db}.rs
  migrations/           never edit a released migration
  .sqlx/                committed offline query metadata
  tests/                integration tests on the real router, in-memory SQLite
  tests/{isolation,products}/   route isolation and barcode wiremock suites
web/                    SvelteKit app: /, /activity, /trends, /settings, /profile, /login, /setup, /invite
  src/lib/api.ts        API barrel. `api/` holds types and endpoints by resource
  src/lib/sync/         op builders, uuidv7, sendOps
  src/lib/offline/      IndexedDB queue, cache, overlay, session cache, sync UI
  src/lib/{today,products,settings,activity,trends}/   screens and their logic
  src/lib/today/RepeatSheet.svelte   recent meals picker, opens Review
  src/lib/today/ReviewItem.svelte    one Review row, its product and unit picker
  src/lib/today/UsualMeal.svelte     one-tap usual chip and its alternatives
  src/lib/today/usual.ts             usuals selection, dismissal, payload
  src/lib/today/productMatch.ts      exact saved-product match for AI items
  src/lib/today/{Jug,TodayActivity,TodayWeek}.svelte   jug budget, steps and gym tiles, desktop week card
  src/lib/activity/day.svelte.ts     Activity page controller (day, window, saves)
  src/lib/settings/summaries.ts      one-line Settings row summaries from loaded state
  src/lib/motion/                    CountUp and its tween; durations come from --dur-* tokens
  src/lib/styles/                    tokens.css, then global.css importing base, buttons, forms, segmented, components, sheet, motion
  src/lib/trends/FrequentMeals.svelte  "Eaten most" card
  src/lib/sheet.ts      dialog exit lifecycle shared by the sheets
  static/fonts/         self-hosted Bricolage Grotesque and Instrument Sans woff2 and their OFL licences
  src/service-worker.ts PWA shell cache, navigation fallback, update prompt
  e2e/                  Playwright against the real server
docs/                   design, offline, import-export, backups, providers, google-health, shortcuts, decisions
data/                   staples, aliases, portions, USDA tools, golden estimates
fixtures/               shared test vectors and samples
scripts/                install-hooks, bump-version, check-version
```

## Commands

```sh
# repo root, once per clone: pre-commit runs fmt, svelte-check and Vitest on staged changes
./scripts/install-hooks.sh

# release pull request, from repo root
node scripts/bump-version.mjs 0.4.0
node scripts/check-version.mjs

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
- Outbound HTTP goes only to configured AI providers, Open Food Facts (unless `CUPFUL_PRODUCT_LOOKUP=off`), Google when configured and connected, and the public GitHub API for the nav star count (cached hourly, no user data). No telemetry.
- No new dependency for a few lines of code. Mark a deliberate shortcut with `// ponytail: <ceiling>, <upgrade path>`.
- Record a deviation or notable choice in `docs/decisions.md`: date, what, why.
- Frontend logic gets a Vitest test next to it. Screens use tokens and `global.css` classes. No CSS framework. Writes go through `sync.sendOps`. Keep `.ts` in `#lib` imports.
- Update this file and `CLAUDE.md` when a milestone completes or the layout, commands or rules change.
