mod engine;

use std::env;
use engine::BrowserEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = env::args().nth(1).unwrap_or_else(|| "https://example.com".into());
    let mut browser = BrowserEngine::new()?;
    let page = browser.navigate(&target).await?;

    println!("Bumblebee");
    println!("URL: {}", page.url);
    println!("Title: {}", page.title.as_deref().unwrap_or("(untitled)"));
    println!("Elements: {}", page.element_count);
    println!("HTML bytes: {}", page.html.len());
    Ok(())
}
