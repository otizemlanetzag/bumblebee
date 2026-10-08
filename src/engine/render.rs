use super::{css::ComputedStyle,layout::{layout_node,LayoutBox},paint::{PaintCommand,paint}};
pub struct RenderPipeline;
impl RenderPipeline{
 pub fn build(&self,id:usize,style:Option<ComputedStyle>,viewport:(f32,f32))->(LayoutBox,Vec<PaintCommand>){
  let tree=layout_node(id,style.unwrap_or_default(),Vec::new(),(0.,0.),viewport,viewport);(tree,paint(&tree))
 }
}
