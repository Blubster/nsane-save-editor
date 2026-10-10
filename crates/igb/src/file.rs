use std::collections::BTreeMap;

use crate::memory::{ElementType, MemoryBlock};
use crate::object::{FieldValue, IgObject};
use crate::reader::{Reader, Writer};
use crate::summary::Summary;
use crate::{IgbError, latin1};

const MAGIC: u32 = 0xFADA;

const OBJECT_DIR_ENTRY: &str = "igObjectDirEntry";
const MEMORY_DIR_ENTRY: &str = "igMemoryDirEntry";

/// A class definition from the meta objects table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: String,
    pub parent: i32,
    /// `(meta field index, slot, runtime size)` per serialized field.
    pub fields: Vec<(u16, u16, u16)>,
}

/// One directory record: the class of the entry (by name) and its values.
///
/// An object entry holds `(name, class index, pool)`, a memory entry
/// `(name, element count, element meta field type, active, alignment, pool)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub class: String,
    pub values: Vec<u32>,
}

/// A parsed save. [`IgbFile::build`] writes it back byte for byte.
///
/// The sections that never change on edit (meta fields, meta objects, memory
/// pools and the pool index list) are kept as raw bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct IgbFile {
    header: [u32; 12],
    summary: Summary,
    /// Index 0 is always the null string (`None`).
    pub(crate) strings: Vec<Option<Vec<u8>>>,
    meta_fields: Vec<String>,
    raw_meta_fields: Vec<u8>,
    classes: Vec<ClassDef>,
    raw_meta_objects: Vec<u8>,
    handle_flag: u32,
    handle_names: Vec<(u32, Vec<u8>)>,
    raw_pools: Vec<u8>,
    pub(crate) directory: Vec<DirEntry>,
    /// One directory index per id. Objects and memory blocks share the id space.
    pub(crate) index: Vec<u16>,
    index_padding: Vec<u8>,
    unknown_after_index: u32,
    pub(crate) objects: BTreeMap<u32, IgObject>,
    pub(crate) memory: BTreeMap<u32, MemoryBlock>,
    padded: bool,
    file_size: usize,
}

fn check_size(section: &'static str, expected: usize, actual: usize) -> Result<(), IgbError> {
    if expected == actual {
        Ok(())
    } else {
        Err(IgbError::SectionSize {
            section,
            expected,
            actual,
        })
    }
}

/// Bytes up to the first NUL, or all of them.
fn until_nul(b: &[u8]) -> &[u8] {
    b.iter().position(|&c| c == 0).map_or(b, |end| &b[..end])
}

