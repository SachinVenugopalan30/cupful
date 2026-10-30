// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure helpers for Settings → Data: attachment filenames, backup parsing and summary sentences.

import type { ImportSummary, RestoreSummary } from '../api';

/** The filename in a Content-Disposition header, or null when it has none. */
export function responseFilename(header: string | null): string | null {
	if (!header) return null;
	// RFC 5987: filename*=UTF-8''cupful%20backup.json
	const extended = /filename\*\s*=\s*(?:UTF-8'')?([^;]+)/i.exec(header);
	if (extended) {
		const raw = extended[1].trim().replace(/^"|"$/g, '');
		try {
			const name = decodeURIComponent(raw);
			if (name) return basename(name);
		} catch {
			// Malformed encoding: fall back to a plain filename below.
		}
	}
	const quoted = /filename\s*=\s*"([^"]+)"/i.exec(header);
	if (quoted) return basename(quoted[1]);
	const bare = /filename\s*=\s*([^;]+)/i.exec(header);
	return bare ? basename(bare[1].trim()) : null;
}

/** A path component can't escape into the download name. */
function basename(name: string): string | null {
	return name.split(/[\\/]/).pop() || null;
}

/** Used when Content-Disposition names no file. `date` is the user's local `YYYY-MM-DD`. */
export function fallbackFilename(path: string, date: string): string {
	if (path.endsWith('.csv')) return `cupful-${date}.csv`;
	if (path.includes('backup')) return `cupful-backup-${date}.json`;
	return `cupful-${date}.bin`;
}

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "Imported 3 meals (1 updated, 2 unchanged), 5 items, 2 days." */
export function formatImportSummary(s: ImportSummary): string {
	const total = s.meals_created + s.meals_updated + s.meals_unchanged;
	const parts: string[] = [];
	if (s.meals_created) parts.push(`${s.meals_created} new`);
	if (s.meals_updated) parts.push(`${s.meals_updated} updated`);
	if (s.meals_unchanged) parts.push(`${s.meals_unchanged} unchanged`);
	const detail = parts.length ? ` (${parts.join(', ')})` : '';
	return `Imported ${count(total, 'meal', 'meals')}${detail}, ${count(s.items, 'item', 'items')}, ${count(s.days, 'day', 'days')}.`;
}

/** "Restored 2 days, 3 meals, 5 items, 4 foods, 1 alias, 2 portions and 0 products." */
export function formatRestoreSummary(s: RestoreSummary): string {
	return `Restored ${count(s.days, 'day', 'days')}, ${count(s.meals, 'meal', 'meals')}, ${count(s.items, 'item', 'items')}, ${count(s.foods, 'food', 'foods')}, ${count(s.aliases, 'alias', 'aliases')}, ${count(s.portions, 'portion', 'portions')} and ${count(s.products, 'product', 'products')}.`;
}

/**
 * Parses a backup file the user chose. Rejects anything that isn't a JSON object with a message
 * that says what to pick instead; the server validates the document itself.
 */
export function parseBackup(text: string): Record<string, unknown> {
	let doc: unknown;
	try {
		doc = JSON.parse(text);
	} catch {
		throw new Error(
			"That file isn't valid JSON. Choose the backup .json file downloaded from Settings → Data."
		);
	}
	if (typeof doc !== 'object' || doc === null || Array.isArray(doc)) {
		throw new Error(
			"That file isn't a Cupful backup. Choose the .json file downloaded from Settings → Data."
		);
	}
	return doc as Record<string, unknown>;
}
