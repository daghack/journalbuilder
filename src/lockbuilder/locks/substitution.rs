use crate::lockbuilder::*;
use std::error::Error;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct SubstitutionCipherOptions {
    pub base_n: u32,
}

pub struct SubstitutionCipherLock {
    pub base_n: u32,
}

impl SubstitutionCipherLock {
    pub fn new(options: SubstitutionCipherOptions) -> Result<SubstitutionCipherLock, Box<dyn Error>> {
        Ok(SubstitutionCipherLock {
            base_n: options.base_n,
        })
    }
}

impl Lock for SubstitutionCipherLock {
    fn description(&self) -> String {
        format!("substitution_cipher_base_{}", self.base_n)
    }
    fn inputs(&self) -> InfoVarTypeSet {
        let mut inputs = InfoVarTypeSet::new();
        inputs.insert("plain_text".to_string(), InfoVarType::BaseNSymbol(self.base_n));
        inputs.insert("ordering".to_string(), InfoVarType::BaseNSymbol(self.base_n));
        inputs
    }
    fn outputs(&self) -> InfoVarTypeSet {
        let mut outputs = InfoVarTypeSet::new();
        outputs.insert("cipher_text".to_string(), InfoVarType::BaseNSymbol(self.base_n));
        outputs.insert("ordering".to_string(), InfoVarType::BaseNSymbol(self.base_n));
        outputs
    }
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let plain_text = inputs.get("plain_text").ok_or("plain_text not found")?;
        let ordering = inputs.get("ordering").ok_or("key not found")?;
        
        // ensure that the input is a BaseNSymbol
        if let InfoVarType::BaseNSymbol(base_n) = plain_text.var_type {
            if base_n != self.base_n {
                return Err("base_n does not match for plain_text".into());
            }
        } else {
            return Err("plain_text is not a BaseNSymbol".into());
        }
        
        if let InfoVarType::BaseNSymbol(base_n) = ordering.var_type {
            if base_n != self.base_n {
                return Err("base_n does not match for key".into());
            }
        } else {
            return Err("ordering is not a BaseNSymbol".into());
        }
        
        let plain_data: &Vec<u32> = plain_text.borrow_data()?;
        let key_data: &Vec<u32> = ordering.borrow_data()?;
        
        // Create substitution map from key
        let mut substitution_map = HashMap::new();
        for (i, &k) in key_data.iter().enumerate() {
            substitution_map.insert(i as u32, k);
        }
        
        // Encrypt the plain text using the substitution map
        let cipher_data: Vec<u32> = plain_data.iter()
            .map(|&symbol| *substitution_map
                .get(&symbol)
                .unwrap_or(&symbol)) // If symbol not in map, keep it unchanged
            .collect();
        let mut output_set = InfoVarSet::new();
        let cipher_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.base_n), cipher_data);
        output_set.insert("cipher_text".to_string(), cipher_var);
        output_set.insert("ordering".to_string(), ordering.clone());
        Ok(output_set)
    }
    fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let cipher_text = outputs.get("cipher_text").ok_or("cipher_text not found")?;
        let ordering = outputs.get("ordering").ok_or("ordering not found")?;
        
        // ensure that the input is a BaseNSymbol
        if let InfoVarType::BaseNSymbol(base_n) = cipher_text.var_type {
            if base_n != self.base_n {
                return Err("base_n does not match for cipher_text".into());
            }
        } else {
            return Err("cipher_text is not a BaseNSymbol".into());
        }
        
        if let InfoVarType::BaseNSymbol(base_n) = ordering.var_type {
            if base_n != self.base_n {
                return Err("base_n does not match for ordering".into());
            }
        } else {
            return Err("ordering is not a BaseNSymbol".into());
        }
        
        let cipher_data: &Vec<u32> = cipher_text.borrow_data()?;
        let key_data: &Vec<u32> = ordering.borrow_data()?;
        
        // Create reverse substitution map from key
        let mut reverse_map = HashMap::new();
        for (i, &k) in key_data.iter().enumerate() {
            reverse_map.insert(k, i as u32);
        }
        
        // Decrypt the cipher text using the reverse substitution map
        let plain_data: Vec<u32> = cipher_data.iter()
            .map(|&symbol| *reverse_map
                .get(&symbol)
                .unwrap_or(&symbol)) // If symbol not in map, keep it unchanged
            .collect();
        
        let mut output_set = InfoVarSet::new();
        let plain_var = InfoVar::new_with(InfoVarType::BaseNSymbol(self.base_n), plain_data);
        output_set.insert("plain_text".to_string(), plain_var);
        output_set.insert("ordering".to_string(), ordering.clone());
        Ok(output_set)
    }
}

