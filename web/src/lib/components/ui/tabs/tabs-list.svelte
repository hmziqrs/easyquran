<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: TabsPrimitive.ListProps = $props();

	/* §38 (docs/design-system.md): tabs are content destinations — an inline row over a
	   hairline divider, not a pill container (pills belong to segmented controls). */
	const listClass =
		"inline-flex items-end gap-1 border-b border-border text-foreground-secondary";
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	class={cn(listClass, className)}
	{...restProps}
/>
