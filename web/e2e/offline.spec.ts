// SPDX-License-Identifier: AGPL-3.0-or-later
// M6 offline behavior (PLAN.md §12, §14 M6): the IndexedDB write queue in front of /api/sync.
// Each test runs against this spec's own `cupful` binary + data dir so the shared Playwright
// server and its database stay untouched.
import { expect, test } from '@playwright/test';
import {
	USER_A,
	USER_B,
	breakNewestQueueOp,
	cacheCountForUser,
	currentUserId,
	localDate,
	login,
	queueCount,
	randomPort,
	saveProfile,
	seedDemoUser,
	seedMeal,
	setPageOffline,
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
	seedDemoUser(server); // the second account for the account-switch tests
});

test.afterAll(() => server?.stop());

test.beforeEach(async ({ page }) => {
	await login(page);
});

/** Expands the meal whose summary contains `description` and clicks "Eat this again". */
async function repeatMeal(page: import('@playwright/test').Page, description: string) {
	const meal = page.getByTestId('meal').filter({ hasText: description });
	await meal.locator('summary').click();
	await meal.getByRole('button', { name: 'Eat this again' }).click();
}

/** Settings → Sync op rows. */
const syncOpRows = (page: import('@playwright/test').Page) =>
	page.locator('[data-testid="sync-op"]');

test('a repeat meal and a weigh-in queue offline, survive a reload, and sync on reconnect', async ({
	page,
}) => {
	await saveProfile(page);
	const date = localDate();
	await seedMeal(page, date, 'offline seed bowl');
	await page.reload();
	await expect(page.getByTestId('meal').filter({ hasText: 'offline seed bowl' })).toHaveCount(1);

	// Cache the day/activity read before going offline (phones open Activity on its own tab).
	await page.getByRole('link', { name: 'Activity' }).click();
	await expect(page.getByLabel(/Weigh-in/)).toBeVisible();
	await page.getByRole('link', { name: 'Trends' }).click();
	await expect(page.getByText(/kcal eaten/)).toBeVisible();
	await page.getByRole('link', { name: 'Today' }).click();
	await page.waitForURL((url) => url.pathname === '/');
	await waitForController(page);
	await expect(page.getByTestId('meal').filter({ hasText: 'offline seed bowl' })).toHaveCount(1);

	await page.context().setOffline(true);
	const bar = page.getByTestId('sync-bar');
	await expect(bar).toContainText('Offline. Changes sync when you reconnect.');

	await repeatMeal(page, 'offline seed bowl');
	await expect(page.getByTestId('meal').filter({ hasText: 'offline seed bowl' })).toHaveCount(2);
	await expect(bar).toContainText(/1 change waiting/);

	await page.getByRole('link', { name: 'Activity' }).click();
	const weight = page.getByLabel(/Weigh-in/);
	await weight.fill('81');
	await weight.blur();
	await expect(weight).toHaveValue('81');
	await expect(bar).toContainText(/2 changes waiting/);

	// Offline reload: the service worker serves the shell, and the cached day + queued overlay
	// still render.
	await page.getByRole('link', { name: 'Today' }).click();
	await page.waitForURL((url) => url.pathname === '/');
	await page.waitForFunction(() => !!navigator.serviceWorker.controller);
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(page.getByTestId('meal').filter({ hasText: 'offline seed bowl' })).toHaveCount(2);

	// Activity and Trends open from the cache on a cold start in airplane mode.
	await page.getByRole('link', { name: 'Activity' }).click();
	await expect(page.getByLabel(/Weigh-in/)).toHaveValue('81');
	await page.getByRole('link', { name: 'Trends' }).click();
	await expect(page.getByText(/kcal eaten/)).toBeVisible();
	await page.getByRole('link', { name: 'Today' }).click();

	await page.context().setOffline(false);
	await expect(bar).toBeHidden({ timeout: 30_000 });

	const meals = (await (await page.request.get(`/api/meals?date=${date}`)).json()) as {
		meals: { description: string }[];
	};
	expect(meals.meals.filter((m) => m.description === 'offline seed bowl')).toHaveLength(2);
	const day = (await (await page.request.get(`/api/days/${date}`)).json()) as {
		weight_kg: number | null;
	};
	expect(day.weight_kg).toBeCloseTo(81, 1);
});

