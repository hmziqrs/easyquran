<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import { Container } from "$lib/components";
  import { stickyNav } from "$lib/stores/sticky-nav.svelte";
  import AppSidebar from "./Sidebar.svelte";

  let { header, children }: { header: Snippet; children: Snippet } = $props();
  let mounted = $state(false);

  let headerTop = $derived(stickyNav.collapsed ? "0px" : `${stickyNav.height}px`);

  onMount(() => {
    mounted = true;
  });
</script>

<SidebarProvider open={false}>
  {#if mounted}
    <AppSidebar />
  {/if}

  <SidebarInset>
    <header
      style:top={headerTop}
      class="sticky z-10 min-h-[49px] border-b border-line bg-bg/80 py-2.5 backdrop-blur-xl transition-[top] duration-200 ease-out"
    >
      <Container class="max-w-[1180px] flex items-center gap-3">
        {#if mounted}
          <SidebarTrigger />
        {/if}
        {@render header()}
      </Container>
    </header>

    <Container class="max-w-[1180px] py-6">
      {@render children()}
    </Container>
  </SidebarInset>
</SidebarProvider>
