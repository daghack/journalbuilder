//! Marked-up text for page layouts: `markdown(text)` makes a flow whose
//! text is styled as it's set, each stretch in its own colour, size, lean
//! and passes, underlined or struck through.
//!
//! ```rhai
//! let body = markdown("Plain writing, *emphasis*, **strong**, ~~struck out~~, "
//!     + "[written in blue]{color: blue}, [twice the size]{size: 2}, "
//!     + "[scribbled out]{color: black, strike: {count: 3, color: hsla(0, 80%, 40%, 0.9)}}.");
//! layout("two_column_article").typeset(#{ flows: #{ body: body, ... }, ... });
//! ```
//!
//! The markup (see `journal::markup`): `*text*` or `_text_` is emphasis,
//! `**text**` or `__text__` strong, `~~text~~` struck through, and
//! `[text]{settings}` text with settings of its own, `key: value` pairs
//! separated by commas (a key on its own is `true`). Spans nest, each
//! changing what it's in. Settings:
//!
//! | key | | |
//! |---|---|---|
//! | `color` (`colour`) | a CSS colour | the ink |
//! | `passes`, `jitter` | as a style's | strokes gone over: how many times, and how far astray (canvas units) |
//! | `italic` | true or false | leant as italics (0.25 radians), or upright |
//! | `skew` | radians | leant this far |
//! | `size` | a number | times the size of the text around it |
//! | `character_padding` | sizes | room between glyphs' outlines, in place of the font's |
//! | `word_spacing` | sizes | a space's width, in place of the font's `space_width` |
//! | `underline` | true, a count, or settings | lines under it |
//! | `strike` (`strikethrough`) | true, a count, or settings | lines through it |
//!
//! Lines under and through text take `true` (one line), a number of lines,
//! `false` (none, inside text that has them), or settings: `count`,
//! `spacing` (how far apart they are, in sizes: how far they must be to
//! show depends on the pen), and the `color`, `passes` and `jitter` they're
//! drawn in (by default, the text's).
//! Lines under text go one below another; lines through it go back and
//! forth through it as one stroke, as scribbling out. They run across the
//! spaces between words, but break where the text does: at the end of a
//! line, either side of what the text avoids, and wherever they'd come
//! near it.
//!
//! What emphasis and strong text look like is up to the role setting the
//! text: its `em` and `strong` settings are maps of the settings above, by
//! default `#{ italic: true }` and `#{ passes: 2, jitter: size / 20 }`
//! (gone over twice, a little astray).

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use std::rc::Rc;

use super::canvas::CanvasData;
use super::style::MAX_PASSES;
use super::{check_finite, RhaiResult, StrokeStyle};
use crate::journal::avoid::AvoidanceTree;
use crate::journal::canvas::{parse_color, StrokeColor};
use crate::journal::fonts::{Font, FontBook};
use crate::journal::markup::{Markup, SpanKind, Value};
use crate::journal::symbol::Symbol;
use crate::journal::typeset::{marked_runs, rule_path, Rule, Setting};

/// How far `italic: true` leans text, in radians.
const ITALIC_SKEW: f32 = 0.25;
/// The most lines a span can be underlined or struck through with.
const MAX_RULES: i64 = 20;

/// Text with markup, as `markdown(text)` makes it: the text without it, and
/// its spans, their settings read.
#[derive(Clone)]
pub struct Markdown {
    markup: Rc<Markup>,
    /// Each span's settings (emphasis and strong text take theirs from the
    /// role setting them).
    styles: Rc<Vec<Option<SpanStyle>>>,
}

impl Markdown {
    fn new(source: &str) -> RhaiResult<Markdown> {
        const CALLER: &str = "markdown";
        let markup = Markup::parse(source).map_err(|e| format!("{}: {}", CALLER, e))?;
        let chars: Vec<char> = markup.text.chars().collect();
        let styles = markup.spans.iter().map(|span| {
            let caller = || format!("{}: [{}]", CALLER, chars[span.range.clone()].iter().collect::<String>());
            match &span.kind {
                SpanKind::Settings(settings) => SpanStyle::read(&caller(), settings).map(Some),
                SpanKind::Strike => Ok(Some(SpanStyle { strike: Some(Some(RuleStyle::lines(1))), ..SpanStyle::default() })),
                SpanKind::Emphasis | SpanKind::Strong => Ok(None),
            }
        }).collect::<RhaiResult<Vec<_>>>()?;
        Ok(Markdown { markup: Rc::new(markup), styles: Rc::new(styles) })
    }
    /// The text, markup taken out.
    pub(super) fn text(&self) -> &str {
        &self.markup.text
    }
}

