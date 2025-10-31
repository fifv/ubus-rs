use crate::{Blob, BlobBuilder, BlobIter, BlobMsgPayload, BlobTag, IO, Payload, UbusError};
use core::convert::TryInto;
use core::mem::{size_of, transmute};
use std::borrow::ToOwned;
use std::dbg;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::string::String;
use std::vec::Vec;
use storage_endian::{BEu16, BEu32};

values!(pub UbusMsgVersion(u8) {
    CURRENT = 0x00,
});

values!(pub UbusCmdType(u8) {
    HELLO           = 0x00,
    STATUS          = 0x01,
    DATA            = 0x02,
    PING            = 0x03,
    LOOKUP          = 0x04,
    INVOKE          = 0x05,
    ADD_OBJECT      = 0x06,
    REMOVE_OBJECT   = 0x07,
    SUBSCRIBE       = 0x08,
    UNSUBSCRIBE     = 0x09,
    NOTIFY          = 0x10,
    MONITOR         = 0x11,
});

values!(pub UbusBlobType(u32) {
    UNSPEC      = 0x00,
    STATUS      = 0x01,
    OBJPATH     = 0x02,
    OBJID       = 0x03,
    METHOD      = 0x04,
    OBJTYPE     = 0x05,
    SIGNATURE   = 0x06,
    DATA        = 0x07,
    TARGET      = 0x08,
    ACTIVE      = 0x09,
    NO_REPLY    = 0x0a,
    SUBSCRIBERS = 0x0b,
    USER        = 0x0c,
    GROUP       = 0x0d,
});

values!(pub UbusMsgStatus(i32) {
    OK                    = 0x00,
    INVALID_COMMAND       = 0x01,
    INVALID_ARGUMENT      = 0x02,
    METHOD_NOT_FOUND      = 0x03,
    NOT_FOUND             = 0x04,
    NO_DATA               = 0x05,
    PERMISSION_DENIED     = 0x06,
    TIMEOUT               = 0x07,
    NOT_SUPPORTED         = 0x08,
    UNKNOWN_ERROR         = 0x09,
    CONNECTION_FAILED     = 0x0a,
    NO_MEMORY             = 0x0b,
    PARSE_ERROR           = 0x0c,
    SYSTEM_ERROR          = 0x0d,
});

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct UbusMsgHeader {
    pub version: UbusMsgVersion,
    pub cmd_type: UbusCmdType,
    pub sequence: BEu16,
    pub peer: BEu32,
}

impl UbusMsgHeader {
    pub const SIZE: usize = size_of::<Self>();

    /// Create MessageHeader from a byte array
    pub fn from_bytes(buffer: [u8; Self::SIZE]) -> Self {
        unsafe { transmute(buffer) }
    }
    // Dump out bytes of MessageHeader
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        unsafe { core::mem::transmute(self) }
    }
}

#[derive(Clone)]
pub struct UbusMsg {
    pub header: UbusMsgHeader,
    pub blob: Blob,
}

impl UbusMsg {
    pub fn from_io<T: IO>(io: &mut T, buffer: &mut [u8]) -> Result<Self, UbusError> {
        let (pre_buffer, buffer) = buffer.split_at_mut(UbusMsgHeader::SIZE + BlobTag::SIZE);

        // Read in the message header and the following blob tag
        io.get(pre_buffer)?;

        let (header, tag) = pre_buffer.split_at(UbusMsgHeader::SIZE);

        let header = UbusMsgHeader::from_bytes(header.try_into().unwrap());
        valid_data!(header.version == UbusMsgVersion::CURRENT, "Wrong version");

        let tag = BlobTag::from_bytes(tag.try_into().unwrap());
        tag.is_valid()?;

        // Get a slice the size of the blob's data bytes (do we need to worry about padding here?)
        let data = &mut buffer[..tag.inner_len()];

        // Receive data into slice
        io.get(data)?;

        // Create the blob from our parts
        let blob = Blob::from_tag_and_data(tag, data).unwrap();

        Ok(UbusMsg { header, blob })
    }
}

impl core::fmt::Debug for UbusMsg {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "Message({:?} seq={} peer={:08x}, size={})",
            self.header.cmd_type,
            self.header.sequence,
            self.header.peer,
            self.blob.data.len()
        )
    }
}

pub struct UbusMsgBuilder {
    buffer: Vec<u8>,
    offset: usize,
}

impl UbusMsgBuilder {
    pub fn new(header: &UbusMsgHeader) -> Result<Self, UbusError> {
        let mut buffer = Vec::new();
        valid_data!(
            buffer.len() >= (UbusMsgHeader::SIZE + BlobTag::SIZE),
            "Builder buffer is too small"
        );

        let header_buf = &mut buffer[..UbusMsgHeader::SIZE];
        let header_buf: &mut [u8; UbusMsgHeader::SIZE] = header_buf.try_into().unwrap();
        *header_buf = header.to_bytes();

        let offset = UbusMsgHeader::SIZE + BlobTag::SIZE;

        Ok(Self { buffer, offset })
    }

