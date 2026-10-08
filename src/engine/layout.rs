use super::style::{Display,Style};
#[derive(Clone,Copy,Debug,Default)] pub struct Rect{pub x:f32,pub y:f32,pub width:f32,pub height:f32}
#[derive(Clone,Debug)] pub struct LayoutBox{pub node_id:usize,pub rect:Rect,pub style:Style,pub children:Vec<LayoutBox>}
pub fn layout_root(id:usize,style:Style,viewport:(f32,f32))->LayoutBox{let width=match style.width{super::style::Length::Px(x)=>x,_=>viewport.0};LayoutBox{node_id:id,rect:Rect{x:0.,y:0.,width,height:viewport.1},style,children:Vec::new()}}
pub fn append_block(parent:&mut LayoutBox,mut child:LayoutBox){if child.style.display==Display::None{return} child.rect.x=parent.style.padding.left+child.style.margin.left;child.rect.y=parent.children.iter().map(|x|x.rect.height).sum::<f32>()+child.style.margin.top;parent.children.push(child);}
