// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from 'vitest';
import { closeSheet } from './sheet.ts';

function fakeDialog() {
	const listeners = new Map<string, EventListener>();
	const dialog = {
		open: true,
		dataset: {} as Record<string, string>,
		close: vi.fn(() => {
			dialog.open = false;
		}),
		addEventListener: vi.fn((type: string, fn: EventListener) => listeners.set(type, fn)),
		removeEventListener: vi.fn((type: string) => listeners.delete(type)),
		dispatch: (type: string, event: Event) => listeners.get(type)?.(event)
	};
	return dialog;
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('closeSheet', () => {
	it('closes immediately under reduced motion', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: true }));
		const dialog = fakeDialog();
		const done = vi.fn();

		closeSheet(dialog as unknown as HTMLDialogElement, done);

		expect(dialog.close).toHaveBeenCalledOnce();
		expect(done).toHaveBeenCalledOnce();
	});

	it('closes only once under reduced motion when called twice', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: true }));
		const dialog = fakeDialog();
		const done = vi.fn();

		closeSheet(dialog as unknown as HTMLDialogElement, done);
		closeSheet(dialog as unknown as HTMLDialogElement, done);

		expect(dialog.close).toHaveBeenCalledOnce();
		expect(done).toHaveBeenCalledOnce();
	});

	it('waits for the exit transition before closing', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: false }));
		const dialog = fakeDialog();
		const done = vi.fn();

		closeSheet(dialog as unknown as HTMLDialogElement, done);

		expect(dialog.dataset.closing).toBe('true');
		expect(dialog.close).not.toHaveBeenCalled();

		dialog.dispatch('transitionend', { target: dialog } as unknown as Event);

		expect(dialog.close).toHaveBeenCalledOnce();
		expect(done).toHaveBeenCalledOnce();
	});

	it('closes only once when called twice while the transition is pending', () => {
		vi.stubGlobal('matchMedia', () => ({ matches: false }));
		const dialog = fakeDialog();
		const done = vi.fn();

		closeSheet(dialog as unknown as HTMLDialogElement, done);
		closeSheet(dialog as unknown as HTMLDialogElement, done);

		dialog.dispatch('transitionend', { target: dialog } as unknown as Event);

		expect(dialog.close).toHaveBeenCalledOnce();
		expect(done).toHaveBeenCalledOnce();
	});
});
