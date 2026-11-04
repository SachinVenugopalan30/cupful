# Meal History and Motion Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an "Eaten most" Trends card and a review-first repeat-meal flow, while making the app's motion calmer and more refined.

**Architecture:** Reuse `meals` and `meal_items` as the history library. Group past meals by normalized description on the server, expose a top-five summary on `/api/trends` and a recent-meals list on `/api/meals/history`. The client opens the existing Review sheet with a new draft, so writes stay on `sendOps`. Motion changes are token and component updates, with a shared sheet-close helper so exit animations can play before unmount.

**Tech Stack:** Rust, Axum, SQLx/SQLite, SvelteKit 5, TypeScript, Vitest, Playwright, CSS custom properties.

**Spec:** `docs/superpowers/specs/2026-11-03-meal-history-and-motion-design.md`

---

## Before you start

- Work in a dedicated worktree. Do not mix this with other branches.
- Every source file starts with `SPDX-License-Identifier: AGPL-3.0-or-later`.
- Every commit is DCO signed: `git commit -s`.
- Logged-data writes go only through `sendOps` in `web/src/lib/sync/index.ts`.
- Run backend commands from `server/`, frontend commands from `web/`.
- After any new `sqlx::query!` macro, refresh `.sqlx` with the command in Task 10.

---

# Phase 0: Motion refresh

## Task 1: Motion tokens and global transitions

**Files:**
- Modify: `web/src/lib/styles/tokens.css`
- Modify: `web/src/lib/styles/global.css`
- Modify: `web/src/lib/products/SavedList.svelte`
- Modify: `web/src/lib/today/MealList.svelte`
- Modify: `web/src/lib/offline/UpdateBanner.svelte`
- Modify: `web/src/lib/offline/SyncBar.svelte`
- Modify: `web/src/routes/setup/+page.svelte`
- Modify: `web/src/routes/invite/+page.svelte`
- Modify: `web/src/routes/login/+page.svelte`

**Step 1: Update the motion tokens**

In `tokens.css`, replace the motion block with:

```css
/* ---- motion ---- */
--dur-fast: 160ms;   /* hover, press, focus, chips */
--dur-med: 240ms;    /* saved text, small state changes */
--dur-slow: 360ms;   /* meters, week bars, progress fills */
--dur-enter: 420ms;  /* sheets */
--dur-exit: 200ms;   /* sheet close, backdrop fade */
--ease: cubic-bezier(0.22, 1, 0.36, 1);
--ease-state: cubic-bezier(0.4, 0, 0.2, 1);
```

In the reduced-motion block, add the new durations:

```css
@media (prefers-reduced-motion: reduce) {
	:root {
		--dur-fast: 0ms;
		--dur-med: 0ms;
		--dur-slow: 0ms;
		--dur-enter: 0ms;
		--dur-exit: 0ms;
	}
}
```

**Step 2: Update global transitions**

In `global.css`, change control transitions from `--ease` to `--ease-state`. The `.btn`, link, and chip transitions should read:

```css
transition:
	background var(--dur-fast) var(--ease-state),
	border-color var(--dur-fast) var(--ease-state),
	color var(--dur-fast) var(--ease-state),
	transform var(--dur-fast) var(--ease-state);
```

Change the meter fill:

```css
transition: width var(--dur-slow) var(--ease);
```

Change the skeleton pulse:

```css
.skeleton {
	animation: pulse 1.8s ease-in-out infinite;
}
@keyframes pulse {
	50% {
		opacity: 0.65;
	}
}
```

Then update the scoped small-control transitions that still use `--ease`:

- `web/src/lib/products/SavedList.svelte`
- `web/src/lib/today/MealList.svelte`
- `web/src/lib/offline/UpdateBanner.svelte`
- `web/src/lib/offline/SyncBar.svelte`
- `web/src/routes/setup/+page.svelte`
- `web/src/routes/invite/+page.svelte`
- `web/src/routes/login/+page.svelte`

Use `--ease-state` for background, border-color, color and transform. Keep `--ease` for movement such as meter fills.

**Step 3: Run the frontend checks**

Run:

```sh
cd web
npm run check
npm run build
```

Expected: both pass. No test asserts CSS tokens; the motion acceptance is checked in Tasks 2, 3, 4 and 18.

**Step 4: Commit**

```sh
git add web/src/lib/styles/tokens.css web/src/lib/styles/global.css \
  web/src/lib/products/SavedList.svelte web/src/lib/today/MealList.svelte \
  web/src/lib/offline/UpdateBanner.svelte web/src/lib/offline/SyncBar.svelte \
  web/src/routes/setup/+page.svelte web/src/routes/invite/+page.svelte \
  web/src/routes/login/+page.svelte
git commit -s -m "Calm the motion tokens and global transitions"
```

---

## Task 2: Shared sheet close lifecycle

**Files:**
- Create: `web/src/lib/sheet.ts`
- Create: `web/src/lib/sheet.test.ts`
- Modify: `web/src/lib/styles/global.css`
- Modify: `web/src/lib/today/Review.svelte`
- Modify: `web/src/lib/products/BarcodeEntry.svelte`

**Step 1: Write the failing test**

Create `web/src/lib/sheet.test.ts`:

```ts
// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest';
import { closeSheet } from './sheet.ts';

function fakeDialog() {
	const listeners = new Map<string, EventListener>();
	const dialog = {
		dataset: {} as Record<string, string>,
		close: vi.fn(),
		addEventListener: vi.fn((type: string, fn: EventListener) => listeners.set(type, fn)),
		removeEventListener: vi.fn((type: string) => listeners.delete(type)),
		dispatch: (type: string, event: Event) => listeners.get(type)?.(event)
	};
	return dialog;
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('closeSheet', () => {
	it('closes immediately under reduced motion', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: true }));
		const dialog = fakeDialog();
		const done = vi.fn();

		closeSheet(dialog as unknown as HTMLDialogElement, done);

		expect(dialog.close).toHaveBeenCalledOnce();
		expect(done).toHaveBeenCalledOnce();
	});

	it('waits for the exit transition before closing', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: false }));
		const dialog = fakeDialog();
		const done = vi.fn();

		closeSheet(dialog as unknown as HTMLDialogElement, done);

		expect(dialog.dataset.closing).toBe('true');
		expect(dialog.close).not.toHaveBeenCalled();

		dialog.dispatch('transitionend', { target: dialog } as unknown as Event);

		expect(dialog.close).toHaveBeenCalledOnce();
		expect(done).toHaveBeenCalledOnce();
	});
});
```

**Step 2: Run the test to verify it fails**

```sh
cd web
npm test -- src/lib/sheet.test.ts
```

Expected: FAIL because `./sheet.ts` does not exist.

**Step 3: Implement the helper**

Create `web/src/lib/sheet.ts`:

```ts
// SPDX-License-Identifier: AGPL-3.0-or-later
// Close a native <dialog> after its exit transition. The dialog stays open while
// [data-closing] runs, then closes and calls the parent's unmount callback.

const EXIT_FALLBACK_MS = 400;

export function closeSheet(dialog: HTMLDialogElement, onClosed: () => void): void {
	const reduced =
		typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
	if (reduced) {
		dialog.close();
		onClosed();
		return;
	}
	if (dialog.dataset.closing === 'true') return;
	dialog.dataset.closing = 'true';

	let done = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const finish = () => {
		if (done) return;
		done = true;
		dialog.removeEventListener('transitionend', onEnd);
		if (timer !== undefined) clearTimeout(timer);
		delete dialog.dataset.closing;
		dialog.close();
		onClosed();
	};
	const onEnd = (event: TransitionEvent) => {
		if (event.target === dialog) finish();
	};
	dialog.addEventListener('transitionend', onEnd);
	timer = setTimeout(finish, EXIT_FALLBACK_MS);
}
```

**Step 4: Run the test to verify it passes**

```sh
cd web
npm test -- src/lib/sheet.test.ts
```

Expected: PASS.

**Step 5: Add the sheet CSS**

In `global.css`, after the `dialog.sheet` rule, add:

