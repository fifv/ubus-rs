use crate::{Blob, BlobBuilder, BlobPayloadParser, BlobTag, MsgTable, UbusError, UbusMsgStatus};
use core::fmt::{LowerHex, UpperHex};
use serde::{Deserialize, Serialize};
use std::{borrow::ToOwned, string::String, vec::Vec};

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

#[derive(Copy, Clone, Default)]
pub struct HexU32(pub u32);
impl core::fmt::Debug for HexU32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}
impl From<u32> for HexU32 {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl From<HexU32> for u32 {
    fn from(value: HexU32) -> Self {
        value.0
    }
}
impl LowerHex for HexU32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}
impl UpperHex for HexU32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:X}", self.0)
    }
}

#[derive(Clone, Debug)]
pub enum UbusBlob {
    Unspec(Vec<u8>),
    Status(UbusMsgStatus),
    ObjPath(String),
    ObjId(HexU32),
    Method(String),
    ObjType(HexU32),
    Signature(MsgTable),
    Data(MsgTable),
    Target(HexU32),
    Active(bool),
    NoReply(bool),
    Subscribers(MsgTable),
    User(String),
    Group(String),
}

// impl core::fmt::Debug for UbusBlob {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         match self {
//             Self::ObjId(id) => f.debug_tuple("ObjId").field(&format_args!("{:#x}", id)).finish(),
//             Self::ObjType(t) => f.debug_tuple("ObjType").field(&format_args!("{:#x}", t)).finish(),
//             Self::Target(t) => f.debug_tuple("Target").field(&format_args!("{:#x}", t)).finish(),

//             Self::Unspec(v) => f.debug_tuple("Unspec").field(v).finish(),
//             Self::Status(s) => f.debug_tuple("Status").field(s).finish(),
//             Self::ObjPath(s) => f.debug_tuple("ObjPath").field(s).finish(),
//             Self::Method(s) => f.debug_tuple("Method").field(s).finish(),
//             Self::Signature(t) => f.debug_tuple("Signature").field(t).finish(),
//             Self::Data(t) => f.debug_tuple("Data").field(t).finish(),
//             Self::Active(b) => f.debug_tuple("Active").field(b).finish(),
//             Self::NoReply(b) => f.debug_tuple("NoReply").field(b).finish(),
//             Self::Subscribers(t) => f.debug_tuple("Subscribers").field(t).finish(),
//             Self::User(s) => f.debug_tuple("User").field(s).finish(),
//             Self::Group(s) => f.debug_tuple("Group").field(s).finish(),
//         }
//     }
// }

impl TryFrom<Blob> for UbusBlob {
    type Error = UbusError;
    fn try_from(value: Blob) -> Result<Self, Self::Error> {
        match value {
            Blob::UbusBlob(blob) => Ok(blob),
            Blob::BlogMsg(_) => Err(UbusError::InvalidData("")),
        }
    }
}

impl TryFrom<&[u8]> for UbusBlob {
    type Error = UbusError;
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        valid_data!(data.len() >= BlobTag::SIZE, "Blob too short");
        // Read the blob's tag
        let (tag, data) = data.split_at(BlobTag::SIZE);
        let tag = BlobTag::from_bytes(tag.try_into().unwrap());
        // dbg!(&tag, &data);
        Self::from_tag_and_data(tag, data)
    }
}

