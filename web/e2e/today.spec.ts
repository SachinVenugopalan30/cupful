// SPDX-License-Identifier: AGPL-3.0-or-later
// Today meal flow. Each describe starts its own server (the debug binary the shared webServer just
// built) on a fresh data dir, so these meals don't mix with other specs' and the AI path can have
// a provider: a tiny mock OpenAI-compatible server answering with a canned estimate.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const BIN = fileURLToPath(new URL('../../server/target/debug/cupful', import.meta.url));
const USER = { username: 'eater', password: 'correct horse' };
const randomPort = () => 20000 + Math.floor(Math.random() * 20000);

async function startCupful(port: number, env: Record<string, string>): Promise<ChildProcess> {
	const proc = spawn(BIN, ['serve'], {
		stdio: 'ignore',
		env: {
			...process.env,
			CUPFUL_BIND: `127.0.0.1:${port}`,
			CUPFUL_DATA_DIR: mkdtempSync(join(tmpdir(), 'cupful-e2e-today-')),
			CUPFUL_SECURE_COOKIES: 'false',
			CUPFUL_PRODUCT_LOOKUP: 'off',
			CUPFUL_PROVIDER_BASE_URL: '',
			CUPFUL_SECRET_KEY: '',
			...env
		}
	});
	const base = `http://127.0.0.1:${port}`;
	for (let i = 0; i < 100; i++) {
		if (await fetch(`${base}/api/health`).then((r) => r.ok, () => false)) break;
		await new Promise((r) => setTimeout(r, 100));
	}
	const res = await fetch(`${base}/api/auth/setup`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json', 'X-Cupful': '1' },
		body: JSON.stringify({ ...USER, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone })
	});
	expect(res.ok).toBe(true);
	return proc;
}

async function login(page: Page) {
	const res = await page.request.post('/api/auth/login', { headers: { 'X-Cupful': '1' }, data: USER });
	expect(res.ok()).toBe(true);
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Log a meal' })).toBeVisible();
}

const dayTotal = (page: Page) => page.getByTestId('day-total');
const kcalOf = async (text: string) => Number(/([\d.]+) kcal/.exec(text)![1]);

async function fillItem(page: Page, n: number, v: [string, string, string, string, string, string]) {
	const item = page.getByRole('dialog').locator('.item').nth(n);
	const labels = ['Item name', 'Grams', 'kcal', 'Protein g', 'Carbs g', 'Fat g'];
	for (let i = 0; i < labels.length; i++) await item.getByLabel(labels[i], { exact: true }).fill(v[i]);
}

test.describe('without an AI provider', () => {
	test.describe.configure({ mode: 'serial' });
	const port = randomPort();
	let server: ChildProcess;
	test.use({ baseURL: `http://127.0.0.1:${port}` });
	test.beforeAll(async () => (server = await startCupful(port, {})));
	test.afterAll(() => void server?.kill());
	test.beforeEach(({ page }) => login(page));

	test('no-provider error is clear; manual entry saves a meal with correct totals', async ({ page }) => {
		await expect(page.getByText('Nothing logged yet.')).toBeVisible();
		await page.getByLabel('What did you eat?').fill('toast with butter');
		await page.getByRole('button', { name: 'Estimate' }).click();
		const alert = page.getByRole('alert');
		await expect(alert).toContainText('No AI provider is set up yet');
		await expect(alert.getByRole('link', { name: 'Open Settings' })).toHaveAttribute('href', '/settings');
		await alert.getByRole('button', { name: 'Enter by hand' }).click();

		const sheet = page.getByRole('dialog', { name: 'Enter by hand' });
		await expect(sheet).toBeVisible();
		await expect(sheet.getByText('Needs your numbers')).toBeVisible();
		await fillItem(page, 0, ['Toast', '50', '130', '4', '24', '2']);
		await sheet.getByRole('button', { name: 'Add item' }).click();
		await fillItem(page, 1, ['Butter', '10', '72', '0', '0', '8']);
		await expect(sheet.locator('.totals')).toContainText('202 kcal');
		await noSideScroll(page);
		await sheet.getByRole('button', { name: 'Save meal' }).click();
		await expect(sheet).toBeHidden();

		await expect(page.getByTestId('meal')).toHaveCount(1);
		await expect(page.getByTestId('meal')).toContainText('toast with butter');
		await expect(dayTotal(page)).toContainText('202 kcal');
		await expect(dayTotal(page)).toContainText('P 4 · C 24 · F 10 g');
		await expect(page.getByLabel('What did you eat?')).toHaveValue('');
		await noSideScroll(page);
	});

	test('edit changes totals', async ({ page }) => {
		await page.getByTestId('meal').locator('summary').click();
		await page.getByRole('button', { name: 'Edit' }).click();
		const sheet = page.getByRole('dialog', { name: 'Edit meal' });
		// Toast 50 g → 100 g: its numbers double (130 → 260 kcal).
		await sheet.locator('.item').first().getByLabel('Grams').fill('100');
		await expect(sheet.locator('.totals')).toContainText('332 kcal');
		await sheet.getByRole('button', { name: 'Save meal' }).click();
		await expect(sheet).toBeHidden();
		await expect(dayTotal(page)).toContainText('332 kcal');
		await expect(page.getByTestId('meal')).toHaveCount(1);
	});

	test('"Eat this again" duplicates the meal', async ({ page }) => {
		await page.getByTestId('meal').locator('summary').click();
		await page.getByRole('button', { name: 'Eat this again' }).click();
		await expect(page.getByTestId('meal')).toHaveCount(2);
		await expect(dayTotal(page)).toContainText('664 kcal');
		await expect(page.getByTestId('meal').nth(1)).toContainText('Eaten again');
	});

	test('delete removes a meal after confirmation', async ({ page }) => {
		const meal = page.getByTestId('meal').first();
		await meal.locator('summary').click();
		await meal.getByRole('button', { name: 'Delete' }).click();
		await meal.getByRole('button', { name: 'Delete meal' }).click();
		await expect(page.getByTestId('meal')).toHaveCount(1);
		await expect(dayTotal(page)).toContainText('332 kcal');
	});
});

