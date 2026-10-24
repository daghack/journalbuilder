//! Marked-up text for page layouts: `markdown(text)` makes a flow whose
//! text is styled as it's set, each stretch in its own style and type.
//!
//! ```rhai
//! let body = markdown("Plain writing, *emphasis*, **strong**, ~~struck out~~, "
//!     + "[written in blue]{color: blue}, [twice the size]{glyph_scale_factor: 2}, "
//!     + "[a warning]{warn}, [a blue warning]{warn, color: blue}, "
//!     + "[scribbled out]{color: black, strike: {count: 3, style: {color: red}}}.",
//!     #{ spans: #{ warn: #{ style: red_ink, type: #{ glyph_scale_factor: 1.2 } } } });
//! layout("two_column_article").typeset(#{ flows: #{ body: body, ... }, ... });
//! ```
//!
//! The markup (see `journal::markup`): `*text*` or `_text_` is the span
//! `em`, `**text**` or `__text__` the span `strong`, `~~text~~` struck
//! through, and `[text]{settings}` text with settings of its own: span
//! names, and `key: value` pairs, separated by commas (a key on its own is
//! `true`). A word on its own is a setting if it's one, and a span's name
//! otherwise; the spans named are applied first, then the settings. Spans
//! nest, each changing what it's in.
//!
//! Settings are a style's (`color`, `passes`, `pass_end_jitter`, ...) and
//! a type's (`italic`, `skew`, `glyph_scale_factor`, `underline`, ...);
//! `glyph_scale_factor` multiplies the size of the text round it. Lines
//! under and through text take `true` (one line), a number of lines,
//! `false` (none, inside text that has them), or `{count, spacing, style}`,
//! drawn by default in the text's style. Lines under text go one below
//! another; lines through it go back and forth through it as one stroke,
//! as scribbling out. They run across the spaces between words, but break
//! where the text does: at the end of a line, either side of what the text
//! avoids, and wherever they'd come near it.
//!
//! Spans are named in a role's `spans` and in `markdown`'s, which win: each
//! `#{ style, type }`, a whole style or type, or a map of some of their
//! settings over the text's. `em` and `strong` are spans too, by default
//! `#{ type: #{ italic: true } }` and
//! `#{ style: #{ passes: 2, pass_end_jitter: 1.3 } }` (gone over twice, a
//! little astray).

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use std::collections::BTreeMap;
use std::rc::Rc;

use super::canvas::CanvasData;
use super::options::{check_keys, Settings};
use super::text_settings::{RuleStyle, TypeStyle};
use super::{check_finite, RhaiResult, Style};
use crate::journal::avoid::AvoidanceTree;
use crate::journal::fonts::FontBook;
use crate::journal::markup::{Markup, SpanKind, Value};
use crate::journal::symbol::Symbol;
use crate::journal::typeset::{marked_runs, rule_path, Rule, Setting};

/// How a span changes the text it covers: a style, a type, each whole or
/// a map of some settings over the text's.
#[derive(Clone, Debug, Default)]
pub(super) struct SpanStyle {
    style: Option<Dynamic>,
    type_: Option<Dynamic>,
}

impl SpanStyle {
    /// A span's definition, `#{ style, type }`, checked.
    pub(super) fn from_dynamic(caller: &str, value: &Dynamic) -> RhaiResult<SpanStyle> {
        let map = value.clone().try_cast::<Map>()
            .ok_or_else(|| format!("{}: a span should be #{{ style, type }}, got {}", caller, value.type_name()))?;
        check_keys(caller, &map, &["style", "type"])?;
        let span = SpanStyle { style: map.get("style").cloned(), type_: map.get("type").cloned() };
        span.check(caller)?;
        Ok(span)
    }
    /// Check its settings, as they'd be applied to the defaults.
    fn check(&self, caller: &str) -> RhaiResult<()> {
        if let Some(style) = &self.style {
            Style::given(&format!("{} style", caller), style, &Style::default_style())?;
        }
        if let Some(t) = &self.type_ {
            TypeStyle::given(&format!("{} type", caller), t, &TypeStyle::empty())?;
        }
        Ok(())
    }
    /// Emphasis, unless a span says otherwise: italics.
    fn emphasis() -> SpanStyle {
        SpanStyle { style: None, type_: Some(Map::from_iter([("italic".into(), true.into())]).into()) }
    }
    /// Strong text, unless a span says otherwise: gone over twice, a
    /// little astray.
    fn strong() -> SpanStyle {
        let style = Map::from_iter([("passes".into(), Dynamic::from(2_i64)), ("pass_end_jitter".into(), Dynamic::from(1.3))]);
        SpanStyle { style: Some(style.into()), type_: None }
    }
    /// Struck through once, as `~~text~~` is.
    fn struck() -> SpanStyle {
        SpanStyle { style: None, type_: Some(Map::from_iter([("strike".into(), true.into())]).into()) }
    }
}

