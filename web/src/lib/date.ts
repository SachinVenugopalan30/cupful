// SPDX-License-Identifier: AGPL-3.0-or-later
// Dates are the user's local calendar date, `YYYY-MM-DD`, in their profile timezone.

/** Today's date in IANA timezone `tz`. */
export function todayIn(tz: string, now = new Date()): string {
	// en-CA formats as YYYY-MM-DD.
	return new Intl.DateTimeFormat('en-CA', { timeZone: tz }).format(now);
}

/** "Today", "Yesterday", "Tue 20 Oct", or with the year when it isn't this year's. */
export function dayLabel(date: string, today: string): string {
	if (date === today) return 'Today';
	if (date === addDays(today, -1)) return 'Yesterday';
	const [y, m, d] = date.split('-').map(Number);
	return new Intl.DateTimeFormat('en-GB', {
		timeZone: 'UTC',
		weekday: 'short',
		day: 'numeric',
		month: 'short',
		year: date.slice(0, 4) === today.slice(0, 4) ? undefined : 'numeric'
	})
		.format(new Date(Date.UTC(y, m - 1, d)))
		.replace(',', '');
}

/** `date` plus `n` calendar days (UTC arithmetic, so DST never shifts it). */
export function addDays(date: string, n: number): string {
	const [y, m, d] = date.split('-').map(Number);
	return new Date(Date.UTC(y, m - 1, d + n)).toISOString().slice(0, 10);
}
