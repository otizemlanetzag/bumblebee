use super::values::{parse_color,parse_length,CssColor,CssLength};

#[derive(Clone,Debug)] pub struct Stylesheet{pub rules:Vec<CssRule>}
#[derive(Clone,Debug)] pub struct CssRule{pub selector:String,pub declarations:Vec<Declaration>}
#[derive(Clone,Debug)] pub struct Declaration{pub property:String,pub value:CssValue,pub important:bool}
#[derive(Clone,Debug)] pub enum CssValue{Keyword(String),Length(CssLength),Color(CssColor),Raw(String)}

pub fn parse_stylesheet(input:&str)->Stylesheet{
    let mut rules=Vec::new();
    for block in input.split('}') {
        let Some((selector,body))=block.split_once('{') else{continue};
        let selector=selector.trim(); if selector.is_empty(){continue}
        let mut declarations=Vec::new();
        for item in body.split(';') {
            let Some((property,value))=item.split_once(':') else{continue};
            let property=property.trim().to_ascii_lowercase();
            let mut value=value.trim().to_string();
            let important=value.ends_with("!important");
            if important {value=value.trim_end_matches("!important").trim().into();}
            let parsed=if let Some(v)=parse_length(&value){CssValue::Length(v)}
                else if let Some(v)=parse_color(&value){CssValue::Color(v)}
                else {CssValue::Keyword(value)};
            declarations.push(Declaration{property,value:parsed,important});
        }
        rules.push(CssRule{selector:selector.into(),declarations});
    }
    Stylesheet{rules}
}
