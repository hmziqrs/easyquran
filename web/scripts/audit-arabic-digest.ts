import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";

const GOLDEN = {
  "quran-uthmani.sqlite": "581cc5405831bc072fccd8db55cd1db72c5c5440c39bd975edbf03447efecf53",
  "quran-simple-clean.sqlite": "a0c52760d6660ac5be1de5c76bb10df7a839a3e8a87ecb0e636fe2ed45b2e4a3",
} satisfies Record<string, string>;

const dir = path.resolve(process.cwd(), "..", "db", "quran", "tanzil", "arabic");
let ok = true;
for (const [file, want] of Object.entries(GOLDEN)) {
  const got = createHash("sha256")
    .update(readFileSync(path.join(dir, file)))
    .digest("hex");
  if (got === want) {
    console.log(`PASS  ${file}`);
  } else {
    ok = false;
    console.log(`FAIL  ${file}\n        got  ${got}\n        want ${want}`);
  }
}
if (!ok) process.exit(1);
console.log("\nArabic sqlite golden digests verified.");
