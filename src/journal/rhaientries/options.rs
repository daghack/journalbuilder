//! Reading arguments: numbers, which scripts may write as whole numbers or
//! decimals, and options maps, `#{ key: value }`, as many functions take.

use rhai::{Dynamic, EvalAltResult, Map};

use super::RhaiResult;

pub(super) fn check_keys(caller: &str, options: &Map, known: &[&str]) -> RhaiResult<()> {
    match options.keys().find(|k| !known.contains(&k.as_str())) {
        Some(key) => Err(format!("{}: unknown option '{}'; options: {}", caller, key, known.join(", ")).into()),
        None => Ok(()),
    }
}

/// A number argument, `what` of `caller`: an int or a float, finite.
pub(super) fn float(caller: &str, what: &str, v: &Dynamic) -> RhaiResult<f64> {
    let n = v.as_float().or_else(|_| v.as_int().map(|i| i as f64))
        .map_err(|type_name| -> Box<EvalAltResult> { format!("{}: {} should be a number, got {}", caller, what, type_name).into() })?;
    if !n.is_finite() {
        return Err(format!("{}: {} must be finite, got {}", caller, what, n).into());
    }
    Ok(n)
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

/// A constructor's options: each key set on `value` exactly as
/// `value.KEY = ...` sets it, through the property's registered setter, so
/// constructors take every settable property with the same checks and
/// errors. `props` are the settable properties' names; others are errors.
pub(super) fn apply_props<T: Clone + 'static>(ctx: &rhai::NativeCallContext, caller: &str, value: T, options: Map, props: &[&str]) -> RhaiResult<T> {
    check_keys(caller, &options, props)?;
    let mut target = Dynamic::from(value);
    for (key, mut v) in options {
        let _ = ctx.call_native_fn_raw(format!("set${}", key), true, &mut [&mut target, &mut v])
            .map_err(|err| -> Box<EvalAltResult> { format!("{}: {}", caller, unwrap_runtime(&err)).into() })?;
    }
    Ok(target.cast::<T>())
}

/// A style: settings scripts set by name, as properties
/// (`style.passes = 2`), as a constructor's options, or as a map of some
/// of them over another (a role's, a span's), each checked the same way.
pub(super) trait Settings: Clone + 'static {
    /// What scripts call it, in messages.
    const NAME: &'static str;
    /// Its settings' names.
    const KEYS: &'static [&'static str];
    fn get(&self, key: &str) -> Dynamic;
    /// Set `key` (one of `KEYS`), checking it alone; `check` checks the
    /// settings together.
    fn set(&mut self, caller: &str, key: &str, value: &Dynamic) -> RhaiResult<()>;
    fn check(&self, _caller: &str) -> RhaiResult<()> {
        Ok(())
    }
    /// This with `map`'s settings, checked together once all are set.
    fn with(&self, caller: &str, map: &Map) -> RhaiResult<Self> {
        check_keys(caller, map, Self::KEYS)?;
        let mut out = self.clone();
        for (key, value) in map {
            out.set(caller, key, value)?;
        }
        out.check(caller)?;
        Ok(out)
    }
    /// A whole one as it is, or a map of settings over `base`.
    fn given(caller: &str, value: &Dynamic, base: &Self) -> RhaiResult<Self> {
        if let Some(whole) = value.clone().try_cast::<Self>() {
            return Ok(whole);
        }
        match value.clone().try_cast::<Map>() {
            Some(map) => base.with(caller, &map),
            None => Err(format!("{}: should be a {} or a map of its settings, got {}", caller, Self::NAME, value.type_name()).into()),
        }
    }
}

/// Register each of `T`'s settings as a property; setting one checks the
/// settings together, as constructors do.
pub(super) fn register_settings<T: Settings>(engine: &mut rhai::Engine) {
    for &key in T::KEYS {
        engine.register_get(key, move |s: &mut T| s.get(key));
        engine.register_set(key, move |s: &mut T, value: Dynamic| -> RhaiResult<()> {
            let mut changed = s.clone();
            changed.set(T::NAME, key, &value)?;
            changed.check(T::NAME)?;
            *s = changed;
            Ok(())
        });
    }
}

/// A setting's number: finite, whole or decimal.
pub(super) fn setting_number(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<f64> {
    float(caller, &format!("'{}'", key), value)
}

/// A setting's number of at least 0.
pub(super) fn at_least_zero(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<f64> {
    let n = setting_number(caller, key, value)?;
    if n < 0.0 {
        return Err(format!("{}: '{}' can't be negative, got {}", caller, key, n).into());
    }
    Ok(n)
}

/// A setting's positive number.
pub(super) fn above_zero(caller: &str, key: &str, value: &Dynamic) -> RhaiResult<f64> {
    let n = setting_number(caller, key, value)?;
    if n <= 0.0 {
        return Err(format!("{}: '{}' must be positive, got {}", caller, key, n).into());
    }
    Ok(n)
}

/// A setting's string, one of `choices`.
pub(super) fn one_of(caller: &str, key: &str, value: &Dynamic, choices: &[&str]) -> RhaiResult<String> {
    let text = value.clone().into_string()
        .map_err(|t| -> Box<EvalAltResult> { format!("{}: '{}' should be a string, got {}", caller, key, t).into() })?;
    if !choices.contains(&text.as_str()) {
        return Err(format!("{}: unknown {} '{}'; expected one of {}", caller, key, text, choices.join(", ")).into());
    }
    Ok(text)
}

/// The message of a runtime error, without rhai's position.
pub(super) fn unwrap_runtime(err: &EvalAltResult) -> String {
    match err {
        EvalAltResult::ErrorRuntime(message, _) => message.to_string(),
        EvalAltResult::ErrorInFunctionCall(_, _, inner, _) => unwrap_runtime(inner),
        other => other.to_string(),
    }
}