```css
dialog.sheet {
	opacity: 1;
	transform: none;
	transition:
		opacity var(--dur-exit) var(--ease-state),
		transform var(--dur-exit) var(--ease-state);
}
dialog.sheet[open] {
	transition-duration: var(--dur-enter);
}
@starting-style {
	dialog.sheet[open] {
		opacity: 0;
		transform: translateY(16px) scale(0.985);
	}
}
dialog.sheet[data-closing] {
	opacity: 0;
	transform: translateY(16px) scale(0.985);
	transition-duration: var(--dur-exit);
}
dialog.sheet::backdrop {
	opacity: 0;
	transition: opacity var(--dur-exit) var(--ease-state);
}
dialog.sheet[open]::backdrop {
	opacity: 1;
	transition-duration: var(--dur-enter);
}
@starting-style {
	dialog.sheet[open]::backdrop {
		opacity: 0;
	}
}
dialog.sheet[data-closing]::backdrop {
	opacity: 0;
}
```

**Step 6: Wire Review to the helper**

In `Review.svelte`:

- Import `closeSheet`:

```ts
import { closeSheet } from '#lib/sheet.ts';
```

- Remove `onclose={onClose}` from the `<dialog>`.
- Add `oncancel`:

```svelte
<dialog
	class="sheet review"
	bind:this={dialog}
	oncancel={(event) => {
		event.preventDefault();
		closeSheet(dialog, onClose);
	}}
	aria-labelledby="review-title-{uid}"
>
```

- Replace the Cancel button handler:

```svelte
<button type="button" class="btn btn-quiet" onclick={() => closeSheet(dialog, onClose)}>Cancel</button>
```

- Replace both `dialog.close()` calls in `save()` with:

```ts
closeSheet(dialog, onClose);
```

**Step 7: Wire BarcodeEntry to the helper**

In `BarcodeEntry.svelte`:

- Import `closeSheet`.
- Remove `onclose={() => (view = null)}` from the `<dialog>`.
- Add:

```svelte
oncancel={(event) => {
	event.preventDefault();
	if (dialog) closeSheet(dialog, () => (view = null));
}}
```

- Replace the close button handler:

```svelte
onclick={() => dialog && closeSheet(dialog, () => (view = null))}
```

- Replace the `dialog?.close()` in `added()`:

```ts
function added() {
	const name = view?.name === 'product' ? view.product.name : '';
	if (dialog) closeSheet(dialog, () => (view = null));
	announce = `Added ${name}.`;
	onSaved();
}
```

**Step 8: Run the frontend checks**

```sh
cd web
npm test
npm run check
npm run build
```

Expected: all pass.

**Step 9: Commit**

```sh
git add web/src/lib/sheet.ts web/src/lib/sheet.test.ts web/src/lib/styles/global.css web/src/lib/today/Review.svelte web/src/lib/products/BarcodeEntry.svelte
git commit -s -m "Animate sheet exits before unmounting"
```

---

## Task 3: Trends motion timings

**Files:**
- Modify: `web/src/lib/trends/Charts.svelte`
- Modify: `web/src/lib/trends/Consistency.svelte`
- Modify: `web/src/lib/trends/Analytics.svelte`
- Modify: `web/src/lib/trends/CountUp.svelte`
- Modify: `web/src/lib/trends/count-up.ts`

**Step 1: Update the bar charts**

In `Charts.svelte`, change both `style:--i={Math.min(i, 40)}` occurrences to:

```svelte
style:--i={Math.min(i, 14)}
```

Change the bar animation:

```css
.bars-svg .bar {
	animation: grow-y 760ms var(--ease) both;
	animation-delay: calc(var(--i) * 16ms);
}
```

Change the weight area:

```css
.weight-svg .area {
	animation: fade-in 600ms var(--ease) 120ms both;
}
```

**Step 2: Update the consistency cells**

In `Consistency.svelte`, change:

```svelte
style:--i={Math.min(i, 13)}
```

Change the cell animation and keyframes:

```css
.grid i {
	animation: fade-in 360ms var(--ease) both;
	animation-delay: calc(var(--i) * 18ms);
}
@keyframes fade-in {
	from {
		opacity: 0;
		transform: scale(0.94);
	}
	to {
		opacity: 1;
		transform: none;
	}
}
```

Remove the old `@keyframes pop`.

**Step 3: Update the analytics bars**

In `Analytics.svelte`, change both `animation: grow-x 620ms var(--ease) both;` lines to:

```css
animation: grow-x 760ms var(--ease) both;
```

Change both `animation-delay: calc(var(--i) * 60ms);` lines to:

```css
animation-delay: calc(var(--i) * 80ms);
```

**Step 4: Update the count-up**

In `CountUp.svelte`, change the duration:

```ts
const progress = Math.min(1, (now - start) / 800);
```

Update the header comment from 700 ms to 800 ms.

In `count-up.ts`, update the comment from 700 ms to 800 ms.

**Step 5: Run the frontend checks**

```sh
cd web
npm test
npm run check
npm run build
```

Expected: all pass.

**Step 6: Commit**

```sh
git add web/src/lib/trends/Charts.svelte web/src/lib/trends/Consistency.svelte web/src/lib/trends/Analytics.svelte web/src/lib/trends/CountUp.svelte web/src/lib/trends/count-up.ts
git commit -s -m "Slow the Trends entrance into a calmer pass"
```

---

## Task 4: Activity motion timings

**Files:**
- Modify: `web/src/lib/activity/ActivityWeek.svelte`
- Modify: `web/src/lib/activity/ActivitySection.svelte`

**Step 1: Update the week bars**

In `ActivityWeek.svelte`, change:

```css
.col {
	transition: background-color var(--dur-fast) var(--ease-state);
}
.bar {
	transition: height var(--dur-slow) var(--ease);
}
```

**Step 2: Update the saved text**

In `ActivitySection.svelte`, change:

```css
.saved-text {
	animation: saved-in var(--dur-med) var(--ease);
}
```

**Step 3: Run the frontend checks**

```sh
cd web
npm test
npm run check
```

Expected: all pass.

**Step 4: Commit**

```sh
git add web/src/lib/activity/ActivityWeek.svelte web/src/lib/activity/ActivitySection.svelte
git commit -s -m "Match Activity motion to the calmer timing"
```

---

## Task 5: Keep Trends data during a refresh

**Files:**
- Modify: `web/src/routes/trends/+page.svelte`

**Step 1: Add the loaded-range guard**

Replace the first `$effect` in `web/src/routes/trends/+page.svelte` with:

```ts
let loadedRange: Range | null = null;

$effect(() => {
	saveRange(range);
	void dataVersion.n;
	void retry;
	const { from, to } = rangeDates(range, today());
	let stale = false;
	if (loadedRange !== range) {
		data = null;
		loadedRange = range;
	}
	error = '';
	getTrends(from, to).then(
		(r) => !stale && (data = r),
		(e) => !stale && (error = errorMessage(e))
	);
	return () => (stale = true);
});
```

This keeps the previous `data` on a `dataVersion` refresh. A range change still clears it and replays the entrance through `{#key range}`.

**Step 2: Run the frontend checks**

```sh
cd web
npm run check
npm run build
```

Expected: both pass.

**Step 3: Manual motion check**

Open `/trends`, let the entrance finish, then log a meal in another tab. The Trends content should stay on screen and should not replay its entrance. A range change should still replay it.

**Step 4: Commit**

```sh
git add web/src/routes/trends/+page.svelte
git commit -s -m "Keep Trends content during a data refresh"
```

---

# Phase 1: Backend history and top meals

## Task 6: Meal grouping helpers with unit tests

**Files:**
- Modify: `server/src/reads.rs`

**Step 1: Write the failing unit tests**

