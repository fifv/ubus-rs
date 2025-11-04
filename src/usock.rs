use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use super::*;
use std::path::Path;

impl AsyncIo for UnixStream {
    type Error = std::io::Error;
    async fn put(&mut self, data: &[u8]) -> Result<(), UbusError> {
        self.write_all(data).await.map_err(UbusError::IO)
    }
    async fn get(&mut self, data: &mut [u8]) -> Result<(), UbusError> {
        self.read_exact(data)
            .await
            .map_err(UbusError::IO)
            .and(Ok(()))
    }
}

impl Connection<UnixStream> {
    pub async fn connect(path: &Path) -> Result<Self, UbusError> {
        Self::new(UnixStream::connect(path).await.map_err(UbusError::IO)?).await
    }
}

impl IOError for std::io::Error {}
impl std::error::Error for Error {}
