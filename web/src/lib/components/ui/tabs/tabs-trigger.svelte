<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: TabsPrimitive.TriggerProps = $props();

	/* §38/§51 (docs/design-system.md): 44px tab targets, underline indicator for the
	   active destination (data-[state=active] from bits-ui), visible --focus-ring focus,
	   and the active state is carried by border + color (never color alone). */
	const triggerClass =
		"inline-flex min-h-11 items-center justify-center gap-2 whitespace-nowrap border-b-2 border-transparent px-3 text-body-s font-medium text-foreground-secondary transition-colors ease-out hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring data-[state=active]:border-primary data-[state=active]:text-foreground disabled:pointer-events-none disabled:opacity-50";
</script>

<TabsPrimitive.Trigger
	bind:ref
	data-slot="tabs-trigger"
	class={cn(triggerClass, className)}
	{...restProps}
/>
