use super::{css::{compute_styles_at,Stylesheet},document::Node,layout::{layout_node,LayoutBox},paint::{paint,PaintCommand}};

pub struct RenderPipeline;

impl RenderPipeline {
    pub fn build_document(&self, root: &Node, sheet: &Stylesheet, viewport: (f32, f32)) -> (LayoutBox, Vec<PaintCommand>) {
        let tree = self.layout_tree(root, sheet, None, 0, (0.0, 0.0), viewport);
        let commands = paint(&tree);
        (tree, commands)
    }

    fn layout_tree(
        &self,
        node: &Node,
        sheet: &Stylesheet,
        parent: Option<&super::css::ComputedStyle>,
        id: usize,
        origin: (f32, f32),
        viewport: (f32, f32),
    ) -> LayoutBox {
        let style = compute_styles_at(node, sheet, parent, viewport);
        // display:none removes the element and its entire subtree from layout and painting.
        if style.display == "none" {
            return layout_node(id, style, Vec::new(), origin, (0.0, 0.0), viewport);
        }

        let mut children = Vec::new();
        let mut cursor_y = 0.0;
        for (i, child) in node.children.iter().enumerate() {
            if matches!(child.tag.as_deref(), Some("style" | "script" | "head" | "title" | "meta" | "link")) {
                continue;
            }
            let child_box = self.layout_tree(
                child, sheet, Some(&style), id.saturating_mul(31).saturating_add(i + 1),
                (origin.0, origin.1 + cursor_y), viewport,
            );
            if child_box.style.display != "none" {
                cursor_y += child_box.rect.height;
                children.push(child_box);
            }
        }
        layout_node(id, style, children, origin, viewport, viewport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{css::parse_stylesheet, document::Node};

    #[test]
    fn pipeline_returns_root_and_paint_list() {
        let mut root = Node::default();
        root.tag = Some("html".into());
        let (layout, commands) = RenderPipeline.build_document(&root, &parse_stylesheet("html { background-color: #fff; }"), (800.0, 600.0));
        assert_eq!(layout.rect.width, 800.0);
        assert!(!commands.is_empty());
    }
}
