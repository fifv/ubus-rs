use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use super::*;
use std::path::Path;

pub trait AsyncIoReader: Send + 'static {
    type Error: IOError;
    fn get(
        &mut self,
        data: &mut [u8],
    ) -> impl std::future::Future<Output = Result<(), UbusError>> + Send;
}

pub trait AsyncIoWriter: Send + 'static {
    type Error: IOError;
    fn put(
        &mut self,
        data: &[u8],
    ) -> impl std::future::Future<Output = Result<(), UbusError>> + Send;
}

impl AsyncIoReader for OwnedReadHalf {
    type Error = std::io::Error;
    async fn get(&mut self, data: &mut [u8]) -> Result<(), UbusError> {
        self.read_exact(data)
            .await
            .map_err(UbusError::IO)
            .and(Ok(()))
    }
}
impl AsyncIoWriter for OwnedWriteHalf {
    type Error = std::io::Error;
    async fn put(&mut self, data: &[u8]) -> Result<(), UbusError> {
        self.write_all(data).await.map_err(UbusError::IO)
    }
}

impl Connection {
    pub async fn connect(path: &Path) -> Result<Self, UbusError> {
        Self::new(
            UnixStream::connect(path)
                .await
                .map_err(UbusError::IO)?
                .into_split(),
        )
        .await
    }
    pub async fn connect_ubusd() -> Result<Self, UbusError> {
        Self::connect(Path::new("/var/run/ubus/ubus.sock")).await
    }
}
