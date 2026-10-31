<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Contributing to Cupful

Cupful is a small self-hosted app for individuals and households. Changes that keep it small and predictable are the most welcome.

## Developer Certificate of Origin

Every commit must carry a `Signed-off-by:` line. Add it with `git commit -s`.

By signing off you certify that you wrote the change, or that you have the right to submit it under the project license. The full text is at [developercertificate.org](https://developercertificate.org/). CI checks every pull request. A commit without sign-off fails the check.

If you forgot, run `git rebase --signoff main` and force-push your branch.

## License

Cupful is licensed under AGPL-3.0-or-later. See [LICENSE.md](LICENSE.md). Contributions use the same license. There is no CLA.

Start every new source file with an SPDX header:

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
```

Use `<!-- ... -->` in Svelte and HTML files, and `#` in shell and YAML files.

## Development setup

Requirements:

- Rust stable, pinned in `rust-toolchain.toml`.
- Node 22 or newer.
- `sqlx-cli`:

```sh
cargo install sqlx-cli --no-default-features --features sqlite,rustls
```

Run the API and the frontend in two terminals:

```sh
# Terminal 1: API on :8080. Debug builds read web/build from disk.
cd server
CUPFUL_SECURE_COOKIES=false cargo run -- serve

# Terminal 2: Vite on :5173, proxying /api to :8080.
cd web
npm install
npm run dev
```

Open <http://localhost:5173>. Data lives in `./.cupful/` relative to the server working directory, unless `CUPFUL_DATA_DIR` says otherwise. Every setting is listed in `.env.example`.

## Git hooks

Install the checked-in hooks once per clone:

```sh
./scripts/install-hooks.sh
```

The pre-commit hook runs `cargo fmt --check` when server files are staged, and `svelte-check` plus Vitest when web files are staged. It stays fast on purpose. Clippy, `cargo test`, the build and Playwright still run in the checks below and in CI.

## CLI commands

Run these with `cargo run -- <command>` from `server/`:

| Command | What it does |
| --- | --- |
| `serve` | Run the server. This is the default. |
| `reset-password <username>` | Set a new password. Reads the password from stdin. |
| `import-foods <file.json>` | Import a USDA FoodData Central Foundation or SR Legacy JSON file. Safe to re-run. |
| `seed-demo [--days N] [--seed N] [--force]` | Create a demo user with generated data and print a one-time password. |
| `eval --provider <id>` | Run the golden estimate cases against a live provider and print accuracy. |

`bind-tailscale <username> <login>` maps an existing user to a tailnet login. See [docs/self-hosting.md](docs/self-hosting.md#tailscale-identity-mode).

## Database changes

Migrations live in `server/migrations/`. Never edit a released migration. After you add one, or change any `query!` macro:

```sh
cd server
mkdir -p ../.cupful
export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
sqlx database create
sqlx migrate run
cargo sqlx prepare -- --all-targets
```

Commit the refreshed `server/.sqlx/` directory. CI fails when it is stale.

## Checks

Run all of these before you push. CI runs the same set. In a fresh checkout or worktree, build the frontend once first because a server test reads `web/build`.

```sh
cd server
cargo fmt --check
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo test
cargo sqlx prepare --check -- --all-targets

cd ../web
npm run check
npm test
npm run build
npm run test:e2e
```

`cargo sqlx prepare --check` needs `DATABASE_URL`; see the database section. Playwright starts the real Rust server and needs Chromium once: `npx playwright install chromium`.

## Test layout

- `server/tests/*.rs`: integration tests on the real router with in-memory SQLite.
- `server/tests/common/`: shared helpers.
- `server/tests/isolation/`: one file per route group. Fails when a user-owned route lacks an isolation test.
- `server/tests/products/` and `server/tests/import/`: barcode and CSV suites with wiremock and fixtures.
- `web/src/**/*.test.ts`: Vitest tests next to the logic.
- `web/e2e/`: Playwright specs against a real server.

## Workflow

- Write the test first for backend logic and for frontend logic that computes something. See it fail, write the least code that passes, then clean up.
- Keep it small. Ask whether the code needs to exist, whether existing code or the standard library already does it, and whether an installed dependency does it.
- Record a deviation. If the design is wrong or an alternative is better, add one line to [docs/decisions.md](docs/decisions.md): date, what changed, why.
- Update the docs. If a change alters commands, layout or conventions, update `AGENTS.md` and the relevant user docs in the same pull request.
- Keep source and test files under 500 lines. Split by behaviour.

## Review checklist

- Does this code need to exist?
- Was existing code or the standard library reused?
- Are tests written test-first, and do they test behavior?
- Are errors handled?
- Do isolation tests cover any new user-owned route?
- Are secrets kept out of logs?
- Is the UI accessible, and does it work at 360 px wide?

## Pull requests

- One concern per pull request. Start large changes with an issue.
- Say what changed, why, and how you tested it.
- All checks green. Every commit signed off.
- No telemetry. Cupful makes outbound calls only to configured AI providers, Open Food Facts and Google when connected. Pull requests that add analytics, crash reporting or update pings will be declined.
