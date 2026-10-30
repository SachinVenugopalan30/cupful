<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  SyncSection — Settings → Sync. Lists every queued write with its status, last error and age, with
  Retry and Discard per item so nothing can sit stuck. No props; reads the shared offline state.
-->
<script lang="ts">
	import { connection, queueState } from '#lib/offline/state.svelte.ts';
	import { discard, retry, syncNow } from '#lib/offline/queue.ts';
	import type { QueueStatus, QueuedOp } from '#lib/offline/types.ts';
	import { dayLabel } from '#lib/date.ts';
	import { today } from '#lib/session.svelte.ts';

	let syncing = $state(false);
	let working = $state<Record<string, boolean>>({});

	const STATUS: Record<QueueStatus, string> = {
		pending: 'Waiting to sync',
		sending: 'Sending…',
		stale: 'Too old to sync',
		rejected: 'Rejected by the server',
		failed: 'Failed'
	};

	const FIELD: Record<string, string> = {
		steps: 'steps',
		gym_minutes: 'gym minutes',
		trained: 'training',
		weight_kg: 'weight'
	};

	/** A short, human summary of an op. Also used verbatim in Retry/Discard accessible names. */
	function summary(entry: QueuedOp): string {
		const op = entry.op;
		if (op.kind === 'meal.upsert') {
			const description = op.payload.description?.trim() || 'Meal';
			return `${description} on ${dayLabel(op.payload.date, today())}`;
		}
		if (op.kind === 'meal.delete') return 'Delete a logged meal';
		const fields = Object.keys(op.payload)
			.filter((key) => key !== 'date')
			.map((key) => FIELD[key] ?? key);
		return `Day ${dayLabel(op.payload.date, today())}: ${fields.length > 0 ? fields.join(', ') : 'no changes'}`;
	}

	function chipClass(status: QueueStatus): string {
		if (status === 'stale') return 'chip chip-warning';
		if (status === 'rejected' || status === 'failed') return 'chip chip-error';
		return 'chip';
	}

	function age(ms: number): string {
		const seconds = Math.max(0, Math.floor((Date.now() - ms) / 1000));
		if (seconds < 60) return 'just now';
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours} h ago`;
		const days = Math.floor(hours / 24);
		return `${days} ${days === 1 ? 'day' : 'days'} ago`;
	}

	const ops = $derived([...queueState.ops].sort((a, b) => a.queued_at - b.queued_at));
	const blocked = $derived(syncing || !connection.online);

	async function sync() {
		syncing = true;
		try {
			await syncNow();
		} finally {
			syncing = false;
		}
	}

	async function act(id: string, fn: (opId: string) => Promise<void>) {
		working[id] = true;
		try {
			await fn(id);
		} catch {
			// The queue keeps the op and its error; the list re-renders from shared state.
		} finally {
			working[id] = false;
		}
	}
</script>

<section
	id="sync"
	class="card stack"
	aria-labelledby="sync-h"
	data-testid="sync-section"
	tabindex="-1"
>
	<div class="stack head">
		<h2 id="sync-h">Sync</h2>
		<p class="small muted">
			Logged changes are saved on this device first and sent when you're online. A change that the
			server rejects stays here until you retry or discard it.
		</p>
		<div class="row">
			<button class="btn" onclick={sync} disabled={blocked}>
				{syncing ? 'Syncing…' : 'Sync now'}
			</button>
		</div>
	</div>

	{#if ops.length === 0}
		<p class="empty">All changes synced.</p>
	{:else}
		<ul class="list" aria-label="Queued changes">
			{#each ops as entry (entry.op_id)}
				{@const summaryText = summary(entry)}
				{@const busy = working[entry.op_id] ?? false}
				<li
					class="op stack"
					data-testid="sync-op"
					data-op-id={entry.op_id}
					data-status={entry.status}
				>
					<div class="stack details">
						<div class="row">
							<strong class="break">{summaryText}</strong>
							<span class={chipClass(entry.status)}>
								{STATUS[entry.status]}
							</span>
						</div>
						<p class="small muted">
							Queued
							<time datetime={new Date(entry.queued_at).toISOString()} title={new Date(entry.queued_at).toLocaleString()}>
								{age(entry.queued_at)}
							</time>
						</p>
						{#if entry.error}
							<p class="small error break" role="alert">{entry.error}</p>
						{/if}
					</div>
					<div class="row">
						<button
							class="btn"
							onclick={() => act(entry.op_id, retry)}
							disabled={busy || !connection.online || entry.status === 'sending'}
							aria-label="Retry {summaryText}"
						>
							Retry
						</button>
						<button
							class="btn btn-danger"
							onclick={() => act(entry.op_id, discard)}
							disabled={busy}
							aria-label="Discard {summaryText}"
						>
							Discard
						</button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}

	{#if !connection.online}
		<p class="notice notice-warning" role="status">Offline. You can discard changes now; retry and sync need a connection.</p>
	{/if}
</section>

<style>
	.head {
		gap: var(--space-2);
	}
	.op {
		align-items: stretch;
		gap: var(--space-2);
	}
	.details {
		gap: var(--space-1);
		min-width: 0;
	}
	.break {
		overflow-wrap: anywhere;
	}
	.error {
		color: var(--error);
	}
</style>