impl IgbFile {
    /// Parses a whole save file: Switch (zero-padded) or PC (unpadded).
    pub fn parse(data: &[u8]) -> Result<Self, IgbError> {
        let mut r = Reader::new(data);
        let mut header = [0u32; 12];
        for word in &mut header {
            *word = r.u32()?;
        }
        if header[10] != MAGIC {
            return Err(IgbError::BadMagic(header[10]));
        }
        let [
            entries_size,
            entries_count,
            meta_objects_size,
            meta_objects_count,
            objects_size,
            objects_count,
            memory_size,
            _memory_count,
            meta_fields_size,
            meta_fields_count,
            ..,
        ] = header.map(|v| v as usize);

        let summary = Summary::parse(&mut r)?;

        // string table: u32 size (itself included), u32 count, then u32 length
        // (NUL included) + bytes per string; length 0 is the null string
        let start = r.pos;
        let (size, count) = (r.len32()?, r.len32()?);
        let mut strings = Vec::new();
        for _ in 0..count {
            let n = r.len32()?;
            strings.push(match r.take(n)? {
                [] => None,
                [s @ .., 0] => Some(s.to_vec()),
                _ => return Err(IgbError::Malformed("string table entry")),
            });
        }
        check_size("string table", size, r.pos - start)?;

        // meta fields: fixed-size records, then the names
        let start = r.pos;
        let mut name_lengths = Vec::new();
        for _ in 0..meta_fields_count {
            let mut record = Reader::new(r.take(12)?);
            name_lengths.push(record.len32()?);
        }
        let mut meta_fields = Vec::new();
        for n in name_lengths {
            meta_fields.push(latin1::decode(until_nul(r.take(n)?)));
        }
        let raw_meta_fields = data[start..r.pos].to_vec();
        check_size("meta fields", meta_fields_size, r.pos - start)?;

        // meta objects (class definitions): fixed-size records, then per class
        // its name and its field list
        let start = r.pos;
        let mut records = Vec::new();
        for _ in 0..meta_objects_count {
            let mut record = Reader::new(r.take(24)?);
            let name_len = record.len32()?;
            record.take(8)?;
            let field_count = usize::try_from(record.i32()?)
                .map_err(|_| IgbError::Malformed("class field count"))?;
            records.push((name_len, field_count, record.i32()?));
        }
        let mut classes = Vec::new();
        for (name_len, field_count, parent) in records {
            let name = latin1::decode(until_nul(r.take(name_len)?));
            let mut fields = Vec::new();
            for _ in 0..field_count {
                fields.push((r.u16()?, r.u16()?, r.u16()?));
            }
            classes.push(ClassDef {
                name,
                parent,
                fields,
            });
        }
        let raw_meta_objects = data[start..r.pos].to_vec();
        check_size("meta objects", meta_objects_size, r.pos - start)?;

        // handle names: the readable name of each handle hash
        let start = r.pos;
        let (size, handle_flag, n) = (r.len32()?, r.u32()?, r.len32()?);
        let mut hashes = Vec::new();
        for _ in 0..n {
            hashes.push(r.u32()?);
        }
        let mut handle_names = Vec::new();
        for hash in hashes {
            handle_names.push((hash, r.cstr()?.to_vec()));
        }
        check_size("handle names", size, r.pos - start)?;

        // memory pool names and pool index list: two size-prefixed blobs, kept raw
        let start = r.pos;
        for _ in 0..2 {
            let size = r.peek_u32()? as usize;
            if size < 4 {
                return Err(IgbError::Malformed("memory pools"));
            }
            r.take(size)?;
        }
        let raw_pools = data[start..r.pos].to_vec();

        // directory
        let start = r.pos;
        let mut directory = Vec::new();
        for _ in 0..entries_count {
            let (class, size) = (r.len32()?, r.len32()?);
            let class = classes.get(class).ok_or(IgbError::BadIndex {
                what: "class",
                index: class,
            })?;
            if size < 8 || size % 4 != 0 {
                return Err(IgbError::Malformed("directory record"));
            }
            let mut values = Vec::new();
            for _ in 0..(size - 8) / 4 {
                values.push(r.u32()?);
            }
            directory.push(DirEntry {
                class: class.name.clone(),
                values,
            });
        }
        check_size("directory", entries_size, r.pos - start)?;

        // object index: one directory index per id, then padding
        let (size, count) = (r.len32()?, r.len32()?);
        let mut index = Vec::new();
        for _ in 0..count {
            index.push(r.u16()?);
        }
        let padding = count
            .checked_mul(2)
            .and_then(|n| n.checked_add(8))
            .and_then(|n| size.checked_sub(n))
            .ok_or(IgbError::Malformed("object index"))?;
        let index_padding = r.take(padding)?.to_vec();
        let unknown_after_index = r.u32()?;

        let entry_of = |dir_idx: u16| {
            directory.get(dir_idx as usize).ok_or(IgbError::BadIndex {
                what: "directory",
                index: dir_idx as usize,
            })
        };

        // objects, in id order
        let start = r.pos;
        let mut objects = BTreeMap::new();
        for (id, &dir_idx) in (0u32..).zip(&index) {
            if entry_of(dir_idx)?.class == OBJECT_DIR_ENTRY {
                let object = read_object(&mut r, id, &classes, &meta_fields)?;
                objects.insert(id, object);
            }
        }
        check_size("objects", objects_size, r.pos - start)?;
        if objects.len() != objects_count {
            return Err(IgbError::ObjectCount {
                section: "objects",
                expected: objects_count,
                count: objects.len(),
            });
        }

        // memory blocks, in id order
        let start = r.pos;
        let mut memory = BTreeMap::new();
        for (id, &dir_idx) in (0u32..).zip(&index) {
            let entry = entry_of(dir_idx)?;
            if entry.class != MEMORY_DIR_ENTRY {
                continue;
            }
            let (&count, &type_idx) = entry
                .values
                .get(1)
                .zip(entry.values.get(2))
                .ok_or(IgbError::Malformed("memory directory entry"))?;
            let type_name = meta_fields
                .get(type_idx as usize)
                .ok_or(IgbError::BadIndex {
                    what: "meta field",
                    index: type_idx as usize,
                })?;
            let element_type = ElementType::from_type_name(type_name)?;
            let n = (count as usize)
                .checked_mul(element_type.size())
                .ok_or(IgbError::Malformed("memory block size"))?;
            let block = MemoryBlock {
                id,
                element_type,
                count,
                data: r.take(n)?.to_vec(),
            };
            memory.insert(id, block);
        }
        check_size("memory", memory_size, r.pos - start)?;

        if let Some(i) = r.rest().iter().position(|&b| b != 0) {
            return Err(IgbError::TrailingData(r.pos + i));
        }
        // Switch saves are zero padded to a fixed size; PC saves stop right after
        // the memory section and are written back at whatever length they need.
        let padded = r.pos < data.len();

        Ok(Self {
            header,
            summary,
            strings,
            meta_fields,
            raw_meta_fields,
            classes,
            raw_meta_objects,
            handle_flag,
            handle_names,
            raw_pools,
            directory,
            index,
            index_padding,
            unknown_after_index,
            objects,
            memory,
            padded,
            file_size: data.len(),
        })
    }

