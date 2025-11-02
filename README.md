ubus (Rust)
===========

**Work in progress**

This is a pure Rust library for implementing [OpenWRT ubus](https://openwrt.org/docs/techref/ubus) clients.

This is a fork of shavac/ubus-rs, using allocations (Vec and String) to convert raw bytes to native rust types.

This makes strong assumption that ubus only use a limit type of Blob, and actual data is always json (`Vec<BlobMsg>`), to make parsing more specific


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

