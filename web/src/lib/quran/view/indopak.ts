export interface IndoPakEnding {
  readonly body: string;
  readonly lastWord: string;
  readonly sign: string;
  readonly following: string;
  readonly ruku: boolean;
}

export function indopakEnding(text: string): IndoPakEnding {
  const ending = /([\uE01A\uE022][\p{M}\p{Cf}]*)$/u.exec(text);
  const suffix = ending?.[1] ?? "";
  const body = text.slice(0, ending?.index ?? text.length);
  const finalWord = /([^\s\u200B]+[\s\u200B]*)$/u.exec(body);
  const ruku = suffix.startsWith("\uE022");
  const sign = /^[\uE01A\uE022][\p{Cf}]*/u.exec(suffix)?.[0] ?? "";
  return {
    body: body.slice(0, finalWord?.index ?? body.length),
    lastWord: finalWord?.[0] ?? "",
    sign,
    following: suffix.slice(sign.length),
    ruku,
  };
}
