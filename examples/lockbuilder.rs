use serde::{Serialize, Deserialize};
use std::rc::Rc;
use std::collections::HashMap;

use scribe::lockbuilder::*;
use scribe::journal::canvas::{Canvas, CanvasStyleOptions};
use scribe::lockbuilder::locks::doublering::{DoubleRingLock, DoubleRingOptions, KeyedDoubleRingLock, KeyedDoubleRingOptions};
use scribe::lockbuilder::locks::util::mergealpha::{MergeAlphaInputs, MergeAlpha26Options};
use scribe::lockbuilder::locks::util::splitsymbols::{SplitSymbolsLock, SplitSymbolsOptions};
use scribe::lockbuilder::locks::util::basechange::{BaseChangeLock, BaseChangeOptions};
use scribe::lockbuilder::locks::shift::{ShiftCipherLock, ShiftCipherOptions, VignCipherLock, VignCipherOptions};
use scribe::lockbuilder::locks::huffman::{HuffmanEncodingLock, HuffmanEncodingOptions};
use scribe::lockbuilder::locks::substitution::{SubstitutionCipherLock, SubstitutionCipherOptions};

macro_rules! lock_from_file {
    ($filename:expr, $($lock_type:literal, $lock_struct:ident, $lock_options:ident),+) => {
        {
            let lock_config = std::fs::read_to_string($filename).expect("Failed to read lock config file");
            let lock_config: LockConfig = serde_json::from_str(&lock_config).expect("Failed to parse lock config JSON");
            CompositeLock::from_lockconfig(lock_config, |lock_type_config| {
                match lock_type_config.type_name.as_str() {
                    $( $lock_type => {
                        let options: $lock_options = serde_json::from_value(lock_type_config.options.clone())
                            .expect(&format!("Failed to parse options for lock type: {}", $lock_type));
                        let lock_instance = $lock_struct::new(options).expect(&format!("Failed to create lock instance for type: {}", $lock_type));
                        Ok(Rc::new(lock_instance) as Rc<dyn Lock>)
                    }, )+
                    _ => Err(format!("Unknown lock type: {}", lock_type_config.type_name).into()),
                }
            })
        }
    };
}

fn string_to_base_n_symbols(s: &str, base: u32) -> InfoVar {
    if base == 10 {
        // special case for base 10 to include digits
        let symbols: Vec<u32> = s.chars()
            .filter_map(|c| {
                if c >= '0' && c <= '9' {
                    Some((c as u32) - (b'0' as u32))
                } else {
                    None
                }
            })
            .collect();
        return InfoVar::new_with(InfoVarType::BaseNSymbol(base), symbols);
    }
    if base == 27 {
        // special case for base 27 to include space character
        let symbols: Vec<u32> = s.to_ascii_uppercase().chars()
            .filter_map(|c| {
                if c == ' ' {
                    Some(0)
                } else if c >= 'A' && c <= 'Z' {
                    Some((c as u32) - (b'A' as u32) + 1)
                } else {
                    None
                }
            })
            .collect();
        return InfoVar::new_with(InfoVarType::BaseNSymbol(base), symbols);
    }
    //filter out any symbols that are not in A..(A+base) range
    let symbols: Vec<u32> = s.to_ascii_uppercase().chars()
        .filter_map(|c| {
            if c < 'A' {
                return None;
            }
            let val = (c as u32) - (b'A' as u32);
            if val < base { Some(val) } else { None }
        })
        .collect();
    InfoVar::new_with(InfoVarType::BaseNSymbol(base), symbols)
}

fn string_infovar_set(lock_output: &InfoVarSet) -> HashMap<String, String> {
    let mut output_set = HashMap::new();
    for (key, var) in lock_output.iter() {
        println!("Processing key: {}, var_type: {:?}", key, var.var_type);
        match var.var_type {
            InfoVarType::BaseNSymbol(base_n) => {
                if let Some(data) = var.data.as_ref() {
                    if let Some(symbols) = data.as_ref().as_any().downcast_ref::<Vec<u32>>() {
                        let output_string = symbols_to_string(symbols, base_n);
                        output_set.insert(key.clone(), output_string);
                    }
                }
            },
            _ => continue,
        }
    }
    output_set
}

#[derive(Serialize, Deserialize)]
pub struct LockOutput {
    pub lock_name: String,
    pub lock_inputs: HashMap<String, String>,
    pub lock_outputs: HashMap<String, String>,
    pub trace_vars: HashMap<String, String>,
    pub unlock_outputs: HashMap<String, String>,
}

fn main() {
    //let input_vars = base_n_input_set!(27,
    //    "split3.plain_text" => "run, jump, duck, and cover",
    //    "vign.key" => "bafflement",
    //    "vign.plain_text" => "defend the east wall of the castle",
    //);
    let input_vars = InfoVarSet::new();

    let test_lock = lock_from_file!("config/lockbuilder.json",
        "shift_cipher", ShiftCipherLock, ShiftCipherOptions,
        "vigenere_cipher", VignCipherLock, VignCipherOptions,
        "huffman_encoding", HuffmanEncodingLock, HuffmanEncodingOptions,
        "doublering", DoubleRingLock, DoubleRingOptions,
        "keyed_doublering", KeyedDoubleRingLock, KeyedDoubleRingOptions,
        "merge_alpha", MergeAlphaInputs, MergeAlpha26Options,
        "split_symbols", SplitSymbolsLock, SplitSymbolsOptions,
        "substitution_cipher", SubstitutionCipherLock, SubstitutionCipherOptions,
        "base_change", BaseChangeLock, BaseChangeOptions
    ).expect("Failed to create CompositeLock from config");

    let input_value_strings = string_infovar_set(&input_vars);
    let (lock_output, trace_vars) = test_lock.trace_lock_info(&input_vars).expect("Failed to lock info");
    let output_value_strings = string_infovar_set(&lock_output);
    let trace_value_strings = string_infovar_set(&trace_vars);
    let unlock_output = test_lock.unlock_info(&lock_output).expect("Failed to unlock info");
    let unlock_outputs = string_infovar_set(&unlock_output);

    let lock_result = LockOutput {
        lock_name: test_lock.description(),
        lock_inputs: input_value_strings,
        lock_outputs: output_value_strings,
        trace_vars: trace_value_strings,
        unlock_outputs: unlock_outputs,
    };
    let lock_result_json = serde_json::to_string_pretty(&lock_result).expect("Failed to serialize lock result to JSON");
    println!("{}", lock_result_json);
}
