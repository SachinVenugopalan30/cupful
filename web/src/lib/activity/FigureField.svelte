<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  FigureField — a big editable number with its unit: the Steps, Gym and Weigh-in figures. Saves on
  change (blur or Enter); the parent's onchange does the parsing. Looks like the .fig primitive.
  Props: id, label (the accessible name), value (bindable text), unit, mode (inputmode),
  placeholder, disabled, onchange.
-->
<script lang="ts">
	let {
		id,
		label,
		value = $bindable(''),
		unit,
		mode = 'numeric',
		placeholder = '0',
		disabled = false,
		onchange
	}: {
		id: string;
		label: string;
		value?: string;
		unit: string;
		mode?: 'numeric' | 'decimal';
		placeholder?: string;
		disabled?: boolean;
		onchange: () => void;
	} = $props();
</script>

<div class="figure">
	<label class="visually-hidden" for={id}>{label}</label>
	<input {id} class="num" inputmode={mode} {placeholder} bind:value {disabled} {onchange} />
	<span class="unit" aria-hidden="true">{unit}</span>
</div>

<style>
	.figure {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		min-height: var(--touch);
		min-width: 0;
	}
	input {
		field-sizing: content;
		min-width: 1ch;
		max-width: 100%;
		min-height: var(--touch);
		padding: 0;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text);
		font-family: var(--font-display);
		font-size: 2.25rem;
		font-weight: 800;
		line-height: 1;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
		transition: background-color var(--dur-fast) var(--ease);
	}
	input:hover:not(:disabled) {
		background: var(--surface-2);
	}
	input:focus-visible {
		outline-offset: 2px;
		background: var(--surface-2);
	}
	input::placeholder {
		color: var(--text-muted);
		opacity: 0.6;
	}
	input:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}
	.unit {
		color: var(--text-muted);
		font-size: var(--text-md);
		font-weight: 600;
	}
</style>
