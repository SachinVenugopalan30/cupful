// SPDX-License-Identifier: AGPL-3.0-or-later
// Close a native <dialog> after its exit transition. The dialog stays open while
// [data-closing] runs, then closes and calls the parent's unmount callback.

const EXIT_FALLBACK_MS = 400;

export function closeSheet(dialog: HTMLDialogElement, onClosed: () => void): void {
	// An already-closed dialog needs no exit, and a second call must not repeat the callback.
	if (!dialog.open || dialog.dataset.closing === 'true') return;
	dialog.dataset.closing = 'true';
	const reduced =
		typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
	if (reduced) {
		delete dialog.dataset.closing;
		dialog.close();
		onClosed();
		return;
	}

	let done = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const finish = () => {
		if (done) return;
		done = true;
		dialog.removeEventListener('transitionend', onEnd);
		if (timer !== undefined) clearTimeout(timer);
		delete dialog.dataset.closing;
		dialog.close();
		onClosed();
	};
	const onEnd = (event: TransitionEvent) => {
		if (event.target === dialog) finish();
	};
	dialog.addEventListener('transitionend', onEnd);
	timer = setTimeout(finish, EXIT_FALLBACK_MS);
}
