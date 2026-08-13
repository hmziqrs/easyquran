import { cpSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const source = join(webRoot, "node_modules", "oxlint-plugin-anti-slop", "src");
const target = join(webRoot, ".cache", "anti-slop");

if (!existsSync(source)) {
  throw new Error("oxlint-plugin-anti-slop is not installed; run pnpm install first.");
}

mkdirSync(target, { recursive: true });
for (const entry of readdirSync(source)) {
  cpSync(join(source, entry), join(target, entry), { force: true, recursive: true });
}
