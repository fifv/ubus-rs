use crate::{
    BlobMsg, BlobMsgPayload, BlobMsgType, MsgTable, UbusBlob, UbusBlobType, UbusError, UbusMsg,
    UbusMsgStatus,
};

use core::convert::{TryFrom, TryInto};
use core::marker::PhantomData;
use core::mem::{align_of, size_of, transmute};
use core::str;
use serde::{Deserialize, Serialize};
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::dbg;
use std::string::{String, ToString};
use std::vec::Vec;
use storage_endian::BEu32;

/**
 * `Blob` is a TLV
 *      IsExtended(1bit) + Type(7bit) + Length(24bit) + Payload
 * `BlobMsg` is Blob with additional TL field name namelen + *name
 *      IsExtended(1bit) + Type(7bit) + Length(24bit) + Name(namelen 16bit + name variable) + Payload
 *
 * They are identified by the Most-Significant-Bit of Blob Type
 *
 * `BlobMsg` is used to represents json-like structure in a binary format, its Type field is fixed,
 * while Type field of normal `Blob` can be anything, defined by `struct blob_attr_info{}` in c
 * Fortunately, in the context of ubus, Blob's Type is limited, and whats more, the payload data structure of
 * specific Type is known. I call this a UbusBlob (any better naming...?) , represented as enum in rust
 *
 * I tried to make the type as strong as possible in rust for useability, and provide a try_from() and try_into()
 * to convert rust structs into raw bytes on wire, so they can send on socket
 */


#[derive(Clone, Debug)]
pub enum Blob {
    UbusBlob(UbusBlob),
    BlogMsg(BlobMsg),
}


/**
 * `BlobIter` used to find all Blob in the bytes
 * Blob are laid one by one in bytes
 *
 * The actual convertion happens in BlobMsg::try_from and UbusBlob::from_bytes
 *
 */
pub struct BlobIter<'a> {
    data: &'a [u8],
    // _phantom: PhantomData<T>,
}
impl<'a> BlobIter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            // _phantom: PhantomData,
        }
    }
}
impl<'a> Iterator for BlobIter<'a> {
    type Item = Blob;
    fn next(&mut self) -> Option<Self::Item> {
        // dbg!(&self.data);
        if self.data.len() < BlobTag::SIZE {
            return None;
        }

        let tag = BlobTag::from_bytes(&self.data[..BlobTag::SIZE].try_into().unwrap());
        if tag.is_extended() {
            if let Ok(blob) = BlobMsg::try_from(&self.data[..]) {
                // Advance the internal pointer to the next tag
                let next_idx = tag.next_tag();
                self.data = &self.data[next_idx..];
                return Some(Blob::BlogMsg(blob));
            }
        } else {
            if let Ok(blob) = UbusBlob::try_from(&self.data[..]) {
                // Advance the internal pointer to the next tag
                let next_idx = tag.next_tag();
                self.data = &self.data[next_idx..];
                return Some(Blob::UbusBlob(blob));
            }
        }


        None
    }
}

impl<'a> core::fmt::Debug for BlobIter<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "BlobIter (data_len={})", self.data.len())
    }
}


/**
 * `BlobTag` represents struct blob_attr.id_len in blob.h
 * which is a 32 bit data, 30~24 bit represents id, 23~0 bit represents length of data next to this BlobTag + 24 bit BlobTag
 *
 * `BlobTag` is used as a helper to convert between raw bytes and meaningful data
 */
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct BlobTag(BEu32);
impl BlobTag {
    pub const SIZE: usize = size_of::<Self>();
    const ID_MASK: u32 = 0x7f;
    const ID_SHIFT: u32 = 24;
    const LEN_MASK: u32 = 0xff_ff_ff;
    const EXTENDED_BIT_MASK: u32 = 1 << 31;
    pub const ALIGNMENT: usize = align_of::<Self>();

    /**
     * * `len`: include header itself (4 bytes)
     */
    pub fn try_build(id: u32, len: usize, extended: bool) -> Result<Self, UbusError> {
        if id > Self::ID_MASK || len < Self::SIZE || len > Self::LEN_MASK as usize {
            Err(UbusError::InvalidData("Invalid TAG construction"))
        } else {
            let id = id & Self::ID_MASK;
            let len = len as u32 & Self::LEN_MASK;
            let mut val = len | (id << Self::ID_SHIFT);
            if extended {
                val |= Self::EXTENDED_BIT_MASK;
            }
            Ok(Self(val.into()))
        }
    }

