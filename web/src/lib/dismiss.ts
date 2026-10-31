// SPDX-License-Identifier: AGPL-3.0-or-later
// A <details> used as a small menu or popover: closes on Esc (focus back to its <summary>), on a
// click or focus outside it, and on any [data-close] button inside. With `focusIn`, opening moves
// focus to the first button or input in the panel. Use as `<details use:dismiss>`.
export function dismiss(node: HTMLDetailsElement, focusIn = false) {
	const summary = node.querySelector('summary')!;
	const close = (refocus: boolean) => {
		if (!node.open) return;
		node.open = false;
		if (refocus) summary.focus();
	};
	const onKey = (e: KeyboardEvent) => {
		if (e.key !== 'Escape' || !node.open) return;
		e.stopPropagation();
		close(true);
	};
	const onOutside = (e: Event) => node.open && !node.contains(e.target as Node) && close(false);
	const onClick = (e: MouseEvent) => (e.target as Element).closest('[data-close]') && close(true);
	const onFocusOut = (e: FocusEvent) => e.relatedTarget && !node.contains(e.relatedTarget as Node) && close(false);
	const onToggle = () => focusIn && node.open && node.querySelector<HTMLElement>('button, input')?.focus();

	node.addEventListener('keydown', onKey);
	node.addEventListener('click', onClick);
	node.addEventListener('focusout', onFocusOut);
	node.addEventListener('toggle', onToggle);
	window.addEventListener('pointerdown', onOutside);
	return {
		destroy() {
			window.removeEventListener('pointerdown', onOutside);
		}
	};
}
