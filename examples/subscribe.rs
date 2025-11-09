use std::{env, path::Path, time::Duration};

use serde_json::json;
use tokio::time::sleep;
use ubus::{MsgTable, UbusServerObjectBuilder};

#[tokio::main]
async fn main() {
    /* enable debug logger */
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    /* connect to ubusd */
    let connection = ubus::Connection::connect_ubusd()
        .await
        .map_err(|err| {
            log::error!("Failed to open ubus socket  ({})", err);
            err
        })
        .unwrap();

    /*
     * subscribe need a server_obj to act as callbacks when notified
     * notification's method name should match this server_obj's method name
     */
    let server_obj1_id = connection
        .add_server(UbusServerObjectBuilder::new("saber").method(
            "click",
            move |req_args: &MsgTable| {
                /* print the notification */
                log::trace!(
                    "click got notified! {}",
                    req_args.to_string_clone().unwrap()
                );
                /* non-sense to reply to a notification */
                json!({}).try_into().unwrap()
            },
        ))
        .await
        .unwrap();

    /* subscribe to a server (note: ubus doesn't have "subscribe to a method") */
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
