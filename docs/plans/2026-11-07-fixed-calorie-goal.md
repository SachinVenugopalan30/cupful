# Fixed Daily Calorie Goal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an optional whole-number daily calorie goal to the profile. A set goal is the day's budget every day. An empty goal keeps the current automatic budget.

**Architecture:** The goal is a nullable column on `profile_versions`, so it follows the existing effective-date history. `budget::compute` branches once: automatic mode keeps the current math, manual mode uses the goal and exposes maintenance plus a signed gap. The server stays the only place that calculates budgets.

**Tech Stack:** Rust, Axum, SQLx/SQLite, SvelteKit 5, TypeScript, Vitest, Playwright.

---

## Task 1: Budget model, migration and fixtures

**Files:**
- Create: `server/migrations/20261107000000_calorie_goal.sql`
- Modify: `server/src/budget.rs`
- Modify: `fixtures/budget_cases.json`
- Test: `server/src/budget.rs` (the existing fixture harness)

**Step 1: Add the failing fixture cases**

Add `"manual": false, "gap": null` to the `expect` object of the first case, "male formula, steps and weight training".

Append these cases to `fixtures/budget_cases.json`. Keep the file valid JSON.

```json
{
  "name": "manual goal with steps and gym",
  "date": "2026-03-10",
  "versions": [
    {
      "effective_from": "2025-01-01",
      "sex": "male",
      "birth_year": 1990,
      "height_cm": 180,
      "start_weight_kg": 85,
      "goal_kg": 75,
      "pace_kg_per_week": 0.5,
      "typical_steps": 8000,
      "calorie_goal": 1800
    }
  ],
  "weight_kg": 80,
  "day": { "steps": 10000, "gym_minutes": 60, "trained": ["Chest", "Arms"] },
  "expect": {
    "manual": true,
    "deficit": 0,
    "gap": 962,
    "floor": 1500,
    "budget": 1800,
    "protein_g": 120.0,
    "fat_g": 50.0,
    "carbs_g": 217.5,
    "flags": []
  }
},
{
  "name": "manual goal on a rest day",
  "date": "2026-03-10",
  "versions": [
    {
      "effective_from": "2025-01-01",
      "sex": "male",
      "birth_year": 1990,
      "height_cm": 180,
      "start_weight_kg": 85,
      "goal_kg": 75,
      "pace_kg_per_week": 0.5,
      "typical_steps": 8000,
      "calorie_goal": 1800
    }
  ],
  "weight_kg": 80,
  "day": { "steps": 0 },
  "expect": { "manual": true, "gap": 300, "budget": 1800, "deficit": 0, "flags": [] }
},
{
  "name": "manual goal below the female floor",
  "date": "2026-03-10",
  "versions": [
    {
      "effective_from": "2025-01-01",
      "sex": "female",
      "birth_year": 1985,
      "height_cm": 165,
      "start_weight_kg": 68,
      "goal_kg": 60,
      "pace_kg_per_week": 0.25,
      "typical_steps": 7000,
      "calorie_goal": 1000
    }
  ],
  "weight_kg": 65,
  "day": { "steps": 6000 },
  "expect": {
    "manual": true,
    "gap": 708,
    "floor": 1200,
    "budget": 1000,
    "deficit": 0,
    "fat_g": 27.8,
    "carbs_g": 91.5,
    "flags": ["below_floor"]
  }
},
{
  "name": "manual goal of 500 over-commits the macros",
  "date": "2026-03-10",
  "versions": [
    {
      "effective_from": "2025-01-01",
      "sex": "male",
      "birth_year": 1990,
      "height_cm": 180,
      "start_weight_kg": 85,
      "goal_kg": 80,
      "pace_kg_per_week": 0.5,
      "typical_steps": 8000,
      "calorie_goal": 500
    }
  ],
  "weight_kg": 80,
  "day": { "steps": 10000, "gym_minutes": 60, "trained": ["Chest", "Arms"] },
  "expect": {
    "manual": true,
    "gap": 2262,
    "budget": 500,
    "deficit": 0,
    "protein_g": 128.0,
    "fat_g": 0.0,
    "carbs_g": 0.0,
    "flags": ["below_floor", "macros_overcommitted"]
  }
},
{
  "name": "manual goal above maintenance",
  "date": "2026-03-10",
  "versions": [
    {
      "effective_from": "2025-01-01",
      "sex": "male",
      "birth_year": 1990,
      "height_cm": 180,
      "start_weight_kg": 85,
      "goal_kg": 75,
      "pace_kg_per_week": 0.5,
      "typical_steps": 8000,
      "calorie_goal": 3000
    }
  ],
  "weight_kg": 80,
  "day": { "steps": 10000, "gym_minutes": 60, "trained": ["Chest", "Arms"] },
  "expect": { "manual": true, "gap": -238, "budget": 3000, "deficit": 0, "flags": [] }
},
{
  "name": "manual goal with no steps keeps the goal and estimates maintenance",
  "date": "2026-03-10",
  "versions": [
    {
      "effective_from": "2025-01-01",
      "sex": "male",
      "birth_year": 1990,
      "height_cm": 180,
      "start_weight_kg": 85,
      "goal_kg": 75,
      "pace_kg_per_week": 0.5,
      "typical_steps": 8000,
      "calorie_goal": 1800
    }
  ],
  "weight_kg": 80,
  "day": {},
  "expect": { "manual": true, "gap": 551, "budget": 1800, "deficit": 0, "flags": ["maintenance_provisional"] }
}
```

