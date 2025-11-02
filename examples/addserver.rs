use std::{collections::HashMap, env, path::Path, thread::sleep, time::Duration};

use ubus::{MethodCallback, MsgTable, UbusError, UbusObject};

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
        MsgTable::try_from(r#"{"haha":1}"#).unwrap()
    }
    let () = connection
        .add_server(
            obj_path,
            HashMap::<String, MethodCallback>::from([
                (
                    "hi".to_string(),
                    handle_hi as MethodCallback,
                ),
                (
                    "hii".to_string(),
                    (|_| MsgTable::try_from(r#"{ "clo": "sure" }"#).unwrap())
                        as MethodCallback,
                ),
                (
                    "echo".to_string(),
                    (|req_args| req_args.to_owned())
                        as MethodCallback,
                ),
            ]),
        )
        .unwrap();
    // connection.listening(id).unwrap();
    sleep(Duration::from_millis(1000000));
    // println!("{:?}", obj);
}
