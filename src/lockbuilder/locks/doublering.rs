    use crate::lockbuilder::*;
    use std::error::Error;
    use serde::{Serialize, Deserialize};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize)]
    pub struct KeyedDoubleRingOptions {
        pub base: u32,
    }

    pub struct KeyedDoubleRingLock {
        pub base_n: u32,
    }
    impl KeyedDoubleRingLock {
        pub fn new(options: KeyedDoubleRingOptions) -> Result<Self, Box<dyn Error>> {
            // base_n must be even and at least 2
            if options.base < 2 || !options.base.is_multiple_of(2) {
                return Err("base_n must be an even number greater than or equal to 2".into());
            }
            Ok(Self { base_n: options.base })
        }
        pub fn encode_msg(&self, input: &[u32], ordering: &[u32]) -> Vec<u32> {
            let mut mapping = HashMap::new();
            for i in 0..(self.base_n / 2) {
                let a = ordering[i as usize];
                let b = ordering[(i + (self.base_n / 2)) as usize];
                mapping.insert(a, b);
                mapping.insert(b, a);
            }
            input.iter().map(|&x| {
                *mapping.get(&x).unwrap_or(&x)
            }).collect()
        }
    }
    impl Lock for KeyedDoubleRingLock {
        fn description(&self) -> String {
            format!("keyed_double_ring_lock_base_{}", self.base_n)
        }
        fn inputs(&self) -> InfoVarTypeSet {
            let mut inputs = InfoVarTypeSet::new();
            inputs.insert("plain_text".to_string(), InfoVarType::BaseNSymbol(self.base_n));
            inputs.insert("base_ordering".to_string(), InfoVarType::BaseNSymbol(self.base_n));
            inputs
        }
        fn outputs(&self) -> InfoVarTypeSet {
            let mut outputs = InfoVarTypeSet::new();
            outputs.insert("cipher_text".to_string(), InfoVarType::BaseNSymbol(self.base_n));
            outputs.insert("base_ordering".to_string(), InfoVarType::BaseNSymbol(self.base_n));
            outputs
        }
        fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            let plain_text = inputs.get("plain_text").ok_or("plain_text not found")?;
            let base_ordering = inputs.get("base_ordering").ok_or("base_ordering not found")?;

            if !plain_text.is(InfoVarType::BaseNSymbol(self.base_n)) {
                return Err(format!("plain_text is not a BaseNSymbol({})", self.base_n).into());
            }
            if !base_ordering.is(InfoVarType::BaseNSymbol(self.base_n)) {
                return Err(format!("base_ordering is not a BaseNSymbol({})", self.base_n).into());
            }

            let plain_data : &Vec<u32> = plain_text.borrow_data()?;
            let base_order_data : &Vec<u32> = base_ordering.borrow_data()?;
            let cipher_data = self.encode_msg(plain_data, base_order_data);
            let cipher_var = InfoVar::new_with(
                InfoVarType::BaseNSymbol(self.base_n), cipher_data
            );
            let mut output_set = InfoVarSet::new();
            output_set.insert("cipher_text".to_string(), cipher_var);
            output_set.insert("base_ordering".to_string(), base_ordering.clone());
            Ok(output_set)
        }
        fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            let cipher_text = outputs.get("cipher_text").ok_or("cipher_text not found")?;
            let base_ordering = outputs.get("base_ordering").ok_or("base_ordering not found")?;

            if !cipher_text.is(InfoVarType::BaseNSymbol(self.base_n)) {
                return Err(format!("cipher_text is not a BaseNSymbol({})", self.base_n).into());
            }
            if !base_ordering.is(InfoVarType::BaseNSymbol(self.base_n)) {
                return Err(format!("base_ordering is not a BaseNSymbol({})", self.base_n).into());
            }

            let cipher_data : &Vec<u32> = cipher_text.borrow_data()?;
            let base_order_data : &Vec<u32> = base_ordering.borrow_data()?;
            let plain_data = self.encode_msg(cipher_data, base_order_data);
            let plain_var = InfoVar::new_with(
                InfoVarType::BaseNSymbol(self.base_n), plain_data
            );
            let mut output_set = InfoVarSet::new();
            output_set.insert("plain_text".to_string(), plain_var);
            output_set.insert("base_ordering".to_string(), base_ordering.clone());
            Ok(output_set)
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct DoubleRingOptions {
        pub canonical_order: String,
    }
    pub struct DoubleRingLock {
        pub canonical_order: [u32; 26],
    }
    impl DoubleRingLock {
        pub fn new(options: DoubleRingOptions) -> Result<Self, Box<dyn Error>> {
            if options.canonical_order.len() != 26 {
                return Err("canonical_order must be 26 characters".into());
            }
            for i in options.canonical_order.chars() {
                if !i.is_ascii_uppercase() && !i.is_ascii_lowercase() {
                    return Err("canonical_order must be A-Z or a-z".into());
                }
            }
            // make all characters uppercase
            let order = options.canonical_order.to_uppercase();
            let mut canonical_order = [0; 26];
            for (i, c) in order.chars().enumerate() {
                canonical_order[i] = (c as u8 - b'A') as u32;
            }
            Ok(Self{ canonical_order })
        }
        pub fn encode_msg(&self, input: Vec<u32>) -> Vec<u32> {
            let mut mapping = HashMap::new();
            for i in 0..13 {
                let a = self.canonical_order[i];
                let b = self.canonical_order[i + 13];
                mapping.insert(a, b);
                mapping.insert(b, a);
            }
            input.iter().map(|&x| {
                *mapping.get(&x).unwrap_or(&x)
            }).collect()
        }
        pub fn encode_info(&self, input: &InfoVarSet, input_key: &str, output_key: &str) -> Result<InfoVarSet, Box<dyn Error>> {
            let input_var = input.get(input_key).ok_or(format!("{} not found", input_key))?;
            let input_data = Self::extract_var_data(input_key, input_var)?;
            let output_data = self.encode_msg(input_data);
            let output_var = InfoVar::new_with(
                InfoVarType::BaseNSymbol(26), output_data
            );
            let mut output_set = InfoVarSet::new();
            output_set.insert(output_key.to_string(), output_var);
            Ok(output_set)
        }
        pub fn extract_var_data(var_key: &str, var: &InfoVar) -> Result<Vec<u32>, Box<dyn Error>> {
            // in the error case, specify the var_key in the error message
            if !var.is(InfoVarType::BaseNSymbol(26)) {
                return Err(format!("var {} is not a BaseNSymbol(26)", var_key).into());
            }
            let data : &Vec<u32> = var.borrow_data()?;
            Ok(data.clone())
        }
    }
    impl Lock for DoubleRingLock {
        fn description(&self) -> String {
            let mut order_str = String::new();
            for &i in &self.canonical_order {
                order_str.push((i as u8 + b'A') as char);
            }
            format!("double_ring_lock_{}", order_str)
        }
        fn inputs(&self) -> InfoVarTypeSet {
            let mut inputs = InfoVarTypeSet::new();
            inputs.insert("plain_text".to_string(), InfoVarType::BaseNSymbol(26));
            inputs
        }
        fn outputs(&self) -> InfoVarTypeSet {
            let mut outputs = InfoVarTypeSet::new();
            outputs.insert("cipher_text".to_string(), InfoVarType::BaseNSymbol(26));
            outputs
        }
        fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            self.encode_info(inputs, "plain_text", "cipher_text")
        }
        fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
            self.encode_info(outputs, "cipher_text", "plain_text")
        }
    }
