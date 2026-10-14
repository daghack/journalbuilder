//! Lock chains: named locks wired output to input, run forward to lock
//! values and backward to unlock them. Lock types are rhai lock files (see
//! `journal::rhaientries::LockLibrary`); values are rhai values.

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::DfsPostOrder;
use rhai::Dynamic;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::rc::Rc;

pub fn symbols_to_string(symbols: &[u32], base: u32) -> String {
    if base == 10 {
        return symbols.iter().map(|&s| s.to_string()).collect::<Vec<String>>().join("");
    }
    if base == 27 {
        return symbols.iter().map(|&s| {
            
            if s == 0 {
                ' '
            } else {
                (s - 1 + b'A' as u32) as u8 as char
            }
        }).collect();
    }
    symbols.iter().map(|&s| (s + b'A' as u32) as u8 as char).collect()
}

/// The type of a value in a lock chain.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum InfoVarType {
    /// Symbols in a base: an array of integers from 0 to base - 1.
    BaseNSymbol(u32),
    /// Matches every type.
    MatchAll,
    /// Any other type, by the name lock files give it, e.g. "Maze".
    Named(String),
}

impl std::fmt::Display for InfoVarType {
    /// The type's name as lock files write it: "Base27Symbol", "Any", "Maze".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfoVarType::BaseNSymbol(base) => write!(f, "Base{}Symbol", base),
            InfoVarType::MatchAll => write!(f, "Any"),
            InfoVarType::Named(name) => write!(f, "{}", name),
        }
    }
}

/// Whether a value of type `a` can go where `b` is expected: the same type,
/// or either is Any.
pub fn compatible(a: &InfoVarType, b: &InfoVarType) -> bool {
    a == b || *a == InfoVarType::MatchAll || *b == InfoVarType::MatchAll
}

/// A value in a lock chain: its type, and the rhai value itself.
#[derive(Clone, Debug)]
pub struct InfoVar {
    pub var_type: InfoVarType,
    pub data: Dynamic,
}

impl InfoVar {
    pub fn new(var_type: InfoVarType, data: Dynamic) -> InfoVar {
        InfoVar { var_type, data }
    }
}

/// TEXT as symbols in `base`, rejecting characters outside the base rather
/// than dropping them. Base 10 takes digits 0-9; base 27 takes space (0)
/// and letters A-Z (1-26); other bases from 2 to 26 take the first `base`
/// letters, A = 0. Letters are case-insensitive.
pub fn base_n_symbols(base: u32, text: &str) -> Result<Vec<u32>, String> {
    if !(2..=27).contains(&base) {
        return Err(format!("base must be between 2 and 27, got {}", base));
    }
    text.chars().map(|c| {
        let upper = c.to_ascii_uppercase();
        let symbol = match base {
            10 => c.to_digit(10),
            27 if c == ' ' => Some(0),
            27 if upper.is_ascii_uppercase() => Some(upper as u32 - 'A' as u32 + 1),
            27 => None,
            _ if upper.is_ascii_uppercase() && (upper as u32 - 'A' as u32) < base => Some(upper as u32 - 'A' as u32),
            _ => None,
        };
        symbol.ok_or_else(|| format!("'{}' isn't a base-{} symbol ({})", c, base, symbol_help(base)))
    }).collect()
}

fn symbol_help(base: u32) -> String {
    match base {
        10 => "digits 0-9".to_string(),
        27 => "letters and spaces".to_string(),
        _ => format!("letters A-{}", (b'A' + base as u8 - 1) as char),
    }
}

pub type InfoVarSet = HashMap<String, InfoVar>;
pub type InfoVarTypeSet = HashMap<String, InfoVarType>;

pub trait Lock {
    fn description(&self) -> String;
    fn inputs(&self) -> InfoVarTypeSet;
    fn outputs(&self) -> InfoVarTypeSet;
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>>;
    fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>>;
}

type LockOutputMapping = (String, String);

#[derive(Clone)]
pub enum LockNodeType {
    Lock(Rc<dyn Lock>, bool),
    Remap(LockOutputMapping),
    Input((String, InfoVarType)),
    Output((String, InfoVarType)),
}

