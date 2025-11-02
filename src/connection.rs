use crate::*;

use core::ops::Not;
use core::panic;
use std::println;
use std::{borrow::ToOwned, collections::HashMap, dbg, string::ToString, vec::Vec};
extern crate alloc;
use alloc::string::String;
use std::format;
use std::vec;
use ubuserror::*;

#[derive(Copy, Clone)]
pub struct ObjectResult<'a> {
    pub path: &'a str,
    pub id: u32,
    pub ty: u32,
}
impl core::fmt::Debug for ObjectResult<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{} @0x{:08x} type={:08x}", self.path, self.id, self.ty)
    }
}

pub struct SignatureResult<'a> {
    pub object: ObjectResult<'a>,
    pub name: String,
    pub args: HashMap<String, BlobMsgType>,
}

#[derive(Clone, Copy)]
pub struct Connection<T: IO> {
    io: T,
    peer: u32,
    sequence: u16,
    buffer: [u8; 64 * 1024],
}

impl<T: IO> Connection<T> {
    /// Create a new ubus connection from an existing IO
    pub fn new(io: T) -> Result<Self, UbusError> {
        let mut conn = Self {
            io,
            peer: 0,
            sequence: 0,
            buffer: [0u8; 64 * 1024],
        };

        // ubus server should say hello on connect
        let message = conn.next_message()?;

        // Verify the header is what we expect
        valid_data!(
            message.header.cmd_type == UbusCmdType::HELLO,
            "Expected hello"
        );

        // Record our peer id
        conn.peer = message.header.peer.into();

        Ok(conn)
    }

    fn header_by_obj_cmd(&mut self, obj_id: u32, cmd: UbusCmdType) -> UbusMsgHeader {
        self.sequence += 1;
        UbusMsgHeader {
            version: UbusMsgVersion::CURRENT,
            cmd_type: cmd,
            sequence: self.sequence.into(),
            peer: obj_id.into(),
        }
    }

    // Get next message from ubus channel (blocking!)
    pub fn next_message(&mut self) -> Result<UbusMsg, UbusError> {
        UbusMsg::from_io(&mut self.io)
    }

    pub fn send(&mut self, message: UbusMsg) -> Result<(), UbusError> {
        // self.io.put(&Into::<Vec<u8>>::into(message))
        self.io.put(&message.to_bytes())
    }

