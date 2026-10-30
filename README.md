# Cupful

Self-hosted tracker for food, exercise and steps. Log meals the way you'd describe them to a friend ("a small cup of rice and one shallow-fried chicken breast") and Cupful turns that into calories and macros using whichever AI model you connect: a local model, OpenAI, Anthropic, Gemini or any other provider. It runs on your own server, installs on your phone as a PWA, and is meant to be reached privately over Tailscale.

**Status:** early development, not usable yet. See `PLAN.md`.

## Running it

```sh
cp .env.example .env   # set TS_AUTHKEY
docker compose up -d
```

Then open `https://cupful.<your-tailnet>.ts.net`. Without Tailscale: `docker run -p 8080:8080 -v cupful-data:/data cupful` behind your own HTTPS reverse proxy.

Steps on iOS: a PWA can't read Apple Health. Enter steps by hand, or use an iOS Shortcuts automation that posts the day's step count to Cupful (see `docs/shortcuts.md`).

## Backups, import and export

Your food log can be exported as CSV and imported from Fuel Ledger (see `docs/import-export.md`). A JSON backup saves one account's data and can be restored elsewhere; for the whole instance, copy the SQLite database (see `docs/backups.md`). CSV is an interchange format, not a Cupful backup.

## Contributing

See `CONTRIBUTING.md`. Commits need a DCO sign-off (`git commit -s`).

## License

Cupful is licensed under the GNU Affero General Public License, version 3 or later (AGPL-3.0-or-later); see `LICENSE.md`.

In plain terms: you can self-host Cupful, change it, and use your changes privately however you like. If you offer a modified version to other people over a network, you must make your modified source code available to them. Settings → About links to the source of the running version; set `CUPFUL_SOURCE_URL` to point at your own if you've changed it.