    /// The whole file. A padded (Switch) file keeps its original size and fails
    /// with [`IgbError::DoesNotFit`] when the content outgrows it.
    pub fn build(&self) -> Result<Vec<u8>, IgbError> {
        let mut out = self.build_unpadded()?;
        if self.padded {
            if out.len() > self.file_size {
                return Err(IgbError::DoesNotFit {
                    needed: out.len(),
                    capacity: self.file_size,
                });
            }
            out.resize(self.file_size, 0);
        }
        Ok(out)
    }

    /// The content without the Switch padding.
    pub fn build_unpadded(&self) -> Result<Vec<u8>, IgbError> {
        let mut directory = Writer::default();
        for entry in &self.directory {
            directory.u32(self.class_index(&entry.class)?);
            directory.len32(8 + 4 * entry.values.len())?;
            for &v in &entry.values {
                directory.u32(v);
            }
        }
        let mut objects = Writer::default();
        for object in self.objects.values() {
            self.write_object(&mut objects, object)?;
        }
        let mut memory = Writer::default();
        for block in self.memory.values() {
            memory.bytes(&block.data);
        }

        let mut header = self.header;
        let words = [
            (0, directory.out.len()),
            (1, self.directory.len()),
            (4, objects.out.len()),
            (5, self.objects.len()),
            (6, memory.out.len()),
            (7, self.memory.len()),
        ];
        for (i, n) in words {
            header[i] = u32::try_from(n).map_err(|_| IgbError::Malformed("section size"))?;
        }

        let mut w = Writer::default();
        for v in header {
            w.u32(v);
        }
        self.summary.build(&mut w)?;

        let mut strings = Writer::default();
        for s in &self.strings {
            match s {
                None => strings.u32(0),
                Some(s) => {
                    strings.len32(s.len() + 1)?;
                    strings.bytes(s);
                    strings.bytes(&[0]);
                }
            }
        }
        w.len32(strings.out.len() + 8)?;
        w.len32(self.strings.len())?;
        w.bytes(&strings.out);

        w.bytes(&self.raw_meta_fields);
        w.bytes(&self.raw_meta_objects);

        let mut handles = Writer::default();
        handles.u32(self.handle_flag);
        handles.len32(self.handle_names.len())?;
        for (hash, _) in &self.handle_names {
            handles.u32(*hash);
        }
        for (_, name) in &self.handle_names {
            handles.bytes(name);
            handles.bytes(&[0]);
        }
        w.len32(handles.out.len() + 4)?;
        w.bytes(&handles.out);

        w.bytes(&self.raw_pools);
        w.bytes(&directory.out);

        w.len32(8 + 2 * self.index.len() + self.index_padding.len())?;
        w.len32(self.index.len())?;
        for &i in &self.index {
            w.u16(i);
        }
        w.bytes(&self.index_padding);
        w.u32(self.unknown_after_index);
        w.bytes(&objects.out);
        w.bytes(&memory.out);
        Ok(w.out)
    }

