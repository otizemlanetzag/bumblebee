#[derive(Clone, Debug, PartialEq)]
pub enum CssLength {
    Px(f32), Percent(f32), Em(f32), Rem(f32),
    Vw(f32), Vh(f32), Vmin(f32), Vmax(f32),
    Auto, Zero, Calc(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssColor { pub r:u8, pub g:u8, pub b:u8, pub a:u8 }
impl CssColor {
    pub const BLACK:Self=Self{r:0,g:0,b:0,a:255};
    pub const WHITE:Self=Self{r:255,g:255,b:255,a:255};
}
impl Default for CssColor { fn default()->Self{Self::BLACK} }
impl Default for CssLength { fn default()->Self{Self::Auto} }

pub fn parse_length(s:&str)->Option<CssLength>{
    let s=s.trim().to_ascii_lowercase();
    if s=="auto"{return Some(CssLength::Auto)}
    if s=="0"{return Some(CssLength::Zero)}
    if s.starts_with("calc(") && s.ends_with(')'){return Some(CssLength::Calc(s[5..s.len()-1].trim().to_string()))}
    for (suffix,kind) in [("px",0),("%",1),("em",2),("rem",3),("vw",4),("vh",5),("vmin",6),("vmax",7)] {
        if let Some(n)=s.strip_suffix(suffix).and_then(|x|x.trim().parse::<f32>().ok()) {
            return Some(match kind {0=>CssLength::Px(n),1=>CssLength::Percent(n),2=>CssLength::Em(n),3=>CssLength::Rem(n),4=>CssLength::Vw(n),5=>CssLength::Vh(n),6=>CssLength::Vmin(n),_=>CssLength::Vmax(n)})
        }
    }
    None
}

pub fn parse_color(s:&str)->Option<CssColor>{
    let s=s.trim().to_ascii_lowercase();
    let named=match s.as_str() {
        "black"=>Some((0,0,0)), "white"=>Some((255,255,255)), "red"=>Some((255,0,0)),
        "green"=>Some((0,128,0)), "blue"=>Some((0,0,255)), "yellow"=>Some((255,255,0)),
        "cyan"|"aqua"=>Some((0,255,255)), "magenta"|"fuchsia"=>Some((255,0,255)),
        "gray"|"grey"=>Some((128,128,128)), "orange"=>Some((255,165,0)),
        "purple"=>Some((128,0,128)), "pink"=>Some((255,192,203)),
        "brown"=>Some((165,42,42)), "transparent"=>return Some(CssColor{r:0,g:0,b:0,a:0}), _=>None
    };
    if let Some((r,g,b))=named{return Some(CssColor{r,g,b,a:255})}
    if let Some(body)=s.strip_prefix("rgb(").and_then(|x|x.strip_suffix(')')) {
        let v:Vec<u8>=body.split(',').filter_map(|x|x.trim().parse().ok()).collect();
        if v.len()==3{return Some(CssColor{r:v[0],g:v[1],b:v[2],a:255})}
    }
    let h=s.strip_prefix('#')?;
    if h.len()==6{return Some(CssColor{r:u8::from_str_radix(&h[0..2],16).ok()?,g:u8::from_str_radix(&h[2..4],16).ok()?,b:u8::from_str_radix(&h[4..6],16).ok()?,a:255})}
    if h.len()==3{return Some(CssColor{r:u8::from_str_radix(&h[0..1].repeat(2),16).ok()?,g:u8::from_str_radix(&h[1..2].repeat(2),16).ok()?,b:u8::from_str_radix(&h[2..3].repeat(2),16).ok()?,a:255})}
    None
}
