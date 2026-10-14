// The API reference: every function journal scripts can call, with its
// arguments, its options and what it gives back, typeset as a book.
//
// Each entry is api("signature", "description"): the signature in red,
// then what it does. tests/api_reference.rs fails when a function scripts
// can call isn't in one of the signatures here; when the API changes
// (a function, an argument, an option, a property, a default), change its
// entry in the same change: see CLAUDE.md. Signatures can't hold a double
// quote. journals/showcase shows what each call draws; this says what
// each one takes.
//
// Render it with: journalbuild api_reference (add --pdf for one file).

// ---- the book's look, as the showcase's ----

const W = 1200.0;
const H = 1600.0;
const INK = "#2b1d12";
const RED = "#9b2318";

// Constant pressure, and text no smaller than 26, so the plain font's dots
// (strokes about a unit long) show.
let pen = new_brush("Ink Pen", 0.4, 0.7, 1.5);
pen.pressure_profile = "constant";
const TEXT = new_style(1.0, INK, pen);
const CAPTION = new_style(1.0, RED, new_brush("Ink Pen", 0.3, 0.7, 1.2));
let nib = new_brush("Calligraphy", 0.4, 0.8, 1.5);
nib.pressure_profile = "constant";
const HEADING = new_style(1.2, RED, nib);

const PLAIN = font("plain");
const LOCHE = font("loche_upper");

// ---- entries ----

// Text as it is, with markdown's marks escaped.
fn esc(s) {
	let t = s;
	for mark in ["\\", "*", "_", "~", "[", "]", "{", "}"] {
		t.replace(mark, "\\" + mark);
	}
	t
}

// A section's heading.
fn heading(title) {
	"[" + esc(title) + "]{size: 1.6, color: #9b2318, underline: {color: #9b2318}}\n\n"
}

// A paragraph of prose.
fn para(text) {
	esc(text) + "\n\n"
}

// One function, property or operator: its signature, then what it does.
fn api(signature, description) {
	"[" + esc(signature) + "]{color: #9b2318}\n" + esc(description) + "\n\n"
}

let doc = "";

// ---- reading this ----

doc += heading("Reading This Reference");
doc += para("A journal is a rhai script. It draws strokes onto canvases, puts canvases on pages, and adds the pages to journal_entry, which journalbuild renders: one image per canvas, or one per page on parchment, or a PDF. This book lists every function a script can call, by what it works on. Each entry gives a signature in red and then what it does.");
doc += para("In signatures, receiver.method(arguments) is called on a value, as path.line_to(point); a bare name(arguments) is a global function. An argument written options is a map, #{ key: value, ... }, whose keys are all optional unless the entry says otherwise; unknown keys are errors. A name = value entry is a property: it can be read, and set unless the entry says it's read only.");
doc += para("Units: coordinates are canvas units, one pixel at --scale 1, with y running down the page. Angles are radians; PI() is rhai's. Numbers can be whole or decimal anywhere a number is taken.");
doc += para("Shared values: paths, glyphs, canvases and avoidance trees are shared. Assigning one, passing it or looping over it gives the same value, so changes through any copy reach it; copy() makes an independent one. Their transforms change the value and return it, so calls chain: p.move_to(a).line_to(b). Vecs, fonts, brushes, styles and pages are plain values: changing one changes that copy only.");
doc += para("Randomness is seeded: random(), glyph variant picks, text layout and hatching all draw on one generator with a fixed seed, so a journal renders the same every run. The procedural art takes seeds of its own.");
doc += para("Sections: the entry and its pages; canvases and strokes; vectors; paths; brushes and styles; gradients; fonts and glyphs; text; keeping text clear; randomness and messages; procedural art; page layouts; marked-up text; locks; lock and layout files; building.");

// ---- the entry and its pages ----