/// What strokes are drawn in, over a role's style: what a span changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Ink {
    color: Option<StrokeColor>,
    passes: Option<i64>,
    jitter: Option<f64>,
    point_jitter: Option<f64>,
}

impl Ink {
    /// `base` with this ink's changes.
    pub(super) fn on(&self, base: &StrokeStyle) -> StrokeStyle {
        let mut style = base.clone();
        if let Some(color) = &self.color {
            style.color = color.clone();
        }
        if let Some(passes) = self.passes {
            style.passes = passes;
        }
        if let Some(jitter) = self.jitter {
            style.jitter = jitter;
        }
        if let Some(jitter) = self.point_jitter {
            style.point_jitter = jitter;
        }
        style
    }
    /// This ink with `other`'s changes on top.
    fn then(&self, other: &Ink) -> Ink {
        Ink {
            color: other.color.clone().or_else(|| self.color.clone()),
            passes: other.passes.or(self.passes),
            jitter: other.jitter.or(self.jitter),
            point_jitter: other.point_jitter.or(self.point_jitter),
        }
    }
}

/// Lines under or through a span, as its settings give them.
#[derive(Clone, Debug, PartialEq)]
struct RuleStyle {
    count: u32,
    /// How far apart, in sizes.
    spacing: Option<f32>,
    ink: Ink,
}

impl RuleStyle {
    fn lines(count: u32) -> RuleStyle {
        RuleStyle { count, spacing: None, ink: Ink::default() }
    }
}

/// A span's settings, read.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SpanStyle {
    ink: Ink,
    italic: Option<bool>,
    skew: Option<f32>,
    /// Times the size around it.
    size: Option<f32>,
    character_padding: Option<f32>,
    word_spacing: Option<f32>,
    /// `Some(None)` takes lines away.
    underline: Option<Option<RuleStyle>>,
    strike: Option<Option<RuleStyle>>,
}

const SPAN_KEYS: [&str; 13] = [
    "color", "colour", "passes", "jitter", "point_jitter", "italic", "skew", "size", "character_padding", "word_spacing", "underline", "strike", "strikethrough",
];
const RULE_KEYS: [&str; 7] = ["count", "spacing", "color", "colour", "passes", "jitter", "point_jitter"];

fn number(caller: &str, key: &str, value: &Value) -> RhaiResult<f64> {
    match value {
        Value::Number(n) => Ok(*n),
        other => Err(format!("{}: '{}' should be a number, got {}", caller, key, other.describe()).into()),
    }
}

fn at_least(caller: &str, key: &str, value: &Value, min: f64) -> RhaiResult<f64> {
    let n = number(caller, key, value)?;
    if n < min {
        return Err(format!("{}: '{}' must be at least {}, got {}", caller, key, min, n).into());
    }
    Ok(n)
}

fn whole(caller: &str, key: &str, value: &Value, max: i64) -> RhaiResult<i64> {
    let n = number(caller, key, value)?;
    if n.fract() != 0.0 || !(1.0..=max as f64).contains(&n) {
        return Err(format!("{}: '{}' should be a whole number from 1 to {}, got {}", caller, key, max, n).into());
    }
    Ok(n as i64)
}

/// The ink settings among `settings`, `key` being one: whether it was.
fn read_ink(caller: &str, ink: &mut Ink, key: &str, value: &Value) -> RhaiResult<bool> {
    match key {
        "color" | "colour" => {
            let Value::Text(colour) = value else {
                return Err(format!("{}: '{}' should be a CSS colour, got {}", caller, key, value.describe()).into());
            };
            parse_color(colour).map_err(|e| format!("{}: {}", caller, e))?;
            ink.color = Some(StrokeColor::Solid(colour.clone()));
        }
        "passes" => ink.passes = Some(whole(caller, key, value, MAX_PASSES)?),
        "jitter" => ink.jitter = Some(at_least(caller, key, value, 0.0)?),
        "point_jitter" => ink.point_jitter = Some(at_least(caller, key, value, 0.0)?),
        _ => return Ok(false),
    }
    Ok(true)
}