**Step 2: Run the budget tests to verify they fail**

```sh
cd server
SQLX_OFFLINE=true cargo test --lib budget
```

Expected: FAIL because the manual cases produce the automatic budget and the new fields are missing.

**Step 3: Add the migration**

Create `server/migrations/20261107000000_calorie_goal.sql`:

```sql
ALTER TABLE profile_versions ADD COLUMN calorie_goal INTEGER;
```

**Step 4: Add the profile field**

In `server/src/budget.rs`, add to `ProfileVersion` after `typical_steps`:

```rust
    #[serde(default)]
    pub calorie_goal: Option<i64>,
```

**Step 5: Add the budget fields**

In `Budget`, after `deficit`:

```rust
    pub manual: bool,
    #[serde(serialize_with = "opt_kcal")]
    pub gap: Option<f64>,
```

Add below `kcal`:

```rust
fn opt_kcal<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(v) => s.serialize_i64(v.round() as i64),
        None => s.serialize_none(),
    }
}
```

**Step 6: Branch `compute`**

Set `manual` before the steps fallback. Replace the steps closure:

```rust
    let manual = v.calorie_goal.is_some();
    let steps = day.steps.unwrap_or_else(|| {
        flags.push(if manual {
            "maintenance_provisional"
        } else {
            "provisional"
        });
        v.typical_steps
    });
```

Move the floor calculation up, then replace the deficit/floor/budget block with:

```rust
    let floor = if male { 1500.0 } else { 1200.0 };
    let (deficit, budget, gap) = if let Some(goal) = v.calorie_goal.map(|g| g as f64) {
        if goal < floor {
            flags.push("below_floor");
        }
        (0.0, goal, Some(maintenance - goal))
    } else {
        let mut deficit = v.pace_kg_per_week * 7700.0 / 7.0;
        if deficit > 0.25 * maintenance {
            deficit = 0.25 * maintenance;
            flags.push("deficit_capped");
        }
        if floor > maintenance - deficit {
            flags.push("floor_applied");
        }
        if floor > maintenance {
            flags.push("floor_above_maintenance");
        }
        (deficit, maintenance.min(floor.max(maintenance - deficit)), None)
    };
```

Add `manual` and `gap` to the returned `Budget`.

**Step 7: Run the budget tests to verify they pass**

