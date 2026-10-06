export function copySpecimens() {
  const selection = document.getSelection();
  const results = [...document.querySelectorAll("[data-specimen] [data-indopak-ayah]")].map(
    (verse) => {
      const range = document.createRange();
      range.selectNodeContents(verse);
      selection.removeAllRanges();
      selection.addRange(range);
      const event = new ClipboardEvent("copy", {
        bubbles: true,
        cancelable: true,
        clipboardData: new DataTransfer(),
      });
      verse.dispatchEvent(event);
      return {
        key: verse.closest("[data-verse-key]").dataset.verseKey,
        copied: event.clipboardData.getData("text/plain"),
        handled: event.defaultPrevented,
      };
    },
  );
  selection.removeAllRanges();
  return results;
}
