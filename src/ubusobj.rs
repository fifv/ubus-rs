extern crate alloc;
use crate::*;
use alloc::{string::ToString, vec::Vec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{borrow::ToOwned, collections::HashMap, dbg, string::String};

pub type MethodCallback = fn(req_args: &MsgTable) -> MsgTable;
// #[derive(Default, Debug, Clone, Serialize, Deserialize)]
// pub struct Method {
//     pub name: String,
//     pub policy: HashMap<String, BlobMsgType>,
// }
#[derive(Default, Debug, Clone)]
pub struct UbusObject {
    pub path: String,
    pub id: u32,
    pub objtype: u32,
    /**
     * used on server side object, the actually callbacks
     */
    pub methods: HashMap<String, MethodCallback>,
    /**
     * used on client side lookup, store what the server says
     */
    pub reported_signature: MsgTable,
}

impl<'a> UbusObject {
    // pub fn args_from_json(&self, method: &'a str, json: &'a str) -> Result<Vec<u8>, UbusError> {
    //     let mut args = Vec::new();
    //     if json.len() == 0 {
    //         return Ok(args);
    //     }
    //     match serde_json::from_str::<Value>(json) {
    //         Ok(value) => {
    //             if let Some(object) = value.as_object() {
    //                 let method = self
    //                     .methods
    //                     .get(method)
    //                     .ok_or(UbusError::InvalidMethod(method.to_string()))?;
    //                 // TODO:
    //                 for (k, v) in object.iter() {
    //                     let mut builder =
    //                         BlobMsgBuilder::new_extended(BlobMsgType::INT32, "wtf this name?");

    //                     builder.push_double(1243123.43)?;

    //                     args.extend_from_slice(builder.data())
    //                 }
    //             }
    //             Ok(args)
    //         }
    //         Err(e) => Err(UbusError::ParseArguments(e)),
    //     }
    // }
}
