//! Images for rhai scripts: pictures a journal keeps in `images/`, and
//! paint that takes its colours from one, a colour to each stroke.
//!
//! ```rhai
//! let sky = image("sunset");          // images/sunset.png (or .jpg)
//! sky.width; sky.height;              // its size in pixels
//! // Laid over the canvas, its top-left corner at `at`, `width` across
//! // (and `height` down; give one and it keeps its shape). Each stroke
//! // drawn in it (each subpath of a path) is all one colour: the
//! // picture's where the stroke is halfway along.
//! let paint = new_image_paint(sky, #{ at: vec2(100, 100), width: 800 });
//! let style = new_style(new_brush_style(paint, "Ink Pen", 3), steady);
//! // Keeping clear of the picture's parts in a colour (or colours), or
//! // near enough to it (in OKLab; 0.1 unless given): text drawn in it
//! // wraps round them, as round what it's told to avoid.
//! let round_the_sun = new_image_paint(sky, #{ width: 800, avoid: "#ffe9a0", avoid_tolerance: 0.15 });
//! avoid.add(round_the_sun, #{ margin: 4 });   // an avoidance tree takes it too
//! ```

use image::RgbaImage;
use rhai::{Engine, EvalAltResult, Map};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use super::options::{check_keys, number};
use crate::journal::canvas::parse_color;
use crate::journal::image_paint::DEFAULT_AVOID_TOLERANCE;
use super::resources::Resources;
use super::{RhaiResult, Vec2};
use crate::journal::image_paint::ImagePaint;

/// A picture from a journal's `images/`.
#[derive(Clone, Debug)]
pub struct Image {
    name: String,
    pixels: Arc<RgbaImage>,
}

/// The picture called `name`, `images/NAME.png` or `.jpg`, the journal's
/// own or the library's, decoded once and kept.
fn load(resources: &Resources, cache: &RefCell<HashMap<String, Image>>, name: &str) -> RhaiResult<Image> {
    if let Some(image) = cache.borrow().get(name) {
        return Ok(image.clone());
    }
    let images = match resources.image_files(name).as_slice() {
        [] => return Err(format!("image: no image named '{}': it'd be {} or {}", name, resources.png_images.path(name).display(), resources.jpg_images.path(name).display()).into()),
        [one] => (*one).clone(),
        _ => return Err(format!("image: '{}' is ambiguous: there's both {}.png and {}.jpg", name, name, name).into()),
    };
    let path = images.path(name);
    let bytes = images.read_bytes(name).map_err(|e| -> Box<EvalAltResult> { format!("image: {}: {}", path.display(), e).into() })?;
    let pixels = ::image::load_from_memory(&bytes)
        .map_err(|e| -> Box<EvalAltResult> { format!("image: {}: {}", path.display(), e).into() })?
        .to_rgba8();
    let image = Image { name: name.to_string(), pixels: Arc::new(pixels) };
    cache.borrow_mut().insert(name.to_string(), image.clone());
    Ok(image)
}

const PAINT: &str = "new_image_paint";

/// `new_image_paint(image[, #{ at, width, height, avoid, avoid_tolerance }])`.
fn new_image_paint(image: Image, options: Map) -> RhaiResult<ImagePaint> {
    check_keys(PAINT, &options, &["at", "width", "height", "avoid", "avoid_tolerance"])?;
    let at = match options.get("at") {
        None => parry2d::math::Vec2::ZERO,
        Some(v) => v.clone().try_cast::<Vec2>().map(Into::into)
            .ok_or_else(|| -> Box<EvalAltResult> { format!("{}: 'at' should be a vec", PAINT).into() })?,
    };
    let (w, h) = (image.pixels.width() as f32, image.pixels.height() as f32);
    if w == 0.0 || h == 0.0 {
        return Err(format!("{}: image '{}' has no pixels", PAINT, image.name).into());
    }
    let size = match (number(PAINT, &options, "width")?, number(PAINT, &options, "height")?) {
        (None, None) => parry2d::math::Vec2::new(w, h),
        (Some(width), None) => parry2d::math::Vec2::new(width, width * h / w),
        (None, Some(height)) => parry2d::math::Vec2::new(height * w / h, height),
        (Some(width), Some(height)) => parry2d::math::Vec2::new(width, height),
    };
    if !(size.x > 0.0 && size.y > 0.0 && size.is_finite()) {
        return Err(format!("{}: 'width' and 'height' must be positive, got {} by {}", PAINT, size.x, size.y).into());
    }
    let avoid = match options.get("avoid") {
        None => Vec::new(),
        Some(given) => {
            let colours = if given.is_array() { given.clone().cast::<rhai::Array>() } else { vec![given.clone()] };
            colours.into_iter().map(|c| {
                let text = c.clone().into_immutable_string()
                    .map_err(|t| -> Box<EvalAltResult> { format!("{}: 'avoid' should be a colour or an array of them, got {}", PAINT, t).into() })?;
                let [r, g, b, _] = parse_color(&text).map_err(|e| -> Box<EvalAltResult> { format!("{}: 'avoid': {}", PAINT, e).into() })?;
                Ok([r, g, b])
            }).collect::<RhaiResult<Vec<_>>>()?
        }
    };
    let tolerance = match number(PAINT, &options, "avoid_tolerance")? {
        None => DEFAULT_AVOID_TOLERANCE,
        Some(t) if t >= 0.0 && t.is_finite() => t,
        Some(t) => return Err(format!("{}: 'avoid_tolerance' must be a number of at least 0, got {}", PAINT, t).into()),
    };
    Ok(ImagePaint::new(image.name, image.pixels, at, size).avoiding(avoid, tolerance))
}