At the bottom of `server/src/reads.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, kcal: f64) -> ItemRow {
        ItemRow {
            id: format!("i-{name}-{kcal}"),
            name: name.into(),
            portion_text: "1 serving".into(),
            grams: Some(100.0),
            kcal,
            protein_g: 1.0,
            carbs_g: 1.0,
            fat_g: 1.0,
            food_id: None,
            origin: "manual".into(),
        }
    }

    fn meal(
        id: &str,
        date: &str,
        updated_at: i64,
        description: &str,
        items: Vec<ItemRow>,
    ) -> MealRow {
        MealRow {
            id: id.into(),
            date: date.into(),
            meal_type: "lunch".into(),
            description: description.into(),
            scale: 1.0,
            assumptions: None,
            confidence: None,
            source: "manual".into(),
            provider_id: None,
            updated_at,
            items,
        }
    }

    #[test]
    fn labels_fall_back_to_item_names() {
        let m = meal("m1", "2026-11-01", 1, "  ", vec![item("Rice", 200.0), item("Chicken", 300.0)]);
        assert_eq!(meal_label(&m), "Rice, Chicken");
    }

    #[test]
    fn keys_collapse_case_and_whitespace() {
        assert_eq!(meal_key("  Oats   With  Banana "), "oats with banana");
    }

    #[test]
    fn groups_by_normalized_description_and_averages_kcal() {
        let meals = vec![
            meal("m1", "2026-11-01", 1, "Oats with banana", vec![item("Oats", 400.0)]),
            meal("m2", "2026-11-02", 2, "oats with banana", vec![item("Oats", 500.0)]),
            meal("m3", "2026-11-02", 3, "Chicken rice", vec![item("Rice", 600.0)]),
        ];
        let mut groups = group_meals(meals);
        groups.sort_by(|a, b| a.label.cmp(&b.label));
        assert_eq!(groups.len(), 2);
        let oats = groups.iter().find(|g| g.label == "Oats with banana").unwrap();
        assert_eq!(oats.times, 2);
        assert_eq!(oats.avg_kcal, 450.0);
        assert_eq!(oats.last_date, "2026-11-02");
        assert_eq!(oats.latest.id, "m2");
    }

    #[test]
    fn latest_representative_uses_updated_at() {
        let meals = vec![
            meal("m1", "2026-11-02", 10, "Rice", vec![item("Rice", 100.0)]),
            meal("m2", "2026-11-02", 20, "Rice", vec![item("Rice", 200.0)]),
        ];
        let groups = group_meals(meals);
        assert_eq!(groups[0].latest.id, "m2");
    }

    #[test]
    fn skips_itemless_meals() {
        let meals = vec![meal("m1", "2026-11-02", 1, "Rice", vec![])];
        assert!(group_meals(meals).is_empty());
    }
}
```

**Step 2: Run the tests to verify they fail**

```sh
cd server
SQLX_OFFLINE=true cargo test --lib reads::tests
```

Expected: FAIL because `ItemRow`, `MealRow`, `meal_label`, `meal_key` and `group_meals` do not exist.

**Step 3: Add the structs and helpers**

Near the top of `server/src/reads.rs`, after the imports, add:

```rust
use std::collections::hash_map::Entry;

struct ItemRow {
    id: String,
    name: String,
    portion_text: String,
    grams: Option<f64>,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    food_id: Option<i64>,
    origin: String,
}

struct MealRow {
    id: String,
    date: String,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    provider_id: Option<i64>,
    updated_at: i64,
    items: Vec<ItemRow>,
}

struct MealGroup {
    label: String,
    times: usize,
    avg_kcal: f64,
    last_date: String,
    latest: MealRow,
}

fn meal_label(m: &MealRow) -> String {
    let description = m.description.trim();
    if !description.is_empty() {
        return description.to_string();
    }
    m.items
        .iter()
        .map(|i| i.name.trim())
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn meal_key(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn group_meals(meals: Vec<MealRow>) -> Vec<MealGroup> {
    let mut groups: HashMap<String, MealGroup> = HashMap::new();
    for meal in meals {
        if meal.items.is_empty() {
            continue;
        }
        let label = meal_label(&meal);
        if label.is_empty() {
            continue;
        }
        let key = meal_key(&label);
        let kcal = meal.items.iter().map(|i| i.kcal).sum::<f64>();
        match groups.entry(key) {
            Entry::Occupied(mut e) => {
                let g = e.get_mut();
                g.times += 1;
                g.avg_kcal += kcal;
                let newer = (meal.date.as_str(), meal.updated_at, meal.id.as_str())
                    > (g.last_date.as_str(), g.latest.updated_at, g.latest.id.as_str());
                if newer {
                    g.last_date = meal.date.clone();
                    g.latest = meal;
                }
            }
            Entry::Vacant(e) => {
                e.insert(MealGroup {
                    label,
                    times: 1,
                    avg_kcal: kcal,
                    last_date: meal.date.clone(),
                    latest: meal,
                });
            }
        }
    }
    let mut out: Vec<_> = groups.into_values().collect();
    for group in &mut out {
        group.avg_kcal /= group.times as f64;
    }
    out
}
```

**Step 4: Run the tests to verify they pass**

```sh
cd server
SQLX_OFFLINE=true cargo test --lib reads::tests
```

Expected: PASS.

**Step 5: Commit**

```sh
git add server/src/reads.rs
git commit -s -m "Group meal history by normalized description"
```

---

## Task 7: Load meals once and reuse the serializer

**Files:**
- Modify: `server/src/reads.rs`

**Step 1: Add `load_meals` and `meal_json`**

Add below `group_meals`:

```rust
async fn load_meals(
    db: &SqlitePool,
    user_id: i64,
    from: &str,
    to: &str,
) -> Result<Vec<MealRow>> {
    let meals = sqlx::query!(
        "SELECT id, date, meal_type, description, scale, assumptions, confidence, source,
                provider_id, updated_at
         FROM meals
         WHERE user_id = ? AND date BETWEEN ? AND ? AND deleted_at IS NULL
         ORDER BY date, updated_at, id",
        user_id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let items = sqlx::query!(
        "SELECT i.meal_id, i.id, i.name, i.portion_text, i.grams, i.kcal, i.protein_g,
                i.carbs_g, i.fat_g, i.food_id, i.origin
         FROM meal_items i JOIN meals m ON m.id = i.meal_id
         WHERE m.user_id = ? AND m.date BETWEEN ? AND ? AND m.deleted_at IS NULL
         ORDER BY i.meal_id, i.position",
        user_id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let mut by_meal: HashMap<String, Vec<ItemRow>> = HashMap::new();
    for i in items {
        by_meal.entry(i.meal_id.clone()).or_default().push(ItemRow {
            id: i.id,
            name: i.name,
            portion_text: i.portion_text,
            grams: i.grams,
            kcal: i.kcal,
            protein_g: i.protein_g,
            carbs_g: i.carbs_g,
            fat_g: i.fat_g,
            food_id: i.food_id,
            origin: i.origin,
        });
    }
    Ok(meals
        .into_iter()
        .map(|m| MealRow {
            id: m.id,
            date: m.date,
            meal_type: m.meal_type,
            description: m.description,
            scale: m.scale,
            assumptions: m.assumptions,
            confidence: m.confidence,
            source: m.source,
            provider_id: m.provider_id,
            updated_at: m.updated_at,
            items: by_meal.remove(&m.id).unwrap_or_default(),
        })
        .collect())
}

fn meal_json(m: &MealRow) -> Value {
    let sum = |f: fn(&ItemRow) -> f64| m.items.iter().map(f).sum::<f64>();
    json!({
        "id": m.id,
        "date": m.date,
        "meal_type": m.meal_type,
        "description": m.description,
        "scale": m.scale,
        "assumptions": m.assumptions,
        "confidence": m.confidence,
        "source": m.source,
        "provider_id": m.provider_id,
        "updated_at": m.updated_at,
        "items": m.items.iter().map(|i| json!({
            "id": i.id,
            "name": i.name,
            "portion_text": i.portion_text,
            "grams": i.grams,
            "kcal": i.kcal,
            "protein_g": i.protein_g,
            "carbs_g": i.carbs_g,
            "fat_g": i.fat_g,
            "food_id": i.food_id,
            "origin": i.origin,
        })).collect::<Vec<_>>(),
        "totals": {
            "kcal": sum(|i| i.kcal),
            "protein_g": sum(|i| i.protein_g),
            "carbs_g": sum(|i| i.carbs_g),
            "fat_g": sum(|i| i.fat_g),
        },
    })
}
```

