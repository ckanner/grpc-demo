#[macro_use]
extern crate lazy_static;
extern crate futures;
extern crate grpcio;
extern crate protobuf;
extern crate protos;
extern crate serde_json;

use std::env;
use std::io::{BufReader, Read};
use std::sync::Arc;
use std::sync::RwLock;
use std::{io, thread};

use futures::sync::oneshot;
use futures::Future;
use grpcio::{ChannelBuilder, Environment, RpcContext, ServerBuilder, UnarySink};
use protobuf::well_known_types::Value as pbValue;
use protobuf::Message;

use protos::diner::{Check, Order};
use protos::diner_grpc::{self, Diner};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::Path;

lazy_static! {
    static ref DATA: RwLock<Vec<protobuf::well_known_types::Struct>> = RwLock::new(vec![]);
}

#[derive(Default, Clone)]
struct DinerService;

impl Diner for DinerService {
    fn eat(&mut self, ctx: RpcContext, _order: Order, sink: UnarySink<Check>) {
        let mut check = Check::new();
        check.set_data(DATA.read().unwrap().get(0).unwrap().clone());
        let body_size = check.compute_size();
        let f = sink
            .success(check.clone())
            .map(move |_| println!("Responded body size {{ {} }}", body_size))
            .map_err(move |err| eprintln!("Failed to reply: {:?}", err));
        ctx.spawn(f)
    }
}

fn read_user_from_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, Value>, Box<Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let u = serde_json::from_reader(reader)?;

    Ok(u)
}

pub fn json_to_pb(val: &HashMap<String, serde_json::Value>) -> protobuf::well_known_types::Struct {
    let mut s = protobuf::well_known_types::Struct::new();
    for (k, v) in val.iter() {
        s.fields.insert(k.to_string(), json_to_pb_val(&v));
    }
    s
}
fn json_to_pb_val(val: &Value) -> pbValue {
    let mut pb_val = pbValue::new();
    match val {
        Value::String(s) => {
            pb_val.set_string_value(s.to_string());
        }
        Value::Bool(b) => {
            pb_val.set_bool_value(b.clone());
        }
        Value::Number(n) => {
            pb_val.set_number_value(n.as_f64().unwrap());
        }
        Value::Array(list) => {
            let mut list_val = protobuf::well_known_types::ListValue::new();
            for v in list.iter() {
                list_val.values.push(json_to_pb_val(v));
            }
            pb_val.set_list_value(list_val);
        }
        Value::Object(o) => {
            let mut s = protobuf::well_known_types::Struct::new();
            for (k, v) in o.into_iter() {
                s.fields.insert(k.to_string(), json_to_pb_val(v));
            }
            pb_val.set_struct_value(s);
        }
        Value::Null => pb_val.set_null_value(protobuf::well_known_types::NullValue::NULL_VALUE),
    }
    pb_val
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        panic!("Expected exactly two argument, the concurrency and count.")
    }
    let concurrency = args[1]
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("{} is not a valid concurrency number", args[1]));;

    let val = read_user_from_file("t.json").unwrap();
    let s = json_to_pb(&val);
    {
        let mut w = DATA.write().unwrap();
        w.push(s);
    }
    println!("load data success.");

    let env = Arc::new(Environment::new(concurrency));
    let service = diner_grpc::create_diner(DinerService);
    let channel_args = ChannelBuilder::new(Arc::clone(&env))
        .max_concurrent_stream(1024)
        .stream_initial_window_size(1073741824)
        .max_receive_message_len(524288000)
        .max_send_message_len(-1)
        .build_args();
    let mut server = ServerBuilder::new(env)
        .channel_args(channel_args)
        .register_service(service)
        .bind("127.0.0.1", 5900)
        .build()
        .unwrap();
    server.start();
    for &(ref host, port) in server.bind_addrs() {
        println!("listening on {}:{}", host, port);
    }
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        println!("Press ENTER to exit...");
        let _ = io::stdin().read(&mut [0]).unwrap();
        tx.send(())
    });
    let _ = rx.wait();
    let _ = server.shutdown().wait();
}
