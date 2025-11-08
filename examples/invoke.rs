use std::path::Path;

use serde_json::json;
use ubus::MsgTable;

#[tokio::main]
async fn main() {
    /* enable debug logger */
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    /* -1- connect to ubusd */
    let mut connection = ubus::Connection::connect_ubusd()
        .await
        .map_err(|err| {
            log::error!("Failed to open ubus socket  ({})", err);
            err
        })
        .unwrap();

    /* -2- use the obj_path to lookup for obj_id. there is a `.call()` which does lookup for you */
    let server_obj_id = connection.lookup_id("ttt").await.unwrap();

    /* -3- invoke with found server_obj_id, method name, and json args */
    let reply_args = connection
        .invoke(
            server_obj_id,
            "echo",
            json!({"some": "value"}).try_into().unwrap(),
        )
        .await
        .unwrap();

    /* -3- you can also use json string as args */
    let reply_args = connection
        .invoke(
            server_obj_id,
            "echo",
            r#"{"id":1,"msg":"a41234123"}"#.try_into().unwrap(),
        )
        .await
        .unwrap();

    /* -4- use the response, or ignore it */
    println!("{}", reply_args.to_string_pretty().unwrap());
}
