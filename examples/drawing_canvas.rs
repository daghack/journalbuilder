use scribe::journal::StrokePath;
use scribe::journal::symboldict;
use parry2d::math::{Point, Vector};
use scribe::journal::symbol::{Symbol, SymbolType, SymbolPlacementArgs};
use scribe::journal::canvas::{Canvas, CanvasShape, CanvasColorMap, StrokeStyle, CanvasStyleOptions, textbox::{TextBox, TextBoxOptions, RemapColorFunc, ColorMappedTextBoxOptions, ColorMappedTextBoxCollection}};
use scribe::journal::styles;

use noise::{NoiseFn, Perlin};

fn lorem_ipsum(word_count: usize) -> String {
    let lorem = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua ut enim ad minim veniam quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum. sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium totam rem aperiam eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt neque porro quisquam est qui dolorem ipsum quia dolor sit amet consectetur adipisci velit sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem ut enim ad minima veniam quis nostrum exercitationem ullam corporis suscipit laboriosam nisi ut aliquid ex ea commodi consequatur quis autem vel eum iure reprehenderit qui in ea voluptate velit esse quam nihil molestiae consequatur vel illum qui dolorem eum fugiat quo voluptas nulla pariatur".split_whitespace().collect::<Vec<&str>>();
    let mut words: Vec<&str> = lorem.clone();
    while words.len() < word_count {
        words.extend_from_slice(&lorem);
    }
    words.truncate(word_count);
    words.join(" ")
}

fn test_content() -> &'static str {
    "That brings us to styles of play. There is the Ebbnari Style, defined as an extremely surreal and alien narrative. It's the favorite playing style of the Wraiths in the Eastern Waste, as it captures the horrific confusion of death and the unknowing. There is the Farakadoza Style, defined as an extremely simplistic way of playing. The weave that is constructed resembles more of a parable, or child's story. Normally, this forces the opponent to adopt the same simplistic style, otherwise the weaving becomes frayed and entirely disjointed. This style, of course, was made famous by Farakadoza, the Pale, who was fond of teaching children the royal game. This style is adopted as either a tactic or as a way to have a light and whimsical match. There is the Fanari Style, defined as a purely epic style of play, where the characters involved all pass through the typical trials and tribulations of the hero. The Sinsara style is defined by personal development and the workings of the soul. The Woadlic style is defined as a purely comedic style of play. It's considered by most as the lowest form of the art, as it ignores fundamental lessons of the Rooted Way. As you may or may not know, it was made infamous by the yellow Harlequin, Woadlic Toadtongue, and his series of games called, Mother Pillows, and her Profoundly Indecent Maids, which all seemed to center around a town called Pillington, inhabited by all manner of raunchy characters. Lenard, the Licking Leper, Fancies Bumrider, etc. etc. Then there is Coltar, a style of play that is purely base and carnal. A popular style among poets, concubines, and prostitutes. Then Pindennix, a rigid style of play that focuses on court intrigue, and finally, Cathar, a style of play that focuses on poetry and song."
}

