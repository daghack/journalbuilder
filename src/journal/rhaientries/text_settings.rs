//! Text styles, and the settings every text call takes.
//!
//! ```rhai
//! // Type: which glyphs, how big, shaped how.
//! let body = new_type_style(glyphset("plain"), 26, #{
//!     glyph_tracking: 0.05,      // extra room after each glyph, in glyph scales (default 0)
//!     glyph_outline_gap: 0.1,    // room between glyphs' outlines, in glyph scales (default: the glyphset's)
//!     space_width: 0.4,          // a space's width, in glyph scales (default: the glyphset's)
//!     skew: 0.2,                 // glyphs leant 0.2 radians (default: the glyphset's); or italic: true
//!     joined: false,             // a cursive glyphset's words written unjoined (default: the glyphset's)
//!     glyph_scale_factor: 1.5,   // times the glyph scale (default 1)
//!     underline: true,           // one line under (or a number of lines, or #{ count, spacing, style })
//!     strike: 2,                 // two lines through
//! });
//!
//! // Paragraphs: how lines are built and set.
//! let para = new_paragraph_style(#{
//!     line_height: 1.3, paragraph_gap: 0.5, indent: 1,    // in glyph scales (paragraph_gap in lines)
//!     align: "justify", valign: "top", fit: false,
//!     layout: "words",            // or "flat", glyph by glyph (default: the call's)
//!     split_words: true, split_padding: 0.3,
//!     clearance: 3,               // from what text avoids, in canvas units (default 3)
//!     line_gap_jitter: 2,         // each line up to 2 units off its even spacing
//!     line_start_jitter: 4,       // each line starting up to 4 units further in
//! });
//!
//! // Every text call takes a style, a type and a paragraph, each whole or
//! // as a map of some settings (over the default style, no type, and the
//! // default paragraph), and the call's own options.
//! write_block("lorem ipsum", vec2(0, 0), 400, #{ style: ink, type: body, paragraph: para, avoid: drawing });
//! path.fill_with_text("lorem ipsum", #{ type: #{ glyphset: glyphset("plain"), glyph_scale: 20 } });
//! ```
//!
//! A call uses the settings it needs and ignores the rest. Jitters, the
//! handwriting's and the paragraph's, are in canvas units (rotation in
//! radians, scale a fraction) and drawn from the journal's generator.

use rand::{Rng, RngCore};
use rhai::{Dynamic, Map};

use super::options::{above_zero, at_least_zero, check_keys, one_of, setting_number, Settings};
use super::style::Style;
use super::RhaiResult;
use crate::journal::canvas::textbox::TextBoxLineMode;
use crate::journal::fonts::Font;
use crate::journal::paths::StrokePath;
use crate::journal::symbol::{Symbol, WordSplit};
use crate::journal::typeset::Align;
use parry2d::math::Vec2;

/// How far `italic: true` leans text, in radians.
pub(super) const ITALIC_SKEW: f64 = 0.25;
/// The most lines text can be underlined or struck through with.
const MAX_RULES: i64 = 20;

/// Lines under or through text.
#[derive(Clone, Debug)]
pub(super) struct RuleStyle {
    pub(super) count: u32,
    /// How far apart, in glyph scales; by default, as the pen sets them.
    pub(super) spacing: Option<f32>,
    /// What they're drawn in: a style, or a map of settings over the text's.
    pub(super) style: Option<Dynamic>,
}

