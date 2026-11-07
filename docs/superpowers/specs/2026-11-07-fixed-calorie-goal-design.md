# Fixed daily calorie goal

Status: revised after spec review.

## Problem

The daily budget is always derived from the profile, the latest weight, the day's steps and the
day's gym minutes. Some people already know the calorie number they want to eat and do not want the
app to recalculate it every day. They need a manual goal that stays fixed, with the automatic
budget as the fallback when no goal is set.

## Goals

- Add an optional daily calorie goal to the profile.
- When set, the goal is the day's budget on every day, regardless of steps or gym minutes.
- When empty, the current automatic budget is unchanged.
- Keep past days honest: the goal lives on the profile version, so changing it later does not
  rewrite earlier days.
- Show the estimated maintenance and the gap to the goal, so the user can see the real deficit.
- Warn, but do not block, when the goal is below the automatic floor.

## Non-goals

- No activity adjustment in this version. The future "above usual" toggle can build on this field
  without changing it.
- No new route.
- No change to how meals, days or trends are stored.
- No client-side budget math. The server remains the only implementation.

## Decisions

- The goal is stored on `profile_versions` as a nullable integer. `NULL` means automatic.
- The accepted range is `500..=10_000` whole kcal. This only catches typos. The below-floor warning
  handles the health side.
- A manual goal is not clamped to the floor and is not reduced by the pace. It is the user's
  number.
- `deficit` keeps its current meaning: the automatic planned cut. In manual mode it is `0`.
  A separate `gap` field carries `maintenance - goal` in manual mode, and is `null` in automatic
  mode.
- `manual` is explicit rather than inferred from `gap !== null`, so the client does not depend on
  the gap being present.
- The gap and maintenance are estimates when no steps are logged. The goal is not.
- An older cached client that saves the profile without `calorie_goal` clears it. This is
  acceptable on a self-hosted single-user instance and is noted in `docs/decisions.md`.

## Data model

New migration `20261107000000_calorie_goal.sql`:

```sql
ALTER TABLE profile_versions ADD COLUMN calorie_goal INTEGER;
```

- `ProfileVersion.calorie_goal: Option<i64>` with `#[serde(default)]`. Existing fixtures and older
  rows read as `None`.
- `ProfileIn.calorie_goal: Option<i64>`. The server checks `500..=10_000` and answers 400 through
  the existing error type when it is outside the range. A fractional value is a serde type
  mismatch and gets Axum's 422, like `birth_year` and `typical_steps`. The body is still
  `{"error": ...}`.
- All profile version queries include the field:
  - `server/src/profile.rs`: select and insert.
  - `server/src/transfer/backup/export.rs`: select.
  - `server/src/transfer/backup/restore.rs`: insert.
  - `server/src/demo.rs`: the demo insert leaves it `NULL`.
- Saving the profile still writes one version effective from the user's today. A goal change then
  applies from today and leaves earlier versions alone.

## Budget computation

`compute` converts `Option<i64>` to `Option<f64>` only for the math.

Automatic mode (`calorie_goal` is `None`) is value-for-value the current behavior: BMR, base, steps,
gym, maintenance, deficit, cap, floor, macros and flags.

Manual mode (`calorie_goal` is `Some(goal)`):

- `budget = goal`.
- `maintenance`, `bmr`, `base`, `steps_kcal`, `gym_kcal`, `steps` and `met` are computed as today.
- `floor` is still 1,200 for female and 1,500 for male.
- `deficit = 0`.
- `gap = maintenance - budget`. Positive means under maintenance, negative means over.
- Flags:
  - `below_floor` when `budget < floor`.
  - `maintenance_provisional` when steps are missing. `provisional` is not set.
  - `macros_overcommitted` under the existing rule.
  - `deficit_capped`, `floor_applied` and `floor_above_maintenance` are never set.
- Macros use the existing formulas with `budget = goal`:
  - `protein_g = 1.6 * goal_kg.unwrap_or(kg)`
  - `fat_g = min(0.25 * budget, budget - protein_kcal).max(0) / 9`
  - `carbs_g = (budget - protein_kcal - 9 * fat_g).max(0) / 4`
  - At 500 kcal the protein target can exceed the goal. Fat and carbs become 0 and
    `macros_overcommitted` fires. One fixture covers this.

`Budget` gains:

```rust
pub manual: bool,
#[serde(serialize_with = "opt_kcal")]
pub gap: Option<f64>,
```

`opt_kcal` rounds `Some` to an integer and serializes `None` as JSON `null`. The existing `deficit`
serializer is unchanged. In automatic mode `manual` is `false` and `gap` is `null`.

## API

`GET /api/profile` returns `version.calorie_goal` as a number or `null`.

`PUT /api/profile` accepts `calorie_goal` as an integer or `null`. Validation happens with the
other profile checks and reuses the existing 400 error shape.

`GET /api/budget/{date}` carries the new `manual` and `gap` fields through the existing serializer.
`/api/trends` builds its days by hand and only exposes `budget_kcal`, which is already whatever
`compute` returns. A manual goal therefore flows into averages, consistency and the charts without
further changes.

## Frontend

### Profile form

Add a field after Pace:

- Label: "Daily calorie goal (kcal, optional)".
- Input: `type="number"`, `inputmode="numeric"`, `min="500"`, `max="10000"`, `step="1"`.
- Hint: "Leave blank to calculate from your profile and activity. A number is used every day."
- Error: "Enter a whole number from 500 to 10,000 kcal, or leave it blank."
- Pace hint gains: "Ignored while a daily goal is set."
- Goal weight hint stays the same, because it still sets the protein target.