**Step 2: Refactor `reads::meals`**

Replace the body of `pub async fn meals` with:

```rust
pub async fn meals(
    State(s): State<AppState>,
    user: User,
    Query(q): Query<DateQuery>,
) -> Result<Json<Value>> {
    let date = parse_date(&q.date)?.to_string();
    let mut meals = load_meals(&s.db, user.id, &date, &date).await?;
    meals.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(json!({
        "meals": meals.iter().map(meal_json).collect::<Vec<_>>(),
    })))
}
```

**Step 3: Run the backend tests**

The new `load_meals` queries need SQLx metadata before the offline test run:

```sh
cd server
mkdir -p ../.cupful
export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
sqlx database create
sqlx migrate run
cargo sqlx prepare -- --all-targets
```

Then run:

```sh
cd server
SQLX_OFFLINE=true cargo test --test sync
SQLX_OFFLINE=true cargo test --test reads
```

Expected: all pass. The existing meal shape is unchanged.

**Step 4: Commit**

```sh
git add server/src/reads.rs
git commit -s -m "Load meals through one shared serializer"
```

---

## Task 8: Add the history endpoint

**Files:**
- Modify: `server/src/reads.rs`
- Modify: `server/src/lib.rs`
- Create: `server/tests/history.rs`

**Step 1: Write the failing integration tests**

Create `server/tests/history.rs`:

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use jiff::ToSpan;
use serde_json::json;

const TZ: &str = "Asia/Kolkata";

fn date(days_ago: i64) -> String {
    (today(TZ) - days_ago.days()).to_string()
}

fn meal_with(id: &str, date: &str, description: &str, kcal: f64) -> serde_json::Value {
    let mut m = meal(id, date, json!([item(&uuid(), kcal)]));
    m["description"] = json!(description);
    m
}

#[tokio::test]
async fn groups_by_description_and_returns_the_latest() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (d1, d2) = (date(2), date(1));
    a.sync(vec![
        op("meal.upsert", &at(-30), meal_with(&uuid(), &d1, "Oats with banana", 400.0)),
        op("meal.upsert", &at(-20), meal_with(&uuid(), &d2, "oats with banana", 500.0)),
        op("meal.upsert", &at(-10), meal_with(&uuid(), &d2, "Chicken rice", 600.0)),
    ])
    .await;

    let (status, body) = a.get("/api/meals/history?limit=50").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let meals = body["meals"].as_array().unwrap();
    assert_eq!(meals.len(), 2);

    let oats = meals.iter().find(|m| m["label"] == "Oats with banana").unwrap();
    assert_eq!(oats["times"], 2);
    assert_eq!(oats["avg_kcal"], 450);
    assert_eq!(oats["date"], d2);
    assert_eq!(oats["items"][0]["kcal"], 500.0);
}

#[tokio::test]
async fn empty_description_falls_back_to_item_names_and_itemless_meals_are_skipped() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    a.sync(vec![
        op("meal.upsert", &at(0), meal_with(&uuid(), &date(1), "", 300.0)),
        op("meal.upsert", &at(0), meal(&uuid(), &date(1), json!([]))),
    ])
    .await;

    let (_, body) = a.get("/api/meals/history?limit=50").await;
    let meals = body["meals"].as_array().unwrap();
    assert_eq!(meals.len(), 1);
    assert_eq!(meals[0]["label"], "rice");
}

#[tokio::test]
async fn groups_before_applying_the_limit_and_excludes_old_or_deleted_meals() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = date(1);
    let id = uuid();
    a.sync(vec![
        op("meal.upsert", &at(-30), meal_with(&uuid(), &d, "Rice", 100.0)),
        op("meal.upsert", &at(-20), meal_with(&uuid(), &d, "rice", 200.0)),
        op("meal.upsert", &at(-10), meal_with(&uuid(), &d, "Rice", 300.0)),
        op("meal.upsert", &at(-10), meal_with(&id, &d, "Old meal", 400.0)),
        op("meal.upsert", &at(-10), meal_with(&uuid(), &date(400), "Too old", 500.0)),
    ])
    .await;
    a.sync(vec![op("meal.delete", &at(0), json!({"id": id}))]).await;

    let (_, body) = a.get("/api/meals/history?limit=1").await;
    let meals = body["meals"].as_array().unwrap();
    assert_eq!(meals.len(), 1);
    assert_eq!(meals[0]["label"], "Rice");
    assert_eq!(meals[0]["times"], 3);
}

#[tokio::test]
async fn limit_is_validated() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for q in ["limit=0", "limit=51", "limit=nope"] {
        let (status, _) = a.get(&format!("/api/meals/history?{q}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{q}");
    }
}
```

**Step 2: Run the tests to verify they fail**

```sh
cd server
SQLX_OFFLINE=true cargo test --test history
```

Expected: FAIL with 404 for `/api/meals/history`.

**Step 3: Add the handler**

In `server/src/reads.rs`, add:

```rust
#[derive(Deserialize)]
pub struct HistoryQuery {
    limit: Option<i64>,
}

pub async fn meal_history(
    State(s): State<AppState>,
    user: User,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>> {
    let limit = q.limit.unwrap_or(20);
    if !(1..=50).contains(&limit) {
        return Err(Error::bad_request("limit must be between 1 and 50"));
    }
    let to = crate::time::user_today(&s.db, user.id).await?;
    let from = to - 364.days();
    let meals = load_meals(&s.db, user.id, &from.to_string(), &to.to_string()).await?;
    let mut groups = group_meals(meals);
    groups.sort_by(|a, b| {
        b.last_date
            .cmp(&a.last_date)
            .then_with(|| b.times.cmp(&a.times))
            .then_with(|| a.label.cmp(&b.label))
    });
    groups.truncate(limit as usize);
    let out: Vec<Value> = groups
        .into_iter()
        .map(|g| {
            let mut v = meal_json(&g.latest);
            v["label"] = json!(g.label);
            v["times"] = json!(g.times);
            v["last_date"] = json!(g.last_date);
            v
        })
        .collect();
    Ok(Json(json!({ "meals": out })))
}
```

**Step 4: Add the route**

In `server/src/lib.rs`, in `routes()`, add next to the meals route:

```rust
("GET", "/api/meals/history", User, get(reads::meal_history)),
```

**Step 5: Run the tests to verify they pass**

```sh
cd server
SQLX_OFFLINE=true cargo test --test history
```

Expected: PASS.

**Step 6: Commit**

```sh
git add server/src/reads.rs server/src/lib.rs server/tests/history.rs
git commit -s -m "Add the recent meal history endpoint"
```

---

## Task 9: Add top meals to Trends

**Files:**
- Modify: `server/src/reads.rs`
- Modify: `server/tests/trends.rs`

**Step 1: Write the failing test**

Add to `server/tests/trends.rs`:

```rust
#[tokio::test]
async fn top_meals_counts_by_description() {
    let mut a = setup().await;
    a.sync(vec![op(
        "meal.upsert",
        &at(-30),
        meal(&uuid(), D3, json!([item(&uuid(), 100.0)])),
    )])
    .await;

    let (_, t) = a.get(&format!("/api/trends?from={D1}&to={D3}")).await;
    let top = t["top_meals"].as_array().unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0]["label"], "rice and chicken");
    assert_eq!(top[0]["times"], 2);
    assert_eq!(top[0]["avg_kcal"], 425);
    assert_eq!(top[0]["last_date"], D3);
}

