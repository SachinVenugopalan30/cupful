<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Self-hosting Cupful

Cupful is one binary with an embedded web app and a SQLite database. You can run it behind a Tailscale sidecar, or behind your own HTTPS reverse proxy.

This page covers both paths, users, upgrades, backups, the CLI and every environment variable.

## What you need

- A Linux server, a home machine or a NAS with Docker and the Compose plugin.
- About 100 MB of disk for the image, plus room for the database.
- A local disk for the data volume. Do not use NFS, SMB or another network filesystem. SQLite's write-ahead log can corrupt there.
- A Tailscale tailnet with HTTPS certificates enabled, or a domain name, a TLS certificate and a reverse proxy you already run.

You do not need a public IP address for the Tailscale path.

## Path 1: Tailscale sidecar

The repository ships `compose.yaml` with two services. The `tailscale` sidecar joins your tailnet and serves HTTPS on port 443. The `cupful` container shares the sidecar's network namespace, so Cupful listens only on loopback and is reachable only through Tailscale Serve.

This is the recommended path. It gives you a private HTTPS address with no certificate work.

### 1. Enable HTTPS certificates

In the Tailscale admin console, open **DNS** and enable **HTTPS Certificates**. Tailscale cannot issue the certificate for your node until this is on.

### 2. Create an auth key

Open <https://login.tailscale.com/admin/settings/keys> and create an auth key. Copy it now. It is shown once and starts with `tskey-auth-`.

A one-off key is enough for a single server. If you expect to recreate the container, use a reusable key, or create a new key each time.

### 3. Configure and start

```sh
git clone https://github.com/SachinVenugopalan30/cupful.git
cd cupful
cp .env.example .env
```

Edit `.env`:

```sh
TS_AUTHKEY=tskey-auth-...
TS_HOSTNAME=cupful
```

`TS_HOSTNAME` becomes the first label of the address. The default `cupful` gives `https://cupful.<your-tailnet>.ts.net`.

Start the services:

```sh
docker compose up -d
```

The first start builds the image, so it takes a few minutes.

### 4. Create the admin account

Open `https://<TS_HOSTNAME>.<your-tailnet>.ts.net`. Cupful sends you to `/setup` while no account exists. Choose a username, a password and your timezone.

Complete setup before you share the address. The first visitor can claim the admin account.

### 5. Check the sidecar

```sh
docker compose ps
docker compose logs -f tailscale
```

The Serve configuration is already in `tailscale/serve.json`. It serves HTTPS on 443 and proxies `/` to `http://127.0.0.1:8080`. You do not need to edit it. The mount is a directory, not a file, so Tailscale notices edits.

Only devices in your tailnet can reach the address. To let someone outside your tailnet use this instance, share the node with them in Tailscale, or add them to the tailnet.

## Path 2: Plain Docker behind a reverse proxy

Use this path when you already have a domain and a proxy that terminates TLS.

Run the published image:

```sh
docker run -d --name cupful \
  -p 127.0.0.1:8080:8080 \
  -v cupful-data:/data \
  --env-file .env \
  --restart unless-stopped \
  ghcr.io/sachinvenugopalan30/cupful:0.1.0
```

The container listens on `0.0.0.0:8080` and keeps its database in the `cupful-data` volume at `/data`.

Publishing the port on `127.0.0.1` keeps Cupful off the network. Your proxy connects from the same host.

### Reverse proxy

Any proxy that terminates TLS works. The proxy must:

- Forward `Host` unchanged.
- Forward `X-Forwarded-For`.
- Allow request bodies up to 10 MB, for CSV import and JSON restore.

A Caddy example for a proxy running on the same host:

```caddyfile
cupful.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Caddy sets `X-Forwarded-For` and obtains a certificate automatically.

Set these values in `.env`:

```sh
CUPFUL_TRUSTED_PROXY=172.17.0.1
CUPFUL_PUBLIC_URL=https://cupful.example.com
```

- `CUPFUL_TRUSTED_PROXY` is the address Cupful sees for your proxy. Only this address may set the client IP through `X-Forwarded-For`. Without it, every request looks like it comes from the proxy, and the login rate limiter cannot tell clients apart.
- `CUPFUL_PUBLIC_URL` is the public HTTPS address, with no trailing slash. Google Health uses it to build the OAuth redirect URI.

The address is inside the container, not on the host. With a host proxy and the port published on `127.0.0.1`, the container usually sees the Docker bridge gateway, `172.17.0.1`. Check it with:

```sh
docker network inspect bridge --format '{{(index .IPAM.Config 0).Gateway}}'
```

If the proxy runs in Docker on a user-defined network shared with Cupful, use the proxy container's address instead. Docker Desktop may use a different host address. Set `CUPFUL_TRUSTED_PROXY` only to the one address your proxy uses.

Secure cookies are on by default. The browser sees HTTPS from the proxy, so the `Secure` flag is correct. Do not set `CUPFUL_SECURE_COOKIES=false` on a public instance.

### Create the admin account

Open `https://cupful.example.com` and complete `/setup`.

