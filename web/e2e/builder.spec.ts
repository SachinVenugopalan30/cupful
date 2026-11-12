// SPDX-License-Identifier: AGPL-3.0-or-later
// Review as the meal builder: name, + Product append, starter-row cleanup and inline scanning,
// against a private server with product lookups off (so every scanned code goes to label entry).
import { expect, test, type Page } from '@playwright/test';
import { login, randomPort, startCupful, type CupfulServer } from './helpers/server.ts';

const PORT = randomPort();
let server: CupfulServer;

test.use({ baseURL: `http://127.0.0.1:${PORT}` });
test.beforeAll(async () => {
	server = await startCupful(PORT);
});
test.afterAll(() => server?.stop());
test.beforeEach(({ page }) => login(page));

async function createProduct(page: Page, name: string, barcode: string, servingG = 40) {
	const res = await page.request.post('/api/products', {
		headers: { 'X-Cupful': '1' },
		data: {
			name,
			brand: 'Acme',
			barcode,
			basis: 'serving',
			serving_g: servingG,
			kcal: 150,
			protein_g: 30,
			carbs_g: 3,
			fat_g: 1
		}
	});
	expect(res.ok()).toBe(true);
}

async function meals(page: Page) {
	const date = await page.evaluate(() => new Date().toLocaleDateString('en-CA'));
	const res = await page.request.get(`/api/meals?date=${date}`);
	return (await res.json()).meals as {
		description: string;
		items: { name: string; grams: number; origin: string }[];
	}[];
}

const hand = async (page: Page, description: string) => {
	await page.getByLabel('What did you eat?').fill(description);
	await page.getByRole('button', { name: 'Enter by hand' }).click();
	return page.getByRole('dialog', { name: 'Enter by hand' });
};

test('the name is prefilled and + Product replaces the untouched starter row', async ({ page }) => {
	await createProduct(page, 'Choc Isolate', '2000000000015');
	const sheet = await hand(page, 'snack plate');
	await expect(sheet.getByLabel('Name (optional)')).toHaveValue('snack plate');
	await sheet.getByLabel('Name (optional)').fill('Evening snack');

	await sheet.getByRole('button', { name: '+ Product' }).click();
	await sheet.getByLabel('Search my products').fill('isolate');
	await sheet.getByRole('button', { name: /Choc Isolate/ }).click();

	// The blank starter row is gone; the product is the only item, at one 40 g serving.
	await expect(sheet.locator('.item')).toHaveCount(1);
	await expect(sheet.locator('.item').first()).toContainText('Choc Isolate');
	await expect(sheet.locator('.item').first().getByLabel('Grams')).toHaveValue('40');
	await sheet.getByRole('button', { name: 'Save meal' }).click();
	await expect(sheet).toBeHidden();

	const saved = (await meals(page)).at(-1)!;
	expect(saved.description).toBe('Evening snack');
	expect(saved.items).toHaveLength(1);
	expect(saved.items[0]).toMatchObject({ name: 'Choc Isolate', origin: 'food_db', grams: 40 });
	await expect(page.getByTestId('meal').filter({ hasText: 'Evening snack' })).toContainText('Choc Isolate');
});

test('a touched starter row is kept; mixed origins show in MealList', async ({ page }) => {
	await createProduct(page, 'Choc Shake', '2000000000022');
	const sheet = await hand(page, 'banana and a shake');
	await sheet.locator('.item').first().getByLabel('Item name').fill('Banana');
	await sheet.locator('.item').first().getByLabel('Grams').fill('100');
	await sheet.locator('.item').first().getByLabel('kcal', { exact: true }).fill('89');
	await sheet.locator('.item').first().getByLabel('Protein g').fill('1');
	await sheet.locator('.item').first().getByLabel('Carbs g').fill('23');
	await sheet.locator('.item').first().getByLabel('Fat g').fill('0.3');

	await sheet.getByRole('button', { name: '+ Product' }).click();
	await sheet.getByLabel('Search my products').fill('shake');
	await sheet.getByRole('button', { name: /Choc Shake/ }).click();
	await expect(sheet.locator('.item')).toHaveCount(2);

	await sheet.getByRole('button', { name: 'Save meal' }).click();
	await expect(sheet).toBeHidden();

	const saved = (await meals(page)).at(-1)!;
	expect(saved.items.map((i) => i.origin)).toEqual(['manual', 'food_db']);
	const row = page.getByTestId('meal').filter({ hasText: 'banana and a shake' });
	await row.locator('summary').click();
	await expect(row).toContainText('By hand');
	await expect(row).toContainText('Database');
	await expect(row).not.toContainText('AI estimate');
});

test('+ Product scans inline and enters a missing label without a second modal', async ({ page }) => {
	const sheet = await hand(page, 'crackers');
	await sheet.getByRole('button', { name: '+ Product' }).click();
	await sheet.getByRole('button', { name: /Scan a product/ }).click();
	await expect(page.getByRole('dialog')).toHaveCount(1);

	await sheet.getByLabel('Or type the barcode').fill('2000000000008');
	await sheet.getByRole('button', { name: 'Look up' }).click();
	await expect(sheet.getByLabel('Barcode')).toHaveValue('2000000000008');
	await sheet.getByLabel('Name', { exact: true }).fill('Sea salt crackers');
	await sheet.getByLabel('Serving size (g)').fill('25');
	await sheet.getByLabel('Calories (kcal)').fill('120');
	await sheet.getByLabel('Protein (g)').fill('3');
	await sheet.getByLabel('Carbs (g)').fill('17');
	await sheet.getByLabel('Fat (g)').fill('4');
	await sheet.getByRole('button', { name: 'Save product' }).click();

	await expect(sheet.locator('.item')).toHaveCount(1);
	await expect(sheet.locator('.item').first()).toContainText('Sea salt crackers');
	await expect(sheet.locator('.item').first().getByLabel('Grams')).toHaveValue('25');
	await sheet.getByRole('button', { name: 'Save meal' }).click();
	await expect(sheet).toBeHidden();
	expect((await meals(page)).at(-1)!.items[0]).toMatchObject({
		name: 'Sea salt crackers',
		origin: 'food_db',
		grams: 25
	});
});
