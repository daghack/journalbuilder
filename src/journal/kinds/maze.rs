//! Maze: a maze, its cells holding symbols.
//!
//! ```rhai
//! #{ kind: "Maze", width: 5, height: 3, seed: 4, strategy: "LeftHanded", symbols: [8, 5, 12, ...] }
//! ```
//!
//! `width` and `height` are its grid in cells: nodes at even coordinates,
//! the passages between connected nodes at odd ones, so a maze of w by h
//! nodes is 2w - 1 by 2h - 1. Its corridors come from its seed and size
//! alone; `symbols` is what each cell holds, in the order `strategy`
//! ("LeftHanded" or "RightHanded") walks them. `new_maze(width, height)`
//! makes one (its symbols random, from the seed), `maze.cells()` reads it
//! and `maze.fill(symbols)` writes into it.

use rand::Fill;
use rhai::{Array, Dynamic, Engine, Map, NativeCallContext, INT};

use super::schema::{list, record, Schema};
use super::{fail, field, read_index, Label, Sketch, ValueKind};
use crate::journal::canvas::artistic::{BuiltMaze, MazeTraversalStrategy};
use crate::journal::paths::StrokePath;

pub struct Maze;

/// What a maze cell holds: a symbol, random in a new maze.
#[derive(Clone, Debug, Default)]
pub struct SymbolInfo {
    pub symbol: u32,
}

impl Fill for SymbolInfo {
    fn fill<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        self.symbol = rng.random_range(0..27);
    }
}

/// A maze value rebuilt: its corridors from its seed and size, its cells
/// holding its symbols.
pub struct Built {
    pub maze: BuiltMaze<SymbolInfo>,
    pub strategy: MazeTraversalStrategy,
    pub seed: u64,
}

impl Built {
    /// A maze of `width` by `height` nodes, its symbols random from the seed.
    pub fn new(seed: u64, width: u32, height: u32, strategy: MazeTraversalStrategy) -> Built {
        Built { maze: BuiltMaze::new(seed, width, height), strategy, seed }
    }

    /// A checked Maze value rebuilt.
    pub fn read(value: &Map) -> Result<Built, String> {
        let n = |key: &str| read_index(field(value, key), key);
        let strategy = strategy_of(&field(value, "strategy").to_string())?;
        let (width, height) = (n("width")?, n("height")?);
        let seed = field(value, "seed").as_int().map_err(|_| "seed should be a whole number")? as u64;
        let mut built = Built::new(seed, (width as u32).div_ceil(2), (height as u32).div_ceil(2), strategy);
        let symbols = field(value, "symbols").read_lock::<Array>().ok_or("symbols should be an array")?.clone();
        let cells = built.maze.all_cells().len();
        if symbols.len() != cells {
            return Err(format!("symbols has {} items, but a {} by {} maze has {} cells", symbols.len(), width, height, cells));
        }
        let symbols = symbols.iter().enumerate().map(|(i, s)| read_index(s, &format!("symbols[{}]", i)).map(|s| s as u32)).collect::<Result<Vec<_>, _>>()?;
        let mut next = symbols.into_iter();
        built.maze.map_inners(strategy, |inner| inner.symbol = next.next().unwrap_or(0));
        Ok(built)
    }

    /// Its grid's width and height in cells.
    pub fn size(&self) -> (i64, i64) {
        let cells = self.maze.all_cells();
        let most = |f: fn(&(i32, i32)) -> i32| cells.iter().map(|c| f(&c.position)).max().map_or(0, |v| v as i64 + 1);
        (most(|p| p.0), most(|p| p.1))
    }

    /// The maze as a Maze value.
    pub fn value(&self) -> Map {
        let (width, height) = self.size();
        let mut m = Map::new();
        m.insert("kind".into(), "Maze".into());
        m.insert("width".into(), Dynamic::from_int(width));
        m.insert("height".into(), Dynamic::from_int(height));
        m.insert("seed".into(), Dynamic::from_int(self.seed as INT));
        m.insert("strategy".into(), (if self.strategy == MazeTraversalStrategy::LeftHanded { "LeftHanded" } else { "RightHanded" }).into());
        m.insert("symbols".into(), Dynamic::from_array(self.maze.iter_inners(self.strategy).map(|(_, inner)| Dynamic::from_int(inner.symbol as INT)).collect()));
        m
    }

    /// Every cell in walking order: #{ x, y, step, symbol }.
    pub fn cells(&self) -> Array {
        self.maze.iter_inners(self.strategy).enumerate().map(|(step, ((x, y), inner))| {
            let mut cell = Map::new();
            cell.insert("x".into(), Dynamic::from_int(x as INT));
            cell.insert("y".into(), Dynamic::from_int(y as INT));
            cell.insert("step".into(), Dynamic::from_int(step as INT));
            cell.insert("symbol".into(), Dynamic::from_int(inner.symbol as INT));
            Dynamic::from_map(cell)
        }).collect()
    }

    /// A path along the corridors, in grid units, walked as the cells are.
    pub fn corridors(&self) -> StrokePath {
        self.maze.corridor_path(self.strategy)
    }
}