```sh
cd server
SQLX_OFFLINE=true cargo test --lib budget
```

Expected: PASS.

**Step 8: Commit**

```sh
git add server/migrations/20261107000000_calorie_goal.sql server/src/budget.rs fixtures/budget_cases.json
git commit -s -m "Add a fixed calorie goal to the budget"
```

---

## Task 2: Profile API

**Files:**
- Modify: `server/src/profile.rs`
- Modify: `server/tests/common/mod.rs` (add `calorie_goal: null` to `profile_body`)
- Modify: `server/tests/profile.rs`
- Refresh: `.sqlx/`

**Step 1: Write the failing profile tests**

Add to `server/tests/profile.rs`:

```rust
use jiff::ToSpan;

#[tokio::test]
async fn calorie_goal_round_trips_and_clears() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let mut b = profile_body();
    b["calorie_goal"] = json!(1800);
    let (status, body) = a.put("/api/profile", b.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, p) = a.get("/api/profile").await;
    assert_eq!(p["version"]["calorie_goal"], 1800);

    b["calorie_goal"] = json!(null);
    a.put("/api/profile", b).await;
    let (_, p) = a.get("/api/profile").await;
    assert!(p["version"]["calorie_goal"].is_null());
}

#[tokio::test]
async fn calorie_goal_is_validated() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for bad in [json!(499), json!(10001)] {
        let mut b = profile_body();
        b["calorie_goal"] = bad;
        let (status, body) = a.put("/api/profile", b).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let mut b = profile_body();
    b["calorie_goal"] = json!(1800.5);
    let (status, body) = a.put("/api/profile", b).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

#[tokio::test]
async fn a_past_version_keeps_its_calorie_goal() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let yesterday = (today("Asia/Kolkata") - 1.days()).to_string();
    sqlx::query(
        "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
             start_weight_kg, goal_kg, pace_kg_per_week, typical_steps, calorie_goal)
         VALUES (1, ?, 'male', 1990, 180.0, 85.0, 75.0, 0.5, 8000, 1600)",
    )
    .bind(&yesterday)
    .execute(&state.db)
    .await
    .unwrap();

    let mut b = profile_body();
    b["calorie_goal"] = json!(1800);
    a.put("/api/profile", b).await;

    let (status, budget) = a.get(&format!("/api/budget/{yesterday}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(budget["manual"], true);
    assert_eq!(budget["budget"], 1600);
}
```

Add `"calorie_goal": null` to `profile_body()` in `server/tests/common/mod.rs`.

Add `("calorie_goal", json!(499))` and `("calorie_goal", json!(10001))` to the existing `profile_is_validated` table.

**Step 2: Run the tests to verify they fail**

```sh
cd server
mkdir -p ../.cupful
export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
sqlx database create
sqlx migrate run
SQLX_OFFLINE=true cargo test --test profile
```

Expected: FAIL because `ProfileIn` has no field and the select does not return it.

**Step 3: Implement the API**

In `server/src/profile.rs`:

- Add `calorie_goal: Option<i64>` to `ProfileIn`.
- Add the range check:

```rust
    check(
        p.calorie_goal.is_none_or(|g| (500..=10_000).contains(&g)),
        "calorie_goal must be 500-10000",
    )?;
```

- Add `calorie_goal` to the `versions` select and mapping.
- Add `calorie_goal` to the insert and the `ON CONFLICT` update.

The insert becomes:

```sql
INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
    start_weight_kg, goal_kg, pace_kg_per_week, typical_steps, calorie_goal)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT (user_id, effective_from) DO UPDATE SET sex = excluded.sex,
    birth_year = excluded.birth_year, height_cm = excluded.height_cm,
    start_weight_kg = excluded.start_weight_kg, goal_kg = excluded.goal_kg,
    pace_kg_per_week = excluded.pace_kg_per_week, typical_steps = excluded.typical_steps,
    calorie_goal = excluded.calorie_goal
```

Bind `p.calorie_goal` last.