/// Named spans, by name.
#[derive(Clone, Debug, Default)]
pub(super) struct Spans(BTreeMap<String, SpanStyle>);

impl Spans {
    /// A `spans` map, `#{ name: #{ style, type } }`, checked.
    pub(super) fn from_dynamic(caller: &str, value: &Dynamic) -> RhaiResult<Spans> {
        let map = value.clone().try_cast::<Map>()
            .ok_or_else(|| format!("{}: 'spans' should be a map of named spans, got {}", caller, value.type_name()))?;
        let mut spans = BTreeMap::new();
        for (name, span) in map {
            if Style::KEYS.contains(&name.as_str()) || TypeStyle::KEYS.contains(&name.as_str()) {
                return Err(format!("{}: a span can't be named '{}', a setting's name", caller, name).into());
            }
            spans.insert(name.to_string(), SpanStyle::from_dynamic(&format!("{} span '{}'", caller, name), &span)?);
        }
        Ok(Spans(spans))
    }
    /// These spans with `other`'s on top.
    pub(super) fn then(&self, other: &Spans) -> Spans {
        let mut spans = self.0.clone();
        spans.extend(other.0.clone());
        Spans(spans)
    }
    fn get(&self, name: &str) -> Option<&SpanStyle> {
        self.0.get(name)
    }
}

/// A setting's value as rhai has it: whole numbers as integers.
fn dynamic(value: &Value) -> Dynamic {
    match value {
        Value::Bool(b) => (*b).into(),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => (*n as i64).into(),
        Value::Number(n) => (*n).into(),
        Value::Text(t) => t.clone().into(),
        Value::Map(entries) => Map::from_iter(entries.iter().map(|(k, v)| (k.into(), dynamic(v)))).into(),
    }
}

/// A `[text]{...}` span's settings: the spans it names, and the settings
/// it gives itself, as a span.
#[derive(Clone, Debug, Default)]
struct Inline {
    names: Vec<String>,
    own: SpanStyle,
}

impl Inline {
    fn read(caller: &str, settings: &[(String, Value)]) -> RhaiResult<Inline> {
        let (mut names, mut style, mut type_) = (Vec::new(), Map::new(), Map::new());
        for (key, value) in settings {
            if Style::KEYS.contains(&key.as_str()) {
                style.insert(key.into(), dynamic(value));
            } else if TypeStyle::KEYS.contains(&key.as_str()) {
                type_.insert(key.into(), dynamic(value));
            } else if *value == Value::Bool(true) {
                names.push(key.clone());
            } else {
                return Err(format!("{}: unknown setting '{}'; settings are a style's or a type's, and a span's name stands alone", caller, key).into());
            }
        }
        let own = SpanStyle {
            style: (!style.is_empty()).then(|| style.into()),
            type_: (!type_.is_empty()).then(|| type_.into()),
        };
        own.check(caller)?;
        Ok(Inline { names, own })
    }
}

/// Text with markup, as `markdown(text)` makes it: the text without it, its
/// spans' settings read, and the spans it names.
#[derive(Clone)]
pub struct Markdown {
    markup: Rc<Markup>,
    /// Each `[text]{...}` span's settings; `None` for the others.
    inline: Rc<Vec<Option<Inline>>>,
    spans: Spans,
}

