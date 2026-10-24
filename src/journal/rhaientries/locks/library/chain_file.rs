//! Chain files: lock types wired together, with the illustrations that draw
//! their values, saved as a file of their own, `<chains dir>/<name>.rhai`,
//! for journals to use with `chain(name)`. A chain file defines
//!
//! ```rhai
//! fn options() { #{ shift: integer(3) } }                 // optional
//! fn chain(o) {
//!     #{
//!         locks: #{ shift: #{ type: "shift_cipher", options: #{ base: 27, shift: o.shift } }, ... },
//!         connect: [["shift.cipher_text", "vig.key"]],
//!         fixed: #{ "vig.plain_text": symbols(27, "HELLO") },
//!         illustrations: #{ msg: #{ type: "trit_message", options: #{ base: 27 }, inputs: #{ message: "vig.cipher_text" } } },
//!     }
//! }
//! fn layout() { #{ shift: [120, 60], vig: [480, 60], msg: [480, 260] } }  // optional
//! fn example() { #{ "shift.plain_text": symbols(27, "KEY") } }            // optional
//! ```
//!
//! Its values are its locks' own, "lock.port", as `new_lock` names them.
//! It runs in the lock files' engine, so `chain(o)` and `example()` write
//! values as lock files do; illustrations are only named here, and load in the
//! journal's engine when a journal uses the chain.

use super::chain::{connections, lock_entries, map, port_type};
use super::{check_options, from_script, same_value, show, sorted, valid_type_name, ExampleResults, LockLibrary, LockType};
use super::super::compose;
use crate::lockbuilder::{CompositeLock, InfoVarSet, InfoVarTypeSet, Lock};
use rhai::{Dynamic, Map};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// The keys a chain file's `chain(o)` returns.
const KEYS: [&str; 4] = ["locks", "connect", "fixed", "illustrations"];

/// What a chain file's `chain(o)` gives, for some options.
pub struct ChainSpec {
    pub name: String,
    /// The file, as errors show it: chains/NAME.rhai.
    pub file: String,
    /// Each lock's name, type and options.
    pub locks: Vec<(String, String, Map)>,
    pub connect: Vec<(String, String)>,
    /// Values for inputs, as `chain(o)` wrote them (symbols as arrays).
    pub fixed: Vec<(String, Dynamic)>,
    pub illustrations: Vec<IllustrationSpec>,
}

/// An illustration a chain file attaches: its id, type and options, and the
/// port of the chain each of its inputs draws.
pub struct IllustrationSpec {
    pub id: String,
    pub illustration: String,
    pub options: Map,
    pub inputs: Vec<(String, String)>,
}

impl LockLibrary {
    /// The chain files, sorted.
    pub fn chain_names(&self) -> Result<Vec<String>, String> {
        let names = self.chains.names()
            .map_err(|e| format!("couldn't read the chain directory {}: {}", self.chains.root().display(), e))?;
        Ok(names.into_iter().filter(|name| valid_type_name(name)).collect())
    }

    /// Whether there's a chain file called `name`.
    pub fn has_chain(&self, name: &str) -> bool {
        valid_type_name(name) && self.chains.has(name)
    }

    /// A chain file's source.
    pub fn chain_source(&self, name: &str) -> Result<String, String> {
        self.chain_file(name)?;
        self.chains.read(name).map_err(|e| format!("chains/{}.rhai: {}", name, e))
    }

    pub(super) fn chain_file(&self, name: &str) -> Result<Rc<LockType>, String> {
        if let Some(file) = self.chain_files.borrow().get(name) {
            return Ok(file.clone());
        }
        if !self.has_chain(name) {
            let names = self.chain_names()?;
            return Err(format!("no chain '{}'; chains: {}", name, if names.is_empty() { "none".to_string() } else { names.join(", ") }));
        }
        let file = Rc::new(self.load_chain(name)?);
        self.chain_files.borrow_mut().insert(name.to_string(), file.clone());
        Ok(file)
    }

