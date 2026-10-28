// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, test, type Page } from '@playwright/test';

async function noSideScroll(page: Page) {
	const overflow = await page.evaluate(
		() => document.documentElement.scrollWidth - document.documentElement.clientWidth
	);
	expect(overflow).toBeLessThanOrEqual(0);
}

test('first run: setup, Today, stays logged in, theme, logout and login', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL(/\/setup$/);
	await expect(page.getByRole('heading', { name: 'Create the admin account' })).toBeVisible();
	await expect(page.getByLabel('Time zone')).not.toHaveValue('');
	await noSideScroll(page);

	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password').fill('correct horse');
	await page.getByRole('button', { name: 'Create account' }).click();

	await expect(page).toHaveURL(/\/$/);
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
	for (const stub of ['Composer', 'BarcodeEntry', 'MealList']) {
		await expect(page.getByText(`${stub} placeholder`)).toBeVisible();
	}
	await expect(page.getByRole('button', { name: 'Next day' })).toBeDisabled();
	await page.getByRole('button', { name: 'Previous day' }).click();
	await expect(page.getByRole('heading', { name: 'Yesterday' })).toBeVisible();
	await noSideScroll(page);

	await page.reload();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();

	await page.getByRole('link', { name: 'Settings' }).click();
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
	await page.getByText('Dark', { exact: true }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await noSideScroll(page);

	await page.getByRole('button', { name: 'Log out' }).click();
	await expect(page).toHaveURL(/\/login$/);
	await page.goto('/');
	await expect(page).toHaveURL(/\/login$/);

	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password').fill('wrong password');
	await page.getByRole('button', { name: 'Log in' }).click();
	await expect(page.getByRole('alert')).toHaveText('Wrong username or password.');

	await page.getByLabel('Password').fill('correct horse');
	await page.getByRole('button', { name: 'Log in' }).click();
	await expect(page.getByRole('heading', { name: 'Today', level: 1 })).toBeVisible();
});
