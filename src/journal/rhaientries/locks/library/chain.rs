//! Composite lock files: a lock type made of other lock types wired
//! together. Such a file defines `chain(o)` in place of `inputs`, `outputs`,
//! `lock` and `unlock`, returning
//!
//! ```rhai
//! #{
//!     locks: #{ shift: #{ type: "shift_cipher", options: #{ base: 27, shift: o.shift } }, ... },
//!     connect: [["shift.cipher_text", "vig.key"]],
//!     fixed: #{ "vig.plain_text": symbols(27, "HELLO") },
//!     inputs: #{ key: "shift.plain_text" },
//!     outputs: #{ cipher_text: "vig.cipher_text", shifted_key: "vig.key" },
//! }
//! ```
//!
//! `inputs` and `outputs` name the chain's open ports, the ones neither
//! connected nor fixed, as the lock type's own inputs and outputs.

use super::{from_script, LockLibrary};
use super::super::compose;
use crate::lockbuilder::{compatible, CompositeLock, InfoVar, InfoVarSet, InfoVarTypeSet, Lock};
use rhai::{Dynamic, Map};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::rc::Rc;

/// The keys `chain(o)` returns.
const KEYS: [&str; 5] = ["locks", "connect", "fixed", "inputs", "outputs"];

/// A composite lock: its locks wired together, and the ports it shows under
/// names of its own.
pub(super) struct Chain {
    lock: CompositeLock,
    /// Each input's name -> the port inside it feeds, "lock.port".
    inputs: BTreeMap<String, String>,
    /// Each output's name -> the port inside it comes from.
    outputs: BTreeMap<String, String>,
    /// Outputs inside that aren't shown, each passing a fixed input
    /// through: unlocking gives them the fixed value back.
    hidden: Vec<(String, InfoVar)>,
}

impl Chain {
    /// The chain built from what `chain(o)` returned, with its inputs and
    /// outputs and their types. Its locks are called `name.<lock>` in errors.
    pub(super) fn build(library: &Rc<LockLibrary>, name: &str, spec: Dynamic) -> Result<(Chain, InfoVarTypeSet, InfoVarTypeSet), String> {
        let spec = spec.try_cast::<Map>()
            .ok_or("chain(o) must return a map, #{ locks, connect, fixed, inputs, outputs }")?;
        if let Some(unknown) = spec.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(format!("chain(o) returned '{}'; it returns {}", unknown, KEYS.join(", ")));
        }

        let mut locks: HashMap<String, Rc<dyn Lock>> = HashMap::new();
        for (lock_name, entry) in map(&spec, "locks", true)? {
            if lock_name.is_empty() || lock_name.contains('.') {
                return Err(format!("locks: '{}' can't be a lock's name; names must be non-empty, without '.'", lock_name));
            }
            let entry = entry.try_cast::<Map>()
                .ok_or_else(|| format!("locks: '{}' should be a map, #{{ type: \"shift_cipher\", options: #{{ ... }} }}", lock_name))?;
            if let Some(unknown) = entry.keys().find(|key| !["type", "options"].contains(&key.as_str())) {
                return Err(format!("locks: '{}' has '{}'; a lock takes type and options", lock_name, unknown));
            }
            let type_name = entry.get("type").and_then(|t| t.clone().into_string().ok())
                .ok_or_else(|| format!("locks: '{}' needs its type, a lock type's name", lock_name))?;
            let options = match entry.get("options") {
                None => Map::new(),
                Some(options) => options.clone().try_cast::<Map>()
                    .ok_or_else(|| format!("locks: the options of '{}' should be a map", lock_name))?,
            };
            let lock = library.build(&format!("{}.{}", name, lock_name), &type_name, options)?;
            locks.insert(lock_name.to_string(), Rc::new(lock));
        }

        let mut connections = Vec::new();
        if let Some(connect) = spec.get("connect") {
            let pairs = connect.clone().into_array().map_err(|_| "connect should be an array of [output, input] pairs")?;
            for pair in pairs {
                let pair: Option<Vec<String>> = pair.into_array().ok()
                    .map(|items| items.into_iter().filter_map(|item| item.into_string().ok()).collect());
                match pair.as_deref() {
                    Some([from, to]) => connections.push((from.clone(), to.clone())),
                    _ => return Err("connect should be an array of [output, input] pairs, e.g. [\"a.cipher_text\", \"b.key\"]".into()),
                }
            }
        }

        let mut fixed = Vec::new();
        for (port, value) in map(&spec, "fixed", false)? {
            let var_type = port_type(&locks, &port, |lock| lock.inputs())
                .ok_or_else(|| format!("fixed: '{}' isn't an input of the chain's locks", port))?;
            let value = from_script(value, &var_type).map_err(|e| format!("fixed: '{}': {}", port, e))?;
            fixed.push((port.to_string(), value));
        }

        let lock = compose(name, locks, connections, &fixed)?;
        let fixed_value = |port: &str| fixed.iter().find(|(name, _)| name == port).map(|(_, value)| value.clone());

