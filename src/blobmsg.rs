use std::borrow::ToOwned;
use std::vec::Vec;
use std::{collections::HashMap, string::String};
use std::{fmt, vec};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Blob, BlobTag, BlobPayloadParser, UbusBlobType, UbusError};


values!(pub BlobMsgType(u32) {
    UNSPEC = 0,
    ARRAY  = 1,
    TABLE  = 2,
    STRING = 3,
    INT64  = 4,
    INT32  = 5,
    INT16  = 6,
    BOOL   = 7,
    INT8   = 7,
    DOUBLE = 8,
});

/**
 * `BlobMsg` can represent json, so they can be converted to serde_json::Value and then to string
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMsg {
    pub name: String,
    pub data: BlobMsgPayload,
}

/**
 * turn raw bytes into BlobMsg
 *
 */
impl TryFrom<&[u8]> for BlobMsg {
    type Error = UbusError;
    // FIXME: value size is not message size!
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < BlobTag::SIZE {
            return Err(UbusError::InvalidData("Data too short to get a BlobTag"));
        }

        let (tag_bytes, data) = data.split_at(BlobTag::SIZE);
        let tag = BlobTag::from_bytes(tag_bytes.try_into().unwrap());
        if !tag.is_extended() {
            return Err(UbusError::InvalidData("Not an extended blob"));
        }

        let (name_len_bytes, data) = data.split_at(size_of::<u16>());
        let name_len = u16::from_be_bytes(name_len_bytes.try_into().unwrap()) as usize;
        // Get the string
        if name_len > data.len() {
            //eprintln!("name_len:{}, data:{:?}", name_len, data);
            return Err(UbusError::InvalidData("name lenth > data lenth"));
        }

        let (name_bytes, data) = data.split_at(name_len);
        let name = String::from_utf8(name_bytes.to_vec()).unwrap();
        // Get the nul terminator (implicit)
        let name_len = name_len + 1;

        let (terminator, data) = data.split_at(1);
        valid_data!(terminator[0] == b'\0', "No extended name nul terminator");

        // Ensure the rest of the payload is aligned
        let name_total_len = size_of::<u16>() + name_len;
        let name_padding =
            BlobTag::ALIGNMENT.wrapping_sub(name_total_len) & (BlobTag::ALIGNMENT - 1);
        // FIXME\: maybe not correct
        /* ISSUE: we must limit the upper bound, if give entire buffer, parsing becomes weird */
        let parser = BlobPayloadParser::from(
            &data[name_padding..tag.inner_len() - name_total_len],
        );
        let data = match BlobMsgType(tag.blob_type()) {
            BlobMsgType::ARRAY => BlobMsgPayload::Array(parser.try_into()?),
            BlobMsgType::TABLE => BlobMsgPayload::Table(parser.try_into()?),
            BlobMsgType::STRING => BlobMsgPayload::String(parser.try_into()?),
            BlobMsgType::INT64 => BlobMsgPayload::Int64(parser.try_into()?),
            BlobMsgType::INT32 => BlobMsgPayload::Int32(parser.try_into()?),
            BlobMsgType::INT16 => BlobMsgPayload::Int16(parser.try_into()?),
            BlobMsgType::INT8 => BlobMsgPayload::Int8(parser.try_into()?),
            BlobMsgType::DOUBLE => BlobMsgPayload::Double(parser.try_into()?),
            id => BlobMsgPayload::Unknown(id.value(), parser.into()),
        };
        Ok(BlobMsg { name, data })
    }
}

/**
 * turn a single BlobMsg into bytes
 * normally BlobMsg should appear as a Vec<BlobMsg>, and here exists a helper struct MsgTable is defined to represents it
 *
 *
 */
