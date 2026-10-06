//! The lock types still written in Rust, built by name. Lock files in
//! locks/ (see `journal::rhaientries::LockLibrary`) take precedence.

use super::Lock;
use super::locks::doublering::{DoubleRingLock, KeyedDoubleRingLock};
use super::locks::huffman::HuffmanEncodingLock;
use super::locks::util::mergealpha::MergeAlphaInputs;
use super::locks::util::splitsymbols::SplitSymbolsLock;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::rc::Rc;

/// Every lock type name `build_lock` accepts.
pub const LOCK_TYPES: &[&str] = &[
    "huffman_encoding",
    "doublering",
    "keyed_doublering",
    "merge_alpha",
    "split_symbols",
];

/// Build a lock of the named type from its JSON options.
pub fn build_lock(type_name: &str, options: &serde_json::Value) -> Result<Rc<dyn Lock>, Box<dyn Error>> {
    match type_name {
        "huffman_encoding" => make(type_name, options, HuffmanEncodingLock::new),
        "doublering" => make(type_name, options, DoubleRingLock::new),
        "keyed_doublering" => make(type_name, options, KeyedDoubleRingLock::new),
        "merge_alpha" => make(type_name, options, MergeAlphaInputs::new),
        "split_symbols" => make(type_name, options, SplitSymbolsLock::new),
        _ => Err(format!("unknown lock type '{}'; available: {}", type_name, LOCK_TYPES.join(", ")).into()),
    }
}

fn make<O, L>(
    type_name: &str,
    options: &serde_json::Value,
    new: impl FnOnce(O) -> Result<L, Box<dyn Error>>,
) -> Result<Rc<dyn Lock>, Box<dyn Error>>
where
    O: DeserializeOwned,
    L: Lock + 'static,
{
    let options: O = serde_json::from_value(options.clone())
        .map_err(|e| format!("invalid options for lock type '{}': {}", type_name, e))?;
    let lock = new(options).map_err(|e| format!("cannot create '{}' lock: {}", type_name, e))?;
    Ok(Rc::new(lock))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_type_is_buildable_by_name() {
        // Empty options fail to parse, but must not be reported as an
        // unknown type: that would mean LOCK_TYPES and the match disagree.
        for type_name in LOCK_TYPES {
            let err = build_lock(type_name, &serde_json::json!({})).err().map(|e| e.to_string());
            assert!(
                err.as_deref().is_none_or(|e| !e.contains("unknown lock type")),
                "{} is listed but not handled: {:?}", type_name, err,
            );
        }
    }

    #[test]
    fn builds_a_lock_from_options() {
        let lock = build_lock("keyed_doublering", &serde_json::json!({ "base": 26 })).expect("should build");
        assert!(lock.inputs().contains_key("plain_text"), "{:?}", lock.inputs());
    }

    #[test]
    fn reports_unknown_type_and_bad_options() {
        let unknown = build_lock("rot13", &serde_json::json!({})).err().unwrap().to_string();
        assert!(unknown.contains("unknown lock type 'rot13'") && unknown.contains("keyed_doublering"), "{}", unknown);
        let bad = build_lock("keyed_doublering", &serde_json::json!({ "base": "lots" })).err().unwrap().to_string();
        assert!(bad.contains("invalid options for lock type 'keyed_doublering'"), "{}", bad);
    }
}
