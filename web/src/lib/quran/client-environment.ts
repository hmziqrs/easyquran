import { PUBLIC_ENV } from "$app/env/public";

import { QuranDataEnvironment, resolveQuranDataEnvironment } from "./environment.js";

/** PUBLIC_ENV with the same dev fallback as the build resolver in config/site.ts. */
export function clientQuranEnvironment(): QuranDataEnvironment {
  return resolveQuranDataEnvironment(
    PUBLIC_ENV,
    import.meta.env.DEV ? QuranDataEnvironment.Local : QuranDataEnvironment.Production,
  );
}
