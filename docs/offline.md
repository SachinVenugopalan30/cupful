<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Offline use and installing

Cupful is a static web app served by the Rust binary. After you open it once, it installs as an app, keeps the last reads for each account on the device, and queues logged-data writes. You can log a meal or a weigh-in with no connection.

Configuration and anything that calls an external service stay online.

This page describes what the app does. The implementation is in `web/src/service-worker.ts` and `web/src/lib/offline/`.

## What works offline

These screens read from a per-user IndexedDB cache:

- **Today:** the day, its meals, the day's budget and the saved product list.
- **Activity:** the selected day, including steps, gym minutes, body parts and weigh-in.
- **Trends:** the cached range response.
- **Saved products:** the list, so previously seen products can be logged again.
- **Profile:** the cached profile and auth state are enough to boot the app for the same account.

Logged-data writes always enter the queue first. That covers meal upserts, meal deletes and day patches. The queue sends them to `POST /api/sync` when a connection is available.

These stay online-only, and the UI says so when offline:

- Settings and the profile form, including timezone and units.
- AI providers and **Test connection**.
- Ingest tokens.
- Aliases: listing, remembering and deleting.
- AI estimates, because they call the configured model.
- A barcode that has never been looked up. Previously saved products and cached reads still work.
- Import, restore, export and backup.

A stale budget is shown offline. Cupful does not recompute a budget or a trend on the device. It shows the last value the server sent. Meal totals come from cached meals plus the overlay of still-queued operations, so the food list is current even when the budget card is not.

## The write queue

Every logged-data write goes through IndexedDB in front of `POST /api/sync`, online or offline. `sync.sendOps` remains the only write path. It enqueues the operation and asks the queue to flush.

An operation has one of five statuses:

| Status | Meaning |
| --- | --- |
| `pending` | Queued, not sent yet. |
| `sending` | Part of the batch currently in flight. |
| `stale` | The server says an older write cannot win. Kept so the user can retry or discard it. |
| `rejected` | The server refused the operation and returned a message. Kept so it can be fixed, retried or discarded. |
| `failed` | The request failed in transport or with a retryable server answer. Retried on reconnect, on the next flush, or from **Sync now**. |

A flush sends the oldest queued operations for one user, in chunks of at most 100 operations. That is the server's request limit. The answer has one result per operation.

- `applied` and `duplicate` are written through to the cached read bodies. Then the queue row is deleted.
- `stale` and `rejected` are kept with their error until the user retries or discards them.
- A chunk that fails before an answer stays queued as `failed` and is retried with backoff.

The banner shows how many changes are waiting. **Settings → Sync** lists each queued operation with its status, last error and **Retry** / **Discard** actions. Nothing can sit stuck forever.

## Identity guard

Before each flush, the queue asks `GET /api/auth/state` who the live session belongs to. It compares that user id with the user id on the operations it is about to send.

- Signed out: the queue pauses with an `auth` message ("Sign in again to send N changes"). It keeps the operations and resumes after the same user signs in.
- A different user: the queue pauses with an `identity` message. Operations and cached reads that belong to the other account are never sent or shown.
- The guard request itself fails: the flush does not run. It backs off and tries the guard again later instead of sending blind.

This is stronger than keying rows by user id. User-keyed rows keep one account's data from being read as another's, but they do not prove the cookie still belongs to that user. Checking `/api/auth/state` before every batch stops a stale tab, an expired session or a login change in another tab from pushing the wrong account's writes.

## Failure behavior

- A 401 during a flush pauses the queue without a redirect. The items stay, the banner asks for a sign-in, and the current page is kept. This differs from the normal API 401 handling, which reloads `/login`, because a queued write must survive.
- A 5xx, 429 or network failure retries with capped exponential backoff. The first retry is a few seconds out, doubles, and stops growing at a small ceiling. Reconnecting or pressing **Sync now** resets the delay.
- A whole batch rejected with another 4xx is resent one operation at a time. Only the change the server refuses is marked failed. The rest of the batch lands.
- If IndexedDB is unavailable, the quota is exceeded or an enqueue fails, Cupful falls back to a direct `POST /api/sync` when online. Offline there is nowhere to persist the write, so the screen reports that the change was not saved.
- Token refresh, provider errors and other online-only actions fail normally with their own messages.

## The read cache

Reads are network-first. A 2xx JSON answer is stored for the current user and returned. On a network error or a 5xx, the cached body for the same user and path is used when it exists. A 401, 403 or 404 is never served from the cache; those are real answers about the current session or resource.

When the browser reports itself offline, the cache is read without attempting the network. A screen opened for the first time in airplane mode renders immediately. A request that hangs on a half-connected network is aborted after 4 seconds and the cached body is used. If nothing is cached, the screen says the page is not saved on the device yet.

Only these reads are cacheable:

