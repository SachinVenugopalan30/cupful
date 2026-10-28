// SPDX-License-Identifier: AGPL-3.0-or-later
// Theme preference, per device. `system` follows prefers-color-scheme (no data-theme attribute);
// `light`/`dark` set `<html data-theme>`. app.html applies the stored value before first paint;
// keep its inline script in step with KEY and the allowed values here.

export type ThemePref = 'system' | 'light' | 'dark';

const KEY = 'cupful-theme';

type Root = { dataset: DOMStringMap };

export function loadTheme(storage: Storage = localStorage): ThemePref {
	try {
		const v = storage.getItem(KEY);
		return v === 'light' || v === 'dark' ? v : 'system';
	} catch {
		return 'system';
	}
}

export function applyTheme(pref: ThemePref, root: Root = document.documentElement): void {
	if (pref === 'system') delete root.dataset.theme;
	else root.dataset.theme = pref;
}

export function saveTheme(
	pref: ThemePref,
	storage: Storage = localStorage,
	root: Root = document.documentElement
): void {
	applyTheme(pref, root);
	try {
		if (pref === 'system') storage.removeItem(KEY);
		else storage.setItem(KEY, pref);
	} catch {
		// Private mode or blocked storage: the choice lasts for this page only.
	}
}