doc += heading("The Entry and Its Pages");
doc += para("A script starts with journal_entry, an empty entry, in scope. Whatever journal_entry holds when the script ends is rendered.");
doc += api("journal_entry.add_page(page)", "Adds a page to the entry. Each page's name, or page_N without one, names its output files; names are letters, digits, underscores, hyphens and dots, not leading, and must be unique.");
doc += api("journal_entry.add_pages(pages)", "Adds an array of pages, such as split tiles or typeset pages.");
doc += api("new_page()", "A new, empty page.");
doc += api("page.add_canvas(canvas)", "Adds a copy of the canvas, as it is now, to the page. Each canvas renders as an image of its own, darkly_PAGE_CANVAS.png, named after the page and the canvas's name or index.");
doc += api("page.name = text", "The page's name in output files, or () for its index.");
doc += api("page.size = new_vec(width, height)", "A set size: the page is exactly the region from the origin to this, and strokes past its edges are cut off. () (the default) makes the page as big as its strokes.");
doc += api("page.margin = number", "Space round each canvas's strokes, in canvas units, 0 for none. () (the default) is half the largest brush size on the canvas, enough that brush marks at the edges aren't cut off.");
doc += api("page.svg_style = options", "How the page's SVG output (--svg) is drawn: () for plain strokes, or a map of roughness, point_sample_distance (positive), trail_off_percent and noise_amplitude, none negative; keys left out keep their defaults.");
doc += api("page.split(columns, rows)", "Cuts a named page into a grid of tiles, each a page of its own named NAME_ROW_COLUMN (counting from 0), row by row. Each tile renders exactly its share of the pixels of the whole page's render, strokes crossing a seam drawn whole and cut off at it, so a canvas's tiles placed side by side make up its render. Gives the tiles as an array.");
doc += api("page.split(options)", "As above, with #{ columns, rows } or #{ tile_width, tile_height } for tiles of a set size (the grid grows evenly round the page to a whole number of them), or one of each, across and down; each is 1 tile by default.");
doc += api("page.tile", "Read only: a tile's place in its page's grid, #{ row, column, rows, columns }, or () for a page that isn't a tile.");

// ---- canvases ----

doc += heading("Canvases and Strokes");
doc += para("A canvas holds strokes, each a path drawn in a style. It's cropped to its strokes when rendered, plus the page's margin.");
doc += api("new_canvas()", "A new, empty canvas.");
doc += api("canvas.add_path(path, style)", "Adds a copy of the path, drawn in the style. Paths with infinite or NaN points are refused.");
doc += api("canvas.add_glyph(glyph, style)", "Adds a copy of a placed glyph's strokes, drawn in the style. Blank glyphs (spaces) add nothing.");
doc += api("canvas.add_glyphs(glyphs, style)", "Adds an array of glyphs, as add_glyph does each.");
doc += api("canvas.add_stroke(stroke)", "Adds a copy of a stroke: a path with its style.");
doc += api("canvas.add_canvas(other)", "Adds a copy of every stroke on another canvas, in its own style.");
doc += api("new_stroke(path, style)", "A stroke: a copy of the path, with the style it's drawn in.");
doc += api("stroke.style = style", "The stroke's style.");
doc += api("canvas.name = text", "The canvas's name in output files, or () for its index on the page.");
doc += api("canvas.bounds()", "What the canvas's strokes cover, their widths included: #{ min, max, center, width, height }, min, max and center being vecs.");
doc += api("canvas.translate_by(offset)", "Moves every stroke by a vec. Returns the canvas.");
doc += api("canvas.scale_by(factor)", "Scales every stroke about the origin. Returns the canvas.");
doc += api("canvas.rotate_around(point, angle)", "Turns every stroke about a point by an angle. Returns the canvas.");
doc += api("canvas.copy()", "An independent copy of the canvas.");

// ---- vectors ----

doc += heading("Vectors");
doc += para("Vecs are points and directions. They're values: methods give new vecs and leave the one they're called on as it was.");
doc += api("new_vec(x, y)", "A vec. Infinite and NaN coordinates (from a division by zero, say) are refused.");
doc += api("v.x = number, v.y = number", "Its coordinates.");
doc += api("a + b, a - b, -a", "Sums, differences and negation.");
doc += api("v * number, number * v, v / number", "Scaled by a number, whole or decimal.");
doc += api("a == b, a != b", "Whether two vecs are the same.");
doc += api("v.length()", "How long it is.");
doc += api("v.distance(other)", "How far it is from another vec.");
doc += api("v.dot(other)", "The dot product.");
doc += api("v.angle()", "Its angle from the x axis, in radians; with y down, positive angles turn clockwise.");
doc += api("v.normalized()", "A vec of length 1, the same way. Fails for the zero vec.");
doc += api("v.rotated(angle)", "Turned about the origin by an angle.");
doc += api("v.to_string(), v.to_debug()", "The vec as text, for print and debug.");

