<script lang="ts">
  import type { Snippet } from "svelte";
  import { SidebarProvider, SidebarInset, SidebarTrigger } from "$lib/components/ui/sidebar";
  import { Container } from "$lib/components";
  import AppSidebar from "./Sidebar.svelte";

  let { header, children }: { header: Snippet; children: Snippet } = $props();
</script>

<SidebarProvider open={false}>
  <AppSidebar />

  <SidebarInset>
    <header class="sticky top-[60px] z-10 border-b border-line bg-bg/80 py-2.5 backdrop-blur-xl">
      <Container class="max-w-[1180px] flex items-center gap-3">
        <SidebarTrigger />
        {@render header()}
      </Container>
    </header>

    <Container class="max-w-[1180px] py-6">
      {@render children()}
    </Container>
  </SidebarInset>
</SidebarProvider>
