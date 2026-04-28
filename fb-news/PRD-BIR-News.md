
**PRD: Official BIR Notices Aggregation**

**Objective**

Implement a compliant official-announcement system for the Rust + GPUI eBIRForms app. The app should surface BIR eBIRForms updates, deadline notices, tax calendar advisories, and RDO announcements from official or controlled sources, while keeping Facebook integration pluggable and disabled until Meta approval/backend support exists.

**Current Repo Context**

Workspace: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir`

Existing relevant files:

- [news_fetcher.rs](/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/crates/bir-core/src/news_fetcher.rs:1): current RSS-based fetcher with placeholder feeds.
- [db.rs](/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/crates/bir-core/src/db.rs:58): existing `Announcement` and SQLite persistence.
- [global_dashboard.rs](/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/crates/bir-desktop/src/views/global_dashboard.rs:55): already refreshes and renders announcements.
- [reference.rs](/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/crates/bir-core/src/reference.rs:1): RDO reference data.
- [Cargo.toml](/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/Cargo.toml:1): already has `tokio`, `reqwest`, `rss`, `regex`, `serde`, `chrono`, `rusqlite`.

**Product Requirements**

1. Show official BIR notices inside Global Dashboard.
2. Show latest official eBIRForms package version and download URL.
3. Normalize notices into structured fields instead of storing only title/content.
4. Deduplicate fetched notices reliably.
5. Work offline from cached SQLite data.
6. Support user-configurable RSS/proxy feeds.
7. Prepare a Facebook Graph provider interface, but do not ship Facebook scraping.
8. Keep networking off the GPUI UI thread.
9. Never store Meta access tokens in the desktop app.
10. Prefer official BIR website/CMS data before Facebook.

**Non-Goals**

- Do not implement Playwright/Facebook scraping.
- Do not scrape Facebook HTML.
- Do not embed a headless browser.
- Do not require Meta login in the desktop app.
- Do not make tax advice or legal conclusions from posts.
- Do not auto-submit or auto-download installers without explicit user action.

**Verified External Constraints**

As of April 25, 2026:

- Meta Page Feed docs require `pages_read_engagement` and `pages_read_user_content` for managed pages.
- For pages the app does not manage, Meta requires Page Public Content Access.
- Meta docs state PPCA use should use a system user access token to avoid rate limiting.
- Therefore Facebook Graph fetching must be server-side, not directly in the GPUI app.
- Official BIR eBIRForms page exposes content through BIR CMS:
  `https://bir-cms-ws.bir.gov.ph/api/pub/templates/3380/datasets?per_page=3000`
- Current official eBIRForms dataset includes ZIP link:
  `https://bir-cdn.bir.gov.ph/BIR/pdf/Offline%20eBIRForms%20Package%20v7.9.5.0%20setup.zip`

Sources:

- [Meta Page Feed](https://developers.facebook.com/docs/graph-api/reference/page/feed/)
- [Meta Features Reference](https://developers.facebook.com/docs/features-reference/)
- [Meta Permissions Reference](https://developers.facebook.com/docs/permissions/)
- [BIR eBIRForms](https://www.bir.gov.ph/ebirforms)

**Core Data Model**

Replace or extend `Announcement` with a richer notice model. Migration must preserve existing rows.

```rust
pub struct BirNotice {
    pub id: Option<i64>,
    pub external_id: String,
    pub source: String,
    pub source_kind: NoticeSourceKind,
    pub source_url: Option<String>,
    pub title: String,
    pub body: String,
    pub notice_type: NoticeType,
    pub rdo_code: Option<String>,
    pub form_code: Option<String>,
    pub deadline: Option<chrono::NaiveDate>,
    pub image_url: Option<String>,
    pub posted_at: Option<String>,
    pub fetched_at: String,
    pub raw_json: Option<String>,
    pub read_status: bool,
}
```

Enums:

```rust
pub enum NoticeSourceKind {
    BirCms,
    Rss,
    Manual,
    FacebookGraph,
}

pub enum NoticeType {
    EbirFormsVersion,
    Deadline,
    TaxCalendar,
    RdoAdvisory,
    SystemAdvisory,
    General,
}
```

**Database Requirements**

Add a new `bir_notices` table or migrate `announcements`.

Recommended table:

```sql
CREATE TABLE IF NOT EXISTS bir_notices (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  external_id TEXT NOT NULL,
  source TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_url TEXT,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  notice_type TEXT NOT NULL,
  rdo_code TEXT,
  form_code TEXT,
  deadline TEXT,
  image_url TEXT,
  posted_at TEXT,
  fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
  raw_json TEXT,
  read_status BOOLEAN NOT NULL DEFAULT 0,
  UNIQUE(source_kind, external_id)
);
```

Add indexes:

```sql
CREATE INDEX IF NOT EXISTS idx_bir_notices_posted_at ON bir_notices(posted_at);
CREATE INDEX IF NOT EXISTS idx_bir_notices_deadline ON bir_notices(deadline);
CREATE INDEX IF NOT EXISTS idx_bir_notices_form_code ON bir_notices(form_code);
CREATE INDEX IF NOT EXISTS idx_bir_notices_rdo_code ON bir_notices(rdo_code);
```

Keep `list_announcements()` temporarily or adapt UI to use `list_bir_notices()`.

**Fetcher Architecture**

Replace the single `NewsFetcher` implementation with provider-based fetchers:

```rust
pub trait NoticeProvider {
    fn source_kind(&self) -> NoticeSourceKind;
    fn fetch(&self) -> anyhow::Result<Vec<RawNotice>>;
}

pub struct BirCmsProvider;
pub struct RssProvider;
pub struct ManualProvider;
pub struct FacebookGraphProvider; // scaffold only
```

`NewsFetcher` should become an orchestrator:

```rust
pub struct NoticeFetcher {
    db: Arc<Mutex<Database>>,
    providers: Vec<Box<dyn NoticeProvider + Send + Sync>>,
}
```

Responsibilities:

- Fetch from all enabled providers.
- Normalize raw notices.
- Save via upsert.
- Continue if one provider fails.
- Log provider-level errors.
- Never block the GPUI thread.

**BIR CMS Provider**

Implement first.

Endpoint:

```text
GET https://bir-cms-ws.bir.gov.ph/api/pub/templates/3380/datasets?per_page=3000
Headers:
client-website-id: 2
origin: https://www.bir.gov.ph
```

Expected response shape:

```json
{
  "data": [
    {
      "id": 56771,
      "code": "eBIRForms",
      "name": "eBIRForms",
      "content": {
        "Contents": "<html content>"
      },
      "is_active": 1
    }
  ]
}
```

Parsing requirements:

- Extract eBIRForms package version from links like `Offline eBIRForms Package v7.9.5.0 setup.zip`.
- Extract package URL.
- Extract form codes from table rows, especially forms already supported or planned: `2551Q`, `1701`, `1701Q`, `1702Q`, `1702-RT`, `2550M`, `2550Q`.
- Extract revision text if present.
- Generate a `BirNotice` of type `EbirFormsVersion`.
- Use `external_id = "bir-cms:ebirforms:{version}"`.
- Use `source = "BIR eBIRForms"`.
- Use `source_url = https://www.bir.gov.ph/ebirforms`.

Optional but recommended:

- Send `HEAD` to ZIP URL and store `ETag`, `Last-Modified`, and `Content-Length` inside `raw_json`.

**RSS Provider**

Replace hardcoded placeholder feeds with config-driven feeds.

Config file:

```toml
[[sources.rss]]
name = "BIR Advisories Proxy"
url = "https://example.com/bir-advisories.xml"
enabled = false

[[sources.rss]]
name = "RDO 057 Proxy"
url = "https://example.com/rdo057.xml"
enabled = false
rdo_code = "057"
```

Requirements:

- Do not ship fake RSS URLs as enabled defaults.
- Parse RSS `guid`, `link`, `title`, `description`, `pubDate`.
- Normalize links and source names.
- Use `guid` if available, else hash `link + title + pubDate`.

**Manual Import**

Add a low-friction way to insert notices manually later. If UI scope is too large, implement DB/API first.

Manual input fields:

- title
- body
- source URL
- source name
- notice type
- form code
- RDO code
- deadline
- posted date

This lets the team paste official BIR/Facebook post links while waiting for Meta approval.

**Facebook Provider**

Scaffold only. Do not enable by default.

Design:

- Desktop calls your backend, not Meta directly.
- Backend owns system user access token.
- Backend endpoint example:
  `GET /api/bir-notices/facebook?handles=birgovph,birgovph.rdo057`
- Backend returns already-normalized `BirNotice` JSON.
- Desktop stores returned notices in SQLite.
- No comments, reactions, profiles, or user data.

Future Facebook fields:

```text
id
message
created_time
permalink_url
full_picture
attachments{media_type,url}
```

Handles to support later:

```text
birgovph
birgovph.rdo057
birgovph.rdo002
birgovph.rdo033
```

**Normalizer Requirements**

Implement functions to infer structured fields:

- Form code regex:
  `\b(2551Q|2550M|2550Q|1701Q|1701|1702Q|1702-RT|1702-MX|1702-EX)\b`
- eBIRForms version regex:
  `(?i)\bv?(\d+\.\d+\.\d+\.\d+)\b`
- Deadline phrase regex examples:
  `(?i)on or before ([A-Z]+ \d{1,2}, \d{4})`
  `(?i)deadline[:\s]+([A-Z]+ \d{1,2}, \d{4})`
- Countdown detection:
  `(?i)\b(\d+)\s+days?\s+left\b`

For countdowns, only compute a deadline if `posted_at` is known. If not known, store no deadline and keep the text.

**Dashboard UI Requirements**

Update Global Dashboard:

- Rename “Important News” to “BIR Notices” or keep if design consistency matters.
- Show source badge.
- Show notice type badge.
- Show form code when available.
- Show RDO code when available.
- Show deadline when available.
- Show latest eBIRForms package notice prominently if present.
- Calendar “Updated” badge must use `form_code` equality, not string search.
- If no notices exist, show a neutral empty state.
- Refresh button should call the new orchestrator.

**Offline Behavior**

- App must render cached notices with no network.
- Refresh failure must not clear existing notices.
- Store `fetched_at`.
- UI should not show scary errors unless all providers fail.
- Log detailed errors via `tracing`.

**Acceptance Criteria**

1. App compiles with stable Rust.
2. Global Dashboard still opens.
3. Refresh fetches official BIR eBIRForms data from BIR CMS.
4. SQLite contains one normalized eBIRForms version notice.
5. Running refresh twice does not duplicate the same notice.
6. Existing announcements are either migrated or still readable.
7. Calendar updated badges use normalized form codes.
8. No Facebook scraping code exists.
9. No Meta tokens are stored in desktop config or database.
10. Network failure leaves cached notices visible.

**Suggested Implementation Order**

1. Add `BirNotice`, enums, and DB table/upsert/list methods.
2. Add `RawNotice`, `NoticeProvider`, and `NoticeFetcher`.
3. Implement `BirCmsProvider`.
4. Implement normalizer and unit tests.
5. Wire `GlobalDashboardView::refresh_news` to new fetcher.
6. Update news cards to render structured notice metadata.
7. Add config loading for RSS sources.
8. Add RSS provider using config.
9. Add migration compatibility for old `announcements`.
10. Scaffold disabled `FacebookGraphProvider`.
11. Run `rtk cargo fmt`.
12. Run `rtk cargo check`.
13. Add focused unit tests for normalization and dedupe.

**Test Plan**

Unit tests in `bir-core`:

- parse eBIRForms version from ZIP URL
- parse form codes from body text
- parse deadline phrase
- no deadline for countdown without posted date
- upsert does not duplicate notices
- RSS item without GUID hashes deterministically

Manual test:

- launch app
- open Global Dashboard
- click Refresh
- confirm eBIRForms notice appears
- quit/reopen app
- confirm cached notice remains

**Risk Notes**

- BIR CMS is public but unofficially documented. Keep provider isolated so endpoint changes only affect one module.
- Facebook must remain backend-gated until PPCA approval.
- BIR content may contain large base64 images; strip or truncate raw HTML before UI display.
- Existing DB uses SQLCipher; keep migrations non-destructive.

This is the version I would hand to another agent for implementation.