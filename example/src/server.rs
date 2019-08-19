#[macro_use]
extern crate lazy_static;
extern crate futures;
extern crate grpcio;
extern crate protos;
extern crate protobuf;

use std::io::Read;
use std::sync::Arc;
use std::collections::HashMap;
use std::env;
use std::{io, thread};
use std::sync::RwLock;

use futures::sync::oneshot;
use futures::Future;
use grpcio::{Environment, RpcContext, ServerBuilder, ChannelBuilder, UnarySink};
use protobuf::Message;

use protos::diner::{Check, Item, Order};
use protos::diner_grpc::{self, Diner};

lazy_static! {
    static ref DATA: RwLock<HashMap<String, u64>> = RwLock::new(HashMap::new());
}

#[derive(Default, Clone)]
struct DinerService;

impl Diner for DinerService {
    fn eat(&mut self, ctx: RpcContext, order: Order, sink: UnarySink<Check>) {
        println!("Received Order {{ {:?} }}", order);
        let mut check = Check::new();
        check.set_total(order.get_items().iter().fold(0.0, |total, &item| {
            total + match item {
                Item::SPAM => 0.05,
                Item::EGGS => 0.25,
                Item::HAM => 1.0,
            }
        }));
        check.set_data(DATA.read().unwrap().clone());
        let body_size = check.compute_size();
        println!("Resp body size: {}", body_size);
        let f = sink
            .success(check.clone())
            .map(move |_| println!("Responded with Check {{ {} }}", body_size))
            .map_err(move |err| eprintln!("Failed to reply: {:?}", err));
        ctx.spawn(f)
    }
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        panic!("Expected exactly two argument, the concurrency and count.")
    }
    let concurrency = args[1].parse::<usize>().unwrap_or_else(|_| panic!("{} is not a valid concurrency number", args[1]));;
    let count = args[2]
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{} is not a valid count number", args[2]));

    {
        let mut w = DATA.write().unwrap();
        for i in 0..count {
            w.insert(format!("{}", i), i);
        }
    }

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