impl UbusBlob {
    pub fn from_bytes(data: &[u8]) -> Result<Self, UbusError> {
        data.try_into()
    }
    pub fn from_tag_and_data(tag: BlobTag, data: &[u8]) -> Result<Self, UbusError> {
        tag.is_valid()?;
        // dbg!(tag, data);
        valid_data!(data.len() >= tag.inner_len(), "Blob too short");

        // Restrict data to payload size
        let data = &data[..tag.inner_len()];
        let parser = BlobPayloadParser::from(data);

        let _len = tag.inner_len();
        let _type = tag.blob_type();

        match UbusBlobType(tag.blob_type()) {
            UbusBlobType::UNSPEC => Ok(UbusBlob::Unspec(parser.into())),
            UbusBlobType::STATUS => Ok(UbusBlob::Status(parser.try_into()?)),
            UbusBlobType::OBJPATH => Ok(UbusBlob::ObjPath(parser.try_into()?)),
            UbusBlobType::OBJID => Ok(UbusBlob::ObjId(parser.try_into()?)),
            UbusBlobType::METHOD => Ok(UbusBlob::Method(parser.try_into()?)),
            UbusBlobType::OBJTYPE => Ok(UbusBlob::ObjType(parser.try_into()?)),
            UbusBlobType::SIGNATURE => Ok(UbusBlob::Signature(parser.try_into()?)),
            UbusBlobType::DATA => Ok(UbusBlob::Data(parser.try_into()?)),
            UbusBlobType::TARGET => Ok(UbusBlob::Target(parser.try_into()?)),
            UbusBlobType::ACTIVE => Ok(UbusBlob::Active(parser.try_into()?)),
            UbusBlobType::NO_REPLY => Ok(UbusBlob::NoReply(parser.try_into()?)),
            UbusBlobType::SUBSCRIBERS => Ok(UbusBlob::Subscribers(parser.try_into()?)),
            UbusBlobType::USER => Ok(UbusBlob::User(parser.try_into()?)),
            UbusBlobType::GROUP => Ok(UbusBlob::Group(parser.try_into()?)),
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
            UbusBlob::Unspec(v) => BlobBuilder::from_bytes(UbusBlobType::UNSPEC.value(), v)
                .unwrap()
                .into(),
            UbusBlob::Status(v) => BlobBuilder::from_u32(UbusBlobType::STATUS.value(), v.0)
                .unwrap()
                .into(),
            UbusBlob::ObjPath(v) => BlobBuilder::from_str(UbusBlobType::OBJPATH.value(), v)
                .unwrap()
                .into(),
            UbusBlob::ObjId(v) => BlobBuilder::from_u32(UbusBlobType::OBJID.value(), (*v).into())
                .unwrap()
                .into(),
            UbusBlob::Method(v) => BlobBuilder::from_str(UbusBlobType::METHOD.value(), v)
                .unwrap()
                .into(),
            UbusBlob::ObjType(v) => {
                BlobBuilder::from_u32(UbusBlobType::OBJTYPE.value(), (*v).into())
                    .unwrap()
                    .into()
            }
            UbusBlob::Signature(v) => {
                /*  */
                BlobBuilder::from_bytes(
                    UbusBlobType::SIGNATURE.value(),
                    <Vec<u8>>::try_from(v.to_owned()).unwrap().iter(),
                )
                .unwrap()
                .into()
            }
            UbusBlob::Data(v) => BlobBuilder::from_bytes(
                UbusBlobType::DATA.value(),
                <Vec<u8>>::try_from(v.to_owned()).unwrap().iter(),
            )
            .unwrap()
            .into(),
            UbusBlob::Target(v) => BlobBuilder::from_u32(UbusBlobType::TARGET.value(), (*v).into())
                .unwrap()
                .into(),
            UbusBlob::Active(v) => BlobBuilder::from_bool(UbusBlobType::ACTIVE.value(), *v)
                .unwrap()
                .into(),
            UbusBlob::NoReply(v) => BlobBuilder::from_bool(UbusBlobType::NO_REPLY.value(), *v)
                .unwrap()
                .into(),
            UbusBlob::Subscribers(v) => BlobBuilder::from_bytes(
                UbusBlobType::SUBSCRIBERS.value(),
                <Vec<u8>>::try_from(v.to_owned()).unwrap().iter(),
            )
            .unwrap()
            .into(),
            UbusBlob::User(v) => BlobBuilder::from_str(UbusBlobType::USER.value(), v)
                .unwrap()
                .into(),
            UbusBlob::Group(v) => BlobBuilder::from_str(UbusBlobType::GROUP.value(), v)
                .unwrap()
                .into(),
        }
    }
}