fn strategy_of(name: &str) -> Result<MazeTraversalStrategy, String> {
    match name {
        "LeftHanded" => Ok(MazeTraversalStrategy::LeftHanded),
        "RightHanded" => Ok(MazeTraversalStrategy::RightHanded),
        other => Err(format!("unknown strategy '{}'; expected \"LeftHanded\" or \"RightHanded\"", other)),
    }
}

/// A map checked as a Maze and rebuilt.
pub fn built(caller: &str, value: &Map) -> Result<Built, String> {
    let kind = Maze;
    super::check(&kind, &Dynamic::from_map(value.clone())).map_err(|err| format!("{}: a bad Maze: {}", caller, err))?;
    Built::read(value).map_err(|err| format!("{}: {}", caller, err))
}

impl ValueKind for Maze {
    fn name(&self) -> &'static str {
        "Maze"
    }

    fn schema(&self) -> Schema {
        record(&[
            ("kind", Schema::Kind("Maze")),
            ("width", Schema::Int),
            ("height", Schema::Int),
            ("seed", Schema::Int),
            ("strategy", Schema::Text),
            ("symbols", list(Schema::Int)),
        ])
    }

    fn validate(&self, value: &Map) -> Result<(), String> {
        for key in ["width", "height"] {
            let n = read_index(field(value, key), key)?;
            if n == 0 || n % 2 == 0 {
                return Err(format!("{} is in cells, so it's odd (2 nodes - 1), got {}", key, n));
            }
        }
        Built::read(value).map(|_| ())
    }

    /// `new_maze(width, height[, #{ seed, strategy }])`, `maze.cells()`
    /// and `maze.fill(symbols)`.
    fn globals(&self, engine: &mut Engine) {
        let new = |ctx: &NativeCallContext, width: INT, height: INT, options: &Map| -> Result<Map, Box<rhai::EvalAltResult>> {
            const CALLER: &str = "new_maze";
            let err = |m: String| fail(ctx, CALLER, m);
            if let Some(unknown) = options.keys().find(|k| !["seed", "strategy"].contains(&k.as_str())) {
                return Err(err(format!("unknown option '{}'; the options are seed, strategy", unknown)));
            }
            let seed = match options.get("seed") {
                None => 0,
                Some(s) => s.as_int().map_err(|t| err(format!("'seed' should be a whole number, got {}", t)))?,
            };
            let strategy = strategy_of(&options.get("strategy").map_or("LeftHanded".to_string(), |s| s.to_string())).map_err(err)?;
            let size = |n: INT, what: &str| u32::try_from(n).ok().filter(|&n| n >= 1).ok_or_else(|| err(format!("{} must be at least 1, got {}", what, n)));
            Ok(Built::new(seed as u64, size(width, "width")?, size(height, "height")?, strategy).value())
        };
        engine
            .register_fn("new_maze", move |ctx: NativeCallContext, w: INT, h: INT| new(&ctx, w, h, &Map::new()))
            .register_fn("new_maze", move |ctx: NativeCallContext, w: INT, h: INT, options: Map| new(&ctx, w, h, &options))
            .register_fn("cells", |ctx: NativeCallContext, maze: &mut Map| -> Result<Array, Box<rhai::EvalAltResult>> {
                Ok(built("cells", maze).map_err(|m| fail(&ctx, "maze", m))?.cells())
            })
            .register_fn("fill", |ctx: NativeCallContext, maze: &mut Map, symbols: Array| -> Result<(), Box<rhai::EvalAltResult>> {
                let mut b = built("fill", maze).map_err(|m| fail(&ctx, "maze", m))?;
                let cells = b.maze.all_cells().len();
                if symbols.len() > cells {
                    return Err(fail(&ctx, "fill", format!("{} symbols don't fit in the maze's {} cells", symbols.len(), cells)));
                }
                let symbols = symbols.iter().enumerate()
                    .map(|(i, s)| s.as_int().ok().and_then(|s| u32::try_from(s).ok()).ok_or_else(|| fail(&ctx, "fill", format!("item {} ({:?}) isn't a symbol", i, s))))
                    .collect::<Result<Vec<u32>, _>>()?;
                let mut next = symbols.into_iter();
                b.maze.map_inners(b.strategy, |inner| inner.symbol = next.next().unwrap_or(0));
                *maze = b.value();
                Ok(())
            });
    }

    fn example(&self) -> Option<&'static str> {
        Some("new_maze(3, 3, #{ seed: 1 })")
    }

    /// Its corridors, and each cell's symbol as a letter (· for 0).
    fn sketch(&self, value: &Map) -> Option<Sketch> {
        let b = Built::read(value).ok()?;
        let labels = b.maze.iter_inners(b.strategy).map(|((x, y), inner)| Label {
            at: parry2d::math::Vec2::new(x as f32, y as f32),
            text: if inner.symbol == 0 { "·".into() } else { char::from_u32(64 + inner.symbol).map_or("?".into(), |c| c.to_string()) },
        }).collect();
        Some(Sketch { parts: vec![("corridors".into(), b.corridors())], labels, label_size: 0.55 })
    }
}
