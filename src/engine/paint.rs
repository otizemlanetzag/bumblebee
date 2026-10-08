use super::layout::{LayoutBox,Rect};
use super::css::CssColor;

#[derive(Clone,Debug)]
pub enum PaintCommand{Rect{rect:Rect,color:CssColor},Text{x:f32,y:f32,text:String,color:CssColor}}

pub fn paint(root:&LayoutBox)->Vec<PaintCommand>{let mut out=Vec::new();paint_box(root,&mut out);out}

fn paint_box(b:&LayoutBox,out:&mut Vec<PaintCommand>){
 if b.style.background_color.a>0{out.push(PaintCommand::Rect{rect:b.rect,color:b.style.background_color});}
 for c in &b.children{paint_box(c,out);}
}
