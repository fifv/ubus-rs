use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Blob, BlobPayloadParser, BlobTag, UbusError};

pub type JsonObject = serde_json::Map<String, Value>;

/* TODO: restructure this file, looks so scattered */

values!(pub BlobMsgType(u32) {
    UNSPEC = 0,
    ARRAY  = 1,
    TABLE  = 2,
    STRING = 3,
    INT64  = 4,
    INT32  = 5,
    INT16  = 6,
    BOOL   = 7,
    INT8   = 7, /* ubus use int8 as true/false */
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlobMsgPayload {
    Array(Vec<BlobMsg>),
    Table(Vec<BlobMsg>),
    String(String),
    Int64(i64),
    Int32(i32),
    Int16(i16),
    Bool(bool),
    Double(f64),
    Unknown(u32, Vec<u8>),
}

/**
 * turn raw bytes into BlobMsg
 *
 */
impl TryFrom<&[u8]> for BlobMsg {
    type Error = UbusError;
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
        let parser = BlobPayloadParser::from(&data[name_padding..tag.inner_len() - name_total_len]);
        let data = match BlobMsgType(tag.blob_type()) {
            BlobMsgType::ARRAY => BlobMsgPayload::Array(parser.try_into()?),
            BlobMsgType::TABLE => BlobMsgPayload::Table(parser.try_into()?),
            BlobMsgType::STRING => BlobMsgPayload::String(parser.try_into()?),
            BlobMsgType::INT64 => BlobMsgPayload::Int64(parser.try_into()?),
            BlobMsgType::INT32 => BlobMsgPayload::Int32(parser.try_into()?),
            BlobMsgType::INT16 => BlobMsgPayload::Int16(parser.try_into()?),
            BlobMsgType::BOOL => BlobMsgPayload::Bool(parser.try_into()?),
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
impl TryFrom<BlobMsg> for Vec<u8> {
    type Error = UbusError;
    fn try_from(blobmsg: BlobMsg) -> Result<Self, Self::Error> {
        let builder = BlobMsgBuilder::try_from(blobmsg)?;
        Ok(builder.data())
    }
}

/**
 * convert between Value and BlobMsgPayload
 */
impl From<Value> for BlobMsgPayload {
    fn from(json_value: Value) -> Self {
        match json_value {
            Value::Null => BlobMsgPayload::Unknown(0, vec![]),

            Value::Bool(b) => BlobMsgPayload::Bool(b),

            Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    if i <= i16::MAX as i64 && i >= i16::MIN as i64 {
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

            Value::String(s) => BlobMsgPayload::String(s.to_string()),

            Value::Array(arr) => BlobMsgPayload::Array(
                arr.into_iter()
                    .map(|v| BlobMsg {
                        name: "".into(),
                        data: BlobMsgPayload::from(v),
                    })
                    .collect(),
            ),

            Value::Object(map) => BlobMsgPayload::Table(
                map.into_iter()
                    .map(|(k, v)| BlobMsg {
                        name: k,
                        data: BlobMsgPayload::from(v),
                    })
                    .collect::<Vec<BlobMsg>>(),
            ),
        }
    }
}

impl TryFrom<BlobMsgPayload> for Value {
    type Error = UbusError;
    fn try_from(blobmsg_payload: BlobMsgPayload) -> Result<Self, Self::Error> {
        Ok(match blobmsg_payload {
            BlobMsgPayload::Bool(b) => Value::Bool(b),
            BlobMsgPayload::Int16(v) => Value::Number(v.into()),
            BlobMsgPayload::Int32(v) => Value::Number(v.into()),
            BlobMsgPayload::Int64(v) => Value::Number(v.into()),
            BlobMsgPayload::Double(f) => Value::Number(
                serde_json::Number::from_f64(f)
                    .ok_or_else(|| UbusError::InvalidData("NaN JSON"))?,
            ),

            BlobMsgPayload::String(s) => Value::String(s),

            BlobMsgPayload::Array(arr) => Value::Array(
                arr.into_iter()
                    .map(|blobmsg| blobmsg.data.try_into())
                    .try_collect::<Vec<Value>>()?,
            ),

            BlobMsgPayload::Table(map) => Value::Object(
                map.into_iter()
                    .map(|blobmsg| Ok::<_, UbusError>((blobmsg.name, blobmsg.data.try_into()?)))
                    .try_collect::<serde_json::Map<String, Value>>()?
                    .into(),
            ),

            BlobMsgPayload::Unknown(_, _) => {
                return Err(UbusError::InvalidData("Unknown blob type"));
            }
        })
    }
}

#[derive(Clone)]
pub struct MsgTable(pub Vec<BlobMsg>);
impl MsgTable {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn to_string(self) -> Result<String, UbusError> {
        // String::try_from(self)
        self.try_into()
    }
    pub fn to_string_clone(&self) -> Result<String, UbusError> {
        // String::try_from(self)
        self.clone().try_into()
    }
    pub fn to_string_pretty(self) -> Result<String, UbusError> {
        // String::try_from(self)
        Ok(serde_json::to_string_pretty(&JsonObject::try_from(self)?)
            .expect("serialize serde_json::Map<String, Value> shouldn't fail"))
    }
}
impl Default for MsgTable {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl core::fmt::Debug for MsgTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MsgTable {}",
            self.to_string_clone().unwrap_or("<FAILED>".to_string())
        )
    }
}

impl TryFrom<MsgTable> for Vec<u8> {
    type Error = UbusError;
    /**
     * turn Vec<BlobMsg> into bytes
     * Real magic happens on `BlobMsg::try_into(Vec<u8>)` -> `BlobMsgBuilder::try_from()`
     */
    fn try_from(msgtable: MsgTable) -> Result<Self, Self::Error> {
        Ok(msgtable
            .0
            .into_iter()
            .map(|blobmsg| <Vec<u8>>::try_from(blobmsg))
            .try_collect::<Vec<Vec<u8>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<u8>>())
    }
}
impl From<Vec<BlobMsg>> for MsgTable {
    fn from(blobmsg: Vec<BlobMsg>) -> Self {
        Self(blobmsg)
    }
}
impl TryFrom<Blob> for BlobMsg {
    type Error = UbusError;
    fn try_from(blob: Blob) -> Result<Self, Self::Error> {
        match blob {
            Blob::BlogMsg(blobmsg) => Ok(blobmsg),
            Blob::UbusBlob(_) => Err(UbusError::InvalidData("")),
        }
    }
}
impl TryFrom<&str> for MsgTable {
    type Error = UbusError;
    fn try_from(json: &str) -> Result<Self, Self::Error> {
        /* empty string is legal */
        if json.is_empty() {
            return Ok(MsgTable::new());
        }
        // top-level MUST be object/array to produce args
        serde_json::from_str::<Value>(json)?.try_into()
    }
}
impl TryFrom<JsonObject> for MsgTable {
    type Error = UbusError;
    fn try_from(json_map: JsonObject) -> Result<Self, Self::Error> {
        Ok(MsgTable(
            json_map
                .into_iter()
                .map(|(k, v)| BlobMsg {
                    name: k,
                    data: v.into(),
                })
                .collect(),
        ))
    }
}
impl TryFrom<Value> for MsgTable {
    type Error = UbusError;
    fn try_from(json_value: Value) -> Result<Self, Self::Error> {
        match json_value {
            Value::Object(map) => map.try_into(),
            _ => Err(UbusError::InvalidData(
                "Invalid JSON, must be object at top-level",
            )),
        }
    }
}

impl TryFrom<MsgTable> for String {
    type Error = UbusError;
    fn try_from(value: MsgTable) -> Result<Self, Self::Error> {
        let json_obj: JsonObject = value.try_into()?;
        Ok(Value::Object(json_obj).to_string())
    }
}
impl TryFrom<MsgTable> for JsonObject {
    type Error = UbusError;
    fn try_from(value: MsgTable) -> Result<Self, Self::Error> {
        value
            .0
            .into_iter()
            .try_fold(JsonObject::new(), |mut obj, msg| {
                let (k, v) = (msg.name, msg.data.try_into()?);
                obj.insert(k, v);
                Ok(obj)
            })
    }
}
/* TODO: use something like Map<String, Map<String, BlobMsgType>> to describe UbusBlob::Signature */
// pub struct MethodSignature(Vec<>)

/**
 * BlobMsgBuilder is used to convert BlobMsg from "native rust struct" to "raw bytes on wire"
 *
 * TODO: move to BlobMsg itself
 */
pub struct BlobMsgBuilder {
    buffer: Vec<u8>,
}

impl TryFrom<BlobMsg> for BlobMsgBuilder {
    type Error = UbusError;

    fn try_from(blobmsg: BlobMsg) -> Result<Self, Self::Error> {
        let name = blobmsg.name;
        match blobmsg.data {
            BlobMsgPayload::String(s) => BlobMsgBuilder::from_str(BlobMsgType::STRING, &name, &s),
            BlobMsgPayload::Int64(num) => {
                BlobMsgBuilder::from_int64(BlobMsgType::INT64, &name, num)
            }
            BlobMsgPayload::Int32(num) => {
                BlobMsgBuilder::from_int32(BlobMsgType::INT32, &name, num)
            }
            BlobMsgPayload::Int16(num) => {
                BlobMsgBuilder::from_int16(BlobMsgType::INT16, &name, num)
            }
            BlobMsgPayload::Double(num) => {
                BlobMsgBuilder::from_double(BlobMsgType::DOUBLE, &name, num)
            }
            BlobMsgPayload::Bool(b) => BlobMsgBuilder::from_bool(BlobMsgType::BOOL, &name, b),
            BlobMsgPayload::Unknown(_typeid, _bytes) => {
                //println!("\"type={} data={:?}\"", typeid, bytes);
                unimplemented!()
            }
            BlobMsgPayload::Array(list) => {
                let mut builder = BlobMsgBuilder::new_extended(BlobMsgType::ARRAY, &name);
                for blobmsg in list {
                    let inner_builder = BlobMsgBuilder::try_from(blobmsg).unwrap();
                    builder.push_bytes(inner_builder.data_as_slice())?;
                }
                Ok(builder)
            }
            BlobMsgPayload::Table(table) => {
                let mut builder = BlobMsgBuilder::new_extended(BlobMsgType::TABLE, &name);
                for blobmsg in table {
                    let inner_builder = BlobMsgBuilder::try_from(blobmsg).unwrap();
                    builder.push_bytes(inner_builder.data_as_slice())?;
                }
                Ok(builder)
            }
        }
    }
}

impl BlobMsgBuilder {
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

    /* TODO: maybe these can be merged by generics */

    pub fn from_bytes<'b>(
        id: BlobMsgType,
        name: &str,
        data: impl IntoIterator<Item = &'b u8>,
    ) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_bytes(data)?;
        Ok(builder)
    }
    pub fn from_int64(id: BlobMsgType, name: &str, data: i64) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_int64(data)?;
        Ok(builder)
    }
    pub fn from_int32(id: BlobMsgType, name: &str, data: i32) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_int32(data)?;
        Ok(builder)
    }
    pub fn from_int16(id: BlobMsgType, name: &str, data: i16) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_int16(data)?;
        Ok(builder)
    }
    pub fn from_double(id: BlobMsgType, name: &str, data: f64) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_double(data)?;
        Ok(builder)
    }
    pub fn from_bool(id: BlobMsgType, name: &str, data: bool) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_bool(data)?;
        Ok(builder)
    }
    pub fn from_str(id: BlobMsgType, name: &str, data: &str) -> Result<Self, UbusError> {
        let mut builder = BlobMsgBuilder::new_extended(id, &name);
        builder.push_str(data)?;
        Ok(builder)
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

    pub fn data_as_slice(&self) -> &[u8] {
        &self.buffer
    }
    pub fn data(self) -> Vec<u8> {
        self.buffer
    }
}
