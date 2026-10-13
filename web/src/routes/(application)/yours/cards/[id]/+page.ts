// Personal, device-local page (§11.3): never prerendered, never cached at the
// edge — the (application) layout defaults to prerender, so the page-level
// override is mandatory (same as /yours itself).
export const prerender = false;
