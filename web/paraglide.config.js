/** @type {import("@inlang/paraglide-js").ParaglideVitePluginOptions} */
export const paraglideOptions = {
  project: "./project.inlang",
  outdir: "./src/lib/paraglide",
  strategy: ["url", "baseLocale"],
  emitTsDeclarations: true,
  outputStructure: "message-modules",
  urlPatterns: [
    {
      pattern: "/",
      localized: [
        ["en", "/"],
        ["ar", "/ar/"],
      ],
    },
    {
      pattern: "/app",
      localized: [
        ["en", "/en/app"],
        ["ar", "/ar/app"],
      ],
    },
    {
      pattern: "/app/:path(.*)",
      localized: [
        ["en", "/en/app/:path(.*)"],
        ["ar", "/ar/app/:path(.*)"],
      ],
    },
  ],
};
