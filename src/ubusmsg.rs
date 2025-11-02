use crate::{BlobIter, BlobTag, IO, UbusBlob, UbusBlobType, UbusError};
use core::convert::TryInto;
use core::mem::{size_of, transmute};
use serde::{Deserialize, Serialize};
use std::vec;
use std::vec::Vec;
use storage_endian::{BEu16, BEu32};

values!(pub UbusMsgVersion(u8) {
    CURRENT = 0x00,
});

values!(pub UbusCmdType(u8) {
    HELLO           = 0x00,
    STATUS          = 0x01,
    DATA            = 0x02,
    PING            = 0x03,
    LOOKUP          = 0x04,
    INVOKE          = 0x05,
    ADD_OBJECT      = 0x06,
    REMOVE_OBJECT   = 0x07,
    SUBSCRIBE       = 0x08,
    UNSUBSCRIBE     = 0x09,
    NOTIFY          = 0x10,
    MONITOR         = 0x11,
});

values!(pub UbusMsgStatus(i32) {
    OK                    = 0x00,
    INVALID_COMMAND       = 0x01,
    INVALID_ARGUMENT      = 0x02,
    METHOD_NOT_FOUND      = 0x03,
    NOT_FOUND             = 0x04,
    NO_DATA               = 0x05,
    PERMISSION_DENIED     = 0x06,
    TIMEOUT               = 0x07,
    NOT_SUPPORTED         = 0x08,
    UNKNOWN_ERROR         = 0x09,
    CONNECTION_FAILED     = 0x0a,
    NO_MEMORY             = 0x0b,
    PARSE_ERROR           = 0x0c,
    SYSTEM_ERROR          = 0x0d,
});

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct UbusMsgHeader {
    pub version: UbusMsgVersion,
    pub cmd_type: UbusCmdType,
    pub sequence: BEu16,
    pub peer: BEu32,
}

impl UbusMsgHeader {
    pub const SIZE: usize = size_of::<Self>();

    /// Create MessageHeader from a byte array
    pub fn from_bytes(buffer: [u8; Self::SIZE]) -> Self {
        unsafe { transmute(buffer) }
    }
    // Dump out bytes of MessageHeader
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        unsafe { core::mem::transmute(self) }
    }
}

#[derive(Clone)]
pub struct UbusMsg {
    pub header: UbusMsgHeader,
    pub ubus_blobs: Vec<UbusBlob>,
}

impl UbusMsg {
    pub fn from_io<T: IO>(io: &mut T) -> Result<Self, UbusError> {
        /* read ubus message header */
        let mut ubusmsg_header_buffer = [0u8; UbusMsgHeader::SIZE];
        io.get(&mut ubusmsg_header_buffer)?;
        let header = UbusMsgHeader::from_bytes(ubusmsg_header_buffer);
        valid_data!(header.version == UbusMsgVersion::CURRENT, "Wrong version");

        /* read the container blob header */
        let mut ubusmsg_blob_header_buffer = [0u8; BlobTag::SIZE];
        io.get(&mut ubusmsg_blob_header_buffer)?;
        let tag = BlobTag::from_bytes(&ubusmsg_blob_header_buffer);
        tag.is_valid()?;

        /* use the length extracted from blob header, read such length of blob data  */
        let mut ubusmsg_data_buffer = vec![0u8; tag.inner_len()];
        io.get(&mut ubusmsg_data_buffer)?;
        let blobs = BlobIter::new(&ubusmsg_data_buffer)
            .map(|blob| blob.try_into())
            .try_collect::<Vec<UbusBlob>>()?;
        // let blob = UbusBlob::from_tag_and_data(tag, ubusmsg_data_buffer).unwrap();

        Ok(UbusMsg {
            header,
            ubus_blobs: blobs,
        })
    }

    pub fn from_header_and_blobs(header: &UbusMsgHeader, blobs: Vec<UbusBlob>) -> Self {
        Self {
            header: *header,
            ubus_blobs: blobs,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let ubusmsg_header_buf = self.header.to_bytes();

        let mut ubusmsg_blobs_buffer = Vec::new();
        for blob in self.ubus_blobs {
            ubusmsg_blobs_buffer.extend_from_slice(&blob.to_bytes());
        }

        let ubusmsg_blob_header_buffer = BlobTag::try_build(
            UbusBlobType::UNSPEC.value(),
            BlobTag::SIZE + ubusmsg_blobs_buffer.len(),
            false,
        )
        .expect("???")
        .to_bytes();

        let mut raw_msg_data = Vec::new();
        raw_msg_data.extend_from_slice(&ubusmsg_header_buf);
        raw_msg_data.extend_from_slice(&ubusmsg_blob_header_buffer);
        raw_msg_data.extend_from_slice(&ubusmsg_blobs_buffer);
        raw_msg_data
    }
}

impl core::fmt::Debug for UbusMsg {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "Message({:?} seq={}, peer={:08x}, blobs={:?})",
            self.header.cmd_type, self.header.sequence, self.header.peer, self.ubus_blobs
        )
    }
}
