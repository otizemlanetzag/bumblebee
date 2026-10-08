use thiserror::Error;
use url::Url;
use document::Document;
use fetch::Fetcher;
use render::RenderPipeline;
use security::{validate_navigation,SecurityPolicy};
use storage::Storage;

pub mod cache; pub mod css; pub mod document; pub mod fetch; pub mod js; pub mod layout; pub mod paint; pub mod parser; pub mod render; pub mod security; pub mod storage; pub mod style;

#[derive(Debug,Error)]
pub enum BrowserError{#[error("invalid URL: {0}")]InvalidUrl(#[from]url::ParseError),#[error("network error: {0}")]Network(#[from]reqwest::Error),#[error("navigation blocked: {0}")]Blocked(String)}

pub struct Page{pub url:Url,pub html:String,pub title:Option<String>,pub element_count:usize,pub document:Document,pub paint_commands:usize}
pub struct BrowserEngine{fetcher:Fetcher,policy:SecurityPolicy,pub storage:Storage,renderer:RenderPipeline}

impl BrowserEngine{
 pub fn new()->Result<Self,BrowserError>{Ok(Self{fetcher:Fetcher::new()?,policy:Default::default(),storage:Storage::default(),renderer:RenderPipeline})}
 pub async fn navigate(&mut self,input:&str)->Result<Page,BrowserError>{
  let url=Url::parse(input)?;validate_navigation(&url,&self.policy).map_err(BrowserError::Blocked)?;
  let html=self.fetcher.get(&url).await?;let document=parser::parse(&html);
  let title=document.title.clone();let element_count=document.element_count;
  let css=collect_styles(&document);let sheet=css::parse_stylesheet(&css);
  let (_,commands)=self.renderer.build_document(document.root.as_ref().unwrap_or(&document::Node::default()),&sheet,(1280.,720.));
  Ok(Page{url,html,title,element_count,document,paint_commands:commands.len()})
 }
 pub fn set_allow_insecure_http(&mut self,allow:bool){self.policy.allow_insecure_http=allow;}
}

fn collect_styles(document:&Document)->String{
 fn walk(n:&document::Node,out:&mut String){
  if n.tag.as_deref().map(|x|x.eq_ignore_ascii_case("style")).unwrap_or(false){if let Some(t)=&n.text{out.push_str(t);out.push('\n');}}
  for c in &n.children{walk(c,out);}
 }
 let mut out=String::new();if let Some(root)=&document.root{walk(root,&mut out);}out
}