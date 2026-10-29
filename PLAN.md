# Cupful — build plan

Cupful is an open-source, self-hosted app for tracking food, exercise and steps. You log meals the way you'd describe them to a friend ("a small cup of rice and one shallow-fried chicken breast"), and Cupful turns that into calories and macros using whichever AI model you connect: a local model, OpenAI, Anthropic, Gemini, or any other provider. It runs on your own server, installs on your phone as a PWA, and is meant to be reached privately over Tailscale.

This document is the brief for building it. Read all of it before writing code.

---

## 0. Before M0

These block the crate name, container image, README and license file, so settle them first:

1. **Name:** `cupful`. Confirm it is free on GitHub, crates.io and the container registry.
2. **License: AGPL-3.0-or-later**, with contributions accepted under the **Developer Certificate of Origin (DCO)**. No CLA.
   - `LICENSE.md` holds the unmodified official license text. Fetch it from gnu.org (`agpl-3.0.md`, or `agpl-3.0.txt` if the Markdown version isn't available); never write or paraphrase the license from memory.
   - Set the license metadata to `AGPL-3.0-or-later`: `license` in `Cargo.toml` and `package.json`, and an `SPDX-License-Identifier: AGPL-3.0-or-later` header in each source file.
   - The README has a License section naming AGPL-3.0-or-later, with a one-paragraph plain-language summary: self-hosting and personal changes are fine; anyone offering a modified version as a network service must publish their changes.
   - **AGPL network clause:** Settings → About shows the running version and a "Source code" link to the repository. The link is configurable (`CUPFUL_SOURCE_URL`) so modified deployments can point to their own source.

**When creating the project (M0), add these files in the first commit:**
- **`LICENSE.md`** as described above.
- **`CONTRIBUTING.md`**, containing:
  - **DCO sign-off:** every commit must carry a `Signed-off-by:` line (`git commit -s`), with a link to developercertificate.org and one sentence on what signing off means. CI checks sign-off on every pull request; a commit without it fails the check.
  - **License of contributions:** contributions are accepted under AGPL-3.0-or-later.
  - **Dev setup:** the dev workflow (section 12) and how to run each check (section 13).
  - **Workflow:** the TDD expectation; the review checklist for anyone without the ponytail plugin (section 13); recording deviations in `docs/decisions.md`.
  - **Pull requests:** expectations for size and description, and the "no telemetry" rule.

  Extend it in later milestones as the project grows; don't leave it as a stub.
- **`AGENTS.md`** and **`CLAUDE.md`**: the agent guide (section 1, "Agent docs"). Kept current from then on.

---

## 1. How to work on this project

### Skills to use (installed locally)

| Skill / plugin | When to use it |
|---|---|
| **ponytail** (plugin) | Always on while writing code, at level `full`. Before adding code, go through its ladder: does this need to exist, can existing code do it, does the standard library or platform already do it, does an installed dependency do it, and only then write the smallest solution. Mark deliberate shortcuts with `shortcut:` comments that say when to revisit them. Run `/ponytail-review` on the diff before every commit, and `/ponytail-audit` at the end of each milestone. Run `/ponytail-debt` at milestone end and list the open shortcuts in the milestone summary. |
| **test-driven-development** | All backend logic and all frontend logic that computes something. Red, green, refactor: write the failing test first, make it pass with the least code, then clean up. This skill sets the testing standard for the project. Where ponytail's "one small test" rule and TDD differ, follow TDD. |
| **frontend-design** | Before building any screen. Use it to set the visual direction (palette, type, spacing, component style, chart styling) and write the result into `web/src/lib/styles/tokens.css` plus a short `docs/design.md`. Build every screen from those tokens. |
| **frontend-design-impeccable** | After a milestone's screens work end to end. A polish pass over spacing, hierarchy, states, motion, dark mode, touch targets and chart readability. Don't use it to change behavior, and don't defer functional states (errors, empty, offline) to it; those belong in the milestone that builds the screen. |

At the start of M0, confirm each of these skills actually loads in the tool doing the build. If one doesn't, say so and use the fallback review checklist in `CONTRIBUTING.md` (section 13) instead of silently skipping the step.

### Working rules

- Work milestone by milestone (section 14). Finish one, including its tests and its review, before starting the next.
- At the start, check the current stable versions of every dependency named here and pin them. Don't rely on remembered versions or APIs; read the current docs for Axum, SQLx, Svelte 5, SvelteKit, LayerChart, `@vite-pwa/sveltekit`, the Tailscale container image, and each AI provider's API.
- Keep the codebase small. This is a self-hosted app for individuals and households, not a SaaS platform. No microservices, no message queues, no ORMs beyond SQLx.
- Commit in small steps with clear messages. Keep `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo sqlx prepare --check`, `svelte-check` and the frontend tests green on every commit.
- **Deviations:** when something in this plan turns out to be wrong, impossible or worse than an alternative, stop and say so. Then record the decision as one line in `docs/decisions.md` (date, what changed, why). Don't work around the plan silently.
- **Agent docs:** `AGENTS.md` is the short, current-state guide for AI coding agents (status, layout, commands, conventions); `CLAUDE.md` imports it (`@AGENTS.md`) and adds Claude Code specifics. Update both in the same commit whenever the layout, commands, conventions or milestone status change. A milestone isn't done while they're stale.

---

## 2. Product scope

### Goals for v0.1
1. Log meals in natural language, optionally with a photo, and get per-item calories, protein, carbs and fat that the user can review, adjust and save. Packaged foods can be logged by scanning their barcode and choosing servings.
2. Log steps, gym sessions (minutes and body parts trained) and weight for each day, by hand or from an automation that posts to Cupful.
3. Show a daily calorie budget that moves with that day's activity, with macro targets and progress.
4. Show trends: intake against budget, weight with a rolling average, and steps.
5. Let each instance connect any AI provider, including fully local models, with no paid key required to run the app.
6. Install as a PWA on Android and iOS, and keep working offline for logging.
7. Be easy for anyone to self-host: one `docker compose up`, private by default over Tailscale.

### Platform reality for steps
- **Android:** manual entry, the Shortcut/automation endpoint (section 11), and, in M4, the Google Health integration (section 15; optional, needs your own Google Cloud OAuth client).
- **iOS:** a PWA cannot read HealthKit. The supported path is manual entry or an iOS Shortcuts automation that reads step count from Health and posts it to Cupful's ingest endpoint. Document this plainly in the README.

### Non-goals for v0.1
- Hosted or shared Google Health credentials. The integration itself moved into M4 (section 15); each self-hoster brings their own OAuth client.
- Public SaaS hosting, billing, or social features.
- Native mobile apps.
- Recipes, meal planning, coaching or chat. Don't build hooks for them now. (Barcode scanning was moved into scope; see section 8, "Packaged products".)
- Medical advice. The app shows estimates and targets from standard formulas; it never diagnoses anything.

---

## 3. Stack

| Layer | Choice |
|---|---|
| Backend | Rust, **Axum** on **Tokio** |
| Database | **SQLite** through **SQLx** (compile-time checked queries, `sqlx migrate`, offline metadata in `.sqlx/`), FTS5 for food search |
| Auth | `argon2` (argon2id) password hashes, `tower-sessions` with a SQLite store |
| HTTP client | `reqwest` + `serde` for every AI provider (no provider SDKs unless one clearly saves code) |
| Secrets at rest | `chacha20poly1305` (XChaCha20-Poly1305) |
| IDs | UUIDv7, generated by the client for every meal and meal item |
| Background jobs | Tokio tasks with intervals; no job queue |
| Logging | `tracing` + `tracing-subscriber` |
| Frontend | **Svelte 5** (runes) with **SvelteKit** in static mode (`adapter-static`, SPA fallback), TypeScript |
| Charts | **LayerChart** only (section 12) |
| PWA | `@vite-pwa/sveltekit` (manifest + service worker via Workbox) |
| Frontend tests | Vitest for logic; Playwright for end-to-end flows |
| Backend tests | `cargo test`, Axum `Router` tested with `tower::ServiceExt::oneshot`, in-memory SQLite, `wiremock` for provider HTTP mocks |
| Packaging | One Rust binary that serves the API and the built frontend (embedded with `rust-embed`) |
| Deployment | Docker image (multi-stage, small final image) + Compose file with a Tailscale sidecar |

### Repository layout

```
cupful/
├── server/                 # Rust crate
│   ├── src/
│   │   ├── main.rs         # also the CLI: serve, reset-password, import-foods, seed-demo, eval
│   │   ├── config.rs
│   │   ├── auth/           # setup, login, sessions, tailscale identity mode
│   │   ├── budget.rs       # the budget math (pure functions)
│   │   ├── sync.rs         # the write contract (section 6)
│   │   ├── nutrition/      # parse, validate, measures, match, oil rule
│   │   ├── ai/             # provider trait + adapters
│   │   ├── crypto.rs       # secrets at rest
│   │   ├── routes/
│   │   └── db/
│   ├── migrations/
│   ├── .sqlx/              # committed SQLx offline metadata
│   └── tests/
├── web/                    # SvelteKit app
│   └── src/lib/
│       ├── styles/tokens.css
│       ├── charts/
│       ├── sync/           # write queue (same contract online and offline)
│       └── api.ts
├── data/
│   ├── staples.csv         # curated staple foods with sources
│   ├── aliases.csv         # aliases for staples and common USDA foods
│   └── golden/             # golden test cases for the nutrition pipeline
├── fixtures/
│   └── budget_cases.json   # shared budget test vectors
├── docs/                   # design.md, decisions.md, self-hosting.md, providers.md, shortcuts.md
├── Dockerfile
├── compose.yaml
├── .env.example
├── README.md
├── CONTRIBUTING.md
├── AGENTS.md               # agent guide: status, layout, commands, conventions
├── CLAUDE.md               # @AGENTS.md + Claude Code specifics
└── LICENSE.md
```

---

## 4. Architecture

```
Phone (PWA) ──HTTPS via Tailscale──▶ cupful binary (Axum)
                                       ├── embedded SvelteKit build
                                       ├── /api/* JSON API (reads: GET, writes: POST /api/sync)
                                       ├── SQLite file (/data/cupful.db)
                                       ├── AI adapter ──▶ any provider (local or hosted)
                                       ├── food database (in SQLite, FTS5)
                                       └── /api/ingest/activity ◀── iOS Shortcuts / Android automations
```

- The binary listens on plain HTTP on a configurable address. HTTPS is provided by what sits in front of it: Tailscale by default, or a reverse proxy the self-hoster chooses.
- All provider keys stay on the server. The browser never sees them.
- The frontend talks only to `/api/*` on the same origin.
- **SQLite settings at startup:** `journal_mode=WAL`, `busy_timeout=5000`, `foreign_keys=ON`, `synchronous=NORMAL`. Test that two concurrent writers don't hit `database is locked`.

---

## 5. Data model (first migrations)

Every user-owned row carries `user_id`. Plan for several users per instance (a household) from the start.

- **`users`** — id, username (unique), password_hash (nullable when using Tailscale identity), tailscale_login (nullable, unique), is_admin, created_at.
- **`profiles`** — user_id, timezone (IANA name, set from the browser at setup), weight_unit (kg/lb), height_unit (cm/ftin).
- **`profile_versions`** — id, user_id, effective_from (date), sex (for the BMR formula: male/female), birth_year, height_cm, start_weight_kg, goal_kg (nullable), pace_kg_per_week, typical_steps. Saving the profile inserts a new version effective from the user's today. Budget math for any date uses the version in effect on that date (section 7).
- **`days`** — user_id, date, and for each field a value, a source and an update time:
  - steps, steps_source, steps_updated_at
  - gym_minutes, gym_source, gym_updated_at
  - trained (JSON array of body parts), trained_updated_at
  - weight_kg, weight_updated_at
  - Unique on (user_id, date). Sources: `manual`, `shortcut`, `import` and `google`.
- **`meals`** — id (client-generated UUIDv7, primary key), user_id, date, meal_type (breakfast/lunch/dinner/snack), description (what the user typed), scale (portion multiplier, default 1), assumptions, confidence (low/medium/high), source (ai/manual/repeat/import), provider_id (nullable), updated_at (effective time, section 6), received_at, deleted_at (nullable; tombstone).
- **`meal_items`** — id (client-generated UUIDv7), meal_id, name, portion_text, grams (nullable), kcal, protein_g, carbs_g, fat_g, food_id (nullable), origin (`food_db` or `ai_estimate`).
- **`foods`** — id, source (`staple`/`usda`/`user`/`off`), source_ref (USDA FDC ID, citation, user id, or Open Food Facts barcode), barcode (nullable, unique; GTIN digits), brand (nullable), serving_g (nullable; one label serving), serving_text (nullable; e.g. "1 biscuit (12 g)"), fetched_at (nullable; for `off` rows), name, name_norm, is_prepared (true for entries that already include cooking fat, e.g. "fried"), kcal, protein_g, carbs_g, fat_g per 100 g, default_portion_g (nullable).
- **`food_aliases`** — food_id, alias, alias_norm, user_id (nullable; null = shipped alias, set = a user's own alias).
- **`foods_fts`** — FTS5 table over food names and aliases.
- **`user_portions`** — user_id, phrase_norm ("my usual chicken"), food_id, grams.
- **`user_products`** — user_id, food_id, last_servings (the serving count last logged), last_used_at. A user's saved packaged products, for quick re-logging (section 8).
- **`ai_providers`** — id, label, kind (`openai_compatible`/`anthropic`/`gemini`), base_url, model, api_key (encrypted, nullable), extra_headers (encrypted JSON, nullable), supports_vision, structured_output_mode, is_default_text, is_default_vision.
- **`ai_usage`** — user_id, date (the user's local date, not UTC), provider_id, calls. Used for the daily call cap.
- **`sync_ops`** — op_id, user_id, received_at. Used only to report duplicates; cleaned up after 30 days.
- **`ingest_tokens`** — id, user_id, label, token_hash, created_at, last_used_at.
- Sessions table as required by `tower-sessions`.

### Photos
Photos are **transient**:
- **Server:** held in memory for one estimate request, sent to the configured provider, and never written to disk or the database. There is no photo column.
- **Client:** the browser keeps the photo until the user saves or discards the estimate, so a retry works without re-taking it.
- **Metadata stripped before upload.** The client re-encodes the photo through a canvas (which also downscales it), and that removes EXIF data, including GPS location. Test that an image with GPS EXIF comes out without it.
- **Type checked server-side** by magic bytes (JPEG, PNG, WebP). The declared content type and file size alone don't prove it's an image.
- **Composer text:** next to the photo button, "Photos are used for this estimate only and aren't saved." Settings also says that photos are sent to the configured provider.

### Secrets at rest
- API keys and extra headers are encrypted with XChaCha20-Poly1305, a random 24-byte nonce per value, stored as `v1:` + base64(nonce ‖ ciphertext). The version prefix allows rotation later.
- **Which key is used:** `CUPFUL_SECRET_KEY` if set; otherwise `/data/secret.key`. If neither exists **and the database holds no encrypted values yet**, generate `/data/secret.key` (mode 0600) and log that it was created. That way `docker compose up` works with no extra step.
- **Key check:** on first encryption, store a check value (a known constant, encrypted) in the database. At startup, verify the chosen key decrypts it. If it doesn't — for example, the env var was removed after secrets were saved with it — refuse to start with a message naming both key sources. Never generate a new key over existing encrypted data.
- Document the trade-off: a key stored next to the database protects less than one held separately.
- Document that losing the key means re-entering provider keys. Nothing else is lost.

---

## 6. Write contract and sync

### Two write paths, by design
- **Logged data (meals and days)** is written only through `POST /api/sync`, online or offline. There is one code path, and offline support in M6 is a queue in front of a contract that already exists and is tested.
- **Account and configuration** is written through ordinary REST endpoints with their own validation. These are online only; the UI disables them when offline. This covers:
  - profile and profile versions, timezone and units;
  - AI providers;
  - ingest tokens;
  - users and invites;
  - imports and restores.

A new kind of data goes through sync only if it must be writable offline. Record that decision in `docs/decisions.md`.

`POST /api/sync`
```json
{"ops":[
  {"op_id":"<uuidv7>","kind":"meal.upsert","at":"2026-10-27T18:02:11Z","payload":{ "id":"<uuidv7>", "date":"2026-10-27", "meal_type":"lunch", "...": "full meal with all items" }},
  {"op_id":"<uuidv7>","kind":"meal.delete","at":"...","payload":{"id":"<uuidv7>"}},
  {"op_id":"<uuidv7>","kind":"day.patch","at":"...","payload":{"date":"2026-10-27","weight_kg":81.4}}
]}
```
Response: one result per op, in order: `applied`, `duplicate` (already applied; same op_id), `stale` (a newer write already won), or `rejected` with a message.

Rules:
- **Idempotent.** Meal and item ids are client-generated UUIDv7s. Re-sending an upsert with the same id updates the meal rather than duplicating it. An id that belongs to another user is rejected.
- **Full payloads.** `meal.upsert` always carries the complete meal and its items, including repeat meals logged offline. Never send a reference to another meal.
- **Items are replaced, not merged.** After an upsert, the meal's items are exactly the ones in the payload. Delete any stored item that isn't in the payload. Test that removing an item lowers the meal total.
- **The portion multiplier is applied before saving.** The client multiplies item grams and nutrients by the chosen multiplier and sends the result. `meals.scale` is stored only so the UI can show "×1.5" and reopen the review; it is never applied again on read. Test that a ×1.5 meal's stored items already include the multiplier.
- **Client clocks are not trusted blindly.**
  - The server records its own `received_at` for every op.
  - An op whose `at` is more than 5 minutes ahead of server time is `rejected`.
  - The effective time is `min(at, received_at)`.
  - An op whose `at` is older than the tombstone retention window (180 days) is `stale`. So a device that has been offline longer than that cannot bring a deleted meal back; it only loses those old writes. Accept this failure mode and document it.
- **Field-level days.** `day.patch` touches only the fields present. Each field resolves last-write-wins by effective time against that field's `_updated_at`, so one device's weight and another's steps never overwrite each other.
- **Meals** resolve last-write-wins on the whole meal by effective time. A delete leaves a tombstone, and a later upsert of a deleted id is `stale`. Tombstones are kept for 180 days.
- **Manual beats automated.** An automated source (`shortcut`, `import`, `google`) never overwrites a field whose current source is `manual`. Among automated sources, latest wins.
- **Dates** are the user's local date, sent by the client. The server rejects dates after the user's today (computed in their profile timezone) plus one day of slack.
- **`op_id`s** are remembered for 30 days, only to report `duplicate`. Correctness doesn't depend on them, because upserts and patches are idempotent anyway.
- **Cleanup:** a daily task deletes expired `op_id`s, tombstones older than 180 days, and expired sessions.
- **Limits:** at most 100 ops per request; reject oversized payloads.

Reads are plain GETs:
- `/api/days/{date}` and `/api/days?from=&to=`; ranged reads cover at most 400 days per request.
- `/api/meals?date=`
- `/api/budget/{date}`
- `/api/trends?from=&to=`

---

## 7. Budget math

Put this in `server/src/budget.rs` as pure functions. The frontend never computes the budget; it shows what the server returns. Offline, it shows the last known budget and sums the meals locally.

### Inputs for a date
- **Profile:** the `profile_versions` row in effect on that date.
- **Weight:** the most recent logged weight on or before that date, else the version's start weight.
- **Age:** `year(date) − birth_year`. This is approximate by up to a year, which shifts the BMR by about 5 kcal; document it.
- **Steps:** that day's steps; if none are entered yet, the version's typical steps, with the budget flagged as provisional.

### Formula
- **BMR (Mifflin–St Jeor):** `10·kg + 6.25·cm − 5·age + 5` (male formula) or `− 161` (female formula).
- **Base:** `BMR × 1.2`.
- **Steps:** `max(0, steps − 2500) × 0.00057 × kg` (about 0.04 kcal per step at 70 kg, counting only steps beyond the everyday movement already inside the 1.2 factor).
- **Gym:** `(MET − 1) × kg × minutes / 60`, with MET 5 for weight training and 7 when the only thing trained is cardio.
- **Maintenance:** `base + steps + gym`.
- **Deficit:** `pace_kg_per_week × 7700 / 7`, capped at 25 % of maintenance. Flag `deficit_capped` when the cap applies.
- **Budget:** `min(maintenance, max(floor, maintenance − deficit))`. The floor is 1500 kcal (male formula) or 1200 kcal (female formula); it is a convention, not a derivation, and is documented as such. Flags:
  - `floor_applied` when the floor raises the budget.
  - `floor_above_maintenance` when the floor is above maintenance. Then the budget equals maintenance and the UI says no deficit is planned for that day.
- **Why the outer `min` stays:** it matters only for the floor: with the deficit capped, `maintenance − deficit` never exceeds maintenance, but the floor can.
- **Macros:**
  - Protein: `protein_g = 1.6 × (goal_kg or current kg)`, and `protein_kcal = 4 × protein_g`.
  - Fat: `fat_g = max(0, min(0.25 × budget, budget − protein_kcal)) / 9`.
  - Carbs: `carbs_g = max(0, budget − protein_kcal − 9 × fat_g) / 4`.
  - **Over-committed macros.** Flag `macros_overcommitted` when `protein_kcal + 0.25 × budget > budget`. In that case carbs come out at 0 and fat gets whatever protein leaves. The UI shows the protein and fat targets and says that protein takes up most of the budget. It must never show a silent 0 g carbs target.

### History
Budgets are always **recomputed from current data**. Because profile changes are versioned and weights are dated, editing the profile today doesn't change past budgets. Backfilling a weigh-in changes the budgets from that date up to the next weigh-in, which is the correct result. The Trends screen says that past budgets use the weights recorded for those days.

### Rounding
Compute in f64 throughout and round once, when serializing: kcal to integers (half away from zero), grams to one decimal.

### Tests
Table-driven tests from `fixtures/budget_cases.json` must cover each flag, both formulas, the deficit cap, the floor above maintenance, over-committed macros, provisional steps, profile version boundaries and age at year boundaries. The API returns every term and flag, so the UI can show "how today's budget is worked out".

---

## 8. Nutrition pipeline

**What this does and doesn't promise.** Numbers come from the food database whenever a food matches. When nothing matches, the item uses the model's own estimate and is labelled "AI estimate" in the UI. Results are consistent across providers only in part: temperature 0 helps, but how a 3B local model and a frontier model read amounts will still differ. `cupful eval` (below) measures how much.

### Steps
1. **Parse.** Send the description (and photo if any) to the user's chosen provider at temperature 0. Ask for JSON only:
   ```json
   {"items":[{"food":"cooked white rice","amount":1,"unit":"small cup","grams":150,
              "cooking":"none","mentions_oil":false,
              "estimate":{"kcal":195,"protein":4,"carbs":43,"fat":0.4}}],
    "assumptions":"one short sentence","confidence":"medium"}
   ```
2. **Validate** before anything reaches the UI:
   - At most 15 items, and the description at most 1000 characters.
   - Grams greater than 0 and at most 2000 per item; otherwise derive grams from the unit table, or reject the item.
   - The estimate must make sense: `4·protein + 4·carbs + 9·fat` within 20 % (+10 kcal) of the estimated kcal. If it isn't, recompute kcal from the macros.
   - Derive the estimate per 100 g from grams. If it is outside 0–900 kcal per 100 g, the estimate is implausible: drop it, and the item needs either a database match or a manual edit.
3. **Household measures.** Check `user_portions` first. Otherwise convert units with a table in code:
   - Weights: g, kg, oz, lb; volumes: ml, l (as grams).
   - Small cup of cooked rice ≈ 150 g, cup ≈ 200 g, plate of rice ≈ 300 g.
   - Bowl of curry, dal, soup, stew or chili ≈ 250 ml.
   - Drinks: cup ≈ 240 ml, glass or mug ≈ 250 ml, can ≈ 355 ml.
   - Teaspoon of oil or butter ≈ 4.5 g, tablespoon ≈ 13.5 g; other foods 5 g and 15 g.
   - Roti ≈ 40 g; chicken breast ≈ 130 g cooked.
   - Anything else (a slice, a bagel, a taco) uses the matched food's USDA portion.
   - **Grams come from, in order:** the user's saved portion; the household measures above; USDA's own portion for that unit of the matched food (`food_portions`, e.g. 1 scoop of whey ≈ 29 g, 1 cup of rolled oats = 81 g); a generic teaspoon (5 g) or tablespoon (15 g); the model's grams; the matched food's default portion. Measured portions beat the model because small local models misjudge weights badly.
   - **Plausibility:** a measured or model weight must be within 10× of the matched food's normal portion (for one portion or for the stated count), else it's treated as a count echoed as a weight ("1 g" of salmon) and the next source is used.
   - **Missed foods:** if the description names a known food (by alias) that no item covers, the review shows "The estimate may have missed: …" so the user can add it. Small models drop items ("a splash of milk").
4. **Match.** Normalize the name (lowercase, strip punctuation, singularize simple plurals). Then:
   1. Look for an exact alias in `food_aliases` (the user's own aliases first).
   2. Otherwise run an FTS5 search (bm25) over names and aliases, preferring `staple` foods.
   3. Classify the result:
      - **Matched:** the top score is above the high threshold, with a clear margin over the second.
      - **Ambiguous:** the top results are close, or the score falls between the thresholds. Return up to 3 candidates; the review UI asks, e.g. "Chicken breast or thigh?".
      - **Unmatched:** keep the model's estimate, `origin = ai_estimate`.

   Thresholds are named constants, tuned with the golden tests.
5. **Cooking oil.** Add a separate "Cooking oil" item (default 2 tsp per portion, editable) only when all three are true:
   - the cooking method uses oil (shallow-fried, pan-fried, sautéed, roasted in oil, tadka);
   - the matched food is not `is_prepared` (a USDA "fried" entry already includes the fat);
   - the user didn't list oil themselves.
6. **Review.** The UI lists each item with where its numbers came from, any ambiguity choices, editable grams, removable items, and a whole-meal portion multiplier (×0.5 … ×2). Nothing is saved without confirmation.
7. **Repeat meals** re-log a saved meal's items with one tap and no model call.

### Food database
- **Staples table (`data/staples.csv`)** — shipped with the app. A few hundred everyday foods with no cuisine favored over another: what people commonly eat across American, European, Latin American, Middle Eastern, East, Southeast and South Asian cooking.
  - **Grains, breads and breakfast:** rice, pasta, noodles, quinoa, couscous, oats and cereal, breads, tortillas, pita, roti, bagels, pancakes.
  - **Legumes:** beans, lentils, chickpeas and dals, tofu, tempeh, hummus.
  - **Eggs, meat and fish:** eggs every common way, chicken, beef, pork, lamb, turkey, deli meats, common fish and seafood.
  - **Dairy and alternatives:** milk, yogurt, cheeses, plant milks.
  - **Fats, sauces and condiments.**
  - **Vegetables and fruit:** the common ones fresh and cooked.
  - **Common mixed dishes:** pizza, burgers, sandwiches, burritos and tacos, pasta dishes, curries, stir-fries, fried rice, sushi, soups, salads.
  - **Snacks, sweets and drinks:** chips, bars, cookies, cake, chocolate, juices, soda, coffee drinks, beer and wine.

  Built by `data/tools/build_staples.py` from `data/tools/staples_spec.txt` (name, FDC id, prepared flag, portion hint), so every number comes from a USDA download. Grow it by adding spec lines and aliases, not by typing values.

  Every row carries its source (a USDA FDC ID, or another source whose license allows redistribution). Don't copy values from sources with unclear licenses, including IFCT. The owner reviews this file before release.
- **USDA FoodData Central** (Foundation and SR Legacy; public domain), loaded with `cupful import-foods`. Ship `data/aliases.csv` mapping everyday names to USDA entries ("chicken breast" → the right roasted entry), because raw USDA names like "Chicken, broilers or fryers, breast, meat only, cooked, roasted" match poorly on their own.
- Users can save an item as their own food or alias from the review screen.

### Packaged products (barcode)
- **Lookup:** `GET /api/products/{barcode}`. The barcode must be 8–14 digits with a valid GTIN check digit. The server looks in `foods` first (any user's earlier scan, or a label someone entered); otherwise it asks **Open Food Facts** (`/api/v2/product/{barcode}` with a `fields` list; check the current API docs and send the `User-Agent` they require, e.g. `Cupful/<version> (+<CUPFUL_SOURCE_URL>)`). Hits are stored as `foods` rows with source `off`, so each product is fetched once per instance. Refetch `off` rows older than 90 days on the next lookup; if that fails, keep the stored copy.
- **Mapping:** per-100 g kcal, protein, carbohydrates and fat from Open Food Facts' nutriments; kcal from kJ ÷ 4.184 when only energy in kJ is given. The serving size comes from its serving quantity in grams, else no serving (grams only). A product with no usable per-100 g values counts as not found. Run the same plausibility check as section 8 step 2 (0–900 kcal per 100 g).
- **Not found:** the UI offers "Enter from the label": name, brand, per-100 g (or per-serving plus serving grams) values. Saved as a `foods` row with source `user`, the barcode, and the user as `source_ref`, so the next scan of that barcode on this instance finds it. A user's own label entry wins over an `off` row for that user.
- **Logging:** the user chooses servings (0.5 steps) or grams. The client computes the item (grams × per-100 g values) and logs it through `/api/sync` like any meal item, with `food_id` set and origin `food_db`. Meal source `manual`.
- **Saved products:** `GET /api/products` lists the user's `user_products`, most recent first, with each food's values. A sync'd meal item whose food has a barcode upserts the `user_products` row (servings and last-used time). Users can remove a product from their list (`DELETE /api/products/{food_id}`, online only); the shared `foods` row stays.
- **Offline:** scanning a product already in the saved list works offline (the client caches the list); a new product needs the network.
- **Privacy and switch:** each lookup tells Open Food Facts which barcode was scanned. On by default; `CUPFUL_PRODUCT_LOOKUP=off` disables the outbound call (label entry still works). Documented in `.env.example`, `docs/self-hosting.md`, and Settings → About.
- **License:** Open Food Facts data is under the Open Database License (ODbL). Settings → About and the product sheet credit "Product data from Open Food Facts (ODbL)" with a link. Stored `off` rows stay out of any shipped data file.
- **Hardening:** connect 5 s, total 10 s, no retries beyond one on 5xx; a timeout reads as "lookup failed, enter from the label". Lookups are rate limited per user (60 per hour). `wiremock` tests cover hit, miss, kJ-only, missing serving, implausible values, timeout, refetch fallback, and that `CUPFUL_PRODUCT_LOOKUP=off` makes no request.

### Golden tests and eval
- `data/golden/*.json` holds fixed descriptions with the expected foods, match status and grams (±15 %).
- CI runs the pipeline on **recorded** model outputs for these cases, which keeps it deterministic. It covers matching, ambiguity, the oil rule, `is_prepared`, validation and unit conversion.
- Each recorded output stores the provider, the model, and the date it was recorded, so stale recordings are visible.
- `cupful eval --provider <id>` runs the golden set against a live provider and reports per-item accuracy. It is not part of CI. Use it to compare models and to tune thresholds.

---

## 9. AI providers

### Adapters (three, covering everything)
1. **`openai_compatible`** — Chat Completions format with a configurable base URL, key and extra headers. Covers OpenAI, Ollama, llama.cpp server, LM Studio, vLLM, OpenRouter, Groq, Mistral, DeepSeek, Together, Fireworks, xAI, and Gemini's OpenAI-compatible endpoint. Azure OpenAI (deployment URL and `api-key` header) is a config variant of this adapter.
2. **`anthropic`** — native Messages API.
3. **`gemini`** — native `generateContent` API.

Providers that need request signing (AWS Bedrock, Vertex AI with service accounts) are out of scope; document reaching them through OpenRouter or a LiteLLM proxy.

### Trait
```rust
#[async_trait]
trait Provider {
    async fn complete_json(&self, req: JsonRequest) -> Result<serde_json::Value, ProviderError>;
    fn capabilities(&self) -> Capabilities; // vision, native_json_schema
}
```
`JsonRequest` holds the instruction, user text, optional images (bytes + media type), and the JSON Schema of the expected reply.

### Structured output
Use each provider's structured-output feature where it exists (check current docs): OpenAI `response_format` with a JSON schema; Anthropic via a forced tool call or its structured-output option; Gemini `responseSchema`; Ollama `format`; llama.cpp server `json_schema`. Otherwise instruct "reply with only JSON" and extract the first JSON object. Validate with serde. On failure, retry once with the validation error included, then fail with a clear error.

### Hardening
- **Timeouts:** connect 10 s; total 60 s for text and 120 s for photos.
- **Request settings:** max output tokens 1024; temperature 0.
- **Retries:** on 429 and 5xx, retry up to 2 times with exponential backoff and jitter, honoring `Retry-After`. Never retry a 4xx auth error.
- **Daily cap:** a per-user daily call cap for hosted providers (default 200, configurable), tracked in `ai_usage`. Local providers have no cap.
- **Images:** the client downscales to about 1600 px; the server rejects anything over 5 MB.
- **Secret redaction:** keys live in a wrapper type whose `Debug` and `Display` print `***`. Provider error bodies are scrubbed of anything that looks like a key before they are logged or returned. Test this.

### Errors the UI must handle (designed in M3, not deferred to polish)
- No provider configured: link to Settings; manual entry still works.
- The provider timed out or is unreachable.
- The reply was unparseable after the retry.
- Rate limited, or the daily cap was reached.
- Offline: estimates need the network.

### Configuration
- Providers are added in Settings (admin only), each with a **Test connection** button that sends a tiny request and reports the result plainly.
- Separate default providers for text and for photos.
- **Env seeding:** `CUPFUL_PROVIDER_*` variables seed providers only when the `ai_providers` table is empty. After that the database wins. If the env values differ from the database, log a notice that the env is ignored. Document this in `.env.example`.
- The app must run with zero hosted providers. The default `.env.example` points at a local Ollama.
- **Ollama from inside Docker** (document in `docs/providers.md`):
  - Ollama must listen on more than localhost (`OLLAMA_HOST=0.0.0.0`).
  - Reach it from Cupful at `http://host.docker.internal:11434/v1`. On Linux, add `extra_hosts: ["host.docker.internal:host-gateway"]` to the `tailscale` service, because Cupful shares that service's network.
  - If Ollama runs on another machine, use that machine's tailnet name or IP instead.
- **Tests:** one `wiremock` suite per adapter covering success, malformed JSON with repair retry, auth error, 429 with backoff, timeout, and key redaction. No real network calls in CI.

---

## 10. Auth and access

- **First run:** if no users exist, every route except setup redirects to a setup page that creates the admin account. Admin creation runs in a transaction that checks no users exist, so two simultaneous first-run requests cannot both succeed.
- **Login:** username and password, argon2id hash. Rate limited at 5 attempts per minute per username, always, and also per client IP where a client IP is known.
- **Client IP:** with the Tailscale sidecar, every request arrives from 127.0.0.1.
  - Trust `X-Forwarded-For` only when `CUPFUL_TRUSTED_PROXY` is set and the request comes from that address.
  - Even then, use the **rightmost** entry, the one the trusted proxy appended. Never use the first entry: the client controls it.
  - Test: no proxy configured, trusted proxy, untrusted source, and a spoofed leading entry.
- **Sessions:** `tower-sessions` with a cookie that is `HttpOnly`, `Secure` (configurable off for local HTTP development) and `SameSite=Lax`, on a 30-day rolling expiry. Issue a new session ID at login. A background task deletes expired session rows daily.
- **CSRF:** mutating requests require the header `X-Cupful: 1` on top of SameSite.
- **Users:** the admin can invite more users with a one-time setup link. No email.
- **Password recovery:** `cupful reset-password <username>` CLI, run inside the container. Document it.
- **Tailscale identity mode** (`CUPFUL_TRUST_TAILSCALE=true`, off by default):
  - When on, trust the `Tailscale-User-Login` header (check current header names in Tailscale's docs) and map it to a user, either auto-created or approved by the admin.
  - The server **refuses to start** in this mode unless it is bound to a loopback address, as it is behind the sidecar.
  - When off, the server **strips** any inbound `Tailscale-User-*` headers before handlers see them.
  - Test spoofed headers in both modes.
- **Ingest tokens:** created in Settings, shown once, stored hashed. They are scoped only to `POST /api/ingest/activity` for their user.
- **No HTTP Basic Auth;** it behaves badly with installed PWAs and service workers.
- **Isolation tests:** a test harness creates two users and, for every user-owned route, proves that user A cannot read, update or delete user B's data. A test that lists the router's routes fails when a user-owned route has no isolation test.

---

## 11. Activity ingest (iOS and automations)

`POST /api/ingest/activity`, with an ingest token sent as a Bearer token:
```json
{"date":"2026-10-27","steps":11234,"gym_minutes":55}
```
- Writes through the same day-patch logic with source `shortcut`, so it never overwrites manual values.
- **Limits:**
  - 60 requests per hour per token.
  - Body at most 4 KB.
  - Dates accepted only from 14 days before the user's today to one day after it, in their timezone.
  - Steps between 0 and 150,000; gym minutes between 0 and 600.
- **Tests:** a valid token cannot write another user's day (the user comes from the token, never from the body); a revoked token is rejected; each limit is enforced.
- `docs/shortcuts.md` gives a step-by-step iOS Shortcuts automation: each evening, find today's step count in Health and post it. It also gives an equivalent for Android automation apps.

---

## 12. Frontend

### Screens
1. **Today:**
   - Budget card: eaten against budget, amount left or over, and any flags (provisional, deficit capped, floor applied).
   - Macro bars for protein, carbs and fat against their targets, plus the "how it's worked out" breakdown.
   - Meal composer: meal type, description, optional photo, Estimate, and **Scan barcode**.
   - Product sheet (after a scan or from the saved list): name, brand, per-serving and per-100 g values, a servings stepper (0.5 steps) with a grams field, Add. "Enter from the label" when not found. Open Food Facts credit.
   - Saved products: searchable list for one-tap re-logging, with remove.
   - Estimate review: source labels, ambiguity choices, editable grams, portion multiplier.
   - Logged meals grouped by meal type, plus "Eat this again".
   - Every AI error state from section 9.
2. **Activity:** steps, gym minutes, body parts trained (Chest, Back, Legs, Shoulders, Arms, Core, Full body, Cardio), and weigh-in, each showing its source. This is a section of Today on wide screens and its own tab on phones.
3. **Trends:**
   - Intake against budget, as bars with a budget tick per day.
   - Weight as points plus a 7-day rolling average.
   - Steps, and average intake against average budget.
   - Range switcher: 7, 30, 90 or 365 days.
4. **Settings** — online only (section 6). Each part arrives with the milestone that builds it:
   - M3: profile and targets (saving creates a new profile version), units, timezone, AI providers.
   - M4: ingest tokens.
   - M5: export, import and backup.
   - M7: users and invites (admin).
5. **Setup and login.**

Day navigation (previous, next, date picker) appears on Today and Activity. Future dates are not loggable.

### Charts: LayerChart only
One library, one theming path, one way of doing accessibility and empty states.
- **Every chart:**
  - readable at 360 px wide;
  - colored from the design tokens, in light and dark themes;
  - shows its value on tap;
  - has a text summary for screen readers;
  - shows a designed empty state when there is no data.
- **Long ranges:** use the range switcher. Add brush zoom on the weight chart only if the current LayerChart version supports it without extra dependencies.
- **Adding a second library:** only if a concrete chart turns out to be impossible in LayerChart. Record that as a decision first.

### PWA and offline (M6)
- **Manifest:** name, maskable icons, theme colors from the tokens, `display: standalone`.
- **Service worker:**
  - Precaches the app shell, with `registerType: 'prompt'`.
  - When a new version is waiting, show an "Update available" banner with a Reload button, and show the app version in Settings.
  - API reads are network-first with a cache fallback.
- **Write queue:** in IndexedDB, in front of `/api/sync` (section 6). Every write goes through it whether or not the app is online.
  - **Indicator:** shows how many changes are waiting.
  - **Detail view:** lists each queued op with its status and last error, with **Retry** and **Discard** per item, so nothing can sit stuck forever.
- **One account's data never shows up in another account.** People share phones, and log out and back in as someone else. Two things can leak meals across accounts: the IndexedDB queue and the service worker's API cache.
  - Every queued op and every cached API response is keyed by user ID.
  - Logging out clears both, after warning if unsynced changes would be lost.
  - The queue refuses to flush ops belonging to a different user ID than the active session.
  - A 401 during flush pauses the queue, prompts re-login, and keeps the items.
- **Photo input:** `<input type="file" accept="image/*" capture="environment">`, re-encoded and downscaled through a canvas before upload (section 5).

### Design
- Run the **frontend-design** skill at the start of M3, before the first screen. Record the direction in `docs/design.md` and `tokens.css`.
- Mobile first. Touch targets at least 44 px. Respect `prefers-reduced-motion`. Light and dark themes.
- Run **frontend-design-impeccable** at the end of M3, M4 and M6, and once more before release.

### Dev workflow (set up in M0)
- **Frontend:** `npm run dev` runs the Vite dev server on 5173 and proxies `/api` to the Rust server on 8080.
- **Backend:** in debug builds, `rust-embed` reads assets from disk, so frontend changes don't need a Rust rebuild.
- **Docs:** write both commands into `CONTRIBUTING.md`.

---

## 13. Repository hygiene and tooling

- **CI (GitHub Actions):**
  - Rust: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
  - SQLx: `cargo sqlx prepare --check` with `SQLX_OFFLINE=true`. Commit `.sqlx/`. A schema change without refreshed metadata fails CI.
  - Frontend: `svelte-check`, Vitest, and Playwright (headless, against `cupful seed-demo` data).
  - Release: multi-arch Docker build (`linux/amd64` and `linux/arm64`).
- **SQLx check in CI** — `--check` needs a real database, so the step is:
  ```sh
  export DATABASE_URL="sqlite://$RUNNER_TEMP/ci.db"
  sqlx database create
  sqlx migrate run --source server/migrations
  cargo sqlx prepare --check -- --all-targets
  ```
  Build and test jobs then run with `SQLX_OFFLINE=true` against the committed `.sqlx/`.
- **`cupful seed-demo --days 90 --seed 42`:** creates a demo user with realistic meals, activity and weights. It uses a fixed random seed, so Playwright assertions and screenshots stay stable. Used for screenshots, Playwright and checking the charts by eye.
- **`CONTRIBUTING.md`:**
  - The dev workflow.
  - How to run each check.
  - A **review checklist** for anyone without the ponytail plugin:
    - Does this code need to exist?
    - Was existing code or the standard library reused?
    - Are the tests in the TDD style?
    - Are errors handled?
    - Do isolation tests cover any new route?
    - Are secrets kept out of logs?
    - Is the UI accessible and does it work at 360 px?
- **`docs/decisions.md`:** one line per deviation from this plan or notable choice.
- **No telemetry.** The only outbound calls go to AI providers the user configured and, unless `CUPFUL_PRODUCT_LOOKUP=off`, Open Food Facts for barcode lookups.

---

## 14. Milestones

Each milestone ends with:
- all checks green;
- `/ponytail-review` and `/ponytail-audit` run and addressed (or the CONTRIBUTING checklist, if the plugin isn't available);
- `/ponytail-debt` listed;
- `docs/decisions.md` updated;
- `AGENTS.md` (status, layout, commands, conventions) and `CLAUDE.md` updated to match the code;
- a short summary of what was built and what's next.

**M0 — Scaffold**
- Cargo project with the CLI skeleton, SvelteKit app, embedded static serving and a health endpoint.
- SQLx migrations, SQLite pragmas and committed `.sqlx/`.
- CI (including the SQLx check step from section 13), the dev workflow (Vite proxy), the multi-arch Dockerfile, `compose.yaml` with the Tailscale sidecar, and `.env.example`.
- `LICENSE.md`, `CONTRIBUTING.md`, `AGENTS.md` and `CLAUDE.md` in the first commit (section 0), the README stub with its License section, license metadata in `Cargo.toml` and `package.json`, and the DCO sign-off check in CI.
- **FTS5 check:** one migration that creates a small FTS5 table, run in CI and in the musl release build. This proves the SQLite build SQLx bundles has FTS5. If it doesn't, stop and record the fix (a feature flag or a system SQLite) before M2 depends on it.

*Done when:*
- `docker compose up` serves a placeholder page that the Docker host can reach (use a local port mapping for this check);
- the FTS5 migration succeeds on both architectures;
- the tailnet HTTPS path is verified once by hand and recorded in `docs/decisions.md`.

**M1 — Accounts, profile, budget engine, write contract**
- **Accounts:** setup (transactional), login, sessions with rotation and cleanup, rate limiting with the trusted-proxy rule, the CSRF header, the `reset-password` CLI, and the isolation-test harness.
- **Profile:** profiles and `profile_versions`, plus timezone.
- **Budget engine:** `budget.rs` with the shared fixtures.
- **Write contract:** the `/api/sync` endpoint with every op kind and rule from section 6, the REST endpoints for profile and settings, the read endpoints, and the daily cleanup task.

*Done when:*
- the budget fixtures pass;
- the sync tests cover:
  - idempotent replay;
  - item replacement;
  - the stored multiplier never being reapplied;
  - per-field last-write-wins;
  - future and too-old timestamps;
  - tombstones;
  - manual beating automated sources;
  - future-date rejection;
- the isolation tests pass for every route.

**M2 — AI providers and nutrition pipeline (API only)**
- **Providers:** the three adapters with hardening, the provider settings API with Test connection, encrypted secrets, env seeding precedence and the daily cap.
- **Food data:** `staples.csv`, the USDA importer, `aliases.csv` and FTS5.
- **Pipeline:** validation, measures, matching with thresholds and ambiguity, and the oil rule with `is_prepared`.
- **Endpoint:** `POST /api/estimate`.
- **Packaged products:** barcode lookup with Open Food Facts and caching, label entry, saved products list (section 8).
- **Testing:** the golden tests and `cupful eval`.

*Done when:*
- the golden suite passes on recorded outputs;
- every adapter's mocked suite passes;
- `curl` against a local Ollama turns "small cup of rice, 1 chicken breast shallow fried" into staple-matched rice, chicken and cooking oil;
- `curl /api/products/<a real barcode>` returns per-100 g and serving values, and a second call makes no outbound request.

**M3 — Today screen**
- Run frontend-design first.
- **Meal flow:** composer, photo, estimate review with source labels and ambiguity choices, and every AI error state.
- **Logging:** save, edit and delete through `/api/sync`, logged meals, repeat meals and manual entry.
- **Barcode:** camera scanning with the browser's `BarcodeDetector` where it exists, else a WebAssembly reader (e.g. `zxing-wasm`) loaded only when Scan is tapped; a typed-barcode field as the last fallback. Product sheet, label entry and saved products.
- **Budget:** the budget and macro cards with flags.
- **Settings:** profile and providers.

*Done when:* on a phone over the tailnet, a meal goes from description to saved log with correct totals, and each AI error state can be triggered and reads clearly; a packaged product is scanned on both an Android and an iPhone, logged at 1.5 servings with correct totals, and re-logged from the saved list. Then run frontend-design-impeccable.

**M4 — Activity and Trends**
- Activity section with field sources, the ingest endpoint and tokens, and `docs/shortcuts.md`.
- Trends with LayerChart, and `cupful seed-demo`.
- Google Health (section 15): optional daily steps sync, source `google`, `docs/google-health.md`.

*Done when:* 90 days of demo data render correctly at 360 px and on desktop, in both themes, and a test Shortcut posts steps that appear without overwriting a manual value. Then run frontend-design-impeccable.

**M5 — Import, export and backups**
- CSV export.
- Fuel Ledger CSV import. The format is `date,meal_id,item_index,meal,description,food,portion,kcal,protein_g,carbs_g,fat_g,steps,gym_minutes,trained,weight_kg`, one row per meal item, and days with no meals have empty meal columns.
  - **Grouping:** rows with the same `meal_id` form one meal.
  - **Ids:**
    - The Cupful meal id is UUIDv5(user, `meal_id`); each item id is UUIDv5(meal id, `item_index`).
    - Two identical meals keep their separate ids, so neither is dropped.
    - Re-importing the same file changes nothing.
  - **Older exports without `meal_id`:**
    - Group consecutive rows that share date, meal type and description into one meal.
    - Derive its id from (user, a hash of all its rows, the occurrence number of that hash in the file), so identical meals stay separate.
    - Document that such files can't tell one meal with two identical items from two identical meals.
  - **Re-import matches by id.** If a description was edited in the source and re-exported under the same `meal_id`, the import updates that meal. With no `meal_id`, the edited row creates a new meal. State this in the import docs.
  - Activity imports with source `import`.
- JSON backup and restore.
- Backup docs (`VACUUM INTO` for a nightly copy).

*Done when:*
- importing the same file twice leaves the data unchanged;
- a file containing two identical meals imports both;
- a backup restores into a fresh instance.

**M6 — PWA and offline**
- Manifest, service worker with the update prompt, IndexedDB queue in front of `/api/sync`, sync detail view with retry and discard, and offline states.

*Done when:*
- on an Android phone, the app installs;
- a repeat meal and a weigh-in logged in airplane mode both sync on reconnect;
- a deliberately broken op can be seen and discarded;
- a write queued as user A, followed by logging out and in as user B, flushes nothing and shows none of A's cached meals.

Then run frontend-design-impeccable.

**M7 — Release v0.1.0**
- Invites, Tailscale identity mode with spoofing tests, and `docs/self-hosting.md` written for someone new to Tailscale (including Ollama setup and iOS Shortcuts).
- CONTRIBUTING, screenshots, a final frontend-design-impeccable pass, the `v0.1.0` tag and the published container image.

---

## 15. Google Health integration (built in M4)

Moved up from v0.2 by the owner. It stays optional: it needs a Google Cloud project for every self-hoster, and while the app is unverified Google expires refresh tokens after 7 days (`docs/google-health.md`). The feature is invisible (routes answer 404) unless `CUPFUL_GOOGLE_CLIENT_ID` and `CUPFUL_GOOGLE_CLIENT_SECRET` are set.

As built (`server/src/google.rs`):
- Use only the **Google Health API v4** (`https://health.googleapis.com/v4`) with Google OAuth 2.0. The legacy Fitbit Web API is shut down.
- Scope `googlehealth.activity_and_fitness.readonly`; steps from `dataTypes/steps/dataPoints:dailyRollUp` for the last 14 days in the user's timezone. Gym minutes are not synced (no documented data type maps cleanly onto "gym minutes"); they stay manual or Shortcut.
- Each self-hoster creates their own Google Cloud project and OAuth client. Never ship a shared client.
- The redirect URI is the instance's own `https://<name>.<tailnet>.ts.net/...`. It doesn't need to be publicly reachable.
- Use the authorization-code flow with PKCE and encrypted tokens.
- Sync daily and offer a "Sync now" button. Write steps and gym minutes with source `google`, following the manual-beats-automated rule.
- Show "Reconnect Google Health" clearly when the token expires.

---

## 16. Deployment

- **Dockerfile:**
  1. Build the SvelteKit app with Node.
  2. Build the Rust binary for the musl target in release mode, stripped, with the frontend embedded. Bundled SQLite is C code, so this stage needs a C cross toolchain: build inside a musl builder image or use `cargo-zigbuild`. Record the choice in the Dockerfile comments.
  3. Final image: distroless or scratch, containing only the binary and CA certificates. Target under 40 MB.
- **Architectures:** publish `linux/amd64` and `linux/arm64` with `docker buildx`. Self-hosters run NAS boxes and Raspberry Pis.
- **Storage:** the `/data` volume must be on local disk. SQLite's WAL mode can corrupt the database on NFS or SMB shares. Say so in bold in `docs/self-hosting.md`, and log a warning at startup if `/data` is on a network filesystem.
- **compose.yaml:**
  - Two services, `cupful` and `tailscale` (the official image). Cupful uses `network_mode: service:tailscale` and binds to 127.0.0.1.
  - The sidecar serves HTTPS on 443 to `http://127.0.0.1:8080`. Check the current Tailscale container docs for the `TS_SERVE_CONFIG` format.
  - Volumes for `/data` and the Tailscale state.
  - `extra_hosts` for Ollama on Linux (section 9).
  - Self-hosters set `TS_AUTHKEY` and a hostname. `CUPFUL_SECRET_KEY` is optional; if set, it always wins over `/data/secret.key` (section 5).
- **Without Tailscale:** the app also runs with `docker run -p 8080:8080` behind any reverse proxy. Document both setups, and `CUPFUL_TRUSTED_PROXY` for the proxy case.
- **Backups:** the database is a single SQLite file, plus `secret.key` if it was generated. Document a nightly `VACUUM INTO` copy, and the JSON export in Settings.

---

## 17. Open decisions for the project owner

1. **Staples review.** The owner reviews `data/staples.csv` values and sources before v0.1.0.
2. **Defaults** worth revisiting after real use: the 25 % deficit cap, the 1500/1200 floors, the protein multiplier, and the daily AI call cap.