test.describe('with a mock AI provider', () => {
	const port = randomPort();
	let server: ChildProcess;
	let mock: Server;
	let lastBody = '';
	test.use({ baseURL: `http://127.0.0.1:${port}` });

	test.beforeAll(async () => {
		const reply = {
			items: [
				{ food: 'fish', amount: 1, unit: 'piece', grams: 120, cooking: 'none', mentions_oil: false,
				  estimate: { kcal: 170, protein: 26, carbs: 2, fat: 7 } },
				{ food: 'cooked white rice', amount: 1, unit: 'small cup', grams: 150, cooking: 'none', mentions_oil: false,
				  estimate: { kcal: 195, protein: 4, carbs: 43, fat: 0.4 } },
				{ food: 'durian', amount: 1, unit: 'slice', grams: 110, cooking: 'none', mentions_oil: false,
				  estimate: { kcal: 162, protein: 1.6, carbs: 30, fat: 5.8 } }
			],
			assumptions: "The fish preparation isn't stated.",
			confidence: 'low'
		};
		mock = createServer((req, res) => {
			let body = '';
			req.on('data', (c) => (body += c));
			req.on('end', () => {
				lastBody = body;
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
			CUPFUL_PROVIDER_LOCAL: 'true',
			CUPFUL_PROVIDER_VISION: 'true'
		});
	});
	test.afterAll(() => {
		server?.kill();
		mock?.close();
	});
	test.beforeEach(({ page }) => login(page));

	test('estimate → review with sources and a choice → ×1.5 → saved totals are 1.5×', async ({ page }) => {
		// A photo carrying GPS EXIF: what's uploaded must be a re-encoded JPEG without it.
		const jpeg = Buffer.from(
			await page.evaluate(async () => {
				const c = Object.assign(document.createElement('canvas'), { width: 2400, height: 1200 });
				c.getContext('2d')!.fillRect(0, 0, 10, 10);
				const b = await new Promise<Blob>((r) => c.toBlob((b) => r(b!), 'image/jpeg'));
				return [...new Uint8Array(await b.arrayBuffer())];
			})
		);
		const photo = Buffer.concat([jpeg.subarray(0, 2), exifApp1(), jpeg.subarray(2)]);
		await page.locator('input[type=file]').setInputFiles({ name: 'meal.jpg', mimeType: 'image/jpeg', buffer: photo });
		await expect(page.getByRole('img', { name: 'Meal attached for the estimate' })).toBeVisible();

		await page.getByLabel('What did you eat?').fill('fish with rice and a slice of durian');
		await page.getByRole('button', { name: 'Estimate' }).click();
		const sheet = page.getByRole('dialog', { name: 'Review estimate' });
		await expect(sheet).toBeVisible();

		// The image the provider received: a JPEG, downscaled, with no EXIF left.
		const url: string = JSON.parse(lastBody).messages[1].content[1].image_url.url;
		const sent = Buffer.from(url.split(',')[1], 'base64');
		expect(url.startsWith('data:image/jpeg;base64,')).toBe(true);
		expect(sent.includes(Buffer.from('Exif'))).toBe(false);
		expect(sent.includes(Buffer.from('GPS-SECRET'))).toBe(false);
		expect(jpegSize(sent)).toEqual({ width: 1600, height: 800 });
		await expect(sheet.getByText('Database').first()).toBeVisible();
		await expect(sheet.getByText('AI estimate').first()).toBeVisible();
		await expect(sheet.getByText("Assumed: The fish preparation isn't stated.")).toBeVisible();
		await expect(sheet.getByText('Confidence: low')).toBeVisible();

		const choice = sheet.getByRole('group', { name: 'Which fish?' });
		await expect(choice).toBeVisible();
		const before = await kcalOf(await sheet.locator('.totals').innerText());
		await choice.getByRole('radio').nth(1).check();
		await expect.poll(async () => kcalOf(await sheet.locator('.totals').innerText())).not.toBe(before);
		const base = await kcalOf(await sheet.locator('.totals').innerText());
		await noSideScroll(page);

		await sheet.getByText('×1.5').click();
		await expect(sheet.locator('.totals')).toContainText('(×1.5)');
		await sheet.getByRole('button', { name: 'Save meal' }).click();
		await expect(sheet).toBeHidden();

		await expect(page.getByTestId('meal')).toContainText('×1.5');
		await expect(page.getByTestId('meal')).toContainText('AI estimate');
		const saved = await kcalOf(await dayTotal(page).innerText());
		expect(Math.abs(saved - base * 1.5)).toBeLessThanOrEqual(1);
		// The photo is gone after saving.
		await expect(page.getByRole('img', { name: 'Meal attached for the estimate' })).toHaveCount(0);
	});
});

test.describe('numbers and my products', () => {
	test.describe.configure({ mode: 'serial' });
	const port = randomPort();
	let server: ChildProcess;
	let mock: Server;
	test.use({ baseURL: `http://127.0.0.1:${port}` });

	test.beforeAll(async () => {
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

	test('use my product + remember: the same phrase then resolves to it (needs backend)', async ({ page }) => {
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

	test('a second product takes over the phrase; the earlier meal is unchanged (needs backend)', async ({ page }) => {
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

		const next = await estimate(page);
		await expect(next.locator('.item').first().getByText('Database')).toBeVisible();
		await expect(next.locator('.totals')).toContainText('56 kcal'); // B's 150 kcal / 40 g at the model's 15 g; no serving text, so no scoop weight
		await next.getByRole('button', { name: 'Cancel' }).click();
		expect(await meals(page)).toEqual(before);
	});
});

async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

/** A JPEG APP1 segment: EXIF (little-endian TIFF) with a GPS IFD holding an ASCII latitude ref. */
function exifApp1(): Buffer {
	const tiff = Buffer.alloc(44 + 10);
	tiff.write('II', 0);
	tiff.writeUInt16LE(42, 2);
	tiff.writeUInt32LE(8, 4);
	tiff.writeUInt16LE(1, 8); // IFD0: one entry
	tiff.writeUInt16LE(0x8825, 10); // GPS IFD pointer
	tiff.writeUInt16LE(4, 12);
	tiff.writeUInt32LE(1, 14);
	tiff.writeUInt32LE(26, 18);
	tiff.writeUInt32LE(0, 22);
	tiff.writeUInt16LE(1, 26); // GPS IFD: one entry
	tiff.writeUInt16LE(0x0001, 28); // GPSLatitudeRef, ASCII, 2 bytes inline
	tiff.writeUInt16LE(2, 30);
	tiff.writeUInt32LE(2, 32);
	tiff.write('N\0', 36);
	tiff.writeUInt32LE(0, 40);
	tiff.write('GPS-SECRET', 44); // a marker to look for in the upload
	const body = Buffer.concat([Buffer.from('Exif\0\0'), tiff]);
	const head = Buffer.from([0xff, 0xe1, 0, 0]);
	head.writeUInt16BE(body.length + 2, 2);
	return Buffer.concat([head, body]);
}

/** Width and height from a JPEG's SOF0/SOF2 marker. */
function jpegSize(b: Buffer) {
	for (let i = 2; i < b.length; ) {
		const marker = b[i + 1];
		if (marker === 0xc0 || marker === 0xc2) return { height: b.readUInt16BE(i + 5), width: b.readUInt16BE(i + 7) };
		i += 2 + b.readUInt16BE(i + 2);
	}
	return null;
}