`ProfileForm` gains `calorie_goal: number | null`. `toForm` reads it from the version.
`fromForm` checks `Number.isInteger` and the range before sending.

### Budget card

- Header: "1,800 kcal goal" when `manual`, otherwise the current "1,807 kcal plan".
- "How it's worked out" keeps the maintenance terms and the Floor row. In manual mode it replaces
  the Deficit and Budget rows with:
  - "Your goal": the goal.
  - "Gap to maintenance": `420 kcal under maintenance`, `120 kcal over maintenance` or
    `at maintenance`.
- When `maintenance_provisional` is set, the gap value is prefixed with `~` and its note says it is
  estimated from typical steps.
- New flag messages:
  - `below_floor`: "Your goal is below 1,200 kcal, the minimum the automatic budget uses." The
    number is read from `b.floor`, so `flagMessages` takes the budget rather than the flag list.
  - `maintenance_provisional`: "No steps logged yet, so maintenance and the gap are estimated from
    your typical steps. Your goal is unaffected."
- `FLAGS` stays a `Record<BudgetFlag, string>`, so the new union members force both messages to
  exist. `flagMessages` replaces the below-floor string with the floor number.

### Activity

`ActivityBudget.svelte` changes for manual mode:

- Automatic: unchanged "Added to this day's budget" line.
- Manual: lead "Burned beyond base", then `Steps +X kcal`, `Gym +Y kcal` and `Goal Z kcal`. It does
  not say the steps and gym add to the budget.

The weigh-in caption in `ActivitySection.svelte:406` changes to "Used from this day until your next
weigh-in." in both modes. It is accurate whether the weigh-in changes the budget or only maintenance
and protein.

`docs/design.md` updates the compact-card sentence to describe both modes.

### Types

`web/src/lib/api/types.ts` adds:

- `ProfileVersion.calorie_goal: number | null`. `ProfileIn` inherits it through the existing
  `Omit`.
- `Budget.manual: boolean` and `Budget.gap: number | null`.
- `BudgetFlag` members `below_floor` and `maintenance_provisional`.

### Trends and reads

No code changes. The per-day `budget_kcal` uses the same `compute`, and the averages and charts treat
a manual goal like any other budget.

## Backup and restore

- `Version` in `server/src/transfer/backup/doc.rs` gains
  `#[serde(default)] calorie_goal: Option<i64>`.
- Export selects it; restore inserts it.
- Validation accepts `None` or `500..=10_000`.
- An older backup without the key restores as `None`.
- `docs/backups.md` says each profile version carries the optional daily goal.

## Tests

### Shared budget fixtures

Add cases to `fixtures/budget_cases.json`, which the existing harness checks key by key:

- Manual 1,800 with steps and gym: budget 1,800, manual true, deficit 0, gap 962, no auto flags.
- Manual 1,000 for a female profile: below_floor, gap positive.
- Manual 500: macros_overcommitted, fat 0, carbs 0.
- Manual above maintenance: negative gap, no below_floor.
- Manual with no steps: maintenance_provisional, no provisional, goal unchanged.

Add `"manual": false, "gap": null` to at least one existing automatic case, because the harness
only checks the keys a case lists. The other 16 cases have no `calorie_goal` key and prove the serde
default keeps automatic mode unchanged.

### Backend

- Profile route: save 1,800, read it back; save `null`, read it back; 499 and 10,001 answer 400,
  and 1,800.5 answers 422. All three carry `{"error": ...}`.
- Insert a version for yesterday with one goal using SQL, as `tests/profile.rs:48` does, save today
  with another goal, and confirm `GET /api/budget/{yesterday}` still uses the yesterday goal.
- Backup: new field round-trips; a document without the field restores as `None`.
- No isolation test is needed because no route is added.

### Frontend

- `profile.test.ts`: form shows and saves the goal; blank is `null`; 1,800.5, 499 and 10,001 are
  rejected with the field error.
- `budget.test.ts`: both new flag messages exist; manual breakdown shows goal, floor and the signed
  gap; the gap gets `~` when maintenance is provisional; the automatic breakdown is unchanged.
- `ActivityBudget` behavior is covered by the E2E test.

### E2E

One Settings → Profile test: set a goal, see the goal in the Today header and the gap in "How it's
worked out", then clear it and see the automatic plan return. Reuse the existing session.

## Docs

- `docs/decisions.md`: versioned goal, 500-10,000 range, warn not clamp, separate gap field, older
  cached client caveat.
- `docs/design.md`: budget header, manual breakdown, Activity card wording.
- `docs/backups.md`: the optional field on each profile version.

## Acceptance criteria

- With no goal, every budget value and flag is identical to today's output.
- With 1,800 kcal set, Today shows 1,800 on a rest day, a 10,000-step day and a gym day.
- Past days keep the goal that was effective on them.
- 500 kcal is accepted, shows the below-floor warning, and produces 0 g fat and carbs with
  `macros_overcommitted`.
- 499 and 10,001 are rejected with a 400, and a fractional value with a 422. All carry
  `{"error": ...}`.
- The Activity line and weigh-in caption are correct in manual mode.
- Backup and restore preserve the goal, including old backups without the field.
- Backend, Vitest, svelte-check, build and Playwright are green.
