use crate::IgbError;

/// Element type of a memory block, from its directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// `igObjectRefMetaField`: object ids.
    ObjRef,
    /// `igBoolMetaField`: one byte per element.
    Bool,
    /// `igIntMetaField`.
    Int,
    /// `igStringMetaField`: string table indices.
    Str,
    /// `igNameMetaField`: `(string index, hash)`.
    Name,
}

impl ElementType {
    pub fn from_type_name(name: &str) -> Result<Self, IgbError> {
        Ok(match name {
            "igObjectRefMetaField" => ElementType::ObjRef,
            "igBoolMetaField" => ElementType::Bool,
            "igIntMetaField" => ElementType::Int,
            "igStringMetaField" => ElementType::Str,
            "igNameMetaField" => ElementType::Name,
            _ => return Err(IgbError::UnsupportedElementType(name.to_owned())),
        })
    }

    pub fn type_name(self) -> &'static str {
        match self {
            ElementType::ObjRef => "igObjectRefMetaField",
            ElementType::Bool => "igBoolMetaField",
            ElementType::Int => "igIntMetaField",
            ElementType::Str => "igStringMetaField",
            ElementType::Name => "igNameMetaField",
        }
    }

    /// Bytes per element.
    pub fn size(self) -> usize {
        match self {
            ElementType::Bool => 1,
            ElementType::Name => 8,
            ElementType::ObjRef | ElementType::Int | ElementType::Str => 4,
        }
    }
}

/// One raw array of the memory section.
///
/// The element count lives in the block's directory entry; `data` always holds
/// exactly `count * element_type.size()` bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBlock {
    pub id: u32,
    pub element_type: ElementType,
    pub(crate) count: u32,
    pub(crate) data: Vec<u8>,
}

impl MemoryBlock {
    pub fn count(&self) -> usize {
        self.count as usize
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    fn word(&self, offset: usize) -> Option<u32> {
        let b = self.data.get(offset..offset.checked_add(4)?)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn set_word(&mut self, offset: usize, value: u32) -> Option<()> {
        let b = self.data.get_mut(offset..offset.checked_add(4)?)?;
        b.copy_from_slice(&value.to_le_bytes());
        Some(())
    }

    /// Element `i` of a 4-byte block (object refs, ints, string indices).
    pub fn u32_at(&self, i: usize) -> Option<u32> {
        match self.element_type {
            ElementType::ObjRef | ElementType::Int | ElementType::Str => {
                self.word(i.checked_mul(4)?)
            }
            _ => None,
        }
    }

    pub fn set_u32(&mut self, i: usize, value: u32) -> Option<()> {
        match self.element_type {
            ElementType::ObjRef | ElementType::Int | ElementType::Str => {
                self.set_word(i.checked_mul(4)?, value)
            }
            _ => None,
        }
    }

    /// Element `i` of a bool block, as its raw byte.
    pub fn bool_at(&self, i: usize) -> Option<u8> {
        match self.element_type {
            ElementType::Bool => self.data.get(i).copied(),
            _ => None,
        }
    }

    pub fn set_bool(&mut self, i: usize, value: u8) -> Option<()> {
        match self.element_type {
            ElementType::Bool => {
                *self.data.get_mut(i)? = value;
                Some(())
            }
            _ => None,
        }
    }

    /// Element `i` of a name block: `(string index, hash)`.
    pub fn name_at(&self, i: usize) -> Option<(u32, u32)> {
        match self.element_type {
            ElementType::Name => {
                let o = i.checked_mul(8)?;
                Some((self.word(o)?, self.word(o + 4)?))
            }
            _ => None,
        }
    }

    pub fn set_name(&mut self, i: usize, value: (u32, u32)) -> Option<()> {
        match self.element_type {
            ElementType::Name => {
                let o = i.checked_mul(8)?;
                self.word(o + 4)?;
                self.set_word(o, value.0)?;
                self.set_word(o + 4, value.1)
            }
            _ => None,
        }
    }
}
