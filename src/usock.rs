use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use super::*;
use std::path::Path;

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
}

impl IOError for std::io::Error {}
impl std::error::Error for Error {}
