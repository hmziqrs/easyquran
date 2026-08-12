/** Global search palette. Any surface can open it; the palette itself owns the query. */
class CommandPalette {
  open = $state(false);
  query = $state("");

  show(seed = ""): void {
    this.query = seed;
    this.open = true;
  }
  hide(): void {
    this.open = false;
    this.query = "";
  }
  toggle(): void {
    if (this.open) this.hide();
    else this.show();
  }
  setQuery(v: string): void {
    this.query = v;
  }
}

export const commandPalette = new CommandPalette();

/** True for the platform-correct "open palette" chord (⌘K on Apple, Ctrl+K elsewhere). */
export function isPaletteChord(e: KeyboardEvent): boolean {
  return e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey) && !e.altKey;
}
