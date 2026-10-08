use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use super::document::Document;

pub fn parse(html: &str) -> Document {
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_default();

    let mut document = Document::default();
    walk(&dom.document, &mut document);
    document
}

fn walk(handle: &Handle, document: &mut Document) {
    match &handle.data {
        NodeData::Element { name, .. } => {
            document.element_count += 1;
            if name.local.as_ref() == "title" {
                let title = handle.children.borrow().iter()
                    .filter_map(|child| match &child.data {
                        NodeData::Text { contents } => Some(contents.borrow().to_string()),
                        _ => None,
                    })
                    .collect::<String>()
                    .trim()
                    .to_owned();
                if !title.is_empty() {
                    document.title = Some(title);
                }
            }
        }
        _ => {}
    }

    for child in handle.children.borrow().iter() {
        walk(child, document);
    }
}
