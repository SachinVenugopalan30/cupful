// SPDX-License-Identifier: AGPL-3.0-or-later
// The small easing behind the Trends headline count-up. 700 ms, ease-out cubic.

/** 0 → 1 with a fast start and a soft landing. */
export function easeOutCubic(t: number): number {
	const x = Math.min(1, Math.max(0, t));
	return 1 - (1 - x) ** 3;
}

/** The number to show at progress `t` between `from` and `to`. */
export function countValue(from: number, to: number, t: number): number {
	return from + (to - from) * easeOutCubic(t);
}
