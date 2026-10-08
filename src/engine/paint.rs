use super::layout::{LayoutBox,Rect};
use super::style::Color;
#[derive(Clone,Debug)] pub enum PaintCommand{Rect{rect:Rect,color:Color},Text{x:f32,y:f32,text:String,color:Color}}
pub fn paint(root:&LayoutBox)->Vec<PaintCommand>{let mut out=Vec::new();paint_box(root,&mut out);out}
fn paint_box(b:&LayoutBox,out:&mut Vec<PaintCommand>){out.push(PaintCommand::Rect{rect:b.rect,color:b.style.background});for c in &b.children{paint_box(c,out);}}
