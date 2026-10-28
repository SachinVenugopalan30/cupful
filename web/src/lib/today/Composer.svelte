<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  Composer — meal type, description, optional photo, Estimate → review → save; manual entry.
  Props (fixed contract; Today page passes these):
    date: string          YYYY-MM-DD the meal is logged on
    onSaved: () => void   call after sendOps reports the meal applied
  Writes only through #lib/sync/index.ts (mealUpsert + sendOps, in Review); scaleItems before saving.
  AI errors: errorMessage(e, 'estimate'), branched on ApiError.code. Photos are re-encoded through a
  canvas before upload and kept in memory until saved or removed, so a retry needs no new photo.
-->
<script lang="ts">
	import { ApiError, errorMessage, estimate } from '#lib/api.ts';
	import { defaultMealType } from '#lib/products/logic.ts';
	import Review from './Review.svelte';
	import {
		MAX_DESCRIPTION,
		MEAL_TYPES,
		draftFromEstimate,
		manualDraft,
		reencodePhoto,
		type Draft
	} from './meal';

	let { date, onSaved }: { date: string; onSaved: () => void } = $props();

	let mealType = $state(defaultMealType(new Date().getHours()));
	let description = $state('');
	let photo = $state<Blob | null>(null);
	let preview = $state('');
	let busy = $state(false);
	let failure = $state<{ code: string; message: string } | null>(null);
	let review = $state<{ draft: Draft; title: string } | null>(null);

	const RETRY = ['timeout', 'unreachable', 'unparseable', 'rate_limited', 'provider_error', 'offline'];
	const SETTINGS = ['no_provider', 'provider_auth'];

	async function pickPhoto(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = ''; // picking the same photo again still fires change
		if (!file) return;
		failure = null;
		try {
			setPhoto(await reencodePhoto(file));
		} catch {
			failure = { code: 'photo', message: "Couldn't read this photo. Try another, or describe the meal." };
		}
	}
	function setPhoto(b: Blob | null) {
		if (preview) URL.revokeObjectURL(preview);
		photo = b;
		preview = b ? URL.createObjectURL(b) : '';
	}

	async function runEstimate() {
		failure = null;
		const text = description.trim();
		if (!text) return void (failure = { code: 'empty', message: 'Describe the meal first, even with a photo.' });
		if (!navigator.onLine) {
			failure = { code: 'offline', message: errorMessage(new TypeError('offline'), 'estimate') };
			return;
		}
		busy = true;
		try {
			const est = await estimate(text, photo ?? undefined);
			review = {
				draft: draftFromEstimate(est, { date, meal_type: mealType, description: text }),
				title: 'Review estimate'
			};
		} catch (e) {
			const code = e instanceof ApiError ? (e.code ?? String(e.status)) : e instanceof TypeError ? 'offline' : 'other';
			failure = { code, message: errorMessage(e, 'estimate') };
		} finally {
			busy = false;
		}
	}

	function byHand() {
		failure = null;
		review = {
			draft: manualDraft({ date, meal_type: mealType, description: description.trim() }),
			title: 'Enter by hand'
		};
	}

	function saved() {
		description = '';
		setPhoto(null);
		onSaved();
	}

	const label = (t: string) => t[0].toUpperCase() + t.slice(1);
</script>

<section class="card composer" aria-labelledby="composer-title">
	<form class="stack" onsubmit={(e) => (e.preventDefault(), runEstimate())}>
		<h2 id="composer-title">Log a meal</h2>
		<fieldset class="segmented" aria-label="Meal">
			{#each MEAL_TYPES as t (t)}
				<label><input type="radio" name="composer-type" value={t} bind:group={mealType} />{label(t)}</label>
			{/each}
		</fieldset>

		<div class="field">
			<label for="composer-description">What did you eat?</label>
			<textarea
				id="composer-description"
				bind:value={description}
				maxlength={MAX_DESCRIPTION}
				placeholder="Small cup of rice, dal, one chicken breast shallow fried"
				aria-describedby="composer-count"
			></textarea>
			<p id="composer-count" class="hint num" aria-live="polite">
				{#if description.length > MAX_DESCRIPTION - 200}{description.length} / {MAX_DESCRIPTION} characters{/if}
			</p>
		</div>

		<div class="photo row">
			{#if preview}
				<img src={preview} alt="Meal attached for the estimate" />
				<button type="button" class="btn btn-quiet" onclick={() => setPhoto(null)}>Remove photo</button>
			{:else}
				<label class="btn">
					<svg viewBox="0 0 24 24" aria-hidden="true"
						><path d="M4 8h3l2-3h6l2 3h3v11H4z" /><circle cx="12" cy="13" r="3.5" /></svg
					>
					Add photo
					<input
						class="visually-hidden"
						type="file"
						accept="image/*"
						capture="environment"
						onchange={pickPhoto}
					/>
				</label>
			{/if}
			<p class="small muted note">Photos are used for this estimate only and aren't saved.</p>
		</div>

		{#if failure}
			<div class="notice {failure.code === 'empty' ? 'notice-warning' : 'notice-error'}" role="alert">
				<p>{failure.message}</p>
				{#if failure.code !== 'empty' && failure.code !== 'photo'}
					<div class="row">
						{#if SETTINGS.includes(failure.code)}<a class="btn" href="/settings">Open Settings</a>{/if}
						{#if RETRY.includes(failure.code)}
							<button type="button" class="btn" onclick={runEstimate}>Try again</button>
						{/if}
						<button type="button" class="btn" onclick={byHand}>Enter by hand</button>
					</div>
				{/if}
			</div>
		{/if}

		<div class="actions">
			<button class="btn btn-primary" disabled={busy} aria-describedby="composer-status">
				{busy ? 'Estimating…' : 'Estimate'}
			</button>
			{#if !failure || failure.code === 'empty' || failure.code === 'photo'}
				<button type="button" class="btn btn-quiet" onclick={byHand}>Enter by hand</button>
			{/if}
		</div>
		<p id="composer-status" class="small muted" role="status">
			{#if busy}Estimating. Local models can take up to a minute.{/if}
		</p>
	</form>
</section>

{#if review}
	<Review draft={review.draft} title={review.title} onClose={() => (review = null)} onSaved={saved} />
{/if}

<style>
	.composer form {
		gap: var(--space-3);
	}
	fieldset {
		min-width: 0;
	}
	.hint:empty {
		display: none;
	}
	.photo {
		gap: var(--space-3);
		flex-wrap: nowrap;
	}
	.photo .btn {
		flex: none;
	}
	.photo svg {
		width: 1.25rem;
		height: 1.25rem;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linejoin: round;
	}
	.photo label:has(input:focus-visible) {
		outline: 3px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.photo img {
		width: 3.5rem;
		height: 3.5rem;
		object-fit: cover;
		border-radius: var(--radius-md);
		flex: none;
	}
	.note {
		min-width: 0;
	}
	.actions {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: var(--space-2);
	}
</style>
