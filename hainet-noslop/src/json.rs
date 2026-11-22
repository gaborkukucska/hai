//! START OF FILE hainet-noslop/src/json.rs
//!
//! Order-preserving JSON that renders exactly like Gson's `JsonElement.toString()`.
//!
//! Why not `serde_json::Value`: the NoSlop envelope signature covers a SHA-256 of the payload *as
//! Gson renders it* (`EnvelopeAuth.payloadDigest`). To reproduce that digest the Hub must keep the
//! member order of the wire, keep every number exactly as written (Gson keeps a
//! `LazilyParsedNumber`'s text, so `1.50` stays `1.50`) and escape strings the way Gson's
//! `JsonWriter` does (it escapes U+2028/U+2029, serde_json does not). Turning on serde_json's
//! `preserve_order`/`arbitrary_precision` features would change every crate in the workspace, so
//! this module has its own small value type.
//!
//! Parsing is strict RFC 8259 JSON (Gson's writer only produces that), with limits on depth and size.

use std::fmt::Write as _;

/// Maximum nesting depth accepted by [`parse`].
pub const MAX_DEPTH: usize = 64;

/// A JSON value with object members kept in wire order and numbers kept as their source text.
#[derive(Debug, Clone, PartialEq)]
pub enum JValue {
    Null,
    Bool(bool),
    /// The number exactly as written (valid JSON number syntax).
    Num(String),
    Str(String),
    Arr(Vec<JValue>),
    Obj(Vec<(String, JValue)>),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum JsonError {
    #[error("unexpected end of input")]
    Eof,
    #[error("unexpected character at byte {0}")]
    Unexpected(usize),
    #[error("nesting deeper than {MAX_DEPTH}")]
    TooDeep,
    #[error("invalid escape at byte {0}")]
    BadEscape(usize),
    #[error("trailing characters at byte {0}")]
    Trailing(usize),
}

impl JValue {
    pub fn obj() -> Self { JValue::Obj(Vec::new()) }
    pub fn str<S: Into<String>>(s: S) -> Self { JValue::Str(s.into()) }
    pub fn int(n: i64) -> Self { JValue::Num(n.to_string()) }

    /// Appends (or replaces) a member, keeping its position when it already exists.
    pub fn set<S: Into<String>>(&mut self, key: S, value: JValue) -> &mut Self {
        if let JValue::Obj(members) = self {
            let key = key.into();
            if let Some(slot) = members.iter_mut().find(|(k, _)| *k == key) {
                slot.1 = value;
            } else {
                members.push((key, value));
            }
        }
        self
    }

    /// Builder form of [`set`](Self::set).
    pub fn with<S: Into<String>>(mut self, key: S, value: JValue) -> Self { self.set(key, value); self }

    /// Like [`with`](Self::with) but leaves the member out when `value` is `None` (as Gson does
    /// for a null field).
    pub fn with_opt<S: Into<String>>(self, key: S, value: Option<JValue>) -> Self {
        match value { Some(v) => self.with(key, v), None => self }
    }

    pub fn remove(&mut self, key: &str) -> Option<JValue> {
        if let JValue::Obj(members) = self {
            if let Some(i) = members.iter().position(|(k, _)| k == key) {
                return Some(members.remove(i).1);
            }
        }
        None
    }

    pub fn get(&self, key: &str) -> Option<&JValue> {
        match self { JValue::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v), _ => None }
    }

