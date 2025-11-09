extern crate alloc;
use crate::*;
use alloc::vec::Vec;
use std::{collections::HashMap, string::String, sync::Arc};

pub type UbusMethod = Arc<dyn Fn(&MsgTable) -> MsgTable + Send + Sync>;
// pub trait UbusMethodLike: Fn(&MsgTable) -> MsgTable + Send + Sync + 'static {}
// impl<T> UbusMethodLike for T where T: Fn(&MsgTable) -> MsgTable + Send + Sync + 'static {}

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
    pub id: HexU32,
    pub objtype: HexU32,
    /**
     * used on server side object, the actually callbacks
     */
    pub methods: HashMap<String, UbusMethod>,
}

#[derive(Default)]
pub struct UbusServerObjectBuilder {
    pub path: String,
    /**
     * used on server side object, the actually callbacks
     */
    pub methods: HashMap<String, UbusMethod>,
}

impl UbusServerObjectBuilder {
    pub fn new(obj_path: &str) -> Self {
        Self {
            path: obj_path.into(),
            ..Default::default()
        }
    }
    pub fn method<M: Fn(&MsgTable) -> MsgTable + Send + Sync + 'static>(
        mut self,
        name: &str,
        callback: M,
    ) -> Self {
        self.methods.insert(name.into(), Arc::new(callback));
        self
    }
    pub async fn register(self, conn: &mut Connection) -> Result<u32, UbusError> {
        conn.add_server(self).await
    }
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

/**
 * used in lookup
 */
#[derive(Default, Debug, Clone)]
pub struct UbusObject {
    pub path: String,
    pub id: HexU32,
    pub objtype: HexU32,
    /**
     * used on client side lookup, store what the server says
     */
    pub reported_signature: MsgTable,
}
