export const prerender = true;

import { SITE, PAGE_META } from "$lib/config/site";

/** llms.txt — a machine-readable index of the site, derived from PAGE_META. */
function render(): string {
  const pages = Object.values(PAGE_META)
    .map((p) => `- [${p.title}](${SITE.url}${p.path}): ${p.description}`)
    .join("\n");

  return `# ${SITE.name}

> ${SITE.tagline}

## Pages

${pages}
`;
}

export function GET() {
  return new Response(render(), {
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}
