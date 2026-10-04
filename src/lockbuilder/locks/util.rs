pub mod mergealpha {
    use crate::lockbuilder::*;
    use std::error::Error;
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    pub struct MergeAlpha26Options {
        pub base_n: u32,
        pub input_count: usize,
    }
    pub struct MergeAlphaInputs {
        pub base_n: u32,
        pub input_count: usize,
    }
    impl MergeAlphaInputs {
        pub fn new(options: MergeAlpha26Options) -> Result<MergeAlphaInputs, Box<dyn Error>> {
            Ok(MergeAlphaInputs {
                input_count: options.input_count,
                base_n: options.base_n,
            })
        }
        pub fn extract_var_data(&self, var: &InfoVar, var_key: &str) -> Result<Vec<u32>, Box<dyn Error>> {
            // in the error case, specify the var_key in the error message
            if !var.is(InfoVarType::BaseNSymbol(self.base_n)) {
                return Err(format!("var {} is not a BaseNSymbol({})", var_key, self.base_n).into());
            }
            let data : &Vec<u32> = var.borrow_data()?;
            Ok(data.clone())
        }
    }

    pub struct MergeAlphaOutputMeta {
        pub chunks: Vec<usize>,
    }

    impl Lock for MergeAlphaInputs {
        fn description(&self) -> String {
            format!("merge_inputs_{}", self.input_count)
        }
        fn inputs(&self) -> InfoVarTypeSet {
            let mut inputs = InfoVarTypeSet::new();
            for i in 0..self.input_count {
                inputs.insert(format!("input_{}", i), InfoVarType::BaseNSymbol(self.base_n));
            }
            inputs
        }
        fn outputs(&self) -> InfoVarTypeSet {
            let mut outputs = InfoVarTypeSet::new();
            outputs.insert("merged_output".to_string(), InfoVarType::BaseNSymbol(self.base_n));
            outputs.insert("location_info".to_string(), InfoVarType::LockProcessInfo);
            outputs
        }
        fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            let mut output_data = Vec::new();
            let mut chunks = Vec::new();
            for i in 0..self.input_count {
                let input_key = format!("input_{}", i);
                let input_var = inputs.get(&input_key).ok_or(format!("{} not found", input_key))?;
                let input_data = self.extract_var_data(input_var, input_key.as_str())?;
                chunks.push(input_data.len());
                output_data.extend(input_data);
            }
            let output_var = InfoVar::new_with(
                InfoVarType::BaseNSymbol(self.base_n),
                output_data
            );
            let meta_var = InfoVar::new_with(
                InfoVarType::LockProcessInfo,
                MergeAlphaOutputMeta { chunks }
            );
            let mut output_set = InfoVarSet::new();
            output_set.insert("merged_output".to_string(), output_var);
            output_set.insert("location_info".to_string(), meta_var);
            Ok(output_set)
        }
        fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            let output_var = outputs.get("merged_output").ok_or("merged_output not found")?;
            let output_data = self.extract_var_data(output_var, "merged_output")?;
            let meta_var = outputs.get("location_info").ok_or("location_info not found")?;
            let metadata : &MergeAlphaOutputMeta = meta_var.borrow_data()?;
            let mut output_set = InfoVarSet::new();
            let mut index = 0;
            for (i, &chunk_size) in metadata.chunks.iter().enumerate() {
                let chunk_data = output_data[index..index + chunk_size].to_vec();
                let chunk_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.base_n), chunk_data);
                output_set.insert(format!("input_{}", i), chunk_var);
                index += chunk_size;
            };
            if index != output_data.len() {
                return Err("output data length does not match metadata".into());
            }
            Ok(output_set)
        }
    }
}

pub mod splitsymbols {
    use crate::lockbuilder::*;
    use std::error::Error;
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    pub struct SplitSymbolsOptions {
        pub base_n: u32,
        pub output_count: usize,
    }

    pub struct SplitSymbolsLock {
        pub base_n: u32,
        pub output_count: usize,
    }
    impl SplitSymbolsLock {
        pub fn new(options: SplitSymbolsOptions) -> Result<SplitSymbolsLock, Box<dyn Error>> {
            Ok(SplitSymbolsLock {
                output_count: options.output_count,
                base_n: options.base_n,
            })
        }
    }
    impl Lock for SplitSymbolsLock {
        fn description(&self) -> String {
            format!("split_symbols_{}", self.output_count)
        }
        fn inputs(&self) -> InfoVarTypeSet {
            let mut inputs = InfoVarTypeSet::new();
            inputs.insert("plain_text".to_string(), InfoVarType::BaseNSymbol(self.base_n));
            inputs
        }
        fn outputs(&self) -> InfoVarTypeSet {
            let mut outputs = InfoVarTypeSet::new();
            for i in 0..self.output_count {
                outputs.insert(format!("output_{}", i), InfoVarType::BaseNSymbol(self.base_n));
            }
            outputs
        }
        fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            let input_var = inputs.get("plain_text").ok_or("plain_text not found")?;
            if !input_var.is(InfoVarType::BaseNSymbol(self.base_n)) {
                return Err(format!("plain_text is not a BaseNSymbol({})", self.base_n).into());
            }
            let input_data : &Vec<u32> = input_var.borrow_data()?;
            let chunk_size = (input_data.len() + self.output_count - 1) / self.output_count; // ceiling division
            let mut output_set = InfoVarSet::new();
            for i in 0..self.output_count {
                let start_index = i * chunk_size;
                let end_index = std::cmp::min(start_index + chunk_size, input_data.len());
                let chunk_data = if start_index < input_data.len() {
                    input_data[start_index..end_index].to_vec()
                } else {
                    Vec::new()
                };
                let chunk_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.base_n), chunk_data);
                output_set.insert(format!("output_{}", i), chunk_var);
            }
            Ok(output_set)
        }
        fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            let mut merged_data : Vec<u32> = Vec::new();
            for i in 0..self.output_count {
                let output_key = format!("output_{}", i);
                let output_var = outputs.get(&output_key).ok_or(format!("{} not found", output_key))?;
                if !output_var.is(InfoVarType::BaseNSymbol(self.base_n)) {
                    return Err(format!("{} is not a BaseNSymbol({})", output_key, self.base_n).into());
                }
                let output_data : &Vec<u32> = output_var.borrow_data()?;
                merged_data.extend(output_data);
            }
            let merged_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.base_n), merged_data);
            let mut output_set = InfoVarSet::new();
            output_set.insert("plain_text".to_string(), merged_var);
            Ok(output_set)
        }
    }
}
