<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import { Container } from "$lib/components";
  import AppSidebar from "./Sidebar.svelte";

  let { header, children }: { header: Snippet; children: Snippet } = $props();
  let mounted = $state(false);

  onMount(() => {
    mounted = true;
  });
</script>

<SidebarProvider open={false}>
  {#if mounted}
    <AppSidebar />
  {/if}

  <SidebarInset>
    <header class="sticky top-[60px] z-10 min-h-[49px] border-b border-line bg-bg/80 py-2.5 backdrop-blur-xl">
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
