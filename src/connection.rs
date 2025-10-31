use crate::*;

use core::panic;
use std::{borrow::ToOwned, collections::HashMap, dbg, string::ToString};
extern crate alloc;
use alloc::string::String;
use std::format;
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
        UbusMsg::from_io(&mut self.io, &mut self.buffer)
    }

    pub fn send(&mut self, message: UbusMsgBuilder) -> Result<(), UbusError> {
        self.io.put(message.into())
    }

    pub fn invoke(
        &mut self,
        obj: u32,
        method: &str,
        args: &[u8],
    ) -> Result<BlobIter<Blob>, UbusError> {
        let header = self.header_by_obj_cmd(obj, UbusCmdType::INVOKE);
        let mut message = UbusMsgBuilder::new(&header).unwrap();
        message.put(UbusMsgAttr::ObjId(obj))?;
        message.put(UbusMsgAttr::Method(method.to_string()))?;

        message.put(UbusMsgAttr::Data(args.to_vec()))?;

        self.send(message)?;

        let mut blob: Option<BlobIter<Blob>> = None;
        'message: loop {
            let message = self.next_message()?;
            if message.header.sequence != header.sequence {
                continue;
            }
            dbg!(&message);

            let attrs = BlobIter::<UbusMsgAttr>::new(message.blob.data);

            match message.header.cmd_type {
                UbusCmdType::STATUS => {
                    for attr in attrs {
                        dbg!(&attr);
                        match attr {
                            UbusMsgAttr::Status(UbusMsgStatus::OK) => {
                                break 'message Ok(blob.expect("why no blob?"));
                            }
                            UbusMsgAttr::Status(status) => return Err(UbusError::Status(status)),
                            _ => continue,
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid status message"));
                }
                UbusCmdType::DATA => {
                    for attr in attrs {
                        dbg!(&attr);
                        if let UbusMsgAttr::Data(data) = attr {
                            blob = Some(BlobIter::<Blob>::new(data));
                            dbg!(&blob);
                            continue 'message;
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid data message"));
                }
                unknown => {
                    std::dbg!(unknown);
                }
            }
        }
    }

    pub fn call<'a>(
        &'a mut self,
        obj_path: &'a str,
        method: &'a str,
        args: &'a str,
    ) -> Result<String, UbusError> {
        let obj_json = self.lookup_object_json(obj_path)?;
        // dbg!(&obj_json);
        let obj: UbusObject = serde_json::from_str(&obj_json)?;
        // dbg!(&obj);
        let args_data = obj.args_from_json(method, args).expect("not valid json");
        dbg!(&args, &args_data);
        let bi = self.invoke(obj.id, method, &args_data)?;
        
        // dbg!(&bi);
        let json = {
            let mut json = String::new();
            json += "{\n";
            let mut first = true;
            for x in bi {
                dbg!(&x);
                if !first {
                    json += ",\n";
                }
                //json_str += &format!("{:?}", x);
                let msg: BlobMsg = x.try_into().unwrap();
                json += &format!("\t{}", msg);
                first = false;
            }
            json += "\n}";
            json
        };


        Ok(json)
    }

    pub fn lookup_object_json<'a>(&'a mut self, obj_path: &'a str) -> Result<String, UbusError> {
        serde_json::to_string_pretty(&self.lookup(obj_path)?)
            .map_err(|e| UbusError::InvalidData("Failed to stringify"))
    }

    pub fn lookup_cb(
        &mut self,
        obj_path: &str,
        mut on_object: impl FnMut(&ObjectResult),
        mut on_signature: impl FnMut(&SignatureResult),
    ) -> Result<(), UbusError> {
        let header = self.header_by_obj_cmd(0, UbusCmdType::LOOKUP);
        let mut request = UbusMsgBuilder::new(&header).unwrap();
        if obj_path.len() != 0 {
            request
                .put(UbusMsgAttr::ObjPath(obj_path.to_string()))
                .unwrap();
        }
        self.send(request)?;

        loop {
            let message = self.next_message()?;
            if message.header.sequence != header.sequence {
                continue;
            }

            let attrs = BlobIter::<UbusMsgAttr>::new(message.blob.data);

            if message.header.cmd_type == UbusCmdType::STATUS {
                for attr in attrs {
                    if let UbusMsgAttr::Status(UbusMsgStatus::OK) = attr {
                        return Ok(());
                    } else if let UbusMsgAttr::Status(status) = attr {
                        return Err(UbusError::Status(status));
                    }
                }
                return Err(UbusError::InvalidData("Invalid status message"));
            }

            if message.header.cmd_type != UbusCmdType::DATA {
                continue;
            }

            let mut obj_path: Option<String> = None;
            let mut obj_id: Option<u32> = None;
            let mut obj_type: Option<u32> = None;
            for attr in attrs {
                match attr {
                    UbusMsgAttr::ObjPath(path) => obj_path = Some(path),
                    UbusMsgAttr::ObjId(id) => obj_id = Some(id),
                    UbusMsgAttr::ObjType(ty) => obj_type = Some(ty),
                    UbusMsgAttr::Signature(nested) => {
                        let object = ObjectResult {
                            path: &obj_path.clone().unwrap(),
                            id: obj_id.unwrap(),
                            ty: obj_type.unwrap(),
                        };
                        on_object(&object);

                        for signature in nested {
                            if let BlobMsgPayload::Table(table) = signature.1 {
                                on_signature(&SignatureResult {
                                    object,
                                    name: signature.0,
                                    args: table
                                        .iter()
                                        .map(|(k, v)| {
                                            if let BlobMsgPayload::Int32(typeid) = *v {
                                                (k.clone(), BlobMsgType::from(typeid as u32))
                                            } else {
                                                panic!()
                                            }
                                        })
                                        .collect(),
                                })
                            }
                        }
                    }
                    _ => continue,
                }
            }
        }
    }

    pub fn lookup_id(&mut self, obj_path: &str) -> Result<u32, UbusError> {
        Ok(self.lookup(obj_path)?.id)
    }

    pub fn lookup(&mut self, obj_path: &str) -> Result<UbusObject, UbusError> {
        let header = self.header_by_obj_cmd(0, UbusCmdType::LOOKUP);
        let mut request = UbusMsgBuilder::new(&header).unwrap();
        if obj_path.len() != 0 {
            request
                .put(UbusMsgAttr::ObjPath(obj_path.to_string()))
                .unwrap();
        }
        self.send(request)?;

        let mut obj = UbusObject::default();

        'message_iter: loop {
            let message = self.next_message()?;
            dbg!(&message);
            if message.header.sequence != header.sequence {
                continue;
            }

            let attrs = BlobIter::<UbusMsgAttr>::new(message.blob.data);
            /* here the `obj` is inserted with some reference from `message`, then if we try to return it, rust ensure `message` should live longer than `obj`   */
            /* i check the code, message is a lot of slice to a global buffer in Connection, each time next_message() got called, the global buffer got overriden */

            match message.header.cmd_type {
                UbusCmdType::STATUS => {
                    for attr in attrs {
                        dbg!(&attr);
                        match attr {
                            UbusMsgAttr::Status(UbusMsgStatus::OK) => break 'message_iter Ok(obj),
                            UbusMsgAttr::Status(status) => return Err(UbusError::Status(status)),
                            _ => continue,
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid status message"));
                }
                UbusCmdType::DATA => {
                    for attr in attrs {
                        dbg!(&attr);
                        match attr {
                            UbusMsgAttr::ObjPath(path) => obj.path = path.to_string(),
                            UbusMsgAttr::ObjId(id) => obj.id = id,
                            UbusMsgAttr::ObjType(ty) => obj.ty = ty,
                            UbusMsgAttr::Signature(nested) => {
                                for item in nested {
                                    let signature = Method {
                                        name: item.0.to_string(),
                                        policy: if let BlobMsgPayload::Table(table) = item.1 {
                                            table
                                                .iter()
                                                .map(|(k, v)| {
                                                    if let BlobMsgPayload::Int32(typeid) = *v {
                                                        (
                                                            k.to_string(),
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
                                    obj.methods.insert(item.0.to_string(), signature);
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

    //  pub fn lookup_object<'a>(&'a mut self, obj_path: &'a str) -> Result<Vec<UbusObject>, UbusError> {
    //     let mut buffer = [0u8; 1024];
    //     let header = self.header_by_obj_cmd(0, UbusCmdType::LOOKUP);
    //     let mut request = UbusMsgBuilder::new(&mut buffer, &header).unwrap();
    //     if obj_path.len() != 0 {
    //         request.put(UbusMsgAttr::ObjPath(obj_path)).unwrap();
    //     }
    //     self.send(request)?;
    //     let mut objs = Vec::new();
    //     loop {
    //         let message = self.next_message()?;
    //         if message.header.sequence != header.sequence {
    //             continue;
    //         }

    //         let attrs = BlobIter::<UbusMsgAttr>::new(message.blob.data);

    //         if message.header.cmd_type == UbusCmdType::STATUS {
    //             for attr in attrs {
    //                 if let UbusMsgAttr::Status(0) = attr {
    //                     return Ok(Vec::new());
    //                 } else if let UbusMsgAttr::Status(status) = attr {
    //                     return Err(UbusError::Status(status));
    //                 }
    //             }
    //             return Err(UbusError::InvalidData("Invalid status message"));
    //         }

    //         if message.header.cmd_type != UbusCmdType::DATA {
    //             continue;
    //         }
    //         let mut obj = UbusObject::default();
    //         for attr in attrs {
    //             match attr {
    //                 UbusMsgAttr::ObjPath(path) => obj.path = path,
    //                 UbusMsgAttr::ObjId(id) => obj.id = id,
    //                 UbusMsgAttr::ObjType(ty) => obj.ty = ty,
    //                 UbusMsgAttr::Signature(nested) => {
    //                     for item in nested {
    //                         let signature = Method {
    //                             name: item.0,
    //                             policy: if let BlobMsgPayload::Table(table) = item.1 {
    //                                 table
    //                                     .iter()
    //                                     .map(|(k, v)| {
    //                                         if let BlobMsgPayload::Int32(typeid) = *v {
    //                                             (*k, BlobMsgType::from(typeid as u32))
    //                                         } else {
    //                                             panic!()
    //                                         }
    //                                     })
    //                                     .collect()
    //                             } else {
    //                                 panic!()
    //                             },
    //                         };
    //                         obj.methods.insert(item.0, signature);
    //                     }
    //                 }
    //                 _ => continue,
    //             }
    //         }
    //         objs.push(obj);
    // }
}
