import { computed, mount, setColorScheme, signal, Scroll, Text, View } from "nsh";
import { Select } from "nsh/base";
import { SAMPLE } from "./data";
import Amiri from "./fonts/amiri.ttf";
import Scheherazade from "./fonts/scheherazade-new.ttf";
import NotoNaskh from "./fonts/noto-naskh-arabic.ttf";
import Hafs from "./fonts/kfgqpc-hafs.ttf";
import Taha from "./fonts/kfgqpc-taha-v1.ttf";
import TahaBold from "./fonts/kfgqpc-taha-v1-bold.ttf";
import Gulzar from "./fonts/Gulzar-Regular.ttf";
import Awami from "./fonts/AwamiNastaliq-Regular.ttf";
import Mehr from "./fonts/mehr-nastaliq.ttf";
import Katibeh from "./fonts/katibeh.ttf";
import Mirza from "./fonts/mirza.ttf";
import Lateef from "./fonts/Lateef-Regular.ttf";
import ArefRuqaa from "./fonts/aref-ruqaa.ttf";
import Lalezar from "./fonts/lalezar.ttf";
import NotoSansArabic from "./fonts/noto-sans-arabic.ttf";

// Same ids, order and defaults as ../flutter-quran-sample/lib/main.dart.
const ARABIC_FONTS: { id: string; family: string; weight?: number }[] = [
  { id: "amiri", family: Amiri },
  { id: "scheherazade-new", family: Scheherazade },
  { id: "noto-naskh-arabic", family: NotoNaskh },
  { id: "kfgqpc-hafs", family: Hafs },
  { id: "kfgqpc-taha-v1", family: Taha, weight: 400 },
  { id: "kfgqpc-taha-v1-bold", family: TahaBold, weight: 700 },
];

// Noto Nastaliq Urdu is named, not imported: iOS ships a font with the same
// PostScript name, so registering a bundled copy fails with an uncaught error.
const URDU_FONTS: { id: string; family: string; height: number }[] = [
  { id: "noto-nastaliq-urdu", family: "Noto Nastaliq Urdu", height: 2.5 },
  { id: "gulzar", family: Gulzar, height: 2.5 },
  { id: "awami-nastaliq", family: Awami, height: 2.6 },
  { id: "mehr-nastaliq-web", family: Mehr, height: 2.9 },
  { id: "katibeh", family: Katibeh, height: 2.2 },
  { id: "mirza", family: Mirza, height: 2.3 },
  { id: "lateef", family: Lateef, height: 2.2 },
  { id: "scheherazade-new", family: Scheherazade, height: 2.2 },
  { id: "amiri", family: Amiri, height: 2.2 },
  { id: "aref-ruqaa", family: ArefRuqaa, height: 2.0 },
  { id: "lalezar", family: Lalezar, height: 2.0 },
  { id: "noto-sans-arabic", family: NotoSansArabic, height: 2.4 },
];

const DATASETS = ["uthmani", "simple", "indopak"];
const ACCENT = "#00695C";
const RULE = "#E0E0E0";

function ayahMarker(n: number): string {
  const digits = [...String(n)].map((c) => String.fromCharCode(0x0660 + Number(c))).join("");
  return `﴿${digits}﴾`;
}

function urduFor(surah: number, n: number): string {
  return SAMPLE.translations.ur[String(surah)]?.find((u) => u.n === n)?.text ?? "";
}

function Selector(props: {
  label: string;
  ids: string[];
  value: () => string;
  onChange: (id: string) => void;
}) {
  return (
    <View class="flex-row items-center gap-2">
      <Text style={{ fontSize: 13, color: "#000000" }}>{props.label}</Text>
      <Select
        accessibilityLabel={props.label}
        items={props.ids.map((id) => ({ value: id, title: id }))}
        value={props.value()}
        onChange={(e) => {
          if (e.value !== null) props.onChange(e.value);
        }}
        popupClass="bg-white rounded-lg border border-gray-300"
        popupStyle={{ minWidth: 220 }}
        itemClass="px-3 py-2 selected:bg-gray-200"
        itemStyle={{ fontSize: 14, color: "#000000" }}
        trigger={
          <View class="flex-row items-center rounded border border-gray-300 px-2 py-1">
            <Text style={{ fontSize: 14, color: "#000000" }}>{props.value()} ▾</Text>
          </View>
        }
      />
    </View>
  );
}

function App() {
  setColorScheme("light");
  const arabicId = signal("kfgqpc-hafs"); // the web reader default
  const urduId = signal("noto-nastaliq-urdu");
  const dataset = signal("uthmani");
  const arabic = computed(() => ARABIC_FONTS.find((f) => f.id === arabicId()) ?? ARABIC_FONTS[0]!);
  const urdu = computed(() => URDU_FONTS.find((f) => f.id === urduId()) ?? URDU_FONTS[0]!);

  return (
    <View
      class="flex-1 pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]"
      style={{ backgroundColor: "#FFFFFF" }}
    >
      <View
        class="flex-row flex-wrap items-center gap-3 px-4 py-2"
        style={{ borderBottomWidth: 1, borderColor: RULE }}
      >
        <Selector label="Arabic font" ids={ARABIC_FONTS.map((f) => f.id)} value={arabicId} onChange={(id) => arabicId.set(id)} />
        <Selector label="Urdu" ids={URDU_FONTS.map((f) => f.id)} value={urduId} onChange={(id) => urduId.set(id)} />
        <Selector label="Database" ids={DATASETS} value={dataset} onChange={(id) => dataset.set(id)} />
      </View>
      <Scroll class="flex-1">
        {SAMPLE.arabic[dataset()]!.surahs.map((surah) => (
          <View>
            <View
              class="items-center py-5"
              style={{ backgroundColor: "#F5F5F5", borderBottomWidth: 1, borderColor: RULE }}
            >
              <Text style={{ fontSize: 15, fontWeight: "600", color: "#000000" }}>
                {`${surah.name_en} — Surah ${surah.number}`}
              </Text>
              <Text style={{ fontFamily: Hafs, fontSize: 28, color: ACCENT, marginTop: 4 }}>{surah.name_ar}</Text>
            </View>
            {surah.ayahs.map((ayah) => (
              <View class="px-8 py-4 gap-2" style={{ borderBottomWidth: 1, borderColor: RULE }}>
                <Text
                  style={{
                    fontFamily: arabic().family,
                    fontWeight: arabic().weight,
                    fontSize: 30,
                    lineHeight: 57,
                    color: "#000000",
                    writingDirection: "rtl",
                    textAlign: "right",
                  }}
                >
                  {ayah.text}
                </Text>
                <Text
                  style={{
                    fontFamily: urdu().family,
                    fontSize: 20,
                    lineHeight: 20 * urdu().height,
                    color: "#00000089",
                    writingDirection: "rtl",
                    textAlign: "right",
                  }}
                >
                  {urduFor(surah.number, ayah.n)}
                </Text>
                <Text style={{ fontFamily: Hafs, fontSize: 16, color: ACCENT, writingDirection: "rtl", textAlign: "right" }}>
                  {ayahMarker(ayah.n)}
                </Text>
              </View>
            ))}
          </View>
        ))}
      </Scroll>
    </View>
  );
}

mount(App);
