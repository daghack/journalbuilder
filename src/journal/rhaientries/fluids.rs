//! Fluids for rhai scripts: a frozen fluid a journal keeps in `fluids/`,
//! made in the editor's Fluid tab, distorting what a page or a canvas
//! draws (see `journal::fluid`).
//!
//! ```rhai
//! let f = fluid("ripples");            // fluids/ripples.json
//! // Laid over another box, and with an effect changed or turned off:
//! let f = fluid("ripples", #{ at: vec2(0, 0), size: vec2(800, 600),
//!                             weight_distort: #{ to: 3.0 } });
//! f.color_distort = ();
//! page.add(f);                         // every canvas on the page
//! canvas.add(f);                       // just this canvas
//! f.sample("speed", vec2(120, 80));    // a channel's value there
//! f.velocity(vec2(120, 80));           // the flow there, a vec
//! ```

use rhai::{Dynamic, Engine, EvalAltResult, Map, NativeCallContext};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::options::apply_props;
use super::resources::Resources;
use super::{RhaiResult, Vec2};
use crate::journal::fluid::{Effects, Fluid, FluidChannel, EFFECTS};

/// The properties `fluid`'s options can set.
const PROPS: [&str; 7] = ["at", "size", "position_distort", "jitter_distort", "weight_distort", "opacity_distort", "color_distort"];

/// The fluid called `name`, `fluids/NAME.json`, the journal's own or the
/// library's, decoded once and kept.
fn load(resources: &Resources, cache: &RefCell<HashMap<String, Fluid>>, name: &str) -> RhaiResult<Fluid> {
    if let Some(fluid) = cache.borrow().get(name) {
        return Ok(fluid.clone());
    }
    if !resources.fluids.has(name) {
        return Err(format!("fluid: no fluid named '{}': it'd be {}; make one in the editor's Fluid tab", name, resources.fluids.path(name).display()).into());
    }
    let text = resources.fluids.read(name)
        .map_err(|e| -> Box<EvalAltResult> { format!("fluid: {}: {}", resources.fluids.path(name).display(), e).into() })?;
    let fluid = Fluid::from_file(name, &text).map_err(|e| -> Box<EvalAltResult> { format!("fluid: {}", e).into() })?;
    cache.borrow_mut().insert(name.to_string(), fluid.clone());
    Ok(fluid)
}

/// A script's value as JSON, for an effect's settings: numbers, strings,
/// booleans, `()` as null, arrays, maps, and vecs as `[x, y]`.
fn to_json(caller: &str, value: &Dynamic) -> RhaiResult<Value> {
    if value.is_unit() {
        return Ok(Value::Null);
    }
    if let Ok(b) = value.as_bool() {
        return Ok(Value::Bool(b));
    }
    if let Ok(i) = value.as_int() {
        return Ok(Value::from(i));
    }
    if let Ok(f) = value.as_float() {
        return serde_json::Number::from_f64(f).map(Value::Number)
            .ok_or_else(|| format!("{}: {} isn't a number a setting can be", caller, f).into());
    }
    if value.is_string() {
        return Ok(Value::String(value.clone().into_string().expect("it's a string")));
    }
    if let Some(v) = value.clone().try_cast::<Vec2>() {
        return Ok(serde_json::json!([v.x, v.y]));
    }
    if value.is_array() {
        let items = value.clone().cast::<rhai::Array>();
        return items.iter().map(|item| to_json(caller, item)).collect::<RhaiResult<Vec<_>>>().map(Value::Array);
    }
    if value.is_map() {
        let map = value.clone().cast::<Map>();
        let mut out = serde_json::Map::new();
        for (key, item) in &map {
            out.insert(key.to_string(), to_json(caller, item)?);
        }
        return Ok(Value::Object(out));
    }
    Err(format!("{}: a {} can't be a setting", caller, value.type_name()).into())
}

