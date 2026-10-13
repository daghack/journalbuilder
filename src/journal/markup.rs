//! Markdown-like markup for text that's styled as it's set: emphasis,
//! strong text, strikethrough, and spans with settings of their own.
//!
//! ```text
//! Plain writing, *emphasis* (or _emphasis_), **strong**, ~~struck out~~,
//! [written in blue]{color: blue}, and
//! [scribbled out]{color: black, strike: {count: 3, color: hsla(0, 80%, 40%, 0.9)}}.
//! ```
//!
//! - `*text*` or `_text_` is emphasis, `**text**` or `__text__` strong, and
//!   `~~text~~` struck through. An opening mark must come before something
//!   other than a blank, and a closing one after it; underscores inside a
//!   word (`snake_case`) are just underscores. A mark that isn't closed is
//!   written as it is.
//! - `[text]{settings}` is a span of text with settings of its own (as
//!   Pandoc's bracketed spans): `key: value` pairs (or `key = value`),
//!   separated by commas. A key on its own is `true`. A value is a number,
//!   `true` or `false`, a map of settings in braces, or text: quoted, or up
//!   to the next comma or closing brace, with brackets kept together, so
//!   `hsla(0, 80%, 40%, 0.9)` is one value.
//! - A backslash before any of `` \ * _ ~ [ ] { } `` writes it as it is.
//!
//! Spans nest, each inside the one around it. What their settings mean is
//! up to whoever sets the text (see `rhaientries/markdown.rs`); this only
//! reads them.

use std::ops::Range;

/// Text with its markup taken out, and the spans the markup made.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Markup {
    /// The text, without the markup.
    pub text: String,
    /// Every span, each after the span around it.
    pub spans: Vec<Span>,
}

/// A stretch of the text that the markup styles.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    /// The characters it covers, by index into the text's characters.
    pub range: Range<usize>,
    /// The span around it, if any, by index.
    pub parent: Option<usize>,
    pub kind: SpanKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpanKind {
    /// `*text*` or `_text_`.
    Emphasis,
    /// `**text**` or `__text__`.
    Strong,
    /// `~~text~~`.
    Strike,
    /// `[text]{settings}`, the settings in the order given.
    Settings(Vec<(String, Value)>),
}

/// A setting's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Text(String),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// The value as it was written, near enough, for messages.
    pub fn describe(&self) -> String {
        match self {
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Text(t) => format!("\"{}\"", t),
            Value::Map(entries) => format!("{{{}}}", entries.iter().map(|(k, v)| format!("{}: {}", k, v.describe())).collect::<Vec<_>>().join(", ")),
        }
    }
}

impl Markup {
    /// Parse marked-up text. Fails only on a span's settings that can't be
    /// read (`[text]{color: }`); other markup that doesn't make sense is
    /// written as it is.
    pub fn parse(source: &str) -> Result<Markup, String> {
        let mut parser = Parser { chars: source.chars().collect(), pos: 0, text: Vec::new(), spans: Vec::new() };
        let end = parser.chars.len();
        parser.inline(None, None, end)?;
        Ok(Markup { text: parser.text.into_iter().collect(), spans: parser.spans })
    }

    /// For each character of the text, the innermost span it's in, if any.
    pub fn innermost(&self) -> Vec<Option<usize>> {
        let mut owner = vec![None; self.text.chars().count()];
        // Spans come after the spans around them, so inner spans overwrite.
        for (i, span) in self.spans.iter().enumerate() {
            for o in &mut owner[span.range.clone()] {
                *o = Some(i);
            }
        }
        owner
    }
}

/// A closing mark the parser is looking for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Closer {
    /// One `*` or `_`.
    Emphasis(char),
    /// Two of them.
    Strong(char),
    /// `~~`.
    Strike,
}

const ESCAPABLE: &str = "\\*_~[]{}";

struct Parser {
    chars: Vec<char>,
    pos: usize,
    /// The text so far, markup taken out.
    text: Vec<char>,
    spans: Vec<Span>,
}

