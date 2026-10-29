<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Google Health (optional steps sync)

Cupful can read your daily step count from the Google Health API (v4) once a day, or when you press **Sync now** in Settings. It is optional and off until you configure it. It needs a Google Cloud project of your own: Cupful ships no shared Google credentials.

What it does: reads steps for the last 14 days and writes them to your days with the source "Google". A value you typed by hand is never overwritten. Gym minutes are not synced.

Privacy: step data flows from Google to your own server only. Cupful sends nothing about you to anyone else, and it contacts Google only after you press Connect. Your tokens are encrypted on your server (see `CUPFUL_SECRET_KEY` in `.env.example`).

## 1. Create the Google Cloud project

1. Open the [Google Cloud console](https://console.cloud.google.com/) and create a project (any name, for example "Cupful").
2. Enable the **Google Health API** for that project (APIs & Services, Library, search "Google Health API", Enable). Setup guide: <https://developers.google.com/health/setup>.

## 2. OAuth consent screen

1. APIs & Services, OAuth consent screen (Google Auth Platform). User type **External**; publishing status **Testing**.
2. Data Access: add the scope `https://www.googleapis.com/auth/googlehealth.activity_and_fitness.readonly`.
3. Audience: under **Test users** add your own Google account (and anyone else on your Cupful who will connect). Only listed users can sign in while the app is in Testing; the limit is 100.

## 3. Create the OAuth client

1. Credentials, Create credentials, OAuth client ID, application type **Web application**.
2. Authorized redirect URI: `https://<name>.<tailnet>.ts.net/api/google/callback`, the same address you open Cupful at, plus `/api/google/callback`. It does not need to be reachable from the internet; only your browser has to reach it. (To try it on your own computer, `http://localhost:8080/api/google/callback` also works.)
3. Copy the client ID and client secret.

## 4. Configure Cupful

Add to your `.env` (or the container environment) and restart:

```
CUPFUL_GOOGLE_CLIENT_ID=...apps.googleusercontent.com
CUPFUL_GOOGLE_CLIENT_SECRET=...
CUPFUL_PUBLIC_URL=https://<name>.<tailnet>.ts.net
```

`CUPFUL_PUBLIC_URL` must match the redirect URI you registered (without the path). If you leave it unset, Cupful builds the address from the request's `Host` header, which only works when that header is exactly the registered host.

Then open Settings, find **Google Health**, and press **Connect Google Health**. Google asks you to approve read-only access to activity and fitness data. Back in Cupful, press **Sync now** to pull the last 14 days.

## Testing mode: reconnect weekly

While the consent screen is in **Testing**, Google expires refresh tokens after **7 days**. Cupful notices (the sync gets `invalid_grant`), stops retrying, and Settings shows **Reconnect Google Health**. One click and the Google consent page fixes it.

Publishing the app to "In production" removes the 7-day limit: refresh tokens then last until you revoke them or leave them unused for about six months. For a personal app you do not have to pass Google's verification review to publish, but until it is verified Google shows an "unverified app" warning at sign-in, and the 100-user cap still applies. Verification is only needed for more users than that.

## Disconnecting

**Disconnect** in Settings asks Google to revoke the token and deletes it from your server. Steps already synced stay. You can also remove access at <https://myaccount.google.com/permissions>.

## Troubleshooting

- **redirect_uri_mismatch** at Google: the URI in the client must equal `CUPFUL_PUBLIC_URL` plus `/api/google/callback`, exactly.
- **access_denied / "app not verified" / "has not completed verification"**: your account is not in the Test users list.
- **403 or missing permission on sync**: the scope was not added on the consent screen. Add it, then Reconnect.
- No Google card in Settings: the client id and secret are not set, or the server was not restarted.
