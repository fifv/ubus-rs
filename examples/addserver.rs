use std::{env, path::Path, time::Duration};

use serde_json::json;
use tokio::time::sleep;
use ubus::{MsgTable, UbusServerObjectBuilder};

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    let args: Vec<String> = env::args().collect();
    let mut obj_path = "ttt";
    if args.len() > 1 {
        obj_path = args[1].as_str();
    }
    let socket = Path::new("/var/run/ubus/ubus.sock");

    let mut connection = match ubus::Connection::connect(&socket).await {
        Ok(connection) => connection,
        Err(err) => {
            log::error!(
                "Failed to open ubus socket: {}  ({})",
                socket.display(),
                err
            );
            return;
        }
    };
    fn handle_hi(_req_args: &MsgTable) -> MsgTable {
        MsgTable::try_from(r#"{"haha": true}"#).unwrap()
    }
    let some_captured_value = 1;
    let server_obj1_id = connection
        .add_server(
            UbusServerObjectBuilder::new(obj_path)
                .method("hi", handle_hi)
                .method("hii", |_req_args: &MsgTable| {
                    MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()
                })
                .method("echo", |req_args: &MsgTable| req_args.to_owned())
                .method("closure", move |_req_args: &MsgTable| {
                    json!({"captured-value":some_captured_value})
                        .try_into()
                        .unwrap()
                }),
        )
        .await
        .unwrap();

    /*
     *  it's okay to register multiple server objects
     *
     *  you can use `builder.register(&mut connection)` , this is same as `connection.add_server(builder)`
     *
     */
    let _ = UbusServerObjectBuilder::new("t2")
        .method("hi", |_req_args: &MsgTable| {
            MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()
        })
        .register(&mut connection)
        .await
        .unwrap();

    loop {
        connection
            .notify(
                server_obj1_id,
                "click",
                json!({"event": "left-click"}).try_into().unwrap(),
            )
            .await
            .unwrap();
        // sleep(Duration::from_millis(1000)).await;
        sleep(Duration::from_millis(3000)).await;
    }

    log::error!("?");
    /* this do nothing, same as sleep(Forever) */
    connection.run().await;

    // connection.listening(id).unwrap();
    // sleep(Duration::M);
    // println!("{:?}", obj);
}
