use std::collections::HashMap;
use super::{parser::{Stylesheet,CssValue},selector::{parse_selector,matches,specificity},values::{CssColor,CssLength}};
use crate::engine::document::Node;

#[derive(Clone,Debug)]
pub struct ComputedStyle {
    pub display:String,pub position:String,pub width:CssLength,pub height:CssLength,
    pub margin:CssLength,pub padding:CssLength,pub color:CssColor,pub background_color:CssColor,
    pub font_size:f32,pub font_weight:String,pub inherited_color:bool,
    pub flex_direction:String,pub flex_wrap:String,pub justify_content:String,pub align_items:String,
    pub flex_grow:f32,pub flex_shrink:f32,pub flex_basis:CssLength,
    pub grid_template_columns:String,pub grid_template_rows:String,pub grid_gap:CssLength,
    pub overflow:String,pub z_index:i32
}
impl Default for ComputedStyle {
    fn default()->Self{Self{display:"block".into(),position:"static".into(),width:CssLength::Auto,height:CssLength::Auto,margin:CssLength::Zero,padding:CssLength::Zero,color:CssColor::BLACK,background_color:CssColor{r:0,g:0,b:0,a:0},font_size:16.,font_weight:"normal".into(),inherited_color:false,flex_direction:"row".into(),flex_wrap:"nowrap".into(),justify_content:"flex-start".into(),align_items:"stretch".into(),flex_grow:0.,flex_shrink:1.,flex_basis:CssLength::Auto,grid_template_columns:"none".into(),grid_template_rows:"none".into(),grid_gap:CssLength::Zero,overflow:"visible".into(),z_index:0}}
}
pub fn compute_styles(node:&Node,sheet:&Stylesheet,parent:Option<&ComputedStyle>)->ComputedStyle{
    let mut out=parent.cloned().unwrap_or_default(); out.inherited_color=parent.is_some();
    let mut chosen:HashMap<String,(bool,super::selector::Specificity,usize,CssValue)>=HashMap::new();
    for (order,rule) in sheet.rules.iter().enumerate(){let sel=parse_selector(&rule.selector);if !matches(&sel,node){continue}let spec=specificity(&sel);for d in &rule.declarations{let replace=match chosen.get(&d.property){None=>true,Some((imp,old,ord,_))=>d.important>*imp||(d.important==*imp&&(spec>*old||(spec==*old&&order>=*ord)))};if replace{chosen.insert(d.property.clone(),(d.important,spec,order,d.value.clone()));}}}
    for(p,(_,_,_,v))in chosen{apply(&mut out,&p,v);} out
}
fn apply(s:&mut ComputedStyle,p:&str,v:CssValue){
    match(p,v){
        ("display",CssValue::Keyword(x))=>s.display=x,("position",CssValue::Keyword(x))=>s.position=x,
        ("width",CssValue::Length(x))=>s.width=x,("height",CssValue::Length(x))=>s.height=x,
        ("margin",CssValue::Length(x))=>s.margin=x,("padding",CssValue::Length(x))=>s.padding=x,
        ("color",CssValue::Color(x))=>s.color=x,("background",CssValue::Color(x))|("background-color",CssValue::Color(x))=>s.background_color=x,
        ("font-size",CssValue::Length(CssLength::Px(x)))=>s.font_size=x,("font-weight",CssValue::Keyword(x))=>s.font_weight=x,
        ("flex-direction",CssValue::Keyword(x))=>s.flex_direction=x,("flex-wrap",CssValue::Keyword(x))=>s.flex_wrap=x,
        ("justify-content",CssValue::Keyword(x))=>s.justify_content=x,("align-items",CssValue::Keyword(x))=>s.align_items=x,
        ("flex-grow",CssValue::Raw(x))|("flex-grow",CssValue::Keyword(x))=>s.flex_grow=x.parse().unwrap_or(0.),
        ("flex-shrink",CssValue::Raw(x))|("flex-shrink",CssValue::Keyword(x))=>s.flex_shrink=x.parse().unwrap_or(1.),
        ("flex-basis",CssValue::Length(x))=>s.flex_basis=x,
        ("grid-template-columns",CssValue::Raw(x))|("grid-template-columns",CssValue::Keyword(x))=>s.grid_template_columns=x,
        ("grid-template-rows",CssValue::Raw(x))|("grid-template-rows",CssValue::Keyword(x))=>s.grid_template_rows=x,
        ("gap",CssValue::Length(x))|("grid-gap",CssValue::Length(x))=>s.grid_gap=x,
        ("overflow",CssValue::Keyword(x))=>s.overflow=x,("z-index",CssValue::Raw(x))|("z-index",CssValue::Keyword(x))=>s.z_index=x.parse().unwrap_or(0),
        _=>{}
    }
}
