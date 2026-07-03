#![deny(warnings)]

//! Lenient-in-encoding, strict-in-value coercion for JSON tool arguments.
//!
//! LLM tool-call plumbing routinely re-encodes an integer as a JSON float
//! (`200` becomes `200.0`) or a string (`"200"`), and a boolean as `"true"`
//! or `1`. Those are lossless, unambiguous encodings of the value the caller
//! meant, so we accept them rather than rejecting a request over a wire
//! encoding the model doesn't control.
//!
//! We do **not** accept genuinely invalid data. Negative, fractional,
//! non-finite, or out-of-range numbers, non-numeric strings, and anything that
//! isn't an unambiguous encoding of the target type are rejected with a
//! specific, non-contradictory message. Domain rules (e.g. "line numbers start
//! at 1", bounds against file length) stay in the operation layer — this
//! module only settles the wire encoding.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Coerce a JSON value to `u64`.
///
/// Accepts a JSON integer, a whole-valued finite JSON float (`200.0`), or a
/// base-10 digit string (`"200"`). Rejects negatives, fractions, non-finite
/// floats, out-of-range values, booleans, null, and non-numeric strings. On
/// failure returns a reason fragment (without a field name) that the caller
/// prefixes with the parameter name.
pub fn value_to_u64(v: &Value) -> Result<u64, String> {
    match v {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(u)
            } else if let Some(i) = n.as_i64() {
                // A representable-but-negative integer.
                Err(format!("must be a non-negative integer (got {i})"))
            } else {
                // Not an integer in serde_json's model, so it is a float.
                let f = n
                    .as_f64()
                    .ok_or_else(|| "must be a non-negative integer".to_string())?;
                float_to_u64(f)
            }
        }
        // Only a clean decimal integer string. `u64::from_str` already rejects
        // fractions ("200.0"), signs beyond a leading '+', whitespace, and
        // radix prefixes, so we stay strict without extra checks.
        Value::String(s) => s
            .parse::<u64>()
            .map_err(|_| format!("must be a non-negative integer (got string {s:?})")),
        Value::Bool(_) => Err("must be a non-negative integer, not a boolean".to_string()),
        Value::Null => Err("must be a non-negative integer, not null".to_string()),
        Value::Array(_) | Value::Object(_) => Err("must be a non-negative integer".to_string()),
    }
}

fn float_to_u64(f: f64) -> Result<u64, String> {
    if !f.is_finite() {
        Err("must be a finite whole number".to_string())
    } else if f < 0.0 {
        Err(format!("must be a non-negative integer (got {f})"))
    } else if f.fract() != 0.0 {
        Err(format!("must be a whole number, not a fraction (got {f})"))
    } else if f > u64::MAX as f64 {
        Err(format!("is too large (got {f})"))
    } else {
        Ok(f as u64)
    }
}

/// Coerce a JSON value to `bool`.
///
/// Accepts a JSON boolean, the strings `"true"`/`"false"`/`"1"`/`"0"` (case
/// insensitive), and the numbers `1`/`0`. Everything else — including numbers
/// other than 0/1 and strings like `"yes"` — is rejected.
pub fn value_to_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(format!("must be a boolean (got string {s:?})")),
        },
        Value::Number(n) => match n.as_u64() {
            Some(1) => Ok(true),
            Some(0) => Ok(false),
            _ => Err("must be a boolean (got a number other than 0 or 1)".to_string()),
        },
        Value::Null => Err("must be a boolean, not null".to_string()),
        Value::Array(_) | Value::Object(_) => Err("must be a boolean".to_string()),
    }
}

/// `#[serde(deserialize_with = "de_u64")]` — coerce a field like
/// [`value_to_u64`] instead of serde's default strict integer decoding.
pub fn de_u64<'de, D>(d: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    value_to_u64(&v).map_err(serde::de::Error::custom)
}

/// `#[serde(deserialize_with = "de_u32")]` — coerce like [`value_to_u64`], then
/// narrow to `u32` (rejecting values that overflow it).
pub fn de_u32<'de, D>(d: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    let n = value_to_u64(&v).map_err(serde::de::Error::custom)?;
    u32::try_from(n).map_err(|_| serde::de::Error::custom(format!("must be at most {}", u32::MAX)))
}

