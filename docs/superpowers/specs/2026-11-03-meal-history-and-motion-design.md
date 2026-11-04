# Meal history and motion refresh

Date: 2026-11-03
Status: Revised after independent review
Scope: Cupful web and API

## 1. Problem

Two gaps in daily use:

1. Trends shows what the range averaged, but not what was eaten most often. The owner wants a count of common meals.
2. Repeating a meal is hidden inside an expanded meal row and only logs onto the day being viewed. Copying yesterday's dinner to today means navigating back to yesterday and using an action that writes to yesterday. The owner wants an easy way to copy a meal and its macros onto the day they are logging now, including a day a few days after the original.

The current motion is also tuned for speed over readability. Feedback is quick, but sheets appear instantly, bars pop, and chart entrances feel abrupt. The owner wants motion that is calmer, smoother and more refined without making the app feel slow.

## 2. Goals

- Show the most frequently logged meals for the selected Trends range.
- Make a previously logged meal repeatable onto the selected day in a few taps. The selected day is today or a past day, because the app does not schedule future meals.
- Always show the copied meal in Review before it is saved, because portions and macros may change.
- Keep logged-data writes on the existing sync path.
- Keep the repeat flow usable offline.
- Refine motion globally, with fast feedback and calmer movement.
- Add no new dependencies and no new saved-meal table.

## 3. Non-goals

- Named saved-meal templates or favourites. History is the library for now.
- Scheduling a meal for a future date. The user opens the app on the day they eat it and repeats from history.
- Re-estimating a repeated meal with AI.
- Sharing meals between users.
- Automatic meal detection or photo history.
- Page transitions or entrance animations on screens other than Trends and sheets.
- Changing the meal schema.

## 4. Phase 0: motion refresh

### 4.1 Principles

- Feedback stays fast. Hover, press and focus answer immediately.
- Movement is slower and softer. Meters, bars and sheets get enough time to read.
- One entrance per surface. No looping motion except the skeleton pulse.
- No spring, bounce or overshoot.
- Motion runs on first mount or a range change, not on every data refresh.
- Stagger is capped so a 365-day range never takes more than about one second.
- `prefers-reduced-motion` turns everything off.

### 4.2 Tokens

Update `web/src/lib/styles/tokens.css`:

| Token | Now | New | Use |
|---|---:|---:|---|
| `--dur-fast` | 120 ms | 160 ms | hover, press, focus, chips |
| `--dur-med` | 180 ms | 240 ms | saved text, small state changes |
| `--dur-slow` | 240 ms | 360 ms | meters, week bars, progress fills |
| `--dur-enter` | none | 420 ms | sheets |
| `--dur-exit` | none | 200 ms | sheet close, backdrop fade |
| `--ease` | `cubic-bezier(0.2, 0.7, 0.2, 1)` | `cubic-bezier(0.22, 1, 0.36, 1)` | movement and entrances |
| `--ease-state` | none | `cubic-bezier(0.4, 0, 0.2, 1)` | hover, press, color, chips |

Under `prefers-reduced-motion: reduce`, set all duration tokens to `0ms` as today.

### 4.3 Global changes

- Buttons, links, chips and other small controls use `--dur-fast` with `--ease-state`.
- Meter fills use `--dur-slow` with `--ease`.
- Saved text uses `--dur-med` with `--ease`.
- Skeleton pulse becomes 1.8 s from 0.65 to 1 opacity, instead of 1.4 s from 0.55.
- Dialogs get a calm entrance and exit:
  - Backdrop fades over `--dur-exit`.
  - Panel fades in, rises 16 px and scales from 0.985 over `--dur-enter`.
  - Close runs in `--dur-exit`.
  - Use CSS `@starting-style` for the entrance. The supported baseline is Chrome 117+, Safari 17.5+ and Firefox 129+. On older browsers the sheet opens instantly and still works. Do not add a Svelte transition fallback.
  - Exit needs an explicit `closing` state. `requestClose()` sets `closing = true` and leaves the dialog open. A `[data-closing]` style runs the exit transition. On `transitionend`, call `dialog.close()` and then the parent's close callback. The native `cancel` event is prevented and routed to `requestClose()`.
  - Reduced motion calls `dialog.close()` and the close callback immediately.
  - Apply the same lifecycle to `Review.svelte`, `BarcodeEntry.svelte` and the new `RepeatSheet.svelte`. Today, `Review` and `BarcodeEntry` unmount from `onclose`, so the exit never plays.
  - Keep focus predictable. Closing a sheet returns focus to its trigger. Opening Review from the repeat sheet focuses the Review heading or its first control. Closing Review returns focus to the composer's "Repeat a meal" button, or to the picked row if the sheet is restored.