fn read_rule(caller: &str, key: &str, value: &Value) -> RhaiResult<Option<RuleStyle>> {
    match value {
        Value::Bool(on) => Ok(on.then_some(RuleStyle::lines(1))),
        Value::Number(_) => Ok(Some(RuleStyle::lines(whole(caller, key, value, MAX_RULES)? as u32))),
        Value::Map(settings) => {
            let caller = format!("{} '{}'", caller, key);
            let mut rule = RuleStyle::lines(1);
            for (k, v) in settings {
                if k == "count" {
                    rule.count = whole(&caller, k, v, MAX_RULES)? as u32;
                } else if k == "spacing" {
                    rule.spacing = Some(at_least(&caller, k, v, 0.0)? as f32);
                } else if !read_ink(&caller, &mut rule.ink, k, v)? {
                    return Err(format!("{}: unknown setting '{}'; settings: {}", caller, k, RULE_KEYS.join(", ")).into());
                }
            }
            Ok(Some(rule))
        }
        Value::Text(_) => Err(format!("{}: '{}' should be true, false, a number of lines, or settings in braces, got {}", caller, key, value.describe()).into()),
    }
}

impl SpanStyle {
    /// Read settings, as a span's braces or a role's `em` and `strong`
    /// give them.
    fn read(caller: &str, settings: &[(String, Value)]) -> RhaiResult<SpanStyle> {
        let mut style = SpanStyle::default();
        for (key, value) in settings {
            if read_ink(caller, &mut style.ink, key, value)? {
                continue;
            }
            let finite = |value: &Value| -> RhaiResult<f32> { Ok(number(caller, key, value)? as f32) };
            match key.as_str() {
                "italic" => match value {
                    Value::Bool(b) => style.italic = Some(*b),
                    other => return Err(format!("{}: 'italic' should be true or false, got {}", caller, other.describe()).into()),
                },
                "skew" => style.skew = Some(finite(value)?),
                "size" => {
                    let size = finite(value)?;
                    if size <= 0.0 {
                        return Err(format!("{}: 'size' must be positive, got {}", caller, size).into());
                    }
                    style.size = Some(size);
                }
                "character_padding" => style.character_padding = Some(at_least(caller, key, value, 0.0)? as f32),
                "word_spacing" => style.word_spacing = Some(at_least(caller, key, value, 0.0)? as f32),
                "underline" => style.underline = Some(read_rule(caller, key, value)?),
                "strike" | "strikethrough" => style.strike = Some(read_rule(caller, key, value)?),
                _ => return Err(format!("{}: unknown setting '{}'; settings: {}", caller, key, SPAN_KEYS.join(", ")).into()),
            }
        }
        Ok(style)
    }

    /// Read a role's `em` or `strong`: a map of the same settings.
    pub(super) fn from_map(caller: &str, value: &Dynamic) -> RhaiResult<SpanStyle> {
        fn convert(caller: &str, value: &Dynamic) -> RhaiResult<Value> {
            if let Ok(b) = value.as_bool() {
                Ok(Value::Bool(b))
            } else if let Ok(n) = value.as_float() {
                Ok(Value::Number(n))
            } else if let Ok(n) = value.as_int() {
                Ok(Value::Number(n as f64))
            } else if let Some(map) = value.clone().try_cast::<Map>() {
                Ok(Value::Map(map.iter().map(|(k, v)| Ok((k.to_string(), convert(caller, v)?))).collect::<RhaiResult<_>>()?))
            } else if let Ok(text) = value.clone().into_immutable_string() {
                Ok(Value::Text(text.to_string()))
            } else {
                Err(format!("{}: settings should be numbers, true or false, strings or maps, got {}", caller, value.type_name()).into())
            }
        }
        match convert(caller, value)? {
            Value::Map(settings) => SpanStyle::read(caller, &settings),
            _ => Err(format!("{}: should be a map of settings, as #{{ italic: true }}", caller).into()),
        }
    }

    /// Emphasis, unless a role says otherwise: italics.
    pub(super) fn emphasis() -> SpanStyle {
        SpanStyle { italic: Some(true), ..SpanStyle::default() }
    }

    /// Strong text, unless a role says otherwise: gone over twice, astray
    /// by a twentieth of the text's `size`.
    pub(super) fn strong(size: f32) -> SpanStyle {
        SpanStyle { ink: Ink { passes: Some(2), jitter: Some(size as f64 / 20.0), point_jitter: None, color: None }, ..SpanStyle::default() }
    }
}

