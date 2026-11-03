# Meal history and motion refresh

Date: 2026-11-03
Status: Draft for review
Scope: Cupful web and API

## 1. Problem

Two gaps in daily use:

1. Trends shows what the range averaged, but not what was eaten most often. The owner wants a count of common meals.
2. Repeating a meal is hidden inside an expanded meal row and only logs onto the day being viewed. Copying yesterday's dinner to today means navigating back to yesterday and using an action that writes to yesterday. The owner wants an easy way to copy a meal and its macros to today or a later day.

The current motion is also tuned for speed over readability. Feedback is quick, but sheets appear instantly, bars pop, and chart entrances feel abrupt. The owner wants motion that is calmer, smoother and more refined without making the app feel slow.

## 2. Goals

- Show the most frequently logged meals for the selected Trends range.
- Make a previously logged meal repeatable onto the selected day in a few taps.
- Always show the copied meal in Review before it is saved, because portions and macros may change.
- Keep logged-data writes on the existing sync path.
- Keep the repeat flow usable offline.
- Refine motion globally, with fast feedback and calmer movement.
- Add no new dependencies and no new saved-meal table.

## 3. Non-goals

- Named saved-meal templates or favourites. History is the library for now.
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
  - Use CSS `@starting-style` with `transition-behavior: allow-discrete` and `overlay` for the native `<dialog>`. If browser support is a concern, use a Svelte transition on the dialog content instead.
  - Reduced motion opens and closes instantly.
- Every explicit animation keeps its `prefers-reduced-motion` override.
- No page transitions. No entrance animations on Today, Activity, Settings, login or setup.

### 4.4 Trends

- Intake and steps bars: 760 ms, 16 ms stagger, capped at 15 bars so the wave adds at most 240 ms.
- Consistency cells: 360 ms, 18 ms stagger, capped at 14 cells. Fade from 0.94 scale and 0 opacity. No pop, no overshoot.
- Macro balance and meal mix bars: 760 ms, 80 ms stagger.
- Headline count-up: 800 ms with the existing ease-out cubic.
- Weight area: 600 ms with a 120 ms delay.
- The weight line keeps no dash animation.

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

- Top five groups by `times` descending, then `last_date` descending.
- `avg_kcal` is rounded on serialize.
- Empty array when the range has no meals.
- No items in this payload.

#### History

Add `GET /api/meals/history?from=YYYY-MM-DD&to=YYYY-MM-DD&limit=20`.

- `from` and `to` are required and must satisfy the existing `check_span` rule, at most 400 days.
- `limit` defaults to 20 and must be between 1 and 50.
- Sort by `last_date` descending, then `times` descending.
- Response:

