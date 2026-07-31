// The design gallery is prerendered like the rest of the site so it can be
// reviewed on any static preview deploy. It is deliberately NOT in
// MARKETING_PAGES, so it never reaches the sitemap, llms.txt or the .md/.txt
// text variants; the layout additionally marks every page noindex.
export const prerender = true;
