use std::{env, path::Path};

#[tokio::main]
async fn main() {
    /* enable debug logger */
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("trace"));

    let args: Vec<String> = env::args().collect();
    let mut obj_path = "";
    if args.len() > 1 {
        obj_path = args[1].as_str();
    }

    let mut connection = ubus::Connection::connect_ubusd()
        .await
        .map_err(|err| {
            log::error!("Failed to open ubus socket  ({})", err);
            err
        })
        .unwrap();

    let objs = connection.lookup(obj_path).await.unwrap();
    // let obj_json = serde_json::to_string_pretty(&obj_json).unwrap();

    for obj in objs {
        println!(
            "`{}` (ObjId={:x} ObjType={:x}) {}",
            obj.path,
            obj.id,
            obj.objtype,
            obj.reported_signature.to_string_pretty().unwrap()
        )
    }
    // println!("{:#?}", &objs);
    // let obj: UbusObject = serde_json::from_str(&obj_json).unwrap();
    // println!("{:?}", obj);
}
