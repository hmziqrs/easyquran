export function inspectReference(key) {
  const element = document.querySelector(`[data-testid="verse-arabic-${key}"]`);
  const data = JSON.parse(document.querySelector("#__NEXT_DATA__").textContent);
  const verse = data.props.pageProps.versesResponse.verses.find((item) => item.verseKey === key);
  const sourceWords = new Map(verse.words.map((word) => [word.location, word]));
  let offset = 0;
  const words = [...element.querySelectorAll("[data-word-location]")].map((word) => {
    const text = word.querySelector("[class*=TextWord_word]");
    const source = sourceWords.get(word.dataset.wordLocation);
    const rect = word.getBoundingClientRect();
    const value = {
      location: word.dataset.wordLocation,
      text: text.textContent,
      legacy: source.textIndopak,
      type: source.charTypeName,
      offset,
      top: rect.top,
      left: rect.left,
      width: rect.width,
      family: getComputedStyle(text).fontFamily,
    };
    if (source.charTypeName === "word") offset += source.textIndopak.match(/\p{Lo}/gu)?.length ?? 0;
    return value;
  });
  const lineStarts = [];
  let previous;
  for (const word of words) {
    if (previous === undefined || Math.abs(word.top - previous) > 2) {
      lineStarts.push({
        offset: word.offset,
        text: word.text,
        type: word.type,
        location: word.location,
      });
      previous = word.top;
    }
  }
  const marker = words.find((word) => word.type !== "word");
  const lastWord = words.findLast((word) => word.type === "word");
  return {
    key,
    build_id: data.buildId,
    preferences: data.props.pageProps.__REDUX_STATE__.quranReaderStyles,
    width: element.getBoundingClientRect().width,
    size: Number.parseFloat(getComputedStyle(element).fontSize),
    dpr: devicePixelRatio,
    font_urls: performance
      .getEntriesByType("resource")
      .map((item) => item.name)
      .filter((url) => url.includes("indopak") && url.endsWith(".woff2")),
    loaded_fonts: [...document.fonts]
      .filter((font) => font.family.includes("IndoPak"))
      .map((font) => ({ family: font.family, status: font.status })),
    words,
    letters: verse.words
      .filter((word) => word.charTypeName === "word")
      .map((word) => word.textIndopak)
      .join("")
      .replace(/[^\p{Lo}]/gu, ""),
    line_starts: lineStarts,
    marker_orphaned: !!marker && Math.abs(marker.top - lastWord.top) > 2,
  };
}

export function inspectLocal({ key, width, size }) {
  const sampleNode = document.querySelector(`[data-specimen="${key}"]`);
  sampleNode.style.width = `${width + 34}px`;
  sampleNode.style.maxWidth = `${width + 34}px`;
  sampleNode.style.setProperty("--reader-arabic-size", `${size}px`);
  const verse = sampleNode.querySelector("[data-indopak-ayah]");
  const clone = verse.cloneNode(true);
  clone.querySelector("[data-indopak-ornament]").remove();
  let offset = 0;
  const words = [...verse.querySelectorAll(".indopak-word, .indopak-final-word")].map((word) => {
    const value = word.cloneNode(true);
    value.querySelector(".indopak-ending")?.remove();
    const text = value.textContent;
    const walker = document.createTreeWalker(word, NodeFilter.SHOW_TEXT);
    let node;
    let top;
    while ((node = walker.nextNode())) {
      const match = /\p{Lo}/u.exec(node.textContent);
      if (!match) continue;
      const range = document.createRange();
      range.setStart(node, match.index);
      range.setEnd(node, match.index + match[0].length);
      top = range.getBoundingClientRect().top;
      break;
    }
    const result = { text, offset, top };
    offset += text.match(/\p{Lo}/gu)?.length ?? 0;
    return result;
  });
  const starts = [];
  let previous;
  for (const word of words) {
    if (previous === undefined || Math.abs(word.top - previous) > 2) {
      starts.push({ text: word.text, offset: word.offset });
      previous = word.top;
    }
  }
  const final = verse.querySelector(".indopak-final-word");
  const ornament = verse.querySelector("[data-indopak-ornament]");
  return {
    text: clone.textContent,
    letters: clone.textContent.replace(/[^\p{Lo}]/gu, ""),
    width: sampleNode.querySelector(".run").getBoundingClientRect().width,
    size: Number.parseFloat(getComputedStyle(verse).fontSize),
    line_starts: starts,
    words,
    final_word_with_marker: final.contains(ornament) && final.getClientRects().length === 1,
  };
}