## Users and invites

Invites work on both install paths.

1. Sign in as an admin.
2. Open **Settings → Users**.
3. Press **Create invite**.
4. Copy the link. It is shown once.
5. Send it to the person. They open it, choose a username, a password and a timezone.

Invites are one-time links. They expire after 7 days. Each link works once. An admin can revoke a pending invite, which deletes it immediately.

The link uses the origin the admin is browsing. It works wherever the instance is reachable over HTTPS. It has no meaning outside this instance.

Anyone with the link can create an account. Send it over a private channel.

## Tailscale identity mode

Identity mode logs people in from the `Tailscale-User-Login` header that Tailscale Serve adds. It removes the invite and password step for tailnet members.

Identity mode is only for the sidecar path. Leave it off on the plain Docker path.

To turn it on:

```sh
CUPFUL_TRUST_TAILSCALE=true
```

The server must bind to loopback. The provided `compose.yaml` sets `CUPFUL_BIND=127.0.0.1:8080`. With any other bind, Cupful refuses to start and prints the reason. This prevents a directly exposed port from trusting a header anyone can send.

What happens in identity mode:

- An anonymous request to `/api/` with the header maps to the user with that `tailscale_login`.
- If no user has that login, Cupful creates a non-admin user. The username comes from the email local part. A collision gets a numeric suffix, then a random suffix.
- An existing session is never replaced by the header. If you signed in with a password, that session stays until you log out.
- Only `/api/` requests are mapped. Static files and `/api/health` never create users or sessions.
- Mapping starts only after an admin account exists, so `/setup` cannot lock itself out.

Password login still works. It is the break-glass path when Tailscale is down or a header is missing.

### Bind the admin to a tailnet login

The admin account created at setup has no `tailscale_login`. After its cookie expires, Tailscale Serve sends a header for a different login, and the admin would land as a non-admin user.

Map the admin account once:

```sh
docker compose run --rm cupful bind-tailscale alice alice@example.com
```

The command sets `tailscale_login` for the existing user `alice`. The login is stored lowercased. It fails if the login already belongs to another user.

For the plain image, use the same command through `docker run`:

```sh
docker run --rm -v cupful-data:/data ghcr.io/sachinvenugopalan30/cupful:0.1.0 \
  bind-tailscale alice alice@example.com
```

### Invites in identity mode

Identity mode and invites are alternatives. While identity mode is on:

- Settings hides the invite controls.
- `POST /api/invites` and the accept endpoint answer 409.

Turn identity mode off and restart to use invites again.

### Caveats

- Any container that shares the sidecar's network namespace can send a forged `Tailscale-User-Login` header. Tailscale Serve strips inbound copies of the header before it proxies. Do not join other containers to this network namespace.
- Two people whose email local parts match get suffixed usernames. The account is still tied to the full login.
- Identity mode does not set passwords. A user created by the header has no password until an admin runs `reset-password`.

## Environment variables

Set these in `.env` for Compose, or pass them with `--env-file` or `-e` for `docker run`.

| Variable | Default | Purpose |
| --- | --- | --- |
| `TS_AUTHKEY` | none | Tailscale auth key. Read by the sidecar, not by Cupful. |
| `TS_HOSTNAME` | `cupful` | Tailscale machine name and first address label. |
| `CUPFUL_TRUST_TAILSCALE` | off | Map `Tailscale-User-Login` to a session. Requires a loopback bind. |
| `CUPFUL_BIND` | `127.0.0.1:8080` | Address and port the server listens on. The image sets `0.0.0.0:8080`; Compose sets `127.0.0.1:8080`. |
| `CUPFUL_DATA_DIR` | `.cupful` | Directory for `cupful.db` and `secret.key`. The image sets `/data`. |
| `CUPFUL_SECURE_COOKIES` | on | `Secure` flag on the session cookie. Set `false` only for plain-HTTP local development. |
| `CUPFUL_TRUSTED_PROXY` | none | Address allowed to set the client IP from `X-Forwarded-For`. |
| `CUPFUL_PUBLIC_URL` | none | Public HTTPS address without a trailing slash. Used for the Google redirect URI. |
| `CUPFUL_SOURCE_URL` | the project repository | Target of the AGPL source link. Set it for a modified version. |
| `CUPFUL_PRODUCT_LOOKUP` | on | `off` disables Open Food Facts lookups. Stored products and label entry still work. |
| `CUPFUL_SECRET_KEY` | generated file | 32 random bytes in base64. Encrypts provider keys and Google tokens. |
| `CUPFUL_PROVIDER_*` | Ollama on the host | Seeds one AI provider while none exists. See below. |
| `CUPFUL_AI_DAILY_CAP` | `200` | Calls per user per local day to hosted providers. Local providers are not capped. |
| `CUPFUL_GOOGLE_CLIENT_ID` | none | Google OAuth client id. Both Google values are needed to show the feature. |
| `CUPFUL_GOOGLE_CLIENT_SECRET` | none | Google OAuth client secret. |
| `RUST_LOG` | `info` | Log level: `error`, `warn`, `info`, `debug` or `trace`. `cupful=debug` logs model replies. |

