// SPDX-License-Identifier: AGPL-3.0-or-later
// Activity: insight cards, the week strip, the workouts panel, and Settings -> Counts as gym.
// /api/workouts and /api/google are MOCKED with route interception (the workouts API is built in
// parallel and Google is not configured on the e2e server); everything else is the real server.
import { expect, test, type Page } from '@playwright/test';
import { login, saveProfile, seedDay, startCupful, type CupfulServer } from './helpers/server.ts';

test.describe.configure({ mode: 'serial' });

const addDays = (iso: string, n: number) => {
	const d = new Date(`${iso}T00:00:00Z`);
	d.setUTCDate(d.getUTCDate() + n);
	return d.toISOString().slice(0, 10);
};
const TODAY = new Date().toLocaleDateString('en-CA');
const YESTERDAY = addDays(TODAY, -1);

const workout = (id: number, date: string, o: Record<string, unknown> = {}) => ({
	id,
	date,
	start: `${date}T07:30:00Z`,
	end: `${date}T08:01:00Z`,
	type: 'RUNNING',
	type_label: 'Running',
	name: 'Morning run',
	active_minutes: 31,
	kcal: 412,
	distance_m: 5200,
	avg_hr: 148,
	counts_as_gym: false,
	...o
});

let server: CupfulServer;
let page: Page;
test.beforeAll(async ({ browser }) => {
	server = await startCupful();
	const ctx = await browser.newContext({ baseURL: server.base, viewport: { width: 360, height: 740 }, hasTouch: true });
	page = await ctx.newPage();
	await login(page);
	await saveProfile(page);
	await seedDay(page, TODAY, { steps: 6640 });
	await seedDay(page, YESTERDAY, { steps: 9100, gym_minutes: 40, weight_kg: 80.4 });
	await page.route('**/api/workouts?**', (route) =>
		route.fulfill({
			json: {
				workouts: [
					workout(1, YESTERDAY, { name: null, type: 'STRENGTH_TRAINING', type_label: 'Strength training', counts_as_gym: true, distance_m: null, avg_hr: 120 }),
					workout(2, TODAY),
					workout(3, TODAY, { name: 'Evening walk', type: 'WALKING', type_label: 'Walking', distance_m: 2300, kcal: null, start: `${TODAY}T18:00:00Z` })
				]
			}
		})
	);
});
test.afterAll(async () => {
	await page.context().close();
	server.stop();
});

const noSideScroll = async () =>
	expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(0);

test('cards, week strip and workouts panel; tapping a day selects it (workouts mocked)', async () => {
	await page.goto('/activity');
	await expect(page.getByRole('heading', { name: 'This week' })).toBeVisible();
	// Steps card: 6,640 of a typical 8,000 is 83%.
	await expect(page.locator('.fact', { hasText: 'of your typical day' })).toContainText('83%');
	// Today's workouts, one with the name and one with a fallback.
	const panel = page.getByRole('region', { name: 'Workouts' });
	await expect(panel.getByTestId('workout')).toHaveCount(2);
	await expect(panel.getByText('Morning run')).toBeVisible();
	await expect(panel.getByText('5.2 km')).toBeVisible();
	await expect(panel.getByText('Counts as gym')).toHaveCount(0);
	await expect(page.getByTestId('activity-budget')).toContainText('kcal budget');
	// Seven day buttons; gym shows on yesterday.
	const cols = page.getByRole('region', { name: 'This week' }).getByRole('button');
	await expect(cols).toHaveCount(7);
	await expect(page.getByRole('button', { name: /^Yesterday: 9,100 steps, gym, 1 workout/ })).toBeVisible();
	await noSideScroll();

	await page.getByRole('button', { name: /^Yesterday:/ }).click();
	await expect(page.locator('#day-picker')).toHaveValue(YESTERDAY);
	await expect(panel.getByTestId('workout')).toHaveCount(1);
	await expect(panel.getByText('Strength training').first()).toBeVisible();
	await expect(panel.getByText('Counts as gym')).toBeVisible();
	await expect(page.getByRole('button', { name: /^Yesterday:/ })).toHaveAttribute('aria-current', 'date');
	const back = await page.getByRole('button', { name: 'Back to today' }).boundingBox();
	const picker = await page.getByLabel('Date').boundingBox();
	expect(back).not.toBeNull();
	expect(picker).not.toBeNull();
	expect(back!.x).toBeLessThan(picker!.x);
	expect(Math.abs(back!.y - picker!.y)).toBeLessThan(8);
	await noSideScroll();
});