impl std::fmt::Debug for LockNodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockNodeType::Lock(lock, reversed) => f.debug_tuple("Lock").field(&lock.description()).field(reversed).finish(),
            LockNodeType::Remap((output, input)) => f.debug_tuple("Remap").field(output).field(input).finish(),
            LockNodeType::Input(input) => f.debug_tuple("Input").field(input).finish(),
            LockNodeType::Output(output) => f.debug_tuple("Output").field(output).finish(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LockNode {
    pub name: String,
    pub node_type: LockNodeType,
}

#[derive(Debug, Clone)]
pub struct LockExecutionGraph {
    graph: DiGraph<LockNode, ()>,
    starts: Vec<NodeIndex>,
}

impl LockExecutionGraph {
    pub fn new() -> LockExecutionGraph {
        LockExecutionGraph {
            graph: DiGraph::new(),
            starts: Vec::new(),
        }
    }
    pub fn add_node(&mut self, node: LockNode) -> NodeIndex {
        self.graph.add_node(node)
    }
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, _: ()) {
        self.graph.add_edge(from, to, ());
    }
    fn set_starts(&mut self, starts: Vec<NodeIndex>) {
        self.starts = starts;
    }
    pub fn execute(&self, inputs: &InfoVarSet) -> Result<(InfoVarSet, InfoVarSet), Box<dyn Error>> {
        let mut current_variable_values = HashMap::new();
        let mut hashmap_trace = HashMap::new();
        let mut computed_variable_values : HashMap<String, InfoVar> = HashMap::new();
        // One walk over every start, so each node runs once and a value wired
        // to several inputs is there for all of them.
        let mut walk = DfsPostOrder::empty(&self.graph);
        for start_node in &self.starts {
            walk.move_to(*start_node);
            while let Some(node_id) = walk.next(&self.graph) {
                let node = self.graph.node_weight(node_id).ok_or("Node not found while executiong lock.")?;
                match &node.node_type {
                    LockNodeType::Input((input_name, input_type)) => {
                        if !current_variable_values.contains_key(input_name) {
                            if let Some(input) = inputs.get(input_name) {
                                if input.var_type != *input_type && *input_type != InfoVarType::MatchAll {
                                    return Err(format!("Input {} has the wrong type", input_name).into());
                                }
                                current_variable_values.insert(input_name.clone(), input.clone());
                                hashmap_trace.insert(input_name.clone(), input.clone());
                            } else {
                                return Err(format!("Input {} not found in inputs", input_name).into());
                            }
                        }
                    },
                    LockNodeType::Output((output_name, output_type)) => {
                        if !current_variable_values.contains_key(output_name) {
                            if let Some(output) = computed_variable_values.get(output_name) {
                                if output.var_type != *output_type && *output_type != InfoVarType::MatchAll {
                                    return Err(format!("Output {} has the wrong type", output_name).into());
                                }
                                current_variable_values.insert(output_name.clone(), output.clone());
                                hashmap_trace.insert(output_name.clone(), output.clone());
                            } else {
                                return Err(format!("Output {} not found in computed variables", output_name).into());
                            }
                        }
                    },
                    LockNodeType::Remap((output, input)) => {
                        let output = current_variable_values.get(output).cloned().ok_or(format!("Output {} not found in current variable values", output))?;
                        current_variable_values.insert(input.clone(), output.clone());
                        hashmap_trace.insert(input.clone(), output.clone());
                    },
                    LockNodeType::Lock(lock, reversed) => {
                        let lock_name = node.name.clone();
                        let mut lock_inputs = InfoVarSet::new();
                        let input_set = if *reversed { lock.outputs() } else { lock.inputs() };
                        for (input_name, _) in input_set {
                            let global_input_name = format!("{}.{}", lock_name, input_name);
                            let input = current_variable_values.get(&global_input_name).ok_or(format!("Input {} not found in current variable values", global_input_name))?;
                            lock_inputs.insert(input_name.clone(), input.clone());
                        }
                        let lfunc = if *reversed { Lock::unlock_info } else { Lock::lock_info };
                        let lock_outputs = lfunc(lock.as_ref(), &lock_inputs)?;
                        for (output_name, output) in lock_outputs {
                            let global_output_name = format!("{}.{}", lock_name, output_name);
                            computed_variable_values.insert(global_output_name.clone(), output.clone());
                            hashmap_trace.insert(global_output_name.clone(), output.clone());
                        }
                    },
                }
            }
        }
        Ok((current_variable_values, hashmap_trace))
    }
}