The provider variables are `CUPFUL_PROVIDER_KIND`, `_LABEL`, `_BASE_URL`, `_MODEL`, `_LOCAL`, `_VISION`, `_API_KEY`, `_EXTRA_HEADERS` and `_STRUCTURED_OUTPUT`. They create one provider on first start, only while the database has none. After that, change providers in Settings. See [providers.md](providers.md).

## Data, backups and the secret key

The database is `/data/cupful.db` in the `cupful-data` volume. The key file is `/data/secret.key` unless you set `CUPFUL_SECRET_KEY`.

Keep the volume on local disk. On Linux, Cupful checks `/proc/mounts` at startup and logs a warning when the data directory is under an NFS, CIFS or SMB mount. Other platforms get no check, so verify the mount yourself.

Cupful creates `secret.key` on first start and logs its path. Back it up with the database. It encrypts AI provider keys and Google tokens; it does not protect meals, days, foods or password hashes.

Run one container per data directory. Two writers on one SQLite file will conflict.

Backup and restore steps, including the nightly SQLite copy and file ownership: [backups.md](backups.md).

## Upgrades

Back up first. Every release runs its database migrations at startup.

Compose path, from your checkout:

```sh
git pull
docker compose up -d --build
```

Image path:

```sh
docker pull ghcr.io/sachinvenugopalan30/cupful:0.1.0
docker stop cupful
docker rm cupful
# Run the docker run command again with the same -v cupful-data:/data.
```

Pin an exact version. `latest` may move to a build that has not been tested on your host. Do not start an older image against a database that a newer version has migrated. Restore a backup instead.

The app shell updates itself. When a new build is available, an **Update available** banner appears; **Reload** switches to it.

## Reset a password

Stop nothing; the command opens the same database.

Compose:

```sh
docker compose run --rm cupful reset-password alice
```

Plain Docker:

```sh
docker run --rm -it -v cupful-data:/data \
  ghcr.io/sachinvenugopalan30/cupful:0.1.0 reset-password alice
```

Type the new password and press Enter. The command ends every session for that user.

Passwords are the break-glass path in identity mode. A reset does not remove the user's `tailscale_login`.

## Logs

Compose:

```sh
docker compose logs -f cupful
docker compose logs -f tailscale
```

Plain Docker:

```sh
docker logs -f cupful
```

Set `RUST_LOG=cupful=debug` to see raw model replies and HTTP details. Leave it at `info` otherwise. Logs contain no API keys; provider errors are scrubbed.

## Install caveats

- **Container image.** The release image is `ghcr.io/sachinvenugopalan30/cupful`. Tags are the version (`0.1.0`) and, for stable releases, `latest`. Both `linux/amd64` and `linux/arm64` are published.
- **Package visibility.** The first published package may be private. If a pull fails with `denied`, either sign in with `docker login ghcr.io` or make the package public in its GitHub package settings.
- **First-run race.** Anyone who reaches an unconfigured instance can create the admin account. Set it up before you open the address to others.
- **No telemetry.** Cupful contacts configured AI providers, Open Food Facts (unless `CUPFUL_PRODUCT_LOOKUP=off`) and Google when connected. It sends nothing else.
- **Network filesystems.** Do not place the data volume on NFS, CIFS or SMB. The startup warning is Linux-only and best effort.
- **Reverse proxy body limit.** Import and restore accept up to 10 MB. Raise your proxy's limit if it is lower.
- **Single admin claim.** There is no password recovery by email. Use `reset-password` if the admin password is lost.
- **Time.** Dates are the user's local date. Set each profile's timezone at setup or in Settings; otherwise "today" is wrong.

## Next steps

- Connect an AI provider: [providers.md](providers.md).
- Install the app and understand offline behavior: [offline.md](offline.md).
- Set up steps from a phone: [shortcuts.md](shortcuts.md) or [google-health.md](google-health.md).
- Move a log in or out: [import-export.md](import-export.md).
