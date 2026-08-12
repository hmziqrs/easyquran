import { getOwnerPublic } from "$lib/server/owner";
import type { OwnerPublic } from "$lib/types/owner";

export async function load(): Promise<{ owner: OwnerPublic; year: number }> {
  return { owner: await getOwnerPublic(), year: new Date().getFullYear() };
}
