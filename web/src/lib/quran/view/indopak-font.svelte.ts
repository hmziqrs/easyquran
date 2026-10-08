import { browser } from "$app/env";

type FontStatus = "idle" | "loading" | "ready" | "error";

export const indopakFont: { status: FontStatus } = $state({ status: "idle" });
let loading: Promise<void> | undefined;

export function loadIndopakFont(): Promise<void> {
  if (!browser || !document.fonts?.load) return Promise.resolve();
  if (loading) return loading;
  indopakFont.status = "loading";
  loading = document.fonts
    .load(
      '33px "IndoPak Reader Compat"',
      "\uE003\uE004\uE01A\uE01B\uE01C\uE01E\uE01F\uE021\uE022\u06DD\u06F1",
    )
    .then(
      (faces) => {
        indopakFont.status = faces.length ? "ready" : "error";
      },
      () => {
        indopakFont.status = "error";
      },
    );
  return loading;
}
