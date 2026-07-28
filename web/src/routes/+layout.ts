// Global routing config. `prerender` is declared per route group:
//   (marketing)  → prerendered public pages
//   (application) → prerendered app shell
// so each half can change independently (e.g. the app moving to CSR-only).
export const trailingSlash = "ignore";