impl Markdown {
    fn new(source: &str, options: &Map) -> RhaiResult<Markdown> {
        const CALLER: &str = "markdown";
        check_keys(CALLER, options, &["spans"])?;
        let spans = options.get("spans").map_or(Ok(Spans::default()), |s| Spans::from_dynamic(CALLER, s))?;
        let markup = Markup::parse(source).map_err(|e| format!("{}: {}", CALLER, e))?;
        let chars: Vec<char> = markup.text.chars().collect();
        let inline = markup.spans.iter().map(|span| {
            let caller = || format!("{}: [{}]", CALLER, chars[span.range.clone()].iter().collect::<String>());
            match &span.kind {
                SpanKind::Settings(settings) => Inline::read(&caller(), settings).map(Some),
                _ => Ok(None),
            }
        }).collect::<RhaiResult<Vec<_>>>()?;
        Ok(Markdown { markup: Rc::new(markup), inline: Rc::new(inline), spans })
    }
    /// The text, markup taken out.
    pub(super) fn text(&self) -> &str {
        &self.markup.text
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

/// What strokes are drawn in, over a slot's style: the styles of the spans
/// they're in, outermost first, each whole or settings over the last.
#[derive(Clone, Debug, Default)]
pub(super) struct Ink(Vec<Dynamic>);

impl Ink {
    /// `base` with this ink's changes.
    pub(super) fn on(&self, caller: &str, base: &Style) -> RhaiResult<Style> {
        let mut style = base.clone();
        for change in &self.0 {
            style = Style::given(caller, change, &style)?;
        }
        Ok(style)
    }
    fn then(&self, change: Option<&Dynamic>) -> Ink {
        let mut ink = self.clone();
        ink.0.extend(change.cloned());
        ink
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
    type_: TypeStyle,
    /// Room between glyphs' outlines a span sets, which glyphs from the
    /// glyphset's fallbacks take too.
    padding: Option<f32>,
    look: Look,
}

impl State {
    /// The text's state at its start, in its role's type.
    fn base(type_: &TypeStyle, rules: &mut Vec<RuleLook>) -> State {
        let mut state = State { type_: type_.clone(), padding: None, look: Look::default() };
        state.rules(true, true, rules);
        state
    }
    /// Lines from the type, where `underline` and `strike` say they're set.
    fn rules(&mut self, underline: bool, strike: bool, rules: &mut Vec<RuleLook>) {
        let ink = self.look.ink.clone();
        for (set, rule, slot, kind) in [(underline, self.type_.underline(), &mut self.look.underline, Rule::Under), (strike, self.type_.strike(), &mut self.look.strike, Rule::Through)] {
            if !set {
                continue;
            }
            *slot = rule.map(|r: &RuleStyle| {
                rules.push(RuleLook { rule: kind, count: r.count, spacing: r.spacing, ink: ink.then(r.style.as_ref()) });
                rules.len() - 1
            });
        }
    }
    fn then(&self, caller: &str, span: &SpanStyle, rules: &mut Vec<RuleLook>) -> RhaiResult<State> {
        let mut state = self.clone();
        state.look.ink = state.look.ink.then(span.style.as_ref());
        if let Some(change) = &span.type_ {
            state.type_ = TypeStyle::given(caller, change, &self.type_)?;
            let map = change.clone().try_cast::<Map>();
            let sets = |key: &str| map.as_ref().is_none_or(|m| m.contains_key(key));
            // A factor multiplies the size round it.
            if map.as_ref().is_some_and(|m| m.contains_key("glyph_scale_factor")) {
                state.type_.set(caller, "glyph_scale_factor", &(self.type_.scale_factor() * state.type_.scale_factor()).into())?;
            }
            if sets("glyph_outline_gap") {
                state.padding = state.type_.outline_gap().map(|gap| gap as f32);
            }
            state.rules(sets("underline"), sets("strike"), rules);
        }
        Ok(state)
    }
}

impl Styled {
    /// Text set in `type_`, its spans styled as they say, by the spans
    /// `spans` names (the markdown's own on top). Glyph variants are picked
    /// with `rng`, as glyphsets pick them.
    pub(super) fn new(caller: &str, text: &FlowText, type_: &TypeStyle, spans: &Spans, symdict: &FontBook, rng: &mut dyn rand::RngCore) -> RhaiResult<Styled> {
        // How big a run's glyphs are next to the text's: as their sizes
        // are, or (for text with no size, a drop cap) their factors.
        let scale_of = |run: &TypeStyle| -> RhaiResult<f32> {
            match type_.has_size() && run.has_size() {
                true => Ok(run.size(caller)? / type_.size(caller)?),
                false => Ok((run.scale_factor() / type_.scale_factor()) as f32),
            }
        };
        let markdown = match text {
            FlowText::Marked(markdown) => markdown,
            FlowText::Plain(text) => {
                let mut rules = Vec::new();
                let base = State::base(type_, &mut rules);
                let glyphs = base.type_.glyphset(caller)?.build(text, symdict, rng);
                return Ok(Styled { look_of: vec![0; glyphs.len()], glyphs, looks: vec![base.look], rules });
            }
        };
        let spans = spans.then(&markdown.spans);
        let (em, strong, struck) = (SpanStyle::emphasis(), SpanStyle::strong(), SpanStyle::struck());
        let markup = &markdown.markup;
        let chars: Vec<char> = markup.text.chars().collect();
        let mut rules = Vec::new();
        let base = State::base(type_, &mut rules);
        let mut states: Vec<State> = Vec::with_capacity(markup.spans.len());
        for (span, inline) in markup.spans.iter().zip(markdown.inline.iter()) {
            let caller = format!("{}: [{}]", caller, chars[span.range.clone()].iter().collect::<String>());
            let around = span.parent.map_or(&base, |p| &states[p]);
            let named = |name: &str| -> RhaiResult<&SpanStyle> {
                spans.get(name).ok_or_else(|| {
                    let names: Vec<&str> = spans.0.keys().map(String::as_str).collect();
                    format!("{}: no span named '{}'; spans: {}", caller, name, names.join(", ")).into()
                })
            };
            let state = match (&span.kind, inline) {
                (SpanKind::Emphasis, _) => around.then(&caller, spans.get("em").unwrap_or(&em), &mut rules)?,
                (SpanKind::Strong, _) => around.then(&caller, spans.get("strong").unwrap_or(&strong), &mut rules)?,
                (SpanKind::Strike, _) => around.then(&caller, &struck, &mut rules)?,
                (SpanKind::Settings(_), Some(inline)) => {
                    let mut state = around.clone();
                    for name in &inline.names {
                        state = state.then(&caller, named(name)?, &mut rules)?;
                    }
                    state.then(&caller, &inline.own, &mut rules)?
                }
                (SpanKind::Settings(_), None) => unreachable!("settings are read when the markdown is made"),
            };
            states.push(state);
        }
        let owner = markup.innermost();
        let (mut glyphs, mut look_of) = (Vec::with_capacity(chars.len()), Vec::with_capacity(chars.len()));
        let mut k = 0;
        while k < chars.len() {
            // A run of characters in the same span, set alike.
            let span = owner[k];
            let end = k + owner[k..].iter().take_while(|o| **o == span).count();
            let state = span.map_or(&base, |s| &states[s]);
            let run: String = chars[k..end].iter().collect();
            let scale = scale_of(&state.type_)?;
            for mut glyph in state.type_.glyphset(caller)?.build(&run, symdict, rng) {
                glyph.set_scale(scale);
                if let (Some(padding), false) = (state.padding, glyph.is_blank()) {
                    glyph = glyph.with_padding(padding);
                }
                glyphs.push(glyph);
                look_of.push(span.map_or(0, |s| s + 1));
            }
            k = end;
        }
        let looks = std::iter::once(base.look).chain(states.into_iter().map(|s| s.look)).collect();
        Ok(Styled { glyphs, look_of, looks, rules })
    }

    /// Glyph `k`'s look.
    pub(super) fn look(&self, k: usize) -> &Look {
        &self.looks[self.look_of[k]]
    }

    /// Draw text set from glyph `start` on: each glyph in its look over
    /// `style`, then the lines under and through it, kept `clearance` clear
    /// of `avoid`, the hands' jitter drawn from `rng`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(&self, canvas: &mut CanvasData, setting: &Setting, start: usize, style: &Style, avoid: &AvoidanceTree, clearance: f32, rng: &mut dyn rand::RngCore) -> RhaiResult<()> {
        const CALLER: &str = "typeset";
        let mut add = |path, style: Style| -> RhaiResult<()> {
            check_finite(CALLER, &path)?;
            canvas.add_path(path, &style, rng);
            Ok(())
        };
        for (glyph, place) in setting.placed.iter().zip(&setting.places) {
            add(glyph.placed_strokepath(), self.look(start + place.index).ink.on(CALLER, style)?)?;
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
                add(path, rule_look.ink.on(CALLER, style)?)?;
            }
        }
        Ok(())
    }
}

pub(super) fn register(engine: &mut Engine) {
    engine.register_type_with_name::<Markdown>("Markdown");
    engine.register_fn("markdown", |text: &str| -> Result<Markdown, Box<EvalAltResult>> { Markdown::new(text, &Map::new()) });
    engine.register_fn("markdown", |text: &str, options: Map| -> Result<Markdown, Box<EvalAltResult>> { Markdown::new(text, &options) });
}

#[cfg(test)]
mod tests {
    use super::super::options::Settings;
    use super::super::{Entry, JournalEngine};
    use crate::journal::canvas::StrokeColor;
    use crate::journal::fonts::FontBook;
    use parry2d::math::Vec2;
    use std::rc::Rc;

