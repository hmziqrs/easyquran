<script lang="ts">
	import type { HTMLInputAttributes, HTMLInputTypeAttribute } from "svelte/elements";
	import { cn, type WithElementRef } from "$lib/utils.js";

	type InputType = Exclude<HTMLInputTypeAttribute, "file">;

	type Props = WithElementRef<
		Omit<HTMLInputAttributes, "type"> &
			({ type: "file"; files?: FileList } | { type?: InputType; files?: undefined })
	>;

	let {
		ref = $bindable(null),
		value = $bindable(),
		type,
		files = $bindable(),
		class: className,
		"data-slot": dataSlot = "input",
		...restProps
	}: Props = $props();

	/* §37 (docs/design-system.md) + plan 03 geometry: 44px height, PILL radius (controls are
	   999px), surface background, border, focus = 2px --focus-ring outline with 2px offset
	   (never removed without replacement). px-4 keeps clear of the pill curve.
	   Semantic tokens only (§61) — works across all 4 palettes × light/dark. */
	const inputClass =
		"border-border bg-surface text-foreground placeholder:text-muted-foreground h-11 w-full min-w-0 rounded-pill border px-4 text-body-s transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring aria-invalid:border-danger disabled:cursor-not-allowed disabled:opacity-50";
</script>

{#if type === "file"}
	<input
		bind:this={ref}
		data-slot={dataSlot}
		class={cn(
			"file:inline-flex file:h-full file:border-0 file:bg-transparent file:px-0 file:font-medium file:text-foreground",
			inputClass,
			className
		)}
		type="file"
		bind:files
		bind:value
		{...restProps}
	/>
{:else}
	<input
		bind:this={ref}
		data-slot={dataSlot}
		class={cn(inputClass, className)}
		{type}
		bind:value
		{...restProps}
	/>
{/if}
