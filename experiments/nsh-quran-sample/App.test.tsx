import { expect, open, test } from "nsh/test";

test("shows the reader with both surahs", async () => {
  const app = await open("./App.tsx");
  expect(app.texts()).toContain("Al-Fatihah — Surah 1");
  expect(app.texts()).toContain("Al-Baqarah — Surah 2");
});
