use crate::lockbuilder::*;
use std::error::Error;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct VignCipherOptions {
    pub key_base: u32,
    pub plain_base: u32,
    pub cipher_base: u32,
}

pub struct VignCipherLock {
    pub key_base: u32,
    pub plain_base: u32,
    pub cipher_base: u32,
}

impl VignCipherLock {
    pub fn new(options: VignCipherOptions) -> Result<VignCipherLock, Box<dyn Error>> {
        Ok(VignCipherLock {
            key_base: options.key_base,
            plain_base: options.plain_base,
            cipher_base: options.cipher_base,
        })
    }
}

impl Lock for VignCipherLock {
    fn description(&self) -> String {
        format!("vign_cipher_key_base_{}_plain_base_{}_cipher_base_{}", self.key_base, self.plain_base, self.cipher_base)
    }
    fn inputs(&self) -> InfoVarTypeSet {
        let mut inputs = InfoVarTypeSet::new();
        inputs.insert("plain_text".to_string(), InfoVarType::BaseNSymbol(self.plain_base));
        inputs.insert("key".to_string(), InfoVarType::BaseNSymbol(self.key_base));
        inputs
    }
    fn outputs(&self) -> InfoVarTypeSet {
        let mut outputs = InfoVarTypeSet::new();
        outputs.insert("cipher_text".to_string(), InfoVarType::BaseNSymbol(self.cipher_base));
        outputs.insert("key".to_string(), InfoVarType::BaseNSymbol(self.key_base));
        outputs
    }
    fn lock_info(&self, inputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let plain_text = inputs.get("plain_text").ok_or("plain_text not found")?;
        let key = inputs.get("key").ok_or("key not found")?;
        
        // ensure that the input is a BaseNSymbol
        if let InfoVarType::BaseNSymbol(base_n) = plain_text.var_type {
            if base_n != self.plain_base {
                return Err("plain_base does not match".into());
            }
        } else {
            return Err("plain_text is not a BaseNSymbol".into());
        }
        
        if let InfoVarType::BaseNSymbol(base_n) = key.var_type {
            if base_n != self.key_base {
                return Err("key_base does not match".into());
            }
        } else {
            let msg = format!("key found type: {:?}, expected BaseNSymbol", key.var_type);
            return Err(msg.into());
        }

        let plain_text : &Vec<u32> = plain_text.borrow_data()?;
        let key_data : &Vec<u32> = key.borrow_data()?;
        let ciphered_text: Vec<u32> = plain_text.iter()
            .zip(key_data.iter().cycle())
            .map(|(&x, &k)| (x + k) % self.cipher_base)
            .collect();
                
        let ciphered_text = InfoVar::new_with(InfoVarType::BaseNSymbol(self.cipher_base), ciphered_text);
        let mut output = InfoVarSet::new();
        output.insert("cipher_text".to_string(), ciphered_text);
        output.insert("key".to_string(), key.clone());
        Ok(output)
    }
    fn unlock_info(&self, outputs: &InfoVarSet) -> Result<InfoVarSet, Box<dyn Error>> {
        let cipher_text = outputs.get("cipher_text").ok_or("cipher_text not found")?;
        let key = outputs.get("key").ok_or("key not found")?;
        
        // ensure that the output is a BaseNSymbol
        if let InfoVarType::BaseNSymbol(base_n) = cipher_text.var_type {
            if base_n != self.cipher_base {
                return Err("cipher_base does not match".into());
            }
        } else {
            return Err("cipher_text is not a BaseNSymbol".into());
        }
        
        if let InfoVarType::BaseNSymbol(base_n) = key.var_type {
            if base_n != self.key_base {
                return Err("key_base does not match".into());
            }
        } else {
            return Err("key is not a BaseNSymbol".into());
        }
        let cipher_text : &Vec<u32> = cipher_text.borrow_data()?;
        let key_data : &Vec<u32> = key.borrow_data()?;
        // decrypt the cipher_text using the key

        let plain_text: Vec<u32> = cipher_text.iter()
            .zip(key_data.iter().cycle())
            .map(|(&x, &k)| (x + self.cipher_base - k) % self.cipher_base)
            .collect();
                
        let plain_text = InfoVar::new_with(InfoVarType::BaseNSymbol(self.plain_base), plain_text);
        let mut output = InfoVarSet::new();
        output.insert("plain_text".to_string(), plain_text);
        Ok(output)
    }
}
