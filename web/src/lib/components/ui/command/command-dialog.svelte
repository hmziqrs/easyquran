<script lang="ts">
  import { Command as CommandPrimitive, Dialog as DialogPrimitive } from "bits-ui";
  import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";
  import type { Snippet } from "svelte";
  import Command from "./command.svelte";

  let {
    open = $bindable(false),
    value = $bindable(""),
    title = "Search",
    description = "Search the Quran and jump anywhere in the app.",
    lang,
    dir,
    class: className,
    portalProps,
    children,
    ...restProps
  }: WithoutChildrenOrChild<CommandPrimitive.RootProps> & {
    open?: boolean;
    title?: string;
    description?: string;
    lang?: string;
    dir?: "ltr" | "rtl";
    portalProps?: WithoutChildrenOrChild<DialogPrimitive.PortalProps>;
    children: Snippet;
  } = $props();
</script>

<DialogPrimitive.Root bind:open>
  <DialogPrimitive.Portal {...portalProps}>
    <DialogPrimitive.Overlay
      data-slot="command-overlay"
      class="fixed inset-0 z-[90] bg-black/55 supports-backdrop-filter:backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0"
    />
    <DialogPrimitive.Content
      data-slot="command-dialog"
      {lang}
      {dir}
      class={cn(
        "fixed left-1/2 top-[12vh] z-[91] w-[calc(100vw-2rem)] max-w-[560px] -translate-x-1/2 overflow-hidden rounded-xl border border-line-2 bg-bg-elev shadow-2xl outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95",
        className,
      )}
    >
      <DialogPrimitive.Title class="sr-only">{title}</DialogPrimitive.Title>
      <DialogPrimitive.Description class="sr-only">{description}</DialogPrimitive.Description>
      <Command bind:value {...restProps}>
        {@render children()}
      </Command>
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>
