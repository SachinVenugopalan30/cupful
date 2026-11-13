// SPDX-License-Identifier: AGPL-3.0-or-later
// Which cached day reads to drop. Pure, so the choice is testable without IndexedDB.

/** Dates kept per user. Older day reads are dropped when a new one is written. */
export const KEEP_DATES = 60;

const DAY_READ = /^\/api\/(?:meals\?date=|days\/|budget\/)(\d{4}-\d{2}-\d{2})$/;

/** The date a day read (meals, day, budget) is for, or null for any other path. */
export function dayReadDate(path: string): string | null {
	return DAY_READ.exec(path)?.[1] ?? null;
}

/**
 * Keys to evict: day reads for dates outside the `keep` most recent, except dates in `protect`
 * (the write queue still references them). Other entries are never selected.
 */
export function keysToEvict(
	entries: { key: string; path: string }[],
	protect: ReadonlySet<string>,
	keep = KEEP_DATES
): string[] {
	const dates = new Set<string>();
	for (const e of entries) {
		const d = dayReadDate(e.path);
		if (d) dates.add(d);
	}
	const kept = new Set([...dates].sort().reverse().slice(0, keep));
	return entries
		.filter((e) => {
			const d = dayReadDate(e.path);
			return d !== null && !kept.has(d) && !protect.has(d);
		})
		.map((e) => e.key);
}
