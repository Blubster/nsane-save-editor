use std::collections::HashSet;

use crate::memory::ElementType;
use crate::object::FieldValue;
use crate::{IgbError, IgbFile, fnv1a};

/// Largest capacity a table may grow to. Real tables hold 64 or 128 slots; this
/// only stops a malformed load factor from doubling a table without end.
const MAX_CAPACITY: u32 = 1 << 24;

/// The fields of an `igStringBoolHashTable` / `igStringIntHashTable` object:
/// 0 = values block, 1 = keys block, 2 = count, 3 = a bool, 4 = load factor.
#[derive(Clone, Copy)]
struct Layout {
    values: u32,
    keys: u32,
    count: i32,
    load_factor: f32,
    capacity: usize,
    value_type: ElementType,
}

fn malformed() -> IgbError {
    IgbError::Malformed("hash table")
}

fn layout(file: &IgbFile, id: u32) -> Result<Layout, IgbError> {
    let object = file.object(id).ok_or(IgbError::BadIndex {
        what: "object",
        index: id as usize,
    })?;
    let (
        Some(&FieldValue::MemRef(values)),
        Some(&FieldValue::MemRef(keys)),
        Some(&FieldValue::Int(count)),
        Some(&FieldValue::Float(load_factor)),
    ) = (
        object.field(0),
        object.field(1),
        object.field(2),
        object.field(4),
    )
    else {
        return Err(malformed());
    };
    let (Some(k), Some(v)) = (file.memory(keys), file.memory(values)) else {
        return Err(malformed());
    };
    if k.element_type != ElementType::Str
        || !matches!(v.element_type, ElementType::Bool | ElementType::Int)
        || k.count() != v.count()
        || keys == values
    {
        return Err(malformed());
    }
    Ok(Layout {
        values,
        keys,
        count,
        load_factor: f32::from_bits(load_factor),
        capacity: k.count(),
        value_type: v.element_type,
    })
}

/// `(slot, found)`: the slot holding `key`, or the empty slot where it would go.
///
/// The slot is FNV-1a-32 of the key bytes (case-sensitive) modulo the capacity,
/// then linear probing. Keys are compared by text, never by string index.
fn find_slot(file: &IgbFile, t: &Layout, key: &[u8]) -> Result<(usize, bool), IgbError> {
    let keys = file.memory(t.keys).ok_or_else(malformed)?;
    if t.capacity == 0 {
        return Err(IgbError::HashTableFull);
    }
    let mut slot = fnv1a(key) as usize % t.capacity;
    for _ in 0..t.capacity {
        match keys.u32_at(slot).ok_or_else(malformed)? {
            0 => return Ok((slot, false)),
            idx if file.string(idx) == Some(key) => return Ok((slot, true)),
            _ => slot = (slot + 1) % t.capacity,
        }
    }
    Err(IgbError::HashTableFull)
}

fn value_at(file: &IgbFile, t: &Layout, slot: usize) -> Option<u32> {
    let values = file.memory(t.values)?;
    match t.value_type {
        ElementType::Bool => values.bool_at(slot).map(u32::from),
        _ => values.u32_at(slot),
    }
}

fn items(file: &IgbFile, t: &Layout) -> Result<Vec<(Vec<u8>, u32)>, IgbError> {
    let keys = file.memory(t.keys).ok_or_else(malformed)?;
    let mut out = Vec::new();
    for slot in 0..t.capacity {
        let idx = keys.u32_at(slot).ok_or_else(malformed)?;
        if idx == 0 {
            continue;
        }
        let key = file
            .string(idx)
            .ok_or(IgbError::Malformed("hash table key"))?;
        let value = value_at(file, t, slot).ok_or_else(malformed)?;
        out.push((key.to_vec(), value));
    }
    Ok(out)
}

/// A read-only view of a string-keyed hash table (`igStringBoolHashTable` or
/// `igStringIntHashTable`), from [`IgbFile::hash_table`].
#[derive(Clone, Copy)]
pub struct HashTable<'a> {
    file: &'a IgbFile,
    layout: Layout,
}

