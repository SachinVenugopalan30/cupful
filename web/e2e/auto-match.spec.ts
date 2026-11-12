// SPDX-License-Identifier: AGPL-3.0-or-later
// An AI estimate item whose name exactly matches a saved product offers that product in one tap:
// the products load once for the whole review sheet, and the swap is the existing food_db path.
import { expect, test, type Page } from '@playwright/test';
import { createServer, type Server } from 'node:http';
import {
	login,
	localDate,
	randomPort,
	startCupful,
	type CupfulServer
} from './helpers/server.ts';

const port = randomPort();
let server: CupfulServer;
let mock: Server;

test.use({ baseURL: `http://127.0.0.1:${port}` });

test.beforeAll(async () => {
	const reply = {
		items: [
			{
				food: 'Whey protein powder',
				amount: 1,
				unit: 'scoop',
				grams: 30,
				cooking: 'none',
				mentions_oil: false,
				estimate: { kcal: 120, protein: 24, carbs: 3, fat: 2 }
			},
			{
				food: 'Banana',
				amount: 1,
				unit: 'medium',
				grams: 118,
				cooking: 'none',
				mentions_oil: false,
				estimate: { kcal: 105, protein: 1.3, carbs: 27, fat: 0.4 }
			}
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
	server?.stop();
	mock?.close();
});
test.beforeEach(({ page }) => login(page));

async function createProduct(page: Page) {
	const res = await page.request.post('/api/products', {
		headers: { 'X-Cupful': '1' },
		data: {
			name: 'Whey protein powder',
			brand: 'Acme',
			barcode: '4006381333931',
			basis: 'serving',
			serving_g: 30,
			serving_text: '1 scoop',
			kcal: 150,
			protein_g: 30,
			carbs_g: 3,
			fat_g: 1
		}
	});
	expect(res.ok()).toBe(true);
	return (await res.json()) as { food_id: number };
}

test('an exact saved-product name offers a one-tap swap and saves as food_db', async ({ page }) => {
	const product = await createProduct(page);

	let productGets = 0;
	page.on('request', (r) => {
		if (r.method() === 'GET' && new URL(r.url()).pathname === '/api/products') productGets++;
	});

	await page.getByLabel('What did you eat?').fill('a scoop of whey protein powder and a banana');
	await page.getByRole('button', { name: 'Estimate' }).click();
	const sheet = page.getByRole('dialog', { name: 'Review estimate' });
	await expect(sheet).toBeVisible();

	// The AI item keeps its estimate (the pipeline found two foods with this name), and the saved
	// product is offered by exact name. Manual items never get this button.
	const first = sheet.locator('.item').first();
	await expect(first.getByText('AI estimate', { exact: true })).toBeVisible();
	const use = first.getByRole('button', { name: 'Use Whey protein powder' });
	await expect(use).toBeVisible();
	await expect(first.getByText('Saved product')).toBeVisible();
	await expect(sheet.locator('.item').nth(1).getByText('Saved product')).toHaveCount(0);

	// One products request for the whole sheet, not one per item.
	await expect.poll(() => productGets).toBe(1);

	await use.click();
	await expect(use).toHaveCount(0);
	await expect(first.getByText('Database')).toBeVisible();
	await expect(first).toContainText('150 kcal');

	await sheet.getByRole('button', { name: 'Save meal' }).click();
	await expect(sheet).toBeHidden();

	const res = await page.request.get(`/api/meals?date=${localDate()}`);
	const meal = (await res.json()).meals.at(-1) as {
		items: { name: string; origin: string; food_id: number | null; kcal: number; protein_g: number; grams: number }[];
	};
	expect(meal.items).toHaveLength(2);
	expect(meal.items[0]).toMatchObject({
		name: 'Whey protein powder',
		origin: 'food_db',
		food_id: product.food_id,
		kcal: 150,
		protein_g: 30,
		grams: 30
	});
});
