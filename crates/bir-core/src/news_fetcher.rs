use crate::db::{Announcement, Database};
use anyhow::Result;
use rss::Channel;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

pub struct NewsFetcher {
    db: Arc<Mutex<Database>>,
    client: reqwest::blocking::Client,
    feed_urls: Vec<String>,
}

impl NewsFetcher {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        // Here we configure the default URLs.
        // Due to Facebook's scraping protection, direct Facebook URLs will fail.
        // Users should use RSS proxy services (like RSS.app, RSS-Bridge, etc.) for Facebook pages.
        let feed_urls = vec![
            // Placeholder proxy URL for Facebook Fan Page (Requires a proxy service)
            "https://rss.app/feeds/facebook_bir_official.xml".to_string(),
            // Mock standard RSS feed
            "https://www.officialgazette.gov.ph/feed/".to_string(),
        ];

        Self {
            db,
            client: reqwest::blocking::Client::new(),
            feed_urls,
        }
    }

    /// Set custom RSS feeds to track.
    pub fn set_feeds(&mut self, urls: Vec<String>) {
        self.feed_urls = urls;
    }

    /// Fetches all configured RSS feeds and saves new items to the local database.
    pub fn fetch_and_sync(&self) -> Result<()> {
        info!("Starting news fetch from {} sources...", self.feed_urls.len());
        
        for url in &self.feed_urls {
            match self.fetch_feed(url) {
                Ok(announcements) => {
                    self.save_to_db(announcements);
                }
                Err(e) => {
                    error!("Failed to fetch feed {}: {}", url, e);
                }
            }
        }
        
        Ok(())
    }

    fn fetch_feed(&self, url: &str) -> Result<Vec<Announcement>> {
        let response = self.client.get(url).send()?.bytes()?;
        
        // Parse the RSS XML
        let channel = Channel::read_from(&response[..])?;
        
        let mut announcements = Vec::new();
        for item in channel.items() {
            let title = item.title().unwrap_or("No Title").to_string();
            let content = item.description().unwrap_or("").to_string();
            let published_at = item.pub_date().unwrap_or("").to_string();
            
            // Basic source determination
            let source = if url.contains("facebook") {
                "Facebook (Proxy)".to_string()
            } else if url.contains("officialgazette") {
                "Official Gazette".to_string()
            } else {
                "RSS Feed".to_string()
            };

            announcements.push(Announcement {
                id: None,
                source,
                title,
                content,
                published_at,
                read_status: false,
            });
        }

        Ok(announcements)
    }

    fn save_to_db(&self, announcements: Vec<Announcement>) {
        if let Ok(db_lock) = self.db.lock() {
            // Very naive insert. In a real scenario, we should check if the announcement already exists
            // based on title or a unique GUID. For now, we will clear and replace or just insert.
            // Let's assume we just want to fetch the latest.
            
            // To prevent infinite duplication, let's fetch existing titles.
            let existing_titles: Vec<String> = db_lock.list_announcements()
                .unwrap_or_default()
                .into_iter()
                .map(|a| a.title)
                .collect();

            for mut ann in announcements {
                if !existing_titles.contains(&ann.title) {
                    // Limit content length for UI display
                    if ann.content.len() > 200 {
                        ann.content = format!("{}...", &ann.content[..197]);
                    }
                    if let Err(e) = db_lock.save_announcement(&ann) {
                        error!("Failed to save announcement to DB: {}", e);
                    } else {
                        info!("Saved new announcement: {}", ann.title);
                    }
                }
            }
        }
    }
}