impl RuleStyle {
    pub(super) fn lines(count: u32) -> RuleStyle {
        RuleStyle { count, spacing: None, style: None }
    }
    /// `true` (one line), `false` (none), a number of lines, or
    /// `#{ count, spacing, style }`.
    fn read(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<Option<RuleStyle>> {
        let count = |value: &Dynamic| -> RhaiResult<u32> {
            match value.as_int() {
                Ok(n) if (1..=MAX_RULES).contains(&n) => Ok(n as u32),
                _ => Err(format!("{}: '{}' lines should be a whole number from 1 to {}, got {}", caller, key, MAX_RULES, value).into()),
            }
        };
        if let Ok(on) = value.as_bool() {
            return Ok(on.then(|| RuleStyle::lines(1)));
        }
        if value.is_int() {
            return Ok(Some(RuleStyle::lines(count(value)?)));
        }
        let Some(settings) = value.clone().try_cast::<Map>() else {
            return Err(format!("{}: '{}' should be true, false, a number of lines or #{{ count, spacing, style }}, got {}", caller, key, value.type_name()).into());
        };
        let caller = format!("{} '{}'", caller, key);
        check_keys(&caller, &settings, &["count", "spacing", "style"])?;
        let mut rule = RuleStyle::lines(1);
        if let Some(n) = settings.get("count") {
            rule.count = count(n)?;
        }
        if let Some(spacing) = settings.get("spacing") {
            rule.spacing = Some(at_least_zero(&caller, "spacing", spacing)? as f32);
        }
        if let Some(style) = settings.get("style") {
            // Checked now, though it's drawn over the text's style.
            Style::given(&format!("{} style", caller), style, &Style::default_style())?;
            rule.style = Some(style.clone());
        }
        Ok(Some(rule))
    }
    fn get(&self) -> Dynamic {
        let mut map = Map::new();
        map.insert("count".into(), (self.count as i64).into());
        if let Some(spacing) = self.spacing {
            map.insert("spacing".into(), (spacing as f64).into());
        }
        if let Some(style) = &self.style {
            map.insert("style".into(), style.clone());
        }
        map.into()
    }
}

/// Type: which glyphs text is set in, how big, and shaped how.
#[derive(Clone)]
pub struct TypeStyle {
    glyphset: Option<Font>,
    glyph_scale: Option<f64>,
    glyph_scale_factor: f64,
    glyph_outline_gap: Option<f64>,
    glyph_tracking: f64,
    space_width: Option<f64>,
    skew: Option<f64>,
    joined: Option<bool>,
    underline: Option<RuleStyle>,
    strike: Option<RuleStyle>,
}

impl TypeStyle {
    /// No glyphset or scale yet: what a map of type settings is read over.
    pub(super) fn empty() -> TypeStyle {
        TypeStyle {
            glyphset: None,
            glyph_scale: None,
            glyph_scale_factor: 1.0,
            glyph_outline_gap: None,
            glyph_tracking: 0.0,
            space_width: None,
            skew: None,
            joined: None,
            underline: None,
            strike: None,
        }
    }
    /// The glyphset, as this type sets it: its gap, space, skew and
    /// joining overridden as the type says. `caller` needs one.
    pub(super) fn glyphset(&self, caller: &str) -> RhaiResult<Font> {
        let mut font = self.glyphset.clone().ok_or_else(|| format!("{}: the type has no 'glyphset', the glyphs text is set in", caller))?;
        if let Some(gap) = self.glyph_outline_gap {
            font.character_padding = gap as f32;
        }
        if let Some(width) = self.space_width {
            font.space_width = width as f32;
        }
        if let Some(skew) = self.skew {
            font.skew = skew as f32;
        }
        if let Some(joined) = self.joined {
            font.joined = joined && font.joins();
        }
        Ok(font)
    }
    /// The glyph scale text is set at, factor included. `caller` needs one.
    pub(super) fn size(&self, caller: &str) -> RhaiResult<f32> {
        let scale = self.glyph_scale.ok_or_else(|| format!("{}: the type has no 'glyph_scale', the size glyphs are set at", caller))?;
        Ok((scale * self.glyph_scale_factor) as f32)
    }
    pub(super) fn has_size(&self) -> bool {
        self.glyph_scale.is_some()
    }
    /// Extra room after each glyph, in glyph scales.
    pub(super) fn tracking(&self) -> f32 {
        self.glyph_tracking as f32
    }
    pub(super) fn scale_factor(&self) -> f64 {
        self.glyph_scale_factor
    }
    pub(super) fn outline_gap(&self) -> Option<f64> {
        self.glyph_outline_gap
    }
    pub(super) fn underline(&self) -> Option<&RuleStyle> {
        self.underline.as_ref()
    }
    pub(super) fn strike(&self) -> Option<&RuleStyle> {
        self.strike.as_ref()
    }
}

impl Settings for TypeStyle {
    const NAME: &'static str = "type_style";
    const KEYS: &'static [&'static str] = &[
        "glyphset", "glyph_scale", "glyph_scale_factor", "glyph_outline_gap", "glyph_tracking", "space_width", "skew", "italic", "joined", "underline", "strike",
    ];
    fn get(&self, key: &str) -> Dynamic {
        let optional = |v: Option<f64>| v.map_or(Dynamic::UNIT, Dynamic::from);
        let rule = |r: &Option<RuleStyle>| r.as_ref().map_or(Dynamic::from(false), RuleStyle::get);
        match key {
            "glyphset" => self.glyphset.clone().map_or(Dynamic::UNIT, Dynamic::from),
            "glyph_scale" => optional(self.glyph_scale),
            "glyph_scale_factor" => self.glyph_scale_factor.into(),
            "glyph_outline_gap" => optional(self.glyph_outline_gap),
            "glyph_tracking" => self.glyph_tracking.into(),
            "space_width" => optional(self.space_width),
            "skew" => optional(self.skew),
            "italic" => self.skew.map_or(Dynamic::UNIT, |s| Dynamic::from(s != 0.0)),
            "joined" => self.joined.map_or(Dynamic::UNIT, Dynamic::from),
            "underline" => rule(&self.underline),
            "strike" => rule(&self.strike),
            _ => Dynamic::UNIT,
        }
    }
    fn set(&mut self, caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()> {
        match key {
            "glyphset" => {
                self.glyphset = Some(value.clone().try_cast::<Font>()
                    .ok_or_else(|| format!("{}: 'glyphset' should be a glyphset, got {}", caller, value.type_name()))?);
            }
            "glyph_scale" => self.glyph_scale = Some(above_zero(caller, key, value)?),
            "glyph_scale_factor" => self.glyph_scale_factor = above_zero(caller, key, value)?,
            "glyph_outline_gap" => self.glyph_outline_gap = Some(at_least_zero(caller, key, value)?),
            "glyph_tracking" => {
                let tracking = setting_number(caller, key, value)?;
                // Down to glyphs touching their neighbours' centres.
                if tracking <= -0.5 {
                    return Err(format!("{}: 'glyph_tracking' must be above -0.5, got {}", caller, tracking).into());
                }
                self.glyph_tracking = tracking;
            }
            "space_width" => self.space_width = Some(above_zero(caller, key, value)?),
            "skew" => self.skew = Some(super::glyphs::skew_angle(caller, value)? as f64),
            "italic" => {
                let italic = value.as_bool().map_err(|t| format!("{}: 'italic' should be true or false, got {}", caller, t))?;
                self.skew = Some(if italic { ITALIC_SKEW } else { 0.0 });
            }
            "joined" => self.joined = Some(value.as_bool().map_err(|t| format!("{}: 'joined' should be true or false, got {}", caller, t))?),
            "underline" => self.underline = RuleStyle::read(caller, key, value)?,
            "strike" => self.strike = RuleStyle::read(caller, key, value)?,
            _ => unreachable!("{} isn't a type setting", key),
        }
        Ok(())
    }
}

/// Paragraphs: how lines of text are built and set.
#[derive(Clone, Debug)]
pub struct ParagraphStyle {
    line_height: f64,
    paragraph_gap: f64,
    indent: f64,
    align: String,
    valign: String,
    fit: bool,
    layout: Option<String>,
    angle: Option<f64>,
    split_words: Option<usize>,
    split_padding: f64,
    clearance: f64,
    line_gap_jitter: f64,
    line_start_jitter: f64,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        ParagraphStyle {
            line_height: 1.25,
            paragraph_gap: 0.5,
            indent: 0.0,
            align: "left".into(),
            valign: "top".into(),
            fit: false,
            layout: None,
            angle: None,
            split_words: None,
            split_padding: 0.0,
            clearance: 3.0,
            line_gap_jitter: 0.0,
            line_start_jitter: 0.0,
        }
    }
}

