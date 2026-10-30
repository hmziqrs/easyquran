import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

void main() {
  runApp(const QuranSampleApp());
}

/// Same font ids as the web reader (`web/src/lib/config/reader-fonts.ts`).
const arabicFonts = <({String id, String family, FontWeight? weight})>[
  (id: 'amiri', family: 'Amiri', weight: null),
  (id: 'scheherazade-new', family: 'Scheherazade New', weight: null),
  (id: 'noto-naskh-arabic', family: 'Noto Naskh Arabic', weight: null),
  (id: 'kfgqpc-hafs', family: 'KFGQPC Uthmanic Hafs', weight: null),
  (id: 'kfgqpc-taha-v1', family: 'KFGQPC Uthman Taha Naskh', weight: FontWeight.w400),
  (id: 'kfgqpc-taha-v1-bold', family: 'KFGQPC Uthman Taha Naskh', weight: FontWeight.w700),
];

const datasets = ['uthmani', 'simple', 'indopak'];
const defaultFontIx = 3; // kfgqpc-hafs, the web reader default

/// Open-license Nastaliq faces. Noto is the default: Mehr Nastaliq Web relies
/// on OpenType chaining contexts that not every desktop text stack applies
/// fully and flattens there. Leading is per font — Nastaliq needs ~2.5–3×
/// at 20px because Flutter does not derive it from the font's vertical
/// metrics the way a browser does.
const urduFonts = <({String id, String family, double height})>[
  (id: 'noto-nastaliq-urdu', family: 'Noto Nastaliq Urdu', height: 2.5),
  (id: 'mehr-nastaliq-web', family: 'Mehr Nastaliq Web', height: 2.9),
];

class Ayah {
  Ayah({required this.n, required this.ar, required this.ur});

  final int n;
  final String ar;
  final String ur;
}

class Surah {
  Surah({required this.number, required this.nameEn, required this.nameAr, required this.ayahs});

  final int number;
  final String nameEn;
  final String nameAr;
  final List<Ayah> ayahs;
}

Future<Map<String, dynamic>> loadSample() async {
  final raw = await rootBundle.loadString('assets/quran-sample.json');
  return jsonDecode(raw) as Map<String, dynamic>;
}

List<Surah> buildSurahs(Map<String, dynamic> sample, String dataset) {
  final arabic = (sample['arabic'][dataset]['surahs'] as List).cast<Map<String, dynamic>>();
  final urdu = sample['translations']['ur'] as Map<String, dynamic>;
  return arabic.map((surah) {
    final urList = ((urdu['${surah['number']}'] ?? []) as List).cast<Map<String, dynamic>>();
    return Surah(
      number: surah['number'] as int,
      nameEn: surah['name_en'] as String,
      nameAr: surah['name_ar'] as String,
      ayahs: (surah['ayahs'] as List).cast<Map<String, dynamic>>().map((ayah) {
        final matches = urList.where((u) => u['n'] == ayah['n']).toList();
        var ur = '';
        if (matches.isNotEmpty) {
          ur = matches.first['text'] as String;
        }
        return Ayah(n: ayah['n'] as int, ar: ayah['text'] as String, ur: ur);
      }).toList(),
    );
  }).toList();
}

String ayahMarker(int n) {
  final digits = n.toString().split('').map((c) => String.fromCharCode(0x0660 + int.parse(c))).join();
  return '﴿$digits﴾';
}

class QuranSampleApp extends StatelessWidget {
  const QuranSampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Quran Flutter Sample',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.light(useMaterial3: true),
      home: const ReaderScreen(),
    );
  }
}

class ReaderScreen extends StatefulWidget {
  const ReaderScreen({super.key});

  @override
  State<ReaderScreen> createState() => _ReaderScreenState();
}

