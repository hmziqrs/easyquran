<!--
  ReaderShell — the sidebar + sticky header + centered reading column shared by
  the surah / juz / page reader routes. Each route fills the `header` snippet
  (the line beside the sidebar trigger) and the default children (the content).
  The reading column uses the same page-width <Container> as the marketing pages.
-->
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
    <!-- top bar: sidebar toggle + route header. Sticky beneath the global nav;
         the bar stretches full-width, its content centered to the page width. -->
    <header class="sticky top-[60px] z-10 border-b border-line bg-bg/80 py-2.5 backdrop-blur-xl">
      <Container class="max-w-[1180px] flex items-center gap-3">
        <SidebarTrigger />
        {@render header()}
      </Container>
    </header>

    <!-- reading column — same width + padding as the marketing pages. -->
    <Container class="max-w-[1180px] py-6">
      {@render children()}
    </Container>
  </SidebarInset>
</SidebarProvider>