    fn load_chain(&self, name: &str) -> Result<LockType, String> {
        let file = format!("chains/{}.rhai", name);
        let ast = self.chains.compile(&self.engine, name).map_err(|e| format!("{}: {}", file, e))?;
        let mut chain_file = LockType { name: name.to_string(), file, ast, options: BTreeMap::new(), composite: true };
        if !chain_file.defines("chain", 1) {
            return Err(format!("{} doesn't define chain(o)", chain_file.file));
        }
        let lock_fns: Vec<&str> = [("inputs", 1), ("outputs", 1), ("lock", 2), ("unlock", 2)].into_iter()
            .filter(|(function, params)| chain_file.defines(function, *params))
            .map(|(function, _)| function)
            .collect();
        if !lock_fns.is_empty() {
            return Err(format!("{} is a chain file, so it can't define {}: lock types go in locks/", chain_file.file, lock_fns.join(", ")));
        }
        if chain_file.defines("options", 0) {
            chain_file.options = self.declared_options(&chain_file)?;
        }
        Ok(chain_file)
    }

    /// What chain file `name` gives for `options` (checked against those it
    /// declares, defaults filled in).
    pub fn chain_spec(&self, name: &str, options: Map) -> Result<ChainSpec, String> {
        let chain_file = self.chain_file(name)?;
        let options = check_options(&chain_file.options, options).map_err(|e| format!("{}: {}", chain_file.file, e))?;
        let spec = self.call(&chain_file, "chain", (options,)).map_err(|e| e.located(&chain_file.file))?;
        parse(name, &chain_file.file, spec).map_err(|e| format!("{}: chain(o): {}", chain_file.file, e))
    }

    /// The chain's locks, wired and with their fixed values, without its
    /// illustrations: what running it locks with.
    pub fn chain_lock(self: &Rc<Self>, spec: &ChainSpec) -> Result<CompositeLock, String> {
        let at = |e: String| format!("{}: {}", spec.file, e);
        let mut locks: HashMap<String, Rc<dyn Lock>> = HashMap::new();
        for (id, type_name, options) in &spec.locks {
            locks.insert(id.clone(), Rc::new(self.build(id, type_name, options.clone()).map_err(at)?));
        }
        let mut fixed = Vec::new();
        for (port, value) in &spec.fixed {
            let var_type = port_type(&locks, port, |lock| lock.inputs())
                .ok_or_else(|| at(format!("fixed: '{}' isn't an input of the chain's locks", port)))?;
            fixed.push((port.clone(), from_script(value.clone(), &var_type).map_err(|e| at(format!("fixed: '{}': {}", port, e)))?));
        }
        for illustration in &spec.illustrations {
            for (input, port) in &illustration.inputs {
                if port_type(&locks, port, |lock| lock.outputs()).or_else(|| port_type(&locks, port, |lock| lock.inputs())).is_none() {
                    return Err(at(format!("illustrations: '{}' draws '{}' at its input '{}', which isn't a port of the chain's locks", illustration.id, port, input)));
                }
            }
        }
        compose(&spec.name, locks, spec.connect.clone(), &fixed).map_err(at)
    }

    /// The inputs running the chain needs: its locks' open inputs, not fixed.
    pub fn chain_inputs(spec: &ChainSpec, lock: &CompositeLock) -> InfoVarTypeSet {
        lock.inputs().into_iter().filter(|(port, _)| !spec.fixed.iter().any(|(fixed, _)| fixed == port)).collect()
    }

    /// Run every chain file's example (see `check_chain`).
    pub fn check_all_chains(self: &Rc<Self>) -> Result<ExampleResults, String> {
        Ok(self.chain_names()?.into_iter()
            .map(|name| {
                let result = self.check_chain(&name);
                (name, result)
            })
            .collect())
    }