**Step 4: Refresh SQLx metadata**

```sh
cd server
export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
cargo sqlx prepare -- --all-targets
```

**Step 5: Run the tests to verify they pass**

```sh
cd server
SQLX_OFFLINE=true cargo test --test profile
SQLX_OFFLINE=true cargo test --lib budget
```

Expected: PASS.

**Step 6: Commit**

```sh
git add server/src/profile.rs server/tests/common/mod.rs server/tests/profile.rs .sqlx
git commit -s -m "Save and validate the profile calorie goal"
```

---

## Task 3: Backup and restore

**Files:**
- Modify: `server/src/transfer/backup/doc.rs`
- Modify: `server/src/transfer/backup/export.rs`
- Modify: `server/src/transfer/backup/restore.rs`
- Modify: `server/src/transfer/backup/validate.rs`
- Modify: `server/tests/backup/main.rs`
- Modify: `server/tests/backup/export.rs`
- Modify: `server/tests/backup/restore.rs`
- Modify: `server/tests/backup/validate.rs`
- Refresh: `.sqlx/`

**Step 1: Write the failing backup tests**

In `server/tests/backup/main.rs`, change the older version insert in `rich_data` to include `calorie_goal`:

```sql
INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
    start_weight_kg, goal_kg, pace_kg_per_week, typical_steps, calorie_goal)
VALUES (?, ?, 'female', 1992, 165.0, 72.0, 65.0, 0.25, 6000, 1600)
```

In `server/tests/backup/export.rs`, after the `profile_versions` length check:

```rust
    assert_eq!(doc["profile_versions"][0]["calorie_goal"], 1600);
    assert!(doc["profile_versions"][1]["calorie_goal"].is_null());
```

In `server/tests/backup/validate.rs`, add to the mutation table:

```rust
        (
            "calorie goal",
            Box::new(|d: &mut Value| d["profile_versions"][0]["calorie_goal"] = json!(499)),
        ),
```

In `server/tests/backup/restore.rs`, add a test:

```rust
#[tokio::test]
async fn an_older_document_without_the_goal_restores_it_as_null() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
    let mut doc = export(&mut a).await;
    for version in doc["profile_versions"].as_array_mut().unwrap() {
        version.as_object_mut().unwrap().remove("calorie_goal");
    }

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let got = export(&mut a).await;
    for version in got["profile_versions"].as_array().unwrap() {
        assert!(version["calorie_goal"].is_null());
    }
}
```

Also assert the restored version keeps 1600 in `restore_replaces_prior_logs_and_profile_versions`:

```rust
    assert_eq!(got["profile_versions"][0]["calorie_goal"], 1600);
```

**Step 2: Run the backup tests to verify they fail**

```sh
cd server
SQLX_OFFLINE=true cargo test --test backup
```

Expected: FAIL because the backup document does not carry the field.

**Step 3: Implement the field**

- `doc.rs`: `Version` gets `#[serde(default)] pub calorie_goal: Option<i64>`.
- `export.rs`: select `calorie_goal` in the version query.
- `restore.rs`: insert `calorie_goal`.
- `validate.rs`:

```rust
        if let Some(g) = v.calorie_goal {
            check((500..=10_000).contains(&g), "calorie_goal must be 500-10000")?;
        }
```

**Step 4: Refresh SQLx metadata**

```sh
cd server
export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
cargo sqlx prepare -- --all-targets
```

**Step 5: Run the tests to verify they pass**

```sh
cd server
SQLX_OFFLINE=true cargo test --test backup
```

Expected: PASS.

**Step 6: Commit**

```sh
git add server/src/transfer/backup server/tests/backup .sqlx
git commit -s -m "Carry the calorie goal through backups"
```

---

## Task 4: Web API types

**Files:**
- Modify: `web/src/lib/api/types.ts`
- Modify: `web/src/lib/today/budget.test.ts` (fixture only)

**Step 1: Update the types**

