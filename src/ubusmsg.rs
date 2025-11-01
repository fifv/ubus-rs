use crate::{
    Blob, BlobBuilder, BlobIter, BlobMsgPayload, BlobTag, IO, UbusBlob, UbusBlobPayload, UbusError,
};
use core::convert::TryInto;
use core::mem::{size_of, transmute};
use serde::{Deserialize, Serialize};
use std::borrow::ToOwned;
use std::collections::HashMap;
use std::string::String;
use std::vec::Vec;
use std::{dbg, vec};
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
    pub blobs: Vec<UbusBlob>,
}


impl UbusMsg {
    pub fn from_io<T: IO>(io: &mut T) -> Result<Self, UbusError> {
        /* read ubus message header */
        let mut ubusmsg_header_buffer = [0u8; UbusMsgHeader::SIZE];
        io.get(&mut ubusmsg_header_buffer)?;
        let header = UbusMsgHeader::from_bytes(ubusmsg_header_buffer);
        valid_data!(header.version == UbusMsgVersion::CURRENT, "Wrong version");

        /* read the container blob header */
        let mut ubusmsg_blob_header_buffer = [0u8; BlobTag::SIZE];
        io.get(&mut ubusmsg_blob_header_buffer)?;
        let tag = BlobTag::from_bytes(&ubusmsg_blob_header_buffer);
        tag.is_valid()?;

        /* use the length extracted from blob header, read such length of blob data  */
        let mut ubusmsg_data_buffer = vec![0u8; tag.inner_len()];
        io.get(&mut ubusmsg_data_buffer)?;
        let blobs = BlobIter::new(ubusmsg_data_buffer)
            .map(|blob| blob.try_into())
            .try_collect::<Vec<UbusBlob>>()?;
        // let blob = UbusBlob::from_tag_and_data(tag, ubusmsg_data_buffer).unwrap();

        Ok(UbusMsg { header, blobs })
    }

    pub fn from_header_and_blobs(header: &UbusMsgHeader, blobs: Vec<UbusBlob>) -> Self {
        Self {
            header: *header,
            blobs: blobs,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let ubusmsg_header_buf = self.header.to_bytes();


        let mut ubusmsg_blobs_buffer = Vec::new();
        for blob in self.blobs {
            ubusmsg_blobs_buffer.extend_from_slice(&blob.to_bytes());
        }

        let ubusmsg_blob_header_buffer = BlobTag::try_build(
            UbusBlobType::UNSPEC,
            BlobTag::SIZE + ubusmsg_blobs_buffer.len(),
            false,
        )
        .expect("???")
        .to_bytes();


        let mut raw_msg_data = Vec::new();
        raw_msg_data.extend_from_slice(&ubusmsg_header_buf);
        raw_msg_data.extend_from_slice(&ubusmsg_blob_header_buffer);
        raw_msg_data.extend_from_slice(&ubusmsg_blobs_buffer);
        raw_msg_data
    }
}

impl core::fmt::Debug for UbusMsg {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "Message({:?} seq={}, peer={:08x}, blobs={:?})",
            self.header.cmd_type, self.header.sequence, self.header.peer, self.blobs
        )
    }
}

// impl Into<Vec<u8>> for UbusMsg {

// }

// pub struct UbusMsgBuilder___ {
//     buffer: Vec<u8>,
//     offset: usize,
// }

// impl UbusMsgBuilder___ {
//     pub fn new(header: &UbusMsgHeader) -> Result<Self, UbusError> {
//         let mut buffer = Vec::new();
//         valid_data!(
//             buffer.len() >= (UbusMsgHeader::SIZE + BlobTag::SIZE),
//             "Builder buffer is too small"
//         );

//         let header_buf = &mut buffer[..UbusMsgHeader::SIZE];
//         let header_buf: &mut [u8; UbusMsgHeader::SIZE] = header_buf.try_into().unwrap();
//         *header_buf = header.to_bytes();

//         let offset = UbusMsgHeader::SIZE + BlobTag::SIZE;

//         Ok(Self { buffer, offset })
//     }

//     pub fn put(&mut self, attr: UbusMsgAttr___) -> Result<(), UbusError> {
//         let mut blob = BlobBuilder::from_bytes(&mut self.buffer[self.offset..]);