// ---- paths ----

doc += heading("Paths");
doc += para("A path is lines and curves, in one or more subpaths. Every path function changes the path and returns it, so statements, assignments and chains all work.");
doc += api("new_path()", "A new, empty path.");
doc += api("path.move_to(point)", "Starts a new subpath at a point.");
doc += api("path.line_to(point)", "A straight line to a point.");
doc += api("path.quad_to(control, point)", "A quadratic curve to a point, bent toward a control point.");
doc += api("path.cubic_to(control1, control2, point)", "A cubic curve to a point, with two control points.");
doc += api("path.close()", "Closes the subpath back to its start.");
doc += api("path.append(other)", "Adds a copy of another path's subpaths, so several shapes are one path: to hatch a shape with holes, say.");
doc += api("new_line(start, end)", "A straight line between two points.");
doc += api("new_circle(center, radius)", "A circle.");
doc += api("new_rectangle(center, width, height)", "A rectangle about its centre.");
doc += api("new_arc(center, radius, start_angle, end_angle)", "An arc of a circle, between two angles.");
doc += api("path.bounds()", "What the path covers: #{ min, max, center, width, height }. Fails for an empty path.");
doc += api("path.translate_by(offset)", "Moves the path by a vec.");
doc += api("path.scale_by(factor)", "Scales the path about the origin.");
doc += api("path.rotate_around(point, angle)", "Turns the path about a point.");
doc += api("path.copy()", "An independent copy of the path.");
doc += api("path.hatch(spacing, angle, roughness)", "Shading for the inside of a closed path, as a new path (the path itself is unchanged): lines spacing apart at angle, their ends jittered by up to roughness. Each line is a stroke of its own. Every subpath is taken as a closed shape, and shapes inside or across others cut holes in them.");
doc += api("path.hatch(options)", "Hatching with options; only spacing is needed. angle: an angle, or an array of them for a layer each (cross-hatching), default PI() / 4. roughness: ends jittered by up to this, default 0. bow: lines bend by up to this from straight, default 0. inset: lines stop this far inside the outline, default 0. fill_rule: holes (the default), evenodd (which also leaves out where a single subpath crosses itself) or nonzero (overlapping shapes merge, as SVG fills do).");
doc += api("path.to_string(), path.to_debug()", "The path's commands as text, for print and debug.");

// ---- brushes and styles ----

doc += heading("Brushes and Styles");
doc += para("A brush is a Darkly brush with pressures and a size; a style is a brush with a width and a colour, and is what strokes are drawn in.");
doc += api("brush_names()", "The name of every Darkly brush, sorted.");
doc += api("new_brush(name, min_pressure, max_pressure, size)", "A brush. The name must be one of brush_names(); size is how wide its marks are, and pressure runs from min_pressure to max_pressure along each stroke.");
doc += api("brush.brush_style = name", "The Darkly brush's name.");
doc += api("brush.min_pressure = number, brush.max_pressure = number", "The least and most pressure along a stroke.");
doc += api("brush.size = number", "How wide its marks are, in canvas units.");
doc += api("brush.pressure_profile = name", "How pressure runs along each stroke: ramp (min to max, the default), taper (min to max at the middle and back) or constant (max).");
doc += api("brush.pressure_variation = number", "Pressure wanders up to this fraction either way along a stroke, so ramps and tapers look less mechanical. Default 0.");
doc += api("brush.variation_length = number", "How far apart, in canvas units, the wandering pressure rises and falls. Default 12.");
doc += api("brush.pool = number", "Ink pooling: strokes this much heavier at the very bottom of each path (each glyph, for text). Default 0.");
doc += api("brush.pool_depth = number", "How far up the pooling rises, as a fraction of the path's height. Default 0.3.");
doc += api("new_style(width, color, brush)", "A style. In Darkly renders width multiplies the brush's pressure, so past 1 / max_pressure a stroke gets no heavier (the render warns); in SVG it's the stroke's width in canvas units. color is a CSS colour or a gradient.");
doc += api("style.width = number", "The style's width.");
doc += api("style.color = color", "A CSS colour string, or a gradient.");
doc += api("style.passes = number", "Draws each stroke in the style this many times over, as a hand going over its writing. Default 1; there's a limit on how many.");
doc += api("style.jitter = number", "How far each pass's ends stray, in canvas units, seeded from each stroke's own points so it's the same every run. With one pass it just roughens a hand. Default 0.");

