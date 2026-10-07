//! Reading options maps, `#{ key: value }`, as many rhai functions take.

use rhai::{Dynamic, EvalAltResult, Map};

use super::RhaiResult;

pub(super) fn check_keys(caller: &str, options: &Map, known: &[&str]) -> RhaiResult<()> {
    match options.keys().find(|k| !known.contains(&k.as_str())) {
        Some(key) => Err(format!("{}: unknown option '{}'; options: {}", caller, key, known.join(", ")).into()),
        None => Ok(()),
    }
}

pub(super) fn as_number(caller: &str, what: &str, v: &Dynamic) -> RhaiResult<f32> {
    v.as_float().map(|f| f as f32)
        .or_else(|_| v.as_int().map(|i| i as f32))
        .map_err(|type_name| format!("{}: {} should be a number, got {}", caller, what, type_name).into())
}

/// A number option, accepting ints as well as floats.
pub(super) fn number(caller: &str, options: &Map, key: &str) -> RhaiResult<Option<f32>> {
    options.get(key).map(|v| as_number(caller, &format!("'{}'", key), v)).transpose()
}

pub(super) fn positive(caller: &str, options: &Map, key: &str) -> RhaiResult<f32> {
    match number(caller, options, key)? {
        Some(n) if n > 0.0 => Ok(n),
        Some(n) => Err(format!("{}: '{}' must be positive, got {}", caller, key, n).into()),
        None => Err(format!("{}: missing '{}'", caller, key).into()),
    }
}

/// A required `[low, high]` option of positive numbers.
pub(super) fn range(caller: &str, options: &Map, key: &str) -> RhaiResult<(f32, f32)> {
    let bad = || -> Box<EvalAltResult> { format!("{}: '{}' should be [low, high], two positive numbers with low <= high", caller, key).into() };
    let pair = options.get(key)
        .ok_or_else(|| -> Box<EvalAltResult> { format!("{}: missing '{}'", caller, key).into() })?
        .clone().into_array().map_err(|_| bad())?;
    if pair.len() != 2 {
        return Err(bad());
    }
    let (low, high) = (as_number(caller, &format!("'{}'", key), &pair[0])?, as_number(caller, &format!("'{}'", key), &pair[1])?);
    if !(low > 0.0 && low <= high) {
        return Err(bad());
    }
    Ok((low, high))
}
