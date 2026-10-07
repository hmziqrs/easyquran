<script lang="ts">
  import { untrack, type Snippet } from "svelte";

  import {
    HOME_VARIANTS,
    homeVariantDefinition,
    type HomeAccent,
    type HomeFont,
    type HomeMode,
    type HomePreviewSettings,
    type HomeSurface,
    type HomeVariant,
  } from "./variants";

  let {
    variant,
    children,
  }: {
    variant: HomeVariant;
    children: Snippet<[HomePreviewSettings]>;
  } = $props();

  const preset = $derived(homeVariantDefinition(variant));
  let font = $state<HomeFont>(untrack(() => homeVariantDefinition(variant).font));
  let accent = $state<HomeAccent>(untrack(() => homeVariantDefinition(variant).accent));
  let surface = $state<HomeSurface>(untrack(() => homeVariantDefinition(variant).surface));
  let mode = $state<HomeMode>("light");

  function reset(): void {
    font = preset.font;
    accent = preset.accent;
    surface = preset.surface;
    mode = "light";
  }
</script>

<aside class="review-bar" aria-label="Homepage design review">
  <div class="review-top">
    <a class="review-home" href="/design">Design lab <span aria-hidden="true">/</span></a>
    <nav class="presets" aria-label="Homepage variants">
      {#each HOME_VARIANTS as option (option.id)}
        <a
          class={{ active: variant === option.id }}
          href={`/design/home/${option.id}`}
          aria-current={variant === option.id ? "page" : undefined}
        >{option.name}</a>
      {/each}
    </nav>
    <a class="live-link" href="/">Current homepage <span aria-hidden="true">↗</span></a>
  </div>
  <div class="review-bottom">
    <p>{preset.description}</p>
    <div class="controls">
      <label>
        <span>Font</span>
        <select bind:value={font}>
          <option value="onest">Onest</option>
          <option value="nunito">Nunito</option>
          <option value="jetbrains">JetBrains Mono</option>
        </select>
      </label>
      <label>
        <span>Accent</span>
        <select bind:value={accent}>
          <option value="cobalt">Cobalt</option>
          <option value="graphite">Graphite</option>
          <option value="slate">Slate</option>
          <option value="berry">Berry</option>
        </select>
      </label>
      <label>
        <span>Background</span>
        <select bind:value={surface}>
          <option value="plain">Plain</option>
          <option value="soft">Soft gray</option>
        </select>
      </label>
      <label>
        <span>Theme</span>
        <select bind:value={mode}>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </label>
      <button type="button" onclick={reset}>Reset</button>
    </div>
  </div>
</aside>

{@render children({ font, accent, surface, mode })}

<style>
  .review-bar {
    position: sticky;
    z-index: 60;
    top: 0;
    border-bottom: 1px solid #353535;
    background: #181818;
    color: #f5f5f5;
    font-family: "Onest Variable", system-ui, sans-serif;
    font-size: 12px;
    font-weight: 450;
    line-height: 1.4;
  }

  .review-top,
  .review-bottom {
    display: flex;
    align-items: center;
    gap: 18px;
    max-width: 1600px;
    margin-inline: auto;
    padding: 10px 28px;
  }

  .review-top {
    min-height: 50px;
  }

  .review-bottom {
    justify-content: space-between;
    border-top: 1px solid #303030;
    padding-block: 8px;
  }

  .review-home {
    display: flex;
    flex-shrink: 0;
    gap: 16px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .review-home span,
  .review-bottom p {
    color: #b7b7b7;
  }

  .presets {
    display: flex;
    gap: 4px;
  }

  .presets a {
    border-radius: 5px;
    padding: 7px 12px;
    color: #c4c4c4;
    font-weight: 550;
    white-space: nowrap;
  }

  .presets a:hover {
    background: #303030;
    color: #ffffff;
  }

  .presets a.active {
    background: #eeeeee;
    color: #161616;
  }

  .live-link {
    margin-inline-start: auto;
    color: #c4c4c4;
    white-space: nowrap;
  }

  .review-bottom p {
    max-width: 60ch;
    font-size: 11px;
  }

  .controls {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 14px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  label > span {
    color: #b7b7b7;
    font-size: 10px;
  }

  select,
  button {
    min-height: 30px;
    border: 1px solid #454545;
    border-radius: 4px;
    padding: 4px 7px;
    background: #262626;
    color: #f5f5f5;
    font: inherit;
    font-size: 11px;
  }

  select {
    max-width: 135px;
    cursor: pointer;
  }

  button {
    padding-inline: 10px;
    cursor: pointer;
  }

  button:hover {
    background: #383838;
  }

  a:focus-visible,
  button:focus-visible,
  select:focus-visible {
    outline: 2px solid #ffffff;
    outline-offset: 3px;
  }

  @media (width < 1120px) {
    .review-bottom p {
      display: none;
    }

    .review-bottom {
      justify-content: flex-end;
    }
  }

  @media (width < 680px) {
    .review-top,
    .review-bottom {
      gap: 8px;
      padding-inline: 14px;
    }

    .review-top {
      flex-wrap: wrap;
    }

    .review-home {
      font-size: 10px;
    }

    .review-home span {
      display: none;
    }

    .presets {
      order: 1;
      width: 100%;
    }

    .presets a {
      flex: 1;
      padding-inline: 7px;
      text-align: center;
    }

    .live-link {
      font-size: 10px;
    }

    .review-bottom {
      justify-content: flex-start;
      overflow-x: auto;
    }

    .controls {
      gap: 10px;
    }

    label {
      flex-direction: column;
      align-items: flex-start;
      gap: 3px;
    }

    .controls button {
      align-self: flex-end;
    }
  }
</style>
