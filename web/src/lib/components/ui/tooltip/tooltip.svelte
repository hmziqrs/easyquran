<script lang="ts" generics="T = never">
	import { Tooltip as TooltipPrimitive } from "bits-ui";

	// Touch-only devices (no hover) never open tooltips: a tap must act, not reveal a label,
	// and hover-reactive content is what makes iOS Safari demand a second tap on a control.
	// globalThis.matchMedia is undefined on the server, so SSR keeps tooltips enabled.
	const touchOnly = globalThis.matchMedia?.("(hover: none)").matches === true;

	let {
		open = $bindable(false),
		disabled = touchOnly,
		...restProps
	}: TooltipPrimitive.RootProps<T> = $props();
</script>

<TooltipPrimitive.Root bind:open {disabled} {...restProps} />