- `GET /api/meals?date=...`
- `GET /api/days/{date}`
- `GET /api/days?from=...&to=...`
- `GET /api/budget/{date}`
- `GET /api/trends?from=...&to=...`
- `GET /api/products` (the saved-product list, exact path and no query)
- `GET /api/profile`

Everything else is online-only. That includes auth, `/api/ai/providers`, ingest tokens, aliases, Google Health status and connection routes, and `/api/health`. The service worker does not cache API responses. This allowlist lives in the app so every entry stays tied to a user id.

Cache keys are `<user_id>:<path and query>`. Logging out clears that user's queue, cache and cached session, and warns first if unsynced changes would be lost. Another account's rows are untouched.

Cache growth is bounded only by logging out. There is no size cap or eviction yet.

## Write-through

When the server answers `applied` or `duplicate`, the queue updates the cached read bodies for that user before deleting the queue row.

- A meal upsert or delete patches the cached meals body for the affected date.
- A day patch patches the cached day body for that date.
- Cached budget and trends bodies that cover the affected date are invalidated. The next read fetches fresh server math.

The order matters. If the queue row were deleted first, a screen refetching in that window would see neither the queued operation nor the new server value.

The UI overlay applies only `pending` and `sending` operations on top of reads. `stale`, `rejected` and `failed` operations are not shown as fact. They may lose, may be refused, or may not have reached the server. They remain visible in **Settings → Sync**.

## Sessions and long absences

The session cookie expires after 30 days of inactivity, and each request rolls it forward. A device that is offline longer than that still has its queue and cache. The first flush needs a fresh login; the identity guard pauses with the `auth` message until then.

The server treats any operation whose `at` is older than the 180-day tombstone window as `stale`. It can no longer prove the operation is newer than a delete. Those operations stay visible and can be retried or discarded, but they never overwrite newer data. That is the documented cost of being offline for more than six months.

## Account switching and stale tabs

- Queue rows and cached bodies are keyed by user id. Two accounts on one phone never share them.
- A tab left open as user A does not flush after user B signs in. The guard sees B's session and pauses with the `identity` message.
- A different account signing in clears the previous user's cached reads and session. If that account still has queued changes, the login screen warns and asks before discarding them.
- A tab that has been open across a logout does not show its old reads to the next login, because logout clears that user's cache and session.
- Logging out ends the server session first, then clears local data. It is refused while offline, so queued changes cannot be deleted without a chance to sync. With queued changes, an in-card confirmation names the count before discarding them.

## The app shell

- The manifest is `web/static/manifest.webmanifest`. It sets the name, short name, standalone display, theme and background colors, and 192, 512 and maskable 512 icons. The background is the light sand `#f8f4ec`.
- The manifest's `theme_color` is static brand blue. `app.html` carries light and dark `theme-color` metas so the browser bar follows the theme. `web/static/icons/` also holds the Apple touch icon.
- The service worker is SvelteKit's built-in file, `web/src/service-worker.ts`. There is no Workbox or `@vite-pwa/sveltekit` dependency.
- The cache name is `cupful-<version>`. The version comes from SvelteKit's `version.name`. `vite.config.ts` sets it to `CUPFUL_BUILD_ID` when provided, otherwise `<package version>+<short git SHA>`, otherwise the package version. A new build gets a new cache and the old `cupful-*` caches are deleted on activate. **Settings → About** shows the same label.
- On install the worker precaches the app shell: the root, `index.html`, every immutable build asset and every static asset, except the barcode scanner's roughly 925 KB wasm file. That file is cached the first time someone scans. Development builds skip caching.
- Navigations are network-first and fall back to the cached `index.html` when offline. Hashed static assets are cache-first and refreshed in the background, so a new deploy is picked up on the next visit.
- API requests never pass through the service worker. The IndexedDB layer owns reads and the queue owns writes.
- When a new worker is waiting, the page shows an **Update available** banner with **Reload**. Reload sends `SKIP_WAITING` to the waiting worker and reloads when it takes control.

## Installing

Install manually from the browser. There is no app store listing.

### Android with Chrome

1. Open the instance over HTTPS.
2. Open the Chrome menu and press **Install app** (or **Add to Home screen**).
3. Cupful opens standalone.

Chrome mints a WebAPK through Google Play services. If "Installing" finishes with no icon, check the app drawer and `chrome://webapks`. The minting step fails silently when Play services or its network is unavailable.

### Android with Brave

**Add to Home screen** creates a shortcut, not a WebAPK. It may open with browser chrome.

### iOS with Safari

1. Open the instance in Safari.
2. Press Share, then **Add to Home Screen**.

iOS uses Safari's own install path, not the Chrome prompt.

A web app cannot read Apple Health. See [shortcuts.md](shortcuts.md) for sending steps.

The Playwright suite covers Chromium offline behavior only. It can go offline, navigate, read cached data, queue writes and flush on reconnect. It cannot exercise the Android install prompt or iOS Add to Home Screen, so check those by hand on a phone.