In `ProfileVersion`, add `calorie_goal: number | null` after `typical_steps`.

In `BudgetFlag`, add `'below_floor'` and `'maintenance_provisional'`.

In `Budget`, add after `deficit`:

```ts
	manual: boolean;
	gap: number | null;
```

**Step 2: Update the test fixture**

Add `manual: false, gap: null` to the `budget` object in `web/src/lib/today/budget.test.ts`.

**Step 3: Run svelte-check**

```sh
cd web
npm run check
```

Expected: 0 errors. UI code that reads the new fields is added in Task 5.

**Step 4: Commit**

```sh
git add web/src/lib/api/types.ts web/src/lib/today/budget.test.ts
git commit -s -m "Type the manual calorie goal"
```

---

## Task 5: Budget helpers and UI

**Files:**
- Modify: `web/src/lib/today/budget.ts`
- Modify: `web/src/lib/today/budget.test.ts`
- Modify: `web/src/lib/today/BudgetCard.svelte`
- Modify: `web/src/lib/activity/ActivityBudget.svelte`
- Modify: `web/src/lib/activity/ActivitySection.svelte`

**Step 1: Write the failing helper tests**

In `budget.test.ts`, replace the flag test with one that passes a budget and add manual breakdown tests:

```ts
describe('flagMessages', () => {
	it('has plain words for every flag, including the floor number', () => {
		const all: BudgetFlag[] = [
			'provisional',
			'below_floor',
			'maintenance_provisional',
			'deficit_capped',
			'floor_applied',
			'floor_above_maintenance',
			'macros_overcommitted'
		];
		const msgs = flagMessages({ ...budget, flags: all });
		expect(msgs).toHaveLength(7);
		expect(msgs[0]).toMatch(/no steps logged yet/i);
		expect(msgs[1]).toMatch(/1,500 kcal/);
		expect(msgs[2]).toMatch(/maintenance and the gap are estimated/i);
		for (const m of msgs) expect(m.length).toBeGreaterThan(20);
	});
});

describe('manual breakdown', () => {
	it('shows the goal and a signed gap instead of a deficit', () => {
		const manual = { ...budget, manual: true, budget: 1800, gap: 420, flags: [] };
		const terms = breakdown(manual);
		const labels = terms.map((t) => t.label);
		expect(labels).toContain('Your goal');
		expect(labels).toContain('Gap to maintenance');
		expect(labels).not.toContain('Deficit');
		expect(terms.find((t) => t.label === 'Gap to maintenance')!.value).toBe(
			'420 kcal under maintenance'
		);
	});

	it('marks the gap as estimated when maintenance is provisional', () => {
		const manual = {
			...budget,
			manual: true,
			budget: 1800,
			gap: 420,
			flags: ['maintenance_provisional'] as BudgetFlag[]
		};
		const gap = breakdown(manual).find((t) => t.label === 'Gap to maintenance')!;
		expect(gap.value).toBe('~420 kcal under maintenance');
	});

	it('says over maintenance when the goal is above it', () => {
		const manual = { ...budget, manual: true, budget: 3000, gap: -238, flags: [] };
		const gap = breakdown(manual).find((t) => t.label === 'Gap to maintenance')!;
		expect(gap.value).toBe('238 kcal over maintenance');
	});
});
```

**Step 2: Run the tests to verify they fail**

```sh
cd web
npm test -- src/lib/today/budget.test.ts
```

Expected: FAIL because `flagMessages` takes flags and the breakdown has no manual branch.

**Step 3: Implement the helpers**

In `budget.ts`:

```ts
import { formatKcal, formatWeight, type WeightUnit } from '../format';

type FlagMessage = (b: Budget) => string;

const FLAGS: Record<BudgetFlag, FlagMessage> = {
	provisional: () =>
		'No steps logged yet — using your typical steps. The budget updates when you add them.',
	below_floor: (b) =>
		`Your goal is below ${formatKcal(b.floor)} kcal, the minimum the automatic budget uses.`,
	maintenance_provisional: () =>
		'No steps logged yet, so maintenance and the gap are estimated from your typical steps. Your goal is unaffected.',
	deficit_capped: () => '…existing text…',
	floor_applied: () => '…existing text…',
	floor_above_maintenance: () => '…existing text…',
	macros_overcommitted: () => '…existing text…'
};

export const flagMessages = (b: Budget) => b.flags.map((f) => FLAGS[f](b));
```

Keep the existing messages for the other four flags.

In `breakdown`, before the automatic array:

```ts
	if (b.manual) {
		const gap = b.gap ?? 0;
		const amount = num(Math.abs(gap));
		const value = gap > 0 ? `${amount} kcal under maintenance` : gap < 0 ? `${amount} kcal over maintenance` : 'at maintenance';
		const estimated = b.flags.includes('maintenance_provisional');
		return [
			{ label: 'Weight used', value: formatWeight(b.weight_kg, unit), note: 'Your latest weigh-in on or before this day, else your start weight.' },
			{ label: 'Age', value: String(b.age), note: 'This year minus your birth year (can be a year off).' },
			{ label: 'BMR', value: `${num(b.bmr)} kcal`, note: 'Energy at rest, from the Mifflin–St Jeor formula.' },
			{ label: 'Base', value: `${num(b.base)} kcal`, note: 'BMR × 1.2, for everyday light movement.' },
			{ label: 'Steps used', value: num(b.steps), note: "This day's steps, or your typical steps if none are logged." },
			{ label: 'Steps kcal', value: `${num(b.steps_kcal)} kcal`, note: 'Extra burn from steps beyond the first 2,500.' },
			{ label: 'MET', value: String(b.met), note: 'Workout intensity: 5 for weight training, 7 for cardio only.' },
			{ label: 'Gym kcal', value: `${num(b.gym_kcal)} kcal`, note: 'Extra burn from gym minutes at that intensity.' },
			{ label: 'Maintenance', value: `${num(b.maintenance)} kcal`, note: 'Base + steps + gym: what keeps your weight steady.' },
			{ label: 'Your goal', value: `${num(b.budget)} kcal`, note: 'The daily goal you set. Activity does not change it.' },
			{ label: 'Floor', value: `${num(b.floor)} kcal`, note: 'The minimum the automatic budget uses. Your goal can go below it.' },
			{
				label: 'Gap to maintenance',
				value: estimated && gap !== 0 ? `~${value}` : value,
				note: estimated
					? 'Estimated from your typical steps. Positive is under maintenance, negative is over.'
					: 'Maintenance minus your goal. Positive is under maintenance, negative is over.'
			}
		];
	}
```

**Step 4: Wire the card**

In `BudgetCard.svelte`:

- Replace the plan paragraph:

```svelte
<p class="plan small muted num">
	{formatKcal(s.budget.budget)} kcal {s.budget.manual ? 'goal' : 'plan'}
</p>
```

- Change `flagMessages(b.flags)` to `flagMessages(b)`.

**Step 5: Wire Activity**

In `ActivityBudget.svelte`, replace the existing `{#if budget} ... {/if}` block with:

```svelte
{#if budget}
	{#if budget.manual}
		<p class="added card" data-testid="activity-budget">
			<span class="lead">Burned beyond base</span>
			<span class="terms">
				<span>Steps <span class="num">+{formatKcal(budget.steps_kcal)}</span> kcal</span>
				<span>Gym <span class="num">+{formatKcal(budget.gym_kcal)}</span> kcal</span>
				<span>Goal <span class="num strong">{formatKcal(budget.budget)}</span> kcal</span>
			</span>
		</p>
	{:else}
		<p class="added card" data-testid="activity-budget">
			<span class="lead">Added to this day's budget</span>
			<span class="terms">
				<span>Steps <span class="num">+{formatKcal(budget.steps_kcal)}</span> kcal</span>
				<span>Gym <span class="num">+{formatKcal(budget.gym_kcal)}</span> kcal</span>
				<span>= <span class="num strong">{formatKcal(budget.budget)}</span> kcal budget</span>
			</span>
		</p>
	{/if}
{/if}
```

