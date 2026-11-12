// SPDX-License-Identifier: AGPL-3.0-or-later
// Usuals chip: a private server seeds past meals, then one tap logs the usual, Undo removes it,
// and the menu lists the close alternatives plus the local dismissal action.
import { expect, test } from '@playwright/test';
import type { ChildProcess } from 'node:child_process';
import {
	login,
	randomPort,
	seedMeal,
	startCupful,
	type MealType
} from './helpers/server';

const port = randomPort();
let server: ChildProcess;

test.use({ baseURL: `http://127.0.0.1:${port}` });

test.beforeAll(async () => {
	server = (await startCupful(port)).proc;
});
test.afterAll(() => void server?.kill());

function slot(hour: number): MealType {
	if (hour >= 5 && hour < 11) return 'breakfast';
	if (hour >= 11 && hour < 15) return 'lunch';
	if (hour >= 18 && hour < 22) return 'dinner';
	return 'snack';
}

function daysAgo(n: number): string {
	const d = new Date();
	d.setDate(d.getDate() - n);
	return d.toLocaleDateString('en-CA');
}

test('shows the usual, logs in one tap, undoes, and lists the menu', async ({ page }) => {
	await login(page);
	const mealType = slot(new Date().getHours());
	for (let i = 1; i <= 4; i++) await seedMeal(page, daysAgo(i), 'Usual oats', mealType);
	for (let i = 1; i <= 3; i++) await seedMeal(page, daysAgo(i), 'Alt bowl', mealType);
	await page.goto('/');

	const usual = page.getByTestId('usual');
	const body = usual.locator('.usual-body');
	await expect(usual).toBeVisible();
	await expect(body).toContainText('Usual oats');
	await expect(body).toContainText('200 kcal');
	await expect(body).not.toContainText('Alt bowl');

	await usual.getByRole('button', { name: 'Log', exact: true }).click();
	await expect(usual).toContainText('Usual logged.');
	await expect(page.getByTestId('meal')).toHaveCount(1);

	await page.getByTestId('usual-undo').click();
	await expect(body).toContainText('Usual oats');
	await expect(page.getByTestId('meal')).toHaveCount(0);

	await usual.getByLabel('More usual options').click();
	await expect(usual.getByText('Other usuals')).toBeVisible();
	await expect(usual.getByRole('button', { name: /Alt bowl/ })).toBeVisible();
	await usual.getByRole('button', { name: "Don't suggest this" }).click();
	await expect(body).toContainText('Alt bowl');
	await expect(body).not.toContainText('Usual oats');

	// The dismissal is local: a reload keeps it hidden for this browser.
	await page.reload();
	await expect(page.getByTestId('usual').locator('.usual-body')).toContainText('Alt bowl');
});
