use super::css::ComputedStyle;
#[derive(Clone,Copy,Debug,Default)] pub struct Rect{pub x:f32,pub y:f32,pub width:f32,pub height:f32}
#[derive(Clone,Debug)] pub struct LayoutBox{pub node_id:usize,pub rect:Rect,pub style:ComputedStyle,pub children:Vec<LayoutBox>}
fn len(v:&super::css::CssLength,base:f32,font:f32,viewport:(f32,f32))->f32{match v{super::css::CssLength::Px(x)=>*x,super::css::CssLength::Percent(x)=>base*(*x)/100.,super::css::CssLength::Em(x)=>font*(*x),super::css::CssLength::Rem(x)=>16.*(*x),super::css::CssLength::Vw(x)=>viewport.0*(*x)/100.,super::css::CssLength::Vh(x)=>viewport.1*(*x)/100.,super::css::CssLength::Vmin(x)=>viewport.0.min(viewport.1)*(*x)/100.,super::css::CssLength::Vmax(x)=>viewport.0.max(viewport.1)*(*x)/100.,super::css::CssLength::Zero=>0.,super::css::CssLength::Auto|super::css::CssLength::Calc(_)=>base}}
pub fn layout_node(id:usize,style:ComputedStyle,children:Vec<LayoutBox>,origin:(f32,f32),available:(f32,f32),viewport:(f32,f32))->LayoutBox{
 let w=match style.width{super::css::CssLength::Auto=>available.0,_=>len(&style.width,available.0,style.font_size,viewport)};
 let h=match style.height{super::css::CssLength::Auto=>0.,_=>len(&style.height,available.1,style.font_size,viewport)};
 let mut y=0.; let mut laid=Vec::new();
 if style.display=="flex" { let mut x=0.; for mut c in children {c.rect.x=x;c.rect.y=0.;x+=c.rect.width;c.rect.height=c.rect.height.max(1.);laid.push(c);} }
 else {for mut c in children{c.rect.x=0.;c.rect.y=y;y+=c.rect.height;laid.push(c);}}
 let content_h=if h>0.{h}else{y.max(1.)};
 LayoutBox{node_id:id,rect:Rect{x:origin.0,y:origin.1,width:w,height:content_h},style,children:laid}
}
pub fn layout_root(id:usize,style:ComputedStyle,viewport:(f32,f32))->LayoutBox{layout_node(id,style,Vec::new(),(0.,0.),viewport,viewport)}