/// Text in a flow or label: plain, or marked up.
#[derive(Clone)]
pub(super) enum FlowText {
    Plain(String),
    Marked(Markdown),
}

impl FlowText {
    /// From a flow's value: a string, or `markdown(...)`.
    pub(super) fn from_dynamic(caller: &str, value: Dynamic) -> RhaiResult<FlowText> {
        if let Some(markdown) = value.clone().try_cast::<Markdown>() {
            return Ok(FlowText::Marked(markdown));
        }
        let type_name = value.type_name();
        value.into_immutable_string().map(|s| FlowText::Plain(s.to_string()))
            .map_err(|_| format!("{} should be a string or markdown(...), got {}", caller, type_name).into())
    }
    /// The text as it's set, without markup.
    pub(super) fn text(&self) -> &str {
        match self {
            FlowText::Plain(text) => text,
            FlowText::Marked(markdown) => markdown.text(),
        }
    }
}

/// How a glyph is drawn: its ink over its slot's style, and the lines under
/// and through it, by index into `Styled::rules`.
#[derive(Clone, Debug, Default)]
pub(super) struct Look {
    pub(super) ink: Ink,
    underline: Option<usize>,
    strike: Option<usize>,
}

/// Lines under or through a span, ready to draw.
#[derive(Clone, Debug)]
struct RuleLook {
    rule: Rule,
    count: u32,
    spacing: Option<f32>,
    ink: Ink,
}

/// Text ready to set in a role: its glyphs, and how each is drawn.
pub(super) struct Styled {
    pub(super) glyphs: Vec<Symbol>,
    /// For each glyph, its look, by index into `looks`.
    look_of: Vec<usize>,
    looks: Vec<Look>,
    rules: Vec<RuleLook>,
}

/// How text is set and drawn at some point in it: the settings of the
/// spans it's in, in turn.
#[derive(Clone)]
struct State {
    font: Font,
    /// Times the role's size.
    scale: f32,
    /// Room between glyphs' outlines a span sets, which glyphs from the
    /// font's fallbacks take too.
    padding: Option<f32>,
    look: Look,
}

impl State {
    fn then(&self, style: &SpanStyle, rules: &mut Vec<RuleLook>) -> State {
        let mut state = self.clone();
        state.look.ink = state.look.ink.then(&style.ink);
        if let Some(italic) = style.italic {
            state.font.skew = match (italic, state.font.skew) {
                (true, 0.0) => ITALIC_SKEW,
                (true, skew) => skew,
                (false, _) => 0.0,
            };
        }
        if let Some(skew) = style.skew {
            state.font.skew = skew;
        }
        if let Some(size) = style.size {
            state.scale *= size;
        }
        if let Some(padding) = style.character_padding {
            state.font.character_padding = padding;
            state.padding = Some(padding);
        }
        if let Some(width) = style.word_spacing {
            state.font.space_width = width;
        }
        // Lines take the ink of the text they're declared on, under their own.
        let ink = state.look.ink.clone();
        for (setting, slot, rule) in [(&style.underline, &mut state.look.underline, Rule::Under), (&style.strike, &mut state.look.strike, Rule::Through)] {
            match setting {
                None => {}
                Some(None) => *slot = None,
                Some(Some(r)) => {
                    rules.push(RuleLook { rule, count: r.count, spacing: r.spacing, ink: ink.then(&r.ink) });
                    *slot = Some(rules.len() - 1);
                }
            }
        }
        state
    }
}

impl Styled {
    /// Plain glyphs, each drawn in its slot's style.
    pub(super) fn plain(glyphs: Vec<Symbol>) -> Styled {
        Styled { look_of: vec![0; glyphs.len()], glyphs, looks: vec![Look::default()], rules: Vec::new() }
    }