    pub fn invoke(
        &mut self,
        obj: u32,
        method: &str,
        req_args: MsgTable,
    ) -> Result<MsgTable, UbusError> {
        let header = self.header_by_obj_cmd(obj, UbusCmdType::INVOKE);
        let req_blobs: Vec<UbusBlob> = vec![
            UbusBlob::ObjId(obj as i32),
            UbusBlob::Method(method.to_string()),
            UbusBlob::Data(req_args),
        ];
        let message = UbusMsg::from_header_and_blobs(&header, req_blobs);
        // let mut message = UbusMsg::new(&header).unwrap();
        // message.put(UbusMsgAttr::ObjId(obj))?;
        // message.put(UbusMsgAttr::Method(method.to_string()))?;

        // message.put(UbusMsgAttr::Data(req_args.to_vec()))?;

        self.send(message)?;

        let mut res_args = MsgTable::new();
        /* Normally we will get a UbusCmdType::DATA then a UbusCmdType::STATUS */
        'messages: loop {
            let message = self.next_message()?;
            if message.header.sequence != header.sequence {
                continue;
            }
            dbg!(&message);

            match message.header.cmd_type {
                UbusCmdType::STATUS => {
                    for blob in message.ubus_blobs {
                        match blob {
                            UbusBlob::Status(UbusMsgStatus::OK) => {
                                break 'messages Ok(res_args);
                            }
                            UbusBlob::Status(status) => {
                                return Err(UbusError::Status(status));
                            }
                            _ => continue,
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid status message"));
                }
                UbusCmdType::DATA => {
                    for blob in message.ubus_blobs {
                        // dbg!(&blob);
                        if let UbusBlob::Data(data) = blob {
                            res_args = data;
                            // dbg!(&res_args);
                            continue 'messages;
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid data message"));
                }
                unknown => {
                    dbg!(unknown);
                }
            }
        }
    }

    pub fn call<'a>(
        &'a mut self,
        obj_path: &'a str,
        method: &'a str,
        req_args: &'a str,
    ) -> Result<String, UbusError> {
        let obj_json = self.lookup_object_json(obj_path)?;
        // dbg!(&obj_json);
        let obj: UbusObject = serde_json::from_str(&obj_json)?;
        // dbg!(&obj);
        // let req_args = obj.args_from_json(method, args).expect("not valid json");
        let req_args = MsgTable::try_from(req_args).unwrap();
        // dbg!(&args, &req_args);
        let res_args = self.invoke(obj.id, method, req_args)?;

        Ok(dbg!(res_args.try_into()?))

        // dbg!(&bi);

        // let json = {
        //     let mut json = String::new();
        //     json += "{\n";
        //     let mut first = true;
        //     for msg in res_args.0 {
        //         // dbg!(&msg);
        //         if !first {
        //             json += ",\n";
        //         }
        //         //json_str += &format!("{:?}", x);
        //         json += &format!("\t{}", msg);
        //         first = false;
        //     }
        //     json += "\n}";
        //     json
        // };
        // Ok(json)
    }

    pub fn lookup_object_json<'a>(&'a mut self, obj_path: &'a str) -> Result<String, UbusError> {
        serde_json::to_string_pretty(&self.lookup(obj_path)?)
            .map_err(|e| UbusError::InvalidData("Failed to stringify"))
    }


    pub fn lookup_id(&mut self, obj_path: &str) -> Result<u32, UbusError> {
        Ok(self.lookup(obj_path)?.id)
    }

    pub fn lookup(&mut self, obj_path: &str) -> Result<UbusObject, UbusError> {
        let header = self.header_by_obj_cmd(0, UbusCmdType::LOOKUP);

        let req_blobs: Vec<UbusBlob> = obj_path
            .is_empty()
            .not()
            .then(|| UbusBlob::ObjPath(obj_path.to_string()))
            .into_iter()
            .collect();

        let request = UbusMsg::from_header_and_blobs(&header, req_blobs);
        self.send(request)?;

        let mut obj = UbusObject::default();

        'message_iter: loop {
            let message = self.next_message()?;
            dbg!(&message);
            // println!("{:#?}", &message);
            if message.header.sequence != header.sequence {
                continue;
            }

            /* here the `obj` is inserted with some reference from `message`, then if we try to return it, rust ensure `message` should live longer than `obj`   */
            /* i check the code, message is a lot of slice to a global buffer in Connection, each time next_message() got called, the global buffer got overriden */

            match message.header.cmd_type {
                UbusCmdType::STATUS => {
                    for blob in message.ubus_blobs {
                        // dbg!(&blob);
                        match blob {
                            UbusBlob::Status(UbusMsgStatus::OK) => {
                                break 'message_iter Ok(obj);
                            }
                            UbusBlob::Status(status) => {
                                return Err(UbusError::Status(status));
                            }
                            _ => continue,
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid status message"));
                }
                UbusCmdType::DATA => {
                    for blob in message.ubus_blobs {
                        match blob {
                            UbusBlob::ObjPath(path) => obj.path = path.to_string(),
                            UbusBlob::ObjId(id) => obj.id = id as u32,
                            UbusBlob::ObjType(ty) => obj.ty = ty as u32,
                            UbusBlob::Signature(nested) => {
                                // dbg!(&nested);
                                for item in nested.0 {
                                    let signature = Method {
                                        name: item.name.to_string(),
                                        policy: if let BlobMsgPayload::Table(table) = item.data {
                                            table
                                                .iter()
                                                .map(|blogmsg| {
                                                    if let BlobMsgPayload::Int32(typeid) =
                                                        blogmsg.data
                                                    {
                                                        (
                                                            blogmsg.name.to_string(),
                                                            BlobMsgType::from(typeid as u32),
                                                        )
                                                    } else {
                                                        panic!()
                                                    }
                                                })
                                                .collect()
                                        } else {
                                            panic!()
                                        },
                                    };
                                    obj.methods.insert(item.name.to_string(), signature);
                                }
                            }
                            _ => continue,
                        }
                    }
                }
                _ => {
                    continue;
                }
            }
        }
    }

}
