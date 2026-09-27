//! Knight relay: a tiny WebSocket server with rooms. Clients send `ClientMsg::Join` first, then
//! `Send` payloads which are forwarded to the other peers in their room. It never looks inside
//! payloads, so any game protocol (state sync, lockstep, chat) works through it.
//!
//! Run: `cargo run -p knight-relay --release -- 0.0.0.0:9001` (default `127.0.0.1:9001`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use knight_net::relay::VERSION;
use knight_net::{ClientMsg, PeerId, Rooms, ServerMsg, Wire};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
struct State {
    rooms: Rooms,
    outboxes: HashMap<PeerId, UnboundedSender<Vec<u8>>>,
}

type Shared = Arc<Mutex<State>>;

fn deliver(state: &State, msgs: Vec<(PeerId, ServerMsg)>) {
    for (to, m) in msgs {
        if let Some(tx) = state.outboxes.get(&to) {
            let _ = tx.send(m.to_bytes());
        }
    }
}

async fn handle(stream: TcpStream, shared: Shared) {
    let addr = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    let ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            log::debug!("{addr}: handshake failed: {e}");
            return;
        }
    };
    let (mut sink, mut source) = ws.split();
    // The first frame must be a Join.
    let (room, name) = loop {
        match source.next().await {
            Some(Ok(Message::Binary(b))) => match ClientMsg::from_bytes(&b) {
                Ok(ClientMsg::Join { version, room, name }) if version == VERSION => break (room, name),
                Ok(ClientMsg::Join { version, .. }) => {
                    let m = ServerMsg::Error { message: format!("protocol {version}, relay speaks {VERSION}") };
                    let _ = sink.send(Message::Binary(m.to_bytes().into())).await;
                    return;
                }
                _ => return,
            },
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            _ => return,
        }
    };
    let (tx, mut rx) = unbounded_channel::<Vec<u8>>();
    let id = {
        let mut st = shared.lock().unwrap();
        let (id, out) = st.rooms.join(&room, &name);
        st.outboxes.insert(id, tx);
        deliver(&st, out);
        id
    };
    log::info!("{addr}: '{name}' joined room '{room}' as {id}");
    let writer = tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            if sink.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(msg)) = source.next().await {
        match msg {
            Message::Binary(b) => {
                if let Ok(ClientMsg::Send { to, data }) = ClientMsg::from_bytes(&b) {
                    let st = shared.lock().unwrap();
                    let out = st.rooms.relay(id, to, &data);
                    deliver(&st, out);
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    {
        let mut st = shared.lock().unwrap();
        st.outboxes.remove(&id);
        let out = st.rooms.leave(id);
        deliver(&st, out);
    }
    writer.abort();
    log::info!("{addr}: '{name}' left room '{room}'");
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:9001".into());
    let listener = TcpListener::bind(&addr).await.unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    log::info!("knight relay listening on ws://{addr}");
    let shared: Shared = Arc::new(Mutex::new(State { rooms: Rooms::new(), outboxes: HashMap::new() }));
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle(stream, shared.clone()));
    }
}