- Every explicit animation keeps its `prefers-reduced-motion` override.
- No page transitions. No entrance animations on Today, Activity, Settings, login or setup.

### 4.4 Trends

- Intake and steps bars: 760 ms, 16 ms stagger, capped at 15 bars so the wave adds at most 224 ms.
- Consistency cells: 360 ms, 18 ms stagger, capped at 14 cells. Fade from 0.94 scale and 0 opacity. No pop, no overshoot.
- Macro balance and meal mix bars: 760 ms, 80 ms stagger.
- Headline count-up: 800 ms with the existing ease-out cubic.
- Weight area: 600 ms with a 120 ms delay.
- The weight line keeps no dash animation.

Update these files, not just the tokens:

| File | Change |
|---|---|
| `web/src/lib/trends/Charts.svelte` | bars 760 ms, 16 ms stagger, `Math.min(i, 14)`; weight area 600 ms with 120 ms delay |
| `web/src/lib/trends/Consistency.svelte` | 360 ms, 18 ms stagger, `Math.min(i, 13)`, fade from 0.94 scale |
| `web/src/lib/trends/Analytics.svelte` | 760 ms, 80 ms stagger |
| `web/src/lib/trends/CountUp.svelte` | 800 ms |
| `web/src/lib/trends/count-up.ts` | update the comment and any timing test |
| `web/src/lib/activity/ActivityWeek.svelte` | bar height `--dur-slow`, background `--dur-fast` |
| `web/src/lib/activity/ActivitySection.svelte` | saved text `--dur-med` |
| `web/src/lib/styles/global.css` | control transitions use `--ease-state`; skeleton 1.8 s |

The new `FrequentMeals.svelte` uses the same bar timing. Its rows animate only on the first mount of a range, not when a data refresh adds a row.

### 4.5 Activity

- Week bar height: `--dur-slow` with `--ease`.
- Day column background: `--dur-fast` with `--ease-state`.
- Saved text: `--dur-med` with `--ease`.

### 4.6 Motion acceptance

- Trends at 7, 30, 90 and 365 days finishes its entrance in about one second or less.
- A sheet opens and closes without a flash or layout jump.
- Activity week bars change height smoothly when the selected day changes.
- `prefers-reduced-motion` shows the final state immediately.
- No animation runs on a data refresh that is not a range change or first mount.
- Changing the Trends range replays the entrance. A meal write that refreshes Trends does not.

## 5. Phase 1: meal history

### 5.1 Data model

No new tables. Reuse `meals` and `meal_items`.

A history group is keyed by a normalized label:

- If `description.trim()` is not empty, the label is the trimmed description.
- Otherwise the label is the item names joined with `, `.
- If both are empty, or the meal has no items, skip the meal.
- The key is the label lowercased with whitespace collapsed.

Each group keeps:

- `label`: the label to show.
- `times`: number of meals in the group.
- `avg_kcal`: mean of each meal's total kcal.
- `last_date`: the latest meal date in the group.
- `latest_meal_id`: the latest meal, used as the representative. Internal to the grouping helper.
- `meal_type`: the latest meal's type.

The latest meal is chosen by date, then by `updated_at`, then by id. UUIDv7 ids sort by creation time, so the newest creation wins a tie.

