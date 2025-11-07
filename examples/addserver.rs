use std::{collections::HashMap, env, path::Path, thread::sleep, time::Duration};

use serde_json::json;
use ubus::{MsgTable, UbusMethod, UbusServerObjectBuilder};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut obj_path = "ttt";
    if args.len() > 1 {
        obj_path = args[1].as_str();
    }
    let socket = Path::new("/var/run/ubus/ubus.sock");

    let mut connection = match ubus::Connection::connect(&socket).await {
        Ok(connection) => connection,
        Err(err) => {
            eprintln!("{}: Failed to open ubus socket. {}", socket.display(), err);
            return;
        }
    };
    fn handle_hi(req_args: &MsgTable) -> MsgTable {
        MsgTable::try_from(r#"{"haha": true}"#).unwrap()
    }
    let some_captured_value = 1;
    let server_obj1 = connection
        .add_server(
            UbusServerObjectBuilder::new(obj_path)
                .method("hi", Box::new(handle_hi))
                .method(
                    "hii",
                    Box::new(|req_args: &MsgTable| {
                        MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()
                    }),
                )
                .method("echo", Box::new(|req_args: &MsgTable| req_args.to_owned()))
                .method(
                    "closure",
                    Box::new(move |req_args: &MsgTable| {
                        json!({"captured-value":some_captured_value})
                            .try_into()
                            .unwrap()
                    }),
                ),
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
        .method(
            "hi",
            Box::new(|req_args: &MsgTable| MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()),
        )
        .register(&mut connection)
        .await
        .unwrap();

    connection.run().await;

    // connection.listening(id).unwrap();
    // sleep(Duration::from_millis(1000000));
    // println!("{:?}", obj);
}