impl ParagraphStyle {
    pub(super) fn line_height(&self) -> f32 {
        self.line_height as f32
    }
    pub(super) fn paragraph_gap(&self) -> f32 {
        self.paragraph_gap as f32
    }
    pub(super) fn indent(&self) -> f32 {
        self.indent as f32
    }
    pub(super) fn align(&self) -> Align {
        Align::from_name(&self.align).expect("checked when set")
    }
    /// How far down the room left text sits: 0 at the top, 1 at the bottom.
    pub(super) fn valign(&self) -> f32 {
        match self.valign.as_str() {
            "center" => 0.5,
            "bottom" => 1.0,
            _ => 0.0,
        }
    }
    pub(super) fn fit(&self) -> bool {
        self.fit
    }
    pub(super) fn angle(&self) -> Option<f32> {
        self.angle.map(|a| a as f32)
    }
    pub(super) fn clearance(&self) -> f32 {
        self.clearance as f32
    }
    pub(super) fn line_gap_jitter(&self) -> f32 {
        self.line_gap_jitter as f32
    }
    pub(super) fn line_start_jitter(&self) -> f32 {
        self.line_start_jitter as f32
    }
    /// Whether words may break round what text avoids, and how.
    pub(super) fn word_split(&self) -> Option<WordSplit> {
        self.split_words.map(|min_piece| WordSplit { min_piece, padding: self.split_padding as f32 })
    }
    /// How lines are built: by words (wrapping at blanks) or glyph by
    /// glyph, `words` unless the paragraph says.
    pub(super) fn line_mode(&self, caller: &str, words: bool) -> RhaiResult<TextBoxLineMode> {
        let words = self.layout.as_deref().map_or(words, |layout| layout == "words");
        match (words, self.word_split()) {
            (true, split) => Ok(TextBoxLineMode::WordsByBlankSymbol(split)),
            (false, None) => Ok(TextBoxLineMode::Flat),
            (false, Some(_)) => Err(format!("{}: 'split_words' needs layout: \"words\" (flat text is placed glyph by glyph already)", caller).into()),
        }
    }
}

