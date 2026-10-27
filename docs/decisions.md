# Decisions

One line per deviation from `PLAN.md` or notable choice: date — what — why.

- 2026-10-27 — Name `cupful` checked: free on crates.io, no GitHub user/org `cupful`, no Docker Hub repos under `cupful`. Image will publish to `ghcr.io/<owner>/cupful`. — Plan §0.
- 2026-10-27 — Rust toolchain pinned to 1.99.1 in `rust-toolchain.toml`. — sqlx 0.9 needs rustc ≥ 1.94.
- 2026-10-27 — SvelteKit 3 keeps its config inside `vite.config.ts` (no `svelte.config.js`); `$lib` is the `#lib/*` package import. — Current scaffold from `sv create`.
- 2026-10-27 — Vitest and Playwright are added with the first frontend logic and flow (M3), not as empty suites in M0. — Nothing to test yet; an empty runner proves nothing.
- 2026-10-27 — Docker builds cross-compile on the build host with `cargo-zigbuild` (zig as the C toolchain for bundled SQLite); final image `distroless/static-debian13:nonroot` (~13 MB). — No QEMU for compiling, no per-arch builder images.
- 2026-10-27 — Image defaults to `CUPFUL_BIND=0.0.0.0:8080` (works with `docker run -p`); `compose.yaml` overrides to `127.0.0.1:8080` inside the sidecar's network. Because of that loopback bind, the M0 compose check curls Cupful from inside the sidecar's network namespace; host reachability is checked with `docker run -p`. — A host port mapping can't reach a loopback-bound process.
- 2026-10-27 — Tailnet HTTPS path verified by hand by the owner: `docker compose up` with a real `TS_AUTHKEY`, `https://cupful.<tailnet>.ts.net` serves the placeholder page with version and source link. — M0 done-criterion.
- 2026-10-27 — Sessions use `tower-sessions` with our own ~80-line SQLite store (`auth::SqliteStore`) instead of `tower-sessions-sqlx-store`. — That crate (0.15) still depends on sqlx 0.8 and tower-sessions 0.14; two sqlx versions can't link two bundled SQLites.
- 2026-10-27 — Times are stored as INTEGER unix milliseconds, not text. — Text timestamps with variable fractional digits don't compare correctly; integers do.
- 2026-10-27 — `GET /api/trends` moves from M1 to M4. — Its shape is set by the charts it feeds; days/meals/budget reads cover M1.
- 2026-10-27 — `meal_items.food_id` and `meals.provider_id` have no foreign keys yet. — `foods` and `ai_providers` arrive in M2; SQLite can't add a foreign key without rebuilding the table, so M2 decides whether that rebuild is worth it.
- 2026-10-27 — "Setup required" is reported by `GET /api/auth/state` (`needs_setup`); the SPA does the redirect (M3). Other API routes answer 401 until someone logs in. — The redirect is a UI concern; the API stays uniform.
- 2026-10-27 — Loopback peers count as "client IP unknown", so behind the Tailscale sidecar only the per-username login limit applies. — Otherwise every household member would share one 5/min IP bucket.
- 2026-10-27 — Clearing `steps` or `gym_minutes` (`null` in `day.patch`) also clears its source, so an automation may fill the field again. — A cleared field holds no manual value to protect.
- 2026-10-27 — `weight_kg` and `trained` have no source column (as in §5) and resolve by time only. — Only manual sync writes them in M1; revisit if M5's import should not overwrite manual weights.
- 2026-10-27 — `meal_items.position` added to keep item order stable. — Not in §5; needed for a predictable review screen.
