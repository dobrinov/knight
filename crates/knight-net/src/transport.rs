//! Transports carry relay messages. [`Client`] wraps one and keeps track of who is in the room.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;

use crate::codec::Wire;
use crate::relay::{ClientMsg, PeerId, Rooms, ServerMsg, VERSION};

/// What a transport reports.
#[derive(Clone, Debug, PartialEq)]
pub enum NetEvent {
    Connected { you: PeerId },
    PeerJoined { id: PeerId, name: String },
    PeerLeft { id: PeerId },
    Data { from: PeerId, data: Vec<u8> },
    Disconnected { reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnState {
    Connecting,
    Connected,
    Closed,
}

/// Something that moves relay messages: in-process, WebSocket, or your own (Steam, WebRTC...).
pub trait Transport {
    /// Queue a client message (sent as soon as the connection is up).
    fn send_raw(&mut self, msg: ClientMsg);
    /// Relay messages received since the last poll.
    fn poll_raw(&mut self) -> Vec<ServerMsg>;
    fn state(&self) -> ConnState;
    /// Why the connection closed, if it did.
    fn error(&self) -> Option<String> {
        None
    }
}

/// A room connection: send payloads to peers, receive events.
pub struct Client {
    transport: Box<dyn Transport>,
    me: Option<PeerId>,
    peers: BTreeMap<PeerId, String>,
    closed_reported: bool,
}

impl Client {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Client { transport, me: None, peers: BTreeMap::new(), closed_reported: false }
    }

    pub fn me(&self) -> Option<PeerId> {
        self.me
    }

    pub fn peers(&self) -> &BTreeMap<PeerId, String> {
        &self.peers
    }

    pub fn state(&self) -> ConnState {
        match self.transport.state() {
            ConnState::Connected if self.me.is_none() => ConnState::Connecting,
            s => s,
        }
    }

    /// Send to everyone else in the room.
    pub fn broadcast(&mut self, data: Vec<u8>) {
        self.transport.send_raw(ClientMsg::Send { to: None, data });
    }

    pub fn send_to(&mut self, to: PeerId, data: Vec<u8>) {
        self.transport.send_raw(ClientMsg::Send { to: Some(to), data });
    }

    pub fn broadcast_msg<T: Wire>(&mut self, msg: &T) {
        self.broadcast(msg.to_bytes());
    }

    pub fn update(&mut self) -> Vec<NetEvent> {
        let mut out = Vec::new();
        for m in self.transport.poll_raw() {
            match m {
                ServerMsg::Welcome { you, peers } => {
                    self.me = Some(you);
                    out.push(NetEvent::Connected { you });
                    for (id, name) in peers {
                        self.peers.insert(id, name.clone());
                        out.push(NetEvent::PeerJoined { id, name });
                    }
                }
                ServerMsg::PeerJoined { id, name } => {
                    self.peers.insert(id, name.clone());
                    out.push(NetEvent::PeerJoined { id, name });
                }
                ServerMsg::PeerLeft { id } => {
                    self.peers.remove(&id);
                    out.push(NetEvent::PeerLeft { id });
                }
                ServerMsg::Data { from, data } => out.push(NetEvent::Data { from, data }),
                ServerMsg::Error { message } => out.push(NetEvent::Disconnected { reason: message }),
            }
        }
        if self.transport.state() == ConnState::Closed && !self.closed_reported {
            self.closed_reported = true;
            let reason = self.transport.error().unwrap_or_else(|| "connection closed".into());
            out.push(NetEvent::Disconnected { reason });
        }
        out
    }
}

// --- In-process hub ------------------------------------------------------------------------

#[derive(Default)]
struct HubInner {
    rooms: Rooms,
    inbox: HashMap<PeerId, VecDeque<ServerMsg>>,
}

/// An in-process relay: several [`Client`]s in one program (tests, hot seat, split screen, bots).
#[derive(Clone, Default)]
pub struct LocalHub {
    inner: Rc<RefCell<HubInner>>,
}

impl LocalHub {
    pub fn new() -> Self {
        LocalHub { inner: Rc::new(RefCell::new(HubInner { rooms: Rooms::new(), inbox: HashMap::new() })) }
    }

    pub fn connect(&self, room: &str, name: &str) -> Client {
        let mut hub = self.inner.borrow_mut();
        let (id, out) = hub.rooms.join(room, name);
        for (to, m) in out {
            hub.inbox.entry(to).or_default().push_back(m);
        }
        Client::new(Box::new(LocalTransport { id, hub: self.inner.clone(), open: true }))
    }
}

struct LocalTransport {
    id: PeerId,
    hub: Rc<RefCell<HubInner>>,
    open: bool,
}

impl Transport for LocalTransport {
    fn send_raw(&mut self, msg: ClientMsg) {
        if let ClientMsg::Send { to, data } = msg {
            let mut hub = self.hub.borrow_mut();
            let out = hub.rooms.relay(self.id, to, &data);
            for (p, m) in out {
                hub.inbox.entry(p).or_default().push_back(m);
            }
        }
    }

    fn poll_raw(&mut self) -> Vec<ServerMsg> {
        self.hub.borrow_mut().inbox.get_mut(&self.id).map(|q| q.drain(..).collect()).unwrap_or_default()
    }

    fn state(&self) -> ConnState {
        if self.open { ConnState::Connected } else { ConnState::Closed }
    }
}

impl Drop for LocalTransport {
    fn drop(&mut self) {
        let mut hub = self.hub.borrow_mut();
        let out = hub.rooms.leave(self.id);
        for (p, m) in out {
            hub.inbox.entry(p).or_default().push_back(m);
        }
        hub.inbox.remove(&self.id);
    }
}

// --- WebSocket ------------------------------------------------------------------------------

/// Connect to a relay over WebSocket (`ws://host:port`). Works natively and in the browser.
pub fn connect_ws(url: &str, room: &str, name: &str) -> Client {
    let join = ClientMsg::Join { version: VERSION, room: room.to_string(), name: name.to_string() };
    Client::new(Box::new(crate::ws::WsTransport::open(url, join)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hub_delivers_and_tracks_peers() {
        let hub = LocalHub::new();
        let mut a = hub.connect("room", "Ann");
        let mut b = hub.connect("room", "Bob");
        let ea = a.update();
        let eb = b.update();
        assert!(matches!(ea[0], NetEvent::Connected { .. }));
        assert!(ea.iter().any(|e| matches!(e, NetEvent::PeerJoined { name, .. } if name == "Bob")));
        assert!(eb.iter().any(|e| matches!(e, NetEvent::PeerJoined { name, .. } if name == "Ann")));
        a.broadcast(b"hi".to_vec());
        assert_eq!(b.update(), vec![NetEvent::Data { from: a.me().unwrap(), data: b"hi".to_vec() }]);
        let bid = b.me().unwrap();
        drop(b);
        assert_eq!(a.update(), vec![NetEvent::PeerLeft { id: bid }]);
        assert!(a.peers().is_empty());
    }
}
