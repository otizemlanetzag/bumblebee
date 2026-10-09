use super::{css::{compute_styles_with_context,Stylesheet},document::Node,layout::{layout_node,LayoutBox},paint::{paint,PaintCommand}};

pub struct RenderPipeline;

impl RenderPipeline {
    pub fn build_document(&self, root: &Node, sheet: &Stylesheet, viewport: (f32, f32)) -> (LayoutBox, Vec<PaintCommand>) {
        let tree = self.layout_tree(root, sheet, None, 0, (0.0, 0.0), viewport, &[], &[], &[]);
        let commands = paint(&tree);
        (tree, commands)
    }

    fn layout_tree(
        &self, node: &Node, sheet: &Stylesheet, parent: Option<&super::css::ComputedStyle>,
        id: usize, origin: (f32, f32), viewport: (f32, f32),
        ancestors: &[&Node], previous_siblings: &[&Node], following_siblings: &[&Node],
    ) -> LayoutBox {
        let style = compute_styles_with_context(node, sheet, parent, viewport, ancestors, previous_siblings, following_siblings);
        if style.display == "none" {
            return layout_node(id, style, Vec::new(), origin, (0.0, 0.0), viewport);
        }

        let mut children = Vec::new();
        let mut cursor_y = 0.0;
        let element_children: Vec<&Node> = node.children.iter().filter(|n| n.tag.is_some()).collect();
        let mut child_ancestors = ancestors.to_vec();
        child_ancestors.push(node);

        for (i, child) in node.children.iter().enumerate() {
            if matches!(child.tag.as_deref(), Some("style" | "script" | "head" | "title" | "meta" | "link")) {
                continue;
            }
            let element_index = element_children.iter().position(|n| std::ptr::eq(*n, child));
            let (prev, next) = if let Some(ei) = element_index {
                (&element_children[..ei], &element_children[ei + 1..])
            } else { (&[][..], &[][..]) };
            let child_box = self.layout_tree(
                child, sheet, Some(&style), id.saturating_mul(31).saturating_add(i + 1),
                (origin.0, origin.1 + cursor_y), viewport, &child_ancestors, prev, next,
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

    #[test]
    fn child_selector_is_applied_in_dom_context() {
        let mut root = Node::default(); root.tag = Some("html".into());
        let mut div = Node::default(); div.tag = Some("div".into());
        let mut p = Node::default(); p.tag = Some("p".into());
        div.children.push(p); root.children.push(div);
        let (tree, _) = RenderPipeline.build_document(&root, &parse_stylesheet("div > p { width: 123px; }"), (800.0,600.0));
        assert_eq!(tree.children[0].children[0].rect.width, 123.0);
    }
}
