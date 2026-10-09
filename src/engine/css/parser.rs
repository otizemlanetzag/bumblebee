use super::values::{parse_color, parse_length, CssColor, CssLength};

#[derive(Clone, Debug, Default)]
pub struct Stylesheet { pub rules: Vec<CssRule>, pub media: Vec<MediaRule> }
#[derive(Clone, Debug)]
pub struct CssRule { pub selector: String, pub declarations: Vec<Declaration> }
#[derive(Clone, Debug)]
pub struct MediaRule { pub condition: String, pub rules: Vec<CssRule> }
#[derive(Clone, Debug)]
pub struct Declaration { pub property: String, pub value: CssValue, pub important: bool }
#[derive(Clone, Debug)]
pub enum CssValue { Keyword(String), Length(CssLength), Color(CssColor), Raw(String) }

pub fn parse_stylesheet(input: &str) -> Stylesheet {
    let clean = strip_comments(input);
    let mut sheet = Stylesheet::default();
    parse_block(&clean, &mut sheet.rules, &mut sheet.media);
    sheet
}

fn strip_comments(input: &str) -> String {
    let b = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut quote = 0u8;
    while i < b.len() {
        if quote != 0 {
            out.push(b[i] as char);
            if b[i] == b'\\' && i + 1 < b.len() { i += 1; out.push(b[i] as char); }
            else if b[i] == quote { quote = 0; }
            i += 1;
        } else if b[i] == b'\'' || b[i] == b'"' {
            quote = b[i]; out.push(b[i] as char); i += 1;
        } else if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') { i += 1; }
            i = (i + 2).min(b.len());
            out.push(' ');
        } else { out.push(b[i] as char); i += 1; }
    }
    out
}

fn parse_block(input: &str, rules: &mut Vec<CssRule>, media: &mut Vec<MediaRule>) {
    let mut i = 0;
    while i < input.len() {
        while i < input.len() && input.as_bytes()[i].is_ascii_whitespace() { i += 1; }
        if i >= input.len() { break; }
        let Some(open) = find_top_level(input, i, b'{') else { break };
        let header = input[i..open].trim();
        let Some(close) = matching_brace(input, open) else { break };
        let body = &input[open + 1..close];
        if header.to_ascii_lowercase().starts_with("@media") {
            let condition = header[6..].trim().to_string();
            let mut nested_rules = Vec::new();
            let mut nested_media = Vec::new();
            parse_block(body, &mut nested_rules, &mut nested_media);
            // Preserve the media condition on every nested rule. Never leak media rules into the global cascade.
            if !nested_rules.is_empty() { media.push(MediaRule { condition: condition.clone(), rules: nested_rules }); }
            for nested in nested_media {
                media.push(MediaRule { condition: format!("{} and ({})", condition, nested.condition), rules: nested.rules });
            }
        } else if !header.starts_with('@') {
            let declarations = parse_declarations(body);
            for selector in split_top_level(header, ',') {
                let selector = selector.trim();
                if !selector.is_empty() && !declarations.is_empty() {
                    rules.push(CssRule { selector: selector.to_string(), declarations: declarations.clone() });
                }
            }
        }
        i = close + 1;
    }
}

pub fn parse_declarations(body: &str) -> Vec<Declaration> {
    split_top_level(body, ';').into_iter().filter_map(|item| {
        let colon = find_top_level(&item, 0, b':')?;
        let property = item[..colon].trim().to_ascii_lowercase();
        if property.is_empty() || property.starts_with("--") { return None; }
        let mut value = item[colon + 1..].trim().to_string();
        let important = value.to_ascii_lowercase().ends_with("!important");
        if important { value.truncate(value.len() - 10); value = value.trim().to_string(); }
        if value.is_empty() { return None; }
        let parsed = if let Some(v) = parse_length(&value) { CssValue::Length(v) }
            else if let Some(v) = parse_color(&value) { CssValue::Color(v) }
            else if value.starts_with("url(") || value.starts_with("var(") || value.starts_with("calc(") { CssValue::Raw(value) }
            else if value.chars().any(char::is_whitespace) || value.contains('(') { CssValue::Raw(value) }
            else { CssValue::Keyword(value) };
        Some(Declaration { property, value: parsed, important })
    }).collect()
}

fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new(); let mut start = 0; let mut depth = 0i32; let mut quote = None;
    let chars: Vec<char> = input.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if let Some(q) = quote { if *c == '\\' { continue; } if *c == q { quote = None; } continue; }
        match *c { '\'' | '"' => quote = Some(*c), '(' | '[' => depth += 1, ')' | ']' => depth -= 1,
            x if x == delimiter && depth == 0 => { parts.push(chars[start..i].iter().collect()); start = i + 1; }, _ => {} }
    }
    parts.push(chars[start..].iter().collect()); parts
}

fn find_top_level(input: &str, start: usize, target: u8) -> Option<usize> {
    let b = input.as_bytes(); let mut depth = 0i32; let mut quote = 0u8; let mut i = start;
    while i < b.len() {
        if quote != 0 { if b[i] == b'\\' { i += 2; continue; } if b[i] == quote { quote = 0; } }
        else if b[i] == b'\'' || b[i] == b'"' { quote = b[i]; }
        else if b[i] == b'(' || b[i] == b'[' { depth += 1; }
        else if b[i] == b')' || b[i] == b']' { depth -= 1; }
        else if b[i] == target && depth == 0 { return Some(i); }
        i += 1;
    }
    None
}
fn matching_brace(input: &str, open: usize) -> Option<usize> {
    let b = input.as_bytes(); let mut depth = 0i32; let mut quote = 0u8; let mut i = open;
    while i < b.len() {
        if quote != 0 { if b[i] == b'\\' { i += 2; continue; } if b[i] == quote { quote = 0; } }
        else if b[i] == b'\'' || b[i] == b'"' { quote = b[i]; }
        else if b[i] == b'{' { depth += 1; }
        else if b[i] == b'}' { depth -= 1; if depth == 0 { return Some(i); } }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn media_rules_do_not_leak() {
        let s = parse_stylesheet("p{color:red}@media (max-width:600px){p{color:blue}}");
        assert_eq!(s.rules.len(), 1);
        assert_eq!(s.media.len(), 1);
        assert_eq!(s.media[0].rules.len(), 1);
    }
    #[test] fn selector_lists_and_function_values_are_kept() {
        let s = parse_stylesheet("h1, h2 { color: rgb(1, 2, 3); width: calc(100% - 2px) }");
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.rules[0].declarations.len(), 2);
    }
}
