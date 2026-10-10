use crate::memory::ElementType;
use crate::object::FieldValue;
use crate::{IgbError, IgbFile};

impl IgbFile {
    /// Index of the first string equal to `text`, if any.
    pub fn find_string(&self, text: &[u8]) -> Option<u32> {
        let i = self
            .strings
            .iter()
            .position(|s| s.as_deref() == Some(text))?;
        u32::try_from(i).ok()
    }

    /// Index of `text` in the string table, inserting it when it is missing.
    ///
    /// The table is kept byte-sorted after the null entry, as the game writes it,
    /// so an insertion renumbers every string reference: `igString` fields,
    /// string memory blocks and the string half of `igName` elements. The keys
    /// of the hash tables are string blocks, so they follow.
    pub fn string_index(&mut self, text: &[u8]) -> Result<u32, IgbError> {
        if let Some(i) = self.find_string(text) {
            return Ok(i);
        }
        if text.contains(&0) {
            return Err(IgbError::InvalidString);
        }
        let Some((None, rest)) = self.strings.split_first() else {
            return Err(IgbError::Malformed("string table"));
        };
        let mut rest: Vec<&[u8]> = rest
            .iter()
            .map(|s| s.as_deref())
            .collect::<Option<_>>()
            .ok_or(IgbError::Malformed("string table"))?;
        let added = rest.len();
        rest.push(text);

        // Stable sort of the positions, so equal strings keep their order and each
        // old index moves to the first copy of its string.
        let mut order: Vec<usize> = (0..rest.len()).collect();
        order.sort_by(|&a, &b| rest[a].cmp(rest[b]));
        let mut remap = vec![0u32; rest.len() + 1];
        let mut run_start = 0;
        for (pos, &i) in order.iter().enumerate() {
            if pos > 0 && rest[order[pos - 1]] != rest[i] {
                run_start = pos;
            }
            remap[i + 1] = u32::try_from(run_start + 1)
                .ok()
                .filter(|&n| n < 0x8000_0000)
                .ok_or(IgbError::Malformed("string table size"))?;
        }

        let mut strings = Vec::with_capacity(order.len() + 1);
        strings.push(None);
        strings.extend(order.iter().map(|&i| Some(rest[i].to_vec())));
        // The last position is the new string; the others are the old indices,
        // with the null entry staying at 0.
        let new_index = remap[added + 1];
        remap.truncate(added + 1);
        self.remap_strings(&remap);
        self.strings = strings;
        Ok(new_index)
    }

    /// Renumbers every string reference with `remap[old] = new`. Values outside
    /// the remap ("no reference" sentinels) are left as they are, and so are the
    /// zero elements of memory blocks.
    fn remap_strings(&mut self, remap: &[u32]) {
        let fix = |idx: u32| remap.get(idx as usize).copied().unwrap_or(idx);
        for object in self.objects.values_mut() {
            for field in &mut object.fields {
                if let FieldValue::Str(idx) = field {
                    *idx = fix(*idx);
                }
            }
        }
        for block in self.memory.values_mut() {
            match block.element_type {
                ElementType::Str => {
                    for i in 0..block.count() {
                        if let Some(idx @ 1..) = block.u32_at(i) {
                            block.set_u32(i, fix(idx));
                        }
                    }
                }
                ElementType::Name => {
                    for i in 0..block.count() {
                        if let Some((idx @ 1.., hash)) = block.name_at(i) {
                            block.set_name(i, (fix(idx), hash));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_to_a_duplicate_move_to_the_first_copy() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../saves/initial_game.sav");
        let mut file = IgbFile::parse(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(file.strings[1].as_deref(), Some(b"---".as_slice()));
        // a second "---" right after the first, referenced by one string field
        file.strings.insert(2, Some(b"---".to_vec()));
        let (id, i) = file
            .objects
            .values()
            .find_map(|o| {
                let i = o
                    .fields
                    .iter()
                    .position(|f| matches!(f, FieldValue::Str(_)))?;
                Some((o.id, i))
            })
            .unwrap();
        file.objects.get_mut(&id).unwrap().fields[i] = FieldValue::Str(2);

        assert_eq!(file.string_index(b"!a").unwrap(), 1);
        assert_eq!(file.find_string(b"---"), Some(2));
        assert_eq!(file.strings[3].as_deref(), Some(b"---".as_slice()));
        assert_eq!(file.objects[&id].fields[i], FieldValue::Str(2));
    }
}
