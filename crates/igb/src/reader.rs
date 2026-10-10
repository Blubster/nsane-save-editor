use crate::IgbError;

/// Bounds-checked little-endian reader. Every read returns `Err` past the end.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], IgbError> {
        let bytes = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or(IgbError::Truncated {
                offset: self.pos,
                needed: n,
            })?;
        self.pos += n;
        Ok(bytes)
    }

    pub fn u16(&mut self) -> Result<u16, IgbError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32, IgbError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Result<i32, IgbError> {
        Ok(self.u32()? as i32)
    }

    /// A u32 used as a length or count.
    pub fn len32(&mut self) -> Result<usize, IgbError> {
        Ok(self.u32()? as usize)
    }

    pub fn peek_u32(&self) -> Result<u32, IgbError> {
        Reader {
            data: self.data,
            pos: self.pos,
        }
        .u32()
    }

    /// A NUL-terminated string, without its NUL.
    pub fn cstr(&mut self) -> Result<&'a [u8], IgbError> {
        let rest = self.data.get(self.pos..).unwrap_or_default();
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(IgbError::Truncated {
                offset: self.pos,
                needed: rest.len() + 1,
            })?;
        let s = &rest[..len];
        self.pos += len + 1;
        Ok(s)
    }

    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or_default()
    }
}

/// Little-endian output buffer.
#[derive(Default)]
pub(crate) struct Writer {
    pub out: Vec<u8>,
}

impl Writer {
    pub fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// A length or count written as u32.
    pub fn len32(&mut self, n: usize) -> Result<(), IgbError> {
        let v = u32::try_from(n).map_err(|_| IgbError::Malformed("length over 4 GiB"))?;
        self.u32(v);
        Ok(())
    }

    pub fn bytes(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
    }
}
