//! Canonical deterministic byte encoding for hashed World Record payloads.
//!
//! SHA-256 digests over [`crate::core::world_record::WorldRecordHeader`] and
//! [`crate::core::world_record::SegmentBody`] must agree across platforms, builds, and
//! `serde_json` feature combinations. This encoder:
//!
//! 1. Rejects non-finite numbers (`NaN` / `±inf`) directly on any `T: Serialize` before
//!    converting via `serde_json::to_value` (which would otherwise coerce them to `null`).
//! 2. Normalizes floating-point negative zero (`-0.0`) to `+0.0`.
//! 3. Recursively sorts every JSON object's keys in UTF-8 byte order so encoding never depends
//!    on map iteration order (`BTreeMap` vs `IndexMap` under `preserve_order`).
//! 4. Emits compact UTF-8 JSON with zero superfluous whitespace (`serde_json` + `float_roundtrip`).

use std::fmt;

use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use serde::Serialize;
use serde_json::{Number, Value};

use super::types::WorldRecordError;

/// Encode any serializable payload into canonical compact JSON bytes.
pub fn canonical_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, WorldRecordError> {
    value
        .serialize(FiniteFloatChecker { path: "$" })
        .map_err(|err| err.into_world_record_error())?;
    let val = serde_json::to_value(value)
        .map_err(|err| WorldRecordError::Serialization(err.to_string()))?;
    let mut out = Vec::with_capacity(256);
    write_canonical_value(&val, "$", &mut out)?;
    Ok(out)
}

#[derive(Debug)]
struct FiniteCheckError(WorldRecordError);

impl FiniteCheckError {
    fn into_world_record_error(self) -> WorldRecordError {
        self.0
    }
}

impl fmt::Display for FiniteCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for FiniteCheckError {}

impl ser::Error for FiniteCheckError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(WorldRecordError::Serialization(msg.to_string()))
    }
}

#[derive(Clone, Copy)]
struct FiniteFloatChecker {
    path: &'static str,
}

#[derive(Clone, Copy)]
struct FiniteCheckCompound {
    path: &'static str,
}

impl Serializer for FiniteFloatChecker {
    type Ok = ();
    type Error = FiniteCheckError;
    type SerializeSeq = FiniteCheckCompound;
    type SerializeTuple = FiniteCheckCompound;
    type SerializeTupleStruct = FiniteCheckCompound;
    type SerializeTupleVariant = FiniteCheckCompound;
    type SerializeMap = FiniteCheckCompound;
    type SerializeStruct = FiniteCheckCompound;
    type SerializeStructVariant = FiniteCheckCompound;

    fn serialize_bool(self, _v: bool) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_i8(self, _v: i8) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_i16(self, _v: i16) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_i32(self, _v: i32) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_i64(self, _v: i64) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_i128(self, _v: i128) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_u8(self, _v: u8) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_u16(self, _v: u16) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_u32(self, _v: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_u64(self, _v: u64) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_u128(self, _v: u128) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        if !v.is_finite() {
            return Err(FiniteCheckError(WorldRecordError::NonFiniteFloat {
                path: self.path.to_string(),
            }));
        }
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        if !v.is_finite() {
            return Err(FiniteCheckError(WorldRecordError::NonFiniteFloat {
                path: self.path.to_string(),
            }));
        }
        Ok(())
    }

    fn serialize_char(self, _v: char) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_str(self, _v: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(Self { path: variant })
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(FiniteCheckCompound { path: self.path })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(FiniteCheckCompound { path: self.path })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(FiniteCheckCompound { path: name })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(FiniteCheckCompound { path: variant })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(FiniteCheckCompound { path: self.path })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(FiniteCheckCompound { path: name })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(FiniteCheckCompound { path: variant })
    }
}

impl SerializeSeq for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: self.path })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeTuple for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: self.path })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeTupleStruct for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: self.path })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeTupleVariant for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: self.path })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeMap for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(FiniteFloatChecker { path: self.path })
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: self.path })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeStruct for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: key })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SerializeStructVariant for FiniteCheckCompound {
    type Ok = ();
    type Error = FiniteCheckError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        value.serialize(FiniteFloatChecker { path: key })
    }

    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
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