impl Default for LockExecutionGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct CompositeLock {
    // a graph, where each lock is a node and each edge represents a mapping from output to input.
    // the overall inputs are all the inputs which do not have outputs mapped to them, and likewise
    // outputs are all the outputs which do not have inputs mapped to them.
    name: String,
    pub locks: HashMap<String, Rc<dyn Lock>>,
    pub connections: Vec<(String, String)>,
    cached_inputs: InfoVarTypeSet,
    cached_outputs: InfoVarTypeSet,
    exec_graph: Option<LockExecutionGraph>,
    pub static_inputs: HashMap<String, InfoVar>,
}

impl CompositeLock {
    pub fn add_static_input(&mut self, input_name: String, input_value: InfoVar) {
        self.static_inputs.insert(input_name, input_value);
    }
    /// A chain of named locks. Each connection feeds one lock's output to
    /// another's input, both written "lock.name"; the chain's inputs and
    /// outputs are those left unconnected.
    pub fn new(name: String, locks: HashMap<String, Rc<dyn Lock>>, connections: Vec<(String, String)>) -> Result<CompositeLock, Box<dyn Error>> {
        let mut input_set = HashMap::new();
        let mut output_set = HashMap::new();
        for (lock_name, lock) in &locks {
            for (input_name, input_type) in lock.inputs() {
                input_set.insert(format!("{}.{}", lock_name, input_name), input_type);
            }
            for (output_name, output_type) in lock.outputs() {
                output_set.insert(format!("{}.{}", lock_name, output_name), output_type);
            }
        }
        let mut remaining_inputs = input_set.clone();
        let mut remaining_outputs = output_set.clone();
        let mut wired = HashSet::new();
        for (output, input) in &connections {
            let output_type = output_set.get(output).ok_or_else(|| format!("Output {} does not exist", output))?;
            let input_type = input_set.get(input).ok_or_else(|| format!("Input {} does not exist", input))?;
            if !compatible(output_type, input_type) {
                return Err(format!("can't connect {} ({}) to {} ({})", output, output_type, input, input_type).into());
            }
            if !wired.insert(input) {
                return Err(format!("input '{}' is already connected", input).into());
            }
            remaining_inputs.remove(input);
            remaining_outputs.remove(output);
        }
        check_for_loops(&locks, &connections)?;
        let mut cl = CompositeLock {
            name,
            locks,
            connections,
            cached_inputs: remaining_inputs,
            cached_outputs: remaining_outputs,
            exec_graph: None,
            static_inputs: HashMap::new(),
        };
        cl.build_execution_graph();
        Ok(cl)
    }
    fn build_execution_graph(&mut self) {
        let (exec_graph, _, _) = self.generate_lock_graph();
        self.exec_graph = Some(exec_graph);
    }
    pub fn generate_lock_graph(&self) -> (LockExecutionGraph, HashMap<String, NodeIndex>, HashMap<String, NodeIndex>) {
        let mut graph = LockExecutionGraph::new();
        let mut input_indices = HashMap::new();
        let mut output_indices = HashMap::new();
        let mut locks: Vec<(&String, &Rc<dyn Lock>)> = self.locks.iter().collect();
        locks.sort_by(|a, b| a.0.cmp(b.0));
        for (lock_name, lock) in locks {
            let mut lock_inputs: Vec<(String, InfoVarType)> = lock.inputs().into_iter().collect();
            lock_inputs.sort_by(|a, b| a.0.cmp(&b.0));
            let mut lock_outputs: Vec<(String, InfoVarType)> = lock.outputs().into_iter().collect();
            lock_outputs.sort_by(|a, b| a.0.cmp(&b.0));
            // add the lock node
            let lock_node = LockNode{ name: lock_name.clone(), node_type: LockNodeType::Lock(lock.clone(), false) };
            let lock_node_index = graph.add_node(lock_node);
            // add the inputs as produce nodes
            for (input_name, input_type) in lock_inputs {
                let input_name = format!("{}.{}", lock_name, input_name);
                let input_node = LockNode{name: input_name.clone(), node_type: LockNodeType::Input((input_name.to_string(), input_type))};
                let input_node_index = graph.add_node(input_node);
                input_indices.insert(input_name, input_node_index);
                graph.add_edge(lock_node_index, input_node_index, ());
            }
            // add the outputs as produce nodes
            for (output_name, output_type) in lock_outputs {
                let output_name = format!("{}.{}", lock_name, output_name);
                let output_node = LockNode{name: output_name.clone(), node_type: LockNodeType::Output((output_name.to_string(), output_type))};
                let output_node_index = graph.add_node(output_node);
                output_indices.insert(output_name, output_node_index);
                graph.add_edge(output_node_index, lock_node_index, ());
            }
        }
        for (output, input) in &self.connections {
            // get the output and input nodes
            // if they are not present, skip
            let output_index = output_indices.get(output);
            let input_index = input_indices.get(input);
            if output_index.is_none() || input_index.is_none() {
                continue;
            }
            let output_index = output_index.unwrap();
            let input_index = input_index.unwrap();
            // add a remap node
            //let remap_node = LockNode::Remap((output.clone(), input.clone()));
            let remap_node = LockNode{name: format!("{}->{}", output, input), node_type: LockNodeType::Remap((output.clone(), input.clone()))};
            let remap_node_index = graph.add_node(remap_node);
            graph.add_edge(remap_node_index, *output_index, ());
            graph.add_edge(*input_index, remap_node_index, ());
        }
        graph.set_starts(by_name(&output_indices, output_indices.keys()));
        (graph, input_indices, output_indices)
    }
    pub fn generate_unlock_graph(&self) -> LockExecutionGraph {
        let (mut exec_graph, input_indices, _output_indices) = self.generate_lock_graph();
        exec_graph.graph.reverse();
        let inputs = self.inputs();
        let starts = by_name(&input_indices, inputs.keys());
        // we also need to loop over all the nodes and if they are a remap node, we need to reverse the remap
        // so that it is Remap((input, output)) instead of Remap((output, input))
        // we also need to change the input and output nodes to be the opposite
        for node_index in exec_graph.graph.node_indices() {
            let node = exec_graph.graph.node_weight_mut(node_index).unwrap();
            let new_node_type = match &node.node_type {
                LockNodeType::Remap((output, input)) => LockNodeType::Remap((input.clone(), output.clone())),
                LockNodeType::Input((input_name, input)) => LockNodeType::Output((input_name.clone(), input.clone())),
                LockNodeType::Output((output_name, output)) => LockNodeType::Input((output_name.clone(), output.clone())),
                LockNodeType::Lock(lock, reversed) => LockNodeType::Lock(lock.clone(), !reversed),
            };
            node.node_type = new_node_type;
        }
        exec_graph.set_starts(starts);
        exec_graph
    }
    pub fn trace_lock_info(&self, inputs: &InfoVarSet) -> Result<(InfoVarSet, InfoVarSet), Box<dyn Error>> {
        let mut inputs = inputs.clone();
        for (static_input_name, static_input_var) in &self.static_inputs {
            if inputs.contains_key(static_input_name) {
                continue;
            }
            // insert the static input
            inputs.insert(static_input_name.clone(), static_input_var.clone());
        }
        // check that all inputs are present
        let mut required_inputs = self.inputs();
        for (input_name, input_type) in &self.cached_inputs {
            if !inputs.contains_key(input_name) {
                return Err(format!("Input {} is missing", input_name).into());
            }
            if inputs.get(input_name).ok_or("Input not found")?.var_type != *input_type && *input_type != InfoVarType::MatchAll {
                return Err(format!("Input {} has the wrong type", input_name).into());
            }
            required_inputs.remove(input_name);
        }
        if !required_inputs.is_empty() {
            return Err("Not all required inputs are present".into());
        }
        let (resulting_vars, var_trace) = self.exec_graph.as_ref().ok_or("Execution Graph not built")?.execute(&inputs)?;
        let mut output_vars = InfoVarSet::new();
        for (output_name, output_type) in &self.cached_outputs {
            if !resulting_vars.contains_key(output_name) {
                return Err(format!("Output {} is missing", output_name).into());
            }
            if !compatible(&resulting_vars.get(output_name).ok_or("Output not found")?.var_type, output_type) {
                return Err(format!("Output {} has the wrong type", output_name).into());
            }
            let output_var = resulting_vars.get(output_name).ok_or("Output not found")?.clone();
            output_vars.insert(output_name.clone(), output_var);
        }
        Ok((output_vars, var_trace))
    }
    pub fn trace_unlock_info(&self, outputs: &InfoVarSet) -> Result<(InfoVarSet, InfoVarSet), Box<dyn Error>> {
        let mut required_outputs = self.outputs();
        for (output_name, output_type) in &self.cached_outputs {
            if !outputs.contains_key(output_name) {
                return Err(format!("Output {} is missing", output_name).into());
            }
            if outputs.get(output_name).ok_or("Output not found")?.var_type != *output_type && *output_type != InfoVarType::MatchAll {
                return Err(format!("Output {} has the wrong type", output_name).into());
            }
            required_outputs.remove(output_name);
        }
        if !required_outputs.is_empty() {
            return Err("Not all required outputs are present".into());
        }
        if self.exec_graph.is_none() {
            return Err("Execution Graph not built".into());
        }

        let (resulting_vars, var_trace) = self.generate_unlock_graph().execute(outputs)?;
        let mut input_vars = InfoVarSet::new();
        for (input_name, input_type) in &self.cached_inputs {
            if !resulting_vars.contains_key(input_name) {
                return Err(format!("Input {} is missing", input_name).into());
            }
            if !compatible(&resulting_vars.get(input_name).ok_or("Input not found")?.var_type, input_type) {
                return Err(format!("Input {} has the wrong type", input_name).into());
            }
            let input_var = resulting_vars.get(input_name).ok_or("Input not found")?.clone();
            input_vars.insert(input_name.clone(), input_var);
        }
        Ok((input_vars, var_trace))
    }
}

