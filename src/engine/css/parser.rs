use super::values::{parse_color,parse_length,CssColor,CssLength};

#[derive(Clone,Debug,Default)] pub struct Stylesheet{pub rules:Vec<CssRule>,pub media:Vec<MediaRule>}
#[derive(Clone,Debug)] pub struct CssRule{pub selector:String,pub declarations:Vec<Declaration>}
#[derive(Clone,Debug)] pub struct MediaRule{pub condition:String,pub rules:Vec<CssRule>}
#[derive(Clone,Debug)] pub struct Declaration{pub property:String,pub value:CssValue,pub important:bool}
#[derive(Clone,Debug)] pub enum CssValue{Keyword(String),Length(CssLength),Color(CssColor),Raw(String)}

pub fn parse_stylesheet(input:&str)->Stylesheet{
    let mut sheet=Stylesheet::default();
    parse_block(input,&mut sheet.rules,&mut sheet.media);
    sheet
}

fn parse_block(input:&str,rules:&mut Vec<CssRule>,media:&mut Vec<MediaRule>){
    let mut i=0;
    while i<input.len(){
        while i<input.len() && input.as_bytes()[i].is_ascii_whitespace(){i+=1}
        if i>=input.len(){break}
        let Some(open)=find_char(input.as_bytes(),i,b'{') else{break};
        let header=input[i..open].trim();
        let Some(close)=matching_brace(input.as_bytes(),open) else{break};
        let body=&input[open+1..close];
        if header.to_ascii_lowercase().starts_with("@media"){
            let condition=header[6..].trim().to_string();
            let mut nested=Vec::new();
            let mut nested_media=Vec::new();
            parse_block(body,&mut nested,&mut nested_media);
            rules.extend(nested);
            media.push(MediaRule{condition,rules:nested_media.into_iter().flat_map(|m|m.rules).collect()});
        }else{
            let declarations=parse_declarations(body);
            if !declarations.is_empty(){rules.push(CssRule{selector:header.to_string(),declarations})}
        }
        i=close+1;
    }
}

fn parse_declarations(body:&str)->Vec<Declaration>{
    body.split(';').filter_map(|item|{
        let (property,value)=item.split_once(':')?;
        let property=property.trim().to_ascii_lowercase();
        if property.is_empty(){return None}
        let mut value=value.trim().to_string();
        let important=value.to_ascii_lowercase().ends_with("!important");
        if important {value=value[..value.len()-10].trim().to_string();}
        let parsed=if let Some(v)=parse_length(&value){CssValue::Length(v)}
            else if let Some(v)=parse_color(&value){CssValue::Color(v)}
            else {CssValue::Raw(value)};
        Some(Declaration{property,value:parsed,important})
    }).collect()
}

fn find_char(bytes:&[u8],start:usize,target:u8)->Option<usize>{(start..bytes.len()).find(|&i|bytes[i]==target)}
fn matching_brace(bytes:&[u8],open:usize)->Option<usize>{
    let mut depth=0;
    for i in open..bytes.len(){
        if bytes[i]==b'{' {depth+=1}
        else if bytes[i]==b'}' {depth-=1;if depth==0{return Some(i)}}
    }
    None
}
