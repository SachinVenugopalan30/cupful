// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Locator } from '@playwright/test';

/**
 * Settings rows are <details>: closed on phones, open on wide screens. Opens the row (the region)
 * if it is closed, so a spec can use its controls at either width.
 */
export async function openRow(row: Locator): Promise<Locator> {
	const details = row.locator('details').first();
	if (!(await details.evaluate((el: HTMLDetailsElement) => el.open))) {
		await row.locator('summary').click();
	}
	return row;
}
