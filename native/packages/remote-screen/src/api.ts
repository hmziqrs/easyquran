/** The dashboard API: one host, declared in `nsh.json` `capabilities.network`. */
export const API = "https://dashboard.example.com";

export type Metric = { id: string; label: string; value: string };

export async function getJson<T>(url: string, signal: AbortSignal): Promise<T> {
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(`status ${response.status}`);
  return (await response.json()) as T;
}

/** Registers a screen; a name the server already has comes back as a conflict. */
export async function registerScreen(name: string): Promise<{ id: string }> {
  const response = await fetch(`${API}/screens`, { method: "POST", body: JSON.stringify({ name }) });
  if (response.status === 409) throw new ScreenTaken();
  if (!response.ok) throw new Error(`status ${response.status}`);
  return (await response.json()) as { id: string };
}

/** The server's answer for a name it already serves. */
export class ScreenTaken extends Error {
  constructor() {
    super("screen name taken");
    this.name = "ScreenTaken";
  }
}
