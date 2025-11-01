use crate::{
    BlobMsg, BlobMsgPayload, BlobMsgType, MsgTable, UbusBlobType, UbusError, UbusMsg, UbusMsgStatus,
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


#[derive(Clone, Debug)]
pub enum Blob {
    UbusBlob(UbusBlob),
    BlogMsg(BlobMsg),
}
impl TryFrom<Blob> for UbusBlob {
    type Error = UbusError;
    fn try_from(value: Blob) -> Result<Self, Self::Error> {
        match value {
            Blob::UbusBlob(blob) => Ok(blob),
            Blob::BlogMsg(_) => Err(UbusError::InvalidData("")),
        }
    }
}

#[derive(Clone, Debug)]
pub enum UbusBlob {
    Unspec(Vec<u8>),
    Status(UbusMsgStatus),
    ObjPath(String),
    ObjId(i32),
    Method(String),
    ObjType(i32),
    Signature(MsgTable),
    Data(MsgTable),
    Target(i32),
    Active(bool),
    NoReply(bool),
    Subscribers(i32),
    User(String),
    Group(String),
}


/**
 * Blob is a TLV
 *      IsExtended(1bit) + Type(7bit) + Length(24bit) + Payload
 * BlobMsg is Blob with additional TL field name namelen + *name
 *      IsExtended(1bit) + Type(7bit) + Length(24bit) + Name(namelen 16bit + name variable) + Payload
 *
 * They are identified by the Most-Significant-Bit of Blob Type
 */

/**
 * This represents struct blob_attr.id_len in blob.h
 * which is a 32 bit data, 30~24 bit represents id, 23~0 bit represents length of data next to this BlobTag + 24 bit BlobTag
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
    pub fn try_build(id: UbusBlobType, len: usize, extended: bool) -> Result<Self, UbusError> {
        if id.value() > Self::ID_MASK || len < Self::SIZE || len > Self::LEN_MASK as usize {
            Err(UbusError::InvalidData("Invalid TAG construction"))
        } else {
            let id = id.value() & Self::ID_MASK;
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
    /// ID code of this blob
    pub fn blob_type(&self) -> UbusBlobType {
        UbusBlobType(u32::from((self.0 >> Self::ID_SHIFT) & Self::ID_MASK))
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
    fn next_tag(&self) -> usize {
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
        write!(f, "BlobTag(id={:?}, len={}{})", id, len, extended)
    }
}


/**
 * `BlobBuilder` is used to convert `UbusBlob` between "native rust struct" and "raw bytes on wire"
 */
pub struct BlobBuilder {
    buffer: Vec<u8>,
    offset: usize,
}

impl BlobBuilder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            offset: 0,
        }
    }

    pub fn push_u32(&mut self, id: UbusBlobType, data: u32) -> Result<(), UbusError> {
        self.push_bytes(id, &data.to_be_bytes())
    }

    pub fn push_bool(&mut self, id: UbusBlobType, data: bool) -> Result<(), UbusError> {
        self.push_bytes(id, if data { &[1] } else { &[0] })
    }

    pub fn push_str(&mut self, id: UbusBlobType, data: &str) -> Result<(), UbusError> {
        self.push_bytes(id, data.as_bytes().iter().chain([0u8].iter()))
    }

    pub fn push_bytes<'b>(
        &mut self,
        id: UbusBlobType,
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
    pub fn from_u32(id: UbusBlobType, data: u32) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_u32(id, data)?;
        Ok(builder)
    }

    pub fn from_bool(id: UbusBlobType, data: bool) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_bool(id, data);
        Ok(builder)
    }

    pub fn from_str(id: UbusBlobType, data: &str) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_str(id, data);
        Ok(builder)
    }

    pub fn from_bytes<'b>(
        id: UbusBlobType,
        data: impl IntoIterator<Item = &'b u8>,
    ) -> Result<Self, UbusError> {
        let mut builder = Self::new();
        builder.push_bytes(id, data);
        Ok(builder)
    }
}