impl Settings for ParagraphStyle {
    const NAME: &'static str = "paragraph_style";
    const KEYS: &'static [&'static str] = &[
        "line_height", "paragraph_gap", "indent", "align", "valign", "fit", "layout", "angle", "split_words", "split_padding", "clearance", "line_gap_jitter", "line_start_jitter",
    ];
    fn get(&self, key: &str) -> Dynamic {
        match key {
            "line_height" => self.line_height.into(),
            "paragraph_gap" => self.paragraph_gap.into(),
            "indent" => self.indent.into(),
            "align" => self.align.clone().into(),
            "valign" => self.valign.clone().into(),
            "fit" => self.fit.into(),
            "layout" => self.layout.clone().map_or(Dynamic::UNIT, Dynamic::from),
            "angle" => self.angle.map_or(Dynamic::UNIT, Dynamic::from),
            "split_words" => self.split_words.map_or(Dynamic::from(false), |n| Dynamic::from(n as i64)),
            "split_padding" => self.split_padding.into(),
            "clearance" => self.clearance.into(),
            "line_gap_jitter" => self.line_gap_jitter.into(),
            "line_start_jitter" => self.line_start_jitter.into(),
            _ => Dynamic::UNIT,
        }
    }
    fn set(&mut self, caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()> {
        match key {
            "line_height" => self.line_height = above_zero(caller, key, value)?,
            "paragraph_gap" => self.paragraph_gap = at_least_zero(caller, key, value)?,
            "indent" => self.indent = at_least_zero(caller, key, value)?,
            "align" => self.align = one_of(caller, key, value, &Align::NAMES)?,
            "valign" => self.valign = one_of(caller, key, value, &["top", "center", "bottom"])?,
            "fit" => self.fit = value.as_bool().map_err(|t| format!("{}: 'fit' should be true or false, got {}", caller, t))?,
            "layout" => self.layout = Some(one_of(caller, key, value, &["flat", "words"])?),
            "angle" => self.angle = Some(setting_number(caller, key, value)?),
            "split_words" => {
                self.split_words = match (value.as_bool(), value.as_int()) {
                    (Ok(on), _) => on.then_some(2),
                    (_, Ok(n)) if n >= 1 => Some(n as usize),
                    _ => return Err(format!("{}: 'split_words' should be true, false, or the fewest glyphs a piece may have (at least 1), got {}", caller, value).into()),
                }
            }
            "split_padding" => self.split_padding = at_least_zero(caller, key, value)?,
            "clearance" => self.clearance = at_least_zero(caller, key, value)?,
            "line_gap_jitter" => self.line_gap_jitter = at_least_zero(caller, key, value)?,
            "line_start_jitter" => self.line_start_jitter = at_least_zero(caller, key, value)?,
            _ => unreachable!("{} isn't a paragraph setting", key),
        }
        Ok(())
    }
    fn check(&self, caller: &str) -> RhaiResult<()> {
        if self.split_padding > 0.0 && self.split_words.is_none() {
            return Err(format!("{}: 'split_padding' needs 'split_words'", caller).into());
        }
        Ok(())
    }
}

