# Decisions

One line per deviation from `PLAN.md` or notable choice: date — what — why.

- 2026-10-27 — Name `cupful` checked: free on crates.io, no GitHub user/org `cupful`, no Docker Hub repos under `cupful`. Image will publish to `ghcr.io/<owner>/cupful`. — Plan §0.
- 2026-10-27 — Rust toolchain pinned to 1.99.1 in `rust-toolchain.toml`. — sqlx 0.9 needs rustc ≥ 1.94.
- 2026-10-27 — SvelteKit 3 keeps its config inside `vite.config.ts` (no `svelte.config.js`); `$lib` is the `#lib/*` package import. — Current scaffold from `sv create`.
- 2026-10-27 — Vitest and Playwright are added with the first frontend logic and flow (M3), not as empty suites in M0. — Nothing to test yet; an empty runner proves nothing.
- 2026-10-27 — Docker builds cross-compile on the build host with `cargo-zigbuild` (zig as the C toolchain for bundled SQLite); final image `distroless/static-debian13:nonroot` (~13 MB). — No QEMU for compiling, no per-arch builder images.
- 2026-10-27 — Image defaults to `CUPFUL_BIND=0.0.0.0:8080` (works with `docker run -p`); `compose.yaml` overrides to `127.0.0.1:8080` inside the sidecar's network. Because of that loopback bind, the M0 compose check curls Cupful from inside the sidecar's network namespace; host reachability is checked with `docker run -p`. — A host port mapping can't reach a loopback-bound process.
- 2026-10-27 — Tailnet HTTPS path (`https://cupful.<tailnet>.ts.net`) is **not yet verified by hand**; needs the owner's `TS_AUTHKEY`. Record the result here when done. — M0 done-criterion still open.