impl TryInto<Vec<u8>> for BlobMsg {
    type Error = UbusError;
    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        let builder = BlobMsgBuilder::try_from(self)?;
        Ok(builder.data().to_owned())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlobMsgPayload {
    Array(Vec<BlobMsg>),
    Table(Vec<BlobMsg>),
    String(String),
    Int64(i64),
    Int32(i32),
    Int16(i16),
    Int8(i8),
    Bool(i8),
    Double(f64),
    Unknown(u32, Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct MsgTable(pub Vec<BlobMsg>);
impl MsgTable {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl TryInto<Vec<u8>> for MsgTable {
    type Error = UbusError;
    /**
     * turn Vec<BlobMsg> into bytes
     * Real magic happens on `BlobMsg::try_into(Vec<u8>)` -> `BlobMsgBuilder::try_from()`
     */
    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(self
            .0
            .into_iter()
            .map(|blobmsg| TryInto::<Vec<u8>>::try_into(blobmsg))
            .try_collect::<Vec<Vec<u8>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<u8>>())
    }
}
impl From<Vec<BlobMsg>> for MsgTable {
    fn from(value: Vec<BlobMsg>) -> Self {
        Self(value)
    }
}
impl TryFrom<Blob> for BlobMsg {
    type Error = UbusError;
    fn try_from(value: Blob) -> Result<Self, Self::Error> {
        match value {
            Blob::BlogMsg(blobmsg) => Ok(blobmsg),
            Blob::UbusBlob(_) => Err(UbusError::InvalidData("")),
        }
    }
}
impl TryFrom<&str> for MsgTable {
    type Error = UbusError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        fn json_to_args(json: &str) -> Result<MsgTable, UbusError> {
            let value: Value = serde_json::from_str(json).expect("Invalid JSON");

            // top-level MUST be object/array to produce args
            match value {
                Value::Object(map) => Ok(MsgTable(
                    map.into_iter()
                        .map(|(k, v)| json_value_to_blobmsg(k, v))
                        .collect(),
                )),
                _ => Err(UbusError::InvalidData(
                    "Invalid JSON, must be object at top-level",
                )),
            }
        }


        fn json_value_to_blobmsg(name: String, value: Value) -> BlobMsg {
            let payload = match value {
                Value::Null => BlobMsgPayload::Unknown(0, vec![]),

                Value::Bool(b) => BlobMsgPayload::Bool(b as i8),

                Value::Number(num) => {
                    if let Some(i) = num.as_i64() {
                        if i <= i8::MAX as i64 && i >= i8::MIN as i64 {
                            BlobMsgPayload::Int8(i as i8)
                        } else if i <= i16::MAX as i64 && i >= i16::MIN as i64 {
                            BlobMsgPayload::Int16(i as i16)
                        } else if i <= i32::MAX as i64 && i >= i32::MIN as i64 {
                            BlobMsgPayload::Int32(i as i32)
                        } else {
                            BlobMsgPayload::Int64(i)
                        }
                    } else if let Some(f) = num.as_f64() {
                        BlobMsgPayload::Double(f)
                    } else {
                        BlobMsgPayload::Unknown(1, vec![])
                    }
                }

                Value::String(s) => BlobMsgPayload::String(s),

                Value::Array(arr) => {
                    let children = arr
                        .into_iter()
                        .map(|v| json_value_to_blobmsg("".into(), v))
                        .collect();
                    BlobMsgPayload::Array(children)
                }

                Value::Object(map) => {
                    let children = map
                        .into_iter()
                        .map(|(k, v)| json_value_to_blobmsg(k, v))
                        .collect();
                    BlobMsgPayload::Table(children)
                }
            };

            BlobMsg {
                name,
                data: payload,
            }
        }
        json_to_args(value)
    }
}


impl fmt::Display for BlobMsgPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobMsgPayload::Array(list) => write!(f, "{}", List(list)),
            BlobMsgPayload::Table(dict) => write!(f, "{}", Dict(dict)),
            BlobMsgPayload::String(s) => write!(f, "\"{}\"", s),
            BlobMsgPayload::Int64(num) => write!(f, "{}", num),
            BlobMsgPayload::Int32(num) => write!(f, "{}", num),
            BlobMsgPayload::Int16(num) => write!(f, "{}", num),
            BlobMsgPayload::Int8(num) => write!(f, "{}", num),
            BlobMsgPayload::Bool(num) => write!(f, "{}", *num == 1),
            BlobMsgPayload::Double(num) => write!(f, "{}", num),
            BlobMsgPayload::Unknown(typeid, bytes) => {
                write!(f, "\"type={} data={:?}\"", typeid, bytes)
            }
        }
    }
}

struct List<'a>(&'a Vec<BlobMsg>);
impl<'a> fmt::Display for List<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for msg in self.0 {
            if !first {
                write!(f, ", ")?;
            } else {
                first = false;
            }
            write!(f, "{}", msg.data)?;
        }
        write!(f, "]")?;
        Ok(())
    }
}

struct Dict<'a>(&'a Vec<BlobMsg>);
impl<'a> fmt::Display for Dict<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for msg in self.0 {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "\"{}\": {}", msg.name, msg.data)?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}


impl fmt::Display for BlobMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name.len() > 0 {
            write!(f, "\"{}\": {}", self.name, self.data)
        } else {
            write!(f, "{}", self.data)
        }
    }
}


/**
 * BlobMsgBuilder is used to convert BlobMsg from "native rust struct" to "raw bytes on wire"
 *
 * TODO: move to BlobMsg itself and move to blobmsg.rs
 */
pub struct BlobMsgBuilder {
    buffer: Vec<u8>,
}

impl TryFrom<BlobMsg> for BlobMsgBuilder {
    type Error = UbusError;

