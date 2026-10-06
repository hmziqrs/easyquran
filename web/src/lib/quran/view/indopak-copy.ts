function verseFor(node: Node): Element | null {
  const element = node instanceof Element ? node : node.parentElement;
  return element?.closest("[data-indopak-ayah]") ?? null;
}

export function indopakSelectionText(selection: Selection | null): string | null {
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  const texts: string[] = [];
  for (let index = 0; index < selection.rangeCount; index += 1) {
    const range = selection.getRangeAt(index);
    const first = verseFor(range.startContainer);
    const last = verseFor(range.endContainer);
    if (!first || !last) return null;
    const fragment = range.cloneContents();
    for (const ornament of fragment.querySelectorAll("[data-indopak-ornament]")) ornament.remove();
    if (first === last) {
      texts.push(fragment.textContent ?? "");
      continue;
    }
    for (const verse of fragment.querySelectorAll("[data-indopak-ayah]"))
      texts.push(verse.textContent ?? "");
  }
  return texts.join("\n");
}

export function copyIndopakSelection(event: ClipboardEvent): void {
  if (event.defaultPrevented || !event.clipboardData) return;
  const text = indopakSelectionText(document.getSelection());
  if (text === null) return;
  event.clipboardData.setData("text/plain", text);
  event.preventDefault();
}
