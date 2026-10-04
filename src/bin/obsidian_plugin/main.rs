fn main() {}
/*
use serde::{Deserialize, Serialize};
use std::io::{self, Read};

use scribe::journal::symboldict::SymbolPathDict;
use scribe::journal::symbol::Symbol;
use scribe::journal::new_rc_ref;
use scribe::journal::styles;
use scribe::journal::journalelems::TextBox;
use scribe::journal::*;

#[derive(Debug, Serialize, Deserialize)]
struct JournalRenderSettings {
    page_width: f32,
    page_height: f32,
    char_size: f32,
    char_percent_margin: Option<f32>,
    tab_spaces: Option<i32>,
    white_space_scalar: Option<f32>,
    symbol_rotational_jitter: Option<f32>,
    symbol_positional_jitter: Option<f32>,
    symbol_scale_jitter: Option<f32>,
    line_thickness: Option<f32>,
    line_variance: Option<f32>,
    interp_resolution: Option<i32>,
    taper_distance: Option<f32>,
    flow_period: Option<f32>,
    flow_samples: Option<i32>,
    journal_line_variance: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalRenderInput {
    #[serde(with = "scribe::base64::string")]
    markdown_content: String,
    journal_render_settings: JournalRenderSettings,
}

fn main() {
    // read a json JournalRenderSettings object from stdin
    // and print it to stdout
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).expect("Failed to read from stdin");
    let input: JournalRenderInput = serde_json::from_str(&input).expect("Failed to parse JSON");


    let JournalRenderSettings {
        page_width,
        page_height,
        char_size,
        ..
    } = input.journal_render_settings;
    let (page_bounding_box, symbol_bounding_box) = get_bounding_boxes(page_width, page_height);

    let symdict_json = include_bytes!("symbol_dictionary.json");
    let symdict = SymbolPathDict::from_bytes(symdict_json).expect("Failed to parse symbol dictionary");
    let mut symbol_set = Vec::new();
    for sym_name in vec!["archangel_0", "archangel_1", "archangel_2", "archangel_3", "archangel_4", "archangel_5", "archangel_6"] {
        let sym_base = symdict.get_symbol_base(sym_name).expect("Failed to get symbol base");
        let symbol = Symbol::new_from_base_rc(sym_base, 1.0, parry2d::math::Point::new(0.0, 0.0), 0.0);
        symbol_set.push((symbol, char_size));
    }

    let msg_symbols = styles::loche::str_to_symbol_vecdeque(&input.markdown_content, &symdict);

    let textbox = new_rc_ref(TextBox::new_with_avoid_iter(
        page_bounding_box,
        char_size,
        msg_symbols,
        new_rc_ref(symbol_set.into_iter().cycle()),
    ));
    let renderer = Box::new(renderers::BasicRender::new(renderers::BasicRenderType::VerticalStackRender, renderers::BasicRenderArgs {
        width: Some(page_width),
        height: Some(page_height),
        positional_jitter: None,
        rotational_jitter: None,
    }));
    let journalpipe = JournalEngine::new(renderer);
    let manifest = journalpipe.generate_journal(vec![textbox]);

    let output_json = serde_json::to_string(&manifest).expect("Failed to serialize manifest");
    println!("{}", output_json);
}

fn get_bounding_boxes(
    page_width: f32,
    page_height: f32,
) -> (parry2d::bounding_volume::Aabb, parry2d::bounding_volume::Aabb) {
    let page_bounding_box = parry2d::bounding_volume::Aabb::new(
        parry2d::math::Point::new(0.0, 0.0),
        parry2d::math::Point::new(page_width, page_height),
    );

    let symbol_size = page_width.max(page_height) / 1.3;
    let (symbol_tl_x, symbol_tl_y) = if page_width > page_height {
        ((page_width - symbol_size) / 2.0, (page_height - symbol_size) / 2.0)
    } else {
        ((page_width - symbol_size) / 2.0, (page_height - symbol_size) / 2.0)
    };
    let symbol_bounding_box = parry2d::bounding_volume::Aabb::new(
        parry2d::math::Point::new(symbol_tl_x, symbol_tl_y),
        parry2d::math::Point::new(symbol_tl_x + symbol_size, symbol_tl_y + symbol_size),
    );


    (page_bounding_box, symbol_bounding_box)
}
*/
