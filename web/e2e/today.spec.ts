// SPDX-License-Identifier: AGPL-3.0-or-later
// Today meal flow. Each describe starts its own server (the debug binary the shared webServer just
// built) on a fresh data dir, so these meals don't mix with other specs' and the AI path can have
// a provider: a tiny mock OpenAI-compatible server answering with a canned estimate.
import { expect, test, type Page } from '@playwright/test';
import { type ChildProcess } from 'node:child_process';
import { createServer, type Server } from 'node:http';
import { dayTotal, fillItem, kcalOf, login, noSideScroll, randomPort, resetSavedAuth, startCupful } from './helpers/today';

test.describe('without an AI provider', () => {
	test.describe.configure({ mode: 'serial' });
	const port = randomPort();
	let server: ChildProcess;
	test.use({ baseURL: `http://127.0.0.1:${port}` });
	test.beforeAll(async () => {
		server = await startCupful(port, {});
		resetSavedAuth();
	});
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

	test('Repeat opens Review and logs a copy', async ({ page }) => {
		// Depends on the previous edit test: the meal is 332 kcal.
		await page.getByTestId('meal').locator('summary').click();
		await page.getByTestId('meal').getByRole('button', { name: 'Repeat', exact: true }).click();
		const sheet = page.getByRole('dialog', { name: /Repeat meal for/ });
		await expect(sheet).toBeVisible();
		await expect(sheet.locator('.totals')).toContainText('332 kcal');
		await sheet.getByRole('button', { name: 'Save meal' }).click();
		await expect(sheet).toBeHidden();
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
		await picker.getByLabel('Search recent meals').fill('no-such-meal');
		await expect(picker.getByText(/No recent meals match/)).toBeVisible();
		await picker.getByRole('button', { name: 'Clear search' }).click();
		await picker.getByLabel('Search recent meals').fill('yesterday');
		await picker.getByRole('button', { name: /yesterday oats/ }).click();
		const review = page.getByRole('dialog', { name: 'Repeat meal for Today' });
		await expect(review.locator('.totals')).toContainText('380 kcal');
		await review.getByRole('button', { name: 'Cancel' }).click();
		await expect(review).toBeHidden();
		await expect(picker).toBeVisible();
		await expect(picker.getByLabel('Search recent meals')).toHaveValue('yesterday');
		await picker.getByRole('button', { name: /yesterday oats/ }).click();
		const reviewAgain = page.getByRole('dialog', { name: 'Repeat meal for Today' });
		await expect(reviewAgain.locator('.totals')).toContainText('380 kcal');
		await reviewAgain.getByRole('button', { name: 'Save meal' }).click();
		await expect(reviewAgain).toBeHidden();
		await expect(page.getByTestId('meal').filter({ hasText: 'yesterday oats' })).toHaveCount(1);
	});

	test('Copy a past meal to today', async ({ page }) => {
		await page.getByRole('button', { name: 'Previous day' }).click();
		await page.getByLabel('What did you eat?').fill('copy seed');
		await page.getByRole('button', { name: 'Enter by hand' }).click();
		const entry = page.getByRole('dialog', { name: 'Enter by hand' });
		await fillItem(page, 0, ['Copy seed', '100', '250', '10', '30', '8']);
		await entry.getByRole('button', { name: 'Save meal' }).click();
		await expect(entry).toBeHidden();

		const meal = page.getByTestId('meal').filter({ hasText: 'copy seed' });
		await meal.locator('summary').click();
		await meal.getByRole('button', { name: 'Copy to today' }).click();
		const review = page.getByRole('dialog', { name: 'Repeat meal for Today' });
		await expect(review).toBeVisible();
		await review.getByRole('button', { name: 'Save meal' }).click();
		await expect(review).toBeHidden();
		await expect(page.getByText('Copied to today.')).toBeVisible();
		await page.getByRole('button', { name: 'View today' }).click();
		await expect(page.getByTestId('meal').filter({ hasText: 'copy seed' })).toHaveCount(1);
	});

	test('Repeat does not clear the composer draft', async ({ page }) => {
		await page.getByLabel('What did you eat?').fill('keep this description');
		await page.getByRole('button', { name: 'Repeat a meal' }).click();
		const picker = page.getByRole('dialog', { name: /Repeat onto Today/ });
		await picker.locator('ul.list button').first().click();
		const review = page.getByRole('dialog', { name: 'Repeat meal for Today' });
		await review.getByRole('button', { name: 'Save meal' }).click();
		await expect(review).toBeHidden();
		await expect(page.getByLabel('What did you eat?')).toHaveValue('keep this description');
		await expect(page.getByRole('button', { name: 'Repeat a meal' })).toBeFocused();
	});

	test('loads when navigator.onLine is false but the server answers', async ({ page }) => {
		// Android can keep navigator.onLine false for a while after Tailscale connects. The app
		// trusts an actual request, not the browser hint.
		await page.addInitScript(() => {
			Object.defineProperty(navigator, 'onLine', { get: () => false, configurable: true });
		});
		await page.goto('/');
		await expect(page.getByRole('heading', { name: 'Log a meal' })).toBeVisible();
		await expect(page.getByTestId('sync-bar')).toHaveCount(0);
	});

	test('does not flash offline when the browser reports a blip', async ({ page }) => {
		await page.evaluate(() => window.dispatchEvent(new Event('offline')));
		await page.waitForTimeout(2_000);
		await expect(page.getByTestId('sync-bar')).toHaveCount(0);
	});
});

test.describe('with a mock AI provider', () => {
	const port = randomPort();
	let server: ChildProcess;
	let mock: Server;
	let lastBody = '';
	test.use({ baseURL: `http://127.0.0.1:${port}` });

	test.beforeAll(async () => {
		resetSavedAuth();
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
