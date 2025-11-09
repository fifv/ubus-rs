#![no_std]
#![allow(dead_code)]
#![feature(iterator_try_collect)]
#![feature(random)]

#[cfg(not(no_std))]
extern crate std;

/**
 * TODO:
 * - Reduce Copy
 * - Reduce Alloc
 * - Better Readibility
 * - Tests
 */
/* communicate with ubusd */
mod connection;
mod usock;
/* the types used in ubus and convertion between raw bytes and rust types  */
mod blob;
mod blobmsg;
mod ubusblob;
mod ubusmsg;
mod ubusobj;
/* utilities */
mod ubuserror;
mod utils;

pub use blob::*;
pub use blobmsg::*;
pub use connection::*;
pub use ubusblob::*;
pub use ubuserror::*;
pub use ubusmsg::*;
pub use ubusobj::*;
// pub use utils::*;

// use crate::values;
