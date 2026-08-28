<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: TabsPrimitive.ListProps = $props();

	/* §38 (docs/design-system.md) + plan 03 geometry: tabs are content destinations rendered
	   as a row of PILL triggers (controls are 999px) — no container chrome, no hairline. */
	const listClass = "inline-flex items-center gap-2 text-foreground-secondary";
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	class={cn(listClass, className)}
	{...restProps}
/>