Grouping is by description only, not by meal type. The same label at lunch and dinner counts together. The latest meal's type is shown as a hint. This is deliberate: the description is the label the user recognises, and grouping by AI item names would split the same meal into near-duplicates.

### 5.2 API

#### Trends

Extend `GET /api/trends` with `top_meals`. The rest of the response is unchanged.

```json
"top_meals": [
  {
    "label": "Oats with banana",
    "meal_type": "breakfast",
    "times": 8,
    "avg_kcal": 420,
    "last_date": "2026-11-02"
  }
]
```

- Top five groups by `times` descending, then `last_date` descending, then `label` ascending.
- `avg_kcal` is rounded on serialize.
- Empty array when the range has no meals.
- No items in this payload.
- Group all meals in the range first, then sort groups, then take the top five. Do not apply a SQL `LIMIT` to meals before grouping.

#### History

Add `GET /api/meals/history?limit=50`.

- The server computes the window from the user's local today: `to = time::user_today`, `from = to - 364 days`.
- The stable URL is deliberate. It keeps one cache entry per user and does not roll at midnight.
- `limit` defaults to 20 and must be between 1 and 50. The sheet requests 50.
- Sort by `last_date` descending, then `times` descending, then `label` ascending.
- Group all meals in the window first, then sort groups, then apply the limit. A SQL `LIMIT` before grouping would return duplicate groups and fewer than `limit` distinct meals.
- Response:

```json
{
  "meals": [
    {
      "id": "019...",
      "date": "2026-11-02",
      "meal_type": "breakfast",
      "label": "Oats with banana",
      "description": "Oats with banana",
      "scale": 1,
      "assumptions": null,
      "confidence": null,
      "source": "ai",
      "provider_id": 1,
      "updated_at": 1760000000000,
      "items": [
        {
          "id": "019...",
          "name": "Oats",
          "portion_text": "60 g",
          "grams": 60,
          "kcal": 228,
          "protein_g": 8,
          "carbs_g": 40,
          "fat_g": 4,
          "food_id": 12,
          "origin": "food_db"
        }
      ],
      "totals": { "kcal": 420, "protein_g": 18, "carbs_g": 62, "fat_g": 11 },
      "times": 8,
      "last_date": "2026-11-02"
    }
  ]
}
```

- Each entry is the latest meal in its group, with the full items and totals, plus `times` and `last_date`.
- The client uses the items to build a repeat draft. It does not need a second request.
- The `source` and `provider_id` are the latest meal's values. The repeat draft overrides them.
- Each entry also carries `label`, the normalized group label, so the sheet and search do not duplicate the description/item-name fallback.

Both endpoints use the same grouping helper. Extract a shared `meal_json` helper used by `reads::meals` and `reads::meal_history` so the meal serialization stays in one place.

Add the client types and reads:

- `TopMeal` and `HistoryMeal` in `web/src/lib/api/types.ts`. `HistoryMeal` is `Meal & { label: string; times: number; last_date: string }`. `Trends` gains `top_meals?: TopMeal[]`, optional because an older cached body may not have it.
- `getMealHistory(limit = 50)` in `web/src/lib/api/reads.ts`, using `cachedGet` and returning `HistoryMeal[]`.

#### Route and storage

- Add `("GET", "/api/meals/history", User, get(reads::meal_history))` to `routes()` in `server/src/lib.rs`.
- Add the route to `server/tests/isolation/routes.rs` and a behavioral test to `server/tests/isolation/users.rs`.
- Refresh `.sqlx` after adding queries.
- No new outbound HTTP.

### 5.3 Trends UI

Add `web/src/lib/trends/FrequentMeals.svelte`.

- Props: `meals: TopMeal[]`, `days: number`.
- Card title: "Eaten most".
- Subtitle: `${days}-day count`.
- Rows:
  - Label.
  - Meal type, `N times`, `last 2 Nov`.
  - Right: `420 kcal avg`.
  - A thin bar behind or beside the row, width relative to the top count.