//         match attr {
//             UbusMsgAttr___::Status(val) => blob.push_u32(UbusBlobType::STATUS, val.0 as u32)?,
//             UbusMsgAttr___::ObjPath(val) => blob.push_str(UbusBlobType::OBJPATH, &val)?,
//             UbusMsgAttr___::ObjId(val) => blob.push_u32(UbusBlobType::OBJID, val)?,
//             UbusMsgAttr___::Method(val) => blob.push_str(UbusBlobType::METHOD, &val)?,
//             //UbusMsgAttr::ObjType(val) => blob.push_u32(BlobAttrId::STATUS, &val)?,
//             UbusMsgAttr___::ObjType(val) => blob.push_u32(UbusBlobType::OBJTYPE, val)?,
//             UbusMsgAttr___::Signature(_) => unimplemented!(),
//             UbusMsgAttr___::Data(val) => blob.push_bytes(UbusBlobType::DATA, &val)?,
//             UbusMsgAttr___::Target(val) => blob.push_u32(UbusBlobType::TARGET, val)?,
//             UbusMsgAttr___::Active(val) => blob.push_bool(UbusBlobType::ACTIVE, val)?,
//             UbusMsgAttr___::NoReply(val) => blob.push_bool(UbusBlobType::NO_REPLY, val)?,
//             UbusMsgAttr___::Subscribers(_) => unimplemented!(),
//             UbusMsgAttr___::User(val) => blob.push_str(UbusBlobType::USER, &val)?,
//             UbusMsgAttr___::Group(val) => blob.push_str(UbusBlobType::GROUP, &val)?,
//             UbusMsgAttr___::Unknown(id, val) => blob.push_bytes(id, &val)?,
//         };

//         self.offset += blob.len();
//         Ok(())
//     }

//     pub fn finish(self) -> Vec<u8> {
//         // Update tag with correct size
//         let tag = BlobTag::try_build(
//             UbusBlobType::UNSPEC,
//             self.offset - UbusMsgHeader::SIZE,
//             false,
//         )
//         .unwrap();
//         let tag_buf = &self.buffer[UbusMsgHeader::SIZE..UbusMsgHeader::SIZE + BlobTag::SIZE];
//         let tag_buf: &[u8; BlobTag::SIZE] = tag_buf.try_into().unwrap();
//         *tag_buf = tag.to_bytes();
//         self.buffer[..self.offset].to_owned()
//     }
// }
// impl<'a> Into<Vec<u8>> for UbusMsgBuilder___ {
//     fn into(self) -> Vec<u8> {
//         self.finish()
//     }
// }

// #[derive(Debug)]
// pub enum UbusMsgAttr___ {
//     Status(UbusMsgStatus),
//     ObjPath(String),
//     ObjId(u32),
//     Method(String),
//     ObjType(u32),
//     Signature(HashMap<String, BlobMsgPayload>),
//     Data(Vec<u8>),
//     Target(u32),
//     Active(bool),
//     NoReply(bool),
//     Subscribers(BlobIter<UbusBlob>),
//     User(String),
//     Group(String),
//     Unknown(UbusBlobType, Vec<u8>),
// }

// impl<'a> From<UbusBlob> for UbusMsgAttr___ {
//     fn from(blob: UbusBlob) -> Self {
//         let payload = UbusBlobPayload::from(&blob.data);
//         match blob.tag.id().into() {
//             UbusBlobType::STATUS => UbusMsgAttr___::Status(payload.try_into().unwrap()),
//             UbusBlobType::OBJPATH => UbusMsgAttr___::ObjPath(payload.try_into().unwrap()),
//             UbusBlobType::OBJID => UbusMsgAttr___::ObjId(payload.try_into().unwrap()),
//             UbusBlobType::METHOD => UbusMsgAttr___::Method(payload.try_into().unwrap()),
//             UbusBlobType::OBJTYPE => UbusMsgAttr___::ObjType(payload.try_into().unwrap()),
//             UbusBlobType::SIGNATURE => UbusMsgAttr___::Signature(payload.try_into().unwrap()),
//             UbusBlobType::DATA => UbusMsgAttr___::Data(payload.try_into().unwrap()),
//             UbusBlobType::TARGET => UbusMsgAttr___::Target(payload.try_into().unwrap()),
//             UbusBlobType::ACTIVE => UbusMsgAttr___::Active(payload.try_into().unwrap()),
//             UbusBlobType::NO_REPLY => UbusMsgAttr___::NoReply(payload.try_into().unwrap()),
//             UbusBlobType::SUBSCRIBERS => UbusMsgAttr___::Subscribers(payload.try_into().unwrap()),
//             UbusBlobType::USER => UbusMsgAttr___::User(payload.try_into().unwrap()),
//             UbusBlobType::GROUP => UbusMsgAttr___::Group(payload.try_into().unwrap()),
//             id => UbusMsgAttr___::Unknown(id, blob.data),
//         }
//     }
// }
