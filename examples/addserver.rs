use std::{collections::HashMap, env, path::Path, thread::sleep, time::Duration};

use serde_json::json;
use ubus::{MsgTable, UbusMethod};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut obj_path = "ttt";
    if args.len() > 1 {
        obj_path = args[1].as_str();
    }
    let socket = Path::new("/var/run/ubus/ubus.sock");

    let mut connection = match ubus::Connection::connect(&socket) {
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
    let server_obj = connection
        .add_server(
            obj_path,
            HashMap::<String, UbusMethod>::from([
                ("hi".to_string(), Box::new(handle_hi) as UbusMethod),
                (
                    "hii".to_string(),
                    Box::new(|req_args: &MsgTable| {
                        MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap()
                    }),
                ),
                (
                    "echo".to_string(),
                    Box::new(|req_args: &MsgTable| req_args.to_owned()),
                ),
                (
                    "closure".to_string(),
                    Box::new(move |req_args: &MsgTable| {
                        json!({"captured-value":some_captured_value})
                            .try_into()
                            .unwrap()
                    }),
                ),
            ]),
        )
        .unwrap();
    
    connection.listening(server_obj).unwrap();
    // connection.listening(id).unwrap();
    sleep(Duration::from_millis(1000000));
    // println!("{:?}", obj);
}