        let open_inputs: InfoVarTypeSet = lock.inputs().into_iter().filter(|(port, _)| fixed_value(port).is_none()).collect();
        let inputs = exposed(&spec, "inputs", &open_inputs, |_| false)?;
        let open_outputs = lock.outputs();
        // An output passing a fixed input through needn't be shown.
        let passes_fixed = |port: &str| fixed_value(port).is_some_and(|value| open_outputs.get(port).is_some_and(|t| compatible(&value.var_type, t)));
        let outputs = exposed(&spec, "outputs", &open_outputs, passes_fixed)?;
        for (shown, port) in &inputs {
            if let Some(output) = outputs.get(shown) && output != port {
                return Err(format!("'{}' is both an input ({}) and an output ({}); a name can be both only for a value passed through", shown, port, output));
            }
        }
        let hidden = open_outputs.keys()
            .filter(|port| !outputs.values().any(|shown| shown == *port))
            .filter_map(|port| fixed_value(port).map(|value| (port.clone(), value)))
            .collect();

        let input_types = inputs.iter().map(|(shown, port)| (shown.clone(), open_inputs[port].clone())).collect();
        let output_types = outputs.iter().map(|(shown, port)| (shown.clone(), open_outputs[port].clone())).collect();
        Ok((Chain { lock, inputs, outputs, hidden }, input_types, output_types))
    }

    /// Lock `inputs` (by the chain's names), returning its outputs and
    /// every value inside, by "lock.port".
    pub(super) fn lock(&self, inputs: &InfoVarSet) -> Result<(InfoVarSet, InfoVarSet), Box<dyn Error>> {
        let inside = rename(inputs, &self.inputs, "input")?;
        let (values, trace) = self.lock.trace_lock_info(&inside)?;
        Ok((renamed_back(&values, &self.outputs, "output")?, trace))
    }

    /// Unlock `outputs` (by the chain's names), returning its inputs and
    /// every value inside.
    pub(super) fn unlock(&self, outputs: &InfoVarSet) -> Result<(InfoVarSet, InfoVarSet), Box<dyn Error>> {
        let mut inside = rename(outputs, &self.outputs, "output")?;
        inside.extend(self.hidden.iter().cloned());
        let (values, trace) = self.lock.trace_unlock_info(&inside)?;
        Ok((renamed_back(&values, &self.inputs, "input")?, trace))
    }
}

/// `spec[key]` as a map, empty if it's missing and not `required`.
fn map(spec: &Map, key: &str, required: bool) -> Result<Map, String> {
    match spec.get(key) {
        None if required => Err(format!("chain(o) needs {}", key)),
        None => Ok(Map::new()),
        Some(value) => value.clone().try_cast::<Map>().ok_or_else(|| format!("{} should be a map", key)),
    }
}

/// The type of `port`, "lock.name", among each lock's `ports`.
fn port_type(locks: &HashMap<String, Rc<dyn Lock>>, port: &str, ports: impl Fn(&Rc<dyn Lock>) -> InfoVarTypeSet) -> Option<crate::lockbuilder::InfoVarType> {
    let (lock, name) = port.split_once('.')?;
    ports(locks.get(lock)?).get(name).cloned()
}

/// `spec[key]`: the chain's names for the `open` ports, each shown once.
/// Ports `may_hide` allows needn't be shown.
fn exposed(spec: &Map, key: &str, open: &InfoVarTypeSet, may_hide: impl Fn(&str) -> bool) -> Result<BTreeMap<String, String>, String> {
    let open_names = || {
        let mut names: Vec<&str> = open.keys().map(String::as_str).collect();
        names.sort();
        if names.is_empty() { "none".to_string() } else { names.join(", ") }
    };
    let mut shown: BTreeMap<String, String> = BTreeMap::new();
    for (name, port) in map(spec, key, true)? {
        if name.is_empty() || name.contains('.') {
            return Err(format!("{}: '{}' can't be a name; names must be non-empty, without '.'", key, name));
        }
        let port = port.into_string().map_err(|_| format!("{}: '{}' should name a port, e.g. \"vig.cipher_text\"", key, name))?;
        if !open.contains_key(&port) {
            return Err(format!("{}: '{}' is {}, which isn't an open {} of the chain; open: {}", key, name, port, &key[..key.len() - 1], open_names()));
        }
        if let Some((other, _)) = shown.iter().find(|(_, p)| **p == port) {
            return Err(format!("{}: {} is shown twice, as '{}' and '{}'", key, port, other, name));
        }
        shown.insert(name.to_string(), port);
    }
    let mut missing: Vec<&String> = open.keys()
        .filter(|port| !shown.values().any(|p| p == *port) && !may_hide(port))
        .collect();
    missing.sort();
    if let Some(port) = missing.first() {
        return Err(format!("{}: the chain's open {} {} needs a name here", key, &key[..key.len() - 1], port));
    }
    Ok(shown)
}

/// `values` by the chain's names, as the ports inside.
fn rename(values: &InfoVarSet, names: &BTreeMap<String, String>, what: &str) -> Result<InfoVarSet, String> {
    names.iter()
        .map(|(shown, port)| {
            let value = values.get(shown).ok_or_else(|| format!("no value for {} '{}'", what, shown))?;
            Ok((port.clone(), value.clone()))
        })
        .collect()
}

/// The ports' `values` by the chain's names.
fn renamed_back(values: &InfoVarSet, names: &BTreeMap<String, String>, what: &str) -> Result<InfoVarSet, String> {
    names.iter()
        .map(|(shown, port)| {
            let value = values.get(port).ok_or_else(|| format!("the chain gave no {} {}", what, port))?;
            Ok((shown.clone(), value.clone()))
        })
        .collect()
}
