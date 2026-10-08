use reqwest::{Client, Url};

pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .user_agent("Bumblebee/0.1 (Rust browser engine)")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;
        Ok(Self { client })
    }

    pub async fn get(&self, url: &Url) -> Result<String, reqwest::Error> {
        let response = self.client.get(url.clone()).send().await?;
        response.error_for_status()?.text().await
    }
}
