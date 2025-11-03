extern crate alloc;
use crate::*;
use alloc::vec::Vec;
use std::{boxed::Box, collections::HashMap, string::String};

pub type UbusMethod = Box<dyn Fn(&MsgTable) -> MsgTable + Send + Sync>;
// #[derive(Default, Debug, Clone, Serialize, Deserialize)]
// pub struct Method {
//     pub name: String,
//     pub policy: HashMap<String, BlobMsgType>,
// }
/**
 * it is reasonable that server object can't be cloned
 */
#[derive(Default)]
pub struct UbusServerObject {
    pub path: String,
    pub id: u32,
    pub objtype: u32,
    /**
     * used on server side object, the actually callbacks
     */
    pub methods: HashMap<String, UbusMethod>,
}

impl std::fmt::Debug for UbusServerObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UbusObject")
            .field("path", &self.path)
            .field("id", &self.id)
            .field("objtype", &self.objtype)
            .field("methods", &self.methods.keys().collect::<Vec<_>>())
            .finish()
    }
}


#[derive(Default, Debug, Clone)]
pub struct UbusObject {
    pub path: String,
    pub id: u32,
    pub objtype: u32,
    /**
     * used on client side lookup, store what the server says
     */
    pub reported_signature: MsgTable,
}
