// SPDX-License-Identifier: AGPL-3.0-or-later
// Which cached day reads to drop. Pure, so the choice is testable without IndexedDB.

/** Dates kept per user after a trim. */
export const KEEP_DATES = 60;

/**
 * Cache entries a user may hold before a write pays for a trim: three day reads per date for ten
 * dates of slack, plus room for the other reads. Below it a write only counts, never scans.
 */
export const EVICT_AT = (KEEP_DATES + 10) * 3 + 20;

const DAY_READ = /^\/api\/(?:meals\?date=|days\/|budget\/)(\d{4}-\d{2}-\d{2})$/;

/** The date a day read (meals, day, budget) is for, or null for any other path. */
export function dayReadDate(path: string): string | null {
	return DAY_READ.exec(path)?.[1] ?? null;
}

/**
 * Keys to evict: day reads for dates outside the `keep` most recently saved (a date's newest
 * `saved_at`, ties to the later date), except dates in `protect` (the write queue still references
 * them). An old day opened just now is recent, so it survives. Other entries are never selected.
 */
export function keysToEvict(
	entries: { key: string; path: string; saved_at: number }[],
	protect: ReadonlySet<string>,
	keep = KEEP_DATES
): string[] {
	const used = new Map<string, number>();
	for (const e of entries) {
		const d = dayReadDate(e.path);
		if (d) used.set(d, Math.max(used.get(d) ?? -Infinity, e.saved_at));
	}
	const ranked = [...used].sort(([da, a], [db, b]) => b - a || (db > da ? 1 : db < da ? -1 : 0));
	const kept = new Set(ranked.slice(0, keep).map(([d]) => d));
	return entries
		.filter((e) => {
			const d = dayReadDate(e.path);
			return d !== null && !kept.has(d) && !protect.has(d);
		})
		.map((e) => e.key);
}