impl UbusBlob {
    pub fn from_bytes(data: &[u8]) -> Result<Self, UbusError> {
        valid_data!(data.len() >= BlobTag::SIZE, "Blob too short");
        // Read the blob's tag
        let (tag, data) = data.split_at(BlobTag::SIZE);
        let tag = BlobTag::from_bytes(tag.try_into().unwrap());
        // dbg!(&tag, &data);
        Self::from_tag_and_data(tag, data)
    }
    pub fn from_tag_and_data(tag: BlobTag, data: &[u8]) -> Result<Self, UbusError> {
        tag.is_valid()?;
        // dbg!(tag, data);
        valid_data!(data.len() >= tag.inner_len(), "Blob too short");

        // Restrict data to payload size
        let data = data[..tag.inner_len()].to_owned();
        let data = UbusBlobPayload::from(data);

        match tag.blob_type() {
            UbusBlobType::UNSPEC => Ok(UbusBlob::Unspec(data.into())),
            UbusBlobType::STATUS => Ok(UbusBlob::Status(data.try_into()?)),
            UbusBlobType::OBJPATH => Ok(UbusBlob::ObjPath(data.try_into()?)),
            UbusBlobType::OBJID => Ok(UbusBlob::ObjId(data.try_into()?)),
            UbusBlobType::METHOD => Ok(UbusBlob::Method(data.try_into()?)),
            UbusBlobType::OBJTYPE => Ok(UbusBlob::ObjType(data.try_into()?)),
            UbusBlobType::SIGNATURE => Ok(UbusBlob::Signature(data.try_into()?)),
            UbusBlobType::DATA => Ok(UbusBlob::Data(data.try_into()?)),
            UbusBlobType::TARGET => Ok(UbusBlob::Target(data.try_into()?)),
            UbusBlobType::ACTIVE => Ok(UbusBlob::Active(data.try_into()?)),
            UbusBlobType::NO_REPLY => Ok(UbusBlob::NoReply(data.try_into()?)),
            UbusBlobType::SUBSCRIBERS => Ok(UbusBlob::Subscribers(data.try_into()?)),
            UbusBlobType::USER => Ok(UbusBlob::User(data.try_into()?)),
            UbusBlobType::GROUP => Ok(UbusBlob::Group(data.try_into()?)),
            unknown_type => Err(UbusError::InvalidBlobType(unknown_type)),
        }
    }

    /**
     *
     * ### Panic
     * if the data is too long and BlobTag can't build, it may panic, should be rarely
     */
    pub fn to_bytes(&self) -> Vec<u8> {
        // create payload bytes depending on variant
        match self {
            UbusBlob::Unspec(v) => {
                BlobBuilder::from_bytes(UbusBlobType::UNSPEC, v)
                    .unwrap()
                    .buffer
            }
            UbusBlob::Status(v) => {
                BlobBuilder::from_u32(UbusBlobType::STATUS, v.0 as u32)
                    .unwrap()
                    .buffer
            }
            UbusBlob::ObjPath(v) => {
                BlobBuilder::from_str(UbusBlobType::OBJPATH, v)
                    .unwrap()
                    .buffer
            }
            UbusBlob::ObjId(v) => {
                BlobBuilder::from_u32(UbusBlobType::OBJID, *v as u32)
                    .unwrap()
                    .buffer
            }
            UbusBlob::Method(v) => {
                BlobBuilder::from_str(UbusBlobType::METHOD, v)
                    .unwrap()
                    .buffer
            }
            UbusBlob::ObjType(v) => {
                BlobBuilder::from_u32(UbusBlobType::OBJTYPE, *v as u32)
                    .unwrap()
                    .buffer
            }
            UbusBlob::Signature(v) => {
                /*  */
                BlobBuilder::from_bytes(UbusBlobType::SIGNATURE, TryInto::<Vec<u8>>::try_into(v.to_owned()).unwrap().iter())
                    .unwrap()
                    .buffer
            }
            UbusBlob::Data(v) => {
                BlobBuilder::from_bytes(UbusBlobType::DATA, TryInto::<Vec<u8>>::try_into(v.to_owned()).unwrap().iter())
                    .unwrap()
                    .buffer
            }
            UbusBlob::Target(v) => {
                BlobBuilder::from_u32(UbusBlobType::TARGET, *v as u32)
                    .unwrap()
                    .buffer
            }
            UbusBlob::Active(v) => {
                BlobBuilder::from_bool(UbusBlobType::ACTIVE, *v)
                    .unwrap()
                    .buffer
            }
            UbusBlob::NoReply(v) => {
                BlobBuilder::from_bool(UbusBlobType::NO_REPLY, *v)
                    .unwrap()
                    .buffer
            }
            UbusBlob::Subscribers(v) => {
                BlobBuilder::from_u32(UbusBlobType::SUBSCRIBERS, *v as u32)
                    .unwrap()
                    .buffer
            }
            UbusBlob::User(v) => BlobBuilder::from_str(UbusBlobType::USER, v).unwrap().buffer,
            UbusBlob::Group(v) => {
                BlobBuilder::from_str(UbusBlobType::GROUP, v)
                    .unwrap()
                    .buffer
            }
        }

        // // build tag: inner_len = payload len
        // let tag = BlobTag::try_build(blob_type, payload_bytes.len(), false).expect("failed");

        // // serialize tag + payload
        // let mut result = Vec::with_capacity(BlobTag::SIZE + payload_bytes.len());
        // result.extend_from_slice(&tag.to_bytes());
        // result.extend_from_slice(&payload_bytes);
        // result
    }
}

