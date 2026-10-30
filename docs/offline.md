# Offline and PWA behavior

Cupful is a static SvelteKit app served by the Rust binary. Once it has been opened, it installs
as a PWA, keeps the last reads for each account in IndexedDB, and queues logged-data writes so a
meal or a weigh-in can be entered with no connection. Configuration and anything that calls an
external service stay online.

This page describes what the app does, not how to build it. The implementation lives in
`web/src/service-worker.ts` and `web/src/lib/offline/`.

## What works offline

These screens read through the per-user IndexedDB cache:

- **Today:** the day, its meals, the budget for the day, and the saved product list.
- **Activity:** the selected day, including steps, gym minutes, body parts and weigh-in.
- **Trends:** the cached range response.
- **Saved products:** the list is cached, so previously seen products can be logged again.
- **Profile:** the cached profile and auth state are enough to boot the app for the same account.

Logged-data writes (meal upserts, meal deletes, day patches) always enqueue first and flush to
`POST /api/sync` when the connection allows.

These stay **online-only** and the UI says so when offline:

- Settings and the profile form, including timezone and units.
- AI providers and Test connection.
- Ingest tokens.
- Aliases: listing, remembering and deleting.
- AI estimates, because they call the configured model.
- A barcode that has never been looked up. Previously saved products and cached reads still work.
- Import, restore, export and backup.

A stale budget is still shown offline. Cupful does not recompute a budget or trend locally; it
shows the last cached server value. Meal totals come from the cached meals plus the overlay of
still-queued ops, so the food list is current even when the budget card is not.

## The write queue

Every logged-data write goes through IndexedDB in front of `POST /api/sync`, online or offline.
`sync.sendOps` is still the only logged-data write path; it now enqueues the op and asks the queue
to flush instead of posting directly.

An op has one of five statuses:

| Status | Meaning |
| --- | --- |
| `pending` | Queued, not sent yet. |
| `sending` | Part of the batch currently in flight. |
| `stale` | The server says an older write cannot win (see below). Kept so the user can retry or discard it. |
| `rejected` | The server refused the op and returned a message. Kept so it can be fixed, retried or discarded. |
| `failed` | The request failed in transport or with a retryable server answer. Retried on reconnect, on the next flush, or from Sync now. |

A flush sends the oldest queued ops for one user, in chunks of at most **100 ops**, which is the
server's request limit. The answer has one result per op:

- `applied` and `duplicate` are written through to the cached read bodies and then the queue row is
  deleted.
- `stale` and `rejected` are kept, with their error, until the user retries or discards them.
- A chunk that fails before an answer stays queued as `failed` and is retried with backoff.

The banner shows how many changes are waiting. **Settings → Sync** lists each queued op with its
status, last error, and **Retry** / **Discard** actions, so nothing can sit stuck forever.

## Identity guard

Before each flush the queue asks `GET /api/auth/state` who the live session belongs to and compares
that user id with the user id on the ops it is about to send.

- Signed out: the queue pauses with an `auth` message ("Sign in again to send N changes"), keeps the
  ops, and resumes after the same user signs in.
- A different user: the queue pauses with an `identity` message. Ops and cached reads belonging to
  the other account are never sent or shown.
- The guard request itself fails (network, 5xx): the flush does not run; it backs off and tries the
  guard again later rather than sending blind.

This is stronger than the account-isolation rule in PLAN §12. Keying queue rows and cache entries by
user id stops one account's data being read as another's, but it does not prove the cookie still
belongs to that user. Checking `/api/auth/state` before every batch makes a stale tab, an expired
session, or a logout/login in another tab unable to push the wrong account's writes.

## Failure behavior

- **401 during a flush** pauses the queue without a redirect. The items stay, the banner asks for a
  sign-in, and the page the user was on is not thrown away. This differs from the normal API 401
  handling, which reloads `/login`, because a queued write must survive.
- **5xx, 429 or a network failure** retries with capped exponential backoff. The first retry is a
  few seconds out, doubles, and stops growing at a small ceiling. Reconnecting or tapping Sync now
  resets the delay.
- **A whole batch rejected with another 4xx** is resent one op at a time, so only the change the
  server refuses is marked failed; the rest of the batch lands.
- **IndexedDB unavailable, a quota error, or a failed enqueue** falls back to a direct
  `POST /api/sync` when the app is online. Offline, there is nowhere to persist the write, so the
  screen reports that the change was not saved.
- Token refresh, provider errors and other online-only actions are unaffected; they fail normally
  with their own messages.

## The read cache

Reads are network-first. A 2xx JSON answer is stored for the current user and returned. On a
network error or a 5xx, the cached body for the same user and path is used when it exists. A 401,
403 or 404 is never served from the cache; those are real answers about the current session or
resource.

Only these reads are cacheable:

