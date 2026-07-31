<!--
  Landing B — "Spec".

  Dense and typographic: mono labels, hairline rules, a numbered index of what
  exists, and no cards anywhere. Left-aligned throughout, with a fixed
  measure — the page reads like a well-kept changelog and claims nothing it
  can't point at.

  The bet here is that for a tool whose whole pitch is "nothing in the way",
  a page with nothing in the way is itself the argument. It leans hardest on
  the surface family: with no illustration and no cards, the palette is the
  only thing carrying warmth.
-->
<script lang="ts">
  import { HERO, VALUES, ROADMAP, FACTS, SECTIONS } from "../copy";

  let { verses, arabicName, name }: { verses: string[]; arabicName: string; name: string } =
    $props();

  const specimen = $derived(verses.slice(0, 3));
  const num = (i: number) => String(i + 1).padStart(2, "0");
</script>

<div class="mx-auto w-full max-w-[860px] px-5">
  <!-- hero: a masthead rule, then the claim, then the actions as text links -->
  <section class="border-b border-line py-16">
    <div class="mb-8 flex items-center justify-between gap-4 font-mono text-[11px] uppercase tracking-[0.14em] text-fg-4">
      <span>EasyQuran</span>
      <span>{HERO.badge}</span>
    </div>

    <h1 class="max-w-[17ch] text-[clamp(34px,5.4vw,56px)] leading-[1.06] tracking-[-0.035em]">
      {HERO.title}
    </h1>
    <p class="mt-5 max-w-[56ch] text-[17px] leading-[1.65] text-fg-2">{HERO.sub}</p>

    <div class="mt-8 flex flex-wrap items-center gap-x-6 gap-y-3">
      <a
        href={HERO.primary.href}
        class="group inline-flex items-baseline gap-2 border-b border-accent pb-0.5 text-[15px] font-medium text-accent"
      >
        {HERO.primary.label}
        <span class="transition-transform duration-150 group-hover:translate-x-0.5">→</span>
      </a>
      <a
        href={HERO.secondary.href}
        class="border-b border-line-2 pb-0.5 text-[15px] text-fg-3 transition-colors hover:text-fg"
      >
        {HERO.secondary.label}
      </a>
    </div>
  </section>

  <!-- facts as a definition row -->
  <section class="grid grid-cols-2 gap-x-6 gap-y-5 border-b border-line py-7 sm:grid-cols-4">
    {#each FACTS as f (f.label)}
      <div class="flex flex-col gap-0.5">
        <!-- font-mono, not the `mono` utility: its calt/ss01 features insert a
             visible gap around the thousands comma at this size. -->
        <span class="font-mono text-[22px] leading-none text-fg">{f.value}</span>
        <span class="font-mono text-[11px] uppercase tracking-[0.12em] text-fg-4">{f.label}</span>
      </div>
    {/each}
  </section>

  <!-- what's here today, as a numbered index -->
  <section class="border-b border-line py-14">
    <div class="mb-8 flex flex-col gap-2">
      <span class="eyebrow text-accent">{SECTIONS.today.eyebrow}</span>
      <h2 class="max-w-[26ch] text-[27px] leading-[1.18] tracking-[-0.025em]">
        {SECTIONS.today.title}
      </h2>
    </div>

    <dl class="flex flex-col">
      {#each VALUES as v, i (v.title)}
        <div
          class="grid grid-cols-[2.5rem_1fr] items-baseline gap-x-4 border-t border-line py-5 first:border-t-0 sm:grid-cols-[2.5rem_11rem_1fr]"
        >
          <span class="font-mono text-[11px] text-fg-4">{num(i)}</span>
          <dt class="text-[15.5px] font-semibold">{v.title}</dt>
          <dd class="col-start-2 text-[14.5px] leading-[1.6] text-fg-2 sm:col-start-3">
            {v.body}
            <span class="ml-2 font-mono text-[11px] text-fg-4">{v.short}</span>
          </dd>
        </div>
      {/each}
    </dl>
  </section>

  <!-- a small inline specimen — proof, sized like a code sample -->
  <section class="border-b border-line py-14">
    <div class="mb-4 flex items-center justify-between font-mono text-[11px] uppercase tracking-[0.12em] text-fg-4">
      <span>Sample · {name}</span>
      <span dir="rtl" class="font-arabic text-sm normal-case tracking-normal text-fg-3">{arabicName}</span>
    </div>
    <div dir="rtl" class="flex flex-col gap-2 rounded-[10px] border border-line bg-bg-2 px-6 py-6">
      {#each specimen as v, i (i)}
        <p class="font-arabic text-[20px] leading-[2] text-fg">{v}</p>
      {/each}
    </div>
  </section>

  <!-- on the way -->
  <section class="py-14">
    <div class="mb-8 flex flex-col gap-2">
      <span class="eyebrow text-fg-3">{SECTIONS.soon.eyebrow}</span>
      <h2 class="max-w-[26ch] text-[27px] leading-[1.18] tracking-[-0.025em]">
        {SECTIONS.soon.title}
      </h2>
      <p class="max-w-[58ch] text-[15px] leading-[1.6] text-fg-3">{SECTIONS.soon.body}</p>
    </div>

    <ul class="flex flex-col">
      {#each ROADMAP as r (r.title)}
        <li
          class="grid grid-cols-1 items-baseline gap-x-4 gap-y-1 border-t border-line py-5 sm:grid-cols-[5rem_11rem_1fr]"
        >
          <span
            class="w-fit rounded border border-line-2 px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.1em] text-fg-4"
            >soon</span
          >
          <span class="text-[15.5px] font-semibold">{r.title}</span>
          <span class="text-[14.5px] leading-[1.6] text-fg-2">{r.body}</span>
        </li>
      {/each}
    </ul>

    <a
      href={HERO.primary.href}
      class="group mt-10 inline-flex items-baseline gap-2 border-b border-accent pb-0.5 text-[15px] font-medium text-accent"
    >
      {HERO.primary.label}
      <span class="transition-transform duration-150 group-hover:translate-x-0.5">→</span>
    </a>
  </section>
</div>
