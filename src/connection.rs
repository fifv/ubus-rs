use crate::*;

use core::ops::Not;
use std::{collections::HashMap, dbg, string::ToString, vec::Vec};
extern crate alloc;
use alloc::string::String;
use std::vec;
use storage_endian::BigEndian;
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
        UbusMsgHeader {
            version: UbusMsgVersion::CURRENT,
            cmd_type: cmd,
            sequence: self.generate_new_request_sequence(),
            peer: obj_id.into(),
        }
    }

    /**
     * sequence is used to identify session, only client need to generate it, server should reply with same sequence with request's
     */
    fn generate_new_request_sequence(&mut self) -> BigEndian<u16> {
        self.sequence += 1;
        BigEndian::<u16>::from(self.sequence)
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
        let request_sequence = self.generate_new_request_sequence();

        self.send(UbusMsg {
            header: UbusMsgHeader {
                version: UbusMsgVersion::CURRENT,
                cmd_type: UbusCmdType::INVOKE,
                sequence: request_sequence,
                peer: obj.into(),
            },
            ubus_blobs: vec![
                UbusBlob::ObjId(obj),
                UbusBlob::Method(method.to_string()),
                UbusBlob::Data(req_args),
            ],
        })?;

        // FIXME: use Option<>
        let mut reply_args = MsgTable::new();
        /* Normally we will get a UbusCmdType::DATA then a UbusCmdType::STATUS */
        'messages: loop {
            let message = self.next_message()?;
            if message.header.sequence != request_sequence {
                // FIXME:
                // continue;
            }
            dbg!(&message);

            match message.header.cmd_type {
                UbusCmdType::STATUS => {
                    for blob in message.ubus_blobs {
                        match blob {
                            UbusBlob::Status(UbusMsgStatus::OK) => {
                                break 'messages Ok(reply_args);
                            }
                            UbusBlob::Status(status) => {
                                return Err(UbusError::Status(status));
                            }
                            _ => {}
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid status message"));
                }
                UbusCmdType::DATA => {
                    for blob in message.ubus_blobs {
                        // dbg!(&blob);
                        if let UbusBlob::Data(data) = blob {
                            reply_args = data;
                            // dbg!(&reply_args);
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
        // let obj_json = self.lookup_object_json(obj_path)?;
        // // dbg!(&obj_json);
        // let obj: UbusObject = serde_json::from_str(&obj_json)?;
        // dbg!(&obj);
        // let req_args = obj.args_from_json(method, args).expect("not valid json");
        let obj_id = self.lookup_id(obj_path)?;
        let req_args = MsgTable::try_from(req_args)?;
        // dbg!(&args, &req_args);
        let reply_args = self.invoke(obj_id, method, req_args)?;

        Ok(dbg!(reply_args.try_into()?))

        // dbg!(&bi);

        // let json = {
        //     let mut json = String::new();
        //     json += "{\n";
        //     let mut first = true;
        //     for msg in reply_args.0 {
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

    // pub fn lookup_object_json<'a>(&'a mut self, obj_path: &'a str) -> Result<String, UbusError> {
    //     serde_json::to_string_pretty(&self.lookup(obj_path)?.get(0))
    //         .map_err(|e| UbusError::InvalidData("Failed to stringify"))
    // }

    pub fn lookup_id(&mut self, obj_path: &str) -> Result<u32, UbusError> {
        Ok(self
            .lookup(obj_path)?
            .get(0)
            .ok_or(UbusError::InvalidPath(obj_path.to_string()))?
            .id)
    }

    pub fn lookup(&mut self, obj_path: &str) -> Result<Vec<UbusObject>, UbusError> {
        let request_sequence = self.generate_new_request_sequence();

        self.send(UbusMsg {
            header: UbusMsgHeader {
                version: UbusMsgVersion::CURRENT,
                cmd_type: UbusCmdType::LOOKUP,
                sequence: request_sequence,
                peer: 0.into(),
            },
            ubus_blobs: obj_path
                .is_empty()
                .not()
                .then(|| UbusBlob::ObjPath(obj_path.to_string()))
                .into_iter()
                .collect(),
        })?;

        let objs = {
            let mut objs = Vec::new();
            /* TODO: optimize logic, too much mut, too much duplicate! */
            'message_iter: loop {
                let message = self.next_message()?;
                dbg!(&message);
                // println!("{:#?}", &message);
                if message.header.sequence != request_sequence {
                    continue;
                }

                /* here the `obj` is inserted with some reference from `message`, then if we try to return it, rust ensure `message` should live longer than `obj`   */
                /* i check the code, message is a lot of slice to a global buffer in Connection, each time next_message() got called, the global buffer got overriden */
                let mut obj = UbusObject::default();

                match message.header.cmd_type {
                    UbusCmdType::STATUS => {
                        for blob in message.ubus_blobs {
                            // dbg!(&blob);
                            match blob {
                                UbusBlob::Status(UbusMsgStatus::OK) => {
                                    break 'message_iter Ok(objs);
                                }
                                UbusBlob::Status(status) => {
                                    break 'message_iter Err(UbusError::Status(status));
                                }
                                _ => {}
                            }
                        }
                        return Err(UbusError::InvalidData("Invalid status message"));
                    }
                    UbusCmdType::DATA => {
                        for blob in message.ubus_blobs {
                            match blob {
                                UbusBlob::ObjPath(path) => obj.path = path.to_string(),
                                UbusBlob::ObjId(id) => obj.id = id as u32,
                                UbusBlob::ObjType(ty) => obj.objtype = ty as u32,
                                UbusBlob::Signature(nested) => {
                                    obj.reported_signature = nested;
                                }
                                _ => {}
                            }
                        }
                        objs.push(obj);
                    }
                    _ => {}
                }
            }
        };
        objs
    }

    /**
     * send:        add_object: {"objpath":"test","signature":{"hello":{"id":5,"msg":3},"watch":{"id":5,"counter":5},"count":{"to":5,"string":3}}}
     * return:      data:       {"objid":2013531835,"objtype":-1292016789}
     */
    pub fn add_server(
        &mut self,
        obj_path: &str,
        methods: HashMap<String, UbusMethod>,
    ) -> Result<(), UbusError> {
        let mut server_obj = UbusServerObject::default();
        server_obj.methods = methods;

        // FIXME\: official ubus cli call stuck while data in monitor looks good <- fixed: replied seq should be same as requested
        {
            let request_sequence = self.generate_new_request_sequence();
            self.send(UbusMsg {
                header: UbusMsgHeader {
                    version: UbusMsgVersion::CURRENT,
                    cmd_type: UbusCmdType::ADD_OBJECT,
                    sequence: request_sequence,
                    peer: 0.into(),
                },
                ubus_blobs: vec![
                    UbusBlob::ObjPath(obj_path.to_string()),
                    UbusBlob::Signature(
                        server_obj
                            .methods
                            .iter()
                            .map(|(method, cb)| BlobMsg {
                                name: method.to_string(),
                                data: BlobMsgPayload::Table(Vec::new()),
                            })
                            .collect::<Vec<BlobMsg>>()
                            .into(),
                    ),
                ],
            })?;

            /* Normally we will get a UbusCmdType::DATA then a UbusCmdType::STATUS */
            let reply_args = 'message_loop: loop {
                let message = self.next_message()?;
                if message.header.sequence != request_sequence {
                    continue;
                }
                dbg!(&message);

                match message.header.cmd_type {
                    UbusCmdType::STATUS => {
                        for blob in message.ubus_blobs {
                            match blob {
                                UbusBlob::Status(UbusMsgStatus::OK) => {
                                    break 'message_loop Ok(());
                                }
                                UbusBlob::Status(status) => {
                                    break 'message_loop Err(UbusError::Status(status));
                                }
                                _ => {}
                            }
                        }
                        break 'message_loop Err(UbusError::InvalidData("Invalid status message"));
                    }
                    UbusCmdType::DATA => {
                        for blob in message.ubus_blobs {
                            // dbg!(&blob);
                            match blob {
                                UbusBlob::ObjId(id) => server_obj.id = id,
                                UbusBlob::ObjType(objtype) => server_obj.objtype = objtype,
                                _ => todo!(),
                            }
                        }
                    }
                    unknown => {
                        dbg!(unknown);
                    }
                }
            };
        }

        /* Normally we will get a UbusCmdType::DATA then a UbusCmdType::STATUS */
        'message_loop: loop {
            let message = self.next_message()?;
            // if message.header.sequence != header.sequence {
            //     continue;
            // }
            dbg!(&message);

            match message.header.cmd_type {
                /*
                 * server object normally won't got a status, instead, server will send back a status OK to terminate client
                 */
                UbusCmdType::STATUS => {
                    for blob in message.ubus_blobs {
                        match blob {
                            UbusBlob::Status(UbusMsgStatus::OK) => {
                                // break 'messages Ok(());
                            }
                            UbusBlob::Status(status) => {
                                return Err(UbusError::Status(status));
                            }
                            _ => {}
                        }
                    }
                    return Err(UbusError::InvalidData("Invalid status message"));
                }
                UbusCmdType::INVOKE => {
                    /*
                     * client's INVOKE contains:
                     *      - `message.header.peer`         : the client's obj_id, should be used as `message.header.peer` when reply
                     *      - `message.header.sequence`     : used to identify current session, should be used as `message.header.sequence` when reply
                     *      - `message.ubus_blobs.?.ObjId`  : current server obj_id, should be used as `message.ubus_blobs.?.ObjId` when reply.
                     *                                        this is same as the response from add_server
                     *      - `message.ubus_blobs.?.Method` : client want to call this method
                     *      - `message.ubus_blobs.?.Data`   : client requested with this json
                     */
                    // TODO: use Option
                    let (client_obj_id, method_name, req_args) = {
                        let mut client_obj_id = 0;
                        let mut method_name = String::new();
                        let mut req_args = MsgTable::new();
                        for blob in message.ubus_blobs {
                            // dbg!(&blob);
                            match blob {
                                UbusBlob::ObjId(id) => client_obj_id = id,
                                UbusBlob::Method(method) => method_name = method,
                                UbusBlob::Data(msg_table) => req_args = msg_table,
                                _ => {}
                            }
                        }
                        (client_obj_id, method_name, req_args)
                    };

                    /* reply to client */

                    match server_obj.methods.get(&method_name) {
                        Some(method) => {
                            let reply_args = method(&req_args);
                            /* here client_obj_id == server objid */
                            self.send(UbusMsg::from_header_and_blobs(
                                &UbusMsgHeader {
                                    version: UbusMsgVersion::CURRENT,
                                    cmd_type: UbusCmdType::DATA,
                                    sequence: message.header.sequence,
                                    peer: message.header.peer,
                                },
                                vec![
                                    UbusBlob::ObjId(client_obj_id),
                                    // UbusBlob::Data(MsgTable::try_from(json!({
                                    //     "wtf": 1
                                    // }))?),
                                    UbusBlob::Data(reply_args),/* data is moved to enum, then moved to UbusMsg */
                                ],
                            ))?;

                            // dbg!(reply_args);

                            // sleep(Duration::from_millis(400));

                            self.send(UbusMsg::from_header_and_blobs(
                                &UbusMsgHeader {
                                    version: UbusMsgVersion::CURRENT,
                                    cmd_type: UbusCmdType::STATUS,
                                    sequence: message.header.sequence,
                                    peer: message.header.peer,
                                },
                                vec![
                                    UbusBlob::ObjId(client_obj_id),
                                    UbusBlob::Status(UbusMsgStatus::OK),
                                ],
                            ))?;
                        }
                        None => {
                            self.send(UbusMsg::from_header_and_blobs(
                                &UbusMsgHeader {
                                    version: UbusMsgVersion::CURRENT,
                                    cmd_type: UbusCmdType::STATUS,
                                    sequence: message.header.sequence,
                                    peer: message.header.peer,
                                },
                                vec![
                                    UbusBlob::ObjId(client_obj_id),
                                    UbusBlob::Status(UbusMsgStatus::METHOD_NOT_FOUND),
                                ],
                            ))?;
                        }
                    }
                }
                unknown => {
                    dbg!(unknown);
                }
            }
        }
    }

    /*
     * server:
     * receive: invoke: {"objid":2013531835,"method":"hello","data":{"msg":"fsdfsdf"},"user":"fifv","group":"fifv"}
     * reply:   data:   {"objid":2013531835,"data":{"message":"test received a message: fsdfsdf"}}
     * reply:   status: {"status":0,"objid":2013531835}
     */
    // pub fn listening(&mut self, objid: u32) -> Result<(), UbusError> {
    // }
}
