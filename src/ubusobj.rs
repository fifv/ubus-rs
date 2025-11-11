extern crate alloc;
use crate::*;
use alloc::vec::Vec;
use core::pin::Pin;
use std::{boxed::Box, collections::HashMap, string::String, sync::Arc};

pub type UbusMethodSync = Arc<dyn Fn(MsgTable) -> MsgTable + Send + Sync>;
pub type UbusMethodAsync =
    Arc<dyn Fn(MsgTable) -> Pin<Box<dyn Future<Output = MsgTable> + Send>> + Send + Sync>;
// pub trait UbusMethodLike: Fn(&MsgTable) -> MsgTable + Send + Sync + 'static {}
// impl<T> UbusMethodLike for T where T: Fn(&MsgTable) -> MsgTable + Send + Sync + 'static {}

#[derive(Clone)]
pub enum UbusMethod {
    Sync(UbusMethodSync),
    Async(UbusMethodAsync),
}

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
    // pub methods_async: HashMap<String, UbusMethodAsync>,
}

#[derive(Default)]
pub struct UbusServerObjectBuilder {
    pub path: String,
    /**
     * used on server side object, the actually callbacks
     */
    pub methods: HashMap<String, UbusMethod>,
    // pub methods_async: HashMap<String, UbusMethodAsync>,
}

impl UbusServerObjectBuilder {
    pub fn new(obj_path: &str) -> Self {
        Self {
            path: obj_path.into(),
            ..Default::default()
        }
    }
    pub fn method<M: Fn(MsgTable) -> MsgTable + Send + Sync + 'static>(
        mut self,
        name: &str,
        callback: M,
    ) -> Self {
        self.methods.insert(
            name.into(),
            UbusMethod::Sync(Arc::new(callback)),
            // Arc::new( |args: &MsgTable|{ Arc::pin(async {callback(args).await})}),
        );
        self
    }

    // pub fn method_async<M: AsyncFn(MsgTable) -> MsgTable + Sync + Send + 'static>(
    //     mut self,
    //     name: &str,
    //     callback: M,
    // ) -> Self {
    //     self.methods_async
    //         .insert(name.into(), Arc::new(move |msg| Box::pin(callback(msg))));
    //     self
    // }

    pub fn method_async<
        M: Fn(MsgTable) -> Fut + Sync + Send + 'static,
        Fut: Future<Output = MsgTable> + Send + Sync + 'static,
    >(
        mut self,
        name: &str,
        callback: M,
    ) -> Self {
        self.methods
            .insert(name.into(), UbusMethod::Async(Arc::new(move |msg| Box::pin(callback(msg)))));
        self
    }

    // pub fn method_async<M, Fut>(mut self, name: &str, callback: M) -> Self
    // where
    //     M: Fn(MsgTable) -> Fut + Send + Sync + 'static,
    //     Fut: Future<Output = MsgTable> + Send + 'static,
    // {
    //     let func: UbusAsyncMethod = Arc::new(move |msg| Box::pin(callback(msg)));
    //     self.methods_async.insert(name.into(), func);
    //     self
    // }

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