    /// Create BlobTag from a byte array
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        unsafe { transmute(bytes.to_owned()) }
    }
    // Dump out bytes of MessageHeader
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        unsafe { core::mem::transmute(self) }
    }
    /// ID code of this blob, technically can be anything, should be converted to specific type in context
    pub fn blob_type(&self) -> u32 {
        u32::from((self.0 >> Self::ID_SHIFT) & Self::ID_MASK)
    }
    /// Total number of bytes this blob contains (header + data)
    pub fn size(&self) -> usize {
        u32::from(self.0 & Self::LEN_MASK) as usize
    }

    pub fn set_size(&mut self, size: usize) {
        *self = Self::try_build(self.blob_type(), size, self.is_extended()).unwrap();
    }
    /// Number of padding bytes between this blob and the next blob
    ///
    /// The padded size of an entire Blob is upper roundded to next multiply of ALIGNMENT
    /// e.g the Blob size (including header) is 13 bytes, then it is padded to 16 bytes, and this padding() will return 16-13=3
    ///
    /// The original algorithm is defined in `blob_pad_len()` in `blob.h`
    pub fn padding(&self) -> usize {
        Self::ALIGNMENT.wrapping_sub(self.size()) & (Self::ALIGNMENT - 1)
    }
    /// Number of bytes to the next tag
    pub fn next_tag(&self) -> usize {
        self.size() + self.padding()
    }
    /// Total number of bytes following the tag (extended header + data)
    pub fn inner_len(&self) -> usize {
        self.size().saturating_sub(Self::SIZE)
    }
    /// Is this an "extended" blob
    pub fn is_extended(&self) -> bool {
        (self.0 & Self::EXTENDED_BIT_MASK) != 0
    }
    /// Does this blob look valid
    pub fn is_valid(&self) -> Result<(), UbusError> {
        valid_data!(self.size() >= Self::SIZE, "Tag size smaller than tag");
        Ok(())
    }
}
impl core::fmt::Debug for BlobTag {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (id, len) = (self.blob_type(), self.size());
        let extended = if self.is_extended() { ", extended" } else { "" };
        write!(
            f,
            "BlobTag(id={:?}, len={}, is_extended={})",
            id, len, extended
        )
    }
}


/**
 * `BlobBuilder` is used to encode `Blob` from "native rust struct" to "raw bytes on wire"
 */
pub struct BlobBuilder {
    buffer: Vec<u8>,
    offset: usize,
}
impl Into<Vec<u8>> for BlobBuilder {
    fn into(self) -> Vec<u8> {
        self.buffer
    }
}

impl BlobBuilder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            offset: 0,
        }
    }

    pub fn to_bytes(self) -> Vec<u8>{
        self.into()
    }

    pub fn to_bytes_clone(&self) -> Vec<u8>{
        self.buffer.to_owned()
    }

    pub fn push_u32(&mut self, id: u32, data: u32) -> Result<(), UbusError> {
        self.push_bytes(id, &data.to_be_bytes())
    }

    pub fn push_bool(&mut self, id: u32, data: bool) -> Result<(), UbusError> {
        self.push_bytes(id, if data { &[1] } else { &[0] })
    }

    pub fn push_str(&mut self, id: u32, data: &str) -> Result<(), UbusError> {
        self.push_bytes(id, data.as_bytes().iter().chain([0u8].iter()))
    }

    pub fn push_bytes<'b>(
        &mut self,
        id: u32,
        data: impl IntoIterator<Item = &'b u8>,
    ) -> Result<(), UbusError> {
        // Collect data into a Vec<u8> first (allocates)
        let bytes: Vec<u8> = data.into_iter().copied().collect();
        let data_len = bytes.len();
        let tag_len = BlobTag::SIZE;

        // Build the tag to compute padding
        let tag = BlobTag::try_build(id, tag_len + data_len, false)?;
        let pad_len = tag.padding();
        let total_len = tag_len + data_len + pad_len;

        // Ensure the buffer is large enough
        if self.offset + total_len > self.buffer.len() {
            self.buffer.resize(self.offset + total_len, 0);
        }

        // Write tag header
        self.buffer[self.offset..self.offset + tag_len].copy_from_slice(&tag.to_bytes());

        // Write data
        self.buffer[self.offset + tag_len..self.offset + tag_len + data_len]
            .copy_from_slice(&bytes);

        // Zero padding
        self.buffer[self.offset + tag_len + data_len..self.offset + total_len].fill(0);

        // Advance offset
        self.offset += total_len;


        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.offset
    }
}
impl BlobBuilder {
    pub fn from_u32(id: u32, data: u32) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_u32(id, data)?;
        Ok(builder)
    }

    pub fn from_bool(id: u32, data: bool) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_bool(id, data)?;
        Ok(builder)
    }

    pub fn from_str(id: u32, data: &str) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_str(id, data)?;
        Ok(builder)
    }

    pub fn from_bytes<'b>(
        id: u32,
        data: impl IntoIterator<Item = &'b u8>,
    ) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_bytes(id, data)?;
        Ok(builder)
    }
}