    /// Text set in `font`, its spans styled as they say, emphasis and strong
    /// text as `em` and `strong` say. Glyph variants are picked with `rng`,
    /// as `Font::build` picks them.
    pub(super) fn new(text: &FlowText, font: &Font, em: &SpanStyle, strong: &SpanStyle, symdict: &FontBook, rng: &mut dyn rand::RngCore) -> Styled {
        let markdown = match text {
            FlowText::Plain(text) => return Styled::plain(font.build(text, symdict, rng)),
            FlowText::Marked(markdown) => markdown,
        };
        let markup = &markdown.markup;
        let base = State { font: font.clone(), scale: 1.0, padding: None, look: Look::default() };
        let mut rules = Vec::new();
        let mut states: Vec<State> = Vec::with_capacity(markup.spans.len());
        for (span, style) in markup.spans.iter().zip(markdown.styles.iter()) {
            let around = span.parent.map_or(&base, |p| &states[p]);
            let style = match (&span.kind, style) {
                (SpanKind::Emphasis, _) => em,
                (SpanKind::Strong, _) => strong,
                (_, Some(style)) => style,
                (_, None) => unreachable!("settings and strikes are read when the markdown is made"),
            };
            let state = around.then(style, &mut rules);
            states.push(state);
        }
        let owner = markup.innermost();
        let chars: Vec<char> = markup.text.chars().collect();
        let (mut glyphs, mut look_of) = (Vec::with_capacity(chars.len()), Vec::with_capacity(chars.len()));
        let mut k = 0;
        while k < chars.len() {
            // A run of characters in the same span, set alike.
            let span = owner[k];
            let end = k + owner[k..].iter().take_while(|o| **o == span).count();
            let state = span.map_or(&base, |s| &states[s]);
            let run: String = chars[k..end].iter().collect();
            for mut glyph in state.font.build(&run, symdict, rng) {
                glyph.set_scale(state.scale);
                if let (Some(padding), false) = (state.padding, glyph.is_blank()) {
                    glyph = glyph.with_padding(padding);
                }
                glyphs.push(glyph);
                look_of.push(span.map_or(0, |s| s + 1));
            }
            k = end;
        }
        let looks = std::iter::once(base.look).chain(states.into_iter().map(|s| s.look)).collect();
        Styled { glyphs, look_of, looks, rules }
    }

    /// Glyph `k`'s look.
    pub(super) fn look(&self, k: usize) -> &Look {
        &self.looks[self.look_of[k]]
    }

