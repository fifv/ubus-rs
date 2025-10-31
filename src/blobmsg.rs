use std::vec::Vec;
use std::{collections::HashMap, string::String};
use std::{fmt, vec};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{BlobTag, UbusError};

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
    Int8(i8),
    Bool(i8),
    Double(f64),
    Unknown(u32, Vec<u8>),
}

pub fn json_to_args(json: &str) -> Result<Vec<BlobMsg>, UbusError> {
    let value: Value = serde_json::from_str(json).expect("Invalid JSON");

    // top-level MUST be object/array to produce args
    match value {
        Value::Object(map) => Ok(map
            .into_iter()
            .map(|(k, v)| json_value_to_blobmsg(k, v))
            .collect()),
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
