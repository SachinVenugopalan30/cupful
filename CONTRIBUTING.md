# Contributing to Cupful

Thanks for helping. Cupful is a small self-hosted app for individuals and households; contributions that keep it small and boring are the most welcome.

## Developer Certificate of Origin (DCO)

Every commit must carry a `Signed-off-by:` line. Add it with `git commit -s`.

By signing off you certify that you wrote the change, or otherwise have the right to submit it under the project's license, as described in the [Developer Certificate of Origin](https://developercertificate.org/). CI checks sign-off on every pull request; a commit without it fails the check.

Forgot to sign off? `git rebase --signoff main` and force-push your branch.

## License of contributions

Cupful is licensed under **AGPL-3.0-or-later** (see `LICENSE.md`). Contributions are accepted under the same license. There is no CLA.

New source files start with an SPDX header:

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
```

(`<!-- ... -->` in Svelte/HTML files, `#` in shell/YAML.)

## Dev setup

Requirements: Rust stable (pinned in `rust-toolchain.toml`), Node 22+, and `sqlx-cli`:

```sh
cargo install sqlx-cli --no-default-features --features sqlite,rustls
```

Run the two halves side by side:

```sh
# Terminal 1 — API on :8080. Debug builds read web/build from disk, so no Rust rebuild for frontend changes.
cd server && CUPFUL_SECURE_COOKIES=false cargo run -- serve

# Terminal 2 — Vite dev server on :5173, proxies /api to :8080.
cd web && npm install && npm run dev
```

Open http://localhost:5173. Data lives in `./.cupful/` (relative to where the server runs) unless `CUPFUL_DATA_DIR` says otherwise. See `.env.example` for every setting.

CLI commands (`cargo run -- <command>`):

- `serve` — run the server (default).
- `reset-password <username>` — set a new password; prompts on stdin.

### Database changes

Migrations live in `server/migrations/`. After adding one or changing any `query!` macro:

```sh
cd server
mkdir -p ../.cupful && export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
sqlx database create && sqlx migrate run
cargo sqlx prepare -- --all-targets
```

Commit the refreshed `server/.sqlx/` directory. CI fails if it's stale.

## Checks

Run all of these before pushing; CI runs the same set. In a fresh checkout or worktree, build the frontend once first (`cd web && npm ci && npm run build`): a server test reads `web/build`.

```sh
cd server
cargo fmt --check
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo test
cargo sqlx prepare --check -- --all-targets   # needs DATABASE_URL, see above

cd ../web
npm run check      # svelte-check
npm run build
```

Vitest and Playwright join the list with the first frontend logic (M3).

## Workflow

- **Tests first.** Backend logic and frontend logic that computes something are written test-first: a failing test, the least code to pass it, then cleanup.
- **Keep it small.** Before adding code ask: does it need to exist, does existing code or the standard library already do it, does an installed dependency do it?
- **Deviations.** If the plan (`PLAN.md`) is wrong or worse than an alternative, record the decision as one line in `docs/decisions.md` (date, what changed, why).
- **Docs stay current.** If your change alters commands, layout or conventions, update `AGENTS.md` (and `CONTRIBUTING.md` where relevant) in the same pull request.

### Review checklist

Contributors using Claude Code with the ponytail plugin run `/ponytail-review`. Everyone else, check:

- Does this code need to exist?
- Was existing code or the standard library reused?
- Are the tests written test-first and do they test behavior?
- Are errors handled?
- Do isolation tests cover any new user-owned route?
- Are secrets kept out of logs?
- Is the UI accessible, and does it work at 360 px wide?

## Pull requests

- Small and focused: one concern per PR. Large changes start with an issue.
- The description says what changed, why, and how it was tested.
- All checks green, every commit signed off.
- **No telemetry.** Cupful makes no outbound calls except to AI providers the user configured. PRs that add analytics, crash reporting or update pings will be declined.
