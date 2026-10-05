pub mod locks;
pub mod registry;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{DfsPostOrder, Walker};
use serde::{Serialize, Deserialize};

use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

#[macro_export]
macro_rules! base_n_input_set {
    ($base:expr, $($key:expr => $value:expr),* $(,)?) => {
        {
            let mut input_set = InfoVarSet::new();
            $(
                let var = string_to_base_n_symbols($value, $base);
                input_set.insert($key.to_string(), var);
            )*
            input_set
        }
    };
}

pub fn symbols_to_string(symbols: &[u32], base: u32) -> String {
    if base == 10 {
        return symbols.iter().map(|&s| s.to_string()).collect::<Vec<String>>().join("");
    }
    if base == 27 {
        return symbols.iter().map(|&s| {
            let m = if s == 0 {
                ' '
            } else {
                (s - 1 + b'A' as u32) as u8 as char
            };
            m
        }).collect();
    }
    symbols.iter().map(|&s| (s + b'A' as u32) as u8 as char).collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum InfoVarGraphType {
    HuffmanEncoding,
    MazeLock,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum InfoVarType {
    String,
    BaseNSymbol(u32),
    Graph(InfoVarGraphType),
    LockProcessInfo,
    MatchAll,
}

pub trait InfoVarData: Any {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> InfoVarData for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct InfoVar {
    pub var_type: InfoVarType,
    pub data: Option<Rc<dyn InfoVarData>>,
}

impl std::fmt::Debug for InfoVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display the type and whether data is initialized
        // If data is initialized, we can also display the data type
        // If the data is a BaseNSymbol, we can display the data as a vector of u32
        if let InfoVarType::BaseNSymbol(base) = self.var_type {
            if let Ok(data) = self.borrow_data::<Vec<u32>>() {
                return f.debug_struct("InfoVar")
                    .field("Type", &self.var_type)
                    .field("Data", &symbols_to_string(data, base))
                    .finish();
            } 
        }
        f.debug_struct("InfoVar")
            .field("Type", &self.var_type)
            .field("Data Initialized", &self.data.is_some())
            .finish()
    }
}

impl From<String> for InfoVar {
    fn from(s: String) -> Self {
        let base_n_regex = regex::Regex::new(r"^b(\d+):(.*)").unwrap();
        if let Some(caps) = base_n_regex.captures(&s) {
            let base_n = caps.get(1).unwrap().as_str().parse::<u32>().unwrap();
            let data = caps.get(2).unwrap().as_str().to_string();
            //filter out character not in 'A'-('A'+base_n-1) after converting to uppercase
            //special case for commonly used bases
            match base_n {
                10 => {
                    let data: Vec<u32> = data.chars()
                        .filter(|c| c.is_ascii_digit())
                        .map(|c| (c as u8 - b'0') as u32)
                        .collect();
                    return InfoVar::new_with(InfoVarType::BaseNSymbol(base_n), data);
                },
                27 => {
                    let data: Vec<u32> = data.to_ascii_uppercase().chars()
                        .filter_map(|c| {
                            if c == ' ' {
                                Some(0)
                            } else if c >= 'A' && c <= 'Z' {
                                let v = (c as u8 - b'A' + 1) as u32;
                                Some(v)
                            } else {
                                None
                            }
                        })
                        .collect();
                    return InfoVar::new_with(InfoVarType::BaseNSymbol(base_n), data);
                },
                _ => {
                    let data: Vec<u32> = data.to_ascii_uppercase().chars()
                        .filter(|c| {
                            let c = c.to_ascii_uppercase();
                            (c >= 'A') && (c < (('A' as u8 + base_n as u8) as char))
                        })
                        .map(|c| (c as u8 - b'A') as u32)
                        .collect();
                    InfoVar::new_with(InfoVarType::BaseNSymbol(base_n), data)
                },
            }
        } else {
            InfoVar::new_with(InfoVarType::String, s)
        }
    }
}

impl InfoVar {
    pub fn new(var_type: InfoVarType) -> InfoVar {
        InfoVar {
            var_type,
            data: None,
        }
    }
    pub fn new_with<T: InfoVarData>(var_type: InfoVarType, data: T) -> InfoVar {
        let any_data: Rc<dyn InfoVarData> = Rc::new(data);
        InfoVar {
            var_type,
            data: Some(any_data),
        }
    }
    pub fn borrow_data<'a, T: InfoVarData + 'a>(&'a self) -> Result<&'a T, Box<dyn Error>> {
        if let Some(data) = &self.data {
            let data = data
                .as_ref().as_any().downcast_ref::<T>()
                .ok_or(format!("Data Type Mismatch: expected {}, found {:?}", std::any::type_name::<T>(), data.as_ref().as_any().type_id()))?;
            Ok(data)
        } else {
            Err("Data is not initialized".into())
        }
    }
    pub fn is(&self, var_type: InfoVarType) -> bool {
        self.var_type == var_type
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
    Input((String, InfoVar)),
    Output((String, InfoVar)),
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
        for start_node in &self.starts {
            let nodewalker = DfsPostOrder::new(&self.graph, *start_node);
            for node_id in nodewalker.iter(&self.graph) {
                let node = self.graph.node_weight(node_id).ok_or("Node not found while executiong lock.")?;
                match &node.node_type {
                    LockNodeType::Input((input_name, input_var)) => {
                        if !current_variable_values.contains_key(input_name) {
                            if let Some(input) = inputs.get(input_name) {
                                if input.var_type != input_var.var_type && input_var.var_type != InfoVarType::MatchAll {
                                    return Err(format!("Input {} has the wrong type", input_name).into());
                                }
                                current_variable_values.insert(input_name.clone(), input.clone());
                                hashmap_trace.insert(input_name.clone(), input.clone());
                            } else {
                                return Err(format!("Input {} not found in inputs", input_name).into());
                            }
                        }
                    },
                    LockNodeType::Output((output_name, output_var)) => {
                        if !current_variable_values.contains_key(output_name) {
                            if let Some(output) = computed_variable_values.get(output_name) {
                                if output.var_type != output_var.var_type && output_var.var_type != InfoVarType::MatchAll {
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
                        let output = current_variable_values.remove(output).ok_or(format!("Output {} not found in current variable values", output))?;
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
    pub fn from_lockconfig(lock_config: LockConfig, f: impl Fn(&LockTypeConfig) -> Result<Rc<dyn Lock>, Box<dyn Error>>) -> Result<CompositeLock, Box<dyn Error>> {
        let mut cl = lock_config.into_composite_lock(f)?;
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
        for (lock_name, lock) in &self.locks {
            // add the lock node
            let lock_node = LockNode{ name: lock_name.clone(), node_type: LockNodeType::Lock(lock.clone(), false) };
            let lock_node_index = graph.add_node(lock_node);
            // add the inputs as produce nodes
            for (input_name, input_type) in lock.inputs() {
                let input_name = format!("{}.{}", lock_name, input_name);
                let input_node = LockNode{name: input_name.clone(), node_type: LockNodeType::Input((input_name.to_string(), InfoVar::new(input_type)))};
                let input_node_index = graph.add_node(input_node);
                input_indices.insert(input_name, input_node_index);
                graph.add_edge(lock_node_index, input_node_index, ());
            }
            // add the outputs as produce nodes
            for (output_name, output_type) in lock.outputs() {
                let output_name = format!("{}.{}", lock_name, output_name);
                let output_node = LockNode{name: output_name.clone(), node_type: LockNodeType::Output((output_name.to_string(), InfoVar::new(output_type)))};
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
        let starts = output_indices.values().cloned().collect();
        graph.set_starts(starts);
        (graph, input_indices, output_indices)
    }
    pub fn generate_unlock_graph(&self) -> LockExecutionGraph {
        let (mut exec_graph, input_indices, _output_indices) = self.generate_lock_graph();
        exec_graph.graph.reverse();
        let mut starts = Vec::new();
        let inputs = self.inputs();
        for (input_name, _) in inputs {
            let input_index = input_indices.get(&input_name).unwrap();
            starts.push(*input_index);
        }
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
            if resulting_vars.get(output_name).ok_or("Output not found")?.var_type != *output_type {
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
            if resulting_vars.get(input_name).ok_or("Input not found")?.var_type != *input_type {
                return Err(format!("Input {} has the wrong type", input_name).into());
            }
            let input_var = resulting_vars.get(input_name).ok_or("Input not found")?.clone();
            input_vars.insert(input_name.clone(), input_var);
        }
        Ok((input_vars, var_trace))
    }
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LockTypeConfig {
    pub type_name: String,
    pub options: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct NamelessLockConfig {
    pub locks: HashMap<String, LockTypeConfig>,
    pub connections: Vec<(String, String)>,
    pub static_inputs: HashMap<String, String>,
}

impl NamelessLockConfig {
    pub fn add_name(self, name: String) -> LockConfig {
        LockConfig {
            name,
            locks: self.locks,
            connections: self.connections,
            static_inputs: self.static_inputs,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LockConfig {
    pub name: String,
    pub locks: HashMap<String, LockTypeConfig>,
    pub connections: Vec<(String, String)>,
    pub static_inputs: HashMap<String, String>,
}

impl LockConfig {
    pub fn into_composite_lock(self, f: impl Fn(&LockTypeConfig) -> Result<Rc<dyn Lock>, Box<dyn Error>>) -> Result<CompositeLock, Box<dyn Error>> {
        let mut locks = HashMap::new();
        let mut input_set = HashMap::new();
        let mut output_set = HashMap::new();
        for (lock_name, lock_type_config) in self.locks {
            let new_lock = f(&lock_type_config)?;

            let inputs = new_lock.inputs();
            let mut lock_inputs = HashMap::new();
            for (input_name, input_type) in inputs {
                let input_name = format!("{}.{}", lock_name, input_name);
                input_set.insert(input_name.clone(), input_type.clone());
                lock_inputs.insert(input_name, input_type);
            }

            let outputs = new_lock.outputs();
            let mut lock_outputs = HashMap::new();
            for (output_name, output_type) in outputs {
                let output_name = format!("{}.{}", lock_name, output_name);
                output_set.insert(output_name.clone(), output_type.clone());
                lock_outputs.insert(output_name, output_type);
            }
            locks.insert(lock_name.clone(), new_lock);
        }
        let mut remaining_inputs = input_set.clone();
        let mut remaining_outputs = output_set.clone();
        for (output, input) in &self.connections {
            // check for existence of output and input
            if !output_set.contains_key(output) {
                return Err(format!("Output {} does not exist", output).into());
            }
            if !input_set.contains_key(input) {
                return Err(format!("Input {} does not exist", input).into());
            }
            // add edge to graph
            let output_lock = output.split('.').next().unwrap().to_string();
            let input_lock = input.split('.').next().unwrap().to_string();
            if !locks.contains_key(&output_lock) {
                return Err(format!("Lock Output {} does not exist", output).into());
            }
            if !locks.contains_key(&input_lock) {
                return Err(format!("Lock Input {} does not exist", input).into());
            }
            remaining_inputs.remove(input);
            remaining_outputs.remove(output);
        }
        // add static inputs
        let mut static_inputs = HashMap::new();
        for (input_name, input_value) in self.static_inputs {
            let required_input_type = remaining_inputs.get(&input_name).ok_or(format!("Input {} not found in remaining inputs", input_name))?;
            let input_var: InfoVar = input_value.into();
            if input_var.var_type != *required_input_type && *required_input_type != InfoVarType::MatchAll {
                return Err(format!("Static input {} has the wrong type", input_name).into());
            }
            static_inputs.insert(input_name, input_var);
        }
        Ok(CompositeLock {
            name: self.name,
            locks,
            connections: self.connections,
            cached_inputs: remaining_inputs,
            cached_outputs: remaining_outputs,
            exec_graph: None,
            static_inputs,
        })
    }
}

pub fn string_infovar_set(lock_output: &InfoVarSet) -> HashMap<String, String> {
    let mut output_set = HashMap::new();
    for (key, var) in lock_output.iter() {
        if let InfoVarType::BaseNSymbol(base_n) = var.var_type {
            if let Some(data) = var.data.as_ref() {
                if let Some(symbols) = data.as_ref().as_any().downcast_ref::<Vec<u32>>() {
                    let output_string = symbols_to_string(symbols, base_n);
                    output_set.insert(key.clone(), output_string);
                }
            }
        } else {
            output_set.insert(key.clone(), format!("<non-BaseNSymbol var type: {:?}>", var.var_type));
        }
    }
    output_set
}
