use std::collections::HashMap;
use super::{parser::{Stylesheet,CssValue},selector::{parse_selector,matches_with_context,specificity},values::{CssColor,CssLength}};
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
pub fn compute_styles(node:&Node,sheet:&Stylesheet,parent:Option<&ComputedStyle>)->ComputedStyle{ compute_styles_at(node,sheet,parent,(1280.,720.)) }
pub fn compute_styles_at(node:&Node,sheet:&Stylesheet,parent:Option<&ComputedStyle>,viewport:(f32,f32))->ComputedStyle{
    compute_styles_with_context(node, sheet, parent, viewport, &[], &[], &[])
}
pub fn compute_styles_with_context(
    node:&Node, sheet:&Stylesheet, parent:Option<&ComputedStyle>, viewport:(f32,f32),
    ancestors:&[&Node], previous_siblings:&[&Node], following_siblings:&[&Node]
)->ComputedStyle{
    // CSS inheritance applies only to inherited properties, not every property.
    let mut out=ComputedStyle::default();
    if let Some(p)=parent { out.color=p.color; out.font_size=p.font_size; out.font_weight=p.font_weight.clone(); out.inherited_color=true; }
    let mut chosen:HashMap<String,(bool,super::selector::Specificity,usize,CssValue)>=HashMap::new();
    let mut active=sheet.rules.clone();
    for media in &sheet.media { if media_matches(&media.condition,viewport) { active.extend(media.rules.clone()); } }
    for (order,rule) in active.iter().enumerate() {
        let sel=parse_selector(&rule.selector);
        if !matches_with_context(&sel,node,ancestors,previous_siblings,following_siblings) { continue; }
        let spec=specificity(&sel);
        for d in &rule.declarations {
            let replace=match chosen.get(&d.property) {
                None=>true,
                Some((imp,old,ord,_))=>d.important>*imp||(d.important==*imp&&(spec>*old||(spec==*old&&order>=*ord)))
            };
            if replace { chosen.insert(d.property.clone(),(d.important,spec,order,d.value.clone())); }
        }
    }
    // Inline declarations have higher specificity than stylesheet selectors; !important is preserved.
    if let Some(inline)=node.attributes.get("style") {
        for (order,d) in super::parser::parse_declarations(inline).iter().enumerate() {
            let spec=super::selector::Specificity(u32::MAX,u32::MAX,u32::MAX);
            let replace=match chosen.get(&d.property) {
                None=>true,
                Some((imp,old,ord,_))=>d.important>*imp||(d.important==*imp&&(spec>*old||(spec==*old&&order>=*ord)))
            };
            if replace { chosen.insert(d.property.clone(),(d.important,spec,usize::MAX,d.value.clone())); }
        }
    }
    for(p,(_,_,_,v))in chosen { apply(&mut out,&p,v); }
    out
}
fn apply(s:&mut ComputedStyle,p:&str,v:CssValue){
    match(p,v){
        ("display",CssValue::Keyword(x))=>s.display=x,("position",CssValue::Keyword(x))=>s.position=x,
        ("width",CssValue::Length(x))=>s.width=x,("height",CssValue::Length(x))=>s.height=x,
        ("margin",CssValue::Length(x))=>s.margin=x,("padding",CssValue::Length(x))=>s.padding=x,
        ("color",CssValue::Color(x))=>s.color=x,("background",CssValue::Color(x))|("background-color",CssValue::Color(x))=>s.background_color=x,
        ("color",CssValue::Keyword(x)) if x=="inherit"=>s.inherited_color=true,
        ("font-size",CssValue::Length(CssLength::Px(x)))=>s.font_size=x,("font-size",CssValue::Length(CssLength::Em(x)))=>s.font_size*=x,
        ("font-size",CssValue::Length(CssLength::Rem(x)))=>s.font_size=16.0*x,("font-size",CssValue::Length(CssLength::Percent(x)))=>s.font_size*=x/100.0,
        ("font-weight",CssValue::Keyword(x))=>s.font_weight=x,
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

fn media_matches(condition:&str,viewport:(f32,f32))->bool{
 let c=condition.to_ascii_lowercase();
 let mut ok=true;
 if let Some(v)=extract_px(&c,"min-width"){ok&=viewport.0>=v;}
 if let Some(v)=extract_px(&c,"max-width"){ok&=viewport.0<=v;}
 if let Some(v)=extract_px(&c,"min-height"){ok&=viewport.1>=v;}
 if let Some(v)=extract_px(&c,"max-height"){ok&=viewport.1<=v;}
 if c.contains("screen")||c.contains("all")||c.trim().is_empty(){ok}else{ok}
}
fn extract_px(s:&str,name:&str)->Option<f32>{
 let i=s.find(name)?;let tail=&s[i+name.len()..];let tail=tail.trim_start_matches(|c:char|c==':'||c.is_whitespace());
 let end=tail.find(|c:char|c==')'||c==';'||c.is_whitespace()).unwrap_or(tail.len());
 tail[..end].trim_end_matches("px").parse().ok()
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn non_inherited_properties_reset_to_initial_values() {
   let parent=ComputedStyle { display:"flex".into(), width:CssLength::Px(400.0), ..ComputedStyle::default() };
   let child=compute_styles_at(&Node::default(),&Stylesheet::default(),Some(&parent),(800.0,600.0));
   assert_eq!(child.display,"block");
   assert_eq!(child.width,CssLength::Auto);
 }
 #[test] fn inline_style_overrides_normal_stylesheet_rule() {
   let mut node=Node::default();
   node.tag=Some("p".into());
   node.attributes.insert("style".into(),"width: 42px; color: blue".into());
   let sheet=super::super::parser::parse_stylesheet("p { width: 10px; color: red; }");
   let computed=compute_styles_at(&node,&sheet,None,(800.0,600.0));
   assert_eq!(computed.width,CssLength::Px(42.0));
   assert_eq!(computed.color,super::super::values::parse_color("blue").unwrap());
 }
}
