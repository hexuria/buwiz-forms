use crate::db::{BirNotice, Database, NoticeSourceKind, NoticeType};
use anyhow::Result;
use rss::Channel;
use std::sync::{Arc, Mutex};
use tracing::{error, info};
use serde::Deserialize;

pub trait NoticeProvider: Send + Sync {
    fn source_kind(&self) -> NoticeSourceKind;
    fn fetch(&self) -> Result<Vec<BirNotice>>;
}

pub struct NoticeFetcher {
    db: Arc<Mutex<Database>>,
    providers: Vec<Box<dyn NoticeProvider + Send + Sync>>,
}

impl NoticeFetcher {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self {
            db,
            providers: vec![
                Box::new(BirCmsProvider::new()),
                Box::new(RssProvider::new(vec!["https://www.officialgazette.gov.ph/feed/".to_string()])),
                Box::new(FacebookGraphProvider),
            ],
        }
    }

    pub fn fetch_and_sync(&self) -> Result<()> {
        info!("Starting notice fetch from {} providers...", self.providers.len());
        
        if let Ok(db_lock) = self.db.lock() {
            for provider in &self.providers {
                match provider.fetch() {
                    Ok(notices) => {
                        for notice in notices {
                            if let Err(e) = db_lock.save_bir_notice(&notice) {
                                error!("Failed to save notice {}: {}", notice.external_id, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch from {:?}: {}", provider.source_kind(), e);
                    }
                }
            }
        }
        
        Ok(())
    }
}

pub struct BirCmsProvider {
    client: reqwest::blocking::Client,
}

impl BirCmsProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct BirCmsResponse {
    data: Vec<BirCmsDataset>,
}

#[derive(Deserialize)]
struct BirCmsDataset {
    id: i64,
    code: String,
    name: String,
    content: Option<BirCmsContent>,
    is_active: i32,
}

#[derive(Deserialize)]
struct BirCmsContent {
    #[serde(rename = "Contents")]
    contents: Option<String>,
}

impl NoticeProvider for BirCmsProvider {
    fn source_kind(&self) -> NoticeSourceKind {
        NoticeSourceKind::BirCms
    }

    fn fetch(&self) -> Result<Vec<BirNotice>> {
        let url = "https://bir-cms-ws.bir.gov.ph/api/pub/templates/3380/datasets?per_page=3000";
        let response_bytes = self.client.get(url)
            .header("client-website-id", "2")
            .header("origin", "https://www.bir.gov.ph")
            .send()?
            .bytes()?;
        let response: BirCmsResponse = serde_json::from_slice(&response_bytes)?;

        let mut notices = Vec::new();

        for item in response.data {
            if item.is_active != 1 {
                continue;
            }

            // Extract content
            let html_content = item.content.and_then(|c| c.contents).unwrap_or_default();
            
            // Minimal regex parsing for eBIRForms version and link
            let mut title = format!("{} Update", item.name);
            let mut notice_type = NoticeType::General;
            let mut external_id = format!("bir-cms:{}", item.id);
            
            if item.code == "eBIRForms" {
                notice_type = NoticeType::EbirFormsVersion;
                // Try to find version
                if let Some(v_idx) = html_content.find("Offline eBIRForms Package v") {
                    let end_idx = html_content[v_idx..].find("setup").unwrap_or(30) + v_idx;
                    title = html_content[v_idx..end_idx].trim().to_string();
                    external_id = format!("bir-cms:ebirforms:{}", title.replace(" ", "-"));
                }
            }

            notices.push(BirNotice {
                id: None,
                external_id,
                source: "BIR CMS".to_string(),
                source_kind: NoticeSourceKind::BirCms,
                source_url: Some("https://www.bir.gov.ph/ebirforms".to_string()),
                title,
                body: "New updates available from BIR CMS.".to_string(), // In reality we should parse HTML to text
                notice_type,
                rdo_code: None,
                form_code: None, // Can extract from HTML using regex if needed
                deadline: None,
                image_url: None,
                posted_at: Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
                fetched_at: chrono::Local::now().to_rfc3339(),
                raw_json: Some(html_content),
                read_status: false,
            });
        }

        Ok(notices)
    }
}

pub struct RssProvider {
    client: reqwest::blocking::Client,
    feed_urls: Vec<String>,
}

impl RssProvider {
    pub fn new(feed_urls: Vec<String>) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            feed_urls,
        }
    }
}

impl NoticeProvider for RssProvider {
    fn source_kind(&self) -> NoticeSourceKind {
        NoticeSourceKind::Rss
    }

    fn fetch(&self) -> Result<Vec<BirNotice>> {
        let mut notices = Vec::new();
        
        for url in &self.feed_urls {
            let response = self.client.get(url).send()?.bytes()?;
            if let Ok(channel) = Channel::read_from(&response[..]) {
                for item in channel.items() {
                    let title = item.title().unwrap_or("No Title").to_string();
                    let content = item.description().unwrap_or("").to_string();
                    let published_at = item.pub_date().unwrap_or("").to_string();
                    let link = item.link().unwrap_or("").to_string();
                    let guid = item.guid().map(|g| g.value().to_string()).unwrap_or_else(|| {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        link.hash(&mut hasher);
                        title.hash(&mut hasher);
                        format!("rss-hash-{}", hasher.finish())
                    });

                    let mut body = content;
                    if body.len() > 200 {
                        body = format!("{}...", &body[..197]);
                    }

                    notices.push(BirNotice {
                        id: None,
                        external_id: guid,
                        source: "RSS Feed".to_string(),
                        source_kind: NoticeSourceKind::Rss,
                        source_url: Some(link),
                        title,
                        body,
                        notice_type: NoticeType::General,
                        rdo_code: None,
                        form_code: None,
                        deadline: None,
                        image_url: None,
                        posted_at: Some(published_at),
                        fetched_at: chrono::Local::now().to_rfc3339(),
                        raw_json: None,
                        read_status: false,
                    });
                }
            }
        }
        
        Ok(notices)
    }
}

pub struct FacebookGraphProvider;

impl NoticeProvider for FacebookGraphProvider {
    fn source_kind(&self) -> NoticeSourceKind {
        NoticeSourceKind::FacebookGraph
    }

    fn fetch(&self) -> Result<Vec<BirNotice>> {
        // Scaffold only, do not implement actual scraping per PRD.
        // Return empty array.
        Ok(vec![])
    }
}
