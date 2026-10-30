<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Sending steps from your phone

Cupful can take your daily step count and gym minutes from an automation on your phone. A value sent this way is marked **Shortcut** and never overwrites a value you typed yourself. If you clear a field, the next automation run fills it again.

You need a Cupful address your phone can reach (the tailnet name with Tailscale on) and an ingest token.

## 1. Create a token

1. Open Settings and find **Ingest tokens**.
2. Press **New token** and give it a label, for example "iPhone".
3. Copy the token now.

The token starts with `cupful_` and is shown once. Cupful stores only a hash, so a lost token cannot be shown again. Revoke it and make another.

Make one token per phone or automation, so you can revoke one without touching the others.

## 2. Test with curl

```sh
curl -X POST https://cupful.your-tailnet.ts.net/api/ingest/activity \
  -H "Authorization: Bearer cupful_..." \
  -H "Content-Type: application/json" \
  -d '{"date": "2026-10-27", "steps": 11234, "gym_minutes": 55}'
```

The reply is `{"changed": true}` when something was saved, and `{"changed": false}` when nothing changed. Nothing changes when, for example, you already typed that day's steps.

Send `steps`, `gym_minutes` or both.

| Status | Meaning |
| --- | --- |
| 400 | Bad date or number, no `steps` or `gym_minutes`, or a date outside the last 14 days through tomorrow in your timezone |
| 401 | Missing, wrong or revoked token |
| 413 | Body over 4 KB |
| 422 | A field Cupful does not accept. Only `date`, `steps` and `gym_minutes` are allowed. |
| 429 | More than 60 requests in an hour for this token |

Steps must be 0 to 150,000. Gym minutes must be 0 to 600. Decimals are rounded.

## 3. iOS Shortcuts

Build the shortcut, then run it every evening from a personal automation.

1. Open Shortcuts and press **New Shortcut**. Name it "Send steps to Cupful".
2. Add **Find Health Samples**. Set type to **Steps** and add the filter "Start Date is today". Sort by Start Date and leave Limit off.
3. Add **Calculate Statistics**. Choose **Sum** of the Health Samples.
4. Add **Format Date**. Use Current Date and the custom format `yyyy-MM-dd`.
5. Add **Get Contents of URL**.
6. Set the URL to `https://cupful.your-tailnet.ts.net/api/ingest/activity`.
7. Set the method to **POST**.
8. Add the headers `Authorization` = `Bearer cupful_...` and `Content-Type` = `application/json`.
9. Set the request body to **JSON** with `date` (Text, the formatted date) and `steps` (Number, the Sum).
10. Run it once by hand and check Today in Cupful. Steps should show the source **Shortcut**.

Then create the automation:

1. Open Shortcuts, then **Automation**, then **New Automation**.
2. Choose **Time of Day**, 21:00, **Daily**, **Run Immediately**.
3. Choose **Run Shortcut** and select "Send steps to Cupful".

Notes:

- Your phone must be on the tailnet (Tailscale on) when the automation runs. If it fails, the day stays unfilled. The next run does not backfill. Run it by hand, or change `date` to send a past day, up to 14 days back.
- Running more than once a day is fine. The newest count replaces the last Shortcut value.
- The token is stored in the Shortcut. Anyone who can open the Shortcut can send data as you. Do not share it. Revoke the token if it leaks.

## 4. Android

Any app that can send an HTTP POST on a schedule works. Two common choices:

### HTTP Shortcuts

HTTP Shortcuts is free and open source.

1. Create a shortcut with method POST and the URL above.
2. Add the header `Authorization: Bearer cupful_...` and content type JSON.
3. Use a body like `{"date": "{date}", "steps": {steps}}`.
4. Use a variable for the date (type "Date/time", format `yyyy-MM-dd`) and one for steps.
5. Schedule it in the app's **Scheduling** settings.

HTTP Shortcuts cannot read Health Connect by itself. Pair it with Tasker or a fitness app that exposes the count.

### Tasker

1. Create a profile "Time 21:00".
2. Add a task that reads your step count, from your fitness app or Health Connect through a plugin.
3. Add an **HTTP Request** action with method POST and the URL above.
4. Set the headers `Authorization:Bearer cupful_...` and `Content-Type:application/json`.
5. Set the body to `{"date":"%DATE_ISO","steps":%STEPS}`. Set `%DATE_ISO` with a Variable Convert or Get Date in `yyyy-MM-dd` form.

The request is the same everywhere: a POST with the bearer header and a small JSON body.
