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
    #[error("Reply Timeout")]
    ReplyTimeout(),
}

pub trait IOError {}
impl IOError for std::io::Error {}
impl std::error::Error for Error {}

#[derive(Debug)]
pub enum NoIO {}
impl core::fmt::Display for NoIO {
    fn fmt(&self, _f: &mut core::fmt::Formatter) -> core::fmt::Result {
        unreachable!()
    }
}

#[derive(Debug)]
pub enum Error<T = NoIO> {
    IO(T),
    InvalidData(&'static str),
    Status(i32),
}

impl<T: core::fmt::Display> core::fmt::Display for Error<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use Error::*;
        match self {
            IO(e) => write!(f, "IO Error: {}", e),
            InvalidData(e) => write!(f, "Invalid Data: {}", e),
            Status(e) => write!(f, "Ubus Status: {}", e),
        }
    }
}

impl<T: IOError> From<Error<NoIO>> for Error<T> {
    fn from(e: Error<NoIO>) -> Self {
        use Error::*;
        match e {
            IO(_) => unreachable!(),
            InvalidData(v) => InvalidData(v),
            Status(v) => Status(v),
        }
    }
}

impl<T> From<core::convert::Infallible> for Error<T> {
    fn from(_: core::convert::Infallible) -> Self {
        unreachable!()
    }
}
