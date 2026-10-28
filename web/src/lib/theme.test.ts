// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { applyTheme, loadTheme, saveTheme } from './theme';

function memory(): Storage {
	const m = new Map<string, string>();
	return {
		getItem: (k) => m.get(k) ?? null,
		setItem: (k, v) => void m.set(k, v),
		removeItem: (k) => void m.delete(k),
		clear: () => m.clear(),
		key: () => null,
		get length() {
			return m.size;
		}
	};
}

const broken = {
	getItem() {
		throw new Error('blocked');
	},
	setItem() {
		throw new Error('blocked');
	},
	removeItem() {
		throw new Error('blocked');
	}
} as unknown as Storage;

function root() {
	return { dataset: {} as DOMStringMap };
}

describe('theme preference', () => {
	it('defaults to system', () => {
		expect(loadTheme(memory())).toBe('system');
	});

	it('round-trips light and dark', () => {
		const s = memory();
		saveTheme('dark', s, root());
		expect(loadTheme(s)).toBe('dark');
		saveTheme('light', s, root());
		expect(loadTheme(s)).toBe('light');
	});

	it('system clears the stored value', () => {
		const s = memory();
		saveTheme('dark', s, root());
		saveTheme('system', s, root());
		expect(s.getItem('cupful-theme')).toBeNull();
		expect(loadTheme(s)).toBe('system');
	});

	it('ignores garbage', () => {
		const s = memory();
		s.setItem('cupful-theme', 'neon');
		expect(loadTheme(s)).toBe('system');
	});

	it('survives storage that throws', () => {
		expect(loadTheme(broken)).toBe('system');
		const r = root();
		expect(() => saveTheme('dark', broken, r)).not.toThrow();
		expect(r.dataset.theme).toBe('dark');
	});

	it('sets data-theme only for an explicit choice', () => {
		const r = root();
		applyTheme('light', r);
		expect(r.dataset.theme).toBe('light');
		applyTheme('system', r);
		expect('theme' in r.dataset).toBe(false);
	});
});
