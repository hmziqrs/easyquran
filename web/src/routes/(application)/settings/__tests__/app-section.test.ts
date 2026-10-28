import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

const SECTION_PATH = join(
  process.cwd(),
  "src/routes/(application)/settings/_components/AppSection.svelte",
);
const PRIVACY_PATH = join(
  process.cwd(),
  "src/routes/(application)/settings/_components/PrivacySection.svelte",
);
const EN_PATH = join(process.cwd(), "messages/en.json");
const AR_PATH = join(process.cwd(), "messages/ar.json");

const sectionSource = readFileSync(SECTION_PATH, "utf-8");
const privacySource = readFileSync(PRIVACY_PATH, "utf-8");
// SAFETY: the message catalogs are repo-owned flat JSON string maps validated by check-i18n.mjs; every leaf is a string, so the cast only names that shape.
const en = JSON.parse(readFileSync(EN_PATH, "utf-8")) as Record<string, string>;
// SAFETY: same flat-catalog contract as messages/en.json above; every leaf is a string.
const ar = JSON.parse(readFileSync(AR_PATH, "utf-8")) as Record<string, string>;

const APP_KEYS = [
  "settings_section_app",
  "settings_app_intro",
  "settings_app_installed",
  "settings_app_latest",
  "settings_app_status_up_to_date",
  "settings_app_status_update_ready",
  "settings_app_status_checking",
  "settings_app_status_unknown",
  "settings_app_check_updates",
  "settings_app_reload_update",
  "settings_app_install",
  "settings_app_install_standalone_note",
  "settings_app_install_browser_note",
  "settings_app_version_not_controlling",
] as const;

function appKeyNames(): string[] {
  const isAppKey = (key: string): boolean =>
    key === "settings_section_app" || key.startsWith("settings_app_");
  return [
    ...Object.keys(en).filter((key) => isAppKey(key)),
    ...Object.keys(ar).filter((key) => isAppKey(key)),
  ];
}

describe("settings App section copy", () => {
  it("keeps en/ar key parity for exactly the settings_app_* surface", () => {
    for (const key of APP_KEYS) {
      expect(en[key], `messages/en.json must define ${key}`).toBeTruthy();
      expect(ar[key], `messages/ar.json must define ${key}`).toBeTruthy();
    }
    expect(new Set(appKeyNames())).toEqual(new Set(APP_KEYS));
  });

  it("the section resolves copy through the SettingsCopy app surface, never raw paraglide", () => {
    expect(sectionSource).toContain('copy: SettingsCopy["app"]');
    expect(sectionSource).not.toContain("#lib/paraglide");
    expect(sectionSource).not.toContain("messages.js");
  });
});

describe("settings App section update wiring", () => {
  it("rows read the UpdateStore version surface and its explicit check", () => {
    expect(sectionSource).toContain("update.installedVersion");
    expect(sectionSource).toContain("update.latestVersion");
    expect(sectionSource).toContain("update.checkNow()");
    expect(sectionSource).toContain("update.apply()");
  });

  it("the status label resolves through a named early-return function, no nested ternaries", () => {
    expect(sectionSource).toContain("function statusLabel(): string {");
    expect(sectionSource).toContain("if (update.checking) return copy.statusChecking;");
    expect(sectionSource).toContain("if (update.available) return copy.statusUpdateReady;");
    expect(sectionSource).toContain("return copy.statusUnknown;");
  });

  it("browser install surfaces are client-guarded inside $effect blocks", () => {
    expect(sectionSource).toContain('window.matchMedia("(display-mode: standalone)")');
    expect(sectionSource).toContain(
      'window.addEventListener("beforeinstallprompt", onInstallPrompt)',
    );
    expect(sectionSource).toContain("event.preventDefault()");
    expect(sectionSource).toContain("await event.prompt()");
  });

  it("the privacy section no longer owns the version/check-updates row", () => {
    expect(privacySource).not.toContain("#lib/offline/update.svelte.js");
    expect(privacySource).not.toContain("updated.check()");
    expect(privacySource).not.toContain("copy.version");
  });
});
