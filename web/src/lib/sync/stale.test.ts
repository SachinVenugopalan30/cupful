// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, test } from 'vitest';
import { staleNotice } from './stale.ts';

test('stale notice names the thing and never claims success', () => {
	expect(staleNotice('meal')).toBe(
		'A newer change to this meal was already saved on another device; showing the latest.'
	);
});