- Do not rely on `title` for the full label. Give the row an `aria-label` with the full label, count, average and last date. If the bar sits behind text, use a low-contrast track and keep the text on the surface, or put the bar below the row.
- Display only. No logging action from Trends.
- Empty state: "No meals logged in this range."
- If `top_meals` is missing from an older cached body, hide the card instead of showing the empty state.
- Render after `Analytics` and before the footnote on the Trends page.
- Use the motion tokens from Phase 0. The bars grow on mount with the same capped stagger as the other Trends bars.
- Change `web/src/routes/trends/+page.svelte` so a `dataVersion` refresh keeps the previous data on screen and only the initial load shows skeletons. Today it sets `data = null` on every refresh, which unmounts the `{#key range}` block and replays every entrance. The `{#key range}` stays, so a range change still replays the entrance.

### 5.4 Today UI

#### Composer

- Add a "Repeat a meal" button to the composer header. It must be `type="button"` because it sits inside the estimate form.
- It opens `web/src/lib/today/RepeatSheet.svelte`.
- The sheet title names the target: "Repeat onto Today" or "Repeat onto Mon 2 Nov". The subtitle says "Recent meals, last year." It fetches `GET /api/meals/history?limit=50`.
- Search filters the 50 most recent distinct groups by label and the latest meal's item names. It is a recent-meals picker, not a full archive search. Say so in the subtitle.
- Each row is a button showing label, meal type, kcal and macros, last date, and count.
- Each row's accessible name includes the full label, kcal, last date and count. Truncation is visual only.
- Loading, error with retry, no-history, and no-search-results states. The no-results state says "No recent meals match ..." and offers Clear search.
- On pick, store the picked meal and the current search query in composer state. Close the sheet with the exit lifecycle from §4.3. After the close callback, open Review with a repeat draft for the selected day.
- If Review is cancelled, reopen the sheet with the same query and list. Do not make the user search again. Track `repeatSaved` in Composer: `Review.onSaved` sets it, and `Review.onClose` reopens the sheet only when it is false.
- On a past day, the composer repeat button targets that past day, and the title says so.
- Do not reuse the estimate/manual `saved()` callback. A repeat save only calls `onSaved()` and must not clear the composer description or photo.

#### Per-meal action

- On today, the expanded meal action says "Repeat" and opens Review for today.
- On a past day, it says "Copy to today" and opens Review for today.
- Both replace the old immediate "Eat this again" save.
- The composer sheet remains the way to repeat onto a past day.
- Refactor `MealList` from `editing: Draft | null` to a `review: { draft: Draft; title: string } | null` so edit and repeat share one Review instance.
- `MealList` compares its `date` prop with `today()` to choose the label and target date.
- Compute the target date when the button is clicked, not at render, so a tab left open across midnight still targets the right day.
- After a past-day copy, the status says "Copied to today." and offers a "View today" button that sets `day.date = today()`. Refreshing the past day alone is not enough feedback.

#### Repeat draft

Add to `web/src/lib/today/meal.ts`:

```ts
export function repeatDraft(m: Meal, date: string): Draft
```

Start from `draftFromMeal(m)`, which already divides stored items by `scale` and maps them to `DraftItem` with `editable`, `phrase`, `swapped`, `remember`, `options` and `chosen`. Then:

- Replace the meal id with `uuidv7()`.
- Replace every item id with `uuidv7()`. Reusing source item ids is rejected by the server as an item owned by another meal.
- Set `date` to the target.
- Keep `meal_type`, `description` and `scale`.
- Set `source: 'repeat'`, `provider_id: null`.
- Clear `assumptions` and `confidence`.
- Clear `warnings`.
- Keep each item's `food_id` and `origin`.

Do not call `scaleItems` directly on `m.items` and treat the result as a `Draft`. It preserves ids and returns `MealItem[]`, which is the wrong shape.

Add `repeatTitle(date: string, today: string): string`:

- "Repeat meal for Today" when the target is today.
- "Repeat meal for Mon 2 Nov" otherwise, using `dayLabel`.

