// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { googleResult } from './google.ts';

describe('googleResult', () => {
	it('reads the redirect outcome', () => {
		expect(googleResult('?google=connected')?.ok).toBe(true);
		expect(googleResult('?google=denied')?.ok).toBe(false);
		expect(googleResult('?google=error')?.ok).toBe(false);
		expect(googleResult('')).toBeNull();
		expect(googleResult('?google=wat')).toBeNull();
	});
});