// ---- gradients ----

doc += heading("Gradients");
doc += api("new_gradient(colors)", "A gradient along each stroke from its start to its end, to use in place of a colour. Each colour is a CSS colour, spaced evenly, or [position, colour] with the position from 0 to 1; unplaced colours are spaced evenly between their neighbours.");
doc += api("new_gradient(colors, options)", "With #{ from, to } (two vecs) laid across the page between two points; with #{ center, radius } out from a point; with #{ vertical: true } down each path (each glyph, for text) from its top to its bottom. In Darkly renders a gradient stroke keeps its brush's texture; in SVG, gradients along a stroke are drawn in short pieces.");
doc += api("gradient.to_string(), gradient.to_debug()", "The gradient as text, for print and debug.");

// ---- fonts and glyphs ----

doc += heading("Fonts and Glyphs");
doc += para("Fonts come from config/fonts.json, each with its glyphs and how text is set in it; sets of symbols are fonts too. A font is a value: changing one changes that copy. A character is looked for as it is in a font and its fallbacks, then in the other case, so a font of capitals sets lowercase text in them.");
doc += api("font(name)", "A font, as the font file sets it.");
doc += api("font(name, options)", "A font with settings changed: character_padding, space_width, skew and fallback, as the properties below.");
doc += api("font_names()", "Every font's name, sorted.");
doc += api("ternary_font(name)", "A font whose letters are balanced-ternary digits: a to m are 1 to 13, n to z are -13 to -1, drawn with the named font's glyphs.");
doc += api("font.name", "Read only: the font's name.");
doc += api("font.character_padding = number", "Room between glyphs' outlines, in font sizes. Text is set with each glyph as close to the last as their outlines allow, plus this.");
doc += api("font.space_width = number", "A space's width, in font sizes.");
doc += api("font.skew = angle", "Leans the font's glyphs, and its fallbacks', into italics: radians, tops forward, short of a right angle either way. Spacing isn't opened up.");
doc += api("font.fallback = font", "Where characters the font hasn't are looked for: a font, a font's name, or () for nowhere.");
doc += api("font.characters()", "The characters the font has glyphs for.");
doc += api("font.glyphs(text)", "A text's glyphs, each at the origin with scale 1, variants picked from the seeded generator.");
doc += api("font.glyph(name)", "One glyph, by its character or name, or a number (as ternary.glyph(-13)).");
doc += api("font.variants(name)", "Every variant of a glyph.");
doc += api("missing_glyphs()", "Characters drawn with the fallback X so far, for want of a glyph: #{ CHARACTER: #{ count, tried } }. journalbuild --strict-glyphs fails on them instead.");
doc += para("A glyph is a symbol placed with a position, scale, rotation and skew. Its path spans -1 to 1 and is drawn at half its scale, so scale is its size in canvas units.");
doc += api("glyph.position = vec", "Where its centre is.");
doc += api("glyph.scale = number", "Its size in canvas units.");
doc += api("glyph.rotation = angle", "Its turn about its own centre.");
doc += api("glyph.skew = angle", "Its lean about its centre, before it's turned.");
doc += api("glyph.is_blank", "Read only: whether it draws nothing, as a space.");
doc += api("glyph.translate_by(offset)", "Moves it. Returns the glyph.");
doc += api("glyph.scale_by(factor)", "Multiplies its scale. Returns the glyph.");
doc += api("glyph.rotate_by(angle)", "Turns it further. Returns the glyph.");
doc += api("glyph.path()", "Its strokes, placed, as a new path; empty for blanks.");
doc += api("glyph.copy()", "An independent copy of the glyph.");