#[tokio::test]
async fn top_meals_is_empty_without_meals() {
    let mut a = setup().await;
    let (_, t) = a.get(&format!("/api/trends?from={D1}&to={D1}")).await;
    assert_eq!(t["top_meals"], json!([]));
}
```

**Step 2: Run the tests to verify they fail**

```sh
cd server
SQLX_OFFLINE=true cargo test --test trends top_meals
```

Expected: FAIL because `top_meals` is missing.

**Step 3: Add the helper**

In `server/src/reads.rs`, add:

```rust
fn top_meals(meals: Vec<MealRow>, limit: usize) -> Vec<Value> {
    let mut groups = group_meals(meals);
    groups.sort_by(|a, b| {
        b.times
            .cmp(&a.times)
            .then_with(|| b.last_date.cmp(&a.last_date))
            .then_with(|| a.label.cmp(&b.label))
    });
    groups.truncate(limit);
    groups
        .into_iter()
        .map(|g| {
            json!({
                "label": g.label,
                "meal_type": g.latest.meal_type,
                "times": g.times,
                "avg_kcal": g.avg_kcal.round() as i64,
                "last_date": g.last_date,
            })
        })
        .collect()
}
```

**Step 4: Add it to the trends response**

In `pub async fn trends`, after the `meal_mix` block and before the `days` vector, add:

```rust
let history = load_meals(&s.db, user.id, &f, &t).await?;
let top = top_meals(history, 5);
```

Then add `"top_meals": top` to the final `json!`:

```rust
Ok(Json(
    json!({ "days": days, "averages": averages, "meal_mix": meal_mix, "top_meals": top }),
))
```

**Step 5: Run the tests to verify they pass**

```sh
cd server
SQLX_OFFLINE=true cargo test --test trends
```

Expected: PASS.

**Step 6: Commit**

```bash
git add server/src/reads.rs server/tests/trends.rs
git commit -s -m "Add the most-eaten meals to Trends"
```

---

## Task 10: Isolation tests and SQLx metadata

**Files:**
- Modify: `server/tests/isolation/routes.rs`
- Modify: `server/tests/isolation/users.rs`
- Refresh: `.sqlx/`

**Step 1: Add the route to the coverage list**

In `server/tests/isolation/routes.rs`, add:

```rust
"GET /api/meals/history",
```

In the anonymous URI builder, extend the query switch:

```rust
} else if path == "/api/meals/history" {
    "?limit=50"
} else if path == "/api/days" || path == "/api/workouts" || path == "/api/trends" {
```

**Step 2: Add the behavioral test**

In `server/tests/isolation/users.rs`, add:

```rust
use jiff::ToSpan;

#[tokio::test]
async fn b_history_and_top_meals_are_empty() {
    let (mut a, mut b, _, _) = two_users().await;

    let (_, history) = b.get("/api/meals/history?limit=50").await;
    assert_eq!(history["meals"], json!([]));
    let (_, trends) = b.get("/api/trends?from=2026-01-04&to=2026-01-06").await;
    assert_eq!(trends["top_meals"], json!([]));

    // A fresh meal keeps the assertion inside the one-year window as the clock moves.
    let d = (today("Asia/Kolkata") - 1.days()).to_string();
    a.sync(vec![op(
        "meal.upsert",
        &at(0),
        meal(&uuid(), &d, json!([item(&uuid(), 500.0)])),
    )])
    .await;
    let (_, history) = a.get("/api/meals/history?limit=50").await;
    assert!(!history["meals"].as_array().unwrap().is_empty());
}
```

**Step 3: Run the isolation tests**

```sh
cd server
SQLX_OFFLINE=true cargo test --test isolation
```

Expected: PASS.

**Step 4: Refresh the SQLx metadata**

```sh
cd server
mkdir -p ../.cupful
export DATABASE_URL=sqlite://$PWD/../.cupful/dev.db
sqlx database create
sqlx migrate run
cargo sqlx prepare -- --all-targets
```

Expected: `.sqlx/` updates.

**Step 5: Run the full backend suite**

```sh
cd server
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Expected: all pass.

**Step 6: Commit**

```sh
git add server/tests/isolation/routes.rs server/tests/isolation/users.rs .sqlx
git commit -s -m "Cover meal history isolation and refresh SQLx metadata"
```

---

# Phase 2: Trends UI and offline cache

## Task 11: API types and history read

**Files:**
- Modify: `web/src/lib/api/types.ts`
- Modify: `web/src/lib/api/reads.ts`

**Step 1: Add the types**

In `types.ts`, add near the Trends types:

```ts
/** One row in the Trends "Eaten most" card. */
export type TopMeal = {
	label: string;
	meal_type: MealType;
	times: number;
	avg_kcal: number;
	last_date: string;
};
```

Change `Trends`:

```ts
export type Trends = {
	days: TrendDay[];
	averages: TrendAverages | null;
	meal_mix: MealMix | null;
	top_meals?: TopMeal[];
};
```

Near the meal types, add:

```ts
/** One row in the repeat-meal sheet: the latest meal in a history group. */
export type HistoryMeal = Meal & { label: string; times: number; last_date: string };
```

**Step 2: Add the read**

In `reads.ts`, add:

```ts
/** Recent distinct meals, newest first. The server uses the profile timezone. */
export async function getMealHistory(limit = 50): Promise<HistoryMeal[]> {
	const body = await cachedGet<{ meals: HistoryMeal[] }>(`/api/meals/history?limit=${limit}`);
	return body.meals;
}
```

Update the import:

```ts
import type { Budget, Day, Health, HistoryMeal, Meal, Trends, Workout } from './types.ts';
```

**Step 3: Run the frontend checks**

```sh
cd web
npm run check
```

Expected: pass.

**Step 4: Commit**

```sh
git add web/src/lib/api/types.ts web/src/lib/api/reads.ts
git commit -s -m "Add meal history API types and read"
```

---

## Task 12: Offline cache allowlist and invalidation

**Files:**
- Modify: `web/src/lib/offline/cache.ts`
- Modify: `web/src/lib/offline/queue.ts`
- Modify: `web/src/lib/offline/cache.test.ts`
- Modify: `web/src/lib/offline/queue-write.test.ts`

**Step 1: Write the failing tests**

In `cache.test.ts`, inside `allows the read endpoints`, add:

```ts
expect(isCacheable('/api/meals/history?limit=50')).toBe(true);
```

In `queue-write.test.ts`, add a test inside `describe('write-through')`:

```ts
it('invalidates cached history and trends on a meal write', async () => {
	const userId = newUser();
	const date = '2026-10-29';
	const historyPath = '/api/meals/history?limit=50';
	const trendsPath = '/api/trends?from=2026-10-01&to=2026-10-30';
	await boot(userId);
	await putCachedBody(userId, historyPath, { meals: [] });
	await putCachedBody(userId, trendsPath, { days: [], top_meals: [] });

	await enqueue([mealUpsert(mealIn('m1', date, [item('i1', 300)]))], userId);
	serving(userId, applied);
	await flush(userId);

	expect(await getCachedBody(userId, historyPath)).toBeUndefined();
	expect(await getCachedBody(userId, trendsPath)).toBeUndefined();
});
```

**Step 2: Run the tests to verify they fail**

```sh
cd web
npm test -- src/lib/offline/cache.test.ts src/lib/offline/queue-write.test.ts
```

Expected: FAIL because history is not cacheable and the cache entries survive.

**Step 3: Allow the history route**

In `cache.ts`, add before the `/api/meals` check:

```ts
if (route === '/api/meals/history') return true;
```

**Step 4: Invalidate cached history and trends on meal writes**

In `queue.ts`, add:

```ts
async function invalidateMealHistoryAndTrends(userId: number): Promise<void> {
	for (const entry of await allCacheForUser(userId)) {
		if (entry.path.startsWith('/api/meals/history') || entry.path.startsWith('/api/trends')) {
			await deleteCacheEntry(userId, entry.path);
		}
	}
}
```

In `writeThrough`, after `await updateMeals(userId, item);`, add:

```ts
await invalidateMealHistoryAndTrends(userId);
```

**Step 5: Run the tests to verify they pass**

```sh
cd web
npm test -- src/lib/offline/cache.test.ts src/lib/offline/queue-write.test.ts
```

Expected: PASS.

**Step 6: Commit**

```sh
git add web/src/lib/offline/cache.ts web/src/lib/offline/queue.ts web/src/lib/offline/cache.test.ts web/src/lib/offline/queue-write.test.ts
git commit -s -m "Cache and invalidate meal history offline"
```

---

## Task 13: The Eaten most card

**Files:**
- Create: `web/src/lib/trends/FrequentMeals.svelte`
- Modify: `web/src/routes/trends/+page.svelte`
- Modify: `web/e2e/trends.spec.ts`

**Step 1: Write the failing e2e test**

Add to `web/e2e/trends.spec.ts`:

```ts
test('Trends shows the meals eaten most often', async () => {
	const tz = (await (await page.request.get('/api/profile')).json()).timezone as string;
	const today = new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(new Date());
	const yesterday = new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(
		new Date(Date.now() - 864e5)
	);
	const id = () => crypto.randomUUID();
	const meal = (date: string, kcal: number) => ({
		id: id(),
		date,
		meal_type: 'lunch',
		description: 'e2e oats',
		scale: 1,
		assumptions: null,
		confidence: null,
		source: 'manual',
		provider_id: null,
		items: [
			{
				id: id(),
				name: 'oats',
				portion_text: '1 bowl',
				grams: 100,
				kcal,
				protein_g: 5,
				carbs_g: 30,
				fat_g: 3,
				food_id: null,
				origin: 'manual'
			}
		]
	});
	const now = new Date().toISOString();
	const synced = await page.request.post('/api/sync', {
		headers: { 'X-Cupful': '1' },
		data: {
			ops: [
				{ op_id: id(), kind: 'meal.upsert', at: now, payload: meal(yesterday, 300) },
				{ op_id: id(), kind: 'meal.upsert', at: now, payload: meal(today, 300) }
			]
		}
	});
	expect(synced.ok()).toBe(true);

	await page.goto('/trends');
	const card = page.getByRole('region', { name: 'Eaten most' });
	await expect(card).toBeVisible();
	await expect(card).toContainText('e2e oats');
	await expect(card).toContainText('2 times');
});
```

**Step 2: Run the test to verify it fails**

```sh
cd web
npm run test:e2e -- e2e/trends.spec.ts
```

Expected: FAIL because the card does not exist.

**Step 3: Add the component**

Create `web/src/lib/trends/FrequentMeals.svelte`:

```svelte
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  FrequentMeals — "Eaten most" for the selected Trends range. Display only.
  Props: meals: TopMeal[]; days: number.
-->
<script lang="ts">
	import type { TopMeal } from '../api';
	import { formatNumber, shortDate } from './logic.ts';

	let { meals, days }: { meals: TopMeal[]; days: number } = $props();
	const top = $derived(Math.max(1, ...meals.map((m) => m.times)));
</script>

<section class="card frequent" aria-labelledby="frequent-h">
	<div class="head">
		<h2 id="frequent-h">Eaten most</h2>
		<span class="muted small">{days}-day count</span>
	</div>
	{#if !meals.length}
		<p class="empty">No meals logged in this range.</p>
	{:else}
		<ul class="list">
			{#each meals as m, i (m.label)}
				<li>
					<span class="label">{m.label}</span>
					<span class="meta small muted">
						{m.meal_type} · {m.times} times · last {shortDate(m.last_date)}
					</span>
					<span class="num">{formatNumber(m.avg_kcal)} kcal avg</span>
					<i class="bar" style:width={`${(m.times / top) * 100}%`} style:--i={i}></i>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.frequent {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
	}
	h2 {
		margin: 0;
	}
	.list {
		list-style: none;
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
	}
	li {
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0 var(--space-3);
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
	}
	li:last-child {
		border-bottom: 0;
	}
	.label {
		font-weight: var(--weight-medium);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.meta {
		grid-column: 1;
	}
	.num {
		grid-row: 1 / span 2;
		grid-column: 2;
		align-self: center;
		font-size: var(--text-xs);
		color: var(--text-muted);
	}
	.bar {
		position: absolute;
		left: 0;
		bottom: 0;
		height: 2px;
		background: var(--accent);
		border-radius: var(--radius-pill);
		transform-origin: left;
		animation: grow-x 760ms var(--ease) both;
		animation-delay: calc(var(--i) * 80ms);
	}
	@keyframes grow-x {
		from {
			transform: scaleX(0);
		}
		to {
			transform: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.bar {
			animation: none;
		}
	}
</style>
```

**Step 4: Render the card**

In `web/src/routes/trends/+page.svelte`, import:

```ts
import FrequentMeals from '#lib/trends/FrequentMeals.svelte';
```

After `<Analytics ... />`, add:

```svelte
{#if data.top_meals}
	<FrequentMeals meals={data.top_meals} days={range} />
{/if}
```

**Step 5: Run the test to verify it passes**

```sh
cd web
npm run test:e2e -- e2e/trends.spec.ts
```

Expected: PASS.

**Step 6: Run the frontend checks**

```sh
cd web
npm test
npm run check
npm run build
```

Expected: all pass.

**Step 7: Commit**

```sh
git add web/src/lib/trends/FrequentMeals.svelte web/src/routes/trends/+page.svelte web/e2e/trends.spec.ts
git commit -s -m "Show the meals eaten most often on Trends"
```

---

# Phase 3: Repeat meal flow

## Task 14: Repeat draft, title and history filter

**Files:**
- Modify: `web/src/lib/today/meal.ts`
- Modify: `web/src/lib/today/meal.test.ts`

**Step 1: Replace the old test**

In `meal.test.ts`, replace the `describe('repeatMeal', ...)` block with:

```ts
describe('repeatDraft', () => {
	it('copies the meal to a date with new ids and source repeat', () => {
		const d = repeatDraft(saved, '2026-10-28');
		expect(d.id).not.toBe(saved.id);
		expect(d.id).toMatch(UUIDV7);
		expect(d.items[0].id).not.toBe(saved.items[0].id);
		expect(d).toMatchObject({
			date: '2026-10-28',
			meal_type: 'dinner',
			source: 'repeat',
			scale: 2,
			provider_id: null,
			assumptions: null,
			confidence: null
		});
		expect(d.items[0]).toMatchObject({ grams: 150, kcal: 195, food_id: 1 });
		// Re-saving unchanged gives the original stored values.
		expect(mealFromDraft(d).items[0]).toMatchObject({ grams: 300, kcal: 390 });
	});
});

describe('repeatTitle', () => {
	it('names the target day', () => {
		expect(repeatTitle('2026-10-28', '2026-10-28')).toBe('Repeat meal for Today');
		expect(repeatTitle('2026-10-27', '2026-10-28')).toBe('Repeat meal for Yesterday');
	});
});

describe('filterHistory', () => {
	const history = [
		{ ...saved, label: 'Oats with banana', times: 2, last_date: '2026-10-28' },
		{ ...saved, label: 'Chicken rice', times: 1, last_date: '2026-10-27' }
	];

	it('matches the label and item names, case-insensitively', () => {
		expect(filterHistory(history, 'OATS')).toHaveLength(1);
		expect(filterHistory(history, 'rice')).toHaveLength(2);
		expect(filterHistory(history, '')).toHaveLength(2);
	});
});
```

Update the imports:

```ts
import {
	// ...
	filterHistory,
	repeatDraft,
	repeatTitle,
	// ...
} from './meal';
```

**Step 2: Run the tests to verify they fail**

```sh
cd web
npm test -- src/lib/today/meal.test.ts
```

Expected: FAIL because the new helpers do not exist.

**Step 3: Implement the helpers**

In `meal.ts`, add the import:

```ts
import { dayLabel } from '#lib/date.ts';
```

Add:

```ts
/** "Eat this again": a new meal on `date`, new ids, no model call. */
export function repeatDraft(m: Meal, date: string): Draft {
	const d = draftFromMeal(m);
	return {
		...d,
		id: uuidv7(),
		date,
		source: 'repeat',
		provider_id: null,
		assumptions: null,
		confidence: null,
		warnings: [],
		items: d.items.map((i) => ({ ...i, id: uuidv7() }))
	};
}

export function repeatTitle(date: string, today: string): string {
	return `Repeat meal for ${dayLabel(date, today)}`;
}

export function filterHistory(meals: HistoryMeal[], query: string): HistoryMeal[] {
	const q = query.trim().toLowerCase();
	if (!q) return meals;
	return meals.filter(
		(m) =>
			m.label.toLowerCase().includes(q) ||
			m.items.some((i) => i.name.toLowerCase().includes(q))
	);
}
```

Add `HistoryMeal` to the type import from `#lib/api.ts`.

Remove the old `repeatMeal` function.

**Step 4: Run the tests to verify they pass**

```sh
cd web
npm test -- src/lib/today/meal.test.ts
```

Expected: PASS.

**Step 5: Commit**

```sh
git add web/src/lib/today/meal.ts web/src/lib/today/meal.test.ts
git commit -s -m "Build repeat drafts from a saved meal"
```

---

## Task 15: The recent-meals sheet

**Files:**
- Create: `web/src/lib/today/RepeatSheet.svelte`

**Step 1: Add the component**

Create `web/src/lib/today/RepeatSheet.svelte`:

```svelte
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  RepeatSheet — recent distinct meals for the selected day. Picking one closes the
  sheet and hands the meal and search query to the composer, which opens Review.
  Props: date, query, onClose, onPick.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { errorMessage, getMealHistory, type HistoryMeal } from '#lib/api.ts';
	import { dayLabel } from '#lib/date.ts';
	import { formatGrams, formatKcal } from '#lib/format.ts';
	import { closeSheet } from '#lib/sheet.ts';
	import { today } from '#lib/session.svelte.ts';
	import { filterHistory } from './meal';

	let {
		date,
		query = '',
		onClose,
		onPick
	}: {
		date: string;
		query?: string;
		onClose: () => void;
		onPick: (meal: HistoryMeal, query: string) => void;
	} = $props();

	let dialog: HTMLDialogElement;
	let meals = $state<HistoryMeal[] | null>(null);
	let error = $state('');
	let q = $state(query);

	const shown = $derived(meals ? filterHistory(meals, q) : []);
	const target = $derived(dayLabel(date, today()));

	onMount(() => {
		dialog.showModal();
		void load();
	});

	async function load() {
		error = '';
		try {
			meals = await getMealHistory(50);
		} catch (e) {
			error = errorMessage(e);
		}
	}

	function close() {
		closeSheet(dialog, onClose);
	}

	function pick(meal: HistoryMeal) {
		closeSheet(dialog, () => onPick(meal, q));
	}

	const macros = (m: HistoryMeal) =>
		`P ${formatGrams(m.totals.protein_g)} · C ${formatGrams(m.totals.carbs_g)} · F ${formatGrams(m.totals.fat_g)} g`;
</script>

<dialog
	class="sheet"
	bind:this={dialog}
	oncancel={(event) => {
		event.preventDefault();
		close();
	}}
	aria-labelledby="repeat-title"
>
	<div class="stack">
		<header class="sheet-head">
			<h2 id="repeat-title">Repeat onto {target}</h2>
			<button type="button" class="btn btn-quiet" onclick={close}>Cancel</button>
		</header>
		<p class="small muted">Recent meals, last year. Pick one, then adjust it before saving.</p>

		<div class="field">
			<label for="repeat-search">Search recent meals</label>
			<input id="repeat-search" type="search" bind:value={q} placeholder="Name or ingredient" />
		</div>

		{#if error}
			<div class="notice notice-error" role="alert">
				<p>{error}</p>
				<div class="row">
					<button type="button" class="btn" onclick={load}>Try again</button>
				</div>
			</div>
		{:else if meals === null}
			<div class="stack" aria-busy="true">
				{#each [0, 1, 2] as i (i)}<div class="skeleton" style="height: 3.5rem"></div>{/each}
			</div>
		{:else if !meals.length}
			<p class="empty">No meals in the last year. Log one and it will show up here.</p>
		{:else if !shown.length}
			<p class="empty">No recent meals match “{q.trim()}”.</p>
			<button type="button" class="btn" onclick={() => (q = '')}>Clear search</button>
		{:else}
			<ul class="list">
				{#each shown as m (m.id)}
					<li>
						<button
							type="button"
							class="pick"
							aria-label="{m.label}, {formatKcal(m.totals.kcal)} kcal, last {dayLabel(m.last_date, today())}, eaten {m.times} times"
							onclick={() => pick(m)}
						>
							<span class="name">{m.label}</span>
							<span class="small muted">
								{m.meal_type} · last {dayLabel(m.last_date, today())}
								{#if m.times > 1}· eaten {m.times} times{/if}
							</span>
							<span class="small muted num">{macros(m)}</span>
						</button>
						<span class="num kcal">{formatKcal(m.totals.kcal)} kcal</span>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</dialog>

<style>
	.pick {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0.125rem;
		min-width: 0;
		min-height: var(--touch);
		padding: var(--space-1) var(--space-2);
		margin-left: calc(-1 * var(--space-2));
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.pick:hover {
		background: var(--surface-2);
		border-color: var(--border);
	}
	.name {
		font-weight: var(--weight-medium);
		overflow-wrap: anywhere;
	}
	li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		border-bottom: 1px solid var(--border);
	}
	li:last-child {
		border-bottom: 0;
	}
	.kcal {
		flex: none;
		font-weight: var(--weight-medium);
	}
</style>
```

**Step 2: Run the frontend checks**

```sh
cd web
npm run check
npm run build
```

Expected: both pass.

**Step 3: Commit**

```sh
git add web/src/lib/today/RepeatSheet.svelte
git commit -s -m "Add the recent meals sheet"
```

---

## Task 16: Composer repeat flow

**Files:**
- Modify: `web/src/lib/today/Composer.svelte`

**Step 1: Add the repeat state**

In the script, add:

```ts
import RepeatSheet from './RepeatSheet.svelte';
import { repeatDraft, repeatTitle } from './meal';
import type { HistoryMeal } from '#lib/api.ts';
```

Add state:

```ts
let repeatOpen = $state(false);
let repeatQuery = $state('');
let reviewFromRepeat = $state(false);
let repeatSaved = $state(false);
```

**Step 2: Add the open and close handlers**

```ts
function openRepeat() {
	repeatQuery = '';
	repeatOpen = true;
}

function picked(meal: HistoryMeal, query: string) {
	repeatQuery = query;
	repeatOpen = false;
	reviewFromRepeat = true;
	repeatSaved = false;
	review = {
		draft: repeatDraft(meal, date),
		title: repeatTitle(date, today())
	};
}

function repeatClosed() {
	repeatOpen = false;
}

function reviewClosed() {
	review = null;
	if (reviewFromRepeat && !repeatSaved) repeatOpen = true;
	reviewFromRepeat = false;
	repeatSaved = false;
}

function repeatSavedHandler() {
	repeatSaved = true;
	onSaved();
}
```

Import `today` from `#lib/session.svelte.ts`.

**Step 3: Add the button**

In the composer header, add:

```svelte
<button type="button" class="btn btn-quiet" onclick={openRepeat}>Repeat a meal</button>
```

**Step 4: Wire the sheet and Review**

After the composer form, add:

```svelte
{#if repeatOpen}
	<RepeatSheet
		{date}
		query={repeatQuery}
		onClose={repeatClosed}
		onPick={picked}
	/>
{/if}
```

Change the existing Review block to use the new close and saved handlers:

```svelte
{#if review}
	<Review
		draft={review.draft}
		title={review.title}
		onClose={reviewClosed}
		onSaved={reviewFromRepeat ? repeatSavedHandler : saved}
	/>
{/if}
```

When opening Review for an estimate or manual entry, set `reviewFromRepeat = false`.

**Step 5: Run the frontend checks**

```sh
cd web
npm run check
npm run build
```

Expected: both pass.

**Step 6: Commit**

```sh
git add web/src/lib/today/Composer.svelte
git commit -s -m "Open Review from the recent meals sheet"
```

---

## Task 17: Per-meal repeat and copy to today

**Files:**
- Modify: `web/src/lib/today/MealList.svelte`

**Step 1: Replace the editing state**

Change:

```ts
let editing = $state<Draft | null>(null);
```

to:

```ts
type ReviewState = { draft: Draft; title: string; kind: 'edit' | 'repeat'; target: string };
let review = $state<ReviewState | null>(null);
let copiedToToday = $state(false);
```

Import `day`, `today` from `#lib/session.svelte.ts`, and `repeatDraft`, `repeatTitle` from `./meal`.

**Step 2: Add the actions**

Replace the Edit button handler:

```ts
review = { draft: draftFromMeal(m), title: 'Edit meal', kind: 'edit', target: date };
```

Replace the "Eat this again" button with:

```svelte
<button
	class="btn"
	onclick={() => {
		const target = date === today() ? date : today();
		review = {
			draft: repeatDraft(m, target),
			title: repeatTitle(target, today()),
			kind: 'repeat',
			target
		};
	}}
>
	{date === today() ? 'Repeat' : 'Copy to today'}
</button>
```

Remove the old `repeatMeal` import and the `send(m, mealUpsert(repeatMeal(...)))` path.

**Step 3: Add the saved and close handlers**

```ts
function saved() {
	if (!review) return;
	if (review.kind === 'repeat') {
		copiedToToday = review.target !== date;
		status = copiedToToday ? 'Copied to today.' : 'Meal logged.';
	} else {
		status = 'Meal updated.';
	}
	onChanged();
}

function closed() {
	review = null;
}
```

**Step 4: Update the Review block**

Replace the old `{#if editing}` block with:

```svelte
{#if review}
	<Review
		draft={review.draft}
		title={review.title}
		onClose={closed}
		onSaved={saved}
		onStale={onChanged}
	/>
{/if}
```

**Step 5: Add the View today notice**

After the status paragraph, add:

```svelte
{#if copiedToToday}
	<div class="notice notice-info row">
		<p>Copied to today.</p>
		<button class="btn" onclick={() => (day.date = today())}>View today</button>
	</div>
{/if}
```

Reset `copiedToToday = false` when the date changes in the load effect.

**Step 6: Run the frontend checks**

```sh
cd web
npm run check
npm run build
```

Expected: both pass.

**Step 7: Commit**

```sh
git add web/src/lib/today/MealList.svelte
git commit -s -m "Repeat a meal from the meal list"
```

---

## Task 18: End-to-end coverage

**Files:**
- Modify: `web/e2e/today.spec.ts`
- Modify: `web/e2e/trends.spec.ts`

**Step 1: Update the old repeat test**

In `today.spec.ts`, replace the `"Eat this again" duplicates the meal` test with:

```ts
test('Repeat opens Review and logs a copy', async ({ page }) => {
	await page.getByTestId('meal').locator('summary').click();
	await page.getByRole('button', { name: 'Repeat' }).click();
	const sheet = page.getByRole('dialog', { name: /Repeat meal for/ });
	await expect(sheet).toBeVisible();
	await expect(sheet.locator('.totals')).toContainText('332 kcal');
	await sheet.getByRole('button', { name: 'Save meal' }).click();
	await expect(sheet).toBeHidden();
	await expect(page.getByTestId('meal')).toHaveCount(2);
	await expect(dayTotal(page)).toContainText('664 kcal');
	await expect(page.getByTestId('meal').nth(1)).toContainText('Eaten again');
});
```

**Step 2: Add a repeat-from-history test**

Add:

```ts
test('Repeat a meal from a previous day', async ({ page }) => {
	await page.getByRole('button', { name: 'Previous day' }).click();
	await page.getByLabel('What did you eat?').fill('yesterday oats');
	await page.getByRole('button', { name: 'Enter by hand' }).click();
	const entry = page.getByRole('dialog', { name: 'Enter by hand' });
	await fillItem(page, 0, ['Oats', '100', '380', '12', '60', '7']);
	await entry.getByRole('button', { name: 'Save meal' }).click();
	await expect(entry).toBeHidden();

	await page.getByRole('button', { name: 'Back to today' }).click();
	await page.getByRole('button', { name: 'Repeat a meal' }).click();
	const picker = page.getByRole('dialog', { name: /Repeat onto Today/ });
	await picker.getByRole('button', { name: /yesterday oats/ }).click();
	const review = page.getByRole('dialog', { name: 'Repeat meal for Today' });
	await expect(review.locator('.totals')).toContainText('380 kcal');
	await review.getByRole('button', { name: 'Save meal' }).click();
	await expect(review).toBeHidden();
	await expect(page.getByTestId('meal').filter({ hasText: 'yesterday oats' })).toHaveCount(1);
});
```

**Step 3: Add a copy-to-today test**

Add:

```ts
test('Copy a past meal to today', async ({ page }) => {
	await page.getByRole('button', { name: 'Previous day' }).click();
	await page.getByTestId('meal').first().locator('summary').click();
	await page.getByRole('button', { name: 'Copy to today' }).click();
	const review = page.getByRole('dialog', { name: 'Repeat meal for Today' });
	await expect(review).toBeVisible();
	await review.getByRole('button', { name: 'Save meal' }).click();
	await expect(review).toBeHidden();
	await expect(page.getByText('Copied to today.')).toBeVisible();
	await page.getByRole('button', { name: 'View today' }).click();
	await expect(page.getByTestId('meal').filter({ hasText: 'yesterday oats' })).toHaveCount(2);
});
```

**Step 4: Run the e2e tests**

```sh
cd web
npm run test:e2e -- e2e/today.spec.ts e2e/trends.spec.ts
```

Expected: all pass.

**Step 5: Commit**

```sh
git add web/e2e/today.spec.ts web/e2e/trends.spec.ts
git commit -s -m "Cover repeat and copy-to-today flows"
```

---

# Phase 4: Docs and final validation

## Task 19: Update docs and run the full suite

**Files:**
- Modify: `docs/design.md`
- Modify: `docs/decisions.md`
- Modify: `docs/offline.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`

**Step 1: Update `docs/design.md`**

- Replace the motion section with the new hierarchy and token values.
- Amend the old "There are no entrance animations" line to allow sheets and Trends.
- Replace "popping consistency cells" and the 12 ms stagger note.
- Add the "Eaten most" card.
- Add the repeat sheet and the per-meal Repeat/Copy to today actions.

**Step 2: Update `docs/decisions.md`**

Add one line each:

```md
- 2026-11-04 - Meal history is the repeat library; no saved-meal table. - Grouping by normalized description is enough for the first version and avoids a new entity.
- 2026-11-04 - Repeat targets the selected day; no future scheduling. - The app already treats today as the write boundary.
- 2026-11-04 - Motion refresh: feedback 160 ms, movement 360 ms, sheets 420 ms. - The old timings read as abrupt on larger surfaces.
```

**Step 3: Update `docs/offline.md`**

Add `/api/meals/history?limit=50` to the cacheable reads and note that meal upserts and deletes invalidate cached history and trends.

**Step 4: Update `AGENTS.md` and `CLAUDE.md`**

Add to the layout:

```text
  src/lib/today/RepeatSheet.svelte   recent meals picker, opens Review
  src/lib/trends/FrequentMeals.svelte  "Eaten most" card
```

**Step 5: Run the full validation**

Backend:

```sh
cd server
SQLX_OFFLINE=true cargo test
SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Frontend:

```sh
cd web
npm test
npm run check
npm run build
npm run test:e2e
```

Expected: all pass.

**Step 6: Commit**

```sh
git add docs/design.md docs/decisions.md docs/offline.md AGENTS.md CLAUDE.md
git commit -s -m "Document meal history and the motion refresh"
```

---

## Final acceptance

- Trends shows the top five meals by count for the selected range.
- The repeat sheet lists recent distinct meals, supports search, and opens Review with the copied items and macros.
- The per-meal action opens Review, targets today on a past day, and never saves immediately.
- A repeated meal keeps its multiplier and item values, and gets new ids.
- The repeat flow works offline through the existing queue.
- Meal writes invalidate cached history and trends.
- Motion tokens and timings are applied across the app.
- Sheets open and close with the new lifecycle. Reduced motion skips the delay.
- Backend, Vitest, svelte-check, build and Playwright are green.
