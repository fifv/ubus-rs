ubus (Rust)
===========

**Work in progress**

This is a pure Rust library for implementing [OpenWRT ubus](https://openwrt.org/docs/techref/ubus) clients.


Technical Notes
---------

This is a fork of shavac/ubus-rs, using allocations (Vec and String) to convert raw bytes to native rust types.

This makes strong assumption that `ubus` only use limited size of `type`s of Blob, and actual data is always json (`Vec<BlobMsg>`), to make parsing more specific:

* `Blob` with its most-significat-bit `1` is `BlobMsg`, which can represents json data
* `Blob` with its most-significat-bit `0` is `UbusBlob`, which has fixed size of `type`s, no other `type`s of Blob used in `ubus`, except the container `Blob` in `UbusMsg`
* `UbusMsg` always has a `UbusMsgHeader`, followed by a giant container `Blob` that has `type 0`
* The container `Blob` contains multiple `UbusBlob`s, and `UbusBlob`s' payload data type is tied to `UbusBlob`s' `type`
  * e.g. If `UbusBlob` has `type STATUS`, then its payload is a `u32`
  * e.g. If `UbusBlob` has `type DATA`, then its payload is multiple `BlobMsg`s, which can be converted to one json object




Supported
---------

* High-level abstraction for `lookup` command
* High level abstraction for `call` command
* JSON support

TODO
----

* Async with Tokio
* High level abstraction for server object
* High level abstraction for `subscribe`/`unsubscribe` commands

