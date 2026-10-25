// The API reference: every function journal scripts can call, with its
// arguments, its options and what it gives back, typeset as a book.
//
// Each entry is api("signature", "description", [arguments]): the
// signature in red, then what it does, then each argument and option on a
// line of its own: arg(name, effect, values) for one that must be given,
// opt(name, default, effect, values) for one that can be left out, and
// req(name, effect, values) for an option key that must be given.
// tests/api_reference.rs fails when a function scripts can call isn't in
// one of the signatures here; when the API changes (a function, an
// argument, an option, a property, a default), change its entry in the
// same change: see CLAUDE.md. Signatures can't hold a double quote.
// journals/showcase shows what each call draws; this says what each one
// takes.
//
// Render it with: journalbuild api_reference (add --pdf for one file).

// ---- the book's look, as the showcase's ----

const W = 1200.0;
const H = 1600.0;
const INK = "#2b1d12";
const RED = "#9b2318";

// Constant pressure, and text no smaller than 26, so the plain glyphset's
// dots (strokes about a unit long) show.
const TEXT = new_style(new_brush_style(INK, "Ink Pen", 1.5, #{ min_pressure: 0.4, pressure_profile: "constant" }), new_handwriting_style());
const CAPTION = new_style(new_brush_style(RED, "Ink Pen", 1.2), new_handwriting_style());
const HEADING = new_style(new_brush_style(RED, "Calligraphy", 1.5, #{ min_pressure: 0.48, max_pressure: 0.96, pressure_profile: "constant" }), new_handwriting_style());

const PLAIN = glyphset("plain");
const LOCHE = glyphset("loche_upper");

// ---- entries ----

// Text as it is, with markdown's marks escaped.
fn esc(s) {
	escape_markdown(s)
}

// A section's heading.
fn heading(title) {
	"[" + esc(title) + "]{glyph_scale_factor: 1.6, color: #9b2318, underline: {style: {color: #9b2318}}}\n\n"
}

// A paragraph of prose.
fn para(text) {
	esc(text) + "\n\n"
}

// An argument (or the value a property is set to) that must be given: its
// name, what it does, and the values it takes.
fn arg(name, effect, values) {
	"- *" + esc(name) + "* — " + esc(effect) + " Values: " + esc(values) + "\n"
}

// An argument or option that can be left out, and what it is then.
fn opt(name, fallback, effect, values) {
	"- *" + esc(name) + "* (optional, default " + esc(fallback) + ") — " + esc(effect) + " Values: " + esc(values) + "\n"
}

// An option key that must be given.
fn req(name, effect, values) {
	"- *" + esc(name) + "* (required) — " + esc(effect) + " Values: " + esc(values) + "\n"
}

// The lines of a listing.
fn lines(items) {
	let s = "";
	for item in items {
		s += item;
	}
	s
}

// One function, property or operator: its signature, then what it does.
fn api(signature, description) {
	"[" + esc(signature) + "]{color: #9b2318}\n" + esc(description) + "\n\n"
}

// ...and then its arguments, a line each.
fn api(signature, description, items) {
	"[" + esc(signature) + "]{color: #9b2318}\n" + esc(description) + "\n" + lines(items) + "\n"
}

// A listing that isn't a call, such as the keys of a style map.
fn listing(intro, items) {
	esc(intro) + "\n" + lines(items) + "\n"
}

// ---- arguments many entries share ----

fn style_arg() { arg("style", "How the strokes are drawn: their brush style and handwriting.", "a style, from new_style or default_style.") }
fn vec_arg(name, effect) { arg(name, effect, "a vec.") }
fn offset_arg() { arg("offset", "How far to move, across and down.", "a vec.") }
fn angle_arg(effect) { arg("angle", effect + " Positive turns clockwise, y being down.", "a number, in radians.") }
fn name_arg(what) { arg("text", "The " + what + "'s name in output files.", "a string of letters, digits, underscores, hyphens and dots, not starting with a dot, unique among its siblings. Reading it gives () until it's set.") }
fn avoid_opt(what) { opt("settings.avoid", "nothing", "What glyphs keep clear of" + what + " (see Keeping Text Clear).", "an avoidance tree, a path, a canvas or a glyph, or an array of any of these.") }

fn seed_opt(what) { opt("options.seed", "one from the generator", "Which " + what + " it draws: the same seed gives the same drawing, without touching the script's generator; left out, the journal's generator picks one.", "a whole number.") }
fn thickening_opts(parts, reach) {
	[
		opt("options.origin", "none", "A growth origin: " + parts + " are thicker toward it, as old wood is. Thorns, stalks, leaves and flowers keep their size. Needed for thickness and reach.", "a vec."),
		opt("options.thickness", "3", "How many times their usual width " + parts + " are at the origin, easing back to it at the reach.", "a number of at least 1."),
		opt("options.reach", reach, "How far from the origin " + parts + " are back to their usual width.", "a positive number."),
	]
}

let doc = "";

// ---- reading this ----

doc += heading("Reading This Reference");
doc += para("A journal is a directory of entries and the files they use (see Journals and Their Files). Each entry is a rhai script: it draws strokes onto canvases, puts canvases on pages, and adds the pages to journal_entry, which journalbuild renders: one image per canvas, or one per page on parchment, or a PDF. This book lists every function a script can call, by what it works on. Each entry gives a signature in red and then what it does.");
doc += para("In signatures, receiver.method(arguments) is called on a value, as path.line_to(point); a bare name(arguments) is a global function. A name = value entry is a property: it can be read, and set unless the entry says it's read only.");
doc += para("Under each entry, each argument has a line of its own: its name in italics, what it does, and the values it takes. An argument marked optional can be left out of the call, and is then its default; the others must be given. An argument written options is a map, #{ key: value, ... }, and its keys are listed as options.KEY: each optional, with its default, unless it's marked required. Unknown keys are errors. For a property, the line is the value it's set to. Words in the values, such as ramp or flat, are strings: write them in double quotes.");
doc += para("Every call has one shape: what it needs, in order, then one map of everything optional, which can be left out: path.hatch(4, #{ angle: 0.3 }). A constructor's options also take any property its value can be set by, checked as setting it is: new_brush_style(colour, brush, brush_size, #{ pressure_profile: constant }) is a brush style with its pressure_profile set. Text calls take one settings map: a style, a type and a paragraph, as layout roles do, so a role can be passed straight in. Canvases, pages, the entry, trees and lock chains all take what's put in them with add, which gives back what it was called on, so calls chain.");
doc += para("Units: coordinates are canvas units, one pixel at --scale 1, with y running down the page. Angles are radians; PI() is rhai's, and degrees(d) converts. Numbers can be whole or decimal anywhere a number is taken, and must be finite; a whole number is needed only where the values say so. Colours are CSS colours: a name, #rgb, #rrggbb, rgb(), rgba(), hsl() or hsla().");
doc += para("Shared values: paths, glyphs, canvases and avoidance trees are shared. Assigning one, passing it or looping over it gives the same value, so changes through any copy reach it; copy() makes an independent one. Their transforms change the value and return it, so calls chain: p.move_to(a).line_to(b). Vecs, glyphsets, styles (of every kind) and pages are plain values: changing one changes that copy only.");
doc += para("Randomness is seeded: random(), glyph variant picks, text layout, hatching, point distributions and random spanning trees all draw on one generator with a fixed seed, so a journal renders the same every run. So does the handwriting that strokes are added to canvases in, and the procedural art when it isn't given a seed of its own.");
doc += para("Sections: the entry and its pages; canvases and strokes; vectors and angles; paths; styles; gradients; glyphsets and glyphs; text; keeping text clear; randomness and messages; generators; balanced ternary; procedural art; books; page layouts; marked-up text; locks; journals and their files; lock, illustration, layout and style files; building.");

// ---- the entry and its pages ----

doc += heading("The Entry and Its Pages");
doc += para("A script starts with journal_entry, an empty entry, in scope. Whatever journal_entry holds when the script ends is rendered.");
doc += api("journal_entry.add(page)", "Adds a copy of a page to the entry. Each page's name, or page_N without one, names its output files. Returns the entry.", [
	arg("page", "The page to add.", "a page, from new_page, page.split or layout.typeset, whose name no other page in the entry has."),
]);
doc += api("journal_entry.add(pages)", "Adds pages, such as split tiles, typeset pages or a book's, in order.", [
	arg("pages", "The pages to add.", "an array of pages, with names no other page in the entry has."),
]);
doc += api("new_page(), new_page(options)", "A new, empty page, its properties (below) set by the options as setting them does.", [
	opt("options.name", "none", "As page.name.", "as page.name."),
	opt("options.size", "()", "As page.size.", "as page.size."),
	opt("options.margin", "()", "As page.margin.", "as page.margin."),
	opt("options.svg_style", "the defaults below", "As page.svg_style.", "as page.svg_style."),
]);
doc += api("page.add(canvas)", "Adds a copy of the canvas, as it is now, to the page, and returns the page, so new_page(options).add(canvas) is a page. Each canvas renders as an image of its own, darkly_PAGE_CANVAS.png, named after the page and the canvas's name or index.", [
	arg("canvas", "The canvas to render on the page; later changes to it don't reach the page.", "a canvas whose name, or index without one, no other canvas on the page has."),
]);
doc += api("page.name = text", "The page's name in output files.", [
	name_arg("page"),
]);
doc += api("page.size = size", "A set size: the page is exactly the region from the origin to this, and strokes past its edges are cut off.", [
	arg("size", "The page's width and height.", "a vec, vec2(width, height), each at least 1; or () (the default) for as big as its strokes."),
]);
doc += api("page.margin = number", "Space round each canvas's strokes.", [
	arg("number", "How much space, in canvas units, 0 for none.", "a number of at least 0; or () (the default) for half the largest brush size on the canvas, enough that brush marks at the edges aren't cut off."),
]);
doc += api("page.svg_style = options", "How the page's SVG output (--svg) is drawn.", [
	arg("options", "Keys left out keep their defaults.", "a map of the keys below, or () for plain strokes."),
	opt("options.roughness", "1", "How far each stroke is roughened.", "a number of at least 0."),
	opt("options.point_sample_distance", "1", "How far apart strokes are sampled, in canvas units.", "a positive number."),
	opt("options.trail_off_percent", "0.15", "How much of each end of a stroke tapers away, as a fraction of it.", "a number of at least 0."),
	opt("options.noise_amplitude", "0.3", "How much noise is in the opacity of the image the SVG is rasterised to.", "a number of at least 0."),
]);
doc += api("page.split(), page.split(options)", "Cuts a named page into a grid of tiles, each a page of its own named NAME_ROW_COLUMN (counting from 0), row by row, the tiles counted or of a set size, across and down. Each tile renders exactly its share of the pixels of the whole page's render, strokes crossing a seam drawn whole and cut off at it, so a canvas's tiles placed side by side make up its render. Gives the tiles as an array. Fails for a page with no name, nothing drawn, or that is a tile itself.", [
	opt("options.columns", "1", "How many tiles across. Not with tile_width.", "a whole number from 1 to 1000."),
	opt("options.rows", "1", "How many tiles down. Not with tile_height.", "a whole number from 1 to 1000."),
	opt("options.tile_width", "none", "Tiles this wide, in canvas units: the grid grows evenly round the page to a whole number of them. Not with columns.", "a positive number."),
	opt("options.tile_height", "none", "Tiles this tall, as tile_width. Not with rows.", "a positive number."),
]);
doc += api("page.tile", "Read only: a tile's place in its page's grid, #{ row, column, rows, columns }, or () for a page that isn't a tile.");

// ---- canvases ----

doc += heading("Canvases and Strokes");
doc += para("A canvas holds strokes, each a path drawn in a style. It's cropped to its strokes when rendered, plus the page's margin.");
doc += api("new_canvas(), new_canvas(options)", "A new, empty canvas.", [
	opt("options.name", "none", "As canvas.name.", "as canvas.name."),
]);
doc += para("canvas.add puts a copy of what it's given on the canvas, as it is now (later changes don't reach the canvas), and returns the canvas, so adds chain.");
doc += api("canvas.add(path, style)", "Adds the path, drawn in the style.", [
	arg("path", "The lines to draw.", "a path whose points are all finite (no infinity or NaN)."),
	style_arg(),
]);
doc += api("canvas.add(glyph, style), canvas.add(glyphs, style)", "Adds a placed glyph's strokes, or each of an array of glyphs', drawn in the style; an array's glyphs join up as glyphset.joined says.", [
	arg("glyph, glyphs", "What to draw, where it's placed.", "a glyph, or an array of them (as glyphset.glyphs or textbox.fill give); a blank one (a space) adds nothing."),
	style_arg(),
]);
doc += api("canvas.add(other), canvas.add(canvases)", "Adds every stroke on another canvas, or each of an array of canvases (as fill_with_text gives), in its own style.", [
	arg("other, canvases", "The canvases whose strokes are copied.", "a canvas (this one included), or an array of them."),
]);
doc += api("canvas.add(parts, styles)", "Adds each path of a map of parts, as art::bramble, art::eye, art::huffman_stars, art::moon and art::snake give them, in the style styles has under its name, in name order; parts with no style aren't drawn.", [
	arg("parts", "The paths, by name.", "a map of names to paths, and to anything else so long as it isn't styled (as an eye's iris_radius)."),
	arg("styles", "Their styles, by part name.", "a map of part names to styles, every name one of the parts'."),
]);
doc += api("canvas.name = text", "The canvas's name in output files; without one, its index on the page.", [
	name_arg("canvas"),
]);
doc += api("canvas.bounds()", "What the canvas's strokes cover, their widths included: #{ min, max, center, width, height }, min, max and center being vecs. Fails for an empty canvas.");
doc += api("canvas.translate_by(offset)", "Moves every stroke. Returns the canvas.", [
	offset_arg(),
]);
doc += api("canvas.scale_by(factor)", "Scales every stroke about the origin; brush sizes stay as they are. Returns the canvas.", [
	arg("factor", "How many times bigger: below 1 shrinks, and negative flips through the origin.", "a number."),
]);
doc += api("canvas.rotate_around(point, angle)", "Turns every stroke about a point. Returns the canvas.", [
	vec_arg("point", "The centre of the turn."),
	angle_arg("How far to turn."),
]);
doc += api("canvas.copy()", "An independent copy of the canvas.");

// ---- vectors ----

doc += heading("Vectors and Angles");
doc += para("Vecs are points and directions: vec2s on the page, and vec3s and vec4s for points of more coordinates, such as distribute_points gives for 3 or 4 bounds. They're values: methods give new vecs and leave the one they're called on as it was. Where an entry says a vec, it means a vec2.");
doc += api("vec2(x, y)", "A vec2, a point or direction on the page.", [
	arg("x", "How far across, growing to the right.", "a finite number; infinity and NaN (from a division by zero, say) are refused."),
	arg("y", "How far down, growing down the page.", "a finite number."),
]);
doc += api("v.x = number, v.y = number", "Its coordinates.", [
	arg("number", "The coordinate.", "a finite number."),
]);
doc += api("a + b, a - b, -a", "Sums, differences and negation.", [
	arg("a, b", "The vecs.", "vecs."),
]);
doc += api("v * number, number * v, v / number", "Scaled by a number.", [
	arg("v", "The vec.", "a vec."),
	arg("number", "What it's multiplied, or divided, by.", "a finite number, whole or decimal; not 0 to divide by."),
]);
doc += api("a == b, a != b", "Whether two vecs are the same.", [
	arg("a, b", "The vecs.", "vecs."),
]);
doc += api("v.length()", "How long it is.");
doc += api("v.distance(other)", "How far it is from another vec.", [
	vec_arg("other", "The other point."),
]);
doc += api("v.dot(other)", "The dot product.", [
	vec_arg("other", "The other vec."),
]);
doc += api("v.angle()", "Its angle from the x axis, from -PI() to PI(); with y down, positive angles turn clockwise.");
doc += api("v.normalized()", "A vec of length 1, the same way. Fails for the zero vec.");
doc += api("v.rotated(angle)", "The vec turned about the origin.", [
	angle_arg("How far to turn."),
]);
doc += api("v.to_string(), v.to_debug()", "The vec as text, (x, y), for print and debug.");
doc += api("vec3(x, y, z), vec3(v, z)", "A vec3: three coordinates.", [
	arg("x, y, z", "Its coordinates.", "finite numbers."),
	arg("v", "Its x and y, from a vec2, with z after.", "a vec2."),
]);
doc += api("vec4(x, y, z, w), vec4(v, w)", "A vec4: four coordinates.", [
	arg("x, y, z, w", "Its coordinates.", "finite numbers."),
	arg("v", "Its x, y and z, from a vec3, with w after.", "a vec3."),
]);
doc += api("v.x = number, v.y = number, v.z = number, v.w = number", "A vec3's or vec4's coordinates; only a vec4 has w.", [
	arg("number", "The coordinate.", "a finite number."),
]);
doc += api("a + b, a - b, -a, v * number, number * v, v / number, a == b, a != b", "For vec3s and vec4s as for vec2s: sums, differences, scaling and comparison, between vecs of one size.", [
	arg("a, b", "The vecs.", "two vec3s or two vec4s."),
	arg("number", "What it's multiplied, or divided, by.", "a finite number; not 0 to divide by."),
]);
doc += api("v.length(), v.distance(other), v.dot(other), v.normalized()", "For vec3s and vec4s as for vec2s: how long, how far from another, the dot product, and length 1 the same way (failing for the zero vec).", [
	arg("other", "The other vec.", "a vec of the same size."),
]);
doc += api("v.xy()", "A vec3's or vec4's first two coordinates, as a vec2: to draw it.");
doc += api("v.xyz()", "A vec4's first three coordinates, as a vec3.");
doc += api("a.cross(b)", "The cross product of two vec3s: at right angles to both, as long as the area they span.", [
	arg("b", "The other vec.", "a vec3."),
]);
doc += api("degrees(d)", "An angle given in degrees, as the radians every angle is: degrees(180) is PI().", [
	arg("d", "The angle, in degrees.", "a finite number."),
]);
doc += api("radians(r)", "An angle given in radians, as it is: for saying which unit a number is in.", [
	arg("r", "The angle, in radians.", "a finite number."),
]);

// ---- paths ----

doc += heading("Paths");
doc += para("A path is lines and curves, in one or more subpaths. Every path function changes the path and returns it, so statements, assignments and chains all work.");
doc += api("new_path()", "A new, empty path.");
doc += api("path.move_to(point)", "Starts a new subpath.", [
	vec_arg("point", "Where it starts."),
]);
doc += api("path.line_to(point)", "A straight line from where the path is.", [
	vec_arg("point", "Where the line ends."),
]);
doc += api("path.quad_to(control, point)", "A quadratic curve from where the path is.", [
	vec_arg("control", "The point the curve bends toward."),
	vec_arg("point", "Where it ends."),
]);
doc += api("path.cubic_to(control1, control2, point)", "A cubic curve from where the path is.", [
	vec_arg("control1", "The point the curve leaves toward."),
	vec_arg("control2", "The point it arrives from."),
	vec_arg("point", "Where it ends."),
]);
doc += api("path.close()", "Closes the subpath back to its start.");
doc += api("path.append(other)", "Adds a copy of another path's subpaths, so several shapes are one path: to hatch a shape with holes, say.", [
	arg("other", "The path whose subpaths are added.", "a path, this one included."),
]);
doc += api("new_line(start, end)", "A straight line between two points.", [
	vec_arg("start", "Where it starts."),
	vec_arg("end", "Where it ends."),
]);
doc += api("new_circle(center, radius)", "A circle.", [
	vec_arg("center", "Its centre."),
	arg("radius", "How far its edge is from the centre.", "a number."),
]);
doc += api("new_rectangle(center, width, height)", "A rectangle about its centre, its sides square to the page.", [
	vec_arg("center", "Its centre."),
	arg("width", "Its size across.", "a number."),
	arg("height", "Its size down.", "a number."),
]);
doc += api("new_arc(center, radius, start_angle, end_angle)", "An arc of a circle, from one angle to another, either way round.", [
	vec_arg("center", "The circle's centre."),
	arg("radius", "The circle's radius.", "a number."),
	arg("start_angle", "Where the arc starts, from the x axis; positive angles are clockwise, y being down.", "a number, in radians."),
	arg("end_angle", "Where it ends: it runs from start_angle toward this.", "a number, in radians."),
]);
doc += api("new_polyline(points), new_polyline(points, options)", "Straight lines through points, in order.", [
	arg("points", "The corners.", "an array of vecs."),
	opt("options.closed", "false", "Whether the last joins back to the first.", "true or false."),
]);
doc += api("new_polygon(center, radius, sides), new_polygon(center, radius, sides, options)", "A regular polygon, its corners radius from the centre; or, with step, a star in one line.", [
	vec_arg("center", "Its centre."),
	arg("radius", "How far its corners are from the centre.", "a number."),
	arg("sides", "How many corners.", "a whole number from 3 to 10000."),
	opt("options.angle", "-PI() / 2, up the page", "Where its first corner is, from the centre.", "a number, in radians."),
	opt("options.step", "1", "Each corner joined to the step-th on: 5 sides and step 2 is a five-pointed star in one line. Where step and sides share a factor, there's a closed outline for each cycle of corners.", "a whole number from 1 to sides - 1."),
]);
doc += api("new_star(center, outer, inner, points), new_star(center, outer, inner, points, options)", "A star's outline: its points outer from the centre, the corners between them inner.", [
	vec_arg("center", "Its centre."),
	arg("outer", "How far its points reach.", "a number."),
	arg("inner", "How far in the corners between them are.", "a number."),
	arg("points", "How many points.", "a whole number from 2 to 10000."),
	opt("options.angle", "-PI() / 2, up the page", "Where its first point is, from the centre.", "a number, in radians."),
]);
doc += api("new_ellipse(center, rx, ry), new_ellipse(center, rx, ry, options)", "An ellipse, as four curves.", [
	vec_arg("center", "Its centre."),
	arg("rx, ry", "Its radii across and down, before it's turned.", "numbers."),
	opt("options.angle", "0", "How far it's turned.", "a number, in radians."),
]);
doc += api("new_wave(start, end), new_wave(start, end, options)", "A wavy line from start to end, each wave a curve rising to its left and falling to its right: up first, going left to right across the page.", [
	vec_arg("start", "Where it starts."),
	vec_arg("end", "Where it ends."),
	opt("options.amplitude", "a twentieth of its length", "How far it rises and falls; negative falls first.", "a number."),
	opt("options.waves", "3", "How many waves.", "a whole number from 1 to 10000."),
]);
doc += api("path.bounds()", "What the path's lines cover: #{ min, max, center, width, height }. Fails for an empty path.");
doc += api("path.translate_by(offset)", "Moves the path.", [
	offset_arg(),
]);
doc += api("path.scale_by(factor)", "Scales the path about the origin.", [
	arg("factor", "How many times bigger: below 1 shrinks, and negative flips through the origin.", "a number."),
]);
doc += api("path.rotate_around(point, angle)", "Turns the path about a point.", [
	vec_arg("point", "The centre of the turn."),
	angle_arg("How far to turn."),
]);
doc += api("path.copy()", "An independent copy of the path.");
doc += api("path.hatch(spacing), path.hatch(spacing, options)", "Shading for the inside of a closed path, as a new path (the path itself is unchanged), each line a stroke of its own. Every subpath is taken as a closed shape, and shapes inside or across others cut holes in them.", [
	arg("spacing", "How far apart the lines are.", "a positive number."),
	opt("options.angle", "PI() / 4", "Which way the lines run.", "a number, in radians, or an array of them for a layer each (cross-hatching)."),
	opt("options.roughness", "0", "How far each line's ends are jittered, at most.", "a number of at least 0."),
	opt("options.bow", "0", "How far each line bends from straight, at most.", "a number of at least 0."),
	opt("options.inset", "0", "How far inside the outline lines stop.", "a number of at least 0."),
	opt("options.fill_rule", "holes", "Which parts of overlapping shapes are inside.", "holes (shapes inside or across others cut holes in them), evenodd (which also leaves out where a single subpath crosses itself) or nonzero (overlapping shapes merge, as SVG fills do)."),
]);
doc += api("path.erase_inside(shape), path.erase_inside(shape, options)", "Cuts away the parts of the path a closed shape covers, as lines behind a figure are, in place; gives the same path. Lines and curves are cut where they cross the shape's outline and stay lines and curves; pieces that run on stay one stroke, and a closed subpath the shape doesn't touch stays closed.", [
	arg("shape", "What covers the path. Every subpath is taken as a closed shape, as for hatching.", "a path, all its points finite."),
	opt("options.fill_rule", "holes", "Which parts of overlapping shapes are inside.", "holes, evenodd or nonzero, as for hatch."),
]);
doc += api("path.erase_outside(shape), path.erase_outside(shape, options)", "Cuts away the parts of the path outside a closed shape, keeping what it covers, in place; gives the same path. Cut as erase_inside cuts.", [
	arg("shape", "What to keep the path inside. Every subpath is taken as a closed shape, as for hatching.", "a path, all its points finite."),
	opt("options.fill_rule", "holes", "Which parts of overlapping shapes are inside.", "holes, evenodd or nonzero, as for hatch."),
]);
doc += api("path.points_along(count), path.points_along(count, options)", "count points evenly spaced along a path of one subpath, by length, each #{ position, angle }: where it is, and which way the path runs there (the angle a glyph turned to the path has). On a path that comes back to its start, as circles do, they go once round without doubling up at the start; on an open one the first and last are at its ends.", [
	arg("count", "How many points.", "a whole number from 0 to 1000000."),
	opt("options.start", "0", "Where the first is, as a fraction of the length: 0.75 is the top of a circle, which starts at its right and runs clockwise.", "a number from 0 to 1."),
]);
doc += api("path.subpaths()", "Each subpath, from one move to the next, as a path of its own, in order: to give each piece of art its own style, say. Moves that draw nothing are left out.");
doc += api("path.is_empty", "Read only: whether the path has nothing in it, as a new moon's lit side.");
doc += api("path.to_string(), path.to_debug()", "The path's commands as text, for print and debug.");

// ---- brushes and styles ----

doc += heading("Styles");
doc += para("A style is what strokes are drawn in: a brush style, the ink (a colour, a Darkly brush, how wide its marks are and how it presses), and a handwriting style, the hand (how it goes over its strokes, and sets glyphs and lines). A style's settings are its two parts', read and set on the style as one: style.color, style.passes. Styles and their parts are values: changing one changes that copy only. Every jitter is in canvas units (rotations in radians, scales a fraction), drawn from the journal's generator as strokes are added to a canvas.");
doc += api("new_style(brush_style, handwriting_style)", "A style: the ink and the hand. Its properties are both parts', below.", [
	arg("brush_style", "The ink; the style keeps a copy.", "a brush style, from new_brush_style."),
	arg("handwriting_style", "The hand; the style keeps a copy.", "a handwriting style, from new_handwriting_style."),
]);
doc += api("default_style()", "The plainest style: black, in an Ink Pen 1.2 wide pressing from 0.3 to 0.7, by a steady hand. Text calls, roles and borders given a map of style settings take them over this.");
doc += api("brush_names()", "The name of every Darkly brush, sorted.");
doc += api("new_brush_style(color, brush, brush_size), new_brush_style(color, brush, brush_size, options)", "A brush style. Pressure runs from min_pressure to max_pressure along each stroke, as its pressure_profile says. Options set its properties (below) as setting them does. In SVG output (--svg) a stroke is brush_size * max_pressure wide.", [
	arg("color", "The ink.", "a CSS colour or a gradient."),
	arg("brush", "Which Darkly brush draws.", "one of brush_names()."),
	arg("brush_size", "How wide its marks are, in canvas units.", "a positive number."),
	opt("options.min_pressure", "0.3", "As brush_style.min_pressure.", "as brush_style.min_pressure."),
	opt("options.max_pressure", "0.7", "As brush_style.max_pressure.", "as brush_style.max_pressure."),
	opt("options.pressure_profile", "ramp", "As brush_style.pressure_profile.", "as brush_style.pressure_profile."),
	opt("options.pressure_wander", "0", "As brush_style.pressure_wander.", "as brush_style.pressure_wander."),
	opt("options.pressure_wander_length", "12", "As brush_style.pressure_wander_length.", "as brush_style.pressure_wander_length."),
	opt("options.pool", "0", "As brush_style.pool.", "as brush_style.pool."),
	opt("options.pool_depth", "0.3", "As brush_style.pool_depth.", "as brush_style.pool_depth."),
	opt("options.color_jitter", "0", "As brush_style.color_jitter.", "as brush_style.color_jitter."),
]);
doc += api("brush_style.color = color", "The ink.", [
	arg("color", "The ink.", "a CSS colour or a gradient."),
]);
doc += api("brush_style.brush = name", "The Darkly brush.", [
	arg("name", "Which brush draws.", "one of brush_names()."),
]);
doc += api("brush_style.brush_size = number", "How wide its marks are.", [
	arg("number", "The width, in canvas units.", "a positive number."),
]);
doc += api("brush_style.min_pressure = number, brush_style.max_pressure = number", "The least and most pressure along a stroke: heavier marks the harder it presses, how much depending on the brush.", [
	arg("number", "The pressure.", "a number from 0 to 1, min_pressure at most max_pressure (checked once a constructor's options are all set)."),
]);
doc += api("brush_style.pressure_profile = name", "How pressure runs along each stroke.", [
	arg("name", "The profile.", "ramp (min to max, the default), taper (min to max at the middle and back), constant (max all along) or pointed (by direction, as a pointed nib: max going down, min going up or across)."),
]);
doc += api("brush_style.pressure_wander = number", "Pressure wanders along a stroke, so ramps and tapers look less mechanical.", [
	arg("number", "How far it wanders either way, as a fraction of the pressure. Default 0.", "a number from 0 up to, not including, 1."),
]);
doc += api("brush_style.pressure_wander_length = number", "How far apart the wandering pressure rises and falls.", [
	arg("number", "The distance, in canvas units. Default 12.", "a positive number."),
]);
doc += api("brush_style.pool = number", "Ink pooling: strokes heavier at the very bottom of each path (each glyph, for text).", [
	arg("number", "How much heavier, as a fraction: 0.4 is 40% heavier. Default 0.", "a number of at least 0."),
]);
doc += api("brush_style.pool_depth = number", "How far up the pooling rises.", [
	arg("number", "The height, as a fraction of the path's. Default 0.3.", "a number above 0 and at most 1."),
]);
doc += api("brush_style.color_jitter = number", "Each stroke a little off its colour, each its own way: every subpath, so every pass of a stroke gone over, too. Colours move in OKLab, so steps look alike whatever the colour: lightness up to number either way, hue and chroma (OKLab's a and b) up to half that times the colour's lightness, so dark inks keep their hue, alpha kept. A gradient's colours move with its stroke. Drawn from the journal's generator as strokes are added to a canvas.", [
	arg("number", "How far lightness moves, OKLab lightness running from 0 (black) to 1 (white): 0.05 is a slight variation. Default 0.", "a number from 0 to 1."),
]);
doc += api("new_handwriting_style(), new_handwriting_style(options)", "A handwriting style: one clean pass, nothing astray, unless the options set its properties (below), as setting them does.", [
	opt("options.passes", "1", "As handwriting_style.passes.", "as handwriting_style.passes."),
	opt("options.pass_end_jitter", "0", "As handwriting_style.pass_end_jitter.", "as handwriting_style.pass_end_jitter."),
	opt("options.stroke_point_jitter", "0", "As handwriting_style.stroke_point_jitter.", "as handwriting_style.stroke_point_jitter."),
	opt("options.glyph_position_jitter", "0", "As handwriting_style.glyph_position_jitter.", "as handwriting_style.glyph_position_jitter."),
	opt("options.glyph_rotation_jitter", "0", "As handwriting_style.glyph_rotation_jitter.", "as handwriting_style.glyph_rotation_jitter."),
	opt("options.glyph_scale_jitter", "0", "As handwriting_style.glyph_scale_jitter.", "as handwriting_style.glyph_scale_jitter."),
	opt("options.baseline_wander", "0", "As handwriting_style.baseline_wander.", "as handwriting_style.baseline_wander."),
]);
doc += api("handwriting_style.passes = number", "Draws each stroke this many times over, as a hand going over its writing.", [
	arg("number", "How many passes. Default 1.", "a whole number from 1 to 64."),
]);
doc += api("handwriting_style.pass_end_jitter = number", "How far each pass's ends stray, each its own way, so a pass shifts, turns and stretches. With one pass it just roughens a hand.", [
	arg("number", "How far, in canvas units. Default 0.", "a number of at least 0."),
]);
doc += api("handwriting_style.stroke_point_jitter = number", "How far every point of every pass strays, control points included, each its own way, so curves change shape rather than just shifting. On top of pass_end_jitter.", [
	arg("number", "How far, in canvas units. Default 0.", "a number of at least 0."),
]);
doc += api("handwriting_style.glyph_position_jitter = number", "How far each glyph text calls place strays, at most, either way.", [
	arg("number", "How far, in canvas units. Default 0.", "a number of at least 0, or [across, down]."),
]);
doc += api("handwriting_style.glyph_rotation_jitter = number", "How far each placed glyph turns, at most, either way.", [
	arg("number", "How far, in radians. Default 0.", "a number of at least 0."),
]);
doc += api("handwriting_style.glyph_scale_jitter = number", "How much each placed glyph's size changes, at most, either way.", [
	arg("number", "How much, as a fraction. Default 0.", "a number from 0 up to, not including, 1."),
]);
doc += api("handwriting_style.baseline_wander = number", "How far the lines write_line and write_block write along wander up and down from straight: smoothly, at random, starting where they're put and bending every few glyph scales. Glyphs follow them.", [
	arg("number", "How far, at most, either way, in canvas units. Default 0.", "a number of at least 0."),
]);
doc += api("style.SETTING = value", "Any brush style or handwriting style setting, read and set on the style: style.color, style.brush_size, style.passes, style.glyph_rotation_jitter and the rest, each taking what it takes above.", [
	arg("value", "The setting's value.", "as its part takes it."),
]);

// ---- gradients ----

doc += heading("Gradients");
doc += api("new_gradient(colors)", "A gradient along each stroke from its start to its end, to use in place of a colour.", [
	arg("colors", "The colours, in order. Unplaced colours at the ends sit at 0 and 1, and the others evenly between their placed neighbours.", "an array of at least two, each a CSS colour or [position, colour] with the position from 0 to 1; positions mustn't go back."),
]);
doc += api("new_gradient(colors, options)", "A gradient laid out another way: give from and to, center and radius, or vertical, or no key for along each stroke. In Darkly renders a gradient stroke keeps its brush's texture; in SVG, gradients along or down a stroke are drawn in short pieces.", [
	arg("colors", "The colours, as above.", "an array of at least two colours or [position, colour] pairs."),
	opt("options.from", "none", "With to: the gradient runs across the page from this point.", "a vec, not the same as to."),
	opt("options.to", "none", "With from: the point it runs to.", "a vec."),
	opt("options.center", "none", "With radius: the gradient spreads out from this point.", "a vec."),
	opt("options.radius", "none", "With center: how far it spreads.", "a positive number."),
	opt("options.vertical", "false", "True runs the gradient down each path (each glyph, for text) from its top to its bottom.", "true or false; true takes no other key."),
]);
doc += api("gradient.to_string(), gradient.to_debug()", "The gradient as text, for print and debug.");

// ---- glyphsets and glyphs ----

doc += heading("Glyphsets and Glyphs");
doc += para("Glyphsets come from the glyphset files, glyphsets/NAME.json, the journal's own and the library's, one per glyphset (copper.json is the glyphset copper), each with its glyphs and how text is set in it by default; sets of symbols are glyphsets too. A glyphset is a value: changing one changes that copy. A character is looked for as it is in a glyphset and its fallbacks, then in the other case, so a glyphset of capitals sets lowercase text in them. A type style can override a glyphset's settings for the text it sets (see Text).");
doc += api("glyphset(name)", "A glyphset, as its file sets it.", [
	arg("name", "Which glyphset.", "one of glyphset_names()."),
]);
doc += api("glyphset(name, options)", "A glyphset with settings changed, as setting the properties below does.", [
	arg("name", "Which glyphset.", "one of glyphset_names()."),
	opt("options.glyph_outline_gap", "the file's", "Room between glyphs' outlines, in glyph scales.", "a number of at least 0."),
	opt("options.space_width", "the file's", "A space's width, in glyph scales.", "a positive number."),
	opt("options.skew", "the file's", "Leans the glyphs into italics, tops forward.", "a number, in radians, between -PI() / 2 and PI() / 2."),
	opt("options.joined", "true if the file gives joins", "As glyphset.joined.", "as glyphset.joined."),
	opt("options.fallback", "the file's", "Where characters the glyphset hasn't are looked for.", "a glyphset, a glyphset's name, or () for nowhere."),
]);
doc += api("glyphset_names()", "Every glyphset's name, sorted.");
doc += api("ternary_glyphset(name), ternary_glyphset(name, options)", "A glyphset whose letters are balanced-ternary digits: a to m are 1 to 13, n to z are -13 to -1.", [
	arg("name", "The glyphset whose glyphs draw the digits.", "one of glyphset_names()."),
	opt("options", "none", "The glyphset's settings, as glyphset(name, options) takes them.", "a map of glyph_outline_gap, space_width, skew, joined and fallback."),
]);
doc += api("glyphset.name", "Read only: the glyphset's name.");
doc += api("glyphset.glyph_outline_gap = number", "Text is set with each glyph as close to the last as their outlines allow, plus this.", [
	arg("number", "The room between outlines, in glyph scales.", "a number of at least 0."),
]);
doc += api("glyphset.space_width = number", "A space's width.", [
	arg("number", "The width, in glyph scales.", "a positive number."),
]);
doc += api("glyphset.skew = angle", "Leans the glyphset's glyphs, and its fallbacks', into italics. Spacing isn't opened up.", [
	arg("angle", "How far they lean, tops forward; negative leans them back.", "a number, in radians, between -PI() / 2 and PI() / 2."),
]);
doc += api("glyphset.joined = on", "Whether a cursive glyphset's words are joined up, each written in one stroke: each glyph's first stroke, joined to the next's by a stroke along both, then their lifts (dots and crossbars). Glyphs join where the file's joins says they do and they're less than half a glyph scale apart, end to start, so text set apart (at a line's end, round what it avoids, by hand) isn't joined; blanks, and changes of ink in marked-up text, break joins. Text calls and canvas.add(glyphs, style) join glyphs; canvas.add(glyph, style) and glyph.path() don't. A glyph after one in the file's leave_high is drawn as a variant entered high, if it has one.", [
	arg("on", "Whether they join. Default true for a glyphset whose file gives joins.", "true or false; true only for a glyphset whose file gives joins."),
]);
doc += api("glyphset.fallback = glyphset", "Where characters the glyphset hasn't are looked for.", [
	arg("glyphset", "The fallback.", "a glyphset, a glyphset's name, or () for nowhere."),
]);
doc += api("glyphset.characters()", "The characters the glyphset has glyphs for.");
doc += api("glyphset.glyphs(text)", "A text's glyphs, each at the origin with scale 1, variants picked from the seeded generator, looked for in the fallbacks too.", [
	arg("text", "The text to set.", "a string; characters with no glyph anywhere come out as X (see missing_glyphs)."),
]);
doc += api("glyphset.glyph(name), glyphset.glyph(name, options)", "One glyph, its first variant, from the glyphset's own glyphs (not its fallbacks), placed as the options say.", [
	arg("name", "Which glyph.", "a character or a glyph's name, as a string, a number (as ternary.glyph(-13)), or a balanced-ternary value (its number)."),
	opt("options.position", "the origin", "As glyph.position.", "as glyph.position."),
	opt("options.scale", "1", "As glyph.scale.", "as glyph.scale."),
	opt("options.rotation", "0", "As glyph.rotation.", "as glyph.rotation."),
	opt("options.skew", "0", "As glyph.skew.", "as glyph.skew."),
]);
doc += api("glyphset.variants(name)", "Every variant of one of the glyphset's own glyphs.", [
	arg("name", "Which glyph.", "a character or a glyph's name, as a string, or a number."),
]);
doc += api("missing_glyphs()", "Characters drawn with the fallback X so far, for want of a glyph: #{ CHARACTER: #{ count, tried } }. journalbuild --strict-glyphs fails on them instead.");
doc += para("A glyph is a symbol placed with a position, scale, rotation and skew. Its path spans -1 to 1 and is drawn at half its scale, so scale is its size in canvas units.");
doc += api("glyph.position = vec", "Where its centre is.", [
	vec_arg("vec", "The centre."),
]);
doc += api("glyph.scale = number", "Its size.", [
	arg("number", "The size, in canvas units.", "a number."),
]);
doc += api("glyph.rotation = angle", "Its turn about its own centre.", [
	angle_arg("The turn."),
]);
doc += api("glyph.skew = angle", "Its lean about its centre, before it's turned.", [
	arg("angle", "How far it leans, top forward.", "a number, in radians, between -PI() / 2 and PI() / 2."),
]);
doc += api("glyph.is_blank", "Read only: whether it draws nothing, as a space.");
doc += api("glyph.translate_by(offset)", "Moves it. Returns the glyph.", [
	offset_arg(),
]);
doc += api("glyph.scale_by(factor)", "Multiplies its scale. Returns the glyph.", [
	arg("factor", "How many times bigger.", "a number."),
]);
doc += api("glyph.rotate_by(angle)", "Turns it further. Returns the glyph.", [
	angle_arg("How much further."),
]);
doc += api("glyphs.translate_by(offset), glyphs.scale_by(factor), glyphs.rotate_around(point, angle)", "An array of glyphs moved, scaled about the origin (positions and sizes alike), or turned about a point, as one: each glyph changed. Returns the array.", [
	offset_arg(),
	arg("factor", "How many times bigger.", "a number."),
	vec_arg("point", "The centre of the turn."),
	angle_arg("How far to turn."),
]);
doc += api("glyph.path()", "Its strokes, placed, as a new path; empty for blanks.");
doc += api("glyph.copy()", "An independent copy of the glyph.");

// ---- text ----

doc += heading("Text");
doc += para("Text layouts place glyphs, scaling each by the glyph scale and turning it to its line on top of any scale and rotation it has. Every text call takes one settings map: a style (what the glyphs are drawn in, and the hand that sets them), a type (which glyphs, how big, shaped how) and a paragraph (how lines are built and set), each whole or a map of some of its settings, and the call's own options. A map of style settings is over default_style(); of type settings, over no glyphset and no glyph scale; of paragraph settings, over the defaults. A call uses the settings it needs and ignores the rest, spans among them, so a role or a text_style(name) can be passed straight in; unknown keys are errors.");
doc += listing("Settings every text call takes:", [
	opt("settings.style", "default_style()", "What the glyphs are drawn in; its hand's glyph jitters set each a little off true, and its baseline_wander bends write_line's and write_block's lines.", "a style, or a map of style settings."),
	opt("settings.type", "none", "The glyphset and glyph scale, and how glyphs are shaped and spaced. Calls that set text from a string need its glyphset; all but fit_text and art::spiral_text need its glyph_scale.", "a type style, or a map of type settings."),
	opt("settings.paragraph", "the defaults below", "How lines are built and set.", "a paragraph style, or a map of paragraph settings."),
]);
doc += api("new_type_style(glyphset, glyph_scale), new_type_style(glyphset, glyph_scale, options)", "A type style: the glyphs text is set in and their scale, shaped as the options say, as setting the properties below does. Settings left unset are the glyphset's.", [
	arg("glyphset", "The glyphs text is set in.", "a glyphset, from glyphset or ternary_glyphset."),
	arg("glyph_scale", "The size glyphs are set at, in canvas units.", "a positive number."),
	opt("options.glyph_scale_factor", "1", "As type_style.glyph_scale_factor.", "as type_style.glyph_scale_factor."),
	opt("options.glyph_outline_gap", "the glyphset's", "As type_style.glyph_outline_gap.", "as type_style.glyph_outline_gap."),
	opt("options.glyph_tracking", "0", "As type_style.glyph_tracking.", "as type_style.glyph_tracking."),
	opt("options.space_width", "the glyphset's", "As type_style.space_width.", "as type_style.space_width."),
	opt("options.skew", "the glyphset's", "As type_style.skew.", "as type_style.skew."),
	opt("options.italic", "the glyphset's", "As type_style.italic.", "as type_style.italic."),
	opt("options.joined", "the glyphset's", "As type_style.joined.", "as type_style.joined."),
	opt("options.underline", "false", "As type_style.underline.", "as type_style.underline."),
	opt("options.strike", "false", "As type_style.strike.", "as type_style.strike."),
]);
doc += api("type_style.glyphset = glyphset", "The glyphs text is set in.", [
	arg("glyphset", "The glyphset.", "a glyphset."),
]);
doc += api("type_style.glyph_scale = number", "The size glyphs are set at.", [
	arg("number", "The size, in canvas units.", "a positive number."),
]);
doc += api("type_style.glyph_scale_factor = number", "Multiplies the glyph scale. In a span, it multiplies the size of the text round it.", [
	arg("number", "How many times bigger. Default 1.", "a positive number."),
]);
doc += api("type_style.glyph_outline_gap = number", "Room between glyphs' outlines, over the glyphset's; glyphs from its fallbacks take it too in spans.", [
	arg("number", "The room, in glyph scales.", "a number of at least 0."),
]);
doc += api("type_style.glyph_tracking = number", "Extra room after each glyph; negative tightens.", [
	arg("number", "The room, in glyph scales. Default 0.", "a number above -0.5."),
]);
doc += api("type_style.space_width = number", "A space's width, over the glyphset's.", [
	arg("number", "The width, in glyph scales.", "a positive number."),
]);
doc += api("type_style.skew = angle, type_style.italic = on", "Leans the glyphs, over the glyphset's skew: italic true leans them 0.25 radians, false sets them upright.", [
	arg("angle", "How far they lean, tops forward.", "a number, in radians, between -PI() / 2 and PI() / 2."),
	arg("on", "Whether the text is italic.", "true or false."),
]);
doc += api("type_style.joined = on", "Whether a cursive glyphset's words are joined up, over the glyphset's (see glyphset.joined).", [
	arg("on", "Whether they join; true joins only a glyphset whose file gives joins.", "true or false."),
]);
doc += api("type_style.underline = lines, type_style.strike = lines", "Lines under the text, one below another, or through it, back and forth as one stroke, scribbling it out. Drawn where text is typeset (flows and labels, and spans in markdown); lines run across the spaces between words and break where the text does.", [
	arg("lines", "Which lines.", "true for one, false for none, a number of lines from 1 to 20, or #{ count, spacing, style }: count lines (default 1), spacing glyph scales apart (default as the pen sets them), drawn in style, a style or a map of style settings over the text's (default the text's)."),
]);
doc += api("new_paragraph_style(), new_paragraph_style(options)", "A paragraph style: the defaults below unless the options set them, as setting the properties does.", [
	opt("options.KEY", "as below", "Any of the properties below.", "as each takes."),
]);
doc += api("paragraph_style.line_height = number", "How far apart lines are, in glyph scales.", [
	arg("number", "Default 1.25.", "a positive number."),
]);
doc += api("paragraph_style.paragraph_gap = number, paragraph_style.indent = number", "Extra room between paragraphs, in lines (default 0.5); and how much further in a paragraph's first line starts, in glyph scales (default 0). Typeset text.", [
	arg("number", "The room.", "a number of at least 0."),
]);
doc += api("paragraph_style.align = name", "Where each line's glyphs go along it: from its start; moved along as far as they still fit, or half that; or spread so the last goes as far as it fits (at the blanks, with layout: words), all but the text's last line.", [
	arg("name", "Default left.", "left, center, right or justify."),
]);
doc += api("paragraph_style.valign = name", "Where text sits down a text box or slot: from as high as its glyphs fit, or set again lower by half, or all, of the room left under its last line.", [
	arg("name", "Default top.", "top, center or bottom."),
]);
doc += api("paragraph_style.fit = on", "Shrinks the text until all of it fits its slot. Typeset text.", [
	arg("on", "Default false.", "true or false."),
]);
doc += api("paragraph_style.layout = name", "How glyphs are placed: glyph by glyph, or words kept whole, wrapping at spaces.", [
	arg("name", "Default the call's: words for write_block, flat otherwise.", "flat or words."),
]);
doc += api("paragraph_style.angle = angle", "In a text box, which way its lines run (0, the default, left to right, each line below the last; other angles turn them, the next line always to their right-hand side). Along a path, holds every glyph at this angle instead of turning it with the path (0 is upright).", [
	arg("angle", "Default none: 0 in a box, turned to the path along one.", "a number, in radians."),
]);
doc += api("paragraph_style.split_words = setting, paragraph_style.split_padding = number", "With layout: words, breaks words round what's avoided rather than moving them past it whole; and how much further the pieces keep from it.", [
	arg("setting", "Default false.", "true, false, or the fewest glyphs a piece may have (a whole number of at least 1; true is 2)."),
	arg("number", "The extra room, in glyph scales. Default 0. Needs split_words.", "a number of at least 0."),
]);
doc += api("paragraph_style.clearance = number", "How far glyphs keep from what they avoid, on top of any margin a tree was built with.", [
	arg("number", "In canvas units. Default 3.", "a number of at least 0."),
]);
doc += api("paragraph_style.line_start_jitter = number, paragraph_style.line_gap_jitter = number", "How much further each line starts, at most, at random; and how much each line's distance from the last varies, at most, either way (write_block and typeset text).", [
	arg("number", "In canvas units. Default 0.", "a number of at least 0."),
]);
doc += api("new_textbox(settings)", "A box, or any closed shape, glyphs flow into, line by line, each glyph's own ink wholly inside the shape: lines run from where the first glyph fits to where the last does, so ink meets the shape's edges, and the first line starts as near its top as glyphs fit. Takes the type's glyph_scale and glyph_tracking, the paragraph's layout (default flat), align, valign, angle, line_start_jitter, clearance and splitting, and the style's glyph jitters.", [
	opt("settings.center", "none: give center, width and height, or shape", "The box's centre.", "a vec."),
	opt("settings.width", "none: needed with center", "Its width.", "a positive number."),
	opt("settings.height", "none: needed with center", "Its height.", "a positive number."),
	opt("settings.shape", "none: give it, or center, width and height", "The shape to fill instead of a box. Each subpath is an outline, closed back to its start, filled where it winds round (so one drawn on past its start, as circles are, is still filled); a place inside an even number of outlines, as in one inside another, is outside. A rectangle at angle 0 fills just as a box does.", "a path with a subpath of at least three points."),
	avoid_opt(""),
]);
doc += api("textbox.fill(glyphs)", "Places what fits: #{ placed, unplaced }, arrays of glyphs. Add the placed ones to a canvas with canvas.add(placed, style).", [
	arg("glyphs", "The glyphs to place, in order.", "an array of glyphs, as glyphset.glyphs gives."),
]);
doc += api("textbox.place_all(glyphs)", "Fills the box again and again until every glyph is placed: an array of arrays of placed glyphs, one per box. Fails if a glyph won't fit even an empty box.", [
	arg("glyphs", "The glyphs to place, in order.", "an array of glyphs."),
]);
doc += api("path.fill_with_text(text, settings)", "The text along the path, each glyph turned to it (or held at the paragraph's angle), over as many copies of the path as it takes: an array of canvases, one per copy. Takes the type's glyphset, glyph_scale and glyph_tracking, the paragraph's layout (default flat; words aren't split between subpaths), angle, clearance and splitting, and the style. Fails if a glyph won't fit the path at all.", [
	arg("text", "The text to write.", "a string."),
	avoid_opt(""),
]);
doc += api("path.fit_text(text, settings)", "The text along the path at the largest glyph scale that fits all of it, as a canvas; a glyph_scale is ignored. Otherwise as fill_with_text.", [
	arg("text", "The text to write.", "a string."),
	avoid_opt(""),
]);
doc += api("write_line(text, at, settings)", "The text written from at to the right, on the line through at (wandering by the hand's baseline_wander), as a canvas: glyphs' centres on it, as along any path. Otherwise as fill_with_text.", [
	arg("text", "The text to write.", "a string."),
	vec_arg("at", "Where the line starts."),
	avoid_opt(""),
]);
doc += api("write_block(text, at, width, settings)", "The text wrapped into lines width long, the first from at (on its line, as write_line sets it) and each line_height glyph scales below the last (give or take line_gap_jitter), each wandering by the hand's baseline_wander, the left edges ragged by line_start_jitter, words kept whole unless the paragraph's layout is flat. Gives #{ canvas, bottom }: the text, and where a line after it would go, for stacking blocks.", [
	arg("text", "The text to write.", "a string."),
	vec_arg("at", "Where the first line starts."),
	arg("width", "How long lines are.", "a positive number."),
	avoid_opt(""),
]);
doc += api("canvas.scatter_notes(messages, width_range, height_range, settings)", "Scatters messages over the canvas in random boxes, clear of its strokes and of each other; gives a canvas of just the messages. Each note's glyphs turn and scale by the hand's glyph_rotation_jitter and glyph_scale_jitter before notes are placed, and stray by its glyph_position_jitter after. Fails for an empty canvas.", [
	arg("messages", "The notes, a box each.", "an array of strings."),
	arg("width_range", "Box widths to try, at random between the two.", "[least, most]: positive numbers, least at most most."),
	arg("height_range", "Box heights to try.", "[least, most]: positive numbers, least at most most."),
	opt("settings.max_attempts", "100", "How many boxes are tried for each message before backing off.", "a whole number of at least 1."),
	avoid_opt(", besides the canvas and other notes"),
]);

// ---- avoidance ----

doc += heading("Keeping Text Clear");
doc += para("avoid, in every text layout, is a tree, a path, a canvas or a glyph, or an array of any of these. Glyphs keep their whole outline (the hull of their strokes) at least the paragraph's clearance from all of it (3 canvas units unless it says), on top of any margin a tree was built with. Glyphs placed by one call keep clear of each other too.");
doc += api("new_avoidance_tree()", "An empty tree of what to keep clear of. Trees are shared: a text box made with one sees what's added to it later, so tree.add(placed) keeps later text off what's just been placed.");
doc += api("tree.add(item), tree.add(item, options)", "Adds what to keep clear of, as it is now. Returns the tree.", [
	arg("item", "What to keep clear of.", "a path (its lines), a canvas (its marks), a glyph (its outline, insides included), another tree (with its own margins), or an array of any of these."),
	opt("options.margin", "0", "How far to keep from it, in canvas units, on top of the clearance text asks for.", "a number of at least 0."),
]);
doc += api("tree.add_hull(item), tree.add_hull(item, options)", "Keeps out of everything inside the item's outline, not just off its marks. Returns the tree.", [
	arg("item", "What to take the outline of.", "a path, a canvas or a glyph, or an array of any of these; not a tree."),
	opt("options.margin", "0", "How far to keep from the outline, in canvas units.", "a number of at least 0."),
]);
doc += api("tree.add_region(path), tree.add_region(path, options)", "Keeps out of a closed shape. Returns the tree.", [
	arg("path", "The shape.", "a path."),
	opt("options.margin", "0", "How far to keep from it, in canvas units.", "a number of at least 0."),
]);
doc += api("tree.blocks(point), tree.blocks(glyph)", "Whether something is too close to what's in the tree.", [
	arg("point or glyph", "What to test.", "a vec, or a placed glyph (a blank one is never blocked)."),
]);
doc += api("tree.is_empty", "Read only: whether the tree holds nothing.");
doc += api("tree.copy()", "An independent copy of the tree.");

// ---- randomness and messages ----

doc += heading("Randomness and Messages");
doc += api("seed(n)", "Reseeds the generator that random, glyph variants, text layout and hatching draw on.", [
	arg("n", "Where the generator starts again: the same seed gives the same numbers after it.", "a whole number."),
]);
doc += api("random()", "A decimal from 0 up to 1.");
doc += api("random_range(low, high)", "A number from low up to, not including, high.", [
	arg("low", "The least it can give.", "a number."),
	arg("high", "What it stays below. Whole numbers at both ends give a whole number, and a decimal at either end gives a decimal.", "a number above low."),
]);
doc += api("warn(message)", "Reports something, with the script line, without stopping the script. Errors give the script line too.", [
	arg("message", "What to report.", "a string."),
]);
doc += api("assert(condition, message)", "Stops the script, with the message and the script line, unless the condition holds: for checking a journal's own sums.", [
	arg("condition", "What must hold.", "true or false."),
	arg("message", "What went wrong, if it doesn't.", "a string."),
]);

// ---- generators ----

doc += heading("Generators");
doc += para("Points to draw from, and the trees that join them. Distributions and the depth_first and random trees draw on the script's generator, so seed(n) fixes them.");
doc += api("distribute_points(kind, count, bounds)", "Exactly count points scattered through a box, as an array of vec2s, vec3s or vec4s: as many coordinates as bounds has pairs.", [
	arg("kind", "How they're scattered. uniform_random: each anywhere, so clumps and gaps. poisson_disk: evenly at random, none close, by weighted sample elimination. jittered_grid: one at random in each of count cells of a grid. grid: the cells' centres, row by row, x fastest, every cell when count fills the grid. halton: the Halton sequence, quasi-random and even, shifted at random. golden: the generalised golden-ratio sequence, like halton but more even.", "uniform_random, poisson_disk, jittered_grid, grid, halton or golden."),
	arg("count", "How many points.", "a whole number from 0 to 1000000."),
	arg("bounds", "The box: the least and greatest each coordinate can be, x first.", "an array of 2, 3 or 4 [low, high] pairs of numbers, low below high."),
]);
doc += api("points.spanning_tree(), points.spanning_tree(options)", "A tree joining every point by one way only, as an array of [parent, child] index pairs into points, each pointing away from the root, in the order the algorithm added them: draw them in order and the tree grows. Every pair is compared, so at most 20000 points. Under 2 points gives [].", [
	arg("points", "What to join.", "an array of vec2s, vec3s or vec4s, all one size."),
	opt("options.algorithm", "prim", "Which tree. prim (or minimum): the shortest in all, grown from the root a nearest point at a time. kruskal: the same tree, its edges shortest first, fragments joining up. maximum: the longest in all, long crossing lines. shortest_path: each point by its shortest way to the root, veins radiating out. breadth_first: by hops from the root, nearest neighbours first. depth_first: a random walk deep before it backtracks, long corridors. random: a uniformly random tree, an even maze. The last four go between neighbours only.", "prim, minimum, kruskal, maximum, shortest_path, breadth_first, depth_first or random."),
	opt("options.metric", "euclidean", "How far apart points are. euclidean: straight. squared_euclidean: straight, squared, the same minimum tree but shortest paths take many short hops. manhattan: along the axes, as streets in a grid. chebyshev: the farthest along any one axis.", "euclidean (or euclidian), squared_euclidean, manhattan (or taxicab) or chebyshev."),
	opt("options.root", "0", "The point the tree grows from.", "a whole number, the index of a point."),
	opt("options.neighbours", "4 per coordinate, less 2: 6 for vec2s", "For the neighbour algorithms, how many nearest points each point is joined to (and they to it); the shortest tree's edges are added too, so it's in one piece.", "a whole number of at least 1."),
]);
doc += api("nearest(points, point)", "The index of the point nearest point: a root for a tree, say.", [
	arg("points", "Where to look.", "a non-empty array of vecs, all one size."),
	arg("point", "What to be near.", "a vec of the same size."),
]);
doc += api("new_edges(points, pairs)", "A path of straight lines, one between the two points each pair indexes: a spanning tree drawn.", [
	arg("points", "The points.", "an array of vec2s."),
	arg("pairs", "Which to join.", "an array of [a, b] indices into points, as spanning_tree gives."),
]);

// ---- balanced ternary ----

doc += heading("Balanced Ternary");
doc += para("A balanced-ternary value is a number's digits, -1, 0 and 1, most significant first, each worth three times the next: 5 is +--, 9 - 3 - 1. It's a value: glyphsets draw it as a glyph and paths write it as marks.");
doc += api("balanced_ternary(n, options), balanced_ternary(text), balanced_ternary(digits)", "A balanced-ternary value: from a whole number, in as few digits as it needs (0 is 0) or at least options.width of them, or from its digits as given, leading 0s kept, as many as there are.", [
	arg("n", "The number.", "a whole number."),
	opt("options.width", "1", "The fewest digits: a shorter number is padded with 0s in front. balanced_ternary(5, #{ width: 5 }) is 00+--.", "a whole number of at least 1."),
	arg("text", "The digits as text: - for -1, 0, and + for 1.", "a non-empty string of -, 0 and +."),
	arg("digits", "The digits.", "a non-empty array of -1, 0 and 1."),
]);
doc += api("bt.value", "Read only: the whole number it is; an error for more digits than a whole number holds (over 39).");
doc += api("bt.digits()", "Its digits, an array of -1, 0 and 1.");
doc += api("bt.digits(width)", "Its digits in chunks of width, an array of arrays, the first padded with 0s in front so every chunk reads as a number: balanced_ternary(2026).digits(3) is [[0, 1, 0], [-1, 1, 0], [0, 0, 1]], 3, -6 and 1.", [
	arg("width", "Digits to a chunk.", "a whole number of at least 1."),
]);
doc += api("bt.to_string(), bt.to_debug(), a == b, a != b", "Its digits as text, such as +--, for print and debug; and whether two values have the same digits.");
doc += api("path.fill_with_text(bt, settings)", "The digits written along the path from its start, as the balanced_ternary glyphset writes them down its stems, the type's glyph scale apart, over as many copies of the path as it takes: an array of canvases, one per copy. A 0 is a bar across the path, and a -1 or 1 a stroke that runs alongside the path for most of its slot, then bends sharply out to the left or right of a traveller going along the path (for a path running left to right across the page, 1 is down). The path must be one subpath, and isn't drawn.", [
	arg("bt", "The digits.", "a balanced-ternary value."),
	opt("settings.style", "default_style()", "What the marks are drawn in.", "a style, or a map of style settings."),
	req("settings.type", "How far along the path each digit's slot runs: its glyph_scale.", "a type style or a map with a glyph_scale no longer than the path."),
	opt("settings.width", "0.6 of the slot", "How far the marks reach out from the path.", "a positive number."),
]);
doc += api("path.fit_text(bt, settings)", "The digits written along the whole path, each a share of its length, as a canvas; otherwise as above.", [
	arg("bt", "The digits.", "a balanced-ternary value."),
	opt("settings.style", "default_style()", "What the marks are drawn in.", "a style, or a map of style settings."),
	opt("settings.width", "0.6 of the slot", "How far the marks reach out from the path.", "a positive number."),
]);

// ---- art ----

doc += heading("Procedural Art");
doc += para("The art module draws shapes from a few parameters and a seed of their own: the same seed gives the same drawing, and the script's generator is neither used nor changed.");
doc += api("art::snake(head_center, tail_end, length), art::snake(head_center, tail_end, length, options)", "A snake seen from above, winding from its head to the tip of its tail. Gives #{ outline, spine }: its outline, which is several strokes, since where it crosses itself the strand beneath stops at the edge of the one on top (the head always goes over); and its centre line, from the back of its head to the tip of its tail, a path to write along.", [
	vec_arg("head_center", "Where the centre of its head is."),
	vec_arg("tail_end", "Where the tip of its tail is: near enough for a snake this long to reach."),
	arg("length", "How long it is along its spine.", "a number at least 4 times max_width."),
	opt("options.head_angle", "facing away from the tail", "Which way its head faces: 0 is +x, PI() / 2 down the page.", "a number, in radians."),
	opt("options.max_width", "length / 12", "How wide it is at its widest.", "a positive number."),
	seed_opt("winding"),
]);
doc += api("art::snake_along(path, max_width), art::snake_along(path, max_width, options)", "A many-headed serpent along a path, as a path: its body follows the path, slithering a little off it and forking where it forks, its tail at the dead end nearest the path's start and a head at every other.", [
	arg("path", "The lines its body follows.", "a path with lines."),
	arg("max_width", "How wide it is at its widest.", "a positive number."),
	seed_opt("serpent"),
]);
doc += api("art::bramble(base, length), art::bramble(base, length, options)", "A blackberry bramble. Gives #{ vines, leaves, flowers }: canes, thorns and stalks; leaflets and veins; flowers, buds and sepals, as paths to stroke in different colours (canvas.add(parts, styles) draws them). Parts in front hide the lines behind them.", [
	vec_arg("base", "Where the cane grows from."),
	arg("length", "How far it grows.", "a number at least 2 times flower_size."),
	opt("options.angle", "-PI() / 2, up the page", "Which way it grows: 0 is +x.", "a number, in radians."),
	opt("options.flower_size", "length / 5", "About how wide its flowers are, which everything else is sized by.", "a positive number."),
	seed_opt("bramble"),
] + thickening_opts("canes and branches", "as far as the cane could grow from the origin"));
doc += api("art::bramble_along(path, flower_size), art::bramble_along(path, flower_size, options)", "A bramble grown along a path, such as a maze's: vines follow its lines, branch at junctions and flower at its dead ends. Gives #{ vines, leaves, flowers }.", [
	arg("path", "The lines its vines follow.", "a path with lines, all its points finite."),
	arg("flower_size", "About how wide its flowers are, which everything else is sized by.", "a positive number."),
	seed_opt("bramble"),
] + thickening_opts("vines", "the farthest point of the path from the origin"));
doc += api("art::eye(center, length), art::eye(center, length, options)", "An eye as a hand sketches one. Gives #{ lines, accent, wash, rim, iris_radius }: lines, the lids gone over and running past the corners, the iris ringed, and a line through the iris bowed toward where it looks; accent, a second outline of the lids, to go round in the iris's colour; wash, a spiral filling the iris; rim, rings round the iris's edge where its wash pools; and iris_radius, a number, to size the wash's brushes by (a third of it suits the wash, an eighth the rim). The iris's parts are hidden where the lids cover it. Draw the colour before the lines, part by part: canvas.add(parts, styles) draws in name order, which would put the wash over the lines.", [
	vec_arg("center", "Its centre."),
	arg("length", "How long it is from corner to corner.", "a positive number."),
	opt("options.gaze_angle", "0", "Which way it looks, on the page: 0 is +x, -PI() / 2 up.", "a number, in radians."),
	opt("options.gaze", "0.5", "How far it looks toward gaze_angle: 0 straight ahead, 1 as far as it goes.", "a number from 0 to 1."),
	opt("options.messiness", "0.3", "From neat, each line drawn once, to as messy as it gets, lines gone over four or five times, wandering and overshooting. Neat lines are as few curves as their shape needs (a flat lid is two straight lines), so a style's point_jitter bends them whole; messier ones are drawn through more points, to wander.", "a number from 0 to 1."),
	seed_opt("eye"),
	opt("options.angle", "0", "The eye's tilt; 0 keeps the corners level.", "a number, in radians."),
	opt("options.openness", "0.32", "Its height over its length.", "a number above 0.05 and at most 1."),
	opt("options.flat", "false", "Lids of straight lines from each corner to a point above and below the middle, as the symbols glyphset's eye has, rather than arcs.", "true or false."),
]);
doc += api("art::spiral_text(text, center, radius, settings)", "The text written in a spiral out from the middle of a circle, clockwise from the top, each glyph turned to the spiral (or held at the paragraph's angle), as large as all of it fits, as a canvas. Glyphs keep one size until they'd come nearer the circle than the paragraph's clearance; from there each fills the room left between the circle and the glyphs on the turn inside it, corners and all, shrinking as that narrows, and the spiral ends where they'd be smaller than minimum_scale of the rest. Turns are the paragraph's line height apart, measured in the height of the text's tallest glyph rather than in glyph scales, so 1 sets turns touching. Takes the type's glyphset, glyph_scale and glyph_tracking, the paragraph's layout (default flat), angle, line_height, clearance and splitting, and the style. Fails if the text doesn't fit at any size.", [
	arg("text", "The text to write.", "a string."),
	vec_arg("center", "The circle's centre."),
	arg("radius", "The circle's radius: glyphs keep the clearance inside it.", "a number bigger than the clearance."),
	opt("settings.type.glyph_scale", "none: as large as fits", "The largest glyphs are set; smaller if the text doesn't fit at it.", "a positive number."),
	opt("settings.inner", "0.85 of the glyph scale", "How far from the centre the spiral starts.", "a number of at least 0."),
	opt("settings.minimum_scale", "1/3", "The smallest the outer turn's glyphs shrink to, as a fraction of the rest; 1 keeps them all one size.", "a positive number, at most 1."),
	avoid_opt(""),
]);
doc += api("art::moon(center, radius, phase)", "The moon in a phase, seen from the northern hemisphere. Gives #{ outline, terminator, lit, dark }: the disc's outline; the terminator, the half ellipse from the top of the disc to the bottom between light and dark (an empty path at new and full); and the lit and dark sides as closed shapes, to hatch or fill (the lit side empty at new, the dark side at full). It takes no seed: the same phase draws the same moon.", [
	vec_arg("center", "Its centre."),
	arg("radius", "Its radius.", "a positive number."),
	arg("phase", "How far through its month it is: 0 new, 0.25 first quarter (the right half lit), 0.5 full, 0.75 last quarter (the left half lit). Phases wrap, so 1.25 is 0.25 and -0.25 is 0.75.", "a number."),
]);

doc += api("art::huffman_stars(symbols, output_base, center, width, height), art::huffman_stars(symbols, output_base, center, width, height, options)", "A star chart whose constellation is a Huffman tree: the code the huffman lock would build from the symbols, rewriting them in output_base (ties go to the lower symbol, so it is the lock's code exactly). Its nodes are stars, bigger the more often their symbols come up (a branch weighs its symbols together, so the root is biggest, the rarest symbols a third its size, on a log scale); its branches are the links, each one digit of the code. The tree is grown out from its root, near the middle, at random angles and lengths, each subtree looking for room, then tidied by forces: each star has a space of its own (stars nearer than their spaces push apart, and off the chart's edge), each link a length of its own (longer links pull together), and links push away stars that aren't theirs. Links never cross, and no link passes nearer a star than its radius and the gap. Gives #{ stars, links, points, edges, nodes }: stars, a circle for every star, and links, a line for every link stopping gap short of its stars, as paths (canvas.add(parts, styles) draws them); points, the stars' centres, and edges, [parent, child] index pairs into them, as new_edges takes; and nodes, a map for every star in the order of points, the root first then breadth first: position, its centre; radius; weight, how often its symbol comes up, smoothing included, or its leaves together; depth, 0 for the root; code, its digits from the root, an array of whole numbers (a leaf's is its symbol's code); symbol, a leaf's symbol, or () for a branch; text, that symbol as text (space for 0 in base 27), or (); and parent, its parent's index, or () for the root. Weightless leaves the lock pads merges with take digits but aren't stars.", [
	arg("symbols", "What to build the code from.", "a lock value of symbols, as symbols(27, TEXT) gives; its base is the input base."),
	arg("output_base", "The base the code writes in: how many children a branch has, at most.", "a whole number from 2 to 27."),
	vec_arg("center", "The middle of the chart's box."),
	arg("width", "How wide the box is: every star is wholly inside it.", "a positive number."),
	arg("height", "How tall the box is.", "a positive number."),
	opt("options.smoothing", "1", "As the lock's: how many times every symbol of the input base counts besides its uses, so with 1 every symbol is a star; with 0 only those used are.", "a whole number of at least 0; with 0, symbols must not be empty."),
	seed_opt("chart"),
	opt("options.variation", "0.6", "How much stars' spaces and pulls, links' lengths and the growth's reaches differ: 0 makes them all alike, 1 as uneven as they get.", "a number from 0 to 1."),
	opt("options.iterations", "150", "How many steps the forces tidy the grown tree by; 0 leaves it as grown.", "a whole number from 0 to 10000."),
	opt("options.star_size", "a seventh of how far apart the stars spread evenly through the box would be", "The biggest star's radius.", "a positive number, less than half the box's width and height."),
	opt("options.gap", "star_size / 4", "How far short of stars links stop.", "a number of at least 0."),
	opt("options.round", "false", "Keeps the stars in the ellipse inscribed in the box rather than the box.", "true or false."),
]);

doc += api("art::constellations(chart, count, center, width, height), art::constellations(chart, count, center, width, height, options)", "A chart's tree cut into constellations, a page each: pieces that hold together, neighbouring pieces sharing the star they meet at, so laid over each other on their shared stars they make the whole. The tree is cut leaves up into pieces of about the same number of links, as many as count or as near as the tree allows. Each piece is fitted, on its own, to the box, across and down apart (an affine stretch, so links still don't cross, neither way more than 4 times the other), so a cluster gets the room the box has its way, and each of its stars gets a spot for a label, clear of its links, its other stars and the labels placed before it, inside the box where it can be, a little toward the upper right all else equal. Gives an array of #{ stars, points, pairs, labels, scale }: stars, the chart's indices of the piece's stars, in order; points, where each goes in the box; pairs, its links as [parent, child] indices into points, as new_edges takes; labels, the middle of each star's label; and scale, how much the chart was scaled by to fit, a vec2: across and down.", [
	arg("chart", "The stars and links to cut.", "a map with points (vec2s), edges ([parent, child] index pairs making one tree) and nodes (maps with a radius), as art::huffman_stars gives."),
	arg("count", "How many pieces.", "a whole number of at least 1."),
	vec_arg("center", "The middle of the box each piece is fitted to."),
	arg("width", "How wide the box is.", "a positive number."),
	arg("height", "How tall the box is.", "a positive number."),
	opt("options.label_width", "30", "How wide a label's box is.", "a number of at least 0."),
	opt("options.label_height", "24", "How tall a label's box is.", "a number of at least 0."),
	opt("options.label_gap", "6", "How far from its star's edge a label sits, at least.", "a number of at least 0."),
	opt("options.most_stars", "no limit", "The most stars a constellation may have: the cut is the one nearest count pieces whose pieces all fit, so there are more pieces than count if that's what it takes (with count 1, as few as fit).", "a whole number of at least 2."),
	opt("options.split_at", "[]", "Stars whose constellation is split in two at them, one more piece each, both keeping the star: its branches from the star (the parts of it that meet only there) are taken round the star by the way they go, from the widest gap between them, and cut into two runs with about as many links each, so the pieces lie either side of it, the leftmost first.", "an array of star indices, each with a constellation that has two or more branches from it."),
]);

// ---- books ----

doc += heading("Books");
doc += para("A book numbers pages in order and names them for their number and section, so their files sort in order, and keeps a contents of where each section starts. How a page looks, its running head and folio, is the journal's.");
doc += api("new_book(), new_book(options)", "An empty book.", [
	opt("options.first", "1", "As book.first.", "as book.first."),
]);
doc += api("book.first = n", "The number the first page gets: 3, say, after a title page and a contents.", [
	arg("n", "The number.", "a whole number; set before any page is added."),
]);
doc += api("book.add(section, page), book.add(section, pages)", "Adds a page, or pages in order, to a section, each named NN_SECTION: its number, two digits at least, and the section with spaces made underscores. Gives the (first) page's number.", [
	arg("section", "The section, as the contents lists it.", "a string; a new section starts whenever it changes."),
	arg("page, pages", "What to add.", "a page, or an array of pages such as typeset or split gives."),
]);
doc += api("book.next", "Read only: the number the next page added gets, for its folio.");
doc += api("book.pages", "Read only: the pages, in order, to add to journal_entry.");
doc += api("book.contents", "Read only: each section and its first page's number, [[section, number], ...].");
doc += api("roman(n)", "n in lowercase roman numerals, for folios: roman(14) is xiv.", [
	arg("n", "The number.", "a whole number from 1 to 3999."),
]);
doc += api("escape_markdown(text)", "The text with markdown's marks escaped, so markdown(escape_markdown(text)) sets it as it is.", [
	arg("text", "The text.", "a string."),
]);

// ---- layouts ----

doc += heading("Page Layouts");
doc += para("A layout describes pages as trees of flexbox boxes, and typeset pours text, art and ornaments into their slots, chaining text through columns and pages until it runs out. Typeset pages are named NAME_N and are exactly the layout's size; they split and go into PDFs like any page.");
doc += api("layout(name), layout(name, options)", "Loads a layout file, which defines page(n, info) and optionally options(), page_size(o) and example(); see Lock, Illustration, Layout and Style Files.", [
	arg("name", "Which file: layouts/NAME.rhai, the journal's or the library's.", "a layout file's name without .rhai: no slashes, not starting with a dot."),
	opt("options", "the file's defaults", "Its options, which its pages see as info.options and page_size(o) as o.", "a map of names its options() gives to values like their defaults: a whole number is fine for a number."),
]);
doc += api("new_layout(name, Fn(page_function))", "A layout from a function in the script.", [
	arg("name", "The layout's name, which its typeset pages are named after unless typeset says otherwise.", "a string of letters, digits, underscores, hyphens and dots, not starting with a dot."),
	arg("page_function", "Gives each page's tree of boxes, given a page number and info, as a layout file's page(n, info) does.", "a function pointer: Fn with the script function's name, as a string."),
]);
doc += api("layout.name", "Read only: the layout's name.");
doc += api("layout.typeset(options)", "Typesets pages until every flow has run out, giving them as an array. Fails, naming the flows, if a page sets none of what's left, or if text is left after max_pages.", [
	opt("options.flows", "none", "The text slots set, by flow name. A blank line starts a paragraph, and a line break breaks a line.", "a map of names to strings or markdown(text)."),
	opt("options.roles", "none", "How each kind of text and stroke looks, by role name (see Roles).", "a map of names to roles."),
	opt("options.art", "none", "What figure and band slots draw, by name.", "a map of names to canvases (drawn in their own styles), paths or glyphs (drawn in the slot's role style), or functions given the box, #{ x, y, width, height, center }, and giving a canvas or path, drawn as it comes."),
	opt("options.name", "the layout's name", "What the pages are named after: NAME_0, NAME_1, and on.", "a string of letters, digits, underscores, hyphens and dots, not starting with a dot."),
	opt("options.width", "the layout file's page_size()", "The page width. Needed if the layout gives none.", "a positive number."),
	opt("options.height", "the layout file's page_size()", "The page height. Needed if the layout gives none.", "a positive number."),
	opt("options.max_pages", "200", "The most pages to set before failing.", "a whole number of at least 1."),
]);
doc += para("Boxes. Every box takes an options map, which can be left out: its flex style, flex, and for slots (every box but rows and columns) the role it takes and a style, type, paragraph or spans of its own, each whole or a map of settings over its role's.");
doc += api("row(children), row(children, options)", "Lays out boxes across.", [
	arg("children", "The boxes it lays out, left to right.", "an array of boxes: rows, columns, flows and the rest."),
	opt("options.flex", "none", "How the row is laid out, and where it sits (see Flex styles).", "a flex style, or a map of flex settings, container settings among them."),
]);
doc += api("column(children), column(children, options)", "Lays out boxes down.", [
	arg("children", "The boxes it lays out, top to bottom.", "an array of boxes."),
	opt("options.flex", "none", "How the column is laid out, and where it sits (see Flex styles).", "a flex style, or a map of flex settings."),
]);
doc += api("flow(name), flow(name, options)", "A slot for text from a flow; what doesn't fit goes on in the next slot of the same name, then on the next page. Its role, unless options say one, is the flow's name.", [
	arg("name", "Which flow it sets.", "a name in typeset's flows."),
	opt("options", "none", "Its flex style and slot settings (see Flex styles and Slots).", "a map."),
]);
doc += api("label(text), label(text, options)", "Fixed text, such as a page number, in the role label unless options say another.", [
	arg("text", "The text it sets.", "a string."),
	opt("options", "none", "Its flex style and slot settings.", "a map."),
]);
doc += api("initial(name), initial(name, options)", "A drop cap: the next letter of a flow, filling its box, in the role initial unless options say another; that role needs a glyphset but no glyph scale. Placed absolutely in a column with the flow's slot, the text wraps round it.", [
	arg("name", "The flow whose next letter it draws.", "a name in typeset's flows."),
	opt("options", "none", "Its flex style and slot settings.", "a map."),
]);
doc += api("figure(name), figure(name, options)", "Art, fitted into the box, keeping its proportions, centred. A path or glyph is drawn in its role's style (the art's name unless options say another), or default_style() without one.", [
	arg("name", "Which art it draws.", "a name in typeset's art."),
	opt("options", "none", "Its flex style and slot settings.", "a map."),
]);
doc += api("band(name), band(name, options)", "Art repeated along the box as an ornamental strip, each copy scaled to the band's breadth and turned to run along it (or by its flex style's turn).", [
	arg("name", "Which art it repeats.", "a name in typeset's art."),
	opt("options", "none", "Its flex style and slot settings.", "a map."),
]);
doc += api("rule(), rule(options)", "A line along the middle of the box, the long way, in the role rule's style unless options say another role.", [
	opt("options", "none", "Its flex style and slot settings.", "a map."),
]);
doc += api("spacer(), spacer(options)", "Nothing: room.", [
	opt("options.flex", "none", "Its flex style.", "a flex style, or a map of flex settings."),
]);
doc += para("A page function gets n (from 0) and info: #{ flows, art, width, height }, flows saying how many characters of each are left and art which art was given, so a layout can change as the text runs out.");
doc += api("new_flex_style(), new_flex_style(options)", "A flex style: how a box is sized and placed, and lays out its children. Its settings are the options below, each also a property, flex.KEY = value, checked as the option is. Lengths are canvas units, a percentage of the parent as N%, or auto.", [
	opt("options.width, options.height", "auto", "The box's size; left out, a flow or label is sized to its text.", "a length."),
	opt("options.min_width, options.min_height, options.max_width, options.max_height", "none", "Limits on its size.", "a length."),
	opt("options.grow", "0", "How much of its parent's spare room it takes, shared out among its siblings by this; a box that grows with no size of its own grows from nothing.", "a number of at least 0."),
	opt("options.shrink", "1", "How much it gives up when its parent is too small, shared out likewise.", "a number of at least 0."),
	opt("options.basis", "auto", "Its size along its parent before growing and shrinking.", "a length."),
	opt("options.margin", "0", "Room outside it.", "one length, [vertical, horizontal], [top, horizontal, bottom], [top, right, bottom, left] or #{ top, right, bottom, left }."),
	opt("options.padding", "0", "Room inside it, round its children or what it draws.", "as margin."),
	opt("options.position", "relative", "Whether it's in its parent's flow.", "relative, or absolute: out of the flow, placed by top, left, right and bottom, with text in its parent wrapping round it."),
	opt("options.top, options.left, options.right, options.bottom", "none", "Where an absolute box sits, from its parent's edges.", "a length."),
	opt("options.aspect", "none", "Its width over its height.", "a number of at least 0."),
	opt("options.align_self", "auto", "Where it sits across its parent, over the parent's align.", "auto, start, end, center or stretch."),
	opt("options.border", "none", "What outlines the box.", "a style, a map of style settings over default_style(), or a role's name (for layout files, which can't see the journal's styles): that role's style."),
	opt("options.gap", "0", "Rows and columns only: room between children.", "a number of at least 0."),
	opt("options.justify", "start", "Rows and columns only: where children sit along it.", "start, end, center, space_between, space_around or space_evenly."),
	opt("options.align", "stretch", "Rows and columns only: where children sit across it.", "start, end, center or stretch."),
	opt("options.align_content", "stretch", "Rows and columns only: how lines of wrapped children share room across it.", "start, end, center, stretch, space_between or space_around."),
	opt("options.wrap", "false", "Rows and columns only: whether children wrap onto more lines.", "true or false."),
	opt("options.text_wrap", "box when absolute, none otherwise", "Slots only: what text wrapping round it keeps clear of.", "box (its box), hull (the outline of what it draws) or none."),
	opt("options.turn", "following the band", "Bands only: turns each copy this far instead; by default it's turned a quarter when the art runs the other way from the band.", "a number, in radians."),
]);
doc += api("flex_style.width, .height, .min_width, .min_height, .max_width, .max_height, .grow, .shrink, .basis, .margin, .padding, .position, .top, .left, .right, .bottom, .aspect, .align_self, .border, .gap, .justify, .align, .align_content, .wrap, .text_wrap, .turn = value", "A flex style's settings as properties: each as new_flex_style takes it.", [
	arg("value", "The setting's value.", "as the option takes it."),
]);
doc += listing("Slots: the keys of a slot's options, besides flex.", [
	opt("role", "named after the slot", "The role it takes: by default its flow's or art's name, or label, initial or rule.", "a role's name, in typeset's roles."),
	opt("style, type, paragraph", "the role's", "Its own, over its role's.", "a style, type style or paragraph style, or a map of their settings over the role's."),
	opt("spans", "the role's", "Named spans of its own, over its role's (see Marked-Up Text).", "a map of names to spans."),
]);
doc += listing("Roles: the keys of each role in typeset's roles. Flows and labels need a type with a glyphset and glyph scale; initials a glyphset; rules, borders and art draw in the style.", [
	opt("style", "default_style()", "What the role's text and strokes are drawn in, and the hand that sets its glyphs.", "a style, or a map of style settings over default_style()."),
	opt("type", "none", "The role's glyphset, glyph scale and shaping (see Text).", "a type style, or a map of type settings."),
	opt("paragraph", "the defaults", "How the role's lines are built and set (see Text): line_height, align, valign, indent, paragraph_gap, fit, clearance, line_start_jitter, line_gap_jitter, split_words and split_padding.", "a paragraph style, or a map of paragraph settings."),
	opt("spans", "em and strong as Marked-Up Text says", "The role's named spans.", "a map of names to spans."),
]);

// ---- markdown ----

doc += heading("Marked-Up Text");
doc += api("markdown(text), markdown(text, options)", "A flow whose text is styled as it's set. Spans nest, each changing what it's in.", [
	arg("text", "The text, with its marks: *emphasis* or _emphasis_ is the span em; **strong** or __strong__ the span strong; ~~text~~ is struck through; [text]{settings} is set with settings of its own: span names, and key: value pairs, separated by commas, a key alone meaning true. A word alone is a setting if it's one, and a span's name otherwise; the spans named are applied first, then the settings. A backslash writes a mark as it is.", "a string."),
	opt("options.spans", "none", "Named spans of the text's own, over its role's.", "a map of names to spans."),
]);
doc += listing("A span: a style and a type, each whole or a map of settings over the text it's in. em is by default #{ type: #{ italic: true } }, and strong #{ style: #{ passes: 2, pass_end_jitter: 1.3 } }: gone over twice, a little astray.", [
	opt("style", "the text's", "What it's drawn in.", "a style, or a map of style settings."),
	opt("type", "the text's", "How it's set: a glyph_scale_factor multiplies the size of the text round it, and underline and strike draw lines under and through it.", "a type style, or a map of type settings."),
]);
doc += para("Settings in a span's braces are a style's and a type's: color, passes, pass_end_jitter and the rest; italic, skew, glyph_scale_factor, glyph_outline_gap, space_width, underline and strike. Lines take true, false (none, inside text that has them), a number of lines, or {count, spacing, style}, the style in braces too.");

// ---- locks ----

doc += heading("Locks");
doc += para("Locks encipher text for a journal to draw. A chain is lock types wired together, each from a lock file in locks/; running it locks the text, and unlock recovers it. Locks and their values are named ID.NAME: the id a lock was added under, and the name its lock file gives the value.");
doc += api("new_lock(name)", "An empty lock chain. The chain is checked after every change, so a mistake fails at the line that made it.", [
	arg("name", "The chain's name, for messages.", "a string."),
]);
doc += api("chain(name), chain(name, options)", "The chain a chain file, chains/NAME.rhai, describes: its locks added, wired and given their fixed values, and its illustrations attached and wired, as new_lock and its calls would build it, so its values are named ID.NAME and more can be added or attached. The web editor's lock editor makes chain files, and as it edits one, journals using it follow along.", [
	arg("name", "Which chain file: chains/NAME.rhai, the journal's or the library's.", "a chain file's name (see Lock, Illustration, Layout and Style Files)."),
	opt("options", "the chain file's defaults", "Its options, as its options() declares them.", "a map of its option names to values, each required unless it has a default."),
]);
doc += api("lock.add(id, lock_type), lock.add(id, lock_type, options)", "Adds a lock to the chain. Returns the chain.", [
	arg("id", "The lock's name in the chain, as in ID.NAME.", "a string, not empty, with no dot, that no other lock in the chain has."),
	arg("lock_type", "Which lock file: locks/TYPE.rhai, the journal's or the library's.", "a lock type's name (see Lock types)."),
	opt("options", "none", "The type's options.", "a map of the type's option names to values: each required unless the type gives it a default."),
]);
doc += api("lock.connect(from, to)", "Wires one lock's output to another's input, or a value of the chain to an attached illustration's input. Returns the chain.", [
	arg("from", "The output, or for an illustration any value of the chain.", "ID.NAME of a lock's output; for an illustration, of any lock's input or output."),
	arg("to", "The input it feeds.", "ID.NAME of an input not already wired, of the output's type (or Any), making no loop; or ID.NAME of an illustration's input, of the value's type, not already wired. An illustration's wire leaves the chain as it was: the value is still an output, or still needs its input."),
]);
doc += api("lock.set_input(input, value)", "Fixes an input, so running doesn't need it. Returns the chain.", [
	arg("input", "The input.", "ID.NAME of an input not wired to an output."),
	arg("value", "Its value.", "a lock value of the input's type, as symbols(27, TEXT) gives, a string for a String input, or a value kind's data, as huffman::code gives (checked against the kind's shape)."),
]);
doc += api("lock.attach(id, illustration), lock.attach(id, illustration, options)", "Attaches an illustration, an illustrations/TYPE.rhai file drawing the values wired to its inputs with lock.connect as canvases, which result.canvases gives. It runs in the journal's engine, not the lock's: locking and unlocking never see it. Returns the chain.", [
	arg("id", "The illustration's name in the chain, as in ID.NAME for its inputs.", "a string, not empty, with no dot, that no lock or illustration in the chain has."),
	arg("illustration", "Which illustration file: illustrations/TYPE.rhai, the journal's or the library's.", "an illustration type's name (see Illustrations)."),
	opt("options", "the illustration's defaults", "Its options.", "a map of names its options() gives to values like their defaults: a whole number is fine for a number."),
]);
doc += api("lock.source(id, kind, name), lock.source(id, kind, name, options)", "Adds a source: a journal file the chain draws on, its value at port ID.value, which lock.connect wires as a lock's output is. Content is data, so it can feed a lock's input (its value fixed there) or an illustration's; a style, glyphset or layout only an illustration's, at an input typed TextStyle, Glyphset or Layout, made when the illustration draws (text_style(name, options), glyphset(name), layout(name, options)). Returns the chain.", [
	arg("id", "Its name in the chain.", "a string, not empty, with no dot, that no lock, illustration or source in the chain has."),
	arg("kind", "What it draws on.", "content, style, glyphset or layout."),
	arg("name", "Which file.", "a content, text style, glyphset or layout name."),
	opt("options.options", "the file's defaults", "A style's or layout's options.", "a map."),
	opt("options.base", "none", "Content only: text content (.txt, .md) as symbols in this base, what doesn't fit dropped; without it, text is a String. A .json content file's symbols are in its own base.", "a whole number from 2 to 27."),
]);
doc += api("lock.illustrations()", "Every illustration attached, a chain file's included: #{ ID: illustration type }.");
doc += api("lock.inputs()", "The inputs still needing a value when it's run: #{ NAME: type }.");
doc += api("lock.outputs()", "The chain's final outputs: #{ NAME: type }.");
doc += api("lock.run(), lock.run(inputs)", "Runs the chain: a lock result of its outputs.", [
	opt("inputs", "none, for a chain whose inputs are all set", "A value for every open input.", "a map of the names lock.inputs() gives to lock values of their types, strings for String inputs, or value kinds' data."),
]);
doc += api("lock.unlock(result)", "Recovers the inputs from a run's result: a lock result.", [
	arg("result", "What running the chain gave.", "a lock result, from run."),
]);
doc += api("lock.unlock(outputs)", "Recovers the inputs from outputs given by hand: a lock result.", [
	arg("outputs", "The chain's outputs.", "a map of the names lock.outputs() gives to lock values, strings or value kinds' data."),
]);
doc += api("result.get(name)", "One value, as a lock value.", [
	arg("name", "Which value.", "one of the result's names, as values() lists them."),
]);
doc += api("result.text(name)", "One value as text.", [
	arg("name", "Which value.", "one of the result's names, symbols or a string."),
]);
doc += api("result.values()", "Every value: #{ NAME: lock value }.");
doc += api("result.trace()", "Every value computed along the way, intermediate ones included.");
doc += api("result.canvases(id)", "What an attached illustration draws of this result's values: an array of canvases, one or more, to add to pages.", [
	arg("id", "Which illustration.", "an id lock.illustrations() gives, every input of it connected."),
]);
doc += api("symbols(base, text), symbols(base, text, options)", "Text as a lock value of symbols in a base.", [
	arg("base", "Which symbols: 10 is digits; 27 is space (0) and A to Z (1 to 26); other bases the first base letters, A being 0.", "a whole number from 2 to 27."),
	arg("text", "The text.", "a string of the base's symbols, letters in any case."),
	opt("options.lenient", "false", "Makes any text fit: in capitals, characters the base hasn't dropped, and runs of spaces and dashes one space where the base has one (not at either end).", "true or false."),
]);
doc += api("value.symbols()", "The value's symbols, as an array of whole numbers. Fails for values that aren't symbols.");
doc += api("value.text()", "The value as text, for symbols and strings.");
doc += api("value.base", "Read only: the base of a value of symbols.");
doc += api("value.value()", "The value as lock files see it: symbols as an array of whole numbers, a value kind's (a maze, a code, a star chart) as its map of plain data.");
doc += api("value.type_name", "Read only: the value's type, such as Base27Symbol, Maze or StarChart.");
doc += api("value.maze()", "A maze output's data, a Maze map, for maze.cells() and maze.path(). Fails for values that aren't mazes.");
doc += api("value.sketch()", "A value kind's default drawing, as art::sketch draws its data. Fails for values that aren't a value kind's, or whose kind has no sketch.");
doc += api("value.to_string(), value.to_debug()", "The value as text, for print and debug.");
doc += api("new_maze(width, height), new_maze(width, height, options)", "A maze of width by height nodes, in a grid of 2w - 1 by 2h - 1 cells, as a Maze value (see Value Kinds). Its corridors depend only on the seed and size; its cells hold random symbols from 0 to 26, from the seed.", [
	arg("width", "How many nodes across.", "a whole number of at least 1."),
	arg("height", "How many nodes down.", "a whole number of at least 1."),
	opt("options.seed", "0", "Which maze.", "a whole number."),
	opt("options.strategy", "LeftHanded", "The order its cells are walked in.", "LeftHanded or RightHanded."),
]);
doc += api("maze.width, maze.height", "Its grid size in cells, passages included: fields of its data.");
doc += api("maze.cells()", "Every cell in walking order, the order a lock writes its text: #{ x, y, step, symbol }. Nodes are at even coordinates, passages between them.");
doc += api("maze.fill(symbols)", "Writes symbols into the cells in walking order, and 0 into any left over: its symbols field, rewritten.", [
	arg("symbols", "What to write.", "an array of whole numbers of at least 0, no more of them than the maze has cells."),
]);
doc += api("maze.path()", "A path along its corridors, in grid units, walked in the same order.");
doc += listing("Lock types: the files in locks/, with the options lock.add gives them and the values they take and give.", [
	arg("doublering", "The 26 letters in two rings of 13, each swapping with the one across; locking and unlocking are the same. Takes plain_text, gives cipher_text, both base 26.", "options: canonical_order (required), the rings: a string of the 26 letters, each once."),
	arg("huffman_encode", "Text rewritten from input_base to output_base by the Huffman code its own symbols build, as huffman::code(plain_text, input_base, output_base) builds it. Takes plain_text (in input_base); gives cipher_text (in output_base) and code, a HuffmanCode: the key, which unlocking reads the cipher text back with and an art node such as star_chart can show.", "options: input_base and output_base (required), whole numbers from 2 to 27; smoothing (optional, default 1), a whole number of at least 0, how many times every symbol counts besides its uses, as huffman::code takes it."),
	arg("huffman_apply", "Text rewritten from input_base to output_base by a Huffman code given as its tree, a HuffmanCode: one huffman_encode gives, or huffman::code builds from a corpus, so a message can be locked with another text's code. Takes plain_text (in input_base) and code; gives cipher_text (in output_base) and code, passed through: the key, which unlocking reads the cipher text back with.", "options: input_base and output_base (required), whole numbers from 2 to 27, the code's own."),
	arg("huffman", "A Huffman code built from a corpus: symbols common in it get short codes, rare ones long, rewriting text from input_base to output_base. Ties go to the lower symbol, so a corpus always gives the same code. Takes corpus and plain_text, both in input_base; gives cipher_text, in output_base, and corpus, which unlocking rebuilds the code from.", "options: input_base and output_base (required), whole numbers from 2 to 27; smoothing (optional, default 1), a whole number of at least 0, how many times every symbol counts besides its uses in the corpus. With smoothing 0, a symbol the corpus lacks has no code, and locking text with one fails."),
	arg("keyed_doublering", "A doublering whose order is an input. Takes plain_text and base_ordering, gives cipher_text and base_ordering, in the base.", "options: base (required), an even whole number."),
	arg("maze_lock", "The text written into a square maze's cells, one symbol each, in walking order, the maze laid out by the seed. Takes maze_text (in input_base) and random_seed (base 10); gives generated_maze, random_seed and text_length.", "options: input_base (required), a whole number from 2 to 27; strategy (optional, default LeftHanded), LeftHanded or RightHanded."),
	arg("merge_alpha", "Texts joined end to end. Takes input_0 to input_N, gives merged_output and location_info, which unlocking splits them by.", "options: base_n (required), a whole number from 2 to 27; input_count (required), a whole number of at least 1."),
	arg("shift_cipher", "Every symbol moved shift places along the base, wrapping round (a Caesar cipher). Takes plain_text, gives cipher_text.", "options: base (required), a whole number from 2 to 27; shift (required), a whole number, negative moving the other way."),
	arg("shift_vigenere", "The key shifted along the alphabet, then used as a Vigenere key; a composite of shift_cipher and vigenere_cipher, base 27. Takes key and plain_text, gives cipher_text and shifted_key.", "options: shift (optional, default 3), a whole number."),
	arg("star_chart", "An art node: a HuffmanCode drawn as a StarChart, every node a named star, every link carrying its digit, the sky cut into constellations of at most most_stars stars, a page each, every one fitted to its page; neighbouring pages share a star under the same name. The root is Shiathri, a letter's star is named for its letter (the space's, in base 27, Vacii). Unlocking reads only what the pages show: it joins them on their shared names, takes the root as the star no link points to, a leaf's symbol from its name's first letter and each digit from its link. Takes code, gives chart.", "options: input_base and output_base (required), the code's (input_base 27 or a base of letters, 2 to 26 but not 10); most_stars (default 9), at least 2; seed (7), the sky's shape; name_seed (11), the names; size_seed (29), the stars' sizes; star_size (22), the biggest star's radius as it's laid out; width and height (1000 and 860), each page's sky; label_width, label_height and label_gap (26.4, 33.6 and 14), the room each star's label needs, as art::constellations takes them."),
	arg("split_symbols", "A text cut into pieces each as long as the first, the last ones shorter or empty. Takes plain_text, gives output_0 to output_N.", "options: base_n (required), a whole number from 2 to 27; output_count (required), a whole number of at least 1."),
	arg("string_to_symbols", "A string read as symbols in the base, as symbols(base, text) reads it. Takes text, a String; gives symbols, in the base. Unlocking writes the symbols back out as text, the text as it was fitted when lenient.", "options: base (required), a whole number from 2 to 27; lenient (optional, default false), true or false: makes any text fit the base first, as symbols' lenient option does; without it, text the base can't write is an error."),
	arg("substitution_cipher", "Each symbol replaced by the ordering's symbol at its position. Takes plain_text and ordering (every symbol of the base once), gives cipher_text and ordering.", "options: base_n (required), a whole number from 2 to 27."),
	arg("vigenere_cipher", "Each plain symbol plus the key symbol under it, the key repeating, mod cipher_base. Takes plain_text and key, gives cipher_text and key.", "options: plain_base, key_base and cipher_base (all required), whole numbers from 2 to 27."),
]);

// ---- value kinds ----

doc += heading("Value Kinds");
doc += para("A value kind is a lock value type whose values are plain data, maps of arrays, numbers and strings tagged with kind, so they cross between journals, lock files and the web editor as they are. Each is checked against its shape wherever it goes into or comes out of a lock, with errors naming the field; a kind's value holds only what its drawing shows. Its functions are the same in journals and lock files, and its sketch, if it has one, is how value.sketch, art::sketch and the editor draw it.");
doc += listing("The kinds and their data.", [
	arg("HuffmanCode", "An M-ary Huffman code as its tree: #{ kind, input_base, output_base, nodes }, nodes an array of #{ parent, digit, symbol }, the root first (its parent and digit ()), then breadth first, each node's children in digit order; a leaf's symbol is the input symbol its digits from the root encode, a branch's ().", "sketch: the tree, top down, each leaf's symbol under it."),
	arg("Maze", "A maze: #{ kind, width, height, seed, strategy, symbols }: its grid in cells (odd), the seed its corridors come from, the order (LeftHanded or RightHanded) its cells are walked in, and each cell's symbol in that order.", "sketch: the corridors, and each cell's symbol as a letter."),
	arg("StarChart", "A sky of named stars over pages, as it's drawn: #{ kind, input_base, output_base, pages }, each page #{ name, stars, links }: its stars #{ name, point, size, label } (where it is, how far its mark reaches, where its label goes, points as [x, y]), and its links [parent, child, digit], indices into its stars. A star on more than one page has the same name on each.", "sketch: the pages in a grid, each star a ring, its links, its number at its label and the page's name over it."),
]);
doc += api("huffman::code(symbols, output_base), huffman::code(symbols, output_base, options), huffman::code(symbols, input_base, output_base), huffman::code(symbols, input_base, output_base, options)", "The HuffmanCode of a corpus, as the huffman lock builds its code: symbols common in it get short codes. In journals symbols is a symbols(...) value, its base the input base (given too, as lock files write it, it must match); in lock files it's an array and its base is given. The lock editor takes this as a test value.", [
	arg("symbols", "The corpus.", "a symbols(base, text) value, or an array of whole numbers from 0 to input_base - 1."),
	arg("input_base", "With an array: its base.", "a whole number from 2 to 27."),
	arg("output_base", "The base the code writes in: how many children a branch has, at most.", "a whole number from 2 to 27."),
	opt("options.smoothing", "1", "How many times every symbol of the input base counts besides its uses: with 1 every symbol has a code.", "a whole number of at least 0."),
]);
doc += api("stars::layout(code), stars::layout(code, options)", "A HuffmanCode's tree laid out as art::huffman_stars lays out its stars: grown from the root at random angles and lengths, then tidied by forces, links never crossing. Its stars are sized by depth, the root biggest, the deepest a third its size. Gives #{ points, sizes }, a [x, y] point and a radius for each node, in the code's order.", [
	arg("code", "The tree.", "a HuffmanCode."),
	opt("options.seed", "0", "Which layout.", "a whole number."),
	opt("options.variation", "0.6", "As art::huffman_stars's.", "a number from 0 to 1."),
	opt("options.iterations", "150", "As art::huffman_stars's.", "a whole number from 0 to 10000."),
	opt("options.star_size", "a seventh of how far apart the stars spread evenly would be", "The root's radius.", "a positive number."),
	opt("options.width, options.height", "1000 and 800", "The box it's laid out in, from (0, 0); only its proportions matter once stars::split fits each piece.", "positive numbers."),
]);
doc += api("stars::split(points, edges, sizes), stars::split(points, edges, sizes, options)", "A laid-out tree cut into constellations, as art::constellations cuts a chart: pieces sharing the stars they meet at, each fitted to the box from (0, 0) to (width, height), each star given a spot for its label. Gives an array of #{ stars, points, labels, pairs }, as art::constellations's pieces, points as [x, y].", [
	arg("points", "Where each star is.", "an array of [x, y] points."),
	arg("edges", "The links, [parent, child] indices into points, making one tree.", "an array of pairs."),
	arg("sizes", "How far each star's mark reaches, which links and labels keep clear of.", "an array of numbers."),
	opt("options.count", "1", "As art::constellations's count.", "a whole number of at least 1."),
	opt("options.most_stars, options.split_at", "no limit, []", "As art::constellations's.", "as there."),
	opt("options.width, options.height", "1000 and 860", "The box.", "positive numbers."),
	opt("options.label_width, options.label_height, options.label_gap", "30, 24 and 6", "As art::constellations's.", "numbers of at least 0."),
]);
doc += listing("Illustrations: the files in illustrations/, with the options lock.attach gives them and the values they draw.", [
	arg("eye", "An eye stood on end, its corners top and bottom: one text's symbols round its iris as balanced-ternary glyphs, clockwise from the top, and another's filling its pupil, as large as they fit; each symbol shifted to sit round 0. One canvas, width across and length down. Draws outer, symbols in outer_base, round the iris, and inner, symbols in inner_base, in the pupil; Welcome Home's eye holds a substitution cipher's ordering and cipher text. Drawn in lid_style, ring_style, iris_style and pupil_style, TextStyles, with iris_glyphs and pupil_glyphs, Glyphsets whose A to M and N to Z are 1 to 13 and -13 to -1; unwired, rough ink pens inked as its options say and balanced ternary.", "options: outer_base and inner_base (27 and 27), whole numbers from 2 to 27; length and width (1640 and 717.2), the eye's size; top_ink and bottom_ink (#09603d and #0b2a1c), its lines' gradient, top to bottom; inner_ink (#000000), the pupil's glyphs; pupil (lines), how the pupil's text goes: lines, across it line under line, or spiral, round and round out from its middle as art::spiral_text writes it, at most size 40; pupil_minimum_scale (0.33), with pupil spiral, the smallest its last turn's glyphs shrink to, as a fraction of the rest."),
	arg("maze_glyphs", "A Maze on a grid, cell by cell: its corridors in one ink and each cell's symbol (0 to 26) as its balanced-ternary glyph, shifted to sit round 0, in another. One canvas. Draws maze, a Maze, in corridor_style and glyph_style, TextStyles, with glyphs, a Glyphset; unwired, ink pens in the corridors and glyphs inks and balanced ternary.", "options: cell (70), a cell's size; corridors and glyphs (#9b2318 and #2b1d12), the inks of the styles' fallbacks."),
	arg("star_pages", "A StarChart page by page, a canvas each, page-sized: its constellation's name over it, each star's mark, each link's digit in balanced-ternary marks, each star numbered on its page in balanced ternary at its label, and an index of the numbers to the names in the bottom right corner; leaves in red. Draws chart, a StarChart, in line_style, leaf_style, fine_style and fine_leaf_style, TextStyles, with heading_glyphs, name_glyphs and digit_glyphs, Glyphsets; unwired, near-black and red ink pens, loche_upper, plain and balanced_ternary.", "options: width and height (1200 and 1600), the canvas; sky_x and sky_y (100 and 180), where the chart's sky starts; large_star (33), the size from which a star takes large_shape rather than small_shape; large_shape and small_shape (corona and rayed), each one of celestial, corona, orrery, rayed, sparkle or star (anything else a disc); digit (24), the link marks' size; index_x and index_y (600 and 1110), where the name index starts."),
	arg("styled_text", "Text written in a text style, wrapped into a block: a chain's content in the hand a style source gives it. One canvas. Draws text, a String, in style, a TextStyle.", "options: width (900), the block's width."),
	arg("trit_message", "Symbols in balanced-ternary marks, row after row under a heading, each shifted to sit round 0 and written in as many trits as its base needs. One canvas. Draws message, symbols in base, in heading_style, mark_style and line_style, TextStyles, the heading in heading_glyphs, a Glyphset; unwired, red and near-black ink pens and loche_upper.", "options: base (3), the symbols' base, a whole number from 2 to 27; title (MESSAGE), the heading; per_row (36), marks a row; width (1200), the canvas's width."),
]);
doc += api("art::sketch(value)", "A value kind's default drawing as a parts map for canvas.add(parts, styles): each part a path (a StarChart's links and stars, a HuffmanCode's links and nodes, a Maze's corridors), plus labels, an array of #{ at, text } to write, and label_size, how big to write them, in the drawing's units.", [
	arg("value", "What to draw.", "a value kind's data whose kind has a sketch."),
]);

// ---- journals ----

doc += heading("Journals and Their Files");
doc += para("A journal is a directory: its manifest, journal.json; its entries, entries/NAME.rhai, each a script building a journal_entry of pages; and files of its own, each kind in a directory of its own: locks/, layouts/, illustrations/, chains/, styles/ and lib/ (rhai files), glyphsets/ (NAME.json), and content/ (NAME.txt, NAME.md or NAME.json). They stand in front of the built-in library's, webapp/library/, which has the same directories: a journal's file stands in for the library's of the same name. Each entry runs on its own, sharing the journal's files but not code with the other entries.");
doc += listing("journal.json: the journal's manifest, a JSON object.", [
	req("version", "Which manifest format.", "1."),
	req("title", "The journal's title.", "a string."),
	opt("description", "none", "What it is.", "a string."),
	req("entries", "Its entries, in order: entries/NAME.rhai each.", "a non-empty array of names: letters, digits and underscores."),
	opt("bundled", "none", "Library files an export copied in beside the journal's own, as paths from the journal (locks/huffman.rhai), so an import can drop them.", "an array of strings."),
]);
doc += listing("glyphsets/NAME.json: a glyphset, a JSON object. Glyph units span about -1 to 1, y down; a glyph scale is 2 of them.", [
	req("glyphs", "Each character's glyph (or each symbol's, by name): SVG path data, or an array of variants, one picked at random each time it's set. A cursive glyph's first stroke runs from where it's entered to where it's left; dots and crossbars come after.", "an object of strings or arrays of strings."),
	opt("character_padding", "0", "Room between glyphs' outlines, in glyph scales.", "a number of at least 0."),
	opt("space_width", "0.5", "A space's width, in glyph scales.", "a positive number."),
	opt("fallback", "none", "The glyphset to look in for characters this one hasn't.", "a glyphset's name."),
	opt("glyph_padding", "none", "Room some glyphs keep on top of character_padding, in glyph scales.", "an object of glyphs to numbers of at least 0."),
	opt("joins", "none", "Joins words up, as glyphset.joined says: enter and leave, the glyphs that join the glyph before and the glyph after; leave_high, those left high, after which a variant entered high is drawn; x_height and baseline, the heights a variant's entry is told high or low by.", "an object: enter, leave and leave_high (default none), strings of glyphs; x_height and baseline, glyph units, x_height less than baseline."),
]);
doc += api("text_style(name), text_style(name, options)", "A text style file's bundle, styles/NAME.rhai, the journal's or the library's: #{ style, type, paragraph, spans }, the style, type and paragraph whole (a map's settings over default_style(), no type and the default paragraph), and spans if it gives any. It's a role for typeset, and the settings map of any text call, which ignores spans unless it sets marked-up text.", [
	arg("name", "Which style file.", "a style file's name without .rhai."),
	opt("options", "the file's defaults", "Its options, which its style(o) sees as o.", "a map of names its options() gives to values like their defaults: a whole number is fine for a number."),
]);
doc += api("content(name)", "A content file, content/NAME.txt, NAME.md or NAME.json, the journal's or the library's: a .txt or .md file's text, without the newline ending the file (markdown(content(name)) sets marked-up text), or a .json file's symbols, #{ base, symbols } in the file, as symbols(base, text) gives them.", [
	arg("name", "Which content file; there mustn't be more than one of that name.", "a content file's name without its extension."),
]);
doc += para("Any script, an entry or a lock, layout, illustration, chain or style file, imports helpers from lib/: import \"lib/NAME\" as NAME; then NAME::f(x) calls lib/NAME.rhai's function f. A helper runs in the engine of the script importing it: a lock file's helper can't draw.");

// ---- files ----

doc += heading("Lock, Illustration, Layout and Style Files");
doc += para("A lock file, locks/TYPE.rhai, defines functions that lock and unlock run, and can call rhai's standard library, the functions below, new_maze and value kinds' functions (huffman::code, stars::layout, stars::split), but nothing unseeded and nothing to draw with; see the library's locks/README.md. o, in each, is the lock's options, checked against their types first.");
doc += api("fn options()", "Gives each option's type: #{ NAME: type }, a type being one of the option types below.");
doc += api("fn check(o)", "Optional: throws to reject options, for what their types can't say, such as a base that must be even. Runs at lock.add.", [
	arg("o", "The options.", "a map of option names to values."),
]);
doc += api("fn inputs(o), fn outputs(o)", "Give the lock's values and their types: #{ NAME: type }. A value that is both an input and an output, such as a key, is passed through.", [
	arg("o", "The options.", "a map."),
]);
doc += api("fn lock(o, inputs)", "Gives every output: #{ NAME: value }, each checked against its type.", [
	arg("o", "The options.", "a map."),
	arg("inputs", "The input values.", "a map of input names to values: symbols as arrays of whole numbers, value kinds' data as maps."),
]);
doc += api("fn unlock(o, outputs)", "Gives the inputs it recovers: #{ NAME: value }; passed-through ones needn't be.", [
	arg("o", "The options.", "a map."),
	arg("outputs", "The output values.", "a map of output names to values."),
]);
doc += api("fn examples()", "Optional: test cases, run by --check-locks and cargo test, each locked and unlocked and needing its inputs back. Gives an array of #{ options, inputs, outputs, error }.", [
	req("options", "The options.", "a map."),
	req("inputs", "The inputs.", "a map of names to symbols(base, text) and the like."),
	opt("outputs", "not checked", "What locking must give.", "a map of names to values."),
	opt("error", "none", "Text the example must fail with, instead of locking.", "a string."),
]);
doc += api("fn chain(o)", "In place of inputs, outputs, lock and unlock: a composite lock's chain, #{ locks, connect, fixed, inputs, outputs }; composite lock files are written by hand.", [
	arg("o", "The options.", "a map."),
	req("locks", "Each part.", "a map of ids to #{ type, options }, options able to use o."),
	req("connect", "The wiring.", "an array of [output, input] pairs, as lock.connect takes them."),
	req("fixed", "Values for inputs, as lock.set_input sets them.", "a map of ID.NAME to values."),
	req("inputs, outputs", "The composite's own names for the chain's open ports.", "a map of names to ID.NAME."),
]);
doc += para("A chain file, chains/NAME.rhai, is a chain of lock types and the illustrations drawing its values, which chain(name) builds; the web editor's lock editor writes them. It runs in the lock files' engine, so it writes values as they do (symbols(27, TEXT) as an array); see the library's chains/README.md.");
doc += api("fn options() in a chain file", "Optional: the chain's options, as a lock file's options() declares them, which chain(name, options) gives.");
doc += api("fn chain(o) in a chain file", "The chain: #{ sources, locks, connect, fixed, illustrations }.", [
	opt("sources", "none", "Journal files it draws on, as lock.source adds them; connect and illustrations' inputs take ID.value.", "a map of ids to #{ type, name, options, base }: type content, style, glyphset or layout."),
	arg("o", "The options.", "a map."),
	req("locks", "Each lock.", "a map of ids to #{ type, options }, options able to use o."),
	opt("connect", "none", "The wiring.", "an array of [output, input] pairs, as lock.connect takes them."),
	opt("fixed", "none", "Values for inputs, as lock.set_input sets them.", "a map of ID.NAME to values."),
	opt("illustrations", "none", "Illustrations attached, as lock.attach attaches them.", "a map of ids to #{ type, options, inputs }: options able to use o, inputs a map of each illustration input to the ID.NAME it draws."),
]);
doc += api("fn layout() in a chain file", "Optional: where the lock editor draws each lock and illustration: #{ ID: [x, y] }.");
doc += api("fn example() in a chain file", "Optional: a value for each of the chain's open inputs, #{ ID.NAME: value }, run with its default options by --check-locks and cargo test, which lock and unlock it and need the inputs back; the lock editor's test case.");
doc += api("integer(), number(), string(), bool(), array()", "Option types, in options(): an option of that type, required unless given a default. An integer is fine for a number.", [
	opt("default", "none, so it's required", "What the option is when lock.add leaves it out.", "a value of the type."),
]);
doc += api("one_of(choices), one_of(choices, default)", "An option type: one of some values.", [
	arg("choices", "The values it can be.", "a non-empty array."),
	opt("default", "none, so it's required", "What it is when left out.", "one of the choices."),
]);
doc += api("symbol_type(base)", "The type of symbols in a base, such as Base27Symbol: an array of whole numbers from 0 to base - 1. Other types are a value kind's (HuffmanCode, Maze, StarChart: values checked against the kind's shape), String, Any (which matches every type) or a name of the file's own.", [
	arg("base", "The base.", "a whole number from 2 to 27."),
]);
doc += api("symbols(base, text), symbols(base, text, options) in lock files", "Text as an array of symbols: symbols(27, HI) is [8, 9].", [
	arg("base", "The base, as symbols takes it in journals.", "a whole number from 2 to 27."),
	arg("text", "The text.", "a string of the base's symbols."),
	opt("options.lenient", "false", "As in journals: any text made to fit.", "true or false."),
]);
doc += api("symbol_text(base, symbols)", "Symbols back as text: symbol_text(27, [8, 9]) is HI.", [
	arg("base", "The base.", "a whole number from 2 to 27."),
	arg("symbols", "The symbols.", "an array of whole numbers from 0 to base - 1."),
]);
doc += api("new_random(seed)", "A generator for lock files, the only randomness they have: the same seed gives the same numbers every run. Copies share it, so a helper handed one moves it on for its caller too.", [
	arg("seed", "Which numbers.", "a whole number."),
]);
doc += api("random.next()", "A number from 0 up to 1.");
doc += api("random.range(low, high)", "A number from low up to high.", [
	arg("low, high", "The range.", "numbers."),
]);
doc += api("random.pick(items)", "One of the items.", [
	arg("items", "What to pick from.", "a non-empty array."),
]);
doc += para("An illustration file, illustrations/TYPE.rhai, runs in the journal's engine, so it can call everything a journal can, and draws lock values as canvases; see the library's illustrations/README.md. o, in each, is its options: its defaults with what lock.attach gave over them.");
doc += api("fn options()", "Optional, in an illustration file: its options and their defaults, #{ NAME: value }; lock.attach can give only these, each like its default.");
doc += api("fn inputs(o)", "In an illustration file: the values it draws and their types, #{ NAME: type }, as a lock file's inputs are written (Base27Symbol, Maze, StarChart, String, Any...), or TextStyle, Glyphset or Layout for what a chain's style, glyphset or layout source gives.", [
	arg("o", "The options.", "a map."),
]);
doc += api("fn render(o, i)", "In an illustration file: a canvas, or an array of canvases, drawing the values.", [
	arg("o", "The options.", "a map."),
	arg("i", "The values, as lock files see them: symbols as arrays of whole numbers, value kinds' data as maps.", "a map of its input names to values."),
]);
doc += api("fn fallbacks(o)", "Optional, in an illustration file: what inputs nothing's wired to draw with, #{ NAME: value }, each NAME one of its inputs; an input without one must be wired. The library's illustrations take the styles and glyphsets they draw in as TextStyle and Glyphset inputs, a style's bundle (they use its style) and a glyphset, with fallbacks made from their options.", [
	arg("o", "The options.", "a map."),
]);
doc += api("fn example()", "Optional, in an illustration file: values to draw it with when nothing's connected, as the web editor previews it and cargo test checks it: #{ NAME: value }.");
doc += para("A layout file, layouts/NAME.rhai, runs in the journal's engine, so it can call everything a journal can; see the library's layouts/README.md.");
doc += api("fn options()", "Optional, in a layout file: its options and their defaults, #{ NAME: value }; layout(name, options) can give only these, each like its default.");
doc += api("fn page(n, info)", "Gives page n's tree of boxes.", [
	arg("n", "Which page, from 0.", "a whole number."),
	arg("info", "What's left to set: flows maps each flow's name to how many characters of it are left, art lists the art's names, width and height are the page's size, and options are the layout's options.", "a map, #{ flows, art, width, height, options }."),
]);
doc += api("fn page_size(o), fn page_size()", "Optional: the page's size, #{ width, height }, positive numbers, given the options. typeset's width and height take its place.");
doc += api("fn example()", "Optional, in a layout file: what to typeset to preview it, as the web editor does and cargo test checks: typeset's options, #{ flows, roles, art, ... }.");
doc += para("A text style file, styles/NAME.rhai, runs in the journal's engine; text_style(name) gives what it describes.");
doc += api("fn options()", "Optional, in a style file: its options and their defaults, #{ NAME: value }; text_style(name, options) can give only these, each like its default.");
doc += api("fn style(o)", "In a style file: #{ style, type, paragraph, spans }, each whole or a map of settings as a role takes them (all optional); style() if it has no options.", [
	arg("o", "The options.", "a map."),
]);
doc += api("fn example()", "Optional, in a style file: the text to preview it with, as the web editor does and cargo test checks: a string.");

// ---- building ----

doc += heading("Building");
doc += api("journalbuild NAME [flags]", "Runs a journal's entries, each on its own, and renders their pages, an entry's into a directory named for it.", [
	arg("NAME", "Which journal.", "a directory with a journal.json in the journals directories (the first that has it), or a journal directory's path (anything with a slash)."),
	opt("--entry E,...", "every entry, in order", "Builds only these entries.", "entry names, separated by commas, as --entry letter_1."),
	opt("--page P,...", "every page", "Renders only these pages of each entry built; other pages' existing output is kept.", "page indices (from 0) or names, separated by commas, as --page 0,maze."),
	opt("--scale S", "1", "Scales output images, brushes included; Darkly work scales with output size, so 0.25 renders much faster.", "a positive number."),
	opt("--supersample K", "1.2", "Renders Darkly output K times larger and filters it back down, smoothing thin, aliased strokes; render time grows with K squared.", "a number of at least 1; 1 turns it off."),
	opt("--svg", "off", "Also renders each canvas as an SVG, PAGE_CANVAS.svg, and that SVG as a PNG.", "a flag."),
	opt("--strict-glyphs", "off", "Fails before rendering if any character had no glyph, rather than drawing it as X and warning.", "a flag."),
	opt("--parchment", "off", "Renders each page as one image, darkly_PAGE.png, its canvases layered on procedural parchment; light inks are drawn dark, and ink soaks in a little unevenly, very slightly thinner in places.", "a flag."),
	opt("--pdf", "off", "Puts each entry's pages in one PDF, ENTRY.pdf, a page each at 150 canvas units to the inch, rather than writing images; on parchment with --parchment; with --stitch N, N pages side by side to a PDF page.", "a flag."),
	opt("--stitch N", "off", "Also lays every N pages side by side, in order, tops level, named for the pages they hold: page_0 to page_5 as darkly_page_0-5.png (the last may have fewer; a page left alone is not stitched). Pages are rendered one image each, as with --parchment, on parchment with it. With --pdf, the PDF's pages are the stitches instead, a page left alone at the end a page of its own.", "a whole number of at least 1."),
	opt("--journals-dir DIR", "the repository's journals/, then webapp/journals/ (the web editor's built-in journals)", "Where journals are, in place of the default's two.", "a directory."),
	opt("--library DIR", "the repository's webapp/library/", "The built-in library the journal's files stand in front of.", "a directory."),
	opt("--output-dir DIR", "NAME_output in the current directory", "Where renders go, a directory for each entry. The default's entry directories are emptied first, except with --page; in a given directory, earlier .png, .svg and .pdf renders in an entry's directory are cleared first, except with --page.", "a directory."),
]);
doc += api("journalbuild --check-locks [NAME] [--journals-dir DIR] [--library DIR]", "Runs every lock file's examples, and every chain file's, through lock and unlock instead of building a journal: the library's, or with NAME, the journal's own.", [
	opt("NAME", "none: the library's", "Which journal's lock and chain files.", "a journal, as journalbuild NAME takes it."),
	opt("--journals-dir DIR, --library DIR", "the repository's", "As for journalbuild NAME.", "a directory."),
]);
doc += api("journalbuild --check-gpu", "Checks that Darkly can render here, that there's a GPU wgpu can use, instead of building a journal. Fails if there isn't one; --svg renders still work without one.");

doc.trim();

// ---- typesetting ----

fn reference_page(n, info) {
	column([
		if n == 0 {
			column([
				flow("title", #{ flex: #{ height: 130 }, role: "heading", paragraph: #{ fit: true } }),
				flow("subtitle", #{ paragraph: #{ align: "center" } }),
				rule(#{ flex: #{ height: 10, margin: [10, 250] } }),
			], #{ flex: #{ gap: 10 } })
		} else {
			column([
				label("api reference", #{ flex: #{ height: 50 }, role: "head" }),
				rule(#{ flex: #{ height: 6 } }),
			], #{ flex: #{ gap: 6 } })
		},
		flow("body", #{ flex: #{ grow: 1 } }),
		label(`${n + 1}`, #{ flex: #{ height: 40 }, role: "folio", paragraph: #{ align: "center" } }),
	], #{ flex: #{ padding: [80, 100, 60, 100], gap: 14 } })
}

let pages = new_layout("reference", Fn("reference_page")).typeset(#{
	name: "api",
	width: W, height: H,
	flows: #{
		title: "API Reference",
		subtitle: "every function a journal script can call",
		body: markdown(doc),
	},
	roles: #{
		heading: #{ style: HEADING, type: new_type_style(LOCHE, 90), paragraph: #{ align: "center", valign: "center" } },
		subtitle: #{ style: TEXT, type: new_type_style(PLAIN, 30) },
		head: #{ style: HEADING, type: new_type_style(LOCHE, 40) },
		body: #{ style: TEXT, type: new_type_style(PLAIN, 26, #{ glyph_tracking: 0.04 }), paragraph: #{ line_height: 1.3, paragraph_gap: 0.6 } },
		rule: #{ style: CAPTION },
		folio: #{ style: TEXT, type: new_type_style(PLAIN, 26) },
	},
});
journal_entry.add(pages);

// Every character above has a glyph.
if missing_glyphs().len() > 0 {
	warn("characters with no glyph: " + missing_glyphs());
}
