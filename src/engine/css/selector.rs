use crate::engine::document::Node;

#[derive(Clone, Debug, Default)]
pub struct Selector {
    pub parts: Vec<SelectorPart>,
}

#[derive(Clone, Debug)]
pub struct SelectorPart {
    pub simple: SimpleSelector,
    pub combinator: Option<Combinator>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    AdjacentSibling,
    GeneralSibling,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttributeSelector>,
    pub pseudos: Vec<PseudoClass>,
}

#[derive(Clone, Debug)]
pub struct AttributeSelector {
    pub name: String,
    pub operator: Option<AttributeOperator>,
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeOperator {
    Exists,
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PseudoClass {
    FirstChild,
    LastChild,
    OnlyChild,
    Empty,
    Root,
    Not(Box<SimpleSelector>),
    Hover,
    Active,
    Focus,
    Checked,
    Disabled,
    Enabled,
    Link,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Specificity(pub u32, pub u32, pub u32);

pub fn parse_selector(input: &str) -> Selector {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut combinator = None;
    let mut chars = input.trim().chars().peekable();

    while let Some(c) = chars.next() {
        if c == '>' || c == '+' || c == '~' {
            if !current.trim().is_empty() {
                parts.push(SelectorPart {
                    simple: parse_simple(current.trim()),
                    combinator,
                });
                current.clear();
            }
            combinator = Some(match c {
                '>' => Combinator::Child,
                '+' => Combinator::AdjacentSibling,
                _ => Combinator::GeneralSibling,
            });
        } else if c.is_whitespace() {
            if !current.trim().is_empty() {
                parts.push(SelectorPart {
                    simple: parse_simple(current.trim()),
                    combinator,
                });
                current.clear();
                combinator = Some(Combinator::Descendant);
            }
        } else {
            current.push(c);
        }
    }

    if !current.trim().is_empty() {
        parts.push(SelectorPart {
            simple: parse_simple(current.trim()),
            combinator,
        });
    }

    Selector { parts }
}

fn parse_simple(input: &str) -> SimpleSelector {
    let mut out = SimpleSelector::default();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '#' | '.' => {
                let marker = chars[i];
                i += 1;
                let start = i;
                while i < chars.len() && is_ident(chars[i]) { i += 1; }
                let value: String = chars[start..i].iter().collect();
                if marker == '#' { out.id = Some(value); } else if !value.is_empty() { out.classes.push(value); }
            }
            '[' => {
                if let Some(end) = find_closing(&chars, i, '[', ']') {
                    let raw: String = chars[i + 1..end].iter().collect();
                    out.attributes.push(parse_attribute(&raw));
                    i = end + 1;
                } else { i += 1; }
            }
            ':' => {
                i += 1;
                let start = i;
                while i < chars.len() && is_ident(chars[i]) { i += 1; }
                let name: String = chars[start..i].iter().collect();
                let arg = if i < chars.len() && chars[i] == '(' {
                    if let Some(end) = find_closing(&chars, i, '(', ')') {
                        let s: String = chars[i + 1..end].iter().collect();
                        i = end + 1;
                        Some(s)
                    } else { None }
                } else { None };
                if let Some(p) = parse_pseudo(&name, arg.as_deref()) { out.pseudos.push(p); }
            }
            '*' => i += 1,
            c if is_ident(c) && out.tag.is_none() => {
                let start = i;
                while i < chars.len() && is_ident(chars[i]) { i += 1; }
                out.tag = Some(chars[start..i].iter().collect());
            }
            _ => i += 1,
        }
    }
    out
}

fn parse_attribute(raw: &str) -> AttributeSelector {
    for (op, operator) in [
        ("~=", AttributeOperator::Includes), ("|=", AttributeOperator::DashMatch),
        ("^=", AttributeOperator::Prefix), ("$=", AttributeOperator::Suffix),
        ("*=", AttributeOperator::Substring), ("=", AttributeOperator::Equals),
    ] {
        if let Some((name, value)) = raw.split_once(op) {
            return AttributeSelector {
                name: name.trim().to_ascii_lowercase(),
                operator: Some(operator),
                value: Some(value.trim().trim_matches('"').trim_matches('\'').to_string()),
            };
        }
    }
    AttributeSelector { name: raw.trim().to_ascii_lowercase(), operator: Some(AttributeOperator::Exists), value: None }
}

fn parse_pseudo(name: &str, arg: Option<&str>) -> Option<PseudoClass> {
    Some(match name.to_ascii_lowercase().as_str() {
        "first-child" => PseudoClass::FirstChild,
        "last-child" => PseudoClass::LastChild,
        "only-child" => PseudoClass::OnlyChild,
        "empty" => PseudoClass::Empty,
        "root" => PseudoClass::Root,
        "hover" => PseudoClass::Hover,
        "active" => PseudoClass::Active,
        "focus" => PseudoClass::Focus,
        "checked" => PseudoClass::Checked,
        "disabled" => PseudoClass::Disabled,
        "enabled" => PseudoClass::Enabled,
        "link" => PseudoClass::Link,
        "not" => PseudoClass::Not(Box::new(parse_simple(arg.unwrap_or("*")))),
        _ => return None,
    })
}

fn is_ident(c: char) -> bool { c.is_ascii_alphanumeric() || c == '-' || c == '_' }
fn find_closing(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for i in start..chars.len() {
        if chars[i] == open { depth += 1; }
        if chars[i] == close {
            depth -= 1;
            if depth == 0 { return Some(i); }
        }
    }
    None
}

pub fn specificity(s: &Selector) -> Specificity {
    let mut a = 0; let mut b = 0; let mut c = 0;
    for part in &s.parts {
        let p = &part.simple;
        if p.id.is_some() { a += 1; }
        b += p.classes.len() as u32 + p.attributes.len() as u32 + p.pseudos.len() as u32;
        if p.tag.is_some() { c += 1; }
    }
    Specificity(a, b, c)
}

pub fn matches(selector: &Selector, node: &Node) -> bool {
    selector.parts.last().map(|p| simple_matches(&p.simple, node)).unwrap_or(false)
}

fn simple_matches(s: &SimpleSelector, node: &Node) -> bool {
    if let Some(tag) = &s.tag {
        if node.tag.as_deref().map(|x| x.eq_ignore_ascii_case(tag)) != Some(true) { return false; }
    }
    if let Some(id) = &s.id {
        if node.id.as_deref() != Some(id) { return false; }
    }
    if s.classes.iter().any(|c| !node.classes.iter().any(|x| x == c)) { return false; }

    for attr in &s.attributes {
        let value = node.attributes.get(&attr.name);
        match attr.operator.unwrap_or(AttributeOperator::Exists) {
            AttributeOperator::Exists => if value.is_none() { return false; },
            AttributeOperator::Equals => if value != attr.value.as_ref() { return false; },
            AttributeOperator::Includes => if !value.unwrap_or(&String::new()).split_whitespace().any(|x| Some(x) == attr.value.as_deref()) { return false; },
            AttributeOperator::DashMatch => if !value.map(|v| v == attr.value.as_deref().unwrap_or("") || v.starts_with(&(attr.value.as_deref().unwrap_or("").to_string() + "-"))).unwrap_or(false) { return false; },
            AttributeOperator::Prefix => if !value.map(|v| v.starts_with(attr.value.as_deref().unwrap_or(""))).unwrap_or(false) { return false; },
            AttributeOperator::Suffix => if !value.map(|v| v.ends_with(attr.value.as_deref().unwrap_or(""))).unwrap_or(false) { return false; },
            AttributeOperator::Substring => if !value.map(|v| v.contains(attr.value.as_deref().unwrap_or(""))).unwrap_or(false) { return false; },
        }
    }
    for pseudo in &s.pseudos {
        let ok = match pseudo {
            PseudoClass::Root => node.tag.as_deref() == Some("html"),
            PseudoClass::Empty => node.children.is_empty() && node.text.as_deref().unwrap_or("").trim().is_empty(),
            PseudoClass::Not(inner) => !simple_matches(inner, node),
            PseudoClass::Checked => node.attributes.contains_key("checked"),
            PseudoClass::Disabled => node.attributes.contains_key("disabled"),
            PseudoClass::Enabled => !node.attributes.contains_key("disabled"),
            PseudoClass::Link => node.tag.as_deref() == Some("a") && node.attributes.contains_key("href"),
            _ => true,
        };
        if !ok { return false; }
    }
    true
}
