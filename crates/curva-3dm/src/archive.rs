//! The binary archive a `.3dm` file is made of: a 32-byte start, then nested chunks, each a type
//! code and a length (or a value, for short chunks), with a CRC at the end of most. Numbers are
//! little-endian. Everything read is checked against what is left, so a damaged file gives an
//! error, never a crash.

use std::fmt;

/// Why a file could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// A result of reading or writing.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad(what: impl Into<String>) -> Error {
    Error(what.into())
}

/// Type codes of the chunks Curva reads and writes (openNURBS `opennurbs_3dm.h`).
#[allow(missing_docs)]
pub mod tcode {
    pub const SHORT: u32 = 0x8000_0000;
    pub const CRC: u32 = 0x8000;
    pub const TABLE: u32 = 0x1000_0000;
    pub const TABLEREC: u32 = 0x2000_0000;
    pub const USER: u32 = 0x4000_0000;
    pub const INTERFACE: u32 = 0x0200_0000;
    pub const OPENNURBS_OBJECT: u32 = 0x0002_0000;

    pub const COMMENTBLOCK: u32 = 0x0000_0001;
    pub const ENDOFFILE: u32 = 0x0000_7FFF;
    pub const ENDOFTABLE: u32 = 0xFFFF_FFFF;

    pub const ANONYMOUS_CHUNK: u32 = USER | CRC;
    pub const UTF8_STRING_CHUNK: u32 = USER | CRC | 0x0001;
    pub const MODEL_ATTRIBUTES_CHUNK: u32 = USER | CRC | 0x0002;

    pub const MATERIAL_TABLE: u32 = TABLE | 0x0010;
    pub const LAYER_TABLE: u32 = TABLE | 0x0011;
    pub const LIGHT_TABLE: u32 = TABLE | 0x0012;
    pub const OBJECT_TABLE: u32 = TABLE | 0x0013;
    pub const PROPERTIES_TABLE: u32 = TABLE | 0x0014;
    pub const SETTINGS_TABLE: u32 = TABLE | 0x0015;
    pub const BITMAP_TABLE: u32 = TABLE | 0x0016;
    pub const USER_TABLE: u32 = TABLE | 0x0017;
    pub const GROUP_TABLE: u32 = TABLE | 0x0018;
    pub const FONT_TABLE: u32 = TABLE | 0x0019;
    pub const DIMSTYLE_TABLE: u32 = TABLE | 0x0020;
    pub const INSTANCE_DEFINITION_TABLE: u32 = TABLE | 0x0021;
    pub const HATCHPATTERN_TABLE: u32 = TABLE | 0x0022;
    pub const LINETYPE_TABLE: u32 = TABLE | 0x0023;
    pub const TEXTURE_MAPPING_TABLE: u32 = TABLE | 0x0025;
    pub const HISTORYRECORD_TABLE: u32 = TABLE | 0x0026;

    pub const PROPERTIES_REVISIONHISTORY: u32 = TABLEREC | CRC | 0x0021;
    pub const PROPERTIES_NOTES: u32 = TABLEREC | CRC | 0x0022;
    pub const PROPERTIES_APPLICATION: u32 = TABLEREC | CRC | 0x0024;
    pub const PROPERTIES_OPENNURBS_VERSION: u32 = TABLEREC | SHORT | 0x0026;
    pub const PROPERTIES_AS_FILE_NAME: u32 = TABLEREC | CRC | 0x0027;

    pub const SETTINGS_UNITSANDTOLS: u32 = TABLEREC | CRC | 0x0031;
    pub const SETTINGS_NAMED_CPLANE_LIST: u32 = TABLEREC | CRC | 0x0035;
    pub const SETTINGS_NAMED_VIEW_LIST: u32 = TABLEREC | CRC | 0x0036;
    pub const SETTINGS_CURRENT_LAYER_INDEX: u32 = TABLEREC | SHORT | 0x0038;

    pub const LAYER_RECORD: u32 = TABLEREC | CRC | 0x0050;
    pub const GROUP_RECORD: u32 = TABLEREC | CRC | 0x0073;
    pub const INSTANCE_DEFINITION_RECORD: u32 = TABLEREC | CRC | 0x0076;
    pub const LINETYPE_RECORD: u32 = TABLEREC | CRC | 0x0078;