In `ActivitySection.svelte`, change the caption at line 406 to:

```svelte
Used from this day until your next weigh-in.
```

**Step 6: Run the tests and checks**

```sh
cd web
npm test -- src/lib/today/budget.test.ts
npm run check
```

Expected: PASS and 0 errors.

**Step 7: Commit**

```sh
git add web/src/lib/today/budget.ts web/src/lib/today/budget.test.ts web/src/lib/today/BudgetCard.svelte web/src/lib/activity/ActivityBudget.svelte web/src/lib/activity/ActivitySection.svelte
git commit -s -m "Show a manual goal in the budget and Activity"
```

---

## Task 6: Profile form

**Files:**
- Modify: `web/src/lib/settings/profile.ts`
- Modify: `web/src/lib/settings/profile.test.ts`
- Modify: `web/src/lib/settings/ProfileSection.svelte`

**Step 1: Write the failing form tests**

Add `calorie_goal: 1800` to `saved.version` in `profile.test.ts`, then add:

```ts
	it('keeps the calorie goal optional, whole and in range', () => {
		const f = toForm(saved, 'UTC');
		expect(f.calorie_goal).toBe(1800);
		expect(fromForm(f, 2026).value!.calorie_goal).toBe(1800);

		const blank = fromForm({ ...f, calorie_goal: null }, 2026);
		expect(blank.errors).toEqual({});
		expect(blank.value!.calorie_goal).toBeNull();

		for (const bad of [1800.5, 499, 10001]) {
			expect(fromForm({ ...f, calorie_goal: bad }, 2026).errors.calorie_goal).toMatch(
				/whole number from 500 to 10,000/
			);
		}
	});
```

Update the existing `toMatchObject` assertions to include `calorie_goal: 1800`.

**Step 2: Run the tests to verify they fail**

```sh
cd web
npm test -- src/lib/settings/profile.test.ts
```

Expected: FAIL because the form has no field.

**Step 3: Implement the form**

In `profile.ts`:

- Add `calorie_goal: number | null` to `ProfileForm`.
- In `toForm`, add `calorie_goal: v?.calorie_goal ?? null`.
- In `fromForm`:

```ts
	if (
		f.calorie_goal != null &&
		(!Number.isInteger(f.calorie_goal) || f.calorie_goal < 500 || f.calorie_goal > 10_000)
	) {
		errors.calorie_goal = 'Enter a whole number from 500 to 10,000 kcal, or leave it blank.';
	}
```

- Add `calorie_goal: f.calorie_goal` to the returned `value`.

In `ProfileSection.svelte`, add after the Pace field:

```svelte
<div class="field">
	<label for="p-calorie-goal">Daily calorie goal (kcal, optional)</label>
	<input id="p-calorie-goal" type="number" inputmode="numeric" min="500" max="10000" step="1"
		bind:value={f.calorie_goal}
		aria-invalid={invalid('calorie_goal')} aria-describedby={describe('calorie_goal', 'p-calorie-goal-hint')} />
	<p id="p-calorie-goal-hint" class="hint">Leave blank to calculate from your profile and activity. A number is used every day.</p>
	{#if errors.calorie_goal}<p id="calorie_goal-err" class="err">{errors.calorie_goal}</p>{/if}
</div>
```

Change the Pace hint to: "How fast to lose weight; 0 to maintain. Ignored while a daily goal is set."

**Step 4: Run the tests and checks**

```sh
cd web
npm test -- src/lib/settings/profile.test.ts
npm run check
```

Expected: PASS and 0 errors.

**Step 5: Commit**

```sh
git add web/src/lib/settings/profile.ts web/src/lib/settings/profile.test.ts web/src/lib/settings/ProfileSection.svelte
git commit -s -m "Add the daily calorie goal field"
```

