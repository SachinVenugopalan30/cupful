// SPDX-License-Identifier: AGPL-3.0-or-later
// The app's own reachability signal. `navigator.onLine` is unreliable on phones that have just
// brought up a VPN such as Tailscale: it can stay false while the tunnel already works, and it
// can stay true while DNS is not ready. A request that reaches the server is the only proof.

import { connection } from './state.svelte.ts';

const PROBE_PATH = '/api/health';
const PROBE_TIMEOUT = 3_000;
const BACKOFF_START = 1_000;
const BACKOFF_MAX = 5_000;

let timer: ReturnType<typeof setTimeout> | null = null;
let delay = 0;
let probing = false;
let listening = false;
const reachable = new Set<() => void>();

function clearTimer(): void {
	if (timer !== null) {
		clearTimeout(timer);
		timer = null;
	}
}

function resetBackoff(): void {
	clearTimer();
	delay = 0;
}

/** A response, even an error one, means the server is reachable. */
export function markOnline(): void {
	const was = connection.online;
	connection.online = true;
	if (connection.paused === 'offline') connection.paused = null;
	resetBackoff();
	if (was) return;
	// Readers and the queue decide what to refetch; a screen read here could hit an expired
	// session and redirect away from the queue's sign-in prompt.
	for (const fn of reachable) fn();
}

/** Only call this when a request could not reach the server at all. */
export function markOffline(): void {
	connection.online = false;
	if (connection.paused === null) connection.paused = 'offline';
	scheduleProbe();
}

/** Runs `fn` after the offline -> online transition. Returns an unsubscribe. */
export function onReachable(fn: () => void): () => void {
	reachable.add(fn);
	return () => reachable.delete(fn);
}

function scheduleProbe(): void {
	if (timer !== null || typeof window === 'undefined') return;
	delay = delay === 0 ? BACKOFF_START : Math.min(delay * 2, BACKOFF_MAX);
	timer = setTimeout(() => {
		timer = null;
		void probeNow();
	}, delay);
}

async function reachableNow(): Promise<boolean> {
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), PROBE_TIMEOUT);
	try {
		const res = await fetch(PROBE_PATH, {
			cache: 'no-store',
			credentials: 'same-origin',
			signal: controller.signal
		});
		return res.ok;
	} catch {
		return false;
	} finally {
		clearTimeout(timeout);
	}
}

/**
 * Probes `/api/health` once. A success flips the app online and wakes queued reads and writes;
 * a failure restarts the backoff. Exported for the visibility handler and for tests.
 */
export async function probeNow(): Promise<boolean> {
	if (probing) return connection.online;
	probing = true;
	try {
		if (await reachableNow()) {
			markOnline();
			return true;
		}
		scheduleProbe();
		return false;
	} finally {
		probing = false;
	}
}

function onWake(): void {
	if (document.visibilityState !== 'visible') {
		clearTimer();
		return;
	}
	if (connection.online) return;
	resetBackoff();
	void probeNow();
}

/** Subscribes to the browser hints and probes when the app returns to the foreground. */
export function initConnectivity(): void {
	if (listening || typeof window === 'undefined') return;
	listening = true;
	window.addEventListener('online', () => {
		resetBackoff();
		void probeNow();
	});
	window.addEventListener('offline', markOffline);
	window.addEventListener('pageshow', onWake);
	document.addEventListener('visibilitychange', onWake);
	if (!connection.online) scheduleProbe();
}
