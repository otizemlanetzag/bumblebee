use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct LexError { pub offset: usize, pub message: String }
impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{} at byte {}", self.message, self.offset) }
}
impl std::error::Error for LexError {}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Number(f64), String(String), Identifier(String),
    Let, Const, True, False, Null, Undefined,
    Plus, Minus, Star, Slash, Percent, Power,
    Bang, Equal, EqualEqual, StrictEqual, BangEqual, StrictBangEqual,
    Less, LessEqual, Greater, GreaterEqual, AndAnd, OrOr,
    LeftParen, RightParen, Semicolon, Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token { pub kind: TokenKind, pub offset: usize }

pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() { i += 1; continue; }
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i += 2; while i < bytes.len() && bytes[i] != b'\n' { i += 1; } continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i; i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
            if i + 1 >= bytes.len() { return Err(LexError { offset: start, message: "unterminated block comment".into() }); }
            i += 2; continue;
        }
        let start = i;
        let kind = if b.is_ascii_digit() || (b == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') { i += 1; }
            let raw = &source[start..i];
            let value = raw.parse::<f64>().map_err(|_| LexError { offset: start, message: format!("invalid number '{raw}'") })?;
            TokenKind::Number(value)
        } else if b == b'\'' || b == b'"' || b == b'`' {
            let quote = b; i += 1; let mut value = String::new();
            while i < bytes.len() && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    i += 1;
                    if i >= bytes.len() { break; }
                    match bytes[i] {
                        b'n' => { value.push('\n'); i += 1; },
                        b'r' => { value.push('\r'); i += 1; },
                        b't' => { value.push('\t'); i += 1; },
                        other if other.is_ascii() => { value.push(other as char); i += 1; },
                        _ => {
                            let ch = source[i..].chars().next().expect("valid UTF-8 boundary");
                            value.push(ch); i += ch.len_utf8();
                        }
                    }
                } else {
                    let ch = source[i..].chars().next().expect("valid UTF-8 boundary");
                    value.push(ch); i += ch.len_utf8();
                }
            }
            if i >= bytes.len() { return Err(LexError { offset: start, message: "unterminated string".into() }); }
            i += 1; TokenKind::String(value)
        } else if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$') { i += 1; }
            match &source[start..i] {
                "let" => TokenKind::Let, "const" => TokenKind::Const,
                "true" => TokenKind::True, "false" => TokenKind::False,
                "null" => TokenKind::Null, "undefined" => TokenKind::Undefined,
                name => TokenKind::Identifier(name.into()),
            }
        } else {
            let rest = &source[i..];
            let (kind, width) = if rest.starts_with("!==") { (TokenKind::StrictBangEqual, 3) }
                else if rest.starts_with("===") { (TokenKind::StrictEqual, 3) }
                else if rest.starts_with("**") { (TokenKind::Power, 2) }
                else if rest.starts_with("==") { (TokenKind::EqualEqual, 2) }
                else if rest.starts_with("!=") { (TokenKind::BangEqual, 2) }
                else if rest.starts_with("<=") { (TokenKind::LessEqual, 2) }
                else if rest.starts_with(">=") { (TokenKind::GreaterEqual, 2) }
                else if rest.starts_with("&&") { (TokenKind::AndAnd, 2) }
                else if rest.starts_with("||") { (TokenKind::OrOr, 2) }
                else { (match b {
                    b'+' => TokenKind::Plus, b'-' => TokenKind::Minus, b'*' => TokenKind::Star,
                    b'/' => TokenKind::Slash, b'%' => TokenKind::Percent, b'!' => TokenKind::Bang,
                    b'=' => TokenKind::Equal, b'<' => TokenKind::Less, b'>' => TokenKind::Greater,
                    b'(' => TokenKind::LeftParen, b')' => TokenKind::RightParen, b';' => TokenKind::Semicolon,
                    _ => return Err(LexError { offset: i, message: format!("unexpected character '{}'", b as char) }),
                }, 1) };
            i += width; kind
        };
        tokens.push(Token { kind, offset: start });
    }
    tokens.push(Token { kind: TokenKind::Eof, offset: source.len() });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn recognizes_keywords_and_multichar_operators() {
        let t = tokenize("let x = 2 === 2").unwrap();
        assert_eq!(t[0].kind, TokenKind::Let);
        assert_eq!(t[2].kind, TokenKind::Equal);
        assert_eq!(t[4].kind, TokenKind::StrictEqual);
    }
    #[test] fn rejects_unterminated_strings() { assert!(tokenize("'oops").is_err()); }
}