test('a deliberately broken op is visible in Settings and can be discarded', async ({ page }) => {
	const date = localDate();
	await seedMeal(page, date, 'broken op seed');
	await page.reload();
	await expect(page.getByTestId('meal').filter({ hasText: 'broken op seed' })).toHaveCount(1);

	await page.context().setOffline(true);
	await repeatMeal(page, 'broken op seed');
	const bar = page.getByTestId('sync-bar');
	await expect(bar).toContainText(/1 change waiting/);

	await breakNewestQueueOp(page, 'broken');
	await page.context().setOffline(false);
	await expect(bar).toContainText('1 changes need attention', {
		timeout: 30_000,
	});

	await page.getByTestId('sync-bar').getByRole('link').click();
	await expect(page).toHaveURL(/#sync$/);
	const section = page.getByTestId('sync-section');
	await expect(section.getByRole('heading', { name: 'Sync' })).toBeVisible();

	const rows = syncOpRows(page);
	await expect(rows).toHaveCount(1);
	await expect(rows.getByRole('alert')).toBeVisible();
	await rows.getByRole('button', { name: /^Discard/ }).click();
	await expect(rows).toHaveCount(0);
	await expect(section).toContainText('All changes synced.');
	expect(await queueCount(page)).toBe(0);
});

test('another tab logging in as a different account does not flush this tab queue', async ({
	page,
	context,
}) => {
	const date = localDate();
	await seedMeal(page, date, 'stale tab seed');
	await page.reload();
	await expect(page.getByTestId('meal').filter({ hasText: 'stale tab seed' })).toHaveCount(1);

	const other = await context.newPage();
	await other.goto('/');
	await expect(other.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();

	// The other tab switches accounts through the API, which (unlike the UI) does not discard the
	// first tab's queue: that is exactly what the queue's identity check must refuse to flush.
	expect(
		(
			await other.request.post('/api/auth/logout', {
				headers: { 'X-Cupful': '1' },
			})
		).ok()
	).toBe(true);
	expect(
		(
			await other.request.post('/api/auth/login', {
				headers: { 'X-Cupful': '1' },
				data: USER_B,
			})
		).ok()
	).toBe(true);
	await other.goto('/');
	await expect(other.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();

	// Tab 1 still has A in memory but the cookie is now B. Queue a write while offline, then
	// reconnect: the flush must refuse because the server says the session is B.
	await context.setOffline(true);
	await repeatMeal(page, 'stale tab seed');
	await expect(page.getByTestId('sync-bar')).toContainText(/1 change waiting/);

	let syncPosts = 0;
	await page.route('**/api/sync', async (route) => {
		if (route.request().method() === 'POST') syncPosts++;
		await route.continue();
	});

	await context.setOffline(false);
	await page.evaluate(() => window.dispatchEvent(new Event('online')));
	await page.getByRole('link', { name: 'Settings' }).click();
	await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
	await page.getByTestId('sync-section').getByRole('button', { name: 'Sync now' }).click();
	await expect(page.getByTestId('sync-bar')).toContainText(
		'Changes on this device belong to another account.',
		{ timeout: 30_000 }
	);
	expect(syncPosts).toBe(0);

	// B's own reads never contain A's meal.
	const bMeals = (await (await other.request.get(`/api/meals?date=${date}`)).json()) as {
		meals: { description: string }[];
	};
	expect(bMeals.meals.map((m) => m.description)).not.toContain('stale tab seed');
});

test('logout warns about queued changes: cancel keeps A, accepting clears A, and B sees none of A', async ({
	page,
}) => {
	const date = localDate();
	const secret = 'account switch secret meal';
	await seedMeal(page, date, secret);
	await page.reload();
	await expect(page.getByTestId('meal').filter({ hasText: secret })).toHaveCount(1);
	const userA = await currentUserId(page);

	// Keep the queued op from flushing on reconnect; everything else stays online.
	await page.route('**/api/sync', (route) => route.abort('blockedbyclient'));
	await page.context().setOffline(true);
	await repeatMeal(page, secret);
	await expect(page.getByTestId('sync-bar')).toContainText(/1 change waiting/);

	await page.getByRole('link', { name: 'Settings' }).click();
	await expect(syncOpRows(page)).toHaveCount(1);

	// Offline, logging out is refused before anything local is deleted.
	await page.getByRole('button', { name: 'Log out', exact: true }).click();
	await expect(page.getByText(/Logging out needs a connection/)).toBeVisible();
	await expect(page.getByText(/Logged in as/)).toContainText(USER_A.username);
	expect(await queueCount(page)).toBeGreaterThan(0);
	await page.context().setOffline(false);

	// Cancel: A stays signed in and the queued change stays.
	await page.getByRole('button', { name: 'Log out', exact: true }).click();
	await expect(page.getByRole('group', { name: 'Confirm logout' })).toContainText(/1\s+change/);
	await page.getByRole('button', { name: 'Cancel' }).click();
	await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible();
	await expect(page.getByText(/Logged in as/)).toContainText(USER_A.username);
	expect(await queueCount(page)).toBeGreaterThan(0);

	// Accept: the warning is the last chance; logging out discards A's queue and caches.
	await page.getByRole('button', { name: 'Log out', exact: true }).click();
	await page.getByRole('button', { name: 'Log out and discard' }).click();
	await page.waitForURL(/\/login$/);
	expect(await queueCount(page)).toBe(0);
	expect(await cacheCountForUser(page, userA)).toBe(0);

	// B's Today shows none of A's meals, in the UI or over the API.
	await page.getByLabel('Username').fill(USER_B.username);
	await page.getByLabel('Password').fill(USER_B.password);
	await page.getByRole('button', { name: 'Log in' }).click();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	await expect(page.getByText(secret)).toHaveCount(0);
	const bMeals = (await (await page.request.get(`/api/meals?date=${date}`)).json()) as {
		meals: { description: string }[];
	};
	expect(bMeals.meals.map((m) => m.description)).not.toContain(secret);
});

test('an expired session keeps A’s queue and warns before B discards it', async ({ page }) => {
	const date = localDate();
	const secret = 'expired session secret';
	await seedMeal(page, date, secret);
	await page.reload();
	await expect(page.getByTestId('meal').filter({ hasText: secret })).toHaveCount(1);
	const userA = await currentUserId(page);

	await page.route('**/api/sync', (route) => route.abort('blockedbyclient'));
	await page.context().setOffline(true);
	await repeatMeal(page, secret);
	await expect(page.getByTestId('sync-bar')).toContainText(/1 change waiting/);
	await page.context().clearCookies();
	await page.context().setOffline(false);

	// The server session is gone; the guard asks for a sign-in, not another account.
	await expect(page.getByTestId('sync-bar')).toContainText('Sign in again to sync your changes.', {
		timeout: 30_000
	});
	expect(await queueCount(page)).toBe(1);

	// The full load lets the app see it is logged out; the queue survives.
	await page.getByTestId('sync-bar').getByRole('link', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'Log in', level: 1 })).toBeVisible();
	expect(await queueCount(page)).toBe(1);

	await page.getByLabel('Username').fill(USER_B.username);
	await page.getByLabel('Password').fill(USER_B.password);
	await page.getByRole('button', { name: 'Log in' }).click();
	await expect(page.getByRole('group', { name: 'Confirm account switch' })).toContainText(/1\s+change/);

	// Cancel: still at the login screen, A's queue untouched.
	await page.getByRole('button', { name: 'Cancel' }).click();
	expect(await queueCount(page)).toBe(1);

	// Accept: B logs in, A's queue and cache are gone.
	await page.getByRole('button', { name: 'Log in' }).click();
	await page.getByRole('button', { name: 'Log in and discard' }).click();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	expect(await queueCount(page)).toBe(0);
	expect(await cacheCountForUser(page, userA)).toBe(0);
});