pub(super) fn register(engine: &mut Engine, resources: Resources) {
    let cache: Rc<RefCell<HashMap<String, Image>>> = Rc::default();
    engine.register_type_with_name::<Image>("Image")
        .register_fn("image", move |name: &str| load(&resources, &cache, name))
        .register_get("name", |i: &mut Image| i.name.clone())
        .register_get("width", |i: &mut Image| i.pixels.width() as i64)
        .register_get("height", |i: &mut Image| i.pixels.height() as i64)
        .register_fn("new_image_paint", |image: Image| new_image_paint(image, Map::new()))
        .register_fn("new_image_paint", new_image_paint)
        .register_fn("to_string", |i: &mut Image| format!("image '{}' ({} x {})", i.name, i.pixels.width(), i.pixels.height()))
        .register_fn("to_debug", |i: &mut Image| format!("image '{}' ({} x {})", i.name, i.pixels.width(), i.pixels.height()));
    engine.register_type_with_name::<ImagePaint>("ImagePaint")
        .register_fn("to_string", describe)
        .register_fn("to_debug", describe);
}

fn describe(p: &mut ImagePaint) -> String {
    let avoiding = if p.avoid.is_empty() { String::new() } else {
        let colours: Vec<String> = p.avoid.iter().map(|[r, g, b]| crate::journal::canvas::css([*r, *g, *b, 1.0])).collect();
        format!(", avoiding {} within {}", colours.join(" and "), p.avoid_tolerance)
    };
    format!("image paint of '{}' at ({}, {}), {} by {}{}", p.name, p.at.x, p.at.y, p.size.x, p.size.y, avoiding)
}

#[cfg(test)]
mod tests {
    use super::super::{JournalEngine, Resources};
    use crate::journal::fonts::{FontBook, DEFAULT_GLYPHSETS};
    use crate::journal::rhaientries::sources::base64;
    use std::rc::Rc;

    /// A picture `w` by `h`, red on its left half and blue on its right,
    /// as a data URL of `format`.
    fn two_tone(w: u32, h: u32, format: image::ImageFormat) -> String {
        let pixels = image::RgbImage::from_fn(w, h, |x, _| if x < w / 2 { image::Rgb([255, 0, 0]) } else { image::Rgb([0, 0, 255]) });
        let mut bytes = std::io::Cursor::new(Vec::new());
        pixels.write_to(&mut bytes, format).unwrap();
        let mime = if format == image::ImageFormat::Png { "image/png" } else { "image/jpeg" };
        format!("data:{};base64,{}", mime, base64(bytes.get_ref()))
    }