/// JSON as a script's value: numbers as floats (integers as integers),
/// arrays and maps.
fn from_json(value: &Value) -> Dynamic {
    match value {
        Value::Null => Dynamic::UNIT,
        Value::Bool(b) => Dynamic::from(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Dynamic::from(i),
            None => Dynamic::from(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => Dynamic::from(s.clone()),
        Value::Array(items) => Dynamic::from_array(items.iter().map(from_json).collect()),
        Value::Object(map) => Dynamic::from_map(map.iter().map(|(k, v)| (k.as_str().into(), from_json(v))).collect()),
    }
}

/// The effects as JSON, numbers in their shortest form (so 0.3 reads back
/// as 0.3, not 0.30000001192092896).
fn effects_json(effects: &Effects) -> Value {
    serde_json::from_str(&serde_json::to_string(effects).expect("effects serialise")).expect("their JSON parses")
}

fn get_effect(fluid: &mut Fluid, key: &str) -> Dynamic {
    from_json(&effects_json(&fluid.effects)[key])
}

/// `fluid.KEY = #{ ... }`: the settings given over those it has (or the
/// effect's defaults, if it's off), or `()` to turn it off.
fn set_effect(fluid: &mut Fluid, key: &str, value: Dynamic) -> RhaiResult<()> {
    let caller = format!("fluid.{}", key);
    let mut json = effects_json(&fluid.effects);
    if value.is_unit() {
        json[key] = Value::Null;
    } else {
        let Value::Object(given) = to_json(&caller, &value)? else {
            return Err(format!("{}: expected a map of settings or (), got {}", caller, value.type_name()).into());
        };
        let mut settings = match json[key].take() {
            Value::Object(settings) => settings,
            // Off: the effect's defaults, which a map with nothing in it gives.
            _ => {
                let mut empty = serde_json::Map::new();
                empty.insert(key.to_string(), Value::Object(serde_json::Map::new()));
                let defaults: Effects = serde_json::from_value(Value::Object(empty)).expect("every effect has defaults");
                match effects_json(&defaults)[key].take() {
                    Value::Object(settings) => settings,
                    _ => unreachable!("an effect given is on"),
                }
            }
        };
        settings.extend(given);
        json[key] = Value::Object(settings);
    }
    let effects: Effects = serde_json::from_value(json).map_err(|e| -> Box<EvalAltResult> { format!("{}: {}", caller, e).into() })?;
    let checked = Fluid { effects, ..fluid.clone() };
    checked.check().map_err(|e| -> Box<EvalAltResult> { format!("{}: {}", caller, e).into() })?;
    *fluid = checked;
    Ok(())
}

fn vec_of(caller: &str, value: Dynamic) -> RhaiResult<parry2d::math::Vec2> {
    let type_name = value.type_name();
    let v = value.try_cast::<Vec2>().ok_or_else(|| -> Box<EvalAltResult> { format!("{}: expected a vec, got {}", caller, type_name).into() })?;
    let v: parry2d::math::Vec2 = v.into();
    if !v.is_finite() {
        return Err(format!("{}: must be finite", caller).into());
    }
    Ok(v)
}

fn channel(caller: &str, name: &str) -> RhaiResult<FluidChannel> {
    FluidChannel::from_name(name).ok_or_else(|| {
        let names: Vec<&str> = FluidChannel::ALL.iter().map(|c| c.name()).collect();
        format!("{}: no channel '{}': it's one of {}", caller, name, names.join(", ")).into()
    })
}

fn describe(f: &mut Fluid) -> String {
    let json = effects_json(&f.effects);
    let on: Vec<&str> = EFFECTS.iter().copied().filter(|key| !json[*key].is_null()).collect();
    format!("fluid '{}' over ({}, {}) {} by {}{}", f.name, f.at.x, f.at.y, f.size.x, f.size.y,
        if on.is_empty() { ", doing nothing".to_string() } else { format!(": {}", on.join(", ")) })
}

pub(super) fn register(engine: &mut Engine, resources: Resources) {
    let cache: Rc<RefCell<HashMap<String, Fluid>>> = Rc::default();
    let (r, c) = (resources.clone(), cache.clone());
    let ty = engine.register_type_with_name::<Fluid>("Fluid")
        .register_fn("fluid", move |name: &str| load(&r, &c, name))
        .register_fn("fluid", move |ctx: NativeCallContext, name: &str, options: Map| {
            let fluid = load(&resources, &cache, name)?;
            apply_props(&ctx, "fluid", fluid, options, &PROPS)
        })
        .register_get("name", |f: &mut Fluid| f.name.clone())
        .register_get_set("at",
            |f: &mut Fluid| Dynamic::from(Vec2::new(f.at.x as f64, f.at.y as f64)),
            |f: &mut Fluid, at: Dynamic| -> RhaiResult<()> {
                f.at = vec_of("fluid.at", at)?;
                Ok(())
            })
        .register_get_set("size",
            |f: &mut Fluid| Dynamic::from(Vec2::new(f.size.x as f64, f.size.y as f64)),
            |f: &mut Fluid, size: Dynamic| -> RhaiResult<()> {
                let size = vec_of("fluid.size", size)?;
                if !(size.x > 0.0 && size.y > 0.0) {
                    return Err(format!("fluid.size: width and height must be positive, got ({}, {})", size.x, size.y).into());
                }
                f.size = size;
                Ok(())
            })
        .register_fn("sample", |f: &mut Fluid, name: &str, point: Vec2| -> RhaiResult<f64> {
            Ok(f.sample(channel("fluid.sample", name)?, point.into()) as f64)
        })
        .register_fn("velocity", |f: &mut Fluid, point: Vec2| -> Vec2 {
            let v = f.velocity(point.into());
            Vec2::new(v.x as f64, v.y as f64)
        })
        .register_fn("to_string", describe)
        .register_fn("to_debug", describe);
    for key in EFFECTS {
        ty.register_get_set(key, move |f: &mut Fluid| get_effect(f, key), move |f: &mut Fluid, value: Dynamic| set_effect(f, key, value));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::journal::fonts::FontBook;
    use crate::journal::rhaientries::{JournalEngine, Resources};
    use std::rc::Rc;

    /// A journal with one fluid, `rightward`: a uniform flow of 10 units a
    /// second to the right over 100 × 50, its dye rising from left to
    /// right, distorting by `effects` (JSON members).
    pub(crate) fn engine_with(effects: &str) -> JournalEngine {
        let field = crate::journal::fluid::tests::rightward(10.0);
        let text = format!(r#"{{ "version": 1, "field": {}{}{} }}"#, serde_json::to_string(&field.encode()).unwrap(), if effects.is_empty() { "" } else { ", " }, effects);
        let mut resources = Resources::library();
        resources.fluids = resources.fluids.with_overlay([("rightward".to_string(), text)]);
        JournalEngine::with_resources(Rc::new(FontBook::from_file(crate::journal::fonts::DEFAULT_GLYPHSETS).unwrap()), resources)
    }

    #[test]
    fn fluids_load_and_take_their_settings() {
        let result = engine_with(r#""weight_distort": { "to": 3 }"#).build_entry_str(r#"
            let f = fluid("rightward");
            assert(f.name == "rightward", "its name");
            assert(f.weight_distort.to == 3.0 && f.weight_distort.from == 1.0, "the file's settings, defaults filling in");
            assert(f.position_distort == (), "off unless given");
            assert(`${f}` == "fluid 'rightward' over (0, 0) 100 by 50: weight_distort", `as text: ${f}`);
            let g = fluid("rightward", #{ position_distort: #{ amount: 0.3 }, at: vec2(5, 6), size: vec2(200, 100) });
            assert(g.position_distort.amount == 0.3 && g.position_distort.by == "advect", "turned on with its defaults");
            assert(g.at.x == 5.0 && g.size.y == 100.0, "laid over another box");
            g.weight_distort = #{ from: 2 };
            assert(g.weight_distort.from == 2.0 && g.weight_distort.to == 3.0, "settings merge");
            g.weight_distort = ();
            assert(g.weight_distort == (), "turned off");
            let speed = f.sample("speed", vec2(50, 25));
            assert(speed > 9.9 && speed < 10.1, `the speed there: ${speed}`);
            assert(f.velocity(vec2(50, 25)).x > 9.9, "the flow there");
            assert(f.sample("dye", vec2(500, 25)) == 0.0, "nothing outside its box");
        "#);
        assert!(result.is_ok(), "{:?}", result.err());
    }

    #[test]
    fn fluid_mistakes_read_clearly() {
        let mut engine = engine_with("");
        for (script, want) in [
            (r#"fluid("nowhere")"#, "no fluid named 'nowhere'"),
            (r#"fluid("rightward", #{ weight_distort: #{ too: 2 } })"#, "unknown field `too`"),
            (r#"fluid("rightward", #{ weight_distort: #{ channel: "sped" } })"#, "unknown variant `sped`"),
            (r#"fluid("rightward", #{ color_distort: #{ colors: ["nope"] } })"#, "nope"),
            (r#"fluid("rightward", #{ wobble: 1 })"#, "wobble"),
            (r#"fluid("rightward").sample("spin", vec2(0, 0))"#, "no channel 'spin': it's one of speed, vorticity, pressure, dye"),
            (r#"fluid("rightward", #{ size: vec2(0, 10) })"#, "must be positive"),
            (r#"fluid("rightward", #{ jitter_distort: #{ length: 0 } })"#, "length must be positive"),
        ] {
            let error = engine.build_entry_str(script).unwrap_err();
            assert!(error.contains(want), "{script}: {error}");
        }
    }

    /// The points of each canvas's first stroke, as page `index` renders it.
    fn rendered_points(entry: &crate::journal::rhaientries::Entry, index: usize) -> Vec<(String, Vec<parry2d::math::Vec2>)> {
        let (_, page) = entry.selected_pages(None).nth(index).unwrap();
        page.rendered_canvases().map(|(label, canvas)| (label, canvas.strokes()[0].path.points_within())).collect()
    }

    #[test]
    fn pages_and_canvases_are_distorted_by_their_fluids() {
        let entry = engine_with(r#""position_distort": { "by": "offset", "amount": 1 }"#).build_entry_str(r#"
            fn canvas(name) {
                let ink = new_style(new_brush_style("black", "Ink Pen", 1.0), new_handwriting_style());
                new_canvas(#{ name: name }).add(new_line(vec2(20, 10), vec2(30, 10)), ink)
            }
            // One canvas of two distorted, then the whole page.
            journal_entry.add(new_page(#{ name: "one" }).add(canvas("a").add(fluid("rightward"))).add(canvas("b")));
            journal_entry.add(new_page(#{ name: "all" }).add(canvas("a")).add(fluid("rightward")).add(canvas("b")));
            // A distorted canvas's strokes stay distorted on another canvas.
            journal_entry.add(new_page(#{ name: "carried" }).add(new_canvas().add(canvas("a").add(fluid("rightward")))));
            // Tiles take the page's fluids, and are cut after.
            let tiled = new_page(#{ name: "tiled" }).add(canvas("a")).add(fluid("rightward"));
            journal_entry.add(tiled.split(#{ columns: 2 }));
        "#).unwrap();
        let moved = |points: &[parry2d::math::Vec2]| (points[0].x - 30.0).abs() < 0.5 && points.len() > 2;
        let still = |points: &[parry2d::math::Vec2]| points == [parry2d::math::Vec2::new(20.0, 10.0), parry2d::math::Vec2::new(30.0, 10.0)];
        let one = rendered_points(&entry, 0);
        assert!(moved(&one[0].1) && still(&one[1].1), "{one:?}");
        let all = rendered_points(&entry, 1);
        assert!(moved(&all[0].1) && moved(&all[1].1), "{all:?}");
        assert!(moved(&rendered_points(&entry, 2)[0].1));
        assert!(moved(&rendered_points(&entry, 3)[0].1), "a tile's");
        let (_, page) = entry.selected_pages(None).nth(1).unwrap();
        assert_eq!(page.fluid_names().into_iter().collect::<Vec<_>>(), ["rightward"]);
    }

    #[test]
    fn a_fluid_weights_and_colours_svg_strokes() {
        let mut engine = engine_with(r##""weight_distort": { "channel": "dye", "from": 1, "to": 4 }, "color_distort": { "colors": ["#000000", "#ff0000"] }"##);
        let entry = engine.build_entry_str(r#"
            let ink = new_style(new_brush_style("black", "Ink Pen", 2.0), new_handwriting_style());
            journal_entry.add(new_page().add(new_canvas().add(new_line(vec2(5, 25), vec2(95, 25)), ink)).add(fluid("rightward")));
        "#).unwrap();
        let (_, page) = entry.selected_pages(None).next().unwrap();
        let (_, canvas) = page.rendered_canvases().next().unwrap();
        // Wider and redder to the right.
        let bounds = canvas.bounds();
        let width = canvas.strokes()[0].style.width;
        assert!(bounds.maxs.y - bounds.mins.y >= 2.0 * 4.0 * width * 0.99, "room for the heaviest: {bounds:?}");
        let svg = canvas.to_svg(None, 0);
        let reds: Vec<u32> = svg.split("stroke:rgba(").skip(1).map(|c| c.split(',').next().unwrap().parse().unwrap()).collect();
        assert!(reds[0] < 30 && *reds.last().unwrap() > 220, "black at the left, red at the right: {reds:?}");
        let widths: Vec<f32> = svg.split("stroke-width:").skip(1).map(|w| w.split(';').next().unwrap().parse().unwrap()).collect();
        assert!(widths.last().unwrap() > &(3.0 * widths.first().unwrap()), "{widths:?}");
        let styled = canvas.to_svg(Some(&crate::journal::canvas::SvgStyle::DEFAULT), 0);
        let reds: Vec<u32> = styled.split("fill=\"rgba(").skip(1).map(|c| c.split(',').next().unwrap().parse().unwrap()).collect();
        assert!(reds.len() > 10 && reds[0] < 30 && *reds.last().unwrap() > 220, "styled, in pieces: {reds:?}");
    }
}
