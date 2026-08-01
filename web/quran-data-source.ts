export type QuranXmlAttrs = Record<string, string>;

export interface QuranSuraAttrs extends QuranXmlAttrs {
  index: string;
  ayas: string;
  start: string;
  name: string;
  tname: string;
  ename: string;
  type: string;
  order: string;
  rukus: string;
}

export interface QuranCoordinateProjection {
  readonly rowCount: number;
  readonly surahs: readonly {
    readonly surah: number;
    readonly startGlobal: number;
    readonly ayahCount: number;
  }[];
}

function extractAttrs(inner: string): QuranXmlAttrs {
  const out: QuranXmlAttrs = {};
  const re = /(\w+)="([^"]*)"/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(inner))) out[match[1]!] = match[2]!;
  return out;
}

export function scanQuranElements(xml: string, tag: string): QuranXmlAttrs[] {
  const out: QuranXmlAttrs[] = [];
  const re = new RegExp(`<${tag}\\b([^/>]*)/>`, "g");
  let match: RegExpExecArray | null;
  while ((match = re.exec(xml))) out.push(extractAttrs(match[1]!));
  return out;
}

export function projectQuranCoordinates(
  surahs: readonly QuranSuraAttrs[],
): QuranCoordinateProjection {
  const projectedSurahs = surahs.map((surah) => ({
    surah: Number(surah.index),
    startGlobal: Number(surah.start) + 1,
    ayahCount: Number(surah.ayas),
  }));
  return {
    rowCount: projectedSurahs.reduce((total, surah) => total + surah.ayahCount, 0),
    surahs: projectedSurahs,
  };
}
