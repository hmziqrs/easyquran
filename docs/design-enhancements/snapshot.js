function changeArabicSize(direction) {
  const root = document.documentElement;
  const current =
    Number.parseFloat(getComputedStyle(root).getPropertyValue("--reader-arabic-size")) || 33;
  const next = current + direction * 2;
  if (next < 24 || next > 56) return;
  root.style.setProperty("--reader-arabic-size", `${next}px`);
}

function previewNotice() {
  let notice = document.querySelector("#preview-notice");
  if (!notice) {
    notice = document.createElement("div");
    notice.id = "preview-notice";
    notice.setAttribute("role", "status");
    notice.style.cssText =
      "position:fixed;inset:auto 20px 20px auto;z-index:1000;max-width:290px;border:1px solid var(--border);background:var(--surface);color:var(--foreground);padding:12px 16px;border-radius:10px;font:14px var(--font-sans);box-shadow:0 4px 20px #00000010";
    document.body.append(notice);
  }
  notice.textContent = "Visual preview. This page action is illustrative.";
  notice.hidden = false;
  window.clearTimeout(previewNotice.timeout);
  previewNotice.timeout = window.setTimeout(() => {
    notice.hidden = true;
  }, 2800);
}

document.addEventListener("click", (event) => {
  const target = event.target.closest("button, a");
  if (!target) return;
  const label = target.getAttribute("aria-label");
  if (label === "Toggle theme") {
    const root = document.documentElement;
    root.dataset.mode = root.dataset.mode === "light" ? "dark" : "light";
    window.parent.postMessage({ type: "preview-mode", mode: root.dataset.mode }, "*");
    return;
  }
  if (label === "Smaller Arabic text") {
    changeArabicSize(-1);
    return;
  }
  if (label === "Larger Arabic text") {
    changeArabicSize(1);
    return;
  }
  const href = target.getAttribute("href");
  if (href?.startsWith("#")) return;
  event.preventDefault();
  previewNotice();
});
document.addEventListener("submit", (event) => {
  event.preventDefault();
  previewNotice();
});
