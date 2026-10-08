#[derive(Debug,Default,Clone)]
pub struct Document{pub title:Option<String>,pub element_count:usize,pub root:Option<Node>}

#[derive(Debug,Default,Clone)]
pub struct Node{
    pub tag:Option<String>,pub id:Option<String>,pub classes:Vec<String>,
    pub attributes:std::collections::HashMap<String,String>,pub text:Option<String>,pub children:Vec<Node>
}
