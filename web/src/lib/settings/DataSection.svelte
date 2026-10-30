<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  DataSection — export the CSV and a JSON backup, import a Fuel Ledger CSV, restore a backup
  (PLAN.md §14 M5). Config REST, online only: the server streams the files and replaces rows.
  Restoring is destructive and asks in the card first.
-->
<script lang="ts">
	import { download, errorMessage, importCsv, restoreBackup } from '#lib/api.ts';
	import { day, loadSession, today } from '#lib/session.svelte.ts';
	import {
		formatImportSummary,
		formatRestoreSummary,
		parseBackup
	} from './data.ts';

	type Busy = '' | 'csv' | 'backup' | 'import' | 'restore';
	let busy = $state<Busy>('');
	let status = $state('');
	let error = $state('');
	// A parsed backup waiting for the user's yes.
	let pending = $state<Record<string, unknown> | null>(null);

	async function saveFile(path: string) {
		status = error = '';
		busy = path.endsWith('.csv') ? 'csv' : 'backup';
		try {
			const { blob, filename } = await download(path);
			const url = URL.createObjectURL(blob);
			const link = document.createElement('a');
			link.href = url;
			link.download = filename;
			link.style.display = 'none';
			document.body.append(link);
			link.click();
			link.remove();
			setTimeout(() => URL.revokeObjectURL(url), 1000);
			status = `Downloaded ${filename}.`;
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = '';
		}
	}

	async function onImport(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		status = error = '';
		busy = 'import';
		try {
			status = formatImportSummary(await importCsv(await file.text()));
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = '';
		}
	}

	async function onRestore(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		status = error = '';
		let doc: Record<string, unknown>;
		try {
			doc = parseBackup(await file.text());
		} catch (err) {
			error = errorMessage(err);
			return;
		}
		pending = doc;
	}

	async function restore() {
		const doc = pending;
		pending = null;
		if (!doc) return;
		busy = 'restore';
		try {
			status = formatRestoreSummary(await restoreBackup(doc));
			// Pick up a restored timezone, units and today's date without a reload.
			await loadSession();
			day.date = today();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = '';
		}
	}

	const blocked = $derived(busy !== '' || pending !== null);
</script>

<section class="card stack" aria-labelledby="data-h">
	<h2 id="data-h">Data</h2>
	<p class="small muted">
		The backup leaves out provider keys, ingest and Google tokens, and photos. Restoring replaces
		this account's meals, days, profile versions, foods, aliases, portions and products. The CSV is
		the Fuel Ledger interchange format.
	</p>

	{#if error}<p class="notice notice-error" role="alert">{error}</p>{/if}
	<p class="status small" role="status">{status}</p>

	<div class="group stack">
		<h3>Export</h3>
		<p class="small muted">Save a copy of your log, or a full backup you can restore later.</p>
		<div class="row">
			<button class="btn" disabled={blocked} onclick={() => saveFile('/api/export.csv')}>
				{busy === 'csv' ? 'Downloading…' : 'Download CSV'}
			</button>
			<button class="btn" disabled={blocked} onclick={() => saveFile('/api/backup')}>
				{busy === 'backup' ? 'Downloading…' : 'Download backup'}
			</button>
		</div>
	</div>

	<div class="group stack">
		<h3>Import CSV</h3>
		<p class="small muted">
			Import a Fuel Ledger CSV. Re-importing the same file changes nothing. A changed row under the
			same meal id updates that meal, unless you deleted or edited it in Cupful since.
		</p>
		<div class="row">
			<label class="btn" class:disabled={blocked} aria-disabled={blocked}>
				{busy === 'import' ? 'Importing…' : 'Import CSV'}
				<input
					class="visually-hidden"
					type="file"
					accept=".csv,text/csv"
					disabled={blocked}
					onchange={onImport}
				/>
			</label>
		</div>
	</div>

	<div class="group stack">
		<h3>Restore backup</h3>
		<p class="small muted">
			Replace this account's logs with a backup file. The file is checked before anything is
			replaced, and you'll be asked to confirm.
		</p>
		{#if pending}
			<div class="notice notice-warning" role="group" aria-label="Confirm restore">
				<p>
					Replace all logged data with this backup? Current meals, days and profile versions are
					replaced, and this cannot be undone.
				</p>
				<div class="row">
					<button class="btn btn-danger" onclick={restore}>Replace my data</button>
					<button class="btn btn-quiet" onclick={() => (pending = null)}>Cancel</button>
				</div>
			</div>
		{/if}
		<div class="row">
			<label class="btn btn-danger" class:disabled={blocked} aria-disabled={blocked}>
				{busy === 'restore' ? 'Restoring…' : 'Restore backup'}
				<input
					class="visually-hidden"
					type="file"
					accept=".json,application/json"
					disabled={blocked}
					onchange={onRestore}
				/>
			</label>
		</div>
	</div>
</section>

<style>
	.status:empty {
		display: none;
	}
	/* The file input is visually hidden, so the label carries the focus ring. */
	.btn:has(input:focus-visible) {
		outline: 3px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.btn.disabled {
		opacity: 0.55;
		cursor: not-allowed;
		transform: none;
	}
	.btn.disabled:hover {
		background: var(--surface);
	}
	.btn-danger.disabled:hover {
		background: transparent;
	}
</style>
