//! A small, dependency-free binary codec. Little-endian, varint lengths, no schema evolution
//! magic: append new fields at the end of messages and bump your protocol version.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "decode error: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

pub type Result<T> = std::result::Result<T, DecodeError>;

#[derive(Default, Debug, Clone)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }

    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(v as u8)
    }

    /// Unsigned LEB128.
    pub fn var(&mut self, mut v: u64) -> &mut Self {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return self;
            }
            self.buf.push(b | 0x80);
        }
    }

    /// Zig-zag signed varint.
    pub fn ivar(&mut self, v: i64) -> &mut Self {
        self.var(((v << 1) ^ (v >> 63)) as u64)
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.var(v.len() as u64);
        self.buf.extend_from_slice(v);
        self
    }

    pub fn str(&mut self, v: &str) -> &mut Self {
        self.bytes(v.as_bytes())
    }

    pub fn put<T: Wire>(&mut self, v: &T) -> &mut Self {
        v.encode(self);
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn u8(&mut self) -> Result<u8> {
        let b = *self.buf.get(self.pos).ok_or(DecodeError("unexpected end"))?;
        self.pos += 1;
        Ok(b)
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub fn var(&mut self) -> Result<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(DecodeError("varint too long"))
    }

    pub fn ivar(&mut self) -> Result<i64> {
        let v = self.var()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(DecodeError("unexpected end"));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.var()? as usize;
        if n > 16 * 1024 * 1024 {
            return Err(DecodeError("length too large"));
        }
        self.take(n)
    }

    pub fn str(&mut self) -> Result<String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| DecodeError("invalid utf-8"))
    }

    pub fn get<T: Wire>(&mut self) -> Result<T> {
        T::decode(self)
    }
}

/// Types that go over the wire.
pub trait Wire: Sized {
    fn encode(&self, w: &mut Writer);
    fn decode(r: &mut Reader) -> Result<Self>;

    fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        self.encode(&mut w);
        w.finish()
    }

    fn from_bytes(b: &[u8]) -> Result<Self> {
        Self::decode(&mut Reader::new(b))
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn encode(&self, w: &mut Writer) {
        w.var(self.len() as u64);
        for v in self {
            v.encode(w);
        }
    }

    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.var()? as usize;
        if n > r.remaining() {
            return Err(DecodeError("list too long"));
        }
        (0..n).map(|_| T::decode(r)).collect()
    }
}

impl Wire for u32 {
    fn encode(&self, w: &mut Writer) {
        w.var(*self as u64);
    }
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(r.var()? as u32)
    }
}

impl Wire for String {
    fn encode(&self, w: &mut Writer) {
        w.str(self);
    }
    fn decode(r: &mut Reader) -> Result<Self> {
        r.str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_primitives() {
        let mut w = Writer::new();
        w.u8(7).var(300).ivar(-5).ivar(1 << 40).u32(0xdead_beef).f32(1.5).str("héllo").bool(true);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u8().unwrap(), 7);
        assert_eq!(r.var().unwrap(), 300);
        assert_eq!(r.ivar().unwrap(), -5);
        assert_eq!(r.ivar().unwrap(), 1 << 40);
        assert_eq!(r.u32().unwrap(), 0xdead_beef);
        assert_eq!(r.f32().unwrap(), 1.5);
        assert_eq!(r.str().unwrap(), "héllo");
        assert!(r.bool().unwrap());
        assert!(r.u8().is_err());
        let v = vec![1u32, 2, 300];
        assert_eq!(Vec::<u32>::from_bytes(&v.to_bytes()).unwrap(), v);
    }
}