/// The nodes for `names`, sorted by name, so runs walk the graph in the same
/// order every time.
fn by_name<'a>(indices: &HashMap<String, NodeIndex>, names: impl Iterator<Item = &'a String>) -> Vec<NodeIndex> {
    let mut names: Vec<&String> = names.collect();
    names.sort();
    names.into_iter().filter_map(|name| indices.get(name).copied()).collect()
}

/// An error if the connections feed a lock's outputs back into itself,
/// directly or through other locks.
fn check_for_loops(locks: &HashMap<String, Rc<dyn Lock>>, connections: &[(String, String)]) -> Result<(), Box<dyn Error>> {
    let mut names: Vec<&String> = locks.keys().collect();
    names.sort();
    let mut graph = DiGraph::<&str, ()>::new();
    let nodes: HashMap<&str, NodeIndex> = names.iter().map(|name| (name.as_str(), graph.add_node(name.as_str()))).collect();
    let lock_of = |port: &str| port.split_once('.').map(|(lock, _)| lock).and_then(|lock| nodes.get(lock).copied());
    for (output, input) in connections {
        if let (Some(from), Some(to)) = (lock_of(output), lock_of(input)) {
            graph.add_edge(from, to, ());
        }
    }
    toposort(&graph, None).map(|_| ()).map_err(|cycle| {
        format!("the connections make a loop through lock '{}'", graph[cycle.node_id()]).into()
    })
}

