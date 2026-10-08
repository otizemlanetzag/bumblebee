use html5ever::{parse_document,tendril::TendrilSink};
use markup5ever_rcdom::{Handle,NodeData,RcDom};
use super::document::{Document,Node};

pub fn parse(html:&str)->Document{
    let dom=parse_document(RcDom::default(),Default::default()).from_utf8().read_from(&mut html.as_bytes()).unwrap_or_default();
    let mut d=Document::default();d.root=Some(convert(&dom.document,&mut d));d
}
fn convert(h:&Handle,d:&mut Document)->Node{
    let mut n=Node::default();
    match &h.data{
        NodeData::Element{name,attrs,..}=>{
            n.tag=Some(name.local.to_string());d.element_count+=1;
            for a in attrs.borrow().iter(){let k=a.name.local.to_string();let v=a.value.to_string();if k=="id"{n.id=Some(v.clone())}else if k=="class"{n.classes=v.split_whitespace().map(String::from).collect()}n.attributes.insert(k,v);}
            if n.tag.as_deref()==Some("title"){let t=h.children.borrow().iter().filter_map(|x|if let NodeData::Text{contents}=&x.data{Some(contents.borrow().to_string())}else{None}).collect::<String>();if !t.trim().is_empty(){d.title=Some(t.trim().into())}}
        },
        NodeData::Text{contents}=>n.text=Some(contents.borrow().to_string()),
        _=>{}
    }
    for c in h.children.borrow().iter(){n.children.push(convert(c,d));}
    n
}
