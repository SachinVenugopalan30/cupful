// SPDX-License-Identifier: AGPL-3.0-or-later
// The easing and tween behind every count-up: ease-out cubic, 520 ms (--dur-slow) by default.

/** 0 → 1 with a fast start and a soft landing. */
export function easeOutCubic(t: number): number {
	const x = Math.min(1, Math.max(0, t));
	return 1 - (1 - x) ** 3;
}

/** The number to show at progress `t` between `from` and `to`. */
export function countValue(from: number, to: number, t: number): number {
	return from + (to - from) * easeOutCubic(t);
}

/** One count: from where the number is now to where it should land. */
export type Tween = { from: number; to: number; start: number; duration: number };

/** Begin a count at `now`. Under reduced motion the duration is 0, so it lands at once. */
export function startTween(
	from: number,
	to: number,
	now: number,
	duration: number,
	reduced: boolean
): Tween {
	return { from, to, start: now, duration: reduced ? 0 : duration };
}

/** The number to show at time `now`. */
export function tweenValue(t: Tween, now: number): number {
	if (t.duration <= 0) return t.to;
	return countValue(t.from, t.to, (now - t.start) / t.duration);
}
