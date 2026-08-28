<script lang="ts">
	import { cn, type WithElementRef, type WithoutChildren } from "$lib/utils.js";
	import type { HTMLTextareaAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		value = $bindable(),
		class: className,
		"data-slot": dataSlot = "textarea",
		...restProps
	}: WithoutChildren<WithElementRef<HTMLTextareaAttributes>> = $props();

	/* §37 (docs/design-system.md) applied to the multiline variant of the input contract:
	   surface background, border, focus = 2px --focus-ring outline + offset. Plan 03
	   geometry: the textarea is a multiline TEXT BLOCK, not a pill control — a 999px radius
	   on a min-h-16 field renders as a lozenge — so it takes the 10px block radius
	   (--radius-md). Semantic tokens only (§61). */
	const textareaClass =
		"border-border bg-surface text-foreground placeholder:text-muted-foreground flex field-sizing-content min-h-16 w-full rounded-md border px-3 py-2 text-body-s transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring aria-invalid:border-danger disabled:cursor-not-allowed disabled:opacity-50";
</script>

<textarea
	bind:this={ref}
	data-slot={dataSlot}
	class={cn(textareaClass, className)}
	bind:value
	{...restProps}
></textarea>
