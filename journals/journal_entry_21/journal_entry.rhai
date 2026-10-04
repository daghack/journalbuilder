const PI = 3.14159;
const TAU = 2 * PI;

fn draw_ternary_circle(radius) {
	let start_rad = global::PI / 2.0;
	let step_arc_rad = global::TAU / 27.0;
	let path = new_strokepath();
	let start_vec = new_vec(cos(start_rad) * radius, sin(start_rad) * radius);
	path = path.move_to(start_vec);
	for i in 0..27 {
		let current_rad = -1 * i * step_arc_rad + start_rad;
		let center_x = cos(current_rad) * radius;
		let center_y = sin(current_rad) * radius;
		let next_vec = new_vec(center_x, center_y);
		path.line_to(next_vec);
	}
	path.line_to(start_vec);
	path
}

let journal_page = new_page();

let canvas = new_canvas();

let style = new_strokestyle(1.5, "darkblue");

let sharp_font = new_fontset(["loche_upper"], ["sharp_lower"]);
let loche_font = new_fontset(["loche_upper"], ["loche_lower"]);
let copper_font = new_fontset(
	["copper_upper", "loche_upper"],
	["copper_lower", "loche_lower"],
);

let calligraphy_brush = new_brush("Calligraphy", 0.25, 0.5, 1.0);
let rough_ink_brush = new_brush("Rough Ink", 0.25, 0.5, 1.0);

let copper_msg = "";

let general_msg = "";

let secret_msg = "";

let next_game_plans = "";

let lock = new_lock(lock_config);

let circle_path = draw_ternary_circle(500.0);

let page = new_page();
let canvas = new_canvas();
let style = new_strokestyle(2.5, "darkred");
canvas.

page.add_canvas(canvas, rough_ink_brush);
journal_entry.add_page(page);
