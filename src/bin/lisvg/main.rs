use kdtree::KdTree;
use parry2d::math::{Point, Vector};
use scribe::journal::canvas::{CanvasColorMap, Canvas, StrokeStyle, CanvasShape};
use scribe::journal::canvas::textbox::{TextBox, TextBoxOptions};
use scribe::journal::{styles, symboldict};

fn lorem_ipsum(word_count: usize) -> String {
    let lorem = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua ut enim ad minim veniam quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum. sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium totam rem aperiam eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt neque porro quisquam est qui dolorem ipsum quia dolor sit amet consectetur adipisci velit sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem ut enim ad minima veniam quis nostrum exercitationem ullam corporis suscipit laboriosam nisi ut aliquid ex ea commodi consequatur quis autem vel eum iure reprehenderit qui in ea voluptate velit esse quam nihil molestiae consequatur vel illum qui dolorem eum fugiat quo voluptas nulla pariatur".split_whitespace().collect::<Vec<&str>>();
    let mut words: Vec<&str> = lorem.clone();
    while words.len() < word_count {
        words.extend_from_slice(&lorem);
    }
    words.truncate(word_count);
    words.join(" ")
}

fn main() {
    // take the first argument as the svg file path we intend to read.
    // take the second argument as the destination svg file path.
    // If no argument is given, print usage and exit.
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_svg_file> <output_svg_file>", args[0]);
        std::process::exit(1);
    }
    let symd = symboldict::SymbolPathDict::from_file("config/symbols.json").expect("Failed to load symbols.json");

    let svg_file_path = &args[1];
    let output_svg_file = &args[2];
    let svg_data = std::fs::read_to_string(svg_file_path)
        .expect("Failed to read SVG file")
        .into_bytes();

    //parse the svg data, and then retrieve the width and height.
    let svg_tree = usvg::Tree::from_data(&svg_data, &usvg::Options::default()).expect("Failed to parse SVG");
    let svg_size = svg_tree.size();
    let width = svg_size.width();
    let height = svg_size.height();
    println!("SVG Width: {}, Height: {}", width, height);

    let textbox_width = width as f32;
    let textbox_height = height as f32;
    let symbol_scale = 10.0;
    let sample_spacing = 5.0;
    let text_bounds = parry2d::bounding_volume::Aabb::new(
        Point::new(-textbox_width / 2.0, -textbox_height / 2.0),
        Point::new(textbox_width / 2.0, textbox_height / 2.0),
    );
    let color_tree = CanvasColorMap::SvgFile(svg_file_path.clone())
        .into_kdtree(sample_spacing, textbox_width, textbox_height);
    //let svg_data = random_eyes_canvas(5, 500.0, symd).to_svg(None);
    //let color_tree = CanvasColorMap::SvgBytes(svg_data.into_bytes())
    //    .into_kdtree(sample_spacing, textbox_width, textbox_height);
    let mut avoid_tree = KdTree::new(2);
    let iter_nearest = color_tree.iter_nearest(&[0.0, 0.0], &kdtree::distance::squared_euclidean).expect("Failed to create color map iterator");
    for (_, (color, point)) in iter_nearest {
        if color.alpha() > 0 {
            continue;
        }
        let point = [point.x + text_bounds.mins.x, point.y + text_bounds.mins.y];
        avoid_tree.add(point, (point[0], point[1])).expect("Failed to add point to KdTree");
    }
    let textbox_opts = TextBoxOptions {
        bounds: text_bounds,
        symbol_scale: symbol_scale,
        avoid_tree: Some(&avoid_tree),
        avoid_scale: symbol_scale / 2.0,
        expected_stroke_width: Some(1.0),
    };
    let textbox = TextBox::new(textbox_opts);
    let builder = styles::lowercase_builder(&symd, "light_lower");
    let lorem = lorem_ipsum(6000);
    let msg = builder.build(lorem.as_str());
    let mut msg_iter = msg.iter().peekable();
    let symbols = textbox.fill_bounds_with(&mut msg_iter);
    let mut canvas = Canvas::new();

    let min_vec : Vector<f32> = Vector::new(
        text_bounds.mins.x,
        text_bounds.mins.y,
    );
    let default_color = "rgba(255, 255, 255, 1.0)".to_string();
        

    for symbol in symbols {
        let mut avg_color = (0.0, 0.0, 0.0, 0.0);
        let symbol_pos = symbol.position() - min_vec;
        let query_point = [symbol_pos.x, symbol_pos.y];
        let nearest = color_tree.nearest(&query_point, 10, &kdtree::distance::squared_euclidean);
        let mut count = 0;
        match nearest {
            Ok(results) => {
                for (_, (color, _)) in results {
                    let alpha = color.alpha();
                    if alpha < 1 {
                        continue;
                    }
                    count += 1;
                    avg_color.0 += color.red() as f32;
                    avg_color.1 += color.green() as f32;
                    avg_color.2 += color.blue() as f32;
                    avg_color.3 += color.alpha() as f32;
                }
            }
            Err(err) => {
                println!("Color lookup error: {}", err);
            }
        }
        let fill_color = if count == 0 {
            default_color.clone()
        } else {
            let color = tiny_skia::ColorU8::from_rgba(
                (avg_color.0 / count as f32) as u8,
                (avg_color.1 / count as f32) as u8,
                (avg_color.2 / count as f32) as u8,
                (avg_color.3 / count as f32) as u8,
            );
            let color = color.premultiply();
            format!("rgba({}, {}, {}, {:.3})", color.red(), color.green(), color.blue(), color.alpha() as f32 / 255.0)};

        let stroke_style = StrokeStyle {
            color: fill_color,
            width: 2.0,
        };
        canvas.add_strokes(symbol, stroke_style);
    }
    let border_color = StrokeStyle {
        color: "black".to_string(),
        width: 2.0,
    };
    let border = CanvasShape::Rectangle {
        width: textbox_width,
        height: textbox_height,
        center: Point::new(0.0, 0.0),
    };
    canvas.add_strokes(border, border_color);
    canvas.save_to_svg(output_svg_file.into(), None);
}
