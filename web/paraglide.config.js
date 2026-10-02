/** @type {import("@inlang/paraglide-js").ParaglideVitePluginOptions} */
export const paraglideOptions = {
  project: "./project.inlang",
  outdir: "./src/lib/paraglide",
  strategy: ["url", "baseLocale"],
  emitTsDeclarations: true,
  outputStructure: "message-modules",
  // URL scheme A: no custom urlPatterns — the generated default `/:locale/...`
  // pattern yields unprefixed `en` and `/{locale}` for every other locale
  // (localizeUrlDefaultPattern / deLocalizeUrlDefaultPattern in the generated
  // runtime). `en` is the base locale and stays prefix-less; `ar` gets `/ar`.
};
