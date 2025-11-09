ubus (Rust)
===========

**Work in progress**

This is a pure Rust library for implementing [OpenWRT ubus](https://openwrt.org/docs/techref/ubus) clients.



Quick Try
---------

1. make sure `ubusd` is running, and you are using `root` to run examples (or hack `ubusd`'s `ubusd_acl_check()`) to skip root check, see above
2. make sure your rust toolchain is nightly, because I use the `try_collect()` features
3. `cargo run --example=addserver`   - start the server
4. `cargo run --example=invoke`      - invoke server's method and print result
5. `cargo run --example=ubuscall ttt echo '{"1": true}'`   - another way to invoke server's method and print result
6. `cargo run --example=subscribe`   - subscribe to server and get notifications

Supported
---------

* High-level abstraction for `lookup` command
* High level abstraction for `call` command
* High level abstraction for server object
* High level abstraction for `subscribe` / `notify` commands
* Async with Tokio
* JSON support
* Strongly typed result

TODO
----

* `unsubscribe` / `remove_server`
* handle IO/channel errors better instead of panic



Technical Notes
---------

This is a fork of shavac/ubus-rs, using allocations (Vec and String) to convert raw bytes to native rust types.

There are almost none documention about libubus, especially about server object. And the libubus source code has no comments at all... I draw a [Figma](https://www.figma.com/board/i1VP9w6dgs5TVCgWswYOmh/ubus-On-Wire) from what I learned by reading source code and debugging.

This makes strong assumption that `ubus` only use limited size of `type`s of Blob, and actual data is always json (`Vec<BlobMsg>`), to make parsing more specific:

* `Blob` with its most-significat-bit `1` is `BlobMsg`, which can represents json data
* `Blob` with its most-significat-bit `0` is `UbusBlob`, which has fixed size of `type`s, no other `type`s of Blob used in `ubus`, except the container `Blob` in `UbusMsg`
* `UbusMsg` always has a `UbusMsgHeader`, followed by a giant container `Blob` that has `type 0`
* The container `Blob` contains multiple `UbusBlob`s, and `UbusBlob`s' payload data type is tied to `UbusBlob`s' `type`
  * e.g. If `UbusBlob` has `type STATUS`, then its payload is a `u32`
  * e.g. If `UbusBlob` has `type DATA`, then its payload is multiple `BlobMsg`s, which can be converted to one json object


Seems only root can connect to `ubusd`? To tests and development, I add an early `return 0;` to beginning of `ubusd_acl.c` -> `ubusd_acl_check()` in `ubusd` to skip auth.

Signature varification is skipped, (`ubusd` also doesn't care about it), making transfer any valid json possible. This is the behaviour of `libubus` and `ubus` cli.

Seems `ubusd` even doesn't care about method existence, its server object's responsibilty to return a `method not found` status.