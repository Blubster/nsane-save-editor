use crate::IgbError;
use crate::reader::Reader;

/// One serialized field of an object. The variant is the field's meta field type.
///
/// Object, memory and vector references hold *ids* (positions in the object
/// index). Values of `0x8000_0000` and above are "no reference" sentinels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldValue {
    /// `igStringMetaField`: an index into the string table.
    Str(u32),
    /// `igObjectRefMetaField`.
    ObjRef(u32),
    /// `igMemoryRefMetaField`.
    MemRef(u32),
    /// `igIntMetaField`.
    Int(i32),
    /// `igEnumMetaField`.
    Enum(i32),
    /// `igUnsignedIntMetaField`.
    UInt(u32),
    /// `igFloatMetaField`, as raw bits so the round trip is exact.
    Float(u32),
    /// `igBoolMetaField`: 4 bytes, the value is the low byte. The upper bytes are
    /// uninitialised memory left by the game; they are kept as is.
    Bool { value: u8, padding: u32 },
    /// `igHandleMetaField`: `(name hash, namespace hash)`.
    Handle(u32, u32),
    /// `igVectorMetaField`.
    Vector(u32, u32),
}

impl FieldValue {
    /// The meta field type name this value is written as.
    pub fn type_name(&self) -> &'static str {
        match self {
            FieldValue::Str(_) => "igStringMetaField",
            FieldValue::ObjRef(_) => "igObjectRefMetaField",
            FieldValue::MemRef(_) => "igMemoryRefMetaField",
            FieldValue::Int(_) => "igIntMetaField",
            FieldValue::Enum(_) => "igEnumMetaField",
            FieldValue::UInt(_) => "igUnsignedIntMetaField",
            FieldValue::Float(_) => "igFloatMetaField",
            FieldValue::Bool { .. } => "igBoolMetaField",
            FieldValue::Handle(..) => "igHandleMetaField",
            FieldValue::Vector(..) => "igVectorMetaField",
        }
    }

    /// Reads a field of the meta field type `type_name`.
    /// `Ok(None)` for a type that saves never use.
    pub(crate) fn read(type_name: &str, r: &mut Reader) -> Result<Option<Self>, IgbError> {
        Ok(Some(match type_name {
            "igStringMetaField" => FieldValue::Str(r.u32()?),
            "igObjectRefMetaField" => FieldValue::ObjRef(r.u32()?),
            "igMemoryRefMetaField" => FieldValue::MemRef(r.u32()?),
            "igIntMetaField" => FieldValue::Int(r.i32()?),
            "igEnumMetaField" => FieldValue::Enum(r.i32()?),
            "igUnsignedIntMetaField" => FieldValue::UInt(r.u32()?),
            "igFloatMetaField" => FieldValue::Float(r.u32()?),
            "igBoolMetaField" => {
                let raw = r.u32()?;
                FieldValue::Bool {
                    value: raw as u8,
                    padding: raw & !0xFF,
                }
            }
            "igHandleMetaField" => FieldValue::Handle(r.u32()?, r.u32()?),
            "igVectorMetaField" => FieldValue::Vector(r.u32()?, r.u32()?),
            _ => return Ok(None),
        }))
    }

    /// The serialized words: one, or two for handles and vectors.
    pub(crate) fn words(&self) -> ([u32; 2], usize) {
        match *self {
            FieldValue::Str(v)
            | FieldValue::ObjRef(v)
            | FieldValue::MemRef(v)
            | FieldValue::UInt(v)
            | FieldValue::Float(v) => ([v, 0], 1),
            FieldValue::Int(v) | FieldValue::Enum(v) => ([v as u32, 0], 1),
            FieldValue::Bool { value, padding } => ([(padding & !0xFF) | u32::from(value), 0], 1),
            FieldValue::Handle(a, b) | FieldValue::Vector(a, b) => ([a, b], 2),
        }
    }

    /// The value as a plain u32 for the one-word types (signed ones reinterpreted).
    pub fn as_u32(&self) -> Option<u32> {
        match self.words() {
            ([v, _], 1) => Some(v),
            _ => None,
        }
    }
}

/// One object of the objects section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgObject {
    pub id: u32,
    /// Class name, from the meta objects table.
    pub class: String,
    /// Fields in the class's declaration order.
    pub fields: Vec<FieldValue>,
}

impl IgObject {
    pub fn field(&self, i: usize) -> Option<&FieldValue> {
        self.fields.get(i)
    }

    pub fn field_mut(&mut self, i: usize) -> Option<&mut FieldValue> {
        self.fields.get_mut(i)
    }
}