    /// A string member, treating JSON null as absent.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) { Some(JValue::Str(s)) => Some(s.as_str()), _ => None }
    }

    /// An integral number member (`i64`), as Gson would coerce a `Long` field. Accepts integral
    /// text only; a fractional or exponent form yields `None`.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        match self.get(key) { Some(v) => v.as_i64(), None => None }
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) { Some(JValue::Bool(b)) => Some(*b), _ => None }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self { JValue::Num(t) => t.parse::<i64>().ok(), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> { match self { JValue::Str(s) => Some(s), _ => None } }
    pub fn as_array(&self) -> Option<&Vec<JValue>> { match self { JValue::Arr(a) => Some(a), _ => None } }
    pub fn is_null(&self) -> bool { matches!(self, JValue::Null) }

    /// This value with every object member whose value is null removed, recursively. Nulls inside
    /// arrays stay (`EnvelopeAuth.withoutNullMembers`).
    pub fn without_null_members(&self) -> JValue {
        match self {
            JValue::Obj(m) => JValue::Obj(
                m.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), v.without_null_members())).collect(),
            ),
            JValue::Arr(a) => JValue::Arr(a.iter().map(|v| v.without_null_members()).collect()),
            other => other.clone(),
        }
    }

    /// Gson's compact rendering (`JsonElement.toString()`).
    pub fn render(&self) -> String {
        let mut out = String::new();
        render_into(self, &mut out);
        out
    }

    /// Converts to `serde_json::Value` (for storage/API replies; numbers become `serde_json`
    /// numbers, so this is never used for anything that is signed).
    pub fn to_serde(&self) -> serde_json::Value {
        match self {
            JValue::Null => serde_json::Value::Null,
            JValue::Bool(b) => serde_json::Value::Bool(*b),
            JValue::Num(t) => serde_json::from_str(t).unwrap_or(serde_json::Value::Null),
            JValue::Str(s) => serde_json::Value::String(s.clone()),
            JValue::Arr(a) => serde_json::Value::Array(a.iter().map(|v| v.to_serde()).collect()),
            JValue::Obj(m) => {
                let mut map = serde_json::Map::new();
                for (k, v) in m { map.insert(k.clone(), v.to_serde()); }
                serde_json::Value::Object(map)
            }
        }
    }
}

fn render_into(v: &JValue, out: &mut String) {
    match v {
        JValue::Null => out.push_str("null"),
        JValue::Bool(true) => out.push_str("true"),
        JValue::Bool(false) => out.push_str("false"),
        JValue::Num(t) => out.push_str(t),
        JValue::Str(s) => render_string(s, out),
        JValue::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 { out.push(','); }
                render_into(x, out);
            }
            out.push(']');
        }
        JValue::Obj(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 { out.push(','); }
                render_string(k, out);
                out.push(':');
                render_into(x, out);
            }
            out.push('}');
        }
    }
}

/// Gson `JsonWriter` string escaping with `htmlSafe == false` (the default for
/// `JsonElement.toString()`): `"` `\` and the short forms `\t \b \n \r \f`, every other control
/// character below U+0020 as `\u00XX` (lower-case hex), and U+2028 / U+2029 escaped. Nothing else
/// is escaped (not `/`, not DEL, not non-ASCII).
fn render_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => { let _ = write!(out, "\\u{:04x}", c as u32); }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses one JSON document (strict RFC 8259; surrounding whitespace allowed).
pub fn parse(text: &str) -> Result<JValue, JsonError> {
    let mut p = Parser { b: text.as_bytes(), i: 0, s: text };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() { return Err(JsonError::Trailing(p.i)); }
    Ok(v)
}

