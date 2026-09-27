//! Two WebSocket clients meet in a room on a running relay and exchange a message.
//! `cargo run -p knight-relay` in one terminal, then `cargo run -p knight-net --example ping`.

use knight_net::{NetEvent, connect_ws};

fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| "ws://127.0.0.1:9001".into());
    let mut a = connect_ws(&url, "ping", "alice");
    let mut b = connect_ws(&url, "ping", "bob");
    let mut sent = false;
    for _ in 0..300 {
        for e in a.update() {
            println!("alice: {e:?}");
        }
        for e in b.update() {
            println!("bob:   {e:?}");
            if let NetEvent::Data { data, .. } = e {
                println!("bob got {:?}", String::from_utf8_lossy(&data));
                return;
            }
        }
        if !sent && a.me().is_some() && a.peers().len() == 1 {
            a.broadcast(b"hello over the relay".to_vec());
            sent = true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    eprintln!("timed out");
    std::process::exit(1);
}
