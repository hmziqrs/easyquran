export interface IndoPakEnding {
  readonly body: string;
  readonly lastWord: string;
  readonly sign: string;
  readonly following: string;
  readonly ruku: boolean;
  readonly annotations: readonly IndoPakEndAnnotation[];
  readonly bodyParts: readonly IndoPakBodyPart[];
}

export interface IndoPakEndAnnotation {
  readonly text: string;
  readonly offset: number;
  readonly widthEm: number | undefined;
}

export interface IndoPakBodyPart {
  readonly text: string;
  readonly offset: number;
  readonly annotations: readonly IndoPakEndAnnotation[];
}

const notoAnnotationInkWidthsEm = new Map([
  [0x0615, 0.227],
  [0x06d6, 0.491],
  [0x06d7, 0.357],
  [0x06d8, 0.22],
  [0x06d9, 0.164],
  [0x06da, 0.209],
  [0x06db, 0.188],
  [0x06dc, 0.34],
  [0xe022, 0.211],
]);

function endAnnotations(suffix: string): IndoPakEndAnnotation[] {
  const parts = [
    ...suffix.matchAll(
      /[\uE01A-\uE01C\uE01E\uE01F\uE022\u0615\u06D6-\u06DC][^\uE01A-\uE01C\uE01E\uE01F\uE022\u0615\u06D6-\u06DC]*/gu,
    ),
  ];
  return parts.map((part) => ({
    text: part[0],
    offset: part.index,
    widthEm: notoAnnotationInkWidthsEm.get(part[0].charCodeAt(0)),
  }));
}

function bodyParts(text: string): IndoPakBodyPart[] {
  const clusters = text.matchAll(
    /[\uE01A-\uE01C\uE01E\uE01F](?:[\p{M}\p{Cf}\s]*[\uE01A-\uE01C\uE01E\uE01F\u0615\u06D6-\u06DC])+[\p{M}\p{Cf}]*/gu,
  );
  const parts: IndoPakBodyPart[] = [];
  let offset = 0;
  for (const cluster of clusters) {
    if (cluster.index > offset)
      parts.push({ text: text.slice(offset, cluster.index), offset, annotations: [] });
    parts.push({
      text: cluster[0],
      offset: cluster.index,
      annotations: endAnnotations(cluster[0]),
    });
    offset = cluster.index + cluster[0].length;
  }
  if (offset < text.length) parts.push({ text: text.slice(offset), offset, annotations: [] });
  return parts;
}

export function indopakEnding(text: string): IndoPakEnding {
  const ending =
    /([\uE01A-\uE01C\uE01E\uE01F\uE022][\uE01A-\uE01C\uE01E\uE01F\uE022\p{M}\p{Cf}\s]*)$/u.exec(
      text,
    );
  const suffix = ending?.[1] ?? "";
  const body = text.slice(0, ending?.index ?? text.length);
  const finalWord = /([^\s\u200B]*\p{L}[^\s\u200B]*[^\p{L}]*)$/u.exec(body);
  const ruku = suffix.startsWith("\uE022");
  const sign = /^[\uE01A-\uE01C\uE01E\uE01F\uE022][\p{Cf}]*/u.exec(suffix)?.[0] ?? "";
  return {
    body: body.slice(0, finalWord?.index ?? body.length),
    lastWord: finalWord?.[0] ?? "",
    sign,
    following: suffix.slice(sign.length),
    ruku,
    annotations: endAnnotations(suffix),
    bodyParts: bodyParts(body.slice(0, finalWord?.index ?? body.length)),
  };
}
