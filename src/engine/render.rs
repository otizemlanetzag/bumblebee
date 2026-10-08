use super::{layout::{layout_root,LayoutBox},paint::{paint,PaintCommand},style::{default_style,Style}};
pub struct RenderPipeline;
impl RenderPipeline{pub fn build(&self,id:usize,style:Option<Style>,viewport:(f32,f32))->(LayoutBox,Vec<PaintCommand>){let tree=layout_root(id,style.unwrap_or_else(default_style),viewport);let commands=paint(&tree);(tree,commands)}}
