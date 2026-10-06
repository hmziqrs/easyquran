import { afterEach, describe, expect, it } from "vite-plus/test";
import { copyIndopakSelection, indopakSelectionText } from "../indopak-copy";

afterEach(() => {
  document.getSelection()?.removeAllRanges();
  document.body.replaceChildren();
});

function select(range: Range): Selection {
  const selection = document.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
  return selection;
}

function verse(text: string): HTMLElement {
  const element = document.createElement("span");
  element.dataset.indopakAyah = "";
  const word = document.createElement("span");
  word.style.display = "inline-flex";
  word.textContent = text;
  const ornament = document.createElement("span");
  ornament.dataset.indopakOrnament = "";
  ornament.textContent = "۝۱";
  element.append(word, ornament);
  document.body.append(element);
  return element;
}

describe("IndoPak selection copy", () => {
  it("preserves private signs and source whitespace while excluding the UI ornament", () => {
    const source = "اَلۡحَمۡدُ  \uE021ۙ\u200B لِلّٰهِ\uE022";
    const element = verse(source);
    const range = document.createRange();
    range.selectNodeContents(element);
    const selection = select(range);
    expect(indopakSelectionText(selection)).toBe(source);
    const event = new ClipboardEvent("copy", {
      cancelable: true,
      clipboardData: new DataTransfer(),
    });
    copyIndopakSelection(event);
    expect(event.defaultPrevented).toBe(true);
    expect(event.clipboardData?.getData("text/plain")).toBe(source);
  });

  it("keeps a partial selection's exact source code points", () => {
    const element = verse("اَ\uE021ۙ بَ\uE022");
    const text = element.firstChild!.firstChild!;
    const range = document.createRange();
    range.setStart(text, 2);
    range.setEnd(text, 6);
    expect(indopakSelectionText(select(range))).toBe(text.textContent!.slice(2, 6));
  });

  it("separates selected verses without copying toolbars between them", () => {
    const first = verse("اَ\uE021");
    const tools = document.createElement("button");
    tools.textContent = "Copy ayah";
    document.body.append(tools);
    const last = verse("بَ\uE022");
    const range = document.createRange();
    range.setStart(first.firstChild!.firstChild!, 0);
    range.setEnd(last.firstChild!.firstChild!, 3);
    expect(indopakSelectionText(select(range))).toBe("اَ\uE021\nبَ\uE022");
  });

  it("leaves ordinary page selections and collapsed selections to the browser", () => {
    const element = document.createElement("p");
    element.textContent = "Ordinary text";
    document.body.append(element);
    const range = document.createRange();
    range.selectNodeContents(element);
    const selection = select(range);
    expect(indopakSelectionText(selection)).toBeNull();
    range.collapse(true);
    expect(indopakSelectionText(select(range))).toBeNull();
  });
});