fn door_and_circle_canvas(symd: &symboldict::SymbolPathDict) -> Canvas {
    let mut canvas = Canvas::new();
    
    let teal_style = StrokeStyle {
        color: "rgba(0, 200, 200, 0.05)".into(),
        width: 2.0,
    };
    let light_teal_style = StrokeStyle {
        color: "hsla(150, 30%, 60%, 0.05)".into(),
        width: 2.0,
    };
    let black_style = StrokeStyle {
        color: "black".into(),
        width: 2.0,
    };
    let white_style = StrokeStyle {
        color: "rgba(255, 255, 255, 0.75)".into(),
        width: 1.0,
    };

    let min_x = -500.0;
    let min_y = 0.0;
    let max_x = 500.0;
    let max_y = 500.0;
    let min_radius = 10.0;
    let max_radius = 80.0;
    let circle_count = 1000;
    let circles : Vec<StrokePath> = (0..circle_count).map(|_| {
        let rand_radius = min_radius + rand::random::<f32>() * (max_radius - min_radius);
        // pick a random center such that the circle is fully within the bounds
        let center_x = min_x + rand::random::<f32>() * (max_x - min_x - 2.0 * rand_radius) + rand_radius;
        let center_y = min_y + rand::random::<f32>() * (max_y - min_y - 2.0 * rand_radius) + rand_radius;
        CanvasShape::Circle {
            center: Point::new(center_x, center_y),
            radius: rand_radius,
        }.into()
    }).collect();
    for circle in circles {
        let rand_stroke_angle = rand::random::<f32>() * std::f32::consts::TAU;
        let substrokes = circle.to_substrokes();
        let rand_step = 3.0 + rand::random::<f32>() * 3.0;
        let rand_roughness = rand::random::<f32>() * 10.0;
        for substroke in substrokes {
            let stroke = substroke.ray_stroke_fill(rand_step, rand_stroke_angle, rand_roughness);
            let color = if rand::random::<bool>() {
                light_teal_style.clone()
            } else {
                teal_style.clone()
            };
            canvas.add_strokes(stroke, color);
        }
    }

    let bounds = canvas.bounds();
    let textbox_opts = TextBoxOptions {
        jagged_edge: 0.0,
        layout: scribe::journal::canvas::textbox::TextBoxLineMode::Flat,
        bounds,
        symbol_scale: 50.0,
        avoid_tree: None,
        expected_stroke_width: Some(2.0),
    };
    let textbox = TextBox::new(textbox_opts);
    let mut builder = styles::mixedcase_builder(&symd, "loche_upper", "light_lower");
    builder.add_filter(|symbol| {
        if let SymbolType::Blank(_, _) = symbol.symbol_type() {
            return;
        }
        let mut duplicate = symbol.clone();
        let rand_scale = 1.0 + (rand::random::<f32>() - 0.5) * 0.7;
        let rand_angle = (rand::random::<f32>() - 0.5) * std::f32::consts::FRAC_PI_8;
        let roughness_theta = rand::random::<f32>() * std::f32::consts::TAU;
        let roughness_magnitude = 0.02 + rand::random::<f32>() * 0.05;
        let roughness_vector = Vector::new(
            roughness_magnitude * roughness_theta.cos(),
            roughness_magnitude * roughness_theta.sin(),
        );
        duplicate.translate_by(roughness_vector);
        duplicate.scale_by(rand_scale);
        duplicate.rotate_rad_by(rand_angle);
        let mut path_a = duplicate.placed_strokepath();
        let path_b = symbol.placed_strokepath();
        path_a.append(path_b);
        let sympos_a = symbol.position();
        let sympos_b = duplicate.position();
        let mid_pos = Point::new(
            (sympos_a.x + sympos_b.x) / 2.0,
            (sympos_a.y + sympos_b.y) / 2.0,
        );
        *symbol = Symbol::new(path_a, symbol.scale(), mid_pos, symbol.rotation_rad())
    });
    let msg = builder.build("now with the capability to color in shapes");
    let (placed_symbols, _) = textbox.fill_bounds_with(msg);
    for symbol in placed_symbols {
        canvas.add_strokes(symbol, white_style.clone());
    }
    let rect : StrokePath = CanvasShape::Rectangle {
        width: 200.0,
        height: 400.0,
        center: Point::new(0.0, 250.0),
    }.into();
    let rect_substrokes = rect.to_substrokes();
    for substroke in rect_substrokes {
        let stroke = substroke.ray_stroke_fill(3.0, std::f32::consts::FRAC_PI_4, 3.0);
        canvas.add_strokes(stroke, black_style.clone());
        let stroke = substroke.ray_stroke_fill(3.0, -std::f32::consts::FRAC_PI_4, 3.0);
        canvas.add_strokes(stroke, black_style.clone());
    }
    canvas
}

