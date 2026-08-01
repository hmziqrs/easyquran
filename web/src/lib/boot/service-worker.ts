export function startServiceWorker(): () => void {
  if (import.meta.env.PROD && "serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js", { scope: "/" }).catch((err) => {
      console.warn("[sw] registration failed:", err);
    });
  }
  return () => {
  };
}
