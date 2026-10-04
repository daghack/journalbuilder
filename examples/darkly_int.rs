use scribe::journal::symboldict;
use scribe::journal::canvas::{Canvas, StrokeStyle};
use parry2d::math::Vec2;
use scribe::journal::darkly;


pub fn main() {
    let symd = symboldict::SymbolPathDict::from_file("config/symbols.json").expect("Failed to load symbols.json");
    let mut eye_path = symd.new_symbol("archangel_5").expect("Unable to retrieve symbol").placed_strokepath();
    eye_path.translate_by(Vec2::new(0.5, 0.5));
    eye_path.scale_by(1000.0);
    let mut canvas = Canvas::new();
    canvas.add_strokes(eye_path, StrokeStyle{ width: 0.0, color: "".into() });

    let mut engine = darkly::new_engine(1920, 1080);
    let buff = darkly::render_canvas(&canvas, &mut engine, "Ink Pen", 10.0, (0.0, 0.25));
    buff.save("darkly_output.png")
        .expect("Could not save png.");
}
