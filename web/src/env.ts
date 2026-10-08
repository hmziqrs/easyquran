import { defineEnvVars } from "@sveltejs/kit/env";

/**
 * Every var is optional: unset reads as `""`. Each consumer already treats an
 * empty string exactly like a missing value (`?? ""` / `|| ""` guards, and
 * `resolveQuranDataEnvironment` falls back on a blank `PUBLIC_ENV`), so the
 * default keeps the old `$env/dynamic` behaviour where absent vars were simply
 * undefined. Kit requires a non-throwing validator to mark a var optional.
 */
const optionalString = (value: string | undefined): string => value ?? "";

/**
 * The web app's environment variables, declared once for kit 3's explicit env
 * system (`$app/env/public` / `$app/env/private`). Replaces the kit 2
 * `$env/dynamic/*` modules, which exposed every `PUBLIC_*` var implicitly.
 *
 * All vars are dynamic (read at runtime, never inlined at build time — matches
 * the old `$env/dynamic` semantics the delivery contract was built on), and
 * typed as plain strings. The API server's own vars (COOKIE_KEY, DATABASE_URL,
 * …) never pass through the web app and are not declared here.
 */
export const variables = defineEnvVars({
  // Server-only: Docker-internal SSR base + shared secret for the API's
  // identity gate, and the owner-profile JSON source.
  INTERNAL_QURAN_API_BASE: { schema: optionalString },
  INTERNAL_QURAN_API_TOKEN: { schema: optionalString },
  OWNER_SOURCE_URL: { schema: optionalString },
  // Browser-visible: API base (same-origin "/api" fallback), Quran artifact
  // environment switch, and the FCM push key.
  PUBLIC_API_BASE_URL: { public: true, schema: optionalString },
  PUBLIC_QURAN_API_BASE: { public: true, schema: optionalString },
  PUBLIC_ENV: { public: true, schema: optionalString },
  PUBLIC_FCM_VAPID_KEY: { public: true, schema: optionalString },
});