---

## Task 7: End-to-end coverage

**Files:**
- Modify: `web/e2e/settings.spec.ts`

**Step 1: Extend the profile test**

After the existing Today budget checks in `settings.spec.ts`, and before the Providers section, add:

```ts
	await page.goto('/profile');
	const profilePage = page.getByRole('region', { name: 'Profile and targets' });
	await profilePage.getByLabel('Daily calorie goal (kcal, optional)').fill('1800');
	await profilePage.getByRole('button', { name: 'Save profile' }).click();
	await expect(profilePage.getByRole('status')).toHaveText('Profile saved. Your budget uses it from today.');

	await page.getByRole('link', { name: 'Today' }).click();
	await expect(budget.getByText('1,800 kcal goal')).toBeVisible();
	await budget.getByText("How it's worked out").click();
	await expect(budget.getByText('Your goal')).toBeVisible();
	await expect(budget.getByText('Gap to maintenance')).toBeVisible();
	await expect(budget.getByText(/kcal under maintenance/)).toBeVisible();
	await page.keyboard.press('Escape');

	await page.getByRole('link', { name: 'Activity' }).click();
	await expect(page.getByTestId('activity-budget')).toContainText('Burned beyond base');
	await expect(page.getByTestId('activity-budget')).toContainText('Goal 1,800 kcal');
	await expect(page.getByText('Used from this day until your next weigh-in.')).toBeVisible();

	await page.goto('/profile');
	await profilePage.getByLabel('Daily calorie goal (kcal, optional)').fill('');
	await profilePage.getByRole('button', { name: 'Save profile' }).click();
	await page.getByRole('link', { name: 'Today' }).click();
	await expect(budget.getByText(/kcal plan/)).toBeVisible();
```

**Step 2: Run the test**

```sh
cd web
npm run test:e2e -- e2e/settings.spec.ts
```

Expected: PASS.

**Step 3: Commit**

```sh
git add web/e2e/settings.spec.ts
git commit -s -m "Cover the fixed calorie goal end to end"
```

---

## Task 8: Docs and full validation

**Files:**
- Modify: `docs/decisions.md`
- Modify: `docs/design.md`
- Modify: `docs/backups.md`

**Step 1: Update docs**

In `docs/decisions.md`, add:

```md
- 2026-11-07 - A daily calorie goal is a nullable `calorie_goal` on each profile version; empty means the automatic budget. - The version keeps past days honest when the goal changes.
- 2026-11-07 - A manual goal is accepted from 500 to 10,000 whole kcal, warned below the floor, and never clamped. - It is the user's number; the warning covers the health side.
- 2026-11-07 - Manual mode keeps `deficit` at 0 and puts the signed maintenance gap in a separate `gap` field. - The old field keeps one meaning instead of silently becoming a signed gap.
- 2026-11-07 - An older cached client that saves the profile without `calorie_goal` clears it. - Acceptable on a self-hosted single-user instance; the update prompt prevents it in practice.
```

In `docs/design.md`, replace the compact Activity sentence with:

```md
- A compact card reports what steps and gym added to the budget, from the server's terms. With a fixed goal it says "Burned beyond base" and shows the goal itself.
```

In `docs/backups.md`, add to the profile line:

```md
- the profile (timezone and unit preferences) and every profile version, including the optional daily calorie goal;
```

**Step 2: Run the full backend suite**

```sh
cd server
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

**Step 3: Run the full frontend suite**

```sh
cd web
npm test
npm run check
npm run build
```

Expected: all pass.

**Step 4: Run Playwright**

```sh
cd web
npm run test:e2e
```

Expected: all pass, with the screenshots spec skipped unless `SCREENSHOTS` is set.

**Step 5: Commit**

```sh
git add docs/decisions.md docs/design.md docs/backups.md
git commit -s -m "Document the fixed calorie goal"
```