fn random_eyes_canvas(count: usize, scale: f32, symd: &symboldict::SymbolPathDict) -> Canvas {
    let eye = symd.new_symbol("basic_eye").expect("Failed to get 'eye' symbol");
    let mut canvas = Canvas::new();
    let min_x = -500.0;
    let min_y = -500.0;
    let max_x = 500.0;
    let max_y = 500.0;
    for _ in 0..count {
        let rand_rotation = (rand::random::<f32>() - 0.5) * std::f32::consts::TAU;
        // pick a random center such that the circle is fully within the bounds
        let center_x = min_x + rand::random::<f32>() * (max_x - min_x);
        let center_y = min_y + rand::random::<f32>() * (max_y - min_y);
        let mut new_eye = eye.clone();
        new_eye.scale_by(scale);
        new_eye.translate_by(Vector::new(center_x, center_y));
        new_eye.rotate_rad_by(rand_rotation);
        let hue = rand::random::<f32>() * 360.0;
        let color = format!("hsla({}, 80%, 60%, 1.0)", hue);
        let stroke_style = StrokeStyle {
            color: color,
            width: 50.0,
        };
        canvas.add_strokes(new_eye, stroke_style);
    }
    canvas
}

fn draw_msg_with_colormask(symd: &symboldict::SymbolPathDict) -> Canvas {
    let textbox_width = 800.0;
    let textbox_height = 1200.0;
    let builder = styles::mixedcase_builder(&symd, "loche_upper", "light_lower");
    let backgroundtxt = "loche";
    let symbols = builder.build(backgroundtxt);
    let bounds = parry2d::bounding_volume::Aabb::new(
        Point::new(-textbox_width / 2.0, -textbox_height / 2.0),
        Point::new(textbox_width / 2.0, textbox_height / 2.0),
    );
    let textbox_opts = TextBoxOptions {
        bounds,
        jagged_edge: 0.0,
        layout: scribe::journal::canvas::textbox::TextBoxLineMode::Flat,
        symbol_scale: 100.0,
        avoid_tree: None,
        expected_stroke_width: Some(5.0),
    };
    let textbox = TextBox::new(textbox_opts);
    let symbols = textbox.fill_bounds_with(symbols).0;
    let mut canvas = Canvas::new();
    for symbol in symbols {
        let stroke_style = StrokeStyle {
            color: "rgba(200, 200, 200, 1.0)".into(),
            width: 15.0,
        };
        canvas.add_strokes(symbol, stroke_style);
    }
    let svg = canvas.to_svg(None).into_bytes();

    let remap_default : RemapColorFunc = Box::new(|nearest_results| {
        let default_color = "rgba(255, 255, 255, 1.0)";
        let mut count = 0;
        let mut avg_color = (0.0, 0.0, 0.0, 0.0);
        for (_, (color, _)) in nearest_results {
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
        if count == 0 {
            return default_color.to_string();
        }
        let color = tiny_skia::ColorU8::from_rgba(
            (avg_color.0 / count as f32) as u8,
            (avg_color.1 / count as f32) as u8,
            (avg_color.2 / count as f32) as u8,
            (avg_color.3 / count as f32) as u8,
        ).premultiply();
        format!("rgba({}, {}, {}, {:.3})", color.red(), color.green(), color.blue(), color.alpha() as f32 / 255.0)
    });

    let textbox_width_a = textbox_width.clone();
    let remap_a : RemapColorFunc = Box::new(move |nearest| {
        let textbox_width = textbox_width_a;
        let mut position = Point::<f32>::new(0.0, 0.0);
        let mut count = 0.0;
        for (_, (_, pos)) in nearest.iter() {
            position += pos.coords;
            count += 1.0;
        }
        if count > 0.0 {
            position.x /= count;
            position.y /= count;
        }
        let percent_x = (position.x + textbox_width / 2.0) / textbox_width;
        let hue = (percent_x * 360.0) % 360.0;
        format!("hsla({}, 100%, 90%, 1.0)", hue)
    });

    let textbox_width_b = textbox_width.clone();
    let remap_b : RemapColorFunc = Box::new(move |nearest| {
        let textbox_width = textbox_width_b;
        let mut position = Point::<f32>::new(0.0, 0.0);
        let mut count = 0.0;
        for (_, (_, pos)) in nearest.iter() {
            position += pos.coords;
            count += 1.0;
        }
        if count > 0.0 {
            position.x /= count;
            position.y /= count;
        }
        let percent_x = (position.x + textbox_width / 2.0) / textbox_width;
        let percent_y = (position.y + textbox_height / 2.0) / textbox_height;
        let perlin = Perlin::new(rand::random::<u32>());
        let noise_value = perlin.get([percent_x as f64 / 20.0, percent_y as f64 / 20.0]) as f32;
        let noise_value = (noise_value + 1.0) / 2.0;
        let hue = noise_value * 360.0;
        format!("hsla({}, 80%, 60%, 1.0)", hue)
    });
    let colormaps = vec![
        (CanvasColorMap::SvgBytes(svg), remap_b),
    ];
    let colormap_options = ColorMappedTextBoxOptions {
        symbol_scale: 30.0,
        sample_spacing: 7.0,
        stroke_width: 3.0,
        width: textbox_width,
        height: textbox_height,
    };
    let cmtextbox = ColorMappedTextBoxCollection::new_with_recolor(colormaps, &colormap_options);
    //let lorem = lorem_ipsum(648);
    //let msg = builder.build(lorem.as_str());
    let msg = builder.build(test_content());
    let canvases = cmtextbox.place_all_symbols_on_all_canvases(msg);

    let page_count = canvases.len();
    let grid_size = (page_count as f32).sqrt().ceil() as usize;
    let page_width = textbox_width + 50.0;
    let page_height = textbox_height + 50.0;
    let mut canvas = Canvas::new();
    let border_color = StrokeStyle {
        color: "black".to_string(),
        width: 1.0,
    };
    for (i, mut page_canvas) in canvases.into_iter().enumerate() {
        let row = i / grid_size;
        let col = i % grid_size;
        let translation = Vector::new(
            col as f32 * page_width,
            row as f32 * page_height,
        );
        let page_bounds = page_canvas.bounds();
        let canvas_width = page_bounds.extents().x;
        let canvas_height = page_bounds.extents().y;
        let center_x = page_bounds.mins.x + canvas_width / 2.0;
        let center_y = page_bounds.mins.y + canvas_height / 2.0;
        let border = CanvasShape::Rectangle {
            width: canvas_width,
            height: canvas_height,
            center: Point::new(center_x, center_y),
        };
        page_canvas.add_strokes(border, border_color.clone());
        page_canvas.translate_by(translation);
        canvas.merge_canvas(page_canvas);
    }
    canvas
}

pub fn stretch_test_canvas(symd: &symboldict::SymbolPathDict) -> Canvas {
    //use stretch::geometry::Size;
    //use stretch::style::*;

    //let mut stretch = stretch::node::Stretch::new();
    //
    //let child = stretch.new_node(
    //    Style { size: Size { width: Dimension::Percent(0.5), height: Dimension::Auto }, ..Default::default() },
    //    vec![],
    //).expect("Failed to create child node");

    //let node = stretch.new_node(
    //    Style {
    //        size: Size { width: Dimension::Points(100.0), height: Dimension::Points(100.0) },
    //        justify_content: JustifyContent::Center,
    //        ..Default::default()
    //    },
    //    vec![child],
    //).expect("Failed to create parent node");

    //stretch.compute_layout(node, Size::undefined()).expect("Failed to compute layout");
    //let layout = stretch.layout(child).expect("Failed to get layout").
    //dbg!(layout);

    let textbox_width = 400.0;
    let textbox_height = 1200.0;
    let builder = styles::mixedcase_builder(&symd, "loche_upper", "light_lower");
    let symbols = builder.build(test_content());
    let bounds = parry2d::bounding_volume::Aabb::new(
        Point::new(-textbox_width / 2.0, -textbox_height / 2.0),
        Point::new(textbox_width / 2.0, textbox_height / 2.0),
    );
    let textbox_opts = TextBoxOptions {
        bounds,
        jagged_edge: 0.5,
        layout: scribe::journal::canvas::textbox::TextBoxLineMode::WordsByBlankSymbol,
        symbol_scale: 30.0,
        avoid_tree: None,
        expected_stroke_width: Some(5.0),
    };
    let textbox = TextBox::new(textbox_opts);
    let symbolsets = textbox.place_all_symbols(symbols);
    let mut canvas = Canvas::new();
    for (i, symbolset) in symbolsets.into_iter().enumerate() {
        let stroke_style = StrokeStyle {
            color: "rgba(200, 200, 200, 1.0)".into(),
            width: 3.0,
        };
        let mut symbol_canvas = Canvas::new();
        for symbol in symbolset {
            symbol_canvas.add_strokes(symbol, stroke_style.clone());
        }
        let x_offset = (i as f32) * (textbox_width + 50.0);
        symbol_canvas.translate_by(Vector::new(x_offset, 0.0));
        canvas.merge_canvas(symbol_canvas);
    }
    canvas
}

pub fn main() {
    let symd = symboldict::SymbolPathDict::from_file("config/symbols.json").expect("Failed to load symbols.json");
    //let canvas = stretch_test_canvas(&symd);
    let mut plough_symbol = symd.new_symbol("ploughdiem").expect("Failed to get 'ploughdiem' symbol");
    plough_symbol.scale_by(5000.0);
    let plough_stroke = plough_symbol.placed_strokepath();
    let builder = styles::lowercase_builder(&symd, "loche_upper");
    let words = builder.build(test_content());
    let mut symbol_placement_args = SymbolPlacementArgs {
        avoid_kdtree: None,
        symbol_scale: 20.0,
    };
    let mut placed = None;
    let mut scale_min = 0.0;
    let mut scale_max = std::f32::MAX;
    let mut iterations = 0;
    while placed.is_none() {
        iterations += 1;
        let (placed_symbols, unplaced_symbols) = Symbol::place_along_path(words.clone(), &plough_stroke, &symbol_placement_args);
        if unplaced_symbols.is_empty() {
            scale_min = symbol_placement_args.symbol_scale;
        } else {
            scale_max = symbol_placement_args.symbol_scale;
        }
        symbol_placement_args.symbol_scale = (scale_min + scale_max) / 2.0;
        if (scale_max - scale_min) < 0.1 {
            placed = Some(placed_symbols);
        }
    }
    let placed_symbol = placed.expect("Failed to place symbols along path");
    println!("Placed {} symbols", placed_symbol.len());
    println!("Final symbol scale: {}", symbol_placement_args.symbol_scale);
    println!("Iterations: {}", iterations);
    let mut canvas = Canvas::new();
    canvas.add_strokes(placed_symbol, StrokeStyle { color: "black".into(), width: 2.0 });
    let canvas_style = CanvasStyleOptions {
        roughness: 1.5,
        point_sample_distance: 2.0,
        trail_off_percent: 0.25,
        noise_amplitude: 1.0,
    };

    let svg_data = canvas.to_svg(None).into_bytes();
    let mut svg_file = std::fs::File::create("drawing_canvas_output.svg").expect("Failed to create output file");
    std::io::copy(&mut &svg_data[..], &mut svg_file).expect("Failed to write SVG to file");

    let png = canvas.to_png(1.0, Some(&canvas_style));
    let mut file = std::fs::File::create("drawing_canvas_output.png").expect("Failed to create output file");
    std::io::copy(&mut &png[..], &mut file).expect("Failed to write PNG to file");
}
