// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, test, type Page } from '@playwright/test';
import { openRow } from './helpers/settings.ts';

async function signIn(page: Page) {
	await page.goto('/');
	// A new browser context is logged out: setup on a fresh server, else login.
	await page.waitForURL(/\/(setup|login)$/);
	if (page.url().endsWith('/setup')) {
		await page.getByLabel('Username').fill('admin');
		await page.getByLabel('Password').fill('correct horse');
		await page.getByRole('button', { name: 'Create account' }).click();
	} else {
		await page.getByLabel('Username').fill('admin');
		await page.getByLabel('Password').fill('correct horse');
		await page.getByRole('button', { name: 'Log in' }).click();
	}
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
}

async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

test('profile in lb/ft-in gives a budget; providers; about; no Google client', async ({ page }) => {
	await signIn(page);
	const budget = page.getByRole('region', { name: 'Budget' });

	// Another spec may have saved a profile first; the set-up card shows only before that.
	const { version } = await (await page.request.get('/api/profile')).json();
	if (version === null) {
		await expect(budget.getByText('Save your profile to see your daily plan')).toBeVisible();
		await budget.getByRole('link', { name: 'Set up your profile' }).click();
	} else {
		await page.goto('/profile');
	}

	const profile = page.getByRole('region', { name: 'Profile and targets' });
	await profile.getByText('lb', { exact: true }).click();
	await profile.getByText('ft/in', { exact: true }).click();
	await profile.getByText('Female', { exact: true }).click();
	await profile.getByLabel('Birth year').fill('1990');
	await profile.getByLabel('Height, feet').fill('5');
	await profile.getByLabel('Height, inches').fill('6');
	await profile.getByLabel('Start weight (lb)').fill('170');
	await profile.getByLabel('Goal weight (lb, optional)').fill('150');
	await profile.getByLabel('Pace (lb per week)').fill('1');
	await profile.getByLabel('Typical steps a day').fill('7000');
	await profile.getByRole('button', { name: 'Save profile' }).click();
	await expect(profile.getByRole('status')).toHaveText('Profile saved. Your budget uses it from today.');
	const saved = await (await page.request.get('/api/profile')).json();
	expect(saved).toMatchObject({ weight_unit: 'lb', height_unit: 'ftin' });
	expect(saved.version.start_weight_kg).toBeCloseTo(77.11, 1);
	expect(saved.version.height_cm).toBeCloseTo(167.64, 1);
	await noSideScroll(page);

	await page.getByRole('link', { name: 'Today' }).click();
	await expect(budget.getByText(/kcal (left|over)/)).toBeVisible();
	await expect(budget.getByText(/No steps logged yet/)).toBeVisible();
	await expect(budget.getByRole('meter', { name: 'Protein' })).toBeVisible();
	await budget.getByText("How it's worked out").click();
	await expect(budget.getByText('BMR', { exact: true })).toBeVisible();
	await expect(budget.getByText('Weight used')).toBeVisible();
	await expect(budget.getByText(/170 lb/)).toBeVisible();
	await expect(budget.getByRole('button', { name: 'Close' })).toBeFocused();
	await page.keyboard.press('Escape');
	await expect(budget.getByText('BMR', { exact: true })).toBeHidden();
	await expect(budget.getByText("How it's worked out")).toBeFocused();
	await noSideScroll(page);

	await page.goto('/profile');
	const profilePage = page.getByRole('region', { name: 'Profile and targets' });
	await profilePage.getByLabel('Daily calorie goal (kcal, optional)').fill('1800');
	await profilePage.getByRole('button', { name: 'Save profile' }).click();
	await expect(profilePage.getByRole('status')).toHaveText('Profile saved. Your budget uses it from today.');

	await page.getByRole('link', { name: 'Today' }).click();
	await expect(budget.getByText('1800 kcal goal')).toBeVisible();
	await budget.getByText("How it's worked out").click();
	await expect(budget.getByText('Your goal', { exact: true })).toBeVisible();
	await expect(budget.getByText('Gap to maintenance')).toBeVisible();
	await expect(budget.getByText(/kcal under maintenance/)).toBeVisible();
	await page.keyboard.press('Escape');

	await page.getByRole('link', { name: 'Activity' }).click();
	await expect(page.getByTestId('activity-budget')).toContainText('Burned beyond base');
	await expect(page.getByTestId('activity-budget')).toContainText('Goal 1800 kcal');
	await expect(page.getByText('Used from this day until your next weigh-in.')).toBeVisible();

	await page.goto('/profile');
	await profilePage.getByLabel('Daily calorie goal (kcal, optional)').fill('');
	await profilePage.getByRole('button', { name: 'Save profile' }).click();
	await page.getByRole('link', { name: 'Today' }).click();
	await expect(budget.getByText(/kcal plan/)).toBeVisible();

	// Providers: add one that can't be reached, test it, edit without touching the key, delete it.
	await page.getByRole('link', { name: 'Settings' }).click();
	const providers = await openRow(page.getByRole('region', { name: 'AI providers' }));
	await providers.getByRole('button', { name: 'Add provider' }).click();
	await providers.getByLabel('Name', { exact: true }).fill('Nowhere');
	await providers.getByLabel('Base URL').fill('http://127.0.0.1:9/v1');
	await providers.getByLabel('Model', { exact: true }).fill('test-model');
	await providers.getByLabel('API key', { exact: true }).fill('sk-test-key');
	await providers.getByRole('button', { name: 'Save provider' }).click();
	const row = providers.getByRole('listitem').filter({ hasText: 'Nowhere' });
	await expect(row.getByText('Key saved')).toBeVisible();

	await row.getByRole('button', { name: 'Test connection' }).click();
	await expect(row.getByText('Connection failed.')).toBeVisible({ timeout: 30_000 });
	await expect(row.getByText(/Couldn't reach the AI provider/)).toBeVisible();

	await row.getByRole('button', { name: 'Edit Nowhere' }).click();
	await expect(providers.getByLabel('API key', { exact: true })).toHaveAttribute('placeholder', /leave blank to keep/);
	await providers.getByLabel('Model', { exact: true }).fill('other-model');
	await providers.getByRole('button', { name: 'Save provider' }).click();
	await expect(row.getByText('other-model')).toBeVisible();
	await expect(row.getByText('Key saved')).toBeVisible();
	const list = await (await page.request.get('/api/ai/providers')).json();
	expect(list.providers.find((p: { label: string }) => p.label === 'Nowhere').has_key).toBe(true);
	await noSideScroll(page);

	page.once('dialog', (d) => d.accept());
	await row.getByRole('button', { name: 'Delete Nowhere' }).click();
	await expect(providers.getByRole('status').first()).toHaveText('Deleted Nowhere.');
	await expect(row).toHaveCount(0);

	const about = await openRow(page.getByRole('region', { name: 'About' }));
	await expect(about.getByText(/Cupful \d+\.\d+\.\d+/)).toBeVisible();
	await expect(about.getByRole('link', { name: 'Source code' })).toHaveAttribute('href', /^https?:\/\//);
	await expect(about.getByText('AGPL-3.0-or-later', { exact: false }).first()).toBeVisible();

	// The e2e server has no Google client configured: the feature is invisible and its routes 404.
	expect(await (await page.request.get('/api/google')).json()).toEqual({ configured: false });
	expect((await page.request.get('/api/google/connect', { maxRedirects: 0 })).status()).toBe(404);
	await expect(page.getByRole('heading', { name: 'Google Health' })).toHaveCount(0);

	// Settings links to the profile page; the avatar opens the same page and logout.
	await page.getByRole('link', { name: 'Open profile' }).click();
	await expect(page.getByRole('heading', { name: 'Profile', level: 1 })).toBeVisible();
	await page.setViewportSize({ width: 1280, height: 900 });
	await page.getByLabel('Account menu for admin').click();
	const menu = page.getByRole('group', { name: 'Account menu' });
	await menu.getByRole('link', { name: 'Profile' }).click();
	await expect(page.getByRole('heading', { name: 'Profile', level: 1 })).toBeVisible();

	await page.getByLabel('Account menu for admin').click();
	await page.getByRole('group', { name: 'Account menu' }).getByRole('button', { name: 'Log out' }).click();
	await expect(page).toHaveURL(/\/login$/);
});

test('a settings row opens from the URL hash, and every row is open on a wide screen', async ({ page }) => {
	await signIn(page);
	const about = page.getByRole('region', { name: 'About' });
	await page.goto('/settings#sync');
	await expect(page.getByTestId('sync-section').getByRole('button', { name: 'Sync now' })).toBeVisible();
	await expect(about.getByText(/App build/)).toBeHidden();

	await page.setViewportSize({ width: 1280, height: 900 });
	await expect(about.getByText(/App build/)).toBeVisible();
	await expect(page.getByRole('navigation', { name: 'Settings sections' })).toBeVisible();
});
