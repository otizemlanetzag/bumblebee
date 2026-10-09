use super::layout::{LayoutBox,Rect};
use super::css::CssColor;

#[derive(Clone,Debug)]
pub enum PaintCommand {
    Rect { rect:Rect, color:CssColor },
    Text { x:f32, y:f32, text:String, color:CssColor, font_size:f32, font_weight:String },
}

pub fn paint(root:&LayoutBox)->Vec<PaintCommand> {
    let mut out=Vec::new();
    paint_box(root,&mut out);
    out
}
fn paint_box(b:&LayoutBox,out:&mut Vec<PaintCommand>) {
    if b.style.background_color.a>0 {
        out.push(PaintCommand::Rect{rect:b.rect,color:b.style.background_color});
    }
    if let Some(text)=&b.text {
        if !text.is_empty() {
            out.push(PaintCommand::Text{x:b.rect.x,y:b.rect.y+b.style.font_size,text:text.clone(),color:b.style.color,font_size:b.style.font_size,font_weight:b.style.font_weight.clone()});
        }
    }
    for c in &b.children { paint_box(c,out); }
}