impl HashTable<'_> {
    /// The value of `key`. Bool tables give 0 or 1 (the raw byte).
    pub fn get(&self, key: &[u8]) -> Option<u32> {
        match find_slot(self.file, &self.layout, key) {
            Ok((slot, true)) => value_at(self.file, &self.layout, slot),
            _ => None,
        }
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// `(key, value)` pairs in slot order.
    pub fn items(&self) -> Result<Vec<(Vec<u8>, u32)>, IgbError> {
        items(self.file, &self.layout)
    }

    /// Number of slots.
    pub fn capacity(&self) -> usize {
        self.layout.capacity
    }

    /// The count field: number of keys.
    pub fn len(&self) -> usize {
        usize::try_from(self.layout.count).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A hash table being edited, from [`IgbFile::hash_table_mut`].
///
/// Inserting a key can insert a string (renumbering every string reference)
/// and grow the table (resizing its two memory blocks, which changes the
/// directory). Any error leaves the file in an unspecified state: edit a copy
/// when it has to survive a failure.
pub struct HashTableMut<'a> {
    file: &'a mut IgbFile,
    id: u32,
}

impl HashTableMut<'_> {
    fn layout(&self) -> Result<Layout, IgbError> {
        layout(self.file, self.id)
    }

    fn view(&self) -> Result<HashTable<'_>, IgbError> {
        Ok(HashTable {
            file: self.file,
            layout: self.layout()?,
        })
    }

    pub fn get(&self, key: &[u8]) -> Option<u32> {
        self.view().ok()?.get(key)
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    pub fn items(&self) -> Result<Vec<(Vec<u8>, u32)>, IgbError> {
        self.view()?.items()
    }

    pub fn capacity(&self) -> usize {
        self.view().map_or(0, |t| t.capacity())
    }

    fn check_value(t: &Layout, value: u32) -> Result<(), IgbError> {
        if t.value_type == ElementType::Bool && value > 0xFF {
            return Err(IgbError::ValueOutOfRange(value));
        }
        Ok(())
    }

    fn set_count(&mut self, count: i32) -> Result<(), IgbError> {
        let object = self.file.object_mut(self.id).ok_or_else(malformed)?;
        *object.field_mut(2).ok_or_else(malformed)? = FieldValue::Int(count);
        Ok(())
    }

    fn write_slot(&mut self, t: &Layout, slot: usize, value: u32) -> Result<(), IgbError> {
        let values = self.file.memory_mut(t.values).ok_or_else(malformed)?;
        match t.value_type {
            // checked by check_value: a bool table value fits a byte
            ElementType::Bool => values.set_bool(slot, value as u8),
            _ => values.set_u32(slot, value),
        }
        .ok_or_else(malformed)
    }

    fn write_key(&mut self, t: &Layout, slot: usize, key: &[u8]) -> Result<(), IgbError> {
        let idx = self.file.string_index(key)?;
        let keys = self.file.memory_mut(t.keys).ok_or_else(malformed)?;
        keys.set_u32(slot, idx).ok_or_else(malformed)
    }

    /// Sets `key` to `value`, inserting the key when it is missing.
    ///
    /// A new key first grows the table to twice its capacity if the count would
    /// pass `capacity × load factor`.
    pub fn set(&mut self, key: &[u8], value: u32) -> Result<(), IgbError> {
        let mut t = self.layout()?;
        Self::check_value(&t, value)?;
        let (mut slot, found) = find_slot(self.file, &t, key)?;
        if !found {
            if key.contains(&0) {
                return Err(IgbError::InvalidString);
            }
            if f64::from(t.count) + 1.0 > t.capacity as f64 * f64::from(t.load_factor) {
                self.grow()?;
            }
            // Growing moves the keys and inserting the string renumbers them, so
            // the slot is looked up again afterwards (by text, as always).
            self.file.string_index(key)?;
            t = self.layout()?;
            slot = find_slot(self.file, &t, key)?.0;
            self.write_key(&t, slot, key)?;
            let count = t.count.checked_add(1).ok_or_else(malformed)?;
            self.set_count(count)?;
        }
        self.write_slot(&t, slot, value)
    }

    /// Removes `key`; the table is rebuilt at the same capacity from the other
    /// items in slot order. `Ok(false)` if the key was not there.
    pub fn remove(&mut self, key: &[u8]) -> Result<bool, IgbError> {
        let t = self.layout()?;
        if !find_slot(self.file, &t, key)?.1 {
            return Ok(false);
        }
        let mut rest = items(self.file, &t)?;
        rest.retain(|(k, _)| k != key);
        self.rebuild(t.capacity, &rest)?;
        Ok(true)
    }

    fn grow(&mut self) -> Result<(), IgbError> {
        let t = self.layout()?;
        let capacity = u32::try_from(t.capacity)
            .ok()
            .and_then(|c| c.checked_mul(2))
            .filter(|&c| c <= MAX_CAPACITY)
            .ok_or(IgbError::Malformed("hash table capacity"))?;
        let all = items(self.file, &t)?;
        self.rebuild(capacity as usize, &all)
    }

    /// Empties the table, resizes it to `capacity` slots and inserts `items` in
    /// order. Empty slots hold 0 keys, and 0 values (bool table) or `0xFA`
    /// bytes (int table), as the game writes them.
    ///
    /// The keys must be distinct. They are looked up or inserted in the string
    /// table, so they may come from another file.
    pub fn rebuild(&mut self, capacity: usize, items: &[(Vec<u8>, u32)]) -> Result<(), IgbError> {
        let t = self.layout()?;
        let capacity = u32::try_from(capacity)
            .ok()
            .filter(|&c| c <= MAX_CAPACITY)
            .ok_or(IgbError::Malformed("hash table capacity"))?;
        if items.len() > capacity as usize {
            return Err(IgbError::HashTableFull);
        }
        let mut seen = HashSet::new();
        for (key, value) in items {
            Self::check_value(&t, *value)?;
            if key.contains(&0) {
                return Err(IgbError::InvalidString);
            }
            if !seen.insert(key.as_slice()) {
                return Err(IgbError::Malformed("duplicate hash table key"));
            }
        }

        // Keys first, then values: each resize can append a directory entry, and
        // the order of those entries is part of the file.
        let empty_value = if t.value_type == ElementType::Int {
            0xFA
        } else {
            0
        };
        for (mem_id, fill) in [(t.keys, 0), (t.values, empty_value)] {
            self.file.resize_memory(mem_id, capacity, fill)?;
            let block = self.file.memory_mut(mem_id).ok_or_else(malformed)?;
            block.data.fill(fill);
        }
        self.set_count(0)?;

        let mut count = 0;
        for (key, value) in items {
            let t = self.layout()?;
            let slot = find_slot(self.file, &t, key)?.0;
            self.write_key(&t, slot, key)?;
            self.write_slot(&t, slot, *value)?;
            count += 1;
            self.set_count(count)?;
        }
        Ok(())
    }
}

impl IgbFile {
    /// The string-keyed hash table held by object `id`.
    pub fn hash_table(&self, id: u32) -> Result<HashTable<'_>, IgbError> {
        Ok(HashTable {
            file: self,
            layout: layout(self, id)?,
        })
    }

    /// The string-keyed hash table held by object `id`, for editing.
    pub fn hash_table_mut(&mut self, id: u32) -> Result<HashTableMut<'_>, IgbError> {
        layout(self, id)?;
        Ok(HashTableMut { file: self, id })
    }
}
