// SPDX-License-Identifier: AGPL-3.0-or-later
// Activity (manual logging, Shortcut ingest never overwriting manual values) and Trends.
// The ingest and trends tests need the M4 backend endpoints; the manual-logging test does not.
import { expect, request as pwRequest, test, type Page } from '@playwright/test';

async function signIn(page: Page) {
	await page.goto('/');
	await page.waitForURL(/\/(setup|login)$/);
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password').fill('correct horse');
	await page.getByRole('button', { name: page.url().endsWith('/setup') ? 'Create account' : 'Log in' }).click();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
}

async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

const stepsField = (page: Page) => page.getByLabel('Steps', { exact: true });
const stepsChip = (page: Page) => page.getByTestId('steps-source');

test.describe.configure({ mode: 'serial' });

// One login for the file: the server rate-limits logins.
let page: Page;
let baseURL: string;
test.beforeAll(async ({ browser }, info) => {
	baseURL = info.project.use.baseURL!;
	const ctx = await browser.newContext({ baseURL, viewport: { width: 360, height: 740 }, hasTouch: true });
	page = await ctx.newPage();
	await signIn(page);
});
test.afterAll(() => page.context().close());

test('log steps, gym, trained and weight; saved with source Manual', async () => {
	await page.goto('/');
	await page.getByRole('link', { name: 'Activity' }).click();
	await expect(page.getByRole('region', { name: 'Activity' })).toBeVisible();
	const unit = (await (await page.request.get('/api/profile')).json()).weight_unit as 'kg' | 'lb';

	await stepsField(page).fill('8123');
	await stepsField(page).blur();
	await expect(page.getByRole('status').filter({ hasText: 'Steps saved.' })).toHaveCount(1);
	await page.getByLabel('Gym minutes').fill('45');
	await page.getByLabel('Gym minutes').blur();
	await expect(page.getByRole('status').filter({ hasText: 'Gym minutes saved.' })).toHaveCount(1);
	const trainedMenu = page.locator('details.trained');
	await trainedMenu.locator('summary').click();
	await trainedMenu.getByRole('checkbox', { name: 'Chest' }).check();
	await expect(page.getByRole('status').filter({ hasText: 'Chest added.' })).toHaveCount(1);
	await trainedMenu.getByRole('checkbox', { name: 'Legs' }).check();
	await expect(page.getByRole('status').filter({ hasText: 'Legs added.' })).toHaveCount(1);
	await expect(trainedMenu.locator('summary')).toContainText('Trained: Chest, Legs');
	await page.keyboard.press('Escape');
	await expect(trainedMenu).not.toHaveAttribute('open', '');
	await expect(trainedMenu.locator('summary')).toBeFocused();
	await page.getByLabel(`Weigh-in (${unit})`).fill(unit === 'kg' ? '81.4' : '179.5');
	await page.getByLabel(`Weigh-in (${unit})`).blur();
	await expect(page.getByRole('status').filter({ hasText: 'Weigh-in saved.' })).toHaveCount(1);

	// The budget card on the tab refreshed with the new steps.
	await expect(page.getByRole('region', { name: 'Budget' })).toBeVisible();
	await noSideScroll(page);

	await page.reload();
	await expect(stepsField(page)).toHaveValue('8123');
	await expect(stepsChip(page)).toHaveText('Manual');
	await expect(page.getByLabel('Gym minutes')).toHaveValue('45');
	await expect(page.locator('details.trained summary')).toContainText('Trained: Chest, Legs');
	await page.locator('details.trained summary').click();
	await expect(page.getByRole('checkbox', { name: 'Chest' })).toBeChecked();
	await expect(page.getByRole('checkbox', { name: 'Back' })).not.toBeChecked();
	await page.getByRole('button', { name: 'Done' }).click();
	await expect(page.locator('details.trained')).not.toHaveAttribute('open', '');
	await expect(page.getByLabel(`Weigh-in (${unit})`)).toHaveValue(unit === 'kg' ? '81.4' : '179.5');
});