// ---- text ----

doc += heading("Text");
doc += para("Text layouts place glyphs, scaling each by the symbol scale and turning it to its line on top of any scale and rotation it has. Every one takes avoid and clearance (see Keeping Text Clear), and spacing: extra room between glyphs as a fraction of the symbol scale, negative to tighten.");
doc += api("new_textbox(options)", "A box glyphs flow into, line by line. center, width, height and symbol_scale are needed. layout: flat (glyph by glyph, the default) or words (wrapping at spaces). jagged_edge: a random line indent, in symbol scales. avoid, clearance, spacing. split_words: with words, break words round what's avoided rather than moving past it whole; true, or the fewest glyphs a piece may have (true is 2). split_padding: how much further the pieces keep from it, in symbol scales.");
doc += api("textbox.fill(glyphs)", "Places what fits of an array of glyphs: #{ placed, unplaced }, arrays of glyphs. Add the placed ones to a canvas with add_glyphs.");
doc += api("textbox.place_all(glyphs)", "Fills the box again and again until every glyph is placed: an array of arrays of placed glyphs, one per box. Fails if a glyph won't fit even an empty box.");
doc += api("path.fill_with_text(text, font, size, style)", "The text along the path, each glyph turned to it, over as many copies of the path as it takes: an array of canvases, one per copy.");
doc += api("path.fill_with_text(text, font, size, style, options)", "With spacing, avoid, clearance, layout (words keeps words from splitting between subpaths), split_words, split_padding, and angle: glyphs held at that angle (0 upright) rather than turned to the path.");
doc += api("path.fit_text(text, font, style)", "The text along the path at the largest size that fits it, as a canvas.");
doc += api("path.fit_text(text, font, style, options)", "With the same options as fill_with_text.");
doc += api("canvas.scatter_notes(messages, font, style, options)", "Scatters an array of messages over the canvas in random boxes, clear of its strokes and of each other; gives a canvas of just the messages. symbol_scale, width_range and height_range ([least, most] box sizes to try) are needed. rotation_jitter: radians either way, default PI() / 16. scale_jitter: a fraction either way, default 0.15. jagged_edge: default 0.1. max_attempts: boxes tried per message before backing off. spacing, avoid, clearance (from the canvas, avoid and other notes). layout: flat (the default) or words, each note in one clear patch.");

// ---- avoidance ----

doc += heading("Keeping Text Clear");
doc += para("avoid, in every text layout, is a tree, a path, a canvas or a glyph, or an array of any of these. Glyphs keep their whole outline (the hull of their strokes) at least clearance from all of it: by default an eighth of the symbol scale, on top of any clearance a tree was built with. Glyphs placed by one call keep clear of each other too.");
doc += api("new_avoidance_tree()", "An empty tree of what to keep clear of. Trees are shared: a text box made with one sees what's added to it later, so tree.add(placed) keeps later text off what's just been placed.");
doc += api("tree.add(item)", "Adds a path's lines, a canvas's marks, glyphs' outlines (insides included), another tree, or an array of any of these. Returns the tree.");
doc += api("tree.add(item, clearance)", "As above, kept this far from.");
doc += api("tree.add_hull(item), tree.add_hull(item, clearance)", "Keeps out of everything inside the item's outline, not just off its marks.");
doc += api("tree.add_region(path), tree.add_region(path, clearance)", "Keeps out of a closed shape.");
doc += api("tree.blocks(point), tree.blocks(glyph)", "Whether a point, or a placed glyph, is too close to something in the tree.");
doc += api("tree.is_empty", "Read only: whether the tree holds nothing.");
doc += api("tree.copy()", "An independent copy of the tree.");

// ---- randomness and messages ----

doc += heading("Randomness and Messages");
doc += api("seed(n)", "Reseeds the generator that random, glyph variants, text layout and hatching draw on.");
doc += api("random()", "A decimal from 0 up to 1.");
doc += api("random_range(low, high)", "A number from low up to high: a whole number if both ends are, a decimal if either is.");
doc += api("warn(message)", "Reports something, with the script line, without stopping the script. Errors give the script line too.");

// ---- art ----

