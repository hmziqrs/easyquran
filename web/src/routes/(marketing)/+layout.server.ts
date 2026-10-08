import { getOwnerPublic } from "#lib/server/owner.js";
import type { OwnerPublic } from "#lib/types/owner.js";

export async function load(): Promise<{ owner: OwnerPublic; year: number }> {
  return { owner: await getOwnerPublic(), year: new Date().getFullYear() };
}