impl Parser {
    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }
    /// How many of `c` there are in a row from `i`, up to `end`.
    fn run(&self, i: usize, c: char, end: usize) -> usize {
        self.chars[i..end].iter().take_while(|&&x| x == c).count()
    }
    fn blank_or_none(c: Option<char>) -> bool {
        c.is_none_or(char::is_whitespace)
    }
    /// Whether a mark of `len` `c`s at `i` can open: it comes before
    /// something other than a blank, and an underscore isn't inside a word.
    fn can_open(&self, i: usize, c: char, len: usize, end: usize) -> bool {
        let next = if i + len < end { self.at(i + len) } else { None };
        !Self::blank_or_none(next) && !(c == '_' && i > 0 && self.at(i - 1).is_some_and(char::is_alphanumeric))
    }
    /// Whether a mark of `len` `c`s at `i` can close what started at
    /// `start`: it comes after something other than a blank, there's
    /// something to close, and an underscore isn't inside a word.
    fn can_close(&self, i: usize, c: char, len: usize, start: usize) -> bool {
        i > start && !Self::blank_or_none(self.at(i - 1)) && !(c == '_' && self.at(i + len).is_some_and(char::is_alphanumeric))
    }

    /// Read text from `self.pos` up to `end`, or until `closer` closes it
    /// (and is consumed), adding its characters and spans. Returns whether
    /// the closer was found.
    fn inline(&mut self, closer: Option<Closer>, parent: Option<usize>, end: usize) -> Result<bool, String> {
        let start = self.pos;
        while self.pos < end {
            let i = self.pos;
            let c = self.chars[i];
            match c {
                '\\' if self.at(i + 1).is_some_and(|n| ESCAPABLE.contains(n)) && i + 1 < end => {
                    self.text.push(self.chars[i + 1]);
                    self.pos += 2;
                }
                '*' | '_' => {
                    let run = self.run(i, c, end);
                    if closer == Some(Closer::Emphasis(c)) && self.can_close(i, c, 1, start) {
                        self.pos += 1;
                        return Ok(true);
                    }
                    if closer == Some(Closer::Strong(c)) && run >= 2 && self.can_close(i, c, 2, start) {
                        self.pos += 2;
                        return Ok(true);
                    }
                    let opened = if run >= 2 && self.can_open(i, c, 2, end) {
                        self.span(Closer::Strong(c), SpanKind::Strong, 2, parent, end)?
                    } else if run == 1 && self.can_open(i, c, 1, end) {
                        self.span(Closer::Emphasis(c), SpanKind::Emphasis, 1, parent, end)?
                    } else {
                        false
                    };
                    if !opened {
                        // Written as it is, one at a time, so the rest of a
                        // run may still open something (`**a*`).
                        self.text.push(c);
                        self.pos += 1;
                    }
                }
                '~' if self.run(i, '~', end) >= 2 => {
                    if closer == Some(Closer::Strike) && self.can_close(i, '~', 2, start) {
                        self.pos += 2;
                        return Ok(true);
                    }
                    if !(self.can_open(i, '~', 2, end) && self.span(Closer::Strike, SpanKind::Strike, 2, parent, end)?) {
                        self.text.push('~');
                        self.pos += 1;
                    }
                }
                '[' => {
                    if !self.bracketed(parent, end)? {
                        self.text.push('[');
                        self.pos += 1;
                    }
                }
                _ => {
                    self.text.push(c);
                    self.pos += 1;
                }
            }
        }
        Ok(false)
    }

    /// Try a span opened by `len` characters at `self.pos`, closed by
    /// `closer`. If it isn't closed before `end`, undo it and return false.
    fn span(&mut self, closer: Closer, kind: SpanKind, len: usize, parent: Option<usize>, end: usize) -> Result<bool, String> {
        let (pos, text, spans) = (self.pos, self.text.len(), self.spans.len());
        let me = self.spans.len();
        self.spans.push(Span { range: text..text, parent, kind });
        self.pos += len;
        if self.inline(Some(closer), Some(me), end)? {
            self.spans[me].range.end = self.text.len();
            return Ok(true);
        }
        self.pos = pos;
        self.text.truncate(text);
        self.spans.truncate(spans);
        Ok(false)
    }

    /// Try a bracketed span, `[text]{settings}`, at `self.pos`: false if
    /// there isn't one (no closing bracket, or no settings after it).
    fn bracketed(&mut self, parent: Option<usize>, end: usize) -> Result<bool, String> {
        let open = self.pos;
        let Some(close) = self.matching(open, '[', ']', end) else { return Ok(false) };
        if self.at(close + 1) != Some('{') || close + 1 >= end {
            return Ok(false);
        }
        let Some(brace) = self.matching(close + 1, '{', '}', end) else { return Ok(false) };
        let source: String = self.chars[close + 2..brace].iter().collect();
        let written: String = self.chars[open..=brace].iter().collect();
        let settings = read_settings(&source).map_err(|e| format!("in {}: {}", written, e))?;
        let me = self.spans.len();
        let start = self.text.len();
        self.spans.push(Span { range: start..start, parent, kind: SpanKind::Settings(settings) });
        self.pos = open + 1;
        self.inline(None, Some(me), close)?;
        self.spans[me].range.end = self.text.len();
        self.pos = brace + 1;
        Ok(true)
    }

    /// Where the `close` matching the `open` at `from` is, before `end`:
    /// nested pairs are skipped, and so are escaped characters and, in
    /// braces, quoted text.
    fn matching(&self, from: usize, open: char, close: char, end: usize) -> Option<usize> {
        let mut depth = 0;
        let mut quote: Option<char> = None;
        let mut i = from;
        while i < end {
            let c = self.chars[i];
            match (quote, c) {
                (_, '\\') => i += 1,
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') if open == '{' => quote = Some(c),
                (None, c) if c == open => depth += 1,
                (None, c) if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        None
    }
}

/// Read settings, `key: value, key, ...`, as written between a span's
/// braces.
pub fn read_settings(source: &str) -> Result<Vec<(String, Value)>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut reader = Settings { chars, pos: 0 };
    let settings = reader.map()?;
    reader.blanks();
    match reader.peek() {
        None => Ok(settings),
        Some(c) => Err(format!("unexpected '{}'", c)),
    }
}

struct Settings {
    chars: Vec<char>,
    pos: usize,
}

impl Settings {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn blanks(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }
    /// Entries up to the end or a closing brace (not consumed).
    fn map(&mut self) -> Result<Vec<(String, Value)>, String> {
        let mut entries: Vec<(String, Value)> = Vec::new();
        loop {
            self.blanks();
            match self.peek() {
                None | Some('}') => return Ok(entries),
                _ => {}
            }
            let key = self.key()?;
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(format!("'{}' is set twice", key));
            }
            self.blanks();
            let value = match self.peek() {
                Some(':' | '=') => {
                    self.pos += 1;
                    self.value(&key)?
                }
                _ => Value::Bool(true),
            };
            entries.push((key, value));
            self.blanks();
            match self.peek() {
                Some(',') => self.pos += 1,
                None | Some('}') => {}
                Some(c) => return Err(format!("expected ',' between settings, got '{}'", c)),
            }
        }
    }
    fn key(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(match self.peek() {
                Some(c) => format!("expected a setting's name, got '{}'", c),
                None => "expected a setting's name".into(),
            });
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }
    fn value(&mut self, key: &str) -> Result<Value, String> {
        self.blanks();
        match self.peek() {
            Some('{') => {
                self.pos += 1;
                let map = self.map()?;
                if self.peek() != Some('}') {
                    return Err(format!("'{}': no closing brace", key));
                }
                self.pos += 1;
                Ok(Value::Map(map))
            }
            Some(q @ ('"' | '\'')) => {
                self.pos += 1;
                let mut text = String::new();
                loop {
                    match self.peek() {
                        None => return Err(format!("'{}': no closing quote", key)),
                        Some('\\') if self.chars.get(self.pos + 1).is_some() => {
                            text.push(self.chars[self.pos + 1]);
                            self.pos += 2;
                        }
                        Some(c) if c == q => {
                            self.pos += 1;
                            return Ok(Value::Text(text));
                        }
                        Some(c) => {
                            text.push(c);
                            self.pos += 1;
                        }
                    }
                }
            }
            _ => {
                // Up to a comma or closing brace outside brackets.
                let start = self.pos;
                let mut depth = 0;
                while let Some(c) = self.peek() {
                    match c {
                        '(' | '[' => depth += 1,
                        ')' | ']' => depth -= 1,
                        ',' | '}' if depth <= 0 => break,
                        _ => {}
                    }
                    self.pos += 1;
                }
                let text: String = self.chars[start..self.pos].iter().collect::<String>().trim().to_string();
                if text.is_empty() {
                    return Err(format!("'{}' has no value", key));
                }
                Ok(match text.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    _ => match text.parse::<f64>() {
                        Ok(n) if n.is_finite() => Value::Number(n),
                        _ => Value::Text(text),
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(source: &str) -> (String, Vec<(String, SpanKind, Option<usize>)>) {
        let m = Markup::parse(source).unwrap();
        let chars: Vec<char> = m.text.chars().collect();
        let spans = m.spans.iter().map(|s| (chars[s.range.clone()].iter().collect(), s.kind.clone(), s.parent)).collect();
        (m.text, spans)
    }

    fn settings(entries: &[(&str, Value)]) -> SpanKind {
        SpanKind::Settings(entries.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
    }

    #[test]
    fn emphasis_strong_and_strikethrough() {
        let (text, s) = spans("plain *em* and _em_, **strong** __strong__ ~~struck~~");
        assert_eq!(text, "plain em and em, strong strong struck");
        assert_eq!(s, vec![
            ("em".into(), SpanKind::Emphasis, None),
            ("em".into(), SpanKind::Emphasis, None),
            ("strong".into(), SpanKind::Strong, None),
            ("strong".into(), SpanKind::Strong, None),
            ("struck".into(), SpanKind::Strike, None),
        ]);
    }

    #[test]
    fn marks_nest() {
        let (text, s) = spans("**strong *and em* still** ***both***");
        assert_eq!(text, "strong and em still both");
        assert_eq!(s, vec![
            ("strong and em still".into(), SpanKind::Strong, None),
            ("and em".into(), SpanKind::Emphasis, Some(0)),
            ("both".into(), SpanKind::Strong, None),
            ("both".into(), SpanKind::Emphasis, Some(2)),
        ]);
        let (text, s) = spans("*em **strong** em*");
        assert_eq!(text, "em strong em");
        assert_eq!(s[1], ("strong".into(), SpanKind::Strong, Some(0)));
    }

    #[test]
    fn marks_that_dont_open_or_close_are_written_as_they_are() {
        for (source, text) in [
            ("2 * 3 * 4", "2 * 3 * 4"),
            ("snake_case_name", "snake_case_name"),
            ("*unclosed", "*unclosed"),
            ("a ** b", "a ** b"),
            ("~ one ~~ not ~~", "~ one ~~ not ~~"),
            ("[no settings] [x](y)", "[no settings] [x](y)"),
            ("**", "**"),
        ] {
            let (got, s) = spans(source);
            assert_eq!((got.as_str(), s.len()), (text, 0), "{}", source);
        }
        let (text, s) = spans("**a*");
        assert_eq!((text.as_str(), s), ("*a", vec![("a".into(), SpanKind::Emphasis, None)]));
    }

    #[test]
    fn backslashes_escape_marks() {
        let (text, s) = spans(r"\*not em\* \[x\]{y} a\\b \q");
        assert_eq!((text.as_str(), s.len()), (r"*not em* [x]{y} a\b \q", 0));
    }

    #[test]
    fn bracketed_spans_carry_settings() {
        let (text, s) = spans("This is [blue]{color: blue}, [*red*, scribbled]{color: black, strikethrough: {count: 3, color: hsla(0, 80%, 40%, 0.9)}}.");
        assert_eq!(text, "This is blue, red, scribbled.");
        assert_eq!(s, vec![
            ("blue".into(), settings(&[("color", Value::Text("blue".into()))]), None),
            ("red, scribbled".into(), settings(&[
                ("color", Value::Text("black".into())),
                ("strikethrough", Value::Map(vec![
                    ("count".into(), Value::Number(3.0)),
                    ("color".into(), Value::Text("hsla(0, 80%, 40%, 0.9)".into())),
                ])),
            ]), None),
            ("red".into(), SpanKind::Emphasis, Some(1)),
        ]);
        let (_, s) = spans("[x]{underline, italic = false, size: 1.5, note: \"a, b}\"}");
        assert_eq!(s[0].1, settings(&[
            ("underline", Value::Bool(true)),
            ("italic", Value::Bool(false)),
            ("size", Value::Number(1.5)),
            ("note", Value::Text("a, b}".into())),
        ]));
        // Spans in spans, and brackets in their text.
        let (text, s) = spans("[a [b]{size: 2} c]{color: red}");
        assert_eq!(text, "a b c");
        assert_eq!(s[1], ("b".into(), settings(&[("size", Value::Number(2.0))]), Some(0)));
    }

    #[test]
    fn unreadable_settings_fail() {
        for (source, want) in [
            ("[x]{color: }", "'color' has no value"),
            ("[x]{note: \"a\" size: 3}", "expected ',' between settings"),
            ("[x]{: red}", "expected a setting's name, got ':'"),
            ("[x]{a: 1, a: 2}", "'a' is set twice"),
            ("[x]{a: {b: 1}", "no closing brace"),
        ] {
            match Markup::parse(source) {
                Err(e) => assert!(e.contains(want) && e.contains(source.split('{').next().unwrap()), "{}: {}", source, e),
                Ok(m) => assert!(want == "no closing brace" && m.spans.is_empty(), "{} should fail", source),
            }
        }
    }

    #[test]
    fn innermost_spans_by_character() {
        let m = Markup::parse("a *b [c]{size: 2}* d").unwrap();
        assert_eq!(m.text, "a b c d");
        assert_eq!(m.innermost(), vec![None, None, Some(0), Some(0), Some(1), None, None]);
    }
}
