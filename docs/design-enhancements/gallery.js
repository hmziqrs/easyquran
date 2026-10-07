const descriptions = {
  current: "Current design, unchanged. Same captured content for comparison.",
  outline:
    "White surfaces, stronger outlines, existing colors on edges and labels. Same flat design.",
  color:
    "Existing four colors in separate cards. Softer corners and clearer verse grouping. Flat white reading surface.",
};
const query = new URLSearchParams(location.search);
const state = {
  concept: Object.hasOwn(descriptions, query.get("concept")) ? query.get("concept") : "outline",
  type: query.get("type") === "manrope" ? "manrope" : "nunito",
  mode: query.get("mode") === "dark" ? "dark" : "light",
  view: ["home", "reader"].includes(query.get("view")) ? query.get("view") : "both",
  device: query.get("device") === "mobile" ? "mobile" : "desktop",
};
const previews = document.querySelector(".previews");
const viewSelect = document.querySelector("#view");
const deviceSelect = document.querySelector("#device");
const typeSelect = document.querySelector("#type");
const modeButton = document.querySelector("#mode");
const frames = [...document.querySelectorAll("iframe")];

function resizeFrames() {
  const width = state.device === "mobile" ? 390 : 1440;
  const height = state.device === "mobile" ? 1040 : 1180;
  for (const frame of frames) {
    const viewport = frame.parentElement;
    const available = viewport.clientWidth;
    if (available === 0) continue;
    const scale = available / width;
    frame.style.width = `${width}px`;
    frame.style.height = `${height}px`;
    frame.style.transform = `scale(${scale})`;
    viewport.style.height = `${height * scale}px`;
  }
}

function render() {
  document.querySelector("#description").textContent = descriptions[state.concept];
  for (const button of document.querySelectorAll("button[data-concept]")) {
    button.setAttribute("aria-pressed", String(button.dataset.concept === state.concept));
  }
  modeButton.setAttribute("aria-pressed", String(state.mode === "dark"));
  typeSelect.value = state.type;
  viewSelect.value = state.view;
  deviceSelect.value = state.device;
  previews.dataset.view = state.view;
  previews.dataset.device = state.device;
  for (const frame of frames) {
    const page = frame.closest(".preview").dataset.page;
    const src = `${page}.html?concept=${state.concept}&mode=${state.mode}&type=${state.type}`;
    const link = document.querySelector(`#${page}-link`);
    link.href = src;
    const doc = frame.contentDocument;
    if (doc?.documentElement.dataset.page === page) {
      doc.documentElement.dataset.concept = state.concept;
      doc.documentElement.dataset.mode = state.mode;
      doc.documentElement.dataset.type = state.type;
    } else {
      frame.src = src;
    }
  }
  const params = new URLSearchParams(state);
  history.replaceState(null, "", `?${params}`);
  requestAnimationFrame(resizeFrames);
}

for (const button of document.querySelectorAll("button[data-concept]")) {
  button.addEventListener("click", () => {
    state.concept = button.dataset.concept;
    render();
  });
}
typeSelect.addEventListener("change", () => {
  state.type = typeSelect.value;
  render();
});
viewSelect.addEventListener("change", () => {
  state.view = viewSelect.value;
  render();
});
deviceSelect.addEventListener("change", () => {
  state.device = deviceSelect.value;
  render();
});
modeButton.addEventListener("click", () => {
  state.mode = state.mode === "light" ? "dark" : "light";
  render();
});
window.addEventListener("message", (event) => {
  if (!frames.some((frame) => frame.contentWindow === event.source)) return;
  if (event.data?.type !== "preview-mode") return;
  if (!["light", "dark"].includes(event.data.mode)) return;
  state.mode = event.data.mode;
  render();
});
const observer = new ResizeObserver(resizeFrames);
for (const frame of frames) {
  observer.observe(frame.parentElement);
  frame.addEventListener("load", render);
}
render();