Replace `repeatMeal` and its test with `repeatDraft` and its test. The old function built a payload for an immediate save and is no longer used.

Add `filterHistory(meals: HistoryMeal[], query: string): HistoryMeal[]` to `meal.ts` for the sheet's search, so the filter is unit-tested without the component. It matches `label` and the latest meal's item names, case-insensitively.

### 5.5 Review

- No new date field. The title carries the target day.
- Review still saves through `mealUpsert` and `sendOps`.
- A queued write counts as saved, as today.
- Stale writes keep the existing notice and refetch behavior.

### 5.6 Offline

- Add `/api/meals/history` to `isCacheable` in `web/src/lib/offline/cache.ts`. The URL has only `limit`, so it is one stable entry per user.
- `getMealHistory` uses `cachedGet`.
- The sheet shows the cached list when offline.
- A repeat write goes through the existing queue and identity guard.
- On an applied meal upsert or delete, invalidate the cached `/api/meals/history` body and the cached `/api/trends` bodies for that user. Today `writeThrough` in `web/src/lib/offline/queue.ts` patches only `/api/meals?date=` and invalidates trends only for `day.patch`. Add a queue-write test for both.
- The history list reflects synced meals. A meal logged while offline appears after it syncs. The current day's meal list still overlays queued meals through the existing path.
- No new offline write path.

### 5.7 Error handling

- History fetch failure: notice in the sheet with "Try again". The sheet stays open.
- Empty history: "No meals in the last year. Log one and it will show up here."
- No search results: "No recent meals match ..." with Clear search.
- Offline with no cache: the existing offline read message.
- Review save failure: existing behavior.
- Invalid limit: server returns a JSON `{"error": ...}` through `error::Query`.

## 6. Edge cases

- Empty description: fall back to item names. If there are no items, skip the meal.
- Empty item names: skip them when building the fallback label.
- Meals with no items: skipped from counts and history.
- Same label, different items: merge the count. The latest meal is the one repeated.
- Same label at different meal types: merge the count. The latest type is shown.
- Deleted meals: excluded from counts and history.
- Long labels: truncate visually, keep the full label in the accessible name.
- Meals with many items: the history response is capped at 50 groups, so the payload stays bounded.
- Same meal already logged on the target day: allowed. Review is the confirmation.
- Multiplier: preserved through `scale`.
- Timezone: Trends uses the client's local range. History uses `time::user_today` on the server, so its window follows the profile timezone.
- A history window with no meals: empty card and empty sheet.
- Older cached Trends bodies may lack `top_meals`. Hide the card when it is missing. Do not show "No meals logged" for a value that was never loaded.
- Order ties deterministically: top meals by `times`, then `last_date`, then `label`; history by `last_date`, then `times`, then `label`.
- The history window is one year. A meal older than that is not in the picker. The empty state says "No meals in the last year" so it is not mistaken for "never logged."

## 7. Testing

### Backend

- Unit tests for label normalization and grouping:
  - case and whitespace collapse
  - empty description fallback
  - meals with no items are skipped
  - latest representative by date, then `updated_at`
  - average kcal
- Integration tests for `GET /api/meals/history`:
  - counts and average
  - one-year window from `time::user_today`
  - deleted meals excluded
  - limit and invalid limit
  - empty description fallback
  - grouping happens before the limit: more meals than the limit share one label
  - deterministic ordering
- Trends test for `top_meals` ordering, empty range, and grouping before the top-five limit.
- Behavioral isolation in `server/tests/isolation/users.rs`: user B's history and `top_meals` are empty, user A's are unchanged.
- Anonymous isolation request includes `?limit=50`.
- `.sqlx` metadata refreshed.

### Web