    fn engine() -> JournalEngine {
        let mut resources = Resources::library();
        let png = two_tone(40, 20, image::ImageFormat::Png);
        resources.png_images = resources.png_images.with_overlay([("flag".to_string(), png.clone()), ("both".to_string(), png), ("broken".to_string(), "data:image/png;base64,AAAA".to_string())]);
        let jpg = two_tone(30, 60, image::ImageFormat::Jpeg);
        resources.jpg_images = resources.jpg_images.with_overlay([("photo".to_string(), jpg.clone()), ("both".to_string(), jpg)]);
        JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources)
    }

    #[test]
    fn images_are_read_and_laid_out() {
        let result = engine().build_entry_str(r#"
            let flag = image("flag");
            assert(flag.name == "flag" && flag.width == 40 && flag.height == 20, "a png's size");
            let photo = image("photo");
            assert(photo.width == 30 && photo.height == 60, "a jpg's size");
            assert(`${flag}` == "image 'flag' (40 x 20)", "as text");
            // Laid out at its size in pixels, or as wide or tall as asked,
            // keeping its shape unless given both.
            assert(`${new_image_paint(flag)}` == "image paint of 'flag' at (0, 0), 40 by 20", "its own size");
            assert(`${new_image_paint(flag, #{ at: vec2(5, 6), width: 100 })}` == "image paint of 'flag' at (5, 6), 100 by 50", "width");
            assert(`${new_image_paint(photo, #{ height: 30 })}` == "image paint of 'photo' at (0, 0), 15 by 30", "height");
            assert(`${new_image_paint(flag, #{ width: 10, height: 70 })}` == "image paint of 'flag' at (0, 0), 10 by 70", "both");
            // In place of a colour.
            let paint = new_image_paint(flag);
            let style = new_style(new_brush_style(paint, "Ink Pen", 1.6), new_handwriting_style());
            assert(type_of(style.color) == "ImagePaint", "read back");
            style.color = "gold";
            style.color = paint;
            let plain = text_style("plain_hand", #{ color: paint });
            assert(type_of(plain.style.color) == "ImagePaint", "a colour option takes it");
            // Its parts in a colour kept clear of: the flag's red left half,
            // laid 0-200 across and 0-100 down.
            let red_half = new_image_paint(flag, #{ width: 200, avoid: "red" });
            assert(`${red_half}` == "image paint of 'flag' at (0, 0), 200 by 100, avoiding rgba(255, 0, 0, 1) within 0.1", "as text");
            let tree = new_avoidance_tree().add(red_half);
            assert(tree.blocks(vec2(50, 50)) && !tree.blocks(vec2(150, 50)), "a tree takes it");
            let both = new_image_paint(flag, #{ width: 200, avoid: ["red", "blue"], avoid_tolerance: 0.2 });
            assert(new_avoidance_tree().add(both, #{ margin: 1 }).blocks(vec2(150, 50)), "colours, with a margin");
            // Text in it wraps round them by itself: a box over the whole
            // flag only sets text on its blue right half.
            let pen = new_style(new_brush_style(red_half, "Ink Pen", 1.0), new_handwriting_style());
            let words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor";
            let box = new_textbox(#{ center: vec2(100, 50), width: 200, height: 100, type: #{ glyph_scale: 8 }, style: pen });
            let placed = box.fill(glyphset("plain").glyphs(words)).placed;
            assert(placed.len() > 10, "some set");
            let b = bounds(new_canvas().add(placed, pen));
            assert(b.min.x >= 100, `all on the right half: ${b.min.x}`);
            let unpainted = new_textbox(#{ center: vec2(100, 50), width: 200, height: 100, type: #{ glyph_scale: 8 } });
            assert(bounds(new_canvas().add(unpainted.fill(glyphset("plain").glyphs(words)).placed, pen)).min.x < 50, "otherwise from the left");
        "#);
        assert!(result.is_ok(), "{:?}", result.err());
        for (script, want) in [
            (r#"image("nope")"#, "image: no image named 'nope': it'd be "),
            (r#"image("both")"#, "image: 'both' is ambiguous: there's both both.png and both.jpg"),
            (r#"image("broken")"#, "image: "),
            (r#"new_image_paint(image("flag"), #{ size: 3 })"#, "unknown option 'size'"),
            (r#"new_image_paint(image("flag"), #{ width: 0 })"#, "'width' and 'height' must be positive"),
            (r#"new_image_paint(image("flag"), #{ at: [1, 2] })"#, "'at' should be a vec"),
            (r#"new_image_paint(image("flag"), #{ avoid: 3 })"#, "'avoid' should be a colour or an array of them"),
            (r#"new_image_paint(image("flag"), #{ avoid: ["red", "nope"] })"#, "'avoid': Invalid color 'nope'"),
            (r#"new_image_paint(image("flag"), #{ avoid: "red", avoid_tolerance: -1 })"#, "'avoid_tolerance' must be a number of at least 0"),
            (r#"new_brush_style(3, "Ink Pen", 1.6)"#, "expected a colour string, a gradient or image paint"),
        ] {
            let err = engine().build_entry_str(script).expect_err(script);
            assert!(err.contains(want), "{}\n  wanted: {}\n  got: {}", script, want, err);
        }
    }

    #[test]
    fn images_are_read_from_disk() {
        let dir = std::env::temp_dir().join(format!("scribe-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        image::RgbaImage::from_pixel(3, 5, image::Rgba([0, 255, 0, 255])).save(dir.join("images/green.png")).unwrap();
        let resources = Resources::dir(&dir).over(Resources::library());
        assert!(resources.png_images.read("green").unwrap().starts_with("data:image/png;base64,"), "read as a data URL");
        let mut engine = JournalEngine::with_resources(Rc::new(FontBook::from_file(DEFAULT_GLYPHSETS).unwrap()), resources);
        let result = engine.build_entry_str(r#"let g = image("green"); assert(g.width == 3 && g.height == 5, "its size");"#);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "{:?}", result.err());
    }
}