/// What a text call is given: a style, a type and a paragraph, and the
/// call's own options.
pub(super) struct TextSettings {
    pub(super) style: Style,
    pub(super) type_: TypeStyle,
    pub(super) paragraph: ParagraphStyle,
    pub(super) options: Map,
}

/// The keys every text call takes, besides its own: a role's, so a role
/// (or a text style) can be passed straight in. `spans` is for marked-up
/// text, and ignored by calls that set plain text.
pub(super) const TEXT_KEYS: [&str; 4] = ["style", "type", "paragraph", "spans"];

impl TextSettings {
    /// `caller`'s settings: a style (default `default_style()`), a type
    /// and a paragraph, each whole or a map of settings, and the options
    /// in `own`.
    pub(super) fn read(caller: &str, mut settings: Map, own: &[&str]) -> RhaiResult<TextSettings> {
        let mut known: Vec<&str> = TEXT_KEYS.to_vec();
        known.extend(own);
        check_keys(caller, &settings, &known)?;
        settings.remove("spans");
        let style = match settings.remove("style") {
            None => Style::default_style(),
            Some(style) => Style::given(&format!("{} 'style'", caller), &style, &Style::default_style())?,
        };
        let type_ = match settings.remove("type") {
            None => TypeStyle::empty(),
            Some(t) => TypeStyle::given(&format!("{} 'type'", caller), &t, &TypeStyle::empty())?,
        };
        let paragraph = match settings.remove("paragraph") {
            None => ParagraphStyle::default(),
            Some(p) => ParagraphStyle::given(&format!("{} 'paragraph'", caller), &p, &ParagraphStyle::default())?,
        };
        Ok(TextSettings { style, type_, paragraph, options: settings })
    }
    /// How far placed glyphs stray, as the style's hand sets them.
    pub(super) fn jitter(&self) -> Jitter {
        Jitter::of(&self.style)
    }
}

/// Each placed glyph set a little off true.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Jitter {
    /// Across and down, in canvas units.
    position: (f64, f64),
    rotation: f64,
    scale: f64,
}

