pub mod cache;
pub mod document;
pub mod fetch;
pub mod js;
pub mod layout;
pub mod paint;
pub mod parser;
pub mod render;
pub mod security;
pub mod storage;
pub mod style;

use thiserror::Error;
use url::Url;
use document::Document;
use fetch::Fetcher;
use render::RenderPipeline;
use security::{validate_navigation,SecurityPolicy};
use storage::Storage;

#[derive(Debug,Error)]
pub enum BrowserError{#[error("invalid URL: {0}")]InvalidUrl(#[from]url::ParseError),#[error("network error: {0}")]Network(#[from]reqwest::Error),#[error("navigation blocked: {0}")]Blocked(String)}

pub struct Page{pub url:Url,pub html:String,pub title:Option<String>,pub element_count:usize,pub document:Document,pub paint_commands:usize}

pub struct BrowserEngine{fetcher:Fetcher,policy:SecurityPolicy,pub storage:Storage,renderer:RenderPipeline}
impl BrowserEngine{
 pub fn new()->Result<Self,BrowserError>{Ok(Self{fetcher:Fetcher::new()?,policy:Default::default(),storage:Storage::default(),renderer:RenderPipeline})}
 pub async fn navigate(&mut self,input:&str)->Result<Page,BrowserError>{let url=Url::parse(input)?;validate_navigation(&url,&self.policy).map_err(BrowserError::Blocked)?;let html=self.fetcher.get(&url).await?;let document=parser::parse(&html);let title=document.title.clone();let element_count=document.element_count;let(_,commands)=self.renderer.build(0,None,(1280.,720.));Ok(Page{url,html,title,element_count,document,paint_commands:commands.len()})}
 pub fn set_allow_insecure_http(&mut self,allow:bool){self.policy.allow_insecure_http=allow;}
}