class _ReaderScreenState extends State<ReaderScreen> {
  Map<String, dynamic>? sample;
  int fontIx = defaultFontIx;
  int urduIx = 0; // noto-nastaliq-urdu
  int datasetIx = 0;
  List<Surah> surahs = [];
  final scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    loadSample().then((data) {
      setState(() {
        sample = data;
        surahs = buildSurahs(data, datasets[datasetIx]);
      });
    });
  }

  @override
  void dispose() {
    scrollController.dispose();
    super.dispose();
  }

  void setDataset(int ix) {
    setState(() {
      datasetIx = ix;
      surahs = buildSurahs(sample!, datasets[ix]);
    });
  }

  @override
  Widget build(BuildContext context) {
    final data = sample;
    if (data == null) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    final font = arabicFonts[fontIx];
    final urdu = urduFonts[urduIx];
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.f9): () => setState(() {
              fontIx = (fontIx + 1) % arabicFonts.length;
            }),
        const SingleActivator(LogicalKeyboardKey.f10): () =>
            setDataset((datasetIx + 1) % datasets.length),
        const SingleActivator(LogicalKeyboardKey.f11): () => setState(() {
              urduIx = (urduIx + 1) % urduFonts.length;
            }),
      },
      child: Focus(
        autofocus: true,
        child: Scaffold(body: buildReader(font, urdu)),
      ),
    );
  }

  Widget buildReader(
      ({String id, String family, FontWeight? weight}) font,
      ({String id, String family, double height}) urdu) {
    return Column(
      children: [
        Material(
          elevation: 1,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            child: Row(
              children: [
                const Text('Arabic font', style: TextStyle(fontSize: 13)),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  value: font.id,
                  underline: const SizedBox.shrink(),
                  items: [
                    for (final f in arabicFonts)
                      DropdownMenuItem(
                          value: f.id, child: Text(f.id, style: const TextStyle(fontSize: 14))),
                  ],
                  onChanged: (id) => setState(() {
                    fontIx = arabicFonts.indexWhere((f) => f.id == id);
                  }),
                ),
                const SizedBox(width: 16),
                const Text('Urdu', style: TextStyle(fontSize: 13)),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  value: urdu.id,
                  underline: const SizedBox.shrink(),
                  items: [
                    for (final f in urduFonts)
                      DropdownMenuItem(
                          value: f.id, child: Text(f.id, style: const TextStyle(fontSize: 14))),
                  ],
                  onChanged: (id) => setState(() {
                    urduIx = urduFonts.indexWhere((f) => f.id == id);
                  }),
                ),
                const SizedBox(width: 16),
                const Text('Database', style: TextStyle(fontSize: 13)),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  value: datasets[datasetIx],
                  underline: const SizedBox.shrink(),
                  items: [
                    for (final d in datasets)
                      DropdownMenuItem(
                          value: d, child: Text(d, style: const TextStyle(fontSize: 14))),
                  ],
                  onChanged: (id) => setDataset(datasets.indexOf(id!)),
                ),
                const Spacer(),
                Text('${font.id} · ${urdu.id}',
                    style: TextStyle(fontSize: 12, color: Colors.grey.shade600)),
              ],
            ),
          ),
        ),
        Expanded(
          child: ListView(
            controller: scrollController,
            children: [
              for (final surah in surahs) ...[
                buildSurahHeader(surah),
                for (final ayah in surah.ayahs) buildAyah(ayah, font, urdu),
              ],
            ],
          ),
        ),
      ],
    );
  }

  Widget buildSurahHeader(Surah surah) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(vertical: 20),
      decoration: BoxDecoration(
        color: Colors.grey.shade100,
        border: Border(bottom: BorderSide(color: Colors.grey.shade300)),
      ),
      child: Column(
        children: [
          Text('${surah.nameEn} — Surah ${surah.number}',
              style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
          const SizedBox(height: 4),
          Text(surah.nameAr,
              style: const TextStyle(
                  fontFamily: 'KFGQPC Uthmanic Hafs',
                  fontSize: 28,
                  color: Color(0xFF00695C))),
        ],
      ),
    );
  }

  Widget buildAyah(Ayah ayah, ({String id, String family, FontWeight? weight}) font,
      ({String id, String family, double height}) urdu) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
      decoration: BoxDecoration(
          border: Border(bottom: BorderSide(color: Colors.grey.shade300))),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Directionality(
            textDirection: TextDirection.rtl,
            child: Text(
              ayah.ar,
              textAlign: TextAlign.right,
              style: TextStyle(
                fontFamily: font.family,
                fontWeight: font.weight,
                fontSize: 30,
                height: 1.9,
              ),
            ),
          ),
          const SizedBox(height: 8),
          Directionality(
            textDirection: TextDirection.rtl,
            child: Text(
              ayah.ur,
              textAlign: TextAlign.right,
              style: TextStyle(
                  fontFamily: urdu.family,
                  fontSize: 20,
                  height: urdu.height,
                  color: Colors.black54),
            ),
          ),
          const SizedBox(height: 8),
          Text(ayahMarker(ayah.n),
              style: const TextStyle(
                  fontFamily: 'KFGQPC Uthmanic Hafs',
                  fontSize: 16,
                  color: Color(0xFF00695C))),
        ],
      ),
    );
  }
}
