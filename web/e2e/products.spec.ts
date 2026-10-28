// SPDX-License-Identifier: AGPL-3.0-or-later
// Barcode products at 360 px. Lookups are off in e2e, so every barcode is "not found": the flow
// goes through label entry. Camera scanning can't run headless; the decoder choice is unit-tested.
import { expect, test, type Page } from '@playwright/test';

async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

/** Logged in on Today, whether or not first-run setup already happened. */
async function loggedIn(page: Page) {
	await page.goto('/');
	const today = page.getByRole('heading', { name: 'Today', level: 1 });
	await expect(today.or(page.getByLabel('Password'))).toBeVisible();
	if (page.url().endsWith('/setup')) {
		await page.getByLabel('Username').fill('admin');
		await page.getByLabel('Password').fill('correct horse');
		await page.getByRole('button', { name: 'Create account' }).click();
	} else if (page.url().endsWith('/login')) {
		await page.getByLabel('Username').fill('admin');
		await page.getByLabel('Password').fill('correct horse');
		await page.getByRole('button', { name: 'Log in' }).click();
	}
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
}

async function mealsToday(page: Page) {
	const date = await page.evaluate(() => new Date().toLocaleDateString('en-CA'));
	const res = await page.request.get(`/api/meals?date=${date}`);
	expect(res.ok()).toBe(true);
	return (await res.json()).meals as {
		description: string;
		source: string;
		totals: { kcal: number };
		items: { grams: number; portion_text: string; origin: string; food_id: number | null }[];
	}[];
}

test('typed barcode, label entry, log 1.5 servings, saved products, re-log, remove', async ({ page }) => {
	await loggedIn(page);
	const before = (await mealsToday(page)).length;
	const sheet = page.getByRole('dialog');

	// Scan → typed barcode (no camera headless) → not found → label entry
	await page.getByRole('button', { name: 'Scan barcode' }).click();
	await expect(sheet.getByRole('heading', { name: 'Scan barcode' })).toBeFocused();
	await sheet.getByLabel('Or type the barcode').fill('4006381333932');
	await sheet.getByRole('button', { name: 'Look up' }).click();
	await expect(sheet.getByText('Check the barcode')).toBeVisible();
	await sheet.getByLabel('Or type the barcode').fill('4006381333931');
	await sheet.getByRole('button', { name: 'Look up' }).click();

	await expect(sheet.getByRole('heading', { name: 'Enter from the label' })).toBeVisible();
	await expect(sheet.getByLabel('Barcode')).toHaveValue('4006381333931');
	await noSideScroll(page);
	await sheet.getByLabel('Name').fill('Oat crackers');
	await sheet.getByLabel('Brand').fill('Acme');
	await sheet.getByLabel('Serving size (g)').fill('30');
	// 300 kcal per 30 g = 1000 per 100 g: too much
	await sheet.getByLabel('Calories (kcal)').fill('300');
	await sheet.getByLabel('Protein (g)').fill('3');
	await sheet.getByLabel('Carbs (g)').fill('20');
	await sheet.getByLabel('Fat (g)').fill('4');
	await sheet.getByRole('button', { name: 'Save product' }).click();
	await expect(sheet.getByRole('alert')).toContainText('0–900 kcal per 100 g');
	await expect(sheet.getByLabel('Calories (kcal)')).toBeFocused();
	await sheet.getByLabel('Calories (kcal)').fill('120');
	await sheet.getByRole('button', { name: 'Save product' }).click();

	// Product sheet: 1 serving → 1.5 servings (45 g, 180 kcal)
	await expect(sheet.getByRole('heading', { name: 'Oat crackers' })).toBeVisible();
	await expect(sheet.getByLabel('Grams')).toHaveValue('30');
	await sheet.getByRole('button', { name: 'More servings' }).click();
	await expect(sheet.getByLabel('Grams')).toHaveValue('45');
	await expect(sheet.getByText('180', { exact: true })).toBeVisible();
	await noSideScroll(page);
	await sheet.getByText('Snack', { exact: true }).click();
	await sheet.getByRole('button', { name: 'Add', exact: true }).click();
	await expect(sheet).toBeHidden();
	await expect(page.getByText('Added Oat crackers.')).toBeAttached();

	let meals = await mealsToday(page);
	expect(meals).toHaveLength(before + 1);
	const logged = meals.find((m) => m.description === 'Oat crackers')!;
	expect(logged.source).toBe('manual');
	expect(logged.totals.kcal).toBe(180);
	expect(logged.items[0]).toMatchObject({ grams: 45, portion_text: '1.5 servings (45 g)', origin: 'food_db' });

	// Saved products: there, re-log with last servings (1.5)
	await page.getByRole('button', { name: 'Saved products' }).click();
	await expect(sheet.getByRole('heading', { name: 'Saved products' })).toBeVisible();
	await sheet.getByLabel('Search saved products').fill('acme');
	await sheet.getByRole('button', { name: /^Oat crackers/ }).click();
	await expect(sheet.getByRole('heading', { name: 'Oat crackers' })).toBeVisible();
	await expect(sheet.getByLabel('Grams')).toHaveValue('45');
	await sheet.getByRole('button', { name: 'Add', exact: true }).click();
	await expect(sheet).toBeHidden();
	meals = await mealsToday(page);
	expect(meals.filter((m) => m.description === 'Oat crackers')).toHaveLength(2);

	// Remove from the list, with confirmation
	await page.getByRole('button', { name: 'Saved products' }).click();
	await sheet.getByRole('button', { name: 'Remove Oat crackers' }).click();
	await expect(sheet.getByText('Remove Oat crackers from saved products?')).toBeVisible();
	await sheet.getByRole('button', { name: 'Remove', exact: true }).click();
	await expect(sheet.getByText('No saved products yet.')).toBeVisible();
	await noSideScroll(page);
	await sheet.getByRole('button', { name: 'Close' }).click();
	await expect(sheet).toBeHidden();
});