// impl core::fmt::Debug for UbusBlob {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         write!(f, "UbusBlob({})", self.)

//     }
// }

// impl<'a> TryInto<BlobMsg> for UbusBlob {
//     type Error = UbusError;
//     fn try_into(self) -> Result<BlobMsg, Self::Error> {

//     }
// }

#[derive(Clone, Debug)]
pub struct UbusBlobPayload(Vec<u8>);
impl<'a> From<&'a [u8]> for UbusBlobPayload {
    fn from(value: &'a [u8]) -> Self {
        UbusBlobPayload(value.to_owned())
    }
}
impl<'a> From<Vec<u8>> for UbusBlobPayload {
    fn from(value: Vec<u8>) -> Self {
        UbusBlobPayload(value)
    }
}


macro_rules! payload_try_into_number {
    ( $( $ty:ty , )* ) => { $( payload_try_into_number!($ty); )* };
    ( $ty:ty ) => {
        impl<'a> TryInto<$ty> for UbusBlobPayload{
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

impl<'a> TryInto<bool> for UbusBlobPayload {
    type Error = UbusError;
    fn try_into(self) -> Result<bool, Self::Error> {
        let value: u8 = self.0[0];
        Ok(value != 0)
    }
}


impl<'a> TryInto<String> for UbusBlobPayload {
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
impl TryInto<MsgTable> for UbusBlobPayload {
    type Error = UbusError;
    fn try_into(self) -> Result<MsgTable, UbusError> {
        Ok(BlobIter::new(self.into())
            .map(|blob| blob.try_into())
            .try_collect::<Vec<BlobMsg>>()?
            .into())
    }
}
impl TryInto<Vec<BlobMsg>> for UbusBlobPayload {
    type Error = UbusError;
    fn try_into(self) -> Result<Vec<BlobMsg>, UbusError> {
        Ok(BlobIter::new(self.into())
            .map(|blob| blob.try_into())
            .try_collect::<Vec<BlobMsg>>()?)
    }
}

// impl<'a> TryInto<HashMap<String, BlobMsgPayload>> for UbusBlobPayload {
//     type Error = UbusError;
//     fn try_into(self) -> Result<HashMap<String, BlobMsgPayload>, UbusError> {
//         let mut map = HashMap::<String, BlobMsgPayload>::new();
//         let iter = BlobIter::<UbusBlob>::new(self.into());
//         for item in iter {
//             let item: BlobMsg = item.try_into()?;
//             map.insert(item.name, item.data);
//         }
//         Ok(map)
//     }
// }

impl<'a> Into<Vec<u8>> for UbusBlobPayload {
    fn into(self) -> Vec<u8> {
        self.0
    }
}

// impl Into<BlobIter> for UbusBlob {
//     fn into(self) -> BlobIter {
//         BlobIter::new(self.data)
//     }
// }

// impl Into<BlobIter> for UbusBlobPayload {
//     fn into(self) -> BlobIter {
//         BlobIter::new(self.into())
//     }
// }

impl TryInto<crate::UbusMsgStatus> for UbusBlobPayload {
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

/**
 * BlobIter used to find all Blob in the bytes
 * Blob are laid one by one in bytes
 * 
 * The actual convertion happens in BlobMsg::try_from and UbusBlob::from_bytes
 * 
 */
pub struct BlobIter {
    data: Vec<u8>,
    // _phantom: PhantomData<T>,
}
impl BlobIter {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            // _phantom: PhantomData,
        }
    }
}
impl Iterator for BlobIter {
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
                self.data = self.data[next_idx..].to_owned();
                return Some(Blob::BlogMsg(blob));
            }
        } else {
            if let Ok(blob) = UbusBlob::from_bytes(&self.data[..]) {
                // Advance the internal pointer to the next tag
                let next_idx = tag.next_tag();
                self.data = self.data[next_idx..].to_owned();
                return Some(Blob::UbusBlob(blob));
            }
        }


        None
    }
}

impl core::fmt::Debug for BlobIter {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "BlobIter (data_len={})", self.data.len())
    }
}
