<script lang="ts">
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
	import { cn } from "#lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		inset,
		children,
		...restProps
	}: DropdownMenuPrimitive.SubTriggerProps & {
		inset?: boolean;
	} = $props();
</script>

<DropdownMenuPrimitive.SubTrigger
	bind:ref
	data-slot="dropdown-menu-sub-trigger"
	data-inset={inset}
	class={cn(
		"flex w-full cursor-default items-center gap-1.5 rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary outline-none select-none transition-colors data-open:bg-surface-hover data-open:text-foreground data-[highlighted]:bg-surface-hover data-[highlighted]:text-foreground data-inset:ps-7 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{@render children?.()}
	<ChevronRightIcon class="ms-auto rtl:-scale-x-100" />
</DropdownMenuPrimitive.SubTrigger>