    pub fn put(&mut self, attr: UbusMsgAttr) -> Result<(), UbusError> {
        let mut blob = BlobBuilder::from_bytes(&mut self.buffer[self.offset..]);

        match attr {
            UbusMsgAttr::Status(val) => blob.push_u32(UbusBlobType::STATUS, val.0 as u32)?,
            UbusMsgAttr::ObjPath(val) => blob.push_str(UbusBlobType::OBJPATH, &val)?,
            UbusMsgAttr::ObjId(val) => blob.push_u32(UbusBlobType::OBJID, val)?,
            UbusMsgAttr::Method(val) => blob.push_str(UbusBlobType::METHOD, &val)?,
            //UbusMsgAttr::ObjType(val) => blob.push_u32(BlobAttrId::STATUS, &val)?,
            UbusMsgAttr::ObjType(val) => blob.push_u32(UbusBlobType::OBJTYPE, val)?,
            UbusMsgAttr::Signature(_) => unimplemented!(),
            UbusMsgAttr::Data(val) => blob.push_bytes(UbusBlobType::DATA, &val)?,
            UbusMsgAttr::Target(val) => blob.push_u32(UbusBlobType::TARGET, val)?,
            UbusMsgAttr::Active(val) => blob.push_bool(UbusBlobType::ACTIVE, val)?,
            UbusMsgAttr::NoReply(val) => blob.push_bool(UbusBlobType::NO_REPLY, val)?,
            UbusMsgAttr::Subscribers(_) => unimplemented!(),
            UbusMsgAttr::User(val) => blob.push_str(UbusBlobType::USER, &val)?,
            UbusMsgAttr::Group(val) => blob.push_str(UbusBlobType::GROUP, &val)?,
            UbusMsgAttr::Unknown(id, val) => blob.push_bytes(id, &val)?,
        };

        self.offset += blob.len();
        Ok(())
    }

    pub fn finish(self) -> Vec<u8> {
        // Update tag with correct size
        let tag = BlobTag::try_build(UbusBlobType::UNSPEC, self.offset - UbusMsgHeader::SIZE, false).unwrap();
        let tag_buf = & self.buffer[UbusMsgHeader::SIZE..UbusMsgHeader::SIZE + BlobTag::SIZE];
        let tag_buf: & [u8; BlobTag::SIZE] = tag_buf.try_into().unwrap();
        *tag_buf = tag.to_bytes();
        self.buffer[..self.offset].to_owned()
    }
}
impl<'a> Into<Vec<u8>> for UbusMsgBuilder {
    fn into(self) -> Vec<u8> {
        self.finish()
    }
}

#[derive(Debug)]
pub enum UbusMsgAttr {
    Status(UbusMsgStatus),
    ObjPath(String),
    ObjId(u32),
    Method(String),
    ObjType(u32),
    Signature(HashMap<String, BlobMsgPayload>),
    Data(Vec<u8>),
    Target(u32),
    Active(bool),
    NoReply(bool),
    Subscribers(BlobIter<Blob>),
    User(String),
    Group(String),
    Unknown(UbusBlobType, Vec<u8>),
}

impl<'a> From<Blob> for UbusMsgAttr {
    fn from(blob: Blob) -> Self {
        let payload = Payload::from(&blob.data);
        match blob.tag.id().into() {
            UbusBlobType::STATUS => UbusMsgAttr::Status(payload.try_into().unwrap()),
            UbusBlobType::OBJPATH => UbusMsgAttr::ObjPath(payload.try_into().unwrap()),
            UbusBlobType::OBJID => UbusMsgAttr::ObjId(payload.try_into().unwrap()),
            UbusBlobType::METHOD => UbusMsgAttr::Method(payload.try_into().unwrap()),
            UbusBlobType::OBJTYPE => UbusMsgAttr::ObjType(payload.try_into().unwrap()),
            UbusBlobType::SIGNATURE => UbusMsgAttr::Signature(payload.try_into().unwrap()),
            UbusBlobType::DATA => UbusMsgAttr::Data(payload.try_into().unwrap()),
            UbusBlobType::TARGET => UbusMsgAttr::Target(payload.try_into().unwrap()),
            UbusBlobType::ACTIVE => UbusMsgAttr::Active(payload.try_into().unwrap()),
            UbusBlobType::NO_REPLY => UbusMsgAttr::NoReply(payload.try_into().unwrap()),
            UbusBlobType::SUBSCRIBERS => UbusMsgAttr::Subscribers(payload.try_into().unwrap()),
            UbusBlobType::USER => UbusMsgAttr::User(payload.try_into().unwrap()),
            UbusBlobType::GROUP => UbusMsgAttr::Group(payload.try_into().unwrap()),
            id => UbusMsgAttr::Unknown(id, blob.data),
        }
    }
}