impl Jitter {
    /// The jitter `style`'s hand sets glyphs with.
    pub(super) fn of(style: &Style) -> Jitter {
        let hand = style.handwriting();
        Jitter { position: hand.glyph_position_jitter(), rotation: hand.glyph_rotation_jitter(), scale: hand.glyph_scale_jitter() }
    }
    /// Just the position jitter.
    pub(super) fn position_only(&self) -> Jitter {
        Jitter { position: self.position, ..Jitter::default() }
    }
    pub fn is_none(&self) -> bool {
        *self == Jitter::default()
    }
    /// Jitter placed glyphs: for each, across, down, the turn and the
    /// scale, in that order. Nothing is drawn from the generator for a
    /// jitter of 0.
    pub fn apply(&self, glyphs: &mut [Symbol], rng: &mut dyn RngCore) {
        if self.is_none() {
            return;
        }
        let mut either_way = |amount: f64| if amount > 0.0 { rng.random_range(-amount..amount) } else { 0.0 };
        for glyph in glyphs {
            let x = either_way(self.position.0);
            let y = either_way(self.position.1);
            glyph.translate_by(parry2d::math::Vec2::new(x as f32, y as f32));
            glyph.rotate_rad_by(either_way(self.rotation) as f32);
            glyph.scale_by((1.0 + either_way(self.scale)) as f32);
        }
    }
}

pub(super) fn register(engine: &mut rhai::Engine) {
    engine.register_type_with_name::<TypeStyle>("TypeStyle");
    super::options::register_settings::<TypeStyle>(engine);
    let new_type_style = |glyphset: Font, glyph_scale: Dynamic| -> RhaiResult<TypeStyle> {
        let mut style = TypeStyle::empty();
        style.set("new_type_style", "glyphset", &Dynamic::from(glyphset))?;
        style.set("new_type_style", "glyph_scale", &glyph_scale)?;
        Ok(style)
    };
    engine.register_fn("new_type_style", new_type_style);
    engine.register_fn("new_type_style", move |glyphset: Font, glyph_scale: Dynamic, options: Map| {
        new_type_style(glyphset, glyph_scale)?.with("new_type_style", &options)
    });

    engine.register_type_with_name::<ParagraphStyle>("ParagraphStyle");
    super::options::register_settings::<ParagraphStyle>(engine);
    engine.register_fn("new_paragraph_style", ParagraphStyle::default);
    engine.register_fn("new_paragraph_style", |options: Map| ParagraphStyle::default().with("new_paragraph_style", &options));
}

/// How far apart, in sizes, a wandering line's turns are.
const WANDER_STEP: f32 = 3.0;

/// A line that wanders smoothly up and down from straight: a random
/// offset every `WANDER_STEP` sizes along it (none at its start, so it
/// starts where it's put), joined by a Catmull-Rom spline.
pub(super) struct Wander {
    step: f32,
    offsets: Vec<f32>,
}

