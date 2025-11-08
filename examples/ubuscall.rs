use serde_json::{Value, to_string_pretty};
use std::env;
use std::path::Path;

#[tokio::main]
async fn main() {
    /* enable debug logger */
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    let args: Vec<String> = env::args().collect();
    let mut obj_path = "";
    let mut method = "";
    let mut data = "";
    if args.len() < 2 || args.len() > 4 {
        eprintln!("{} <object> <method> [arguments as json]", args[0]);
        return;
    } else if args.len() >= 3 {
        obj_path = &args[1];
        method = &args[2];
    }
    if args.len() == 4 {
        data = &args[3];
    }

    let mut connection = ubus::Connection::connect_ubusd()
        .await
        .map_err(|err| {
            log::error!("Failed to open ubus socket  ({})", err);
            err
        })
        .unwrap();

    match connection
        .call(obj_path, method, data.try_into().unwrap())
        .await
    {
        Ok(json) => {
            println!("{}", json.to_string_pretty().unwrap());
        }
        Err(e) => {
            eprintln!("Failed to call, with error: {}", e);
            // panic!()
        }
    }
}
