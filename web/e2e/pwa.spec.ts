// SPDX-License-Identifier: AGPL-3.0-or-later
// M6 PWA shell (PLAN.md §12, §14 M6): manifest and icons are served, the service worker installs
// and controls the tab, and the cached app shell opens offline, including at a deep link.
import { expect, test } from '@playwright/test';
import {
	localDate,
	login,
	randomPort,
	seedMeal,
	saveProfile,
	startCupful,
	waitForController,
	type CupfulServer,
} from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });

const port = randomPort();
test.use({ baseURL: `http://127.0.0.1:${port}` });

let server: CupfulServer;

test.beforeAll(async () => {
	server = await startCupful(port);
});

test.afterAll(() => server?.stop());

test('manifest, icons, service worker and offline shell incl. a deep link', async ({ page }) => {
	await login(page);
	await saveProfile(page);
	await seedMeal(page, localDate(), 'pwa shell seed');
	await page.reload();
	await waitForController(page);

	const manifest = await page.request.get('/manifest.webmanifest');
	expect(manifest.status()).toBe(200);
	expect(manifest.headers()['content-type']).toMatch(/manifest\+json|application\/json/);
	const body = (await manifest.json()) as {
		name: string;
		display: string;
		icons: { src: string }[];
	};
	expect(body.name).toBe('Cupful');
	expect(body.display).toBe('standalone');

	for (const icon of ['/icons/icon-192.png', '/icons/icon-512.png', '/icons/maskable-512.png']) {
		const res = await page.request.get(icon);
		expect(res.status(), icon).toBe(200);
		expect(res.headers()['content-type'], icon).toBe('image/png');
		expect(body.icons.map((i) => i.src)).toContain(icon);
	}

	await page.context().setOffline(true);
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(page.getByTestId('meal').filter({ hasText: 'pwa shell seed' })).toHaveCount(1);

	await page.goto('/trends');
	await expect(page.getByRole('heading', { name: /Today|Trends/, level: 1 })).toBeVisible();
	expect(new URL(page.url()).pathname).toBe('/trends');
});