doc += heading("Procedural Art");
doc += para("The art module draws shapes from a few parameters and a seed of their own: the same seed gives the same drawing, and the script's generator is neither used nor changed.");
doc += api("art::snake_path(head_center, head_angle, tail_end, length, max_width, seed)", "The outline of a snake seen from above, its head centred at head_center and facing head_angle, winding to the tip of its tail at tail_end, length long along its spine and max_width across at its widest. Where it crosses itself the strand beneath stops at the edge of the one on top (the head always goes over), so the path is several strokes.");
doc += api("art::snake_spine(head_center, head_angle, tail_end, length, max_width, seed)", "The same snake's centre line, from the back of its head to the tip of its tail: a path to write along.");
doc += api("art::snake_along(path, max_width, seed)", "A many-headed serpent along a path: its body follows the path, slithering a little off it and forking where it forks, its tail at the dead end nearest the path's start and a head at every other.");
doc += api("art::bramble(base, angle, length, flower_size, seed)", "A blackberry bramble growing from base toward angle, length long, with flowers about flower_size across, which everything else is sized by. Gives #{ vines, leaves, flowers }: canes, thorns and stalks; leaflets and veins; flowers, buds and sepals, as paths to stroke in different colours. Parts in front hide the lines behind them.");
doc += api("art::bramble_along(path, flower_size, seed)", "A bramble grown along a path, such as a maze's: vines follow its lines, branch at junctions and flower at its dead ends. Gives #{ vines, leaves, flowers }.");
doc += api("art::eye(center, length, color, gaze_angle, messiness, seed)", "An eye as a hand sketches one, as a canvas: lids gone over and running past the corners, an iris ringed and washed with color, and a line through the iris bowed toward where it looks. length is corner to corner; gaze_angle is which way it looks, on the page; messiness runs from neat (0) to as messy as it gets (1).");
doc += api("art::eye(center, length, color, gaze_angle, messiness, seed, options)", "With angle: the eye's tilt, default 0. gaze: how far it looks, from 0 (ahead) to 1, default 0.5. openness: its height over its length, default 0.32. ink: the style of its lines, default a near-black ink pen.");

// ---- layouts ----

doc += heading("Page Layouts");
doc += para("A layout describes pages as trees of flexbox boxes, and typeset pours text, art and ornaments into their slots, chaining text through columns and pages until it runs out. Typeset pages are named NAME_N and are exactly the layout's size; they split and go into PDFs like any page.");
doc += api("layout(name)", "Loads layouts/NAME.rhai (or from --layouts-dir): a layout file, which defines page(n, info) and optionally page_size().");
doc += api("new_layout(name, Fn(page_function))", "A layout from a function in the script, given a page number and info and giving that page's tree of boxes.");
doc += api("layout.name", "Read only: the layout's name.");
doc += api("layout.typeset(options)", "Typesets pages, giving them as an array. flows: text by name, each a string or markdown(text); a blank line starts a paragraph and a line break breaks a line. roles: how each kind of text and stroke looks, by name. art: canvases, paths, glyphs, or functions given their box, #{ x, y, width, height, center }, and giving a canvas or path. name: the pages' name, by default the layout's. width, height: the page size, if the layout has none. max_pages: default 200. Fails, naming the flow, if a page sets none of what's left.");
doc += para("A page function gets n (from 0) and info: #{ flows, art, width, height }, flows saying how many characters of each are left and art which art was given, so a layout can change as the text runs out.");
doc += api("row(children), row(style, children)", "Lays out an array of boxes across.");
doc += api("column(children), column(style, children)", "Lays out an array of boxes down.");
doc += api("flow(name), flow(name, style)", "A slot for text from a flow; what doesn't fit goes on in the next slot of the same name, then on the next page.");
doc += api("label(text), label(text, style)", "Fixed text, such as a page number.");
doc += api("initial(name), initial(name, style)", "A drop cap: the next letter of a flow, filling its box.");
doc += api("figure(name), figure(name, style)", "Art, fitted into the box.");
doc += api("band(name), band(name, style)", "Art repeated along the box as an ornamental strip, each copy scaled to the band's breadth and turned to run along it; turn: an angle turns it yourself.");
doc += api("rule(), rule(style)", "A line along the box.");
doc += api("spacer(), spacer(style)", "Nothing: room.");
doc += para("Box styles. Lengths are canvas units, a percentage of the parent as N%, or auto. On any box: width, height, min_width, min_height, max_width, max_height; grow, shrink, basis; margin and padding (one value, [vertical, horizontal], [top, right, bottom, left] or #{ top, right, bottom, left }); position: absolute with top, left, right, bottom; aspect (width over height); align_self (auto, start, end, center, stretch); border, a role whose stroke outlines the box. On rows and columns: gap; justify (start, end, center, space_between, space_around, space_evenly); align and align_content (start, end, center, stretch); wrap: true to wrap children onto more lines. On leaves: wrap, what text wrapping round it keeps clear of: box, hull (what's drawn) or none; role, and any role setting of its own. An absolutely placed box is out of the flow, and text in its parent wraps round it.");
doc += para("Roles. A slot's role is the one named after it (its flow's or art's name, or initial, label, rule) unless it names another with role. Text roles need font, style and size (the symbol scale). line_height: in sizes, default 1.25. align: left, center, right or justify. valign: top, center or bottom. indent: a paragraph's first line, in sizes. paragraph_gap: in lines, default 0.5. spacing: between glyphs, in sizes. jagged: lines start up to this much further in, at random. clearance: from what it wraps round, default size / 4. split_words, split_padding: as for text boxes. fit: shrink the text until it all fits. em, strong: how markdown's emphasis and strong text look. Art, bands, rules and borders need only a style.");