    pub const OBJECT_RECORD: u32 = TABLEREC | CRC | 0x0070;
    pub const OBJECT_RECORD_TYPE: u32 = INTERFACE | SHORT | 0x0071;
    pub const OBJECT_RECORD_ATTRIBUTES: u32 = INTERFACE | CRC | 0x0072;
    pub const OBJECT_RECORD_ATTRIBUTES_USERDATA: u32 = INTERFACE | 0x0073;
    pub const OBJECT_RECORD_END: u32 = INTERFACE | SHORT | 0x007F;

    pub const OPENNURBS_CLASS: u32 = OPENNURBS_OBJECT | 0x7FFA;
    pub const OPENNURBS_CLASS_UUID: u32 = OPENNURBS_OBJECT | CRC | 0x7FFB;
    pub const OPENNURBS_CLASS_DATA: u32 = OPENNURBS_OBJECT | CRC | 0x7FFC;
    pub const OPENNURBS_CLASS_USERDATA: u32 = OPENNURBS_OBJECT | 0x7FFD;
    pub const OPENNURBS_CLASS_USERDATA_HEADER: u32 = OPENNURBS_OBJECT | CRC | 0x7FF9;
    pub const OPENNURBS_CLASS_END: u32 = OPENNURBS_OBJECT | SHORT | 0x7FFF;
}

/// A 16-byte identifier, kept as its bytes in file order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid(pub [u8; 16]);

impl Uuid {
    /// The nil id.
    pub const NIL: Self = Self([0; 16]);

    /// Parses the usual text form, `8-4-4-4-12` hexadecimal digits, as openNURBS writes ids in
    /// its source; the first three groups are stored little-endian, as Windows GUIDs are.
    #[must_use]
    pub const fn parse(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut digits = [0u8; 32];
        let mut count = 0;
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            let value = match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            if let Some(v) = value {
                if count < 32 {
                    digits[count] = v;
                }
                count += 1;
            }
            i += 1;
        }
        let mut plain = [0u8; 16];
        let mut k = 0;
        while k < 16 {
            plain[k] = digits[2 * k] * 16 + digits[2 * k + 1];
            k += 1;
        }
        Self([
            plain[3], plain[2], plain[1], plain[0], plain[5], plain[4], plain[7], plain[6],
            plain[8], plain[9], plain[10], plain[11], plain[12], plain[13], plain[14], plain[15],
        ])
    }

    /// A fresh id from a counter and the clock, unique enough within and between files.
    #[must_use]
    pub fn fresh(seed: u64) -> Self {
        use std::hash::{BuildHasher, Hasher};
        let mut bytes = [0u8; 16];
        for (half, chunk) in bytes.chunks_mut(8).enumerate() {
            let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
            hasher.write_u64(seed);
            hasher.write_usize(half);
            chunk.copy_from_slice(&hasher.finish().to_le_bytes());
        }
        // Version 4 (random), variant 1.
        bytes[7] = (bytes[7] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        Self(bytes)
    }
}

/// A chunk read from the file.
#[derive(Clone, Debug)]
pub struct Chunk<'a> {
    /// Its type code.
    pub tcode: u32,
    /// For a short chunk, its value; for a long one, its length.
    pub value: i64,
    /// What is inside a long chunk, without its CRC; empty for a short one.
    pub body: Reader<'a>,
}

impl Chunk<'_> {
    /// Whether it is a short chunk, which holds only its value.
    #[must_use]
    pub fn is_short(&self) -> bool {
        self.tcode & tcode::SHORT != 0 || self.value < 0
    }
}

/// Reads from part of a file.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    /// The file's archive version: 1 to 5, or 50, 60, 70, 80.
    pub version: u32,
}

impl<'a> Reader<'a> {
    /// Reads `data` as written in archive `version`.
    #[must_use]
    pub fn new(data: &'a [u8], version: u32) -> Self {
        Self {
            data,
            at: 0,
            version,
        }
    }