test('a busy day shows six workouts and "Show N more"', async () => {
	await page.unroute('**/api/workouts?**');
	await page.route('**/api/workouts?**', (route) =>
		route.fulfill({ json: { workouts: Array.from({ length: 9 }, (_, i) => workout(100 + i, TODAY, { name: `Session ${i + 1}` })) } })
	);
	await page.goto('/activity');
	const panel = page.getByRole('region', { name: 'Workouts' });
	await expect(panel.getByTestId('workout')).toHaveCount(6);
	await panel.getByRole('button', { name: 'Show 3 more' }).click();
	await expect(panel.getByTestId('workout')).toHaveCount(9);
	await expect(panel.getByRole('button', { name: 'Show fewer' })).toHaveAttribute('aria-expanded', 'true');
	await noSideScroll();
});

test('an empty day says so', async () => {
	await page.goto('/activity');
	await page.getByLabel('Previous day').click();
	await page.getByLabel('Previous day').click();
	await expect(page.getByText('No workouts synced for this day.')).toBeVisible();
});

test('selecting a week date keeps the page in place', async () => {
	await page.goto('/activity');
	const week = page.getByRole('region', { name: 'This week' });
	await expect(week.getByRole('button')).toHaveCount(7);

	await page.route(/\/api\/days(\/|\?)/, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		return route.continue();
	});
	await page.getByRole('button', { name: /^Yesterday:/ }).click();
	// The old window still contains yesterday, so the strip and cards stay on screen while the
	// new day loads instead of collapsing into skeletons.
	await expect(week.getByRole('button')).toHaveCount(7);
	await expect(page.getByLabel('Weigh-in (kg)')).toHaveValue('80.4');
	await page.unroute(/\/api\/days(\/|\?)/);
});

test('Activity carries the last weigh-in forward', async () => {
	await page.goto('/activity');
	await expect(page.getByLabel('Weigh-in (kg)')).toHaveValue('80.4');
	await expect(page.getByText('Carried', { exact: true })).toBeVisible();
	await expect(page.getByText(/Carried from Yesterday\./)).toBeVisible();
});

test('Today syncs steps when Google Health is connected', async () => {
	let syncs = 0;
	await page.route(/\/api\/google$/, (route) =>
		route.request().method() === 'GET'
			? route.fulfill({
					json: {
						configured: true,
						connected: true,
						needs_reconnect: false,
						last_sync_at: null,
						last_error: null,
						gym_types: [],
						exercise_types: []
					}
				})
			: route.continue()
	);
	await page.route('**/api/google/sync', (route) => {
		syncs++;
		return route.fulfill({ json: { days: 1, changed: 1, workouts: 0 } });
	});

	await page.setViewportSize({ width: 1280, height: 900 });
	await page.goto('/');
	await page.getByRole('button', { name: 'Sync steps' }).click();
	await expect(page.getByText('Steps synced.')).toBeVisible();
	expect(syncs).toBe(1);

	await page.unroute(/\/api\/google$/);
	await page.unroute('**/api/google/sync');
	await page.setViewportSize({ width: 360, height: 740 });
});

test('Settings: pick gym workout types and save (google mocked)', async () => {
	let body: unknown = null;
	await page.route(/\/api\/google$/, (route) =>
		route.request().method() === 'GET'
			? route.fulfill({
					json: {
						configured: true,
						connected: true,
						needs_reconnect: false,
						last_sync_at: null,
						last_error: null,
						gym_types: ['STRENGTH_TRAINING'],
						exercise_types: [
							{ value: 'RUNNING', label: 'Running' },
							{ value: 'STRENGTH_TRAINING', label: 'Strength training' },
							{ value: 'YOGA', label: 'Yoga' }
						]
					}
				})
			: route.continue()
	);
	await page.route('**/api/google/gym-types', (route) => {
		body = route.request().postDataJSON();
		return route.fulfill({ json: {} });
	});
	await page.goto('/settings');
	const strength = page.getByRole('checkbox', { name: 'Strength training' });
	await expect(strength).toBeChecked();
	await expect(page.getByText('Anything you enter by hand wins.')).toBeVisible();
	await page.getByRole('checkbox', { name: 'Yoga' }).check();
	await strength.uncheck();
	await page.getByRole('button', { name: 'Save gym types' }).click();
	await expect(page.getByText('Gym workout types saved.')).toBeVisible();
	expect(body).toEqual({ types: ['YOGA'] });
	await noSideScroll();
});
