// SPDX-License-Identifier: AGPL-3.0-or-later
// Today: numbers edits and saved products. Starts its own server like today.spec.ts, with a mock
// OpenAI-compatible provider answering with a canned estimate.
import { expect, test, type Page } from '@playwright/test';
import { type ChildProcess } from 'node:child_process';
import { createServer, type Server } from 'node:http';
import { login, noSideScroll, randomPort, resetSavedAuth, startCupful } from './helpers/today';

test.describe('numbers and my products', () => {
	test.describe.configure({ mode: 'serial' });
	const port = randomPort();
	let server: ChildProcess;
	let mock: Server;
	test.use({ baseURL: `http://127.0.0.1:${port}` });

	test.beforeAll(async () => {
		resetSavedAuth();
		const reply = {
			items: [
				{ food: 'chocolate protein powder', amount: 1, unit: 'scoop', grams: 15, cooking: 'none', mentions_oil: false,
				  estimate: { kcal: 60, protein: 12, carbs: 2, fat: 1 } }
			],
			assumptions: null,
			confidence: 'medium'
		};
		mock = createServer((req, res) => {
			req.resume();
			req.on('end', () => {
				res.setHeader('Content-Type', 'application/json');
				res.end(JSON.stringify({ choices: [{ message: { role: 'assistant', content: JSON.stringify(reply) } }] }));
			});
		});
		await new Promise<void>((r) => mock.listen(0, '127.0.0.1', r));
		const mockPort = (mock.address() as { port: number }).port;
		server = await startCupful(port, {
			CUPFUL_PROVIDER_KIND: 'openai_compatible',
			CUPFUL_PROVIDER_BASE_URL: `http://127.0.0.1:${mockPort}/v1`,
			CUPFUL_PROVIDER_MODEL: 'mock',
			CUPFUL_PROVIDER_LOCAL: 'true'
		});
	});
	test.afterAll(() => {
		server?.kill();
		mock?.close();
	});
	test.beforeEach(({ page }) => login(page));

	const meals = async (page: Page) => {
		const date = await page.evaluate(() => new Date().toLocaleDateString('en-CA'));
		const res = await page.request.get(`/api/meals?date=${date}`);
		return (await res.json()).meals as {
			items: { name: string; kcal: number; protein_g: number; grams: number; origin: string; food_id: number | null }[];
		}[];
	};
	const estimate = async (page: Page) => {
		await page.getByLabel('What did you eat?').fill('a scoop of chocolate protein powder');
		await page.getByRole('button', { name: 'Estimate' }).click();
		const sheet = page.getByRole('dialog', { name: 'Review estimate' });
		await expect(sheet).toBeVisible();
		return sheet;
	};
	const field = (sheet: ReturnType<Page['locator']>, label: string) => sheet.locator('.item').first().getByLabel(label, { exact: true });

	test('editing kcal scales the macros; the item is saved as Edited', async ({ page }) => {
		const sheet = await estimate(page);
		await sheet.getByRole('button', { name: 'Edit numbers' }).click();
		const protein = Number(await field(sheet, 'Protein g').inputValue());
		const kcal = Number(await field(sheet, 'kcal').inputValue());
		await field(sheet, 'kcal').fill('90');
		const count = await meals(page).then((m) => m.length);
		await expect(sheet.locator('.item').first().getByText('Edited')).toBeVisible();
		await expect.poll(async () => Number(await field(sheet, 'Protein g').inputValue())).toBeCloseTo((protein * 90) / kcal, 1);
		await expect(sheet.locator('.totals')).toContainText('90 kcal');
		await noSideScroll(page);
		await sheet.getByRole('button', { name: 'Save meal' }).click();
		await expect(sheet).toBeHidden();
		const saved = await meals(page);
		expect(saved).toHaveLength(count + 1);
		expect(saved.at(-1)!.items[0]).toMatchObject({ kcal: 90, origin: 'manual' });
		await expect(page.getByTestId('meal').last()).toContainText('Edited');
	});

	test('use my product + remember: the same phrase then resolves to it', async ({ page }) => {
		const made = await page.request.post('/api/products', {
			headers: { 'X-Cupful': '1' },
			data: { name: 'Choc Whey', brand: 'Acme', barcode: '4006381333931', basis: 'serving', serving_g: 31, serving_text: '1 scoop', kcal: 120, protein_g: 24, carbs_g: 3, fat_g: 2 }
		});
		expect(made.ok()).toBe(true);
		const product = await made.json();

		let sheet = await estimate(page);
		await sheet.getByRole('button', { name: 'Use my product' }).click();
		await sheet.getByLabel('Search my products').fill('whey');
		await sheet.getByRole('button', { name: /Choc Whey/ }).click();
		await expect(sheet.locator('.item').first().getByText('Choc Whey')).toBeVisible();
		await expect(sheet.locator('.totals')).toContainText('120 kcal'); // 1 scoop x 31 g
		await sheet.getByLabel("Always use this for 'chocolate protein powder'").check();
		await noSideScroll(page);
		await sheet.getByRole('button', { name: 'Save meal' }).click();
		await expect(sheet).toBeHidden();
		const item = (await meals(page)).at(-1)!.items[0];
		expect(item).toMatchObject({ food_id: product.food_id, grams: 31, origin: 'food_db' });
		expect(item.kcal).toBeCloseTo(120, 0);

		// Needs the backend: the estimate now resolves the phrase to the product at its serving.
		sheet = await estimate(page);
		await expect(sheet.locator('.item').first().getByText('Database')).toBeVisible();
		await expect(field(sheet, 'Grams')).toHaveValue('31');
		await expect(sheet.locator('.totals')).toContainText('120 kcal');
		await sheet.getByRole('button', { name: 'Cancel' }).click();
	});

	test('a second product takes over the phrase; the earlier meal is unchanged', async ({ page }) => {
		const before = await meals(page);
		const sheet = page.getByRole('dialog');
		await page.getByRole('button', { name: 'Scan barcode' }).click();
		await sheet.getByLabel('Or type the barcode').fill('5901234123457');
		await sheet.getByRole('button', { name: 'Look up' }).click();
		await sheet.getByLabel('Name').fill('Choc Isolate');
		await sheet.getByLabel('Serving size (g)').fill('40');
		await sheet.getByLabel('Calories (kcal)').fill('150');
		await sheet.getByLabel('Protein (g)').fill('30');
		await sheet.getByLabel('Carbs (g)').fill('3');
		await sheet.getByLabel('Fat (g)').fill('1');
		await sheet.getByRole('button', { name: 'Save product' }).click();

		await expect(sheet.getByRole('heading', { name: 'Use for descriptions' })).toBeVisible();
		await noSideScroll(page);
		await sheet.getByRole('button', { name: 'Use this product instead for chocolate protein powder' }).click();
		await expect(sheet.getByText('chocolate protein powder', { exact: true })).toBeVisible();
		await expect(sheet.getByRole('button', { name: 'Remove chocolate protein powder' })).toBeVisible();
		await sheet.getByRole('button', { name: 'Close' }).click();
		await expect(sheet).toBeHidden();

		const next = await estimate(page);
		await expect(next.locator('.item').first().getByText('Database')).toBeVisible();
		await expect(next.locator('.totals')).toContainText('150 kcal'); // a scoop of B is one 40 g serving
		await next.getByRole('button', { name: 'Cancel' }).click();
		expect(await meals(page)).toEqual(before);
	});
});
