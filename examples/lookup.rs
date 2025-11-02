use std::{env, path::Path};

use ubus::{UbusError, UbusObject};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut obj_path = "";
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
    let obj_json = connection.lookup(obj_path).unwrap();
    let obj_json = serde_json::to_string_pretty(&obj_json).unwrap();

    println!("{}", &obj_json);
    let obj: UbusObject = serde_json::from_str(&obj_json).unwrap();
    // println!("{:?}", obj);
}
