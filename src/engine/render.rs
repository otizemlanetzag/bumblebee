use super::{css::{compute_styles,Stylesheet},document::Node,layout::{layout_node,LayoutBox},paint::{paint,PaintCommand}};

pub struct RenderPipeline;

impl RenderPipeline{
 pub fn build_document(&self,root:&Node,sheet:&Stylesheet,viewport:(f32,f32))->(LayoutBox,Vec<PaintCommand>){
  let tree=self.layout_tree(root,sheet,None,0,(0.,0.),viewport);
  let commands=paint(&tree);(tree,commands)
 }
 fn layout_tree(&self,node:&Node,sheet:&Stylesheet,parent:Option<&super::css::ComputedStyle>,id:usize,origin:(f32,f32),viewport:(f32,f32))->LayoutBox{
  let style=compute_styles(node,sheet,parent);
  let mut children=Vec::new(); let mut cursor_y=0.;
  for (i,child) in node.children.iter().enumerate(){
   if child.tag.as_deref()==Some("style") || child.tag.as_deref()==Some("script"){continue}
   let child_box=self.layout_tree(child,sheet,Some(&style),id+i+1,(origin.0,origin.1+cursor_y),viewport);
   cursor_y+=child_box.rect.height; children.push(child_box);
  }
  layout_node(id,style,children,origin,viewport,viewport)
 }
}