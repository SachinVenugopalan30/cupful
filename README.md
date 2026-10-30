<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Cupful

Cupful is a self-hosted tracker for food, exercise and steps. You describe a meal in plain words. Cupful splits it into foods and portions, then estimates calories and macros with an AI provider you choose: a local model, OpenAI, Anthropic, Gemini or any OpenAI-compatible server.

It runs on your own server. It installs on a phone as a web app. It works offline for the screens you use most. It sends no telemetry.

Version 0.1.0.

## Features

- Describe meals in words, with optional photos. Review and edit before saving.
- Barcode scan, label entry and saved products.
- Daily calorie and macro budget from your profile.
- Trends for intake, weight and steps.
- Activity log for steps, gym minutes, body parts and weigh-ins.
- Offline reads and a write queue that syncs when the connection returns.
- Several people per instance. An admin creates one-time invite links.
- Optional Google Health steps sync.
- A token API for iOS Shortcuts and Android automation apps.
- CSV import and export for Fuel Ledger. JSON backup per account.
- One binary with SQLite and an embedded web app.

## Install with Tailscale

This is the shortest path. Cupful runs behind a Tailscale sidecar and gets HTTPS on your tailnet.

```sh
git clone https://github.com/SachinVenugopalan30/cupful.git
cd cupful
cp .env.example .env
# Edit .env and set TS_AUTHKEY.
docker compose up -d
```

Open `https://cupful.<your-tailnet>.ts.net` and create the admin account.

Full setup, including the Tailscale auth key and first-run steps: [docs/self-hosting.md](docs/self-hosting.md).

## Install with plain Docker

No Tailscale. Put your own HTTPS reverse proxy in front.

```sh
docker run -d --name cupful \
  -p 127.0.0.1:8080:8080 \
  -v cupful-data:/data \
  --env-file .env \
  ghcr.io/sachinvenugopalan30/cupful:0.1.0
```

Read [docs/self-hosting.md](docs/self-hosting.md) before you expose it. Secure cookies, the trusted proxy setting and the public URL all matter behind a proxy.

## Documentation

| Guide | What it covers |
| --- | --- |
| [Self-hosting](docs/self-hosting.md) | Install, Tailscale, reverse proxy, users, upgrades, environment variables, logs |
| [AI providers](docs/providers.md) | Add a provider, adapter kinds, Ollama in Docker, daily cap, key storage |
| [Offline and install](docs/offline.md) | What works offline, the write queue, PWA install on Android and iOS |
| [Import and export](docs/import-export.md) | CSV format, Fuel Ledger import, round trips and limits |
| [Backups](docs/backups.md) | JSON backup and restore, nightly SQLite copy, secret key |
| [Shortcuts and ingest](docs/shortcuts.md) | Tokens, the activity endpoint, iOS Shortcuts, Android apps |
| [Google Health](docs/google-health.md) | Optional steps sync and its Google Cloud setup |
| [Design](docs/design.md) | Visual direction, theme, palette, tokens and component rules |
| [Decisions](docs/decisions.md) | Engineering log of deviations and notable choices |

## Steps on iOS

A web app cannot read Apple Health. Enter steps by hand, or build an iOS Shortcut that posts the day's count to Cupful. See [docs/shortcuts.md](docs/shortcuts.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Every commit needs a DCO sign-off (`git commit -s`).

## License

Cupful is licensed under the GNU Affero General Public License, version 3 or later (AGPL-3.0-or-later). See [LICENSE.md](LICENSE.md).

In plain terms: you may self-host Cupful, change it and use your changes privately. If you offer a modified version to other people over a network, you must make your modified source available to them. Settings, the login screen and the setup screen link to the source of the running version. Set `CUPFUL_SOURCE_URL` if you run a modified version.