- `repeatDraft` test: new meal and item ids, target date, source, cleared assumptions, preserved scale, item count, and `mealFromDraft(repeatDraft(...))` reproduces the original totals.
- `repeatTitle` test.
- `filterHistory` test: label match, item-name match, case-insensitive, empty query.
- Trends card test: missing `top_meals` hides the card; an empty array shows the empty state.
- Offline queue-write test: meal upsert/delete invalidates cached history and trends.
- Cache allowlist test: `/api/meals/history?limit=50` is cacheable, while auth and provider routes are not.
- Repeat sheet no-results state: covered by Playwright, because the repo has no component-test harness and adding one is out of scope.
- Composer draft preservation after a repeat save: covered by Playwright for the same reason.
- Playwright:
  - Log a meal on yesterday, select today, open Repeat a meal, pick the meal, Review shows the same macros, save, meal appears.
  - On a past day, "Copy to today" opens Review for today.
  - Cancel Review from a repeat and the repeat sheet returns with the same search.
  - Trends "Eaten most" shows the count after two logs.

### Motion

- Manual check at 7, 30, 90 and 365 days.
- Sheet open and close on phone and desktop.
- Activity week bar change.
- `prefers-reduced-motion` shows the final state immediately.
- No animation on data refresh.
- A meal write does not replay the Trends entrance.

## 8. Docs and rollout

- Update `docs/design.md`:
  - Motion section with the new hierarchy, tokens and timings.
  - Amend the old "There are no entrance animations" and "popping consistency cells" lines, and the 12 ms stagger note, so the doc matches the new plan.
  - "Eaten most" card.
  - Repeat sheet and per-meal action.
- Update `docs/decisions.md`:
  - History is the meal library, no saved-meal table.
  - Grouping by normalized description.
  - Repeat targets the selected day. No future scheduling.
  - Motion refresh.
- Update `AGENTS.md` and `CLAUDE.md` layout with `web/src/lib/today/RepeatSheet.svelte` and `web/src/lib/trends/FrequentMeals.svelte`.
- Update `docs/offline.md` with the history cache entry and the meal-write invalidation rule.
- Phase 0 lands first so the new sheet and card inherit the new motion.
- Phase 1 lands the API and tests.
- Phase 2 lands the Trends card.
- Phase 3 lands the repeat sheet and per-meal action.
- Phase 4 lands docs and e2e.

## 9. Risks and trade-offs

- **Description grouping merges different meals.** Accepted. The latest meal is repeated, and the count is a rough frequency signal, not an exact recipe count.
- **History payload size.** Fifty groups with full items is bounded. If a user has many 100-item meals, the response grows. The limit and one-year window keep it acceptable for a self-hosted app.
- **Recent-meals search is not a full archive.** The sheet searches the 50 most recent distinct groups in the last year. A meal outside that set is not reachable. This is stated in the sheet, and a full archive search is a later feature.
- **Cache invalidation touches existing write-through.** Meal upserts and deletes must invalidate cached history and trends. This is a small change to `queue.ts` but it affects existing offline behavior, so it needs its own test.
- **Motion changes affect the whole app.** Feedback stays at 160 ms, so controls still feel responsive. Movement gets slower. Review on a real phone before shipping.
- **`@starting-style` support.** The supported baseline is Chrome 117+, Safari 17.5+ and Firefox 129+. Older browsers open sheets instantly. Functionality is unchanged.
- **Sheet exit lifecycle.** Every sheet needs a `closing` state so the exit can play before unmount. This is the riskiest motion change because it touches Review and BarcodeEntry, not just the new sheet.

## 10. Acceptance criteria

- Trends shows the top five meals by count for the selected range.
- The repeat sheet lists recent distinct meals, supports search, and opens Review with the copied items and macros.
- The per-meal action opens Review, targets today on a past day, and never saves immediately.
- A repeated meal keeps its multiplier and item values, and gets new ids.
- The repeat flow works offline through the existing queue.
- Meal writes invalidate cached history and trends.
- The motion tokens and timings from Phase 0 are applied across the app.
- Sheets open and close with the new lifecycle. Reduced motion skips the delay.
- Reduced motion turns all of it off.
- Backend, Vitest, svelte-check, build and Playwright are green.