    fn try_from(blobmsg: BlobMsg) -> Result<Self, Self::Error> {
        let name = blobmsg.name;
        let blob = match blobmsg.data {
            BlobMsgPayload::String(s) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::STRING, &name);
                blob.push_str(&s)?;
                blob
            }
            BlobMsgPayload::Int64(num) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::INT64, &name);
                blob.push_int64(num)?;
                blob
            }
            BlobMsgPayload::Int32(num) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::INT32, &name);
                blob.push_int32(num)?;
                blob
            }
            BlobMsgPayload::Int16(num) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::INT16, &name);
                blob.push_int16(num)?;
                blob
            }
            BlobMsgPayload::Int8(num) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::INT8, &name);
                blob.push_int8(num)?;
                blob
            }
            BlobMsgPayload::Double(num) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::DOUBLE, &name);
                blob.push_double(num)?;
                blob
            }
            BlobMsgPayload::Bool(b) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::BOOL, &name);
                blob.push_int8(b)?;
                blob
            }
            BlobMsgPayload::Unknown(_typeid, _bytes) => {
                //println!("\"type={} data={:?}\"", typeid, bytes);
                unimplemented!()
            }
            BlobMsgPayload::Array(list) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::ARRAY, &name);
                for item in list {
                    let inner_blob = BlobMsgBuilder::try_from(item).unwrap();
                    blob.push_bytes(inner_blob.data())?;
                }
                blob
            }
            BlobMsgPayload::Table(table) => {
                let mut blob = BlobMsgBuilder::new_extended(BlobMsgType::TABLE, &name);
                for blobmsg in table {
                    let inner_blobmsg = BlobMsg {
                        name: blobmsg.name,
                        data: blobmsg.data,
                    };
                    let inner_blob = BlobMsgBuilder::try_from(inner_blobmsg).unwrap();
                    //let inner_blob = inner_blob.build();
                    blob.push_bytes(inner_blob.data())?;
                }
                blob
            }
        };
        Ok(blob)
    }
}

impl BlobMsgBuilder {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            buffer: bytes.to_vec(),
        }
    }
    pub fn new_extended(id: BlobMsgType, name: &str) -> Self {
        // let _phantom = PhantomData::<&mut [u8]>;
        let mut blob = Self { buffer: Vec::new() };
        //blob.buffer.extend(&[0u8; BlobTag::SIZE]);
        let tag = BlobTag::try_build(id.value(), BlobTag::SIZE, true).unwrap();
        blob.buffer.extend(tag.to_bytes());
        let len_bytes = u16::to_be_bytes(name.len() as u16);
        blob.buffer.extend(len_bytes);
        blob.buffer.extend_from_slice(name.as_bytes());
        blob.buffer.push(b'\0');
        let name_total_len = size_of::<u16>() + name.len() + 1;
        let name_padding =
            BlobTag::ALIGNMENT.wrapping_sub(name_total_len) & (BlobTag::ALIGNMENT - 1);
        blob.buffer.resize(blob.buffer.len() + name_padding, 0u8);
        let tag = BlobTag::try_build(id.value(), blob.buffer.len(), true).unwrap();
        blob.buffer[..BlobTag::SIZE].copy_from_slice(&tag.to_bytes());
        blob
    }

    pub fn tag(&self) -> BlobTag {
        let tag_bytes: [u8; BlobTag::SIZE] = self.buffer[..BlobTag::SIZE].try_into().unwrap();
        BlobTag::from_bytes(&tag_bytes)
    }

    pub fn push_bytes<'b>(
        &mut self,
        data: impl IntoIterator<Item = &'b u8>,
    ) -> Result<(), UbusError> {
        for b in data {
            self.buffer.push(*b);
        }
        let mut tag = self.tag();
        tag.set_size(self.buffer.len());
        self.buffer[..4].copy_from_slice(&tag.to_bytes());
        self.buffer.resize(self.buffer.len() + tag.padding(), 0u8);
        Ok(())
    }

    pub fn push_int64(&mut self, data: i64) -> Result<(), UbusError> {
        //self.id = BlobMsgType::INT64.value();
        self.push_bytes(&data.to_be_bytes())
    }

    pub fn push_int32(&mut self, data: i32) -> Result<(), UbusError> {
        //self.id = BlobMsgType::INT32.value();
        self.push_bytes(&data.to_be_bytes())
    }

    pub fn push_int16(&mut self, data: i16) -> Result<(), UbusError> {
        //self.id = BlobMsgType::INT16.value();
        self.push_bytes(&data.to_be_bytes())
    }

    pub fn push_int8(&mut self, data: i8) -> Result<(), UbusError> {
        //self.id = BlobMsgType::INT8.value();
        self.push_bytes(&data.to_be_bytes())
    }

    pub fn push_double(&mut self, data: f64) -> Result<(), UbusError> {
        //self.id = BlobMsgType::DOUBLE.value();
        self.push_bytes(&data.to_be_bytes())
    }

    pub fn push_bool(&mut self, data: bool) -> Result<(), UbusError> {
        //self.id = BlobMsgType::BOOL.value();
        let tf: i8 = if data { 1 } else { 0 };
        self.push_bytes(&tf.to_be_bytes())
    }

    pub fn push_str(&mut self, data: &str) -> Result<(), UbusError> {
        //self.id = BlobMsgType::STRING.value();
        self.push_bytes(data.as_bytes().iter().chain([0u8].iter()))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    // pub fn build(&self) -> UbusBlob {
    //     let data = self.data();
    //     let tag = BlobTag::from_bytes(self.buffer[..4].try_into().unwrap());
    //     let data = data[4..].to_owned();
    //     // TODO: implement raw bytes -> BlobMsg
    //     UbusBlob { tag, data }
    // }
}
