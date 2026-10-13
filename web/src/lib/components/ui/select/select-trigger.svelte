<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
	import { cn, type WithoutChild } from "#lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		size = "default",
		...restProps
	}: WithoutChild<SelectPrimitive.TriggerProps> & {
		size?: "sm" | "default";
	} = $props();
</script>

<!-- §37 + plan 03 geometry: the select trigger rides the input contract — a PILL
     field (border-border bg-surface, --focus-ring outline with offset); the sm
     size serves compact caption-row controls at the caption ramp. -->
<SelectPrimitive.Trigger
	bind:ref
	data-slot="select-trigger"
	data-size={size}
	class={cn(
		"flex w-fit items-center justify-between gap-1.5 rounded-pill border border-border bg-surface px-3 text-sm whitespace-nowrap text-foreground transition-colors select-none hover:border-border-strong focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring disabled:pointer-events-none disabled:opacity-50 data-[size=default]:h-9 data-[size=sm]:h-7 data-[size=sm]:px-2.5 data-[size=sm]:text-[12.5px] data-placeholder:text-muted *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-1.5 *:data-[slot=select-value]:line-clamp-1 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{@render children?.()}
	<ChevronDownIcon class="pointer-events-none text-muted" />
</SelectPrimitive.Trigger>