/// `#[serde(deserialize_with = "de_bool")]` — coerce a field like
/// [`value_to_bool`].
pub fn de_bool<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    value_to_bool(&v).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn u64_accepts_lossless_encodings() {
        assert_eq!(value_to_u64(&json!(200)).unwrap(), 200);
        assert_eq!(value_to_u64(&json!(200.0)).unwrap(), 200); // float from LLM plumbing
        assert_eq!(value_to_u64(&json!("200")).unwrap(), 200); // stringified integer
        assert_eq!(value_to_u64(&json!(0)).unwrap(), 0);
        assert_eq!(value_to_u64(&json!(0.0)).unwrap(), 0);
    }

    #[test]
    fn u64_rejects_invalid_data() {
        // Negative — as integer and as float and as string.
        assert!(value_to_u64(&json!(-5)).is_err());
        assert!(value_to_u64(&json!(-5.0)).is_err());
        assert!(value_to_u64(&json!("-5")).is_err());
        // Fractional.
        assert!(value_to_u64(&json!(200.5)).is_err());
        assert!(value_to_u64(&json!("200.0")).is_err());
        // Non-numeric / malformed strings.
        assert!(value_to_u64(&json!("abc")).is_err());
        assert!(value_to_u64(&json!("200px")).is_err());
        assert!(value_to_u64(&json!("0x10")).is_err());
        assert!(value_to_u64(&json!("1e3")).is_err());
        assert!(value_to_u64(&json!("")).is_err());
        // Wrong types.
        assert!(value_to_u64(&json!(true)).is_err());
        assert!(value_to_u64(&json!(null)).is_err());
        assert!(value_to_u64(&json!([1])).is_err());
        assert!(value_to_u64(&json!({"n": 1})).is_err());
    }

    #[test]
    fn u64_error_message_is_not_contradictory() {
        // The bug: a valid integer sent as a float used to be rejected with
        // "must be a non-negative integer". It must now succeed.
        assert!(value_to_u64(&json!(200.0)).is_ok());
        // And a genuinely bad value names what is actually wrong.
        let msg = value_to_u64(&json!(200.5)).unwrap_err();
        assert!(msg.contains("whole number"), "got: {msg}");
    }

    #[test]
    fn u64_rejects_overflowing_float() {
        // Larger than u64::MAX; must not silently wrap.
        assert!(value_to_u64(&json!(1e30)).is_err());
    }

    #[test]
    fn bool_accepts_lossless_encodings() {
        assert!(value_to_bool(&json!(true)).unwrap());
        assert!(!value_to_bool(&json!(false)).unwrap());
        assert!(value_to_bool(&json!("true")).unwrap());
        assert!(value_to_bool(&json!("True")).unwrap());
        assert!(!value_to_bool(&json!("FALSE")).unwrap());
        assert!(value_to_bool(&json!("1")).unwrap());
        assert!(!value_to_bool(&json!("0")).unwrap());
        assert!(value_to_bool(&json!(1)).unwrap());
        assert!(!value_to_bool(&json!(0)).unwrap());
    }

    #[test]
    fn bool_rejects_ambiguous_or_invalid() {
        assert!(value_to_bool(&json!("yes")).is_err());
        assert!(value_to_bool(&json!("")).is_err());
        assert!(value_to_bool(&json!(2)).is_err());
        assert!(value_to_bool(&json!(-1)).is_err());
        assert!(value_to_bool(&json!(null)).is_err());
        assert!(value_to_bool(&json!([true])).is_err());
    }

    #[test]
    fn de_u32_narrows_and_rejects_overflow() {
        #[derive(serde::Deserialize)]
        struct T {
            #[serde(deserialize_with = "de_u32")]
            n: u32,
        }
        let ok: T = serde_json::from_value(json!({"n": "7"})).unwrap();
        assert_eq!(ok.n, 7);
        let ok2: T = serde_json::from_value(json!({"n": 7.0})).unwrap();
        assert_eq!(ok2.n, 7);
        assert!(serde_json::from_value::<T>(json!({"n": 5_000_000_000u64})).is_err());
    }

    #[test]
    fn de_u64_and_de_bool_coerce_in_structs() {
        #[derive(serde::Deserialize)]
        struct T {
            #[serde(deserialize_with = "de_u64")]
            line: u64,
            #[serde(default, deserialize_with = "de_bool")]
            flag: bool,
        }
        let t: T = serde_json::from_value(json!({"line": 12.0, "flag": "true"})).unwrap();
        assert_eq!(t.line, 12);
        assert!(t.flag);
        // Absent bool falls back to the default without invoking the coercer.
        let t2: T = serde_json::from_value(json!({"line": "3"})).unwrap();
        assert_eq!(t2.line, 3);
        assert!(!t2.flag);
        // Fractional line is still rejected.
        assert!(serde_json::from_value::<T>(json!({"line": 12.5})).is_err());
    }
}
