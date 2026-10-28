<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  BarcodeEntry — the "Scan barcode" button and saved-products entry point. Opens the scanner
  (BarcodeDetector, else a lazily loaded WASM reader, else a typed barcode), the product sheet
  (servings in 0.5 steps or grams, Open Food Facts credit), "Enter from the label", and the
  saved products list; logs the product as a meal.
  Props (fixed contract; Today page passes these):
    date: string          YYYY-MM-DD the meal is logged on
    onSaved: () => void   call after sendOps reports the meal applied
  API: products.lookup/label/list/remove from #lib/api.ts, errorMessage(e, 'product').
  Item: itemFromPer100(product, grams, portionText) from #lib/sync/index.ts; meal source 'manual'.
-->
<script lang="ts">
	let { date }: { date: string; onSaved: () => void } = $props();
</script>

<section class="stub" aria-label="Packaged food">
	<p>BarcodeEntry placeholder · {date}</p>
</section>
