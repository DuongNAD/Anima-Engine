//! Canonical deterministic byte encoding for hashed World Record payloads.
//!
//! SHA-256 digests over [`crate::core::world_record::WorldRecordHeader`] and
//! [`crate::core::world_record::SegmentBody`] must agree across platforms, builds, and
//! `serde_json` feature combinations. This encoder:
//!
//! 1. Rejects non-finite numbers (`NaN` / `±inf`).
//! 2. Normalizes floating-point negative zero (`-0.0`) to `+0.0`.
//! 3. Recursively sorts every JSON object's keys in UTF-8 byte order so encoding never depends
//!    on map iteration order (`BTreeMap` vs `IndexMap` under `preserve_order`).
//! 4. Emits compact UTF-8 JSON with zero superfluous whitespace (`serde_json` + `float_roundtrip`).

use serde::Serialize;
use serde_json::{Number, Value};

use super::types::WorldRecordError;

/// Encode any serializable payload into canonical compact JSON bytes.
pub fn canonical_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, WorldRecordError> {
    let val = serde_json::to_value(value)
        .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;
    let mut out = Vec::with_capacity(256);
    write_canonical_value(&val, "$", &mut out)?;
    Ok(out)
}

fn write_canonical_value(
    value: &Value,
    path: &str,
    out: &mut Vec<u8>,
) -> Result<(), WorldRecordError> {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Number(num) => write_canonical_number(num, path, out)?,
        Value::String(s) => serde_json::to_writer(&mut *out, s)
            .map_err(|err| WorldRecordError::Serialization(err.to_string()))?,
        Value::Array(items) => {
            out.push(b'[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(b',');
                }
                let child_path = format!("{path}[{idx}]");
                write_canonical_value(item, &child_path, out)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&str, &Value)> =
                map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            entries.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push(b'{');
            for (idx, (key, val)) in entries.iter().enumerate() {
                if idx > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key)
                    .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;
                out.push(b':');
                let child_path = format!("{path}.{key}");
                write_canonical_value(val, &child_path, out)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

fn write_canonical_number(
    num: &Number,
    path: &str,
    out: &mut Vec<u8>,
) -> Result<(), WorldRecordError> {
    if let Some(u) = num.as_u64() {
        return serde_json::to_writer(&mut *out, &u)
            .map_err(|err| WorldRecordError::Serialization(err.to_string()));
    }
    if let Some(i) = num.as_i64() {
        return serde_json::to_writer(&mut *out, &i)
            .map_err(|err| WorldRecordError::Serialization(err.to_string()));
    }
    let Some(f) = num.as_f64() else {
        return Err(WorldRecordError::NonFiniteFloat {
            path: path.to_string(),
        });
    };
    if !f.is_finite() {
        return Err(WorldRecordError::NonFiniteFloat {
            path: path.to_string(),
        });
    }
    let normalized = if f == 0.0 { 0.0_f64 } else { f };
    serde_json::to_writer(&mut *out, &normalized)
        .map_err(|err| WorldRecordError::Serialization(err.to_string()))
}