    /// The index written for a class name: the first class with that name.
    fn class_index(&self, name: &str) -> Result<u32, IgbError> {
        self.classes
            .iter()
            .position(|c| c.name == name)
            .map(|i| i as u32)
            .ok_or_else(|| IgbError::UnknownClass(name.to_owned()))
    }

    fn write_object(&self, w: &mut Writer, object: &IgObject) -> Result<(), IgbError> {
        let mut body = Writer::default();
        for field in &object.fields {
            let (words, n) = field.words();
            for &v in &words[..n] {
                body.u32(v);
            }
        }
        w.u32(self.class_index(&object.class)?);
        w.len32(body.out.len() + 8)?;
        w.bytes(&body.out);
        Ok(())
    }

    // ------------------------------------------------------------------ accessors

    /// True for the Switch layout (zero-padded to a fixed size), false for PC.
    pub fn padded(&self) -> bool {
        self.padded
    }

    /// Size of the file this was parsed from.
    pub fn file_size(&self) -> usize {
        self.file_size
    }

    /// Container version (header word 11).
    pub fn version(&self) -> u32 {
        self.header[11]
    }

    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    pub fn summary_mut(&mut self) -> &mut Summary {
        &mut self.summary
    }

    /// The string table, null entries included.
    pub fn strings(&self) -> &[Option<Vec<u8>>] {
        &self.strings
    }

    /// String `idx` of the string table. `None` for the null entry, a
    /// "no reference" sentinel (`>= 0x8000_0000`) or an index out of range.
    pub fn string(&self, idx: u32) -> Option<&[u8]> {
        if idx >= 0x8000_0000 {
            return None;
        }
        self.strings.get(idx as usize)?.as_deref()
    }

    /// Meta field type names (`igIntMetaField`, …), in table order.
    pub fn meta_fields(&self) -> &[String] {
        &self.meta_fields
    }

    pub fn classes(&self) -> &[ClassDef] {
        &self.classes
    }

    /// `(hash, name)` pairs, in file order.
    pub fn handle_names(&self) -> &[(u32, Vec<u8>)] {
        &self.handle_names
    }

    /// The readable name of a handle `(name hash, namespace hash)`.
    pub fn handle_name(&self, handle: (u32, u32)) -> Option<&[u8]> {
        self.handle_names
            .iter()
            .find(|(hash, _)| *hash == handle.0)
            .map(|(_, name)| name.as_slice())
    }

    pub fn directory(&self) -> &[DirEntry] {
        &self.directory
    }

    /// One directory index per id.
    pub fn index(&self) -> &[u16] {
        &self.index
    }

    /// All objects, in id order.
    pub fn objects(&self) -> impl Iterator<Item = &IgObject> {
        self.objects.values()
    }

    /// Objects of one class, in id order.
    pub fn objects_of<'a>(&'a self, class: &'a str) -> impl Iterator<Item = &'a IgObject> {
        self.objects.values().filter(move |o| o.class == class)
    }

    pub fn object(&self, id: u32) -> Option<&IgObject> {
        self.objects.get(&id)
    }

    pub fn object_mut(&mut self, id: u32) -> Option<&mut IgObject> {
        self.objects.get_mut(&id)
    }

    /// All memory blocks, in id order.
    pub fn memory_blocks(&self) -> impl Iterator<Item = &MemoryBlock> {
        self.memory.values()
    }

    pub fn memory(&self, id: u32) -> Option<&MemoryBlock> {
        self.memory.get(&id)
    }

    pub fn memory_mut(&mut self, id: u32) -> Option<&mut MemoryBlock> {
        self.memory.get_mut(&id)
    }