// ---- markdown ----

doc += heading("Marked-Up Text");
doc += api("markdown(text)", "A flow whose text is styled as it's set. *emphasis* or _emphasis_ looks as the role's em says, by default #{ italic: true }; **strong** or __strong__ as its strong says, by default gone over twice, astray by a twentieth of the size; ~~text~~ is struck through. [text]{settings} sets text with settings of its own, key: value pairs separated by commas, a key alone meaning true. Spans nest. A backslash writes a mark as it is.");
doc += para("Span settings. color (or colour): a CSS colour. passes, jitter: as a style's. italic: true or false. skew: radians. size: times the size round it; lines grow to fit. character_padding: room between glyphs' outlines, in sizes. word_spacing: a space's width, in sizes. underline, and strike (or strikethrough): true for one line, a number of lines, false for none, or settings: count, spacing (in sizes) and the color, passes and jitter they're drawn in. Lines under text go one below another; lines through it go back and forth as one stroke, scribbling it out. They run across the spaces between words and break where the text does.");

// ---- locks ----

doc += heading("Locks");
doc += para("Locks encipher text for a journal to draw. A chain is lock types wired together, each from a lock file in locks/; running it locks the text, and unlock recovers it.");
doc += api("new_lock(name)", "An empty lock chain. The chain is checked after every change, so a mistake fails at the line that made it.");
doc += api("lock.add(id, lock_type, options)", "Adds a lock of a type (a lock file's name) under an id, with that type's options. Returns the chain.");
doc += api("lock.connect(from, to)", "Wires one lock's output to another's input, each as ID.NAME; their types must match. Returns the chain.");
doc += api("lock.set_input(input, value)", "Fixes an input, as ID.NAME, to a lock value. Returns the chain.");
doc += api("lock.inputs()", "The inputs still needing a value when it's run: #{ NAME: type }.");
doc += api("lock.outputs()", "The chain's final outputs: #{ NAME: type }.");
doc += api("lock.run(inputs)", "Runs the chain on a map of ID.NAME to lock values: a lock result.");
doc += api("lock.unlock(result)", "Recovers the inputs from a run's result: a lock result.");
doc += api("lock.unlock(outputs)", "Recovers the inputs from a map of outputs given by hand. Chains with maze outputs need a run's result.");
doc += api("result.get(name)", "One value, as a lock value.");
doc += api("result.text(name)", "One value as text.");
doc += api("result.values()", "Every value: #{ NAME: lock value }.");
doc += api("result.trace()", "Every value computed along the way, intermediate ones included.");
doc += api("symbols(base, text)", "Text as a lock value of symbols in a base: base 10 takes digits, base 27 letters and spaces, other bases up to 26 the first base letters, any case.");
doc += api("value.symbols()", "The value's symbols, as an array of whole numbers.");
doc += api("value.text()", "The value as text, for symbols and strings.");
doc += api("value.base()", "The base of a value of symbols.");
doc += api("value.value()", "The value as lock files see it: symbols as an array of whole numbers, a maze as a maze.");
doc += api("value.type_name()", "The value's type, such as Base27Symbol or Maze.");
doc += api("value.maze()", "A maze output, for drawing.");
doc += api("value.to_string(), value.to_debug()", "The value as text, for print and debug.");
doc += api("new_maze(seed, width, height), new_maze(seed, width, height, strategy)", "A maze of width by height nodes (a grid of 2w - 1 by 2h - 1 cells), walked LeftHanded (the default) or RightHanded. Its layout depends only on the seed and size; its cells hold random symbols.");
doc += api("maze.width(), maze.height()", "Its grid size in cells, passages included.");
doc += api("maze.cells()", "Every cell in walking order, the order a lock writes its text: #{ x, y, step, symbol }. Nodes are at even coordinates, passages between them.");
doc += api("maze.fill(symbols)", "Writes an array of symbols into the cells in walking order, and 0 into any left over.");
doc += api("maze.path()", "A path along its corridors, in grid units, walked in the same order.");

