use crate::lockbuilder::*;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Serialize, Deserialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::collections::hash_map::Entry;
use std::error::Error;

#[derive(Debug, Clone, Eq, PartialEq)]
struct Node {
    value: Option<u32>,
    count: u32,
    index: Option<NodeIndex>,
}

impl Node {
    fn new(index: Option<NodeIndex>, value: Option<u32>, count: u32) -> Result<Self, Box<dyn Error>> {
        Ok(Node {
            value,
            count,
            index,
        })
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

fn make_encoding(frequency_counts: &HashMap<u32, u32>, original_base: u32, encoding_base: u32) -> (DiGraph<Option<u32>, u32>, NodeIndex) {
    let mut heap = BinaryHeap::new();
    let mut huff_tree : DiGraph<Option<u32>, u32> = DiGraph::new();
    let mut index_map = HashMap::new();

    for symbol in 0 .. original_base {
        let default = 0;
        let symbol_count = frequency_counts.get(&symbol).or(Some(&default)).unwrap();
        let symbol_index = huff_tree.add_node(Some(symbol));
        heap.push(Node::new(Some(symbol_index), Some(symbol), *symbol_count).unwrap());
        index_map.insert(symbol, symbol_index);
    }

    if encoding_base > 2 {
        while heap.len() % (encoding_base - 1) as usize != 1 {
            heap.push(Node::new(None, None, 0).unwrap());
        }
    }

    let mut root_index = None;
    while root_index.is_none() {
        let parent_index = huff_tree.add_node(None);
        let mut parent = Node::new(Some(parent_index), None, 0).unwrap();
        for nth_symbol in 0..encoding_base {
            if let Some(node) = heap.pop() {
                parent.count += node.count;
                if let Some(child_index) = node.index {
                    huff_tree.add_edge(parent_index, child_index, nth_symbol);
                }
            }
        }
        heap.push(parent);
        if heap.len() == 1 {
            root_index = Some(parent_index);
        }
    }

    (huff_tree, root_index.unwrap())
}


#[derive(Serialize, Deserialize)]
pub struct HuffmanEncodingOptions{
    incoming_base: u32,
    outgoing_base: u32,
    frequency_counts : HashMap<u32, u32>,
}

pub struct HuffmanEncodingLock {
    incoming_base: u32,
    outgoing_base: u32,
    frequency_counts: HashMap<u32, u32>,
}

impl HuffmanEncodingLock {
    pub fn new(options: HuffmanEncodingOptions) -> Result<HuffmanEncodingLock, Box<dyn Error>> {
        Ok(HuffmanEncodingLock{
            incoming_base: options.incoming_base,
            outgoing_base: options.outgoing_base,
            frequency_counts: options.frequency_counts,
        })
    }
    fn build_encoding_maps(&self, huffman_tree: &DiGraph<Option<u32>, u32>, root_index: NodeIndex) -> (HashMap<u32, Vec<u32>>, HashMap<Vec<u32>, u32>) {
        let mut source_base_to_encoded = HashMap::new();
        let mut encoded_to_source_base = HashMap::new();
        let mut current_path = Vec::new();
        petgraph::visit::depth_first_search(huffman_tree, Some(root_index), |event| {
            match event {
                petgraph::visit::DfsEvent::Discover(node_index, _) => {
                    let node = huffman_tree[node_index];
                    if let Some(value) = node {
                        source_base_to_encoded.insert(value, current_path.clone());
                        encoded_to_source_base.insert(current_path.clone(), value);
                    }
                }
                petgraph::visit::DfsEvent::TreeEdge(source_index, target_index) => {
                    // Add the edge weight to the current path
                    // (edge weights here represent the symbol for generating the path to the leaf)
                    let edge_index = huffman_tree.find_edge(source_index, target_index).unwrap();
                    let edge_weight = huffman_tree[edge_index];
                    current_path.push(edge_weight);
                }
                petgraph::visit::DfsEvent::Finish(_, _) => {
                    if current_path.len() > 0 {
                        current_path.pop();
                    }
                }
                _ => {}
            }
        });
        (source_base_to_encoded, encoded_to_source_base)
    }
}

impl Lock for HuffmanEncodingLock {
    fn description(&self) -> String {
        format!("huffman_enc_base_{}_to_base_{}", self.incoming_base, self.outgoing_base)
    }
    fn inputs(&self) -> InfoVarTypeSet {
        let mut inputs = InfoVarTypeSet::new();
        inputs.insert("plain_text".to_string(), InfoVarType::BaseNSymbol(self.incoming_base));
        inputs
    }
    fn outputs(&self) -> InfoVarTypeSet {
        let mut outputs = InfoVarTypeSet::new();
        outputs.insert("cipher_text".to_string(), InfoVarType::BaseNSymbol(self.outgoing_base));
        outputs.insert("encoding_tree".to_string(), InfoVarType::Graph(InfoVarGraphType::HuffmanEncoding));
        outputs
    }
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let incoming = inputs.get("incoming").ok_or("incoming variable not found")?;
        let incoming_msg = incoming.data.as_ref().ok_or("incoming data not found")?
            .as_ref().as_any().downcast_ref::<Vec<u32>>()
            .ok_or("incoming data is not a Vec<u32>")?;

        let (huffman_tree, root_index) = make_encoding(&self.frequency_counts, self.incoming_base, self.outgoing_base);
        let (encoding_map, _) = self.build_encoding_maps(&huffman_tree, root_index);
        let mut outgoing : Vec<u32> = Vec::new();
        for symbol in incoming_msg {
            let encoded_symbol = encoding_map.get(symbol).ok_or("Missing symbol in encoding_map")?;
            outgoing.extend(encoded_symbol);
        }

        let outgoing_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.outgoing_base), outgoing);
        let huffman_tree_var = InfoVar::new_with(InfoVarType::Graph(InfoVarGraphType::HuffmanEncoding), (huffman_tree, root_index));
        let mut output = InfoVarSet::new();
        output.insert("outgoing".to_string(), outgoing_var);
        output.insert("encoding_tree".to_string(), huffman_tree_var);
        Ok(output)
    }
    fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let huffman_tree = outputs.get("encoding_tree").ok_or("huffman_tree variable not found")?;
        // ensure that the output is a DiGraph<Option<u32>, u32>
        let (huff_tree, root_index) = huffman_tree.data.as_ref().ok_or("huffman_tree data not found")?.
            as_ref().as_any().downcast_ref::<(DiGraph<Option<u32>, u32>, NodeIndex)>()
            .ok_or("huffman_tree data is not a (DiGraph, NodeIndex)")?;
        let outgoing_msg = outputs.get("outgoing").ok_or("outgoing variable not found")?.
            data.as_ref().ok_or("outgoing data not found")?
            .as_ref().as_any().downcast_ref::<Vec<u32>>()
            .ok_or("outgoing data is not a Vec<u32>")?;

        let (_, mut reversed_encoding) = self.build_encoding_maps(huff_tree, *root_index);
        let mut current_symbols = Vec::new();
        let mut incoming : Vec<u32> = Vec::new();
        for symbol in outgoing_msg {
            current_symbols.push(*symbol);
            if let Entry::Occupied(entry) = reversed_encoding.entry(current_symbols.clone()) {
                incoming.push(*entry.get());
                current_symbols.clear();
            }
        }
        let incoming_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.incoming_base), incoming);
        let mut input = InfoVarSet::new();
        input.insert("incoming".to_string(), incoming_var);
        Ok(input)
    }
}