    fn engine() -> JournalEngine {
        JournalEngine::new(Rc::new(FontBook::from_file(crate::journal::fonts::DEFAULT_FONTS).unwrap()))
    }

    /// A one-column page set from `body`, with `role` added to the body
    /// role's settings.
    fn typeset(body: &str, role: &str, setup: &str) -> Result<Entry, String> {
        engine().build_entry_str(&format!(r#"
            let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            {}
            let one = new_layout("one", |n, info| column([flow("body", #{{ flex: #{{ grow: 1 }} }})], #{{ flex: #{{ padding: 20 }} }}));
            journal_entry.add(one.typeset(#{{
                width: 600, height: 400,
                flows: #{{ body: {} }},
                roles: #{{ body: #{{ style: ink, type: new_type_style(glyphset("plain"), 20) {} }} }},
            }}));
        "#, setup, body, role))
    }

    fn strokes(entry: &Entry) -> Vec<(Vec<Vec2>, crate::journal::rhaientries::Stroke)> {
        entry.pages[0].canvases()[0].strokes.iter().map(|s| (s.path.points_within(), s.clone())).collect()
    }

    fn color(stroke: &crate::journal::rhaientries::Stroke) -> String {
        match stroke.style.color() {
            StrokeColor::Solid(c) => c.clone(),
            StrokeColor::Gradient(_) => "gradient".into(),
        }
    }

    /// A stroke's style's setting, as a number.
    fn setting(stroke: &crate::journal::rhaientries::Stroke, key: &str) -> f64 {
        let value = stroke.style.get(key);
        value.as_float().or_else(|_| value.as_int().map(|n| n as f64)).unwrap()
    }

    #[test]
    fn spans_set_style_and_type() {
        let entry = typeset(r#"markdown("ab [cd]{color: red, passes: 3, pass_end_jitter: 2, stroke_point_jitter: 1.5} [ef]{glyph_scale_factor: 2}")"#, "", "").unwrap();
        let s = strokes(&entry);
        // One stroke per glyph: a b c d e f.
        assert_eq!(s.len(), 6);
        let colours: Vec<String> = s.iter().map(|(_, st)| color(st)).collect();
        assert_eq!(colours, ["black", "black", "red", "red", "black", "black"]);
        let hand = |st: &crate::journal::rhaientries::Stroke| (setting(st, "passes"), setting(st, "pass_end_jitter"), setting(st, "stroke_point_jitter"));
        assert_eq!((hand(&s[2].1), hand(&s[0].1)), ((3.0, 2.0, 1.5), (1.0, 0.0, 0.0)));
        let height = |points: &[Vec2]| points.iter().map(|p| p.y).fold(f32::MIN, f32::max) - points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        // e's passes are one; c's three passes make its points three times as many.
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
    fn emphasis_and_strong_are_spans_the_role_and_the_text_can_name() {
        let passes = |role: &str, text: &str| {
            let entry = typeset(text, role, "").unwrap();
            strokes(&entry).into_iter().map(|(_, s)| (setting(&s, "passes"), setting(&s, "pass_end_jitter"))).collect::<Vec<_>>()
        };
        let text = r#"markdown("a *b* **c**")"#;
        assert_eq!(passes("", text), vec![(1.0, 0.0), (1.0, 0.0), (2.0, 1.3)]);
        let role = r#", spans: #{ strong: #{ style: #{ passes: 4 } }, em: #{ style: #{ color: "blue" } } }"#;
        assert_eq!(passes(role, text), vec![(1.0, 0.0), (1.0, 0.0), (4.0, 0.0)]);
        // The text's own spans win over the role's.
        let own = r#"markdown("a *b* **c**", #{ spans: #{ strong: #{ style: #{ passes: 6 } } } })"#;
        assert_eq!(passes(role, own), vec![(1.0, 0.0), (1.0, 0.0), (6.0, 0.0)]);
    }

    #[test]
    fn named_spans_go_first_then_inline_settings() {
        let text = r#"markdown("a [b]{warn} [c]{warn, color: blue}", #{ spans: #{ warn: #{ style: #{ color: "red", passes: 2 } } } })"#;
        let s = strokes(&typeset(text, "", "").unwrap());
        let looks: Vec<(String, f64)> = s.iter().map(|(_, st)| (color(st), setting(st, "passes"))).collect();
        assert_eq!(looks, vec![("black".into(), 1.0), ("red".into(), 2.0), ("blue".into(), 2.0)]);
        let err = typeset(r#"markdown("[b]{shout}")"#, "", "").expect_err("no such span");
        assert!(err.contains("no span named 'shout'"), "{}", err);
        let err = engine().build_entry_str("let ok = 1;\nmarkdown(\"x\", #{ spans: #{ italic: #{ type: #{ skew: 0.1 } } } });").expect_err("a setting's name");
        assert!(err.contains("a span can't be named 'italic'") && err.contains("line 2"), "{}", err);
    }

    #[test]
    fn underlines_run_across_words_and_break_at_line_ends() {
        // Three words underlined, the line too narrow for all three: one
        // run on each line, under the words, not past them.
        let entry = engine().build_entry_str(r#"
            let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let one = new_layout("one", |n, info| column([flow("body", #{ flex: #{ grow: 1 } })], #{ flex: #{ padding: 20 } }));
            journal_entry.add(one.typeset(#{
                width: 160, height: 400,
                flows: #{ body: markdown("[lorem ipsum dolor]{underline: {style: {color: red}}} sit") },
                roles: #{ body: #{ style: ink, type: new_type_style(glyphset("plain"), 20) } },
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
        let entry = typeset(r#"markdown("[scribbled out]{strike: {count: 3, style: {color: hsla(0, 80%, 40%, 0.9)}}} and ~~once~~")"#, "", "").unwrap();
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
            let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
            let page = |n, info| column([
                figure("post", #{ flex: #{ position: "absolute", top: 0, left: 150, width: 10, height: 300 } }),
                flow("body", #{ flex: #{ grow: 1 }, paragraph: #{ split_words: true } }),
            ], #{ flex: #{ padding: 20 } });
            journal_entry.add(new_layout("posted", page).typeset(#{
                width: 600, height: 300,
                flows: #{ body: markdown("[abcdefghijklmnopqrstuvwxyz]{underline: {style: {color: red}}}") },
                roles: #{ body: #{ style: ink, type: new_type_style(glyphset("plain"), 20) }, post: #{ style: ink } },
                art: #{ post: new_line(vec2(0, 0), vec2(0, 10)) },
            }));
        "#).unwrap();
        let s = strokes(&entry);
        let lines: Vec<&Vec<Vec2>> = s.iter().filter(|(_, st)| color(st) == "red").map(|(p, _)| p).collect();
        assert_eq!(lines.len(), 2, "{:?}", lines);
        // The post's box is 150 to 160 across, kept 3 (the clearance) clear.
        assert!(lines[0][1].x < 147.0 && lines[1][0].x > 163.0, "either side of the post: {:?}", lines);
    }

    #[test]
    fn bad_markdown_fails_where_its_made() {
        for (text, want) in [
            ("[x]{colr: red}", "markdown: [x]: unknown setting 'colr'"),
            ("[x]{color: reddish}", "Invalid color 'reddish'"),
            ("[x]{passes: 0}", "'passes' should be from 1 to 64"),
            ("[x]{glyph_scale_factor: -1}", "'glyph_scale_factor' must be positive"),
            ("[x]{underline: {count: 2, width: 3}}", "unknown option 'width'"),
            ("[x]{strike: lots}", "'strike' should be true, false, a number of lines"),
            ("[x]{color: }", "'color' has no value"),
        ] {
            let err = engine().build_entry_str(&format!("let ok = 1;\nmarkdown({:?});", text)).expect_err(text);
            assert!(err.contains(want) && err.contains("line 2"), "{}: {}", want, err);
        }
        let err = typeset("3", "", "").expect_err("a number isn't text");
        assert!(err.contains("flow 'body' should be a string or markdown(...)"), "{}", err);
        let err = typeset("\"x\"", ", spans: #{ em: #{ style: #{ color: 3 } } }", "").expect_err("bad em");
        assert!(err.contains("'color'") || err.contains("colour"), "{}", err);
    }
}
