<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: TabsPrimitive.TriggerProps = $props();

	/* §38/§51 (docs/design-system.md) + plan 03 geometry/state matrix: 44px PILL targets
	   (controls are 999px), active destination = primary fill (data-[state=active] from
	   bits-ui), hover = --surface-hover on the inactive state, visible --focus-ring focus,
	   and the active state is carried by fill + color (never color alone). */
	const triggerClass =
		"inline-flex min-h-11 items-center justify-center gap-2 whitespace-nowrap rounded-pill border border-transparent px-4 text-body-s font-medium text-foreground-secondary transition-colors ease-out hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring data-[state=active]:border-transparent data-[state=active]:bg-primary data-[state=active]:text-primary-foreground data-[state=active]:hover:bg-primary data-[state=active]:hover:text-primary-foreground disabled:pointer-events-none disabled:opacity-50";
</script>

<TabsPrimitive.Trigger
	bind:ref
	data-slot="tabs-trigger"
	class={cn(triggerClass, className)}
	{...restProps}
/>