impl std::fmt::Debug for CompositeLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeLock")
            .field("LockCount", &self.locks.len())
            .field("Connections", &self.connections)
            .field("Input", &self.cached_inputs)
            .field("Output", &self.cached_outputs)
            .finish()
    }
}

impl Lock for CompositeLock {
    fn description(&self) -> String {
        self.name.clone()
    }
    fn inputs(&self) -> InfoVarTypeSet {
        self.cached_inputs.clone()
    }
    fn outputs(&self) -> InfoVarTypeSet {
        self.cached_outputs.clone()
    }
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        self.trace_lock_info(inputs).map(|(resulting_vars, _)| resulting_vars)
    }
    fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        self.trace_unlock_info(outputs).map(|(resulting_vars, _)| resulting_vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_base_n_text() {
        assert_eq!(base_n_symbols(27, "Hi z"), Ok(vec![8, 9, 0, 26]));
        assert_eq!(base_n_symbols(10, "1084"), Ok(vec![1, 0, 8, 4]));
        assert_eq!(base_n_symbols(26, "baz"), Ok(vec![1, 0, 25]));
        assert_eq!(base_n_symbols(3, "CAB"), Ok(vec![2, 0, 1]));
    }

    #[test]
    fn rejects_characters_outside_the_base() {
        let cases = [
            (27, "HELLO, WORLD", "',' isn't a base-27 symbol"),
            (10, "12a", "'a' isn't a base-10 symbol"),
            (26, "hello world", "' ' isn't a base-26 symbol"),
            (3, "ABD", "'D' isn't a base-3 symbol (letters A-C)"),
            (40, "A", "between 2 and 27"),
        ];
        for (base, text, expected) in cases {
            let err = base_n_symbols(base, text).err().unwrap_or_else(|| panic!("{} should fail", text));
            assert!(err.contains(expected), "{}: {}", text, err);
        }
    }
}