```json
{
  "meals": [
    {
      "id": "019...",
      "date": "2026-11-02",
      "meal_type": "breakfast",
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

Both endpoints use the same grouping helper. Extract a shared `meal_json` helper used by `reads::meals` and `reads::meal_history` so the meal serialization stays in one place.

Add the client types and reads:

- `TopMeal` and `HistoryMeal` in `web/src/lib/api/types.ts`. `Trends` gains `top_meals?: TopMeal[]`, optional because an older cached body may not have it.
- `getMealHistory(from, to, limit)` in `web/src/lib/api/reads.ts`, using `cachedGet`.

#### Route and storage

- Add `("GET", "/api/meals/history", User, get(reads::meal_history))` to `route_table()`.
- Add the route to `server/tests/isolation/routes.rs`.
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
- Display only. No logging action from Trends.
- Empty state: "No meals logged in this range."
- Render after `Analytics` and before the footnote on the Trends page.
- Use the motion tokens from Phase 0. The bars grow on mount with the same capped stagger as the other Trends bars.

### 5.4 Today UI

#### Composer

- Add a "Repeat a meal" button to the composer header.
- It opens `web/src/lib/today/RepeatSheet.svelte`.
- The sheet fetches history for the last 365 days, limit 20.
- Search filters by label and item names.
- Each row is a button showing label, meal type, kcal and macros, last date, and count.
- Loading, error with retry, and empty states.
- On pick, close the sheet and open the existing Review sheet with a repeat draft for the selected day.
- Close the repeat sheet before opening Review so only one modal dialog is open. Keep the picked meal in composer state and open Review from the sheet's close handler.

#### Per-meal action

- On today, the expanded meal action says "Repeat" and opens Review for today.
- On a past day, it says "Copy to today" and opens Review for today.
- Both replace the old immediate "Eat this again" save.
- The composer sheet remains the way to repeat onto a past day.
- Refactor `MealList` from `editing: Draft | null` to a `review: { draft: Draft; title: string } | null` so edit and repeat share one Review instance.
- `MealList` compares its `date` prop with `today()` to choose the label and target date.

#### Repeat draft

Add to `web/src/lib/today/meal.ts`:

```ts
export function repeatDraft(m: Meal, date: string): Draft
```

- New meal id and new item ids.
- Target `date`.
- Keep `meal_type`, `description` and `scale`.
- Set `source: 'repeat'`, `provider_id: null`.
- Clear `assumptions` and `confidence`.
- Clear `warnings`.
- Items are `scaleItems(m.items, 1 / m.scale)`, so Review shows the base items and reapplies the multiplier on save. This mirrors `draftFromMeal`.
- Keep each item's `food_id` and `origin`.

Add `repeatTitle(date: string, today: string): string`:

- "Repeat meal for today" when the target is today.
- "Repeat meal for 2 Nov" otherwise, using `dayLabel`.

Replace `repeatMeal` and its test with `repeatDraft` and its test. The old function built a payload for an immediate save and is no longer used.

Add `filterHistory(meals: HistoryMeal[], query: string): HistoryMeal[]` to `meal.ts` for the sheet's search, so the filter is unit-tested without the component.

### 5.5 Review

- No new date field. The title carries the target day.
- Review still saves through `mealUpsert` and `sendOps`.
- A queued write counts as saved, as today.
- Stale writes keep the existing notice and refetch behavior.

### 5.6 Offline

- Add `/api/meals/history` with a query string to `isCacheable` in `web/src/lib/offline/cache.ts`.
- `getMealHistory` uses `cachedGet`.
- The sheet shows the cached list when offline.
- A repeat write goes through the existing queue and identity guard.
- The history list reflects synced meals. A meal logged while offline appears after it syncs. The current day's meal list still overlays queued meals through the existing path.
- No new offline write path.

### 5.7 Error handling

- History fetch failure: notice in the sheet with "Try again". The sheet stays open.
- Empty history: "No past meals yet. Log one and it will show up here."
- Offline with no cache: the existing offline read message.
- Review save failure: existing behavior.
- Invalid range or limit: server returns a JSON `{"error": ...}` through `error::Query`.

## 6. Edge cases

- Empty description: fall back to item names. If there are no items, skip the meal.
- Meals with no items: skipped from counts and history.
- Same label, different items: merge the count. The latest meal is the one repeated.
- Same label at different meal types: merge the count. The latest type is shown.
- Deleted meals: excluded from counts and history.
- Long labels: truncate visually, keep the full label in the title attribute.
- Meals with many items: the history response is capped at 20 groups, so the payload stays bounded.
- Same meal already logged on the target day: allowed. Review is the confirmation.
- Multiplier: preserved through `scale`.
- Timezone: the client computes `from` and `to` from the user's local today.
- A history range with no meals: empty card and empty sheet.
- Older cached Trends bodies may lack `top_meals`. The card treats a missing value as an empty array.

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
  - range filtering
  - deleted meals excluded
  - limit and invalid limit
  - invalid span
  - empty description fallback
- Trends test for `top_meals` ordering and empty range.
- Isolation test for the new route.
- `.sqlx` metadata refreshed.

### Web

- `repeatDraft` test: new ids, target date, source, cleared assumptions, preserved scale, item count.
- `repeatTitle` test.
- `filterHistory` test: label match, item-name match, case-insensitive, empty query.
- Trends card test: missing `top_meals` renders the empty state.
- Playwright:
  - Log a meal, move to the next day, open Repeat a meal, pick the meal, Review shows the same macros, save, meal appears.
  - On a past day, "Copy to today" opens Review for today.
  - Trends "Eaten most" shows the count after two logs.

### Motion

- Manual check at 7, 30, 90 and 365 days.
- Sheet open and close on phone and desktop.
- Activity week bar change.
- `prefers-reduced-motion` shows the final state immediately.
- No animation on data refresh.

## 8. Docs and rollout

- Update `docs/design.md`:
  - Motion section with the new hierarchy, tokens and timings.
  - "Eaten most" card.
  - Repeat sheet and per-meal action.
- Update `docs/decisions.md`:
  - History is the meal library, no saved-meal table.
  - Grouping by normalized description.
  - Motion refresh.
- Update `AGENTS.md` layout with `web/src/lib/today/RepeatSheet.svelte` and `web/src/lib/trends/FrequentMeals.svelte`.
- Update `docs/offline.md` if the cache allowlist description needs the history endpoint.
- Phase 0 lands first so the new sheet and card inherit the new motion.
- Phase 1 lands the API and tests.
- Phase 2 lands the Trends card.
- Phase 3 lands the repeat sheet and per-meal action.
- Phase 4 lands docs and e2e.

## 9. Risks and trade-offs

- **Description grouping merges different meals.** Accepted. The latest meal is repeated, and the count is a rough frequency signal, not an exact recipe count.
- **History payload size.** Twenty groups with full items is bounded. If a user has many 100-item meals, the response grows. The limit and 365-day range keep it acceptable for a self-hosted app.
- **Motion changes affect the whole app.** Feedback stays at 160 ms, so controls still feel responsive. Movement gets slower. Review on a real phone before shipping.
- **`@starting-style` support.** Modern browsers support it. A Svelte transition is the fallback if needed.

## 10. Acceptance criteria

- Trends shows the top five meals by count for the selected range.
- The repeat sheet lists recent distinct meals, supports search, and opens Review with the copied items and macros.
- The per-meal action opens Review, targets today on a past day, and never saves immediately.
- A repeated meal keeps its multiplier and item values, and gets new ids.
- The repeat flow works offline through the existing queue.
- The motion tokens and timings from Phase 0 are applied across the app.
- Reduced motion turns all of it off.
- Backend, Vitest, svelte-check, build and Playwright are green.
