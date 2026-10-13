<script lang="ts">
	import { DropdownMenu as DropdownMenuPrimitive } from "bits-ui";
	import { cn } from "#lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		inset,
		variant = "default",
		...restProps
	}: DropdownMenuPrimitive.ItemProps & {
		inset?: boolean;
		variant?: "default" | "destructive";
	} = $props();
</script>

<!-- House menu row: highlighted = surface-hover fill + foreground text, the
     destructive variant stays danger-red through its highlight. -->
<DropdownMenuPrimitive.Item
	bind:ref
	data-slot="dropdown-menu-item"
	data-inset={inset}
	data-variant={variant}
	class={cn(
		"relative flex w-full cursor-default items-center gap-1.5 rounded-sm px-2.5 py-1.5 text-start text-[13px] text-foreground-secondary outline-none select-none transition-colors data-[variant=destructive]:text-danger data-[variant=destructive]:data-[highlighted]:text-danger data-[highlighted]:bg-surface-hover data-[highlighted]:text-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-40 data-inset:ps-7",
		className
	)}
	{...restProps}
/>
