use crate::engine::document::Node;
#[derive(Clone,Debug,Default)] pub struct Selector{pub parts:Vec<SimpleSelector>}
#[derive(Clone,Debug)] pub struct SimpleSelector{pub tag:Option<String>,pub id:Option<String>,pub classes:Vec<String>}
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq,Ord,PartialOrd)] pub struct Specificity(pub u32,pub u32,pub u32);

pub fn parse_selector(input:&str)->Selector{
    let mut parts=Vec::new();
    for raw in input.split_whitespace() {
        let mut s=SimpleSelector{tag:None,id:None,classes:Vec::new()};
        let mut buf=String::new();
        let mut mode='t';
        for c in raw.chars() {
            if c=='#'||c=='.' {
                if !buf.is_empty(){if mode=='t'{s.tag=Some(buf.clone())}else if mode=='#'{s.id=Some(buf.clone())}else{s.classes.push(buf.clone())}buf.clear();}
                mode=c;
            } else {buf.push(c)}
        }
        if !buf.is_empty(){if mode=='t'{s.tag=Some(buf)}else if mode=='#'{s.id=Some(buf)}else{s.classes.push(buf)}}
        parts.push(s);
    }
    Selector{parts}
}
pub fn specificity(s:&Selector)->Specificity{
    let mut a=0;let mut b=0;let mut c=0;
    for p in &s.parts {if p.id.is_some(){a+=1} b+=p.classes.len() as u32;if p.tag.is_some(){c+=1}}
    Specificity(a,b,c)
}
pub fn matches(selector:&Selector,node:&Node)->bool{
    let Some(last)=selector.parts.last() else{return false};
    simple_matches(last,node)
}
fn simple_matches(s:&SimpleSelector,node:&Node)->bool{
    if let Some(tag)=&s.tag {if node.tag.as_deref().map(|x|x.to_ascii_lowercase())!=Some(tag.to_ascii_lowercase()){return false}}
    if let Some(id)=&s.id {if node.id.as_deref()!=Some(id){return false}}
    for class in &s.classes {if !node.classes.iter().any(|x|x==class){return false}}
    true
}