impl Wander {
    /// A line `length` long, straying up to `amount` canvas units either
    /// way, for text set at `size`. Nothing is drawn from the generator
    /// for an amount of 0.
    pub fn new(amount: f32, size: f32, length: f32, rng: &mut dyn RngCore) -> Wander {
        let step = WANDER_STEP * size;
        let knots = (length / step).ceil().max(1.0) as usize + 1;
        let offsets = (0..knots)
            .map(|k| if k == 0 || amount == 0.0 { 0.0 } else { rng.random_range(-amount..amount) })
            .collect();
        Wander { step, offsets }
    }
    /// The offset at knot `k`, held level past the ends.
    fn at(&self, k: isize) -> f32 {
        self.offsets[k.clamp(0, self.offsets.len() as isize - 1) as usize]
    }
    /// The spline's slope at knot `k`, in offset per knot.
    fn tangent(&self, k: isize) -> f32 {
        (self.at(k + 1) - self.at(k - 1)) / 2.0
    }
    /// Knot `k` and how far on from it `x` (along the line) is, from 0 to 1.
    fn segment(&self, x: f32) -> (isize, f32) {
        let last = self.offsets.len() as isize - 2;
        let k = ((x / self.step).floor() as isize).clamp(0, last.max(0));
        (k, (x / self.step - k as f32).clamp(0.0, 1.0))
    }
    /// How far the line is from straight `x` along it (down the page positive).
    pub fn offset(&self, x: f32) -> f32 {
        let (k, t) = self.segment(x);
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * self.at(k) + (t3 - 2.0 * t2 + t) * self.tangent(k)
            + (-2.0 * t3 + 3.0 * t2) * self.at(k + 1) + (t3 - t2) * self.tangent(k + 1)
    }
    /// The way the line runs `x` along it, in radians.
    pub fn angle(&self, x: f32) -> f32 {
        let (k, t) = self.segment(x);
        let t2 = t * t;
        let per_knot = (6.0 * t2 - 6.0 * t) * self.at(k) + (3.0 * t2 - 4.0 * t + 1.0) * self.tangent(k)
            + (-6.0 * t2 + 6.0 * t) * self.at(k + 1) + (3.0 * t2 - 2.0 * t) * self.tangent(k + 1);
        (per_knot / self.step).atan()
    }
    /// The line from `start` to the right, as a path: a cubic between
    /// each pair of knots, the same curve `offset` follows.
    pub fn path(&self, start: Vec2) -> StrokePath {
        let mut path = StrokePath::new().move_to(start.x, start.y + self.at(0));
        for k in 0..self.offsets.len() as isize - 1 {
            let x = start.x + k as f32 * self.step;
            let (c1, c2) = (self.at(k) + self.tangent(k) / 3.0, self.at(k + 1) - self.tangent(k + 1) / 3.0);
            path = path.cubic_curve_to(
                x + self.step / 3.0, start.y + c1,
                x + self.step * 2.0 / 3.0, start.y + c2,
                x + self.step, start.y + self.at(k + 1),
            );
        }
        path
    }
    /// Glyphs set on a straight line from `left` moved onto this one:
    /// each moved by the offset under its centre and turned to the way
    /// the line runs there.
    pub fn bend(&self, glyphs: &mut [Symbol], left: f32) {
        for glyph in glyphs {
            let x = glyph.position().x - left;
            glyph.translate_by(Vec2::new(0.0, self.offset(x)));
            glyph.rotate_rad_by(self.angle(x));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn a_wander_starts_on_the_line_stays_near_it_and_matches_its_path() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let wander = Wander::new(5.0, 10.0, 200.0, &mut rng);
        assert_eq!(wander.offset(0.0), 0.0);
        let xs: Vec<f32> = (0..=200).map(|x| x as f32).collect();
        // Catmull-Rom overshoots its knots a little, never by half again.
        assert!(xs.iter().all(|&x| wander.offset(x).abs() <= 5.0 * 1.5), "strayed too far");
        assert!(xs.iter().any(|&x| wander.offset(x).abs() > 0.5), "didn't wander");
        // The path is the same curve, and runs the way `angle` says.
        for (points, _) in wander.path(Vec2::new(0.0, 0.0)).flattened(0.01) {
            for p in points.iter().filter(|p| p.x <= 200.0) {
                assert!((p.y - wander.offset(p.x)).abs() < 0.05, "path {:?} off the curve at {}", p, wander.offset(p.x));
            }
        }
        let x = 37.0;
        let rise = (wander.offset(x + 0.01) - wander.offset(x - 0.01)) / 0.02;
        assert!((wander.angle(x) - rise.atan()).abs() < 1e-2);
    }

    #[test]
    fn no_wander_draws_nothing() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let wander = Wander::new(0.0, 10.0, 200.0, &mut rng);
        assert!((0..200).all(|x| wander.offset(x as f32) == 0.0));
        let next: u64 = rng.random();
        assert_eq!(next, rand::rngs::StdRng::seed_from_u64(0).random::<u64>());
    }
}