test('a Shortcut fills steps and never overwrites a manual value (needs ingest endpoints)', async () => {
	const request = await pwRequest.newContext({ baseURL });
	const created = await page.request.post('/api/ingest-tokens', {
		headers: { 'X-Cupful': '1' },
		data: { label: 'e2e' }
	});
	expect(created.ok()).toBe(true);
	const { token } = await created.json();

	const tz = (await (await page.request.get('/api/profile')).json()).timezone as string;
	const yesterday = new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(new Date(Date.now() - 864e5));
	const ingest = (steps: number) =>
		request.post('/api/ingest/activity', {
			headers: { Authorization: `Bearer ${token}` },
			data: { date: yesterday, steps }
		});

	expect((await (await ingest(4321)).json()).changed).toBe(true);
	await page.goto('/activity');
	await page.getByRole('button', { name: 'Previous day' }).click();
	await expect(stepsField(page)).toHaveValue('4321');
	await expect(stepsChip(page)).toHaveText('Shortcut');

	// A manual value wins, and a later Shortcut run does not change it.
	await stepsField(page).fill('5000');
	await stepsField(page).blur();
	await expect(stepsChip(page)).toHaveText('Manual');
	expect((await (await ingest(9999)).json()).changed).toBe(false);
	await page.reload();
	await page.getByRole('button', { name: 'Previous day' }).click();
	await expect(stepsField(page)).toHaveValue('5000');
	await expect(stepsChip(page)).toHaveText('Manual');
});

test('ingest tokens: create shows the token once, revoke asks first (needs token endpoints)', async () => {
	await page.getByRole('link', { name: 'Settings' }).click();
	const tokens = page.getByRole('region', { name: 'Ingest tokens' });
	await tokens.getByLabel('New token name').fill('Phone');
	await tokens.getByRole('button', { name: 'Create token' }).click();
	const shown = tokens.getByTestId('new-token');
	await expect(shown).toHaveText(/^cupful_/);
	await expect(tokens.getByText('Test it with curl')).toBeVisible();
	await tokens.getByRole('button', { name: 'Done' }).click();
	await expect(shown).toHaveCount(0);

	await tokens.getByRole('button', { name: 'Revoke Phone' }).click();
	await expect(tokens.getByText('Automations using it stop working.')).toBeVisible();
	await tokens.getByRole('button', { name: 'Revoke token' }).click();
	await expect(tokens.getByRole('button', { name: 'Revoke Phone' })).toHaveCount(0);
});

test('Trends: charts, range switch remembered, empty state, no side scroll', async () => {
	await page.getByRole('link', { name: 'Trends' }).click();
	await expect(page.getByRole('heading', { name: 'Trends', level: 1 })).toBeVisible();

	// Weighed in earlier in this file, so the weight chart is drawn.
	const weight = page.getByRole('region', { name: 'Weight' });
	await expect(weight.locator('.plot svg').first()).toBeVisible();
	await expect(weight.locator('.plot')).toHaveAttribute('aria-label', /Weigh-ins .* 1 in all/);
	await noSideScroll(page);

	await page.locator('label', { hasText: '90 days' }).click();
	await expect(page.getByRole('radio', { name: '90 days' })).toBeChecked();
	await page.reload();
	await expect(page.getByRole('radio', { name: '90 days' })).toBeChecked();
	await noSideScroll(page);

	// Other spec files share the server, so meals may exist: a chart or its empty state, never neither.
	const intake = page.getByRole('region', { name: 'Intake', exact: true });
	await expect(
		intake.locator('.plot svg').first().or(intake.getByText('No meals logged in this range'))
	).toBeVisible();
});

test('Activity uses the desktop width: budget beside the stat cards', async () => {
	await page.setViewportSize({ width: 1280, height: 900 });
	await page.goto('/activity');
	await expect(page.getByRole('heading', { name: 'Activity', level: 1 })).toBeVisible();
	await expect(page.getByRole('region', { name: 'Activity' })).toBeVisible();

	const steps = await page.locator('.stat-steps').boundingBox();
	const budget = await page.getByRole('region', { name: 'Budget' }).boundingBox();
	expect(steps).not.toBeNull();
	expect(budget).not.toBeNull();
	expect(budget!.x).toBeGreaterThan(steps!.x + steps!.width);
	await noSideScroll(page);
});
