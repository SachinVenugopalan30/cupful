<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Google Health steps sync

Cupful can read your daily step count from the Google Health API v4. It does this once a day, or when you press **Sync now** in Settings.

The feature is optional and off until you configure it. It needs a Google Cloud project of your own. Cupful ships no shared Google credentials.

What it does: it reads steps and exercise sessions for the last 14 days. Steps go to your days with the source "Google". A value you typed by hand is never overwritten. Workouts are stored and shown in Cupful, and fill a day's gym minutes (see Workouts).

## Workouts

- Cupful reads Google Health `exercise` data points (runs, walks, gym sessions and so on) with `GET /v4/users/me/dataTypes/exercise/dataPoints`, filtered on `exercise.interval.civil_start_time` for the last 14 days. The same `activity_and_fitness.readonly` scope is used; no new consent is needed beyond what Google's scope page implies for activity and fitness data (it does not list exercise explicitly, so a 403 here shows as **Reconnect Google Health**). The location scope is never requested, so routes are not read.
- Only workouts that reach Google Health appear. A session recorded in another app shows up only if that app syncs it to Google Health.
- Google returns 25 sessions per page, newest first. Cupful follows the page tokens up to 20 pages (500 sessions) per sync.
- Re-syncing updates a changed session. A session you delete in Google Health stays in Cupful, and its stored copy stays; if that leaves a day with no matching workout, the day's Google-written gym minutes are cleared on the next sync.
- **Gym types:** the exercise types that count as gym minutes. The default is `STRENGTH_TRAINING`, `FUNCTIONAL_STRENGTH_TRAINING`, `WEIGHTLIFTING`, `POWERLIFTING`, `WEIGHT_MACHINES`, `FREE_WEIGHTS`, `WEIGHTS`, `BODY_WEIGHT`, `RESISTANCE_BANDS`, `TRX`, `CALISTHENICS`, `HIIT`, `CIRCUIT_TRAINING`, `CROSSFIT` and the indoor machines `TREADMILL`, `ELLIPTICAL`, `ROWING_MACHINE`, `STAIRCLIMBER`, `SPINNING`, `STATIONARY_BIKE` and `ASSAULT_BIKE` (all names from Google's `Exercise.ExerciseType`). Outdoor running, walking, hiking and biking are not in the default. The choices are a curated list plus every type present in your stored workouts, so anything Google sends can be ticked. You can change the list in Settings (`PUT /api/google/gym-types`, at most 30). After each sync and each change, every day with a matching workout gets gym minutes = the sum of their active minutes, rounded, at least 1; a day whose Google-written gym minutes no longer have a matching workout is cleared. A value you typed by hand is never touched.
- Workouts are derived data: they are not in the JSON backup or CSV export (a sync restores them). The gym type choice is part of the backup.

Privacy: step data flows from Google to your server only. Cupful sends nothing about you to anyone else, and it contacts Google only after you press **Connect**. Tokens are encrypted on your server. See `CUPFUL_SECRET_KEY` in `.env.example`.

## 1. Create the Google Cloud project

1. Open the [Google Cloud console](https://console.cloud.google.com/) and create a project. Any name works, for example "Cupful".
2. Enable the **Google Health API** for that project. Go to APIs & Services, Library, search for "Google Health API" and press Enable. The setup guide is at <https://developers.google.com/health/setup>.

## 2. Configure the OAuth consent screen

1. Open APIs & Services, OAuth consent screen (Google Auth Platform). Set user type **External** and publishing status **Testing**.
2. Under Data Access, add the scope `https://www.googleapis.com/auth/googlehealth.activity_and_fitness.readonly`.
3. Under Audience, add your own Google account to **Test users**. Add anyone else on your Cupful who will connect. Only listed users can sign in while the app is in Testing. The limit is 100.

## 3. Create the OAuth client

1. Open Credentials, press Create credentials, choose OAuth client ID, and set application type **Web application**.
2. Add the authorized redirect URI `https://<name>.<tailnet>.ts.net/api/google/callback`. Use the address you open Cupful at, plus `/api/google/callback`. It does not need to be reachable from the internet; only your browser has to reach it. For a local test, `http://localhost:8080/api/google/callback` also works.
3. Copy the client ID and client secret.

## 4. Configure Cupful

Add these to `.env`, or to the container environment, and restart:

```sh
CUPFUL_GOOGLE_CLIENT_ID=...apps.googleusercontent.com
CUPFUL_GOOGLE_CLIENT_SECRET=...
CUPFUL_PUBLIC_URL=https://<name>.<tailnet>.ts.net
```

`CUPFUL_PUBLIC_URL` must match the redirect URI without the path. If it is unset, Cupful builds the address from the request's `Host` header. That only works when the header is exactly the registered host.

Open Settings, find **Google Health**, and press **Connect Google Health**. Google asks you to approve read-only access to activity and fitness data. Back in Cupful, press **Sync now** to pull the last 14 days.

## Testing mode: reconnect weekly

While the consent screen is in **Testing**, Google expires refresh tokens after 7 days. Cupful notices, stops retrying, and Settings shows **Reconnect Google Health**. One click on the Google consent page fixes it.

Publishing the app to "In production" removes the 7-day limit. Refresh tokens then last until you revoke them or leave them unused for about six months.

For a personal app you do not have to pass Google's verification review to publish. Until the app is verified, Google shows an "unverified app" warning at sign-in, and the 100-user cap still applies. Verification is only needed for more users than that.

## Disconnect

**Disconnect** in Settings asks Google to revoke the token and deletes it from your server. Steps and workouts already synced stay.

You can also remove access at <https://myaccount.google.com/permissions>.

## Troubleshooting

- **`redirect_uri_mismatch` at Google:** the URI in the client must equal `CUPFUL_PUBLIC_URL` plus `/api/google/callback`, exactly.
- **`access_denied`, "app not verified" or "has not completed verification":** your account is not in the Test users list.
- **403 or a missing permission on sync:** the scope was not added on the consent screen. Add it, then reconnect.
- **No Google card in Settings:** the client id and secret are not set, or the server was not restarted.
