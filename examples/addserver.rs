use std::{env, path::Path, thread::sleep, time::Duration};

use ubus::{UbusError, UbusObject};

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
    let (id, objtype) = connection.add_server(obj_path, &[("hi", || {})]).unwrap();
    // connection.listening(id).unwrap();
    sleep(Duration::from_millis(1000000));
    // println!("{:?}", obj);
}
