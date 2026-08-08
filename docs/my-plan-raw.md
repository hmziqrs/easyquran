This document should not be touched by any AI agent. This is just my raw thoughts that I want to write in words in my own words, so it is easy for me to understand and navigate through the document. Again, no writing by AI agent. This is just for read only only in case of explicitly specified other than that never touch it.

API:
 - We completely load Arabic database in memory on the boot.
 - We do load all translated database on boot instead based on cached TTL and popularity. Because multiple languages are translated by multiple people, so it is possible a language translated by one or some people might not be as popular and not getting enough hit, so it doesn't make sense to keep them in hot cache, which is our memory.
 - Our service is proxied behind the cloud flare As well as art traefik Reverse proxy which is set up via Docker labels. For example, setting up the domain and stuff.
 - Our API is not being used a lot, but still we have a two-tier rate limiter. First one is our memory, and second one is SQLite for persistence. The reason I do not use Redis is due to resources it consumes for the little amount of work we need, like we only need rate limiting, so it makes sense to use an in-memory setup with this persistence in case we want to ban our IP for a longer time, and this way we can also set up a service or something where we can block some bad actors on reverse proxy level or if possible on Cloudflare level.


# Web:
- We don't completely rely on API at all.
- We have SSG generated arabic all (juzz, surah and global page no) pages. 
  - After the first boot we cache the arabic sqlite on browser disk and start using that in SPA mode and even stop relying on SSG pages.
  - SQlite database is 1.5mb and might take some time to load we use fall back of API first and then SSG last. as API first still gives us SPA experience. 
    - Also, falling back from API to SSG only. It works for if the page we are viewing does not have pagination. For example, Surah no:1 is 1 page only. So it makes sense. If the API is not working, we do SSG for next page.
    - But if it is multi-page, then SSG won't work. I mean it will create an experience of reloading the page even it's supposed to be one same page.
  - SQlite database is 1.5mb and might take some time to load we use fall back of API first and then SSG last. as API first still gives us SPA experience. Also, falling back from API to SSG only. It works for if the page we are viewing does not have page initiation. For example,
  - So this is how I supposed our arabic content to be rendered for the best experience. Also, SSG pages are hydrated once the cache is available. 
- Translation we do only (surah) translated pages (this is for SEO purposes) 


```
Another thing I am so much inclined towards using SPA experience is only because of authentication. We will implement authentication, and I believe for hydrating authentication on server-side generated pages is difficult, and I think anti-user experience even SSR is better than SSG, but still, SPA just provides a seamless experience for web apps with both authenticated and unauthenticated mode.;
```