    /// Build chain file `name` with its defaults and, if it has an
    /// example, lock it and unlock what that gives, which must be the
    /// example's inputs back. Returns how many examples it has, 0 or 1.
    pub fn check_chain(self: &Rc<Self>, name: &str) -> Result<usize, String> {
        let chain_file = self.chain_file(name)?;
        let spec = self.chain_spec(name, Map::new())?;
        let lock = self.chain_lock(&spec)?;
        if !chain_file.defines("example", 0) {
            return Ok(0);
        }
        let at = |e: String| format!("{}: example(): {}", chain_file.file, e);
        let given = self.call(&chain_file, "example", ())
            .map_err(|e| e.located(&chain_file.file))?
            .try_cast::<Map>()
            .ok_or_else(|| at("must return a map of the chain's open inputs to values, #{ \"lock.port\": value }".into()))?;
        let open = Self::chain_inputs(&spec, &lock);
        if let Some(unknown) = given.keys().find(|port| !open.contains_key(port.as_str())) {
            return Err(at(format!("'{}' isn't an open input of the chain; open: {}", unknown, sorted(open.keys()).into_iter().cloned().collect::<Vec<_>>().join(", "))));
        }
        let mut inputs = InfoVarSet::new();
        for port in sorted(open.keys()) {
            let value = given.get(port.as_str()).ok_or_else(|| at(format!("lacks '{}'", port)))?;
            inputs.insert(port.clone(), from_script(value.clone(), &open[port]).map_err(|e| at(format!("'{}': {}", port, e)))?);
        }
        let outputs = lock.lock_info(&inputs).map_err(|e| at(e.to_string()))?;
        let back = lock.unlock_info(&outputs).map_err(|e| at(format!("unlocking: {}", e)))?;
        for port in sorted(inputs.keys()) {
            let given = &inputs[port];
            let back = back.get(port).ok_or_else(|| at(format!("unlocking didn't give back '{}'", port)))?;
            if !same_value(&back.data, &given.data) {
                return Err(at(format!("unlocking gave back '{}' as {}, not {}", port, show(&back.data, &given.var_type), show(&given.data, &given.var_type))));
            }
        }
        Ok(1)
    }
}

/// What `chain(o)` returned, checked for its shape.
pub(super) fn parse(name: &str, file: &str, spec: Dynamic) -> Result<ChainSpec, String> {
    let spec = spec.try_cast::<Map>().ok_or("must return a map, #{ locks, connect, fixed, illustrations }")?;
    if let Some(unknown) = spec.keys().find(|key| !KEYS.contains(&key.as_str())) {
        return Err(format!("returned '{}'; it returns {}", unknown, KEYS.join(", ")));
    }
    let locks = lock_entries(&spec)?;
    let fixed = map(&spec, "fixed", false)?.into_iter().map(|(port, value)| (port.to_string(), value)).collect();
    let mut illustrations = Vec::new();
    for (id, entry) in map(&spec, "illustrations", false)? {
        if id.is_empty() || id.contains('.') {
            return Err(format!("illustrations: '{}' can't be an illustration's id; ids must be non-empty, without '.'", id));
        }
        if locks.iter().any(|(lock, _, _)| lock == id.as_str()) {
            return Err(format!("illustrations: '{}' is a lock's name too", id));
        }
        let bad = |what: &str| format!("illustrations: '{}' {}", id, what);
        let entry = entry.try_cast::<Map>().ok_or_else(|| bad("should be a map, #{ type, options, inputs }"))?;
        if let Some(unknown) = entry.keys().find(|key| !["type", "options", "inputs"].contains(&key.as_str())) {
            return Err(bad(&format!("has '{}'; an illustration takes type, options and inputs", unknown)));
        }
        let illustration = entry.get("type").and_then(|t| t.clone().into_string().ok()).ok_or_else(|| bad("needs its type, an illustration's name"))?;
        let options = match entry.get("options") {
            None => Map::new(),
            Some(options) => options.clone().try_cast::<Map>().ok_or_else(|| bad("has options that aren't a map"))?,
        };
        let inputs = match entry.get("inputs") {
            None => Map::new(),
            Some(inputs) => inputs.clone().try_cast::<Map>().ok_or_else(|| bad("has inputs that aren't a map of its inputs to ports"))?,
        };
        let inputs = inputs.into_iter()
            .map(|(input, port)| Ok((input.to_string(), port.into_string().map_err(|_| bad(&format!("gives its input '{}' a port that isn't a string", input)))?)))
            .collect::<Result<Vec<_>, String>>()?;
        illustrations.push(IllustrationSpec { id: id.to_string(), illustration, options, inputs });
    }
    Ok(ChainSpec { name: name.to_string(), file: file.to_string(), locks, connect: connections(&spec)?, fixed, illustrations })
}
