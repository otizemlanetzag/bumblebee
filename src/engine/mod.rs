pub mod document;
pub mod fetch;
pub mod parser;

use thiserror::Error;
use url::Url;

use document::Document;
use fetch::Fetcher;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
}

pub struct Page {
    pub url: Url,
    pub html: String,
    pub title: Option<String>,
    pub element_count: usize,
    pub document: Document,
}

pub struct BrowserEngine {
    fetcher: Fetcher,
}

impl BrowserEngine {
    pub fn new() -> Result<Self, BrowserError> {
        Ok(Self { fetcher: Fetcher::new()? })
    }

    pub async fn navigate(&mut self, input: &str) -> Result<Page, BrowserError> {
        let url = Url::parse(input)?;
        let html = self.fetcher.get(&url).await?;
        let document = parser::parse(&html);

        let title = document.title.clone();
        let element_count = document.element_count;

        Ok(Page { url, html, title, element_count, document })
    }
}
