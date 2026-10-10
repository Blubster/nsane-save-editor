//! The key → value table right after the header, shown on the save slot screen.
//!
//! Layout: `u32 3`, then three size-prefixed blobs: the entries
//! `(u32 type, u32 key offset, u32 value offset)`, the NUL-terminated keys and the
//! values. Type 0 is an i32, 1 an f32 and 2 a NUL-terminated string.

use crate::reader::{Reader, Writer};
use crate::{IgbError, latin1};

#[derive(Debug, Clone, PartialEq)]
pub enum SummaryValue {
    Int(i32),
    Float(f32),
    /// Latin-1 bytes, without the NUL.
    Str(Vec<u8>),
}

impl SummaryValue {
    fn type_code(&self) -> u32 {
        match self {
            SummaryValue::Int(_) => 0,
            SummaryValue::Float(_) => 1,
            SummaryValue::Str(_) => 2,
        }
    }

    /// A string value is written NUL-terminated, so it can't hold a NUL.
    fn check(&self) -> Result<(), IgbError> {
        match self {
            SummaryValue::Str(s) if s.contains(&0) => Err(IgbError::Malformed("summary string")),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummaryEntry {
    /// Latin-1 bytes, without the NUL.
    pub key: Vec<u8>,
    pub value: SummaryValue,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Summary {
    entries: Vec<SummaryEntry>,
}

/// Bytes from `offset` up to the next NUL.
fn cstr_at(blob: &[u8], offset: usize) -> Result<&[u8], IgbError> {
    let rest = blob
        .get(offset..)
        .ok_or(IgbError::Malformed("summary offset"))?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or(IgbError::Malformed("summary string"))?;
    Ok(&rest[..end])
}

impl Summary {
    pub(crate) fn parse(r: &mut Reader) -> Result<Self, IgbError> {
        if r.u32()? != 3 {
            return Err(IgbError::Malformed("summary"));
        }
        let n = r.len32()?;
        let table = r.take(n)?;
        let n = r.len32()?;
        let keys = r.take(n)?;
        let n = r.len32()?;
        let values = r.take(n)?;
        let (records, []) = table.as_chunks::<12>() else {
            return Err(IgbError::Malformed("summary entries"));
        };
        let mut entries = Vec::new();
        for record in records {
            let mut t = Reader::new(record);
            let (vtype, koff, voff) = (t.u32()?, t.len32()?, t.len32()?);
            let key = cstr_at(keys, koff)?.to_vec();
            let value = match vtype {
                0 | 1 => {
                    let bytes = voff
                        .checked_add(4)
                        .and_then(|end| values.get(voff..end))
                        .ok_or(IgbError::Malformed("summary value"))?;
                    let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
                    if vtype == 0 {
                        SummaryValue::Int(i32::from_le_bytes(bytes))
                    } else {
                        SummaryValue::Float(f32::from_le_bytes(bytes))
                    }
                }
                2 => SummaryValue::Str(cstr_at(values, voff)?.to_vec()),
                _ => return Err(IgbError::Malformed("summary value type")),
            };
            entries.push(SummaryEntry { key, value });
        }
        Ok(Self { entries })
    }

    /// Keys and values are written in entry order, each blob packed with no gaps.
    pub(crate) fn build(&self, w: &mut Writer) -> Result<(), IgbError> {
        let (mut table, mut keys, mut values) = (Writer::default(), Vec::new(), Vec::new());
        for e in &self.entries {
            table.u32(e.value.type_code());
            table.len32(keys.len())?;
            table.len32(values.len())?;
            keys.extend_from_slice(&e.key);
            keys.push(0);
            match &e.value {
                SummaryValue::Int(v) => values.extend_from_slice(&v.to_le_bytes()),
                SummaryValue::Float(v) => values.extend_from_slice(&v.to_le_bytes()),
                SummaryValue::Str(s) => {
                    values.extend_from_slice(s);
                    values.push(0);
                }
            }
        }
        w.u32(3);
        for blob in [&table.out, &keys, &values] {
            w.len32(blob.len())?;
            w.bytes(blob);
        }
        Ok(())
    }

    pub fn entries(&self) -> &[SummaryEntry] {
        &self.entries
    }

    fn position(&self, key: &str) -> Option<usize> {
        let key = latin1::encode(key)?;
        self.entries.iter().position(|e| e.key == key)
    }

    pub fn get(&self, key: &str) -> Option<&SummaryValue> {
        self.position(key).map(|i| &self.entries[i].value)
    }

    /// Changes an existing value. The key must exist and keep its value type.
    pub fn set(&mut self, key: &str, value: SummaryValue) -> Result<(), IgbError> {
        let i = self
            .position(key)
            .ok_or_else(|| IgbError::MissingKey(key.to_owned()))?;
        let entry = &mut self.entries[i];
        if entry.value.type_code() != value.type_code() {
            return Err(IgbError::TypeMismatch(key.to_owned()));
        }
        value.check()?;
        entry.value = value;
        Ok(())
    }

    /// Adds a new entry right after the existing key `after`.
    pub fn insert_after(
        &mut self,
        after: &str,
        key: &str,
        value: SummaryValue,
    ) -> Result<(), IgbError> {
        if self.position(key).is_some() {
            return Err(IgbError::DuplicateKey(key.to_owned()));
        }
        value.check()?;
        let encoded = latin1::encode(key)
            .filter(|k| !k.contains(&0))
            .ok_or(IgbError::Malformed("summary key"))?;
        let i = self
            .position(after)
            .ok_or_else(|| IgbError::MissingKey(after.to_owned()))?;
        self.entries.insert(
            i + 1,
            SummaryEntry {
                key: encoded,
                value,
            },
        );
        Ok(())
    }
}
