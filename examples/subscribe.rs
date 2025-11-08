use std::{env, path::Path, time::Duration};

use serde_json::json;
use tokio::time::sleep;
use ubus::{MsgTable, UbusServerObjectBuilder};

#[tokio::main]
async fn main() {
    /* enable debug logger */
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    let mut connection = ubus::Connection::connect_ubusd()
        .await
        .map_err(|err| {
            log::error!("Failed to open ubus socket  ({})", err);
            err
        })
        .unwrap();

    let some_captured_value = 1;
    let server_obj1_id = connection
        .add_server(UbusServerObjectBuilder::new("saber").method(
            "click",
            move |req_args: &MsgTable| {
                log::trace!(
                    "click got notified! {}",
                    req_args.to_string_clone().unwrap()
                );
                json!({"captured-value":some_captured_value})
                    .try_into()
                    .unwrap()
            },
        ))
        .await
        .unwrap();

    let server_obj_id = connection.lookup_id("ttt").await.unwrap().into();
    connection
        .subscribe(server_obj1_id.into(), server_obj_id)
        .await
        .unwrap();
    /* this do nothing, same as sleep(Forever) */
    connection.run().await;

    // connection.listening(id).unwrap();
    // sleep(Duration::M);
    // println!("{:?}", obj);
}
