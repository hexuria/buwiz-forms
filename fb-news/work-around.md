That error is Meta telling you exactly what we talked about: **you're trying to read a page you don't own, without PPCA.**

```
code 100, subcode 33 = "you don't have permission to see this object"
```

It has nothing to do with the ID being wrong. ID `100064932332446` is a real BIR page (RDO 051 - Pasay, from your CSV), but Graph API blocks it because your app token isn't approved for Page Public Content Access yet.

Here's how to read the error and fix it:

### Why you're seeing it

1. **No PPCA** — without Business Verification + approved PPCA, any call to `/{page_id}` or `/{page_id}/posts` for a page you don't admin returns this exact error. It's by design since 2020.
2. **Wrong token type** — if you used a User Access Token from Graph Explorer, it only works for pages you manage. App token (`APP_ID|APP_SECRET`) is required for PPCA, but it still won't work until approved.
3. **Page restrictions** — some RDO pages are set to "Philippines only". Even with PPCA, a US-based app token can get blocked. Yours is PH, so that's not it here.

Quick check: paste this in your browser:
```
https://www.facebook.com/100064932332446
```
It loads — so the page exists. The API is blocking you, not Facebook.

### How to confirm

Run this diagnostic (replace with your token):

```bash
curl -i "https://graph.facebook.com/v21.0/100064932332446?metadata=1&access_token=APP_ID|APP_SECRET"
```

You'll get the same error. Now try with a page you own:

```bash
curl -i "https://graph.facebook.com/v21.0/me/accounts?access_token=USER_TOKEN"
# copy one of your page IDs
curl "https://graph.facebook.com/v21.0/YOUR_PAGE_ID?fields=id,name&access_token=APP_ID|APP_SECRET"
```

That works — because you admin it.

### What to do right now (since you're independent)

**Don't waste time hitting real RDO IDs.** You will get subcode 33 every time until PPCA is approved.

Do this instead:

1. **Create your test page** (takes 2 min)
   - facebook.com/pages/create → "RDO News Dev Test"
   - Post 3 dummy announcements

2. **Get its ID**
```bash
curl "https://graph.facebook.com/v21.0/me/accounts?access_token=USER_TOKEN_WITH_pages_show_list"
```

3. **Use that ID in your code**
```python
PAGE_ID = "your_test_page_id"  # works now
# PAGE_ID = "100064932332446"  # will fail until PPCA
```

Build your entire poller, database, and e-BIRForms UI against the test page. When BIR sponsors the app (or you get PPCA via your own business), you just swap the ID list — zero code changes.

### If you must test a real RDO today

You have two legal workarounds:

**Option A — Get added as Analyst**
Ask the RDO 051 admin to add your Facebook account as "Analyst" on their page. Then your user token can read posts. It's manual, but works for 1-2 pages for demo.

**Option B — Use a PPCA-enabled proxy**
Services like Ayrshare already have approval. Their call:
```bash
curl "https://api.ayrshare.com/facebook/post?url=https://www.facebook.com/100064932332446"
```
Costs about $0.02 per fetch, no Meta review needed. Perfect for MVP.

### Bottom line

That error is not a bug — it's Meta's gate. You hit it because you're doing exactly what PPCA is meant to control: reading third-party public pages at scale.

Keep developing with your test page. When you're ready to go live, either:
- get BIR to host the app (Path 1 from last message), or
- plug in Ayrshare for the first 6 months

Want me to give you the 5-line Python script that swaps between test mode and production mode so you don't have to rewrite anything later?