extern crate alloc;
use core::str::Utf8Error;
use std::{io, string::FromUtf8Error};

use alloc::string::String;
use thiserror::Error;

use crate::UbusBlobType;

#[derive(Debug, Error)]
pub enum UbusError {
    #[error("io error")]
    IO(#[from] io::Error),
    #[error("Invalid decoding string")]
    Utf8(#[from] Utf8Error),
    #[error("Invalid decoding string")]
    FromUtf8(#[from] FromUtf8Error),
    #[error("Invalid Data")]
    InvalidData(&'static str),
    #[error("Ubus return ErrorCode({0})")]
    Status(crate::UbusMsgStatus),
    #[error("Error parse arguments string:{0}")]
    ParseArguments(#[from] serde_json::Error),
    #[error("Invalid method:{0}")]
    InvalidMethod(String),
    #[error("Invalid blog type:{0}")]
    InvalidBlobType(UbusBlobType),
    #[error("No such path:{0}")]
    InvalidPath(String),
    #[error("Channel closed")]
    UnexpectChannelClosed(),
}