    /// Draw text set from glyph `start` on: each glyph in its look over
    /// `style`, then the lines under and through it, kept `clearance` clear
    /// of `avoid`.
    pub(super) fn draw(&self, canvas: &mut CanvasData, setting: &Setting, start: usize, style: &StrokeStyle, avoid: &AvoidanceTree, clearance: f32) -> RhaiResult<()> {
        let mut add = |path, style: StrokeStyle| -> RhaiResult<()> {
            check_finite("typeset", &path)?;
            canvas.add_path(path, style);
            Ok(())
        };
        for (glyph, place) in setting.placed.iter().zip(&setting.places) {
            add(glyph.placed_strokepath(), self.look(start + place.index).ink.on(style))?;
        }
        for rule in [Rule::Under, Rule::Through] {
            let mark = |k: usize| {
                let look = self.look(start + k);
                if rule == Rule::Under { look.underline } else { look.strike }
            };
            for (r, run) in marked_runs(setting, mark) {
                let glyphs: Vec<&Symbol> = run.iter().map(|&p| &setting.placed[p]).collect();
                let rule_look = &self.rules[r];
                let path = rule_path(&glyphs, rule_look.rule, rule_look.count, rule_look.spacing, Some(avoid), clearance);
                add(path, rule_look.ink.on(style))?;
            }
        }
        Ok(())
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine.register_type_with_name::<Markdown>("Markdown");
    engine.register_fn("markdown", |text: &str| -> Result<Markdown, Box<EvalAltResult>> { Markdown::new(text) });
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, JournalEngine};
    use crate::journal::canvas::StrokeColor;
    use crate::journal::fonts::FontBook;
    use parry2d::math::Vec2;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).unwrap()))
    }

    /// A one-column page set from `body`, with `roles` overriding the body
    /// role's settings.
    fn typeset(body: &str, role: &str, setup: &str) -> Result<Entry, String> {
        engine().build_entry_str(&format!(r#"
            let ink = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            {}
            let one = new_layout("one", |n, info| column([flow("body", #{{ grow: 1 }})], #{{ padding: 20 }}));
            journal_entry.add(one.typeset(#{{
                width: 600, height: 400,
                flows: #{{ body: {} }},
                roles: #{{ body: #{{ font: font("plain"), style: ink, size: 20 {} }} }},
            }}));
        "#, setup, body, role))
    }

    fn strokes(entry: &Entry) -> Vec<(Vec<Vec2>, crate::journal::rhaientries::Stroke)> {
        entry.pages[0].canvases()[0].strokes.iter().map(|s| (s.path.points_within(), s.clone())).collect()
    }

    fn color(stroke: &crate::journal::rhaientries::Stroke) -> String {
        match &stroke.style.color {
            StrokeColor::Solid(c) => c.clone(),
            StrokeColor::Gradient(_) => "gradient".into(),
        }
    }

    #[test]
    fn spans_set_colour_passes_and_size() {
        let entry = typeset(r#"markdown("ab [cd]{color: red, passes: 3, jitter: 2, point_jitter: 1.5} [ef]{size: 2}")"#, "", "").unwrap();
        let s = strokes(&entry);
        // One stroke per glyph: a b c d e f.
        assert_eq!(s.len(), 6);
        let colours: Vec<String> = s.iter().map(|(_, st)| color(st)).collect();
        assert_eq!(colours, ["black", "black", "red", "red", "black", "black"]);
        assert_eq!((s[2].1.style.passes, s[2].1.style.jitter, s[2].1.style.point_jitter, s[0].1.style.passes, s[0].1.style.point_jitter), (3, 2.0, 1.5, 1, 0.0));
        let height = |points: &[Vec2]| points.iter().map(|p| p.y).fold(f32::MIN, f32::max) - points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(height(&s[4].0) > 1.8 * height(&s[0].0), "e is twice a's size: {} vs {}", height(&s[4].0), height(&s[0].0));
        // On one baseline: the big glyphs' bottoms are where the small ones' are.
        let bottom = |points: &[Vec2]| points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((bottom(&s[4].0) - bottom(&s[0].0)).abs() < 2.0, "{} vs {}", bottom(&s[4].0), bottom(&s[0].0));
    }

    #[test]
    fn plain_and_marked_text_set_alike() {
        let plain = typeset(r#""the quick brown fox""#, "", "").unwrap();
        let marked = typeset(r#"markdown("the quick brown fox")"#, "", "").unwrap();
        let paths = |e: &Entry| e.pages[0].canvases()[0].strokes.iter().map(|s| s.path.to_string()).collect::<Vec<_>>();
        assert_eq!(paths(&plain), paths(&marked));
    }

    #[test]
    fn emphasis_and_strong_follow_the_role() {
        let skews = |role: &str| {
            let entry = typeset(r#"markdown("a *b* **c**")"#, role, "").unwrap();
            strokes(&entry).into_iter().map(|(_, s)| (s.style.passes, s.style.jitter)).collect::<Vec<_>>()
        };
        assert_eq!(skews(""), vec![(1, 0.0), (1, 0.0), (2, 1.0)]);
        assert_eq!(skews(", strong: #{ passes: 4 }, em: #{ color: \"blue\" }"), vec![(1, 0.0), (1, 0.0), (4, 0.0)]);
    }

    #[test]
    fn underlines_run_across_words_and_break_at_line_ends() {
        // Three words underlined, the line too narrow for all three: one
        // run on each line, under the words, not past them.
        let entry = engine().build_entry_str(r#"
            let ink = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let one = new_layout("one", |n, info| column([flow("body", #{ grow: 1 })], #{ padding: 20 }));
            journal_entry.add(one.typeset(#{
                width: 160, height: 400,
                flows: #{ body: markdown("[lorem ipsum dolor]{underline: {color: red}} sit") },
                roles: #{ body: #{ font: font("plain"), style: ink, size: 20 } },
            }));
        "#).unwrap();
        let s = strokes(&entry);
        let lines: Vec<&Vec<Vec2>> = s.iter().filter(|(_, st)| color(st) == "red").map(|(p, _)| p).collect();
        let glyphs: Vec<&Vec<Vec2>> = s.iter().filter(|(_, st)| color(st) == "black").map(|(p, _)| p).collect();
        assert_eq!(lines.len(), 2, "one underline per line");
        let ys: Vec<f32> = lines.iter().map(|l| l[0].y).collect();
        assert!(ys[1] > ys[0] + 15.0, "{:?}", ys);
        // Lines: "lorem ipsum", "dolor sit". Each underline is level, just
        // below its line's glyphs, from the first to the last underlined.
        let mut rights = Vec::new();
        for line in &lines {
            assert!((line[0].y - line[1].y).abs() < 1e-3, "underlines are level");
            let on_line: Vec<&&Vec<Vec2>> = glyphs.iter().filter(|g| g.iter().any(|p| (p.y - line[0].y).abs() < 15.0)).collect();
            let xs = || on_line.iter().flat_map(|g| g.iter()).map(|p| p.x);
            let (left, right) = (xs().fold(f32::MAX, f32::min), xs().fold(f32::MIN, f32::max));
            assert!((line[0].x - left).abs() < 2.0, "{:?} starts at its words, {}", line, left);
            rights.push((line[1].x, right));
        }
        assert!((rights[0].0 - rights[0].1).abs() < 2.0, "the first runs to its line's end: {:?}", rights);
        assert!(rights[1].0 < rights[1].1 - 20.0, "the second stops before \"sit\": {:?}", rights);
    }

    #[test]
    fn strikes_scribble_back_and_forth() {
        let entry = typeset(r#"markdown("[scribbled out]{strike: {count: 3, color: hsla(0, 80%, 40%, 0.9)}} and ~~once~~")"#, "", "").unwrap();
        let s = strokes(&entry);
        let red: Vec<&Vec<Vec2>> = s.iter().filter(|(_, st)| color(st).starts_with("hsla")).map(|(p, _)| p).collect();
        assert_eq!(red.len(), 1, "one stroke, back and forth");
        // Three lines across, joined at the ends, each below the last.
        let (xs, ys): (Vec<f32>, Vec<f32>) = red[0].iter().map(|p| (p.x, p.y)).unzip();
        assert_eq!(xs.len(), 6);
        assert!(xs[0] < xs[1] && xs[1] == xs[2] && xs[2] > xs[3] && xs[3] == xs[4] && xs[4] < xs[5], "{:?}", xs);
        assert!(ys.windows(2).all(|w| w[1] > w[0]), "{:?}", ys);
        let once: Vec<&Vec<Vec2>> = s.iter().filter(|(p, st)| color(st) == "black" && p.len() == 2 && (p[0].y - p[1].y).abs() < 1e-3).map(|(p, _)| p).collect();
        assert_eq!(once.len(), 1, "~~once~~ is struck through once");
    }

    #[test]
    fn underlines_break_round_what_text_avoids() {
        // A post through the line: the words split round it, and so does
        // their underline.
        let entry = engine().build_entry_str(r#"
            let ink = new_style("black", new_brush("Ink Pen", 0.3, 0.7, 1.0));
            let page = |n, info| column([
                figure("post", #{ position: "absolute", top: 0, left: 150, width: 10, height: 300 }),
                flow("body", #{ grow: 1, split_words: true }),
            ], #{ padding: 20 });
            journal_entry.add(new_layout("posted", page).typeset(#{
                width: 600, height: 300,
                flows: #{ body: markdown("[abcdefghijklmnopqrstuvwxyz]{underline: {color: red}}") },
                roles: #{ body: #{ font: font("plain"), style: ink, size: 20 }, post: #{ style: ink } },
                art: #{ post: new_line(vec2(0, 0), vec2(0, 10)) },
            }));
        "#).unwrap();
        let s = strokes(&entry);
        let lines: Vec<&Vec<Vec2>> = s.iter().filter(|(_, st)| color(st) == "red").map(|(p, _)| p).collect();
        assert_eq!(lines.len(), 2, "{:?}", lines);
        // The post's box is 150 to 160 across, kept 5 (a quarter size) clear.
        assert!(lines[0][1].x < 145.0 && lines[1][0].x > 165.0, "either side of the post: {:?}", lines);
    }

    #[test]
    fn bad_markdown_fails_where_its_made() {
        for (text, want) in [
            ("[x]{colr: red}", "markdown: [x]: unknown setting 'colr'"),
            ("[x]{color: reddish}", "Invalid color 'reddish'"),
            ("[x]{passes: 0}", "'passes' should be a whole number from 1 to 64"),
            ("[x]{size: -1}", "'size' must be positive"),
            ("[x]{underline: {count: 2, width: 3}}", "unknown setting 'width'"),
            ("[x]{strike: lots}", "'strike' should be true, false, a number of lines"),
            ("[x]{color: }", "'color' has no value"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\nmarkdown({:?});", text)).expect_err(text);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        let err = typeset("3", "", "").expect_err("a number isn't text");
        assert!(err.contains("flow 'body' should be a string or markdown(...)"), "{}", err);
        let err = typeset("\"x\"", ", em: #{ colour: 3 }", "").expect_err("bad em");
        assert!(err.contains("'colour' should be a CSS colour"), "{}", err);
    }
}