struct Parser<'a> { b: &'a [u8], i: usize, s: &'a str }

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') { self.i += 1; }
    }
    fn peek(&self) -> Option<u8> { self.b.get(self.i).copied() }
    fn expect(&mut self, c: u8) -> Result<(), JsonError> {
        match self.peek() { Some(x) if x == c => { self.i += 1; Ok(()) } Some(_) => Err(JsonError::Unexpected(self.i)), None => Err(JsonError::Eof) }
    }
    fn lit(&mut self, word: &str, v: JValue) -> Result<JValue, JsonError> {
        if self.b[self.i..].starts_with(word.as_bytes()) { self.i += word.len(); Ok(v) } else { Err(JsonError::Unexpected(self.i)) }
    }
    fn value(&mut self, depth: usize) -> Result<JValue, JsonError> {
        if depth > MAX_DEPTH { return Err(JsonError::TooDeep); }
        match self.peek().ok_or(JsonError::Eof)? {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(JValue::Str(self.string()?)),
            b't' => self.lit("true", JValue::Bool(true)),
            b'f' => self.lit("false", JValue::Bool(false)),
            b'n' => self.lit("null", JValue::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(JsonError::Unexpected(self.i)),
        }
    }
    fn object(&mut self, depth: usize) -> Result<JValue, JsonError> {
        self.expect(b'{')?;
        let mut members = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') { self.i += 1; return Ok(JValue::Obj(members)); }
        loop {
            self.ws();
            if self.peek() != Some(b'"') { return Err(JsonError::Unexpected(self.i)); }
            let k = self.string()?;
            self.ws();
            self.expect(b':')?;
            self.ws();
            let v = self.value(depth + 1)?;
            // Gson's JsonObject keeps the last value for a repeated key, at the first key's place.
            if let Some(slot) = members.iter_mut().find(|(mk, _): &&mut (String, JValue)| *mk == k) { slot.1 = v; } else { members.push((k, v)); }
            self.ws();
            match self.peek().ok_or(JsonError::Eof)? {
                b',' => { self.i += 1; }
                b'}' => { self.i += 1; return Ok(JValue::Obj(members)); }
                _ => return Err(JsonError::Unexpected(self.i)),
            }
        }
    }
    fn array(&mut self, depth: usize) -> Result<JValue, JsonError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') { self.i += 1; return Ok(JValue::Arr(items)); }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek().ok_or(JsonError::Eof)? {
                b',' => { self.i += 1; }
                b']' => { self.i += 1; return Ok(JValue::Arr(items)); }
                _ => return Err(JsonError::Unexpected(self.i)),
            }
        }
    }
    fn number(&mut self) -> Result<JValue, JsonError> {
        let start = self.i;
        if self.peek() == Some(b'-') { self.i += 1; }
        match self.peek() {
            Some(b'0') => { self.i += 1; }
            Some(b'1'..=b'9') => { while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; } }
            _ => return Err(JsonError::Unexpected(self.i)),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) { return Err(JsonError::Unexpected(self.i)); }
            while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) { self.i += 1; }
            if !matches!(self.peek(), Some(b'0'..=b'9')) { return Err(JsonError::Unexpected(self.i)); }
            while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; }
        }
        Ok(JValue::Num(self.s[start..self.i].to_string()))
    }
    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.i + 4 > self.b.len() { return Err(JsonError::Eof); }
        let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|_| JsonError::BadEscape(self.i))?;
        let v = u32::from_str_radix(h, 16).map_err(|_| JsonError::BadEscape(self.i))?;
        self.i += 4;
        Ok(v)
    }
    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\' && self.b[self.i] >= 0x20 { self.i += 1; }
            out.push_str(&self.s[start..self.i]);
            match self.peek().ok_or(JsonError::Eof)? {
                b'"' => { self.i += 1; return Ok(out); }
                b'\\' => {
                    let at = self.i;
                    self.i += 1;
                    let e = self.peek().ok_or(JsonError::Eof)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'), b'\\' => out.push('\\'), b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'), b'f' => out.push('\u{c}'), b'n' => out.push('\n'),
                        b'r' => out.push('\r'), b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                // A surrogate pair. Gson's writer never escapes surrogates, so a lone
                                // escaped surrogate can't come from a NoSlop node; it is refused.
                                if self.b.get(self.i) != Some(&b'\\') || self.b.get(self.i + 1) != Some(&b'u') { return Err(JsonError::BadEscape(at)); }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) { return Err(JsonError::BadEscape(at)); }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(JsonError::BadEscape(at));
                            } else { hi };
                            out.push(char::from_u32(cp).ok_or(JsonError::BadEscape(at))?);
                        }
                        _ => return Err(JsonError::BadEscape(at)),
                    }
                }
                _ => return Err(JsonError::Unexpected(self.i)), // raw control character
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_order_and_numbers() {
        let v = parse(r#"{"b":1.50,"a":[1e3,-0,true,null],"c":{"x":"y"}}"#).unwrap();
        assert_eq!(v.render(), r#"{"b":1.50,"a":[1e3,-0,true,null],"c":{"x":"y"}}"#);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("{\"a\":1,}").is_err());
        assert!(parse("[01]").is_err());
        assert!(parse("\"\\ud800\"").is_err());
        assert!(parse("\"a\u{1}b\"").is_err());
        assert!(parse("{} x").is_err());
        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert_eq!(parse(&deep), Err(JsonError::TooDeep));
    }

    #[test]
    fn surrogate_pair_escape_decodes() {
        assert_eq!(parse("\"\\ud83d\\ude00\"").unwrap(), JValue::Str("😀".into()));
    }

    #[test]
    fn set_keeps_position() {
        let mut v = JValue::obj().with("a", JValue::int(1)).with("b", JValue::int(2));
        v.set("a", JValue::int(3));
        assert_eq!(v.render(), r#"{"a":3,"b":2}"#);
    }
}
