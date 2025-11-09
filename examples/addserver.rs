use std::{env, path::Path, time::Duration};

use serde_json::json;
use tokio::{
    sync::mpsc,
    time::{self, sleep},
};
use ubus::{MsgTable, UbusServerObjectBuilder};

#[tokio::main]
async fn main() {
    /* enable debug logger */
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    /* connect to ubusd */
    let mut connection = ubus::Connection::connect_ubusd()
        .await
        .map_err(|err| {
            log::error!("Failed to open ubus socket  ({})", err);
            err
        })
        .unwrap();

    fn handle_hi(_req_args: &MsgTable) -> MsgTable {
        MsgTable::try_from(r#"{"haha": true}"#).unwrap()
    }
    /* closure can capture */
    let some_closure = |_req_args: &MsgTable| MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap();
    let some_captured_value = 1;
    /*
     * add a server object with some methods, closure with capture is okay
     */
    let server_obj1_id = connection
        .add_server(
            UbusServerObjectBuilder::new("ttt")
                /* a normal function */
                .method("hi", handle_hi)
                /* a closure variable */
                .method("hiii", some_closure)
                /* an inline closure */
                .method("hii", |_req_args: &MsgTable| {
                    MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()
                })
                /* an inline closure, echo request args */
                .method("echo", |req_args: &MsgTable| req_args.to_owned())
                /* a closure with capture */
                .method("closure", move |_req_args: &MsgTable| {
                    json!({"captured-value":some_captured_value})
                        .try_into()
                        .unwrap()
                }),
        )
        .await
        .unwrap();

    /*
     * another way to register a server object
     *
     * it's okay to register multiple server objects
     *  you can use `builder.register(&mut connection)` , this is same as `connection.add_server(builder)`
     */
    let _ = UbusServerObjectBuilder::new("t2")
        .method("hi", |_req_args: &MsgTable| {
            MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()
        })
        .register(&mut connection)
        .await
        .unwrap();

    /* let's notify subscribers. */

    /*
     * you may need to call notify in spawned tasks, which is not easy because the connection can't be cloned
     * one option is wrapping the connection with Arc then clone and move
     * another options is use channel, move tx into tasks and use a loop to receive rx and call notify()
     */
    let (tx, mut rx) = mpsc::channel(4);
    tokio::spawn(async move {
        let mut int = time::interval(Duration::from_millis(1000));
        for i in 0.. {
            int.tick().await;

            let _ = tx.send(json!({"event": "left-click", "count": i})).await;
            // sleep(Duration::from_millis(1000)).await;
        }
    });

    loop {
        let data = rx.recv().await;
        if let Some(data) = data {
            connection
                .notify(server_obj1_id, "click", data.try_into().unwrap())
                .await
                .unwrap();
        }
    }

    /* this does nothing, same as sleep(Forever), prevent connection being dropped */
    connection.run().await;
}