- `GET /api/meals?date=...`
- `GET /api/days/{date}`
- `GET /api/days?from=...&to=...`
- `GET /api/budget/{date}`
- `GET /api/trends?from=...&to=...`
- `GET /api/products` (the saved-product list; exact path and no query)
- `GET /api/profile`

Everything else is online-only, including auth, `/api/providers`, ingest tokens, aliases, Google
Health status and connection routes, and `/api/health`. The service worker does not cache API
responses at all; this allowlist lives in the app so every entry stays tied to a user id.

Cache keys are `<user_id>:<path and query>`. Logging out clears that user's queue, cache and cached
session, and warns first if unsynced changes would be lost. Another account's rows are untouched.

Cache growth is bounded only by logging out. There is no size cap or eviction yet.

`// ponytail: no eviction or byte budget, logout is the only cleanup; add a per-user LRU or a quota
check when long-lived accounts fill a phone's storage.`

## Write-through

When the server answers `applied` or `duplicate`, the queue updates the cached read bodies for that
user before deleting the queue row:

- A meal upsert or delete patches the cached meals body for the affected date.
- A day patch patches the cached day body for that date.
- Cached budget and trends bodies that cover the affected date are invalidated, so the next read
  fetches fresh server math.

The order matters. If the queue row were deleted first, a screen refetching in that window would see
neither the queued op nor the new server value. Write-through closes that gap.

The UI overlay applies only `pending` and `sending` ops on top of reads. `stale`, `rejected` and
`failed` ops are not shown as fact: they may lose, may be refused, or may not have reached the
server at all. They remain visible in Settings → Sync.

## Sessions and long absences

The session cookie expires after 30 days of inactivity and each request rolls it forward. A device
that is offline for longer than that still has its queue and cache, but the first flush needs a
fresh login; the identity guard pauses with the `auth` message until then.

The server treats any op whose `at` is older than the 180-day tombstone window as `stale`, because
it can no longer prove the op is newer than a delete. Those ops stay visible and can be discarded or
retried, but they never overwrite newer data. This is the documented cost of being offline for more
than six months.

Account switching and stale tabs:

- Queue rows and cached bodies are keyed by user id, so two accounts on one phone never share them.
- A tab left open as user A does not flush after user B signs in; the guard sees B's session and
  pauses with the `identity` message.
- A different account signing in clears the previous cached user's reads and session. If that
  account still has queued changes, the login screen warns and asks before discarding them.
- A tab that has been open across a logout does not show its old reads to the next login, because
  logout clears that user's cache and session.
- Logging out ends the server session first, then clears local data. It is refused while offline, so
  queued changes cannot be deleted without a chance to sync. With queued changes, an in-card
  confirmation names the count before discarding them.

## PWA

- The manifest is `web/static/manifest.webmanifest`: name, short name, standalone display, theme
  and background colors, and 192, 512 and maskable 512 icons. The background is the light sand
  `#f8f4ec`. The manifest's `theme_color` is static (brand blue); `app.html` carries light and dark
  `theme-color` metas so the browser bar follows the theme. `web/static/icons/` also holds the
  Apple touch icon.
- The app uses SvelteKit 3's built-in service worker file, `web/src/service-worker.ts`. There is no
  Workbox or `@vite-pwa/sveltekit` dependency.
- The cache name is `cupful-<version>`, where `version` comes from SvelteKit's `version.name`.
  `vite.config.ts` sets it to `CUPFUL_BUILD_ID` when provided, else `<package version>+<short git
  SHA>`, else the package version. A new build therefore gets a new cache and the old `cupful-*`
  caches are deleted on activate. Settings → About shows the same label.
- On install the worker precaches the app shell: the root, `index.html`, every immutable build asset
  and every static asset, except the barcode scanner's ~925 KB wasm file. That file is cached the
  first time someone scans. Development builds skip caching entirely.
- Navigations are network-first and fall back to the cached `index.html` when offline. Hashed static
  assets are cache-first and refreshed in the background, so a new deploy is picked up on the next
  visit.
- API requests are never handled by the service worker; the app's IndexedDB layer owns reads and the
  queue owns writes.
- When a new worker is waiting, the page shows an "Update available" banner with Reload. Reload sends
  `SKIP_WAITING` to the waiting worker and reloads when the new worker takes control.

## Installing

Install manually:

- **Android (Chrome):** open the instance over HTTPS, then Chrome menu → **Install app** (or **Add to
  Home screen**). Cupful opens standalone.
- **iOS (Safari):** open the instance, Share → **Add to Home Screen**. iOS uses Safari's own install
  path, not the Chrome prompt.

The headless Playwright suite covers Chromium offline mechanics only: it can go offline, navigate,
read cached data, queue writes and flush on reconnect. It cannot exercise the Android install prompt
or iOS Add to Home Screen, so those are checked by hand on a phone.