    /// How many bytes are left.
    #[must_use]
    pub fn left(&self) -> usize {
        self.data.len() - self.at
    }

    /// Whether everything has been read.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.left() == 0
    }

    /// The next `count` bytes.
    ///
    /// # Errors
    /// If fewer are left.
    pub fn bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        if count > self.left() {
            return Err(bad("the file ends too soon"));
        }
        let out = &self.data[self.at..self.at + count];
        self.at += count;
        Ok(out)
    }

    /// Skips `count` bytes.
    ///
    /// # Errors
    /// If fewer are left.
    pub fn skip(&mut self, count: usize) -> Result<()> {
        self.bytes(count).map(|_| ())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    /// One byte.
    ///
    /// # Errors
    /// At the end.
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    /// A yes or no, stored as a byte.
    ///
    /// # Errors
    /// At the end.
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    /// A 16-bit number.
    ///
    /// # Errors
    /// At the end.
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    /// A 32-bit number.
    ///
    /// # Errors
    /// At the end.
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// A signed 32-bit number.
    ///
    /// # Errors
    /// At the end.
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    /// A signed 64-bit number.
    ///
    /// # Errors
    /// At the end.
    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    /// A double.
    ///
    /// # Errors
    /// At the end.
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    /// A float.
    ///
    /// # Errors
    /// At the end.
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    /// `N` doubles.
    ///
    /// # Errors
    /// At the end.
    pub fn f64s<const N: usize>(&mut self) -> Result<[f64; N]> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f64()?;
        }
        Ok(out)
    }

    /// A count of things each at least `each` bytes long, checked against what is left.
    ///
    /// # Errors
    /// If it is negative or could not fit.
    pub fn count(&mut self, each: usize) -> Result<usize> {
        let n = self.i32()?;
        let n = usize::try_from(n).map_err(|_| bad("a negative count"))?;
        if n.saturating_mul(each.max(1)) > self.left() {
            return Err(bad("a count larger than the data"));
        }
        Ok(n)
    }

    /// An id.
    ///
    /// # Errors
    /// At the end.
    pub fn uuid(&mut self) -> Result<Uuid> {
        Ok(Uuid(self.array()?))
    }

    /// A colour, as red, green, blue and alpha bytes (alpha 0 is opaque in openNURBS).
    ///
    /// # Errors
    /// At the end.
    pub fn color(&mut self) -> Result<[u8; 4]> {
        self.array()
    }

    /// A wide string: a count of UTF-16 units with the closing nul, then the units.
    ///
    /// # Errors
    /// If it runs past the end.
    pub fn wstring(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        if n.saturating_mul(2) > self.left() {
            return Err(bad("a string longer than the data"));
        }
        let mut units = Vec::with_capacity(n);
        for _ in 0..n {
            units.push(self.u16()?);
        }
        while units.last() == Some(&0) {
            units.pop();
        }
        Ok(String::from_utf16_lossy(&units))
    }

    /// A narrow string: a count of bytes with the closing nul, then the bytes, as UTF-8.
    ///
    /// # Errors
    /// If it runs past the end.
    pub fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        let bytes = self.bytes(n)?;
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }

    /// The next chunk, whole; the reader moves past it.
    ///
    /// # Errors
    /// If the chunk runs past the end or its CRC is wrong.
    pub fn chunk(&mut self) -> Result<Chunk<'a>> {
        let code = self.u32()?;
        let value = if self.version >= 50 {
            self.i64()?
        } else if code & tcode::SHORT != 0 || code == tcode::ENDOFTABLE {
            i64::from(self.u32()?)
        } else {
            i64::from(self.i32()?)
        };
        let short = code & tcode::SHORT != 0 || value < 0 || (code == 0 && self.version == 1);
        if short {
            return Ok(Chunk {
                tcode: code,
                value,
                body: Reader::new(&[], self.version),
            });
        }
        let length = usize::try_from(value).map_err(|_| bad("a chunk too long"))?;
        let all = self.bytes(length)?;
        let crc = code & tcode::CRC != 0 && self.version != 1;
        // A chunk's CRC covers only the bytes written directly in it, not the chunks nested in
        // it, so it cannot be checked without reading those; the bounds checks guard instead.
        let body = if crc && length >= 4 {
            &all[..length - 4]
        } else {
            all
        };
        Ok(Chunk {
            tcode: code,
            value,
            body: Reader::new(body, self.version),
        })
    }

    /// The next chunk, which must be of type `code`.
    ///
    /// # Errors
    /// If it is another type, or damaged.
    pub fn expect(&mut self, code: u32) -> Result<Chunk<'a>> {
        let chunk = self.chunk()?;
        if chunk.tcode != code {
            return Err(bad(format!(
                "expected a chunk of type {code:#x}, found {:#x}",
                chunk.tcode
            )));
        }
        Ok(chunk)
    }

    /// A chunk with a major and minor version at its start, as openNURBS'
    /// `BeginRead3dmChunk(tcode, &major, &minor)` reads it.
    ///
    /// # Errors
    /// If it is another type, or damaged.
    pub fn versioned(&mut self, code: u32) -> Result<(i32, i32, Reader<'a>)> {
        let mut chunk = self.expect(code)?;
        let major = chunk.body.i32()?;
        let minor = chunk.body.i32()?;
        Ok((major, minor, chunk.body))
    }

    /// An anonymous chunk with its version (major 1), as `BeginRead3dmAnonymousChunk` reads it.
    ///
    /// # Errors
    /// If it is another type, or damaged.
    pub fn anonymous(&mut self) -> Result<(i32, Reader<'a>)> {
        let (_, minor, body) = self.versioned(tcode::ANONYMOUS_CHUNK)?;
        Ok((minor, body))
    }

    /// A version byte as openNURBS writes it with `Write3dmChunkVersion`: major in the high
    /// nibble, minor in the low one.
    ///
    /// # Errors
    /// At the end.
    pub fn chunk_version(&mut self) -> Result<(u8, u8)> {
        let v = self.u8()?;
        Ok((v >> 4, v & 0x0F))
    }

    /// The type code of the next chunk, without reading it.
    #[must_use]
    pub fn peek_tcode(&self) -> Option<u32> {
        let b = self.data.get(self.at..self.at + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A zlib-compressed buffer as openNURBS writes it (`WriteCompressedBuffer`): its size,
    /// its CRC, and then the bytes, stored or deflated.
    ///
    /// # Errors
    /// If it is damaged.
    pub fn compressed(&mut self) -> Result<Vec<u8>> {
        let size = self.u64_size()?;
        if size == 0 {
            return Ok(Vec::new());
        }
        let _crc = self.u32()?;
        let method = self.u8()?;
        let size = usize::try_from(size).map_err(|_| bad("a buffer too large"))?;
        match method {
            0 => Ok(self.bytes(size)?.to_vec()),
            1 => {
                let mut chunk = self.chunk()?;
                let packed = chunk.body.bytes(chunk.body.left())?;
                let mut out = Vec::new();
                use std::io::Read;
                flate2::read::ZlibDecoder::new(packed)
                    .take(size as u64)
                    .read_to_end(&mut out)
                    .map_err(|e| bad(format!("a damaged compressed buffer: {e}")))?;
                if out.len() != size {
                    return Err(bad("a compressed buffer of the wrong size"));
                }
                Ok(out)
            }
            _ => Err(bad("an unknown compression")),
        }
    }

    /// A size, 4 bytes before archive version 50 and 8 after.
    ///
    /// # Errors
    /// At the end.
    pub fn u64_size(&mut self) -> Result<u64> {
        if self.version >= 50 {
            u64::try_from(self.i64()?).map_err(|_| bad("a negative size"))
        } else {
            Ok(u64::from(self.u32()?))
        }
    }
}

/// A chunk being written: its type, where its length goes, and the spans of the chunks nested
/// in it.
type Open = (u32, usize, Vec<(usize, usize)>);

/// Writes an archive.
#[derive(Debug)]
pub struct Writer {
    /// What is written so far.
    pub out: Vec<u8>,
    /// Each open chunk: its type, where its length goes, and the spans of the chunks nested in
    /// it, which its CRC leaves out.
    open: Vec<Open>,
    /// The archive version written: always 70 (Rhino 7).
    pub version: u32,
}

impl Default for Writer {
    fn default() -> Self {
        Self {
            out: Vec::new(),
            open: Vec::new(),
            version: 70,
        }
    }
}

#[allow(missing_docs)]
impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.out.push(u8::from(v));
    }
    pub fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64s(&mut self, vs: &[f64]) {
        for v in vs {
            self.f64(*v);
        }
    }
    pub fn uuid(&mut self, id: Uuid) {
        self.out.extend_from_slice(&id.0);
    }
    pub fn color(&mut self, c: [u8; 4]) {
        self.out.extend_from_slice(&c);
    }
    /// A count, written as a signed 32-bit number.
    pub fn count(&mut self, n: usize) {
        self.i32(i32::try_from(n).unwrap_or(i32::MAX));
    }

    /// A wide string, as UTF-16 with its closing nul; an empty string is a zero count.
    pub fn wstring(&mut self, s: &str) {
        let units: Vec<u16> = s.encode_utf16().collect();
        if units.is_empty() {
            self.u32(0);
            return;
        }
        self.u32(u32::try_from(units.len() + 1).unwrap_or(u32::MAX));
        for u in units {
            self.u16(u);
        }
        self.u16(0);
    }

    /// A narrow UTF-8 string with its closing nul; an empty string is a zero count.
    pub fn string(&mut self, s: &str) {
        if s.is_empty() {
            self.u32(0);
            return;
        }
        self.u32(u32::try_from(s.len() + 1).unwrap_or(u32::MAX));
        self.out.extend_from_slice(s.as_bytes());
        self.u8(0);
    }

    /// A version byte: major in the high nibble, minor in the low one.
    pub fn chunk_version(&mut self, major: u8, minor: u8) {
        self.u8((major << 4) | (minor & 0x0F));
    }

    /// Starts a long chunk; [`Self::end`] closes it.
    pub fn begin(&mut self, code: u32) {
        self.u32(code);
        let at = self.out.len();
        self.i64(0);
        self.open.push((code, at, Vec::new()));
    }

    /// Starts a long chunk with a major and minor version.
    pub fn begin_versioned(&mut self, code: u32, major: i32, minor: i32) {
        self.begin(code);
        self.i32(major);
        self.i32(minor);
    }

    /// Starts an anonymous chunk with its version.
    pub fn begin_anonymous(&mut self, minor: i32) {
        self.begin_versioned(tcode::ANONYMOUS_CHUNK, 1, minor);
    }

    /// Closes the chunk last started: adds its CRC if its type has one, and fills in its length.
    pub fn end(&mut self) {
        let Some((code, at, nested)) = self.open.pop() else {
            return;
        };
        let start = at + 8;
        if code & tcode::CRC != 0 {
            let mut hasher = crc32fast::Hasher::new();
            let mut from = start;
            for (a, b) in nested {
                hasher.update(&self.out[from..a]);
                from = b;
            }
            hasher.update(&self.out[from..]);
            self.u32(hasher.finalize());
        }
        let length = (self.out.len() - start) as i64;
        self.out[at..at + 8].copy_from_slice(&length.to_le_bytes());
        if let Some(parent) = self.open.last_mut() {
            parent.2.push((at - 4, self.out.len()));
        }
    }

    /// A short chunk, which holds only `value`.
    pub fn short(&mut self, code: u32, value: i64) {
        let from = self.out.len();
        self.u32(code);
        self.i64(value);
        if let Some(parent) = self.open.last_mut() {
            parent.2.push((from, self.out.len()));
        }
    }

    /// A buffer as `WriteCompressedBuffer` writes it, deflated.
    pub fn compressed(&mut self, data: &[u8]) {
        self.i64(data.len() as i64);
        if data.is_empty() {
            return;
        }
        self.u32(crc32fast::hash(data));
        self.u8(1);
        use std::io::Write;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let _ = encoder.write_all(data);
        let packed = encoder.finish().unwrap_or_default();
        self.begin(tcode::ANONYMOUS_CHUNK);
        self.out.extend_from_slice(&packed);
        self.end();
    }
}
