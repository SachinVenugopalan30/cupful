// SPDX-License-Identifier: AGPL-3.0-or-later
// "Update available" prompt for a waiting service worker (registerType: 'prompt').

/** `available` means a new version is installed and waiting; `offlineReady` is informational. */
export const updateState = $state({ available: false, offlineReady: false });

let initialized = false;
let reloading = false;

function supported(): boolean {
	return typeof navigator !== 'undefined' && 'serviceWorker' in navigator;
}

function watch(registration: ServiceWorkerRegistration): void {
	if (registration.waiting && navigator.serviceWorker.controller) {
		updateState.available = true;
	}
	registration.addEventListener('updatefound', () => {
		const installing = registration.installing;
		if (!installing) return;
		installing.addEventListener('statechange', () => {
			if (
				installing.state === 'installed' &&
				navigator.serviceWorker.controller &&
				registration.waiting
			) {
				updateState.available = true;
			}
		});
	});
}

/** Starts watching for a waiting worker. Safe to call more than once. */
export function initUpdatePrompt(): void {
	if (initialized || !supported()) return;
	initialized = true;

	void navigator.serviceWorker.getRegistration().then((registration) => {
		if (registration) watch(registration);
	});
}

/** Tells the waiting worker to take over; the page reloads when it controls the tab. */
export function applyUpdate(): void {
	if (!supported()) return;
	void navigator.serviceWorker.getRegistration().then((registration) => {
		if (!registration?.waiting) return;
		if (!reloading) {
			reloading = true;
			navigator.serviceWorker.addEventListener('controllerchange', () => location.reload());
		}
		registration.waiting.postMessage({ type: 'SKIP_WAITING' });
	});
}
