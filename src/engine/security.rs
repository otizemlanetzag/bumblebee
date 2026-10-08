use url::Url;
#[derive(Clone,Debug)]pub struct SecurityPolicy{pub allow_insecure_http:bool,pub max_redirects:usize}
impl Default for SecurityPolicy{fn default()->Self{Self{allow_insecure_http:true,max_redirects:10}}}
pub fn validate_navigation(u:&Url,p:&SecurityPolicy)->Result<(),String>{match u.scheme(){"http"|"https"=>{},s=>return Err(format!("unsupported URL scheme: {s}"))}if u.scheme()=="http"&&!p.allow_insecure_http{return Err("insecure HTTP disabled".into())}Ok(())}
