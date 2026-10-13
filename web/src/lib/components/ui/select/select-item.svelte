<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import CheckIcon from "@lucide/svelte/icons/check";
	import { cn, type WithoutChild } from "#lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		value,
		label,
		children: childrenProp,
		...restProps
	}: WithoutChild<SelectPrimitive.ItemProps> = $props();
</script>

<!-- House menu row: highlighted = surface-hover fill + foreground text (never
     color alone), logical paddings (§50) so the check indicator mirrors to the
     inline-start side in the RTL locale. -->
<SelectPrimitive.Item
	bind:ref
	{value}
	{label}
	data-slot="select-item"
	class={cn(
		"relative flex w-full cursor-default items-center gap-1.5 rounded-sm py-1.5 pe-8 ps-2.5 text-start text-[13px] text-foreground-secondary outline-none select-none transition-colors data-[highlighted]:bg-surface-hover data-[highlighted]:text-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-40",
		className
	)}
	{...restProps}
>
	{#snippet children({ selected, highlighted })}
		<span class="pointer-events-none absolute end-2 flex size-4 items-center justify-center">
			{#if selected}
				<CheckIcon class="pointer-events-none" />
			{/if}
		</span>
		<span class="flex min-w-0 flex-1 gap-2 whitespace-nowrap">
			{#if childrenProp}
				{@render childrenProp({ selected, highlighted })}
			{:else}
				{label || value}
			{/if}
		</span>
	{/snippet}
</SelectPrimitive.Item>