    /// Changes the element count of memory block `id`. New bytes are `fill`.
    ///
    /// Blocks with the same count and type share one directory entry, so the
    /// block moves to the first entry matching its new values, or to a new entry
    /// appended at the end. Entries no id uses any more are then dropped.
    pub fn resize_memory(&mut self, id: u32, count: u32, fill: u8) -> Result<(), IgbError> {
        let bad_block = IgbError::BadIndex {
            what: "memory block",
            index: id as usize,
        };
        let block = self.memory.get(&id).ok_or(bad_block.clone())?;
        let size = (count as usize)
            .checked_mul(block.element_type.size())
            .ok_or(IgbError::Malformed("memory block size"))?;
        let slot = *self.index.get(id as usize).ok_or(bad_block)?;
        let mut values = self
            .directory
            .get(slot as usize)
            .ok_or(IgbError::BadIndex {
                what: "directory",
                index: slot as usize,
            })?
            .values
            .clone();
        *values
            .get_mut(1)
            .ok_or(IgbError::Malformed("memory directory entry"))? = count;
        let entry = DirEntry {
            class: MEMORY_DIR_ENTRY.to_owned(),
            values,
        };

        let dir_idx = match self.directory.iter().position(|e| *e == entry) {
            Some(i) => i,
            None => {
                self.directory.push(entry);
                self.directory.len() - 1
            }
        };
        let Ok(dir_idx) = u16::try_from(dir_idx) else {
            self.directory.pop();
            return Err(IgbError::Malformed("directory size"));
        };
        self.index[id as usize] = dir_idx;
        if let Some(block) = self.memory.get_mut(&id) {
            block.data.resize(size, fill);
            block.count = count;
        }
        self.drop_unused_directory_entries();
        Ok(())
    }

    /// Removes the directory entries no id points to, keeping the order of the
    /// others.
    fn drop_unused_directory_entries(&mut self) {
        let mut used = vec![false; self.directory.len()];
        for &i in &self.index {
            if let Some(u) = used.get_mut(i as usize) {
                *u = true;
            }
        }
        if used.iter().all(|&u| u) {
            return;
        }
        let mut remap = vec![0u16; used.len()];
        let mut next = 0;
        for (i, &u) in used.iter().enumerate() {
            if u {
                remap[i] = next;
                next += 1;
            }
        }
        let mut used = used.into_iter();
        self.directory.retain(|_| used.next().unwrap_or(true));
        for slot in &mut self.index {
            *slot = remap.get(*slot as usize).copied().unwrap_or(*slot);
        }
    }

    /// The ids held by a list object: field 0 is the count, field 2 the memory
    /// block of object refs. A "no reference" block gives an empty list.
    pub fn deref_list(&self, list: &IgObject) -> Result<Vec<u32>, IgbError> {
        let malformed = IgbError::Malformed("list object");
        let count = match list.field(0) {
            Some(FieldValue::Int(n)) => usize::try_from(*n).map_err(|_| malformed.clone())?,
            _ => return Err(malformed),
        };
        let mem_id = match list.field(2) {
            Some(FieldValue::MemRef(id)) => *id,
            _ => return Err(malformed),
        };
        if mem_id >= 0x8000_0000 {
            return Ok(Vec::new());
        }
        let block = self.memory(mem_id).ok_or(IgbError::BadIndex {
            what: "memory block",
            index: mem_id as usize,
        })?;
        (0..count)
            .map(|i| {
                block
                    .u32_at(i)
                    .ok_or(IgbError::Malformed("list memory block"))
            })
            .collect()
    }
}

fn read_object(
    r: &mut Reader,
    id: u32,
    classes: &[ClassDef],
    meta_fields: &[String],
) -> Result<IgObject, IgbError> {
    let start = r.pos;
    let (class_idx, size) = (r.len32()?, r.len32()?);
    let class = classes.get(class_idx).ok_or(IgbError::BadIndex {
        what: "class",
        index: class_idx,
    })?;
    let mut fields = Vec::new();
    for &(type_idx, _, _) in &class.fields {
        let type_name = meta_fields
            .get(type_idx as usize)
            .ok_or(IgbError::BadIndex {
                what: "meta field",
                index: type_idx as usize,
            })?;
        let value =
            FieldValue::read(type_name, r)?.ok_or_else(|| IgbError::UnsupportedFieldType {
                class: class.name.clone(),
                field_type: type_name.clone(),
            })?;
        fields.push(value);
    }
    check_size("object", size, r.pos - start)?;
    Ok(IgObject {
        id,
        class: class.name.clone(),
        fields,
    })
}