// ---- files ----

doc += heading("Lock and Layout Files");
doc += para("A lock file, locks/TYPE.rhai, defines options() (option name to type: integer(), number(), string(), bool(), array(), or one_of([...]), each optionally with a default), optionally check(o), inputs(o) and outputs(o) (value name to type: symbol_type(base), Maze, String, Any, or a name of its own), lock(o, inputs) and unlock(o, outputs), and optionally examples(), run by --check-locks. A composite lock file defines options() and chain(o) instead. Lock files can call symbols, symbol_text, symbol_type and new_maze, and nothing random; see locks/README.md.");
doc += para("A layout file, layouts/NAME.rhai, defines page(n, info), giving page n's tree of boxes, and optionally page_size(), giving #{ width, height }. It runs in the journal's engine, so it can call everything a journal can; see layouts/README.md.");

// ---- building ----

doc += heading("Building");
doc += para("journalbuild NAME runs journals/NAME/journal_entry.rhai and renders its pages to its output directory. --page 0,maze: only these pages, by index or name. --scale 0.25: smaller output, much faster. --svg: also an SVG of each canvas, and a PNG of it. --strict-glyphs: fail on characters with no glyph. --parchment: each page as one image on procedural parchment. --pdf: the pages in one PDF. --journals-dir, --output-dir, --fonts, --locks-dir, --layouts-dir: read or write elsewhere. --check-locks: run every lock file's examples.");

doc.trim();

// ---- typesetting ----

fn reference_page(n, info) {
	column(#{ padding: [80, 100, 60, 100], gap: 14 }, [
		if n == 0 {
			column(#{ gap: 10 }, [
				flow("title", #{ role: "heading", height: 130, fit: true }),
				flow("subtitle", #{ align: "center" }),
				rule(#{ height: 10, margin: [10, 250] }),
			])
		} else {
			column(#{ gap: 6 }, [
				label("api reference", #{ role: "head", height: 50 }),
				rule(#{ height: 6 }),
			])
		},
		flow("body", #{ grow: 1 }),
		label(`${n + 1}`, #{ role: "folio", align: "center", height: 40 }),
	])
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
		heading: #{ font: LOCHE, style: HEADING, size: 90, align: "center", valign: "center" },
		subtitle: #{ font: PLAIN, style: TEXT, size: 30 },
		head: #{ font: LOCHE, style: HEADING, size: 40 },
		body: #{ font: PLAIN, style: TEXT, size: 26, line_height: 1.3, paragraph_gap: 0.6 },
		rule: #{ style: CAPTION },
		folio: #{ font: PLAIN, style: TEXT, size: 26 },
	},
});
journal_entry.add_pages(pages);

// Every character above has a glyph.
if missing_glyphs().len() > 0 {
	warn("characters with no glyph: " + missing_glyphs());
}
