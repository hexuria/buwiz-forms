Yes — it's possible, but not plug-and-play. With Meta's current Graph API setup (2024-2026), you can build exactly the RDO-scoped news feed you described, as long as you go through the official "Page Public Content Access" review.

Here's how it stands today:

## Short answer
- You **can** read public posts from pages you don't own (like `birgovphrdo38`, RDO 049, RDO 099, etc.) using the Graph API
- You **cannot** just scrape or call the API with a normal developer key — you need PPCA approval, business verification, and an app access token
- Once approved, your backend can poll each RDO page, store the posts, and serve only the matching RDO to e-BIRForms users

## How the Facebook API works now

Meta locked this down after Cambridge Analytica:

- A page access token alone only reads posts from pages you admin
- To read **publicly shared Page posts** from third-party pages, you need both the `pages_read_engagement` permission **and** the Page Public Content Access feature
- PPCA "lets you use an app to access and read public data from Facebook Pages without needing extra permissions" — that's the intended use for news aggregators
- Since May 2020, Meta restricted PPCA so you can't just pull any page by ID without review

What you get with PPCA (v18-v21 as of 2026):
- `/{page-id}/posts` → message, created_time, permalink_url, attachments{media}
- `/{page-id}/feed` → same, plus shared posts
- No comments data unless you moderate the page (removed in v11), but you don't need that for announcements

Rate limits are generous for this use: ~200 calls per user per hour, and you can batch 50 page IDs per request.

## What you need to get approved

1. **Create a Meta App** (Business type) in developers.facebook.com
2. **Business verification** — since this is for BIR/e-BIRForms, use government documents; Meta fast-tracks public-service use cases
3. **Apply for two features:**
   - Page Public Content Access (PPCA) — for reading posts
   - Page Public Metadata Access (PPMA) — for searching pages by name to build your list
4. **In the review, describe exactly your use case:** "Aggregate official BIR RDO Facebook announcements to display RDO-specific tax deadline extensions and office closures inside the taxpayer's e-BIRForms portal. No user data collected, public government pages only, attribution shown."
5. **Sign the supplemental terms** — you agree not to use data for surveillance, not to store beyond 90 days without refresh (you'll re-poll anyway)

Approval typically takes 2-4 weeks for government apps. Once granted, you use an **App Access Token**, not a user token — no login needed for your backend.

## Collating the RDO pages

There is no central BIR API listing them. Pages are scattered and use inconsistent naming:

- `birgovphrdo38` (your example)
- "BIR Revenue District Office 049-North Makati"
- "BIR Revenue District Office 014 - Bayombong, Nueva Vizcaya"
- "BIR Revenue District Office No. 103 - Butuan City"
- "BIR Revenue District Office 099 - Bukidnon" (10k followers as of March 2026) — from recent Meta data

Plan:
1. Manual seed list (~120 RDOs). Start with the pattern `birgovphrdoXX`, then search via PPMA: `GET /pages/search?q="BIR Revenue District Office"`
2. Store **page ID** (numeric), not username — usernames change
3. Map ID → RDO code (038, 049, etc.) in your DB

You can automate discovery, but expect to curate — some RDOs have multiple pages or none.

## Backend design for e-BIRForms

Simple architecture that works:

- **Poller (cron every 15 min):** for each page ID, call `/{id}/posts?fields=id,message,created_time,permalink_url,attachments,updated_time&limit=10&since=last_check`
- **Normalizer:** strip to title + snippet + link, tag with RDO code
- **Store:** Postgres table `rdo_posts(rdo_code, fb_post_id, created_at, message, url)`
- **API for e-BIRForms:** `GET /news?rdo=038` returns latest 20, ordered by created_time
- **Scope logic:** user's tax profile already has RDO → only that feed

Why this fits: you're not building a general social listening tool, you're mirroring official government communications. That's exactly what PPCA was designed to allow.

## Risks and workarounds

- **If PPCA is denied:** use a vetted third-party that already has PPCA (Supermetrics, Ayrshare, SociableKIT). They charge per page but save the review.
- **Pages with age/country restrictions:** API returns empty. Ask RDO admins to set "Page visibility: Public, no restrictions" — most BIR pages already are.
- **Breaking changes:** Meta deprecates fields yearly. Build a health check that alerts if `message` disappears.
- **No scraping fallback:** automated scraping violates Meta ToS and will get IP-blocked. Don't build it.

---

If you want, I can draft the exact Graph API calls and the app-review screencast script Meta requires, or help you build the initial RDO-to-Page-ID mapping table. Do you already have a list of the ~120 RDO codes you need to cover?