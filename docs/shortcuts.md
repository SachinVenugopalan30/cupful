# Sending steps from your phone

Cupful can take your daily step count (and gym minutes) from an automation on your phone. A value sent this way is marked **Shortcut** and never overwrites a value you typed in yourself; if you clear a field, the next automation run fills it again.

You need a Cupful address your phone can reach (the tailnet name, with Tailscale on) and an ingest token.

## 1. Create a token

Settings, Ingest tokens, New token. Give it a label ("iPhone"). The token (starts with `cupful_`) is shown **once**: copy it now. Cupful stores only a hash, so a lost token cannot be shown again; revoke it and make another.

Make one token per phone or automation, so you can revoke one without touching the others.

## 2. Test with curl

```sh
curl -X POST https://cupful.your-tailnet.ts.net/api/ingest/activity \
  -H "Authorization: Bearer cupful_..." \
  -H "Content-Type: application/json" \
  -d '{"date": "2026-10-27", "steps": 11234, "gym_minutes": 55}'
```

The reply is `{"changed": true}` when something was saved and `{"changed": false}` when nothing changed, for example because you had already typed that day's steps. Send `steps`, `gym_minutes`, or both.

| Status | Meaning |
| --- | --- |
| 400 | Bad date or number, no `steps`/`gym_minutes`, or a date outside the last 14 days through tomorrow (your timezone) |
| 401 | Missing, wrong or revoked token |
| 413 | Body over 4 KB |
| 422 | A field Cupful does not accept (only `date`, `steps`, `gym_minutes`) |
| 429 | More than 60 requests in an hour for this token |

Steps must be 0 to 150,000 and gym minutes 0 to 600. Decimals are rounded.

## 3. iOS Shortcuts

Build the shortcut, then run it every evening from a personal automation.

1. Shortcuts, **New Shortcut**. Name it "Send steps to Cupful".
2. **Find Health Samples**: Type **Steps**, Start Date **is today** (add a filter "Start Date is today"), Sort by Start Date. Leave Limit off.
3. **Calculate Statistics**: **Sum** of the Health Samples. (If you prefer, use "Get Details of Health Sample" then a sum; the Sum statistic is simplest.)
4. **Format Date**: Current Date, Custom format `yyyy-MM-dd`.
5. **Get Contents of URL**:
   - URL: `https://cupful.your-tailnet.ts.net/api/ingest/activity`
   - Method: **POST**
   - Headers: `Authorization` = `Bearer cupful_...`, `Content-Type` = `application/json`
   - Request Body: **JSON**, with `date` (Text, the formatted date), `steps` (Number, the Sum).
6. Run it once by hand and check Today in Cupful: Steps should show the source **Shortcut**.

Then the automation: Shortcuts, **Automation**, **New Automation**, **Time of Day** 21:00, **Daily**, **Run Immediately**, action **Run Shortcut** = "Send steps to Cupful".

Notes:

- Your phone must be on the tailnet (Tailscale on) when it runs. If the automation fails, the day simply stays unfilled; the next evening's run does not backfill, so run it by hand or change `date` to send a past day (up to 14 days back).
- Running more than once a day is fine: the newest count replaces the last Shortcut value.
- The token is in the Shortcut. Anyone who can open the Shortcut can send data as you, so don't share it; revoke the token if it leaks.

## 4. Android

Any app that can send an HTTP POST on a schedule works. Two common ones:

**HTTP Shortcuts** (free, open source): create a shortcut with method POST, the URL above, header `Authorization: Bearer cupful_...`, content type JSON, and a body like `{"date": "{date}", "steps": {steps}}`. Use a variable for the date (type "Date/time", format `yyyy-MM-dd`) and one for steps, and schedule it from the app's "Scheduling" settings. Steps have to come from somewhere: Health Connect is not readable by HTTP Shortcuts itself, so pair it with Tasker or a fitness app that exposes the count.

**Tasker**: a profile "Time 21:00" with a task that reads your step count (from your fitness app or Health Connect via a plugin), then an **HTTP Request** action: Method POST, URL as above, Headers `Authorization:Bearer cupful_...` and `Content-Type:application/json`, Body `{"date":"%DATE_ISO","steps":%STEPS}` (set `%DATE_ISO` with a Variable Convert or `Get Date` in the `yyyy-MM-dd` form).

The request is the same everywhere: a POST with the bearer header and a small JSON body.
