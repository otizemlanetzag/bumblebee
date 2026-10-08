#[derive(Clone, Debug, PartialEq)]
pub enum CssLength { Px(f32), Percent(f32), Em(f32), Rem(f32), Auto, Zero }
impl Default for CssLength { fn default()->Self{Self::Auto} }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssColor { pub r:u8,pub g:u8,pub b:u8,pub a:u8 }
impl CssColor {
    pub const BLACK:Self=Self{r:0,g:0,b:0,a:255};
    pub const WHITE:Self=Self{r:255,g:255,b:255,a:255};
}
impl Default for CssColor { fn default()->Self{Self::BLACK} }

pub fn parse_length(s:&str)->Option<CssLength>{
    let s=s.trim().to_ascii_lowercase();
    if s=="auto"{return Some(CssLength::Auto)}
    if s=="0"{return Some(CssLength::Zero)}
    for (suffix,kind) in [("px",0),("%",1),("em",2),("rem",3)] {
        if let Some(n)=s.strip_suffix(suffix).and_then(|x|x.trim().parse::<f32>().ok()) {
            return Some(match kind {0=>CssLength::Px(n),1=>CssLength::Percent(n),2=>CssLength::Em(n),_=>CssLength::Rem(n)})
        }
    }
    None
}
pub fn parse_color(s:&str)->Option<CssColor>{
    let s=s.trim().to_ascii_lowercase();
    let named=match s.as_str() {
        "black"=>Some((0,0,0)), "white"=>Some((255,255,255)), "red"=>Some((255,0,0)),
        "green"=>Some((0,128,0)), "blue"=>Some((0,0,255)), "transparent"=>return Some(CssColor{r:0,g:0,b:0,a:0}),
        _=>None
    };
    if let Some((r,g,b))=named{return Some(CssColor{r,g,b,a:255})}
    let h=s.strip_prefix('#')?;
    if h.len()==6 {
        Some(CssColor{r:u8::from_str_radix(&h[0..2],16).ok()?,g:u8::from_str_radix(&h[2..4],16).ok()?,b:u8::from_str_radix(&h[4..6],16).ok()?,a:255})
    } else if h.len()==3 {
        Some(CssColor{r:u8::from_str_radix(&h[0..1].repeat(2),16).ok()?,g:u8::from_str_radix(&h[1..2].repeat(2),16).ok()?,b:u8::from_str_radix(&h[2..3].repeat(2),16).ok()?,a:255})
    } else { None }
}
