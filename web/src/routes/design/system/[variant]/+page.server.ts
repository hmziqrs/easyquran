import { error } from "@sveltejs/kit";

import { loadMixData } from "../../_variants/mix/data.server";
import { isVariantId } from "../../_variants/registry";
import type { PageServerLoad } from "./$types";

// Reads the local translation sqlite files, so it renders on request, never at build time.
export const prerender = false;

const TRANSLATIONS = ["en.sahih", "ur.maududi", "en.pickthall"];

export const load: PageServerLoad = ({ params }) => {
  if (!isVariantId(params.variant)) throw error(404, `Unknown system: ${params.variant}`);
  return { variant: params.variant, ...loadMixData(TRANSLATIONS) };
};