/**
 * `BlobPayloadParser` is used to parse bytes into rust struct
 */

#[derive(Clone, Debug)]
pub struct BlobPayloadParser<'a>(&'a [u8]);

impl<'a> From<&'a [u8]> for BlobPayloadParser<'a> {
    fn from(value: &'a [u8]) -> Self {
        BlobPayloadParser(value)
    }
}
// impl<'a> From<Vec<u8>> for BlobPayloadParser<'a> {
//     fn from(value: Vec<u8>) -> Self {
//         BlobPayloadParser(value)
//     }
// }


macro_rules! payload_try_into_number {
    ( $( $ty:ty , )* ) => { $( payload_try_into_number!($ty); )* };
    ( $ty:ty ) => {
        impl<'a> TryInto<$ty> for BlobPayloadParser<'a>{
            type Error = UbusError;
            fn try_into(self) -> Result<$ty, Self::Error> {
                let size = size_of::<$ty>();
                if let Ok(bytes) = self.0[..size].try_into() {
                    Ok(<$ty>::from_be_bytes(bytes))
                } else {
                    Err(UbusError::InvalidData(stringify!("Blob wrong size for " $ty)))
                }
            }
        }
    };
}
payload_try_into_number!(u8, i8, u16, i16, u32, i32, u64, i64, f64,);

impl<'a> TryInto<bool> for BlobPayloadParser<'a> {
    type Error = UbusError;
    fn try_into(self) -> Result<bool, Self::Error> {
        let value: u8 = self.0[0];
        Ok(value != 0)
    }
}


impl<'a> TryInto<String> for BlobPayloadParser<'a> {
    type Error = UbusError;
    fn try_into(self) -> Result<String, UbusError> {
        let data = if self.0.last() == Some(&b'\0') {
            self.0[..self.0.len() - 1].to_vec()
        } else {
            self.into()
        };
        String::from_utf8(data.to_vec()).map_err(UbusError::from)
    }
}

/**
 * parse raw bytes into Vec<BlobMsg>
 * main magic happens in BlobIter::next() -> BlobMsg::from_bytes()
 */
impl<'a> TryInto<MsgTable> for BlobPayloadParser<'a> {
    type Error = UbusError;
    fn try_into(self) -> Result<MsgTable, UbusError> {
        Ok(BlobIter::new(self.into())
            .map(|blob| blob.try_into())
            .try_collect::<Vec<BlobMsg>>()?
            .into())
    }
}
impl<'a> TryInto<Vec<BlobMsg>> for BlobPayloadParser<'a> {
    type Error = UbusError;
    fn try_into(self) -> Result<Vec<BlobMsg>, UbusError> {
        Ok(BlobIter::new(self.into())
            .map(|blob| blob.try_into())
            .try_collect::<Vec<BlobMsg>>()?)
    }
}

impl<'a> Into<Vec<u8>> for BlobPayloadParser<'a> {
    fn into(self) -> Vec<u8> {
        self.0.to_owned()
    }
}

impl<'a> Into<&'a [u8]> for BlobPayloadParser<'a> {
    fn into(self) -> &'a [u8] {
        self.0
    }
}

impl<'a> TryInto<crate::UbusMsgStatus> for BlobPayloadParser<'a> {
    type Error = UbusError;
    fn try_into(self) -> Result<crate::UbusMsgStatus, Self::Error> {
        let size = size_of::<i32>();
        if let Ok(bytes) = self.0[..size].try_into() {
            Ok(crate::UbusMsgStatus(i32::from_be_bytes(bytes)))
        } else {
            Err(UbusError::InvalidData(stringify!("Blob wrong size for")))
        }
    }
}
