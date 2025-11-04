use std::path::Path;

use ubus::MsgTable;

#[tokio::main]
async fn main() {
    let obj_path = "fifv";
    let method = "echo";
    let req_args = r#"{"name":"eth0"}"#;

    let obj_path = "test";
    let method = "hello";
    let req_args = r#"{"id":1,"msg":"a41234123"}"#;

    let socket = Path::new("/var/run/ubus/ubus.sock");

    let mut connection = ubus::Connection::connect(&socket)
        .await
        .map_err(|err| {
            eprintln!("{}: Failed to open ubus socket. {}", socket.display(), err);
            err
        })
        .unwrap();
    let objs = connection.lookup(obj_path).await.unwrap();
    let obj = objs.get(0).unwrap();
    dbg!("{}", &obj);
    // let obj: UbusObject = serde_json::from_str(&obj).unwrap();
    let req_args = MsgTable::try_from(req_args).unwrap();
    let reply_args = connection.invoke(obj.id, method, req_args).await.unwrap();
    println!("{}", String::try_from(reply_args).unwrap());

    // Value::from(bi);
    // let json_str = {
    //     let mut json_str = String::new();
    //     json_str = "{\n".to_string();
    //     let mut first = true;
    //     for x in reply_args.0 {
    //         if !first {
    //             json_str += ",\n";
    //         }
    //         //json_str += &format!("{:?}", x);
    //         let msg: BlobMsg = x.try_into().unwrap();
    //         json_str += &format!("{}", msg);
    //         first = false;
    //     }
    //     json_str += "\n}";
    //     json_str
    // };
    // println!("{}", json_str);
}
