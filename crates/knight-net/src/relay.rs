//! The relay protocol: clients join a room on a relay server and exchange opaque payloads.
//! A relay (see `apps/relay`) is all a web game needs: browsers cannot accept connections, so
//! peers talk through it. The same protocol runs in-process via [`crate::LocalHub`].

use crate::codec::{DecodeError, Reader, Result, Wire, Writer};

pub type PeerId = u32;

/// Protocol version; the relay rejects clients with a different one.
pub const VERSION: u32 = 1;

/// Client → relay.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientMsg {
    Join {
        version: u32,
        room: String,
        name: String,
    },
    /// Send a payload to one peer, or to everyone else in the room.
    Send {
        to: Option<PeerId>,
        data: Vec<u8>,
    },
}

/// Relay → client.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerMsg {
    /// You joined: your id and who is already here.
    Welcome {
        you: PeerId,
        peers: Vec<(PeerId, String)>,
    },
    PeerJoined {
        id: PeerId,
        name: String,
    },
    PeerLeft {
        id: PeerId,
    },
    Data {
        from: PeerId,
        data: Vec<u8>,
    },
    Error {
        message: String,
    },
}

impl Wire for ClientMsg {
    fn encode(&self, w: &mut Writer) {
        match self {
            ClientMsg::Join { version, room, name } => {
                w.u8(0).var(*version as u64).str(room).str(name);
            }
            ClientMsg::Send { to, data } => {
                w.u8(1).var(to.map_or(0, |t| t as u64 + 1)).bytes(data);
            }
        }
    }

    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(match r.u8()? {
            0 => ClientMsg::Join { version: r.var()? as u32, room: r.str()?, name: r.str()? },
            1 => {
                let to = r.var()?;
                ClientMsg::Send { to: (to > 0).then(|| (to - 1) as PeerId), data: r.bytes()?.to_vec() }
            }
            _ => return Err(DecodeError("unknown client message")),
        })
    }
}

impl Wire for ServerMsg {
    fn encode(&self, w: &mut Writer) {
        match self {
            ServerMsg::Welcome { you, peers } => {
                w.u8(0).var(*you as u64).var(peers.len() as u64);
                for (id, name) in peers {
                    w.var(*id as u64).str(name);
                }
            }
            ServerMsg::PeerJoined { id, name } => {
                w.u8(1).var(*id as u64).str(name);
            }
            ServerMsg::PeerLeft { id } => {
                w.u8(2).var(*id as u64);
            }
            ServerMsg::Data { from, data } => {
                w.u8(3).var(*from as u64).bytes(data);
            }
            ServerMsg::Error { message } => {
                w.u8(4).str(message);
            }
        }
    }

    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(match r.u8()? {
            0 => {
                let you = r.var()? as PeerId;
                let n = r.var()? as usize;
                if n > r.remaining() {
                    return Err(DecodeError("too many peers"));
                }
                let peers = (0..n).map(|_| Ok((r.var()? as PeerId, r.str()?))).collect::<Result<_>>()?;
                ServerMsg::Welcome { you, peers }
            }
            1 => ServerMsg::PeerJoined { id: r.var()? as PeerId, name: r.str()? },
            2 => ServerMsg::PeerLeft { id: r.var()? as PeerId },
            3 => ServerMsg::Data { from: r.var()? as PeerId, data: r.bytes()?.to_vec() },
            4 => ServerMsg::Error { message: r.str()? },
            _ => return Err(DecodeError("unknown server message")),
        })
    }
}

/// Server-side room bookkeeping shared by the relay binary and [`crate::LocalHub`]:
/// given an incoming message, which messages go to which peers.
#[derive(Default, Debug)]
pub struct Rooms {
    next_id: PeerId,
    /// peer → (room, name)
    peers: std::collections::BTreeMap<PeerId, (String, String)>,
}

impl Rooms {
    pub fn new() -> Self {
        Rooms { next_id: 1, peers: Default::default() }
    }

    /// A new connection joined `room`. Returns its id and the messages to deliver.
    pub fn join(&mut self, room: &str, name: &str) -> (PeerId, Vec<(PeerId, ServerMsg)>) {
        let id = self.next_id;
        self.next_id += 1;
        let peers: Vec<(PeerId, String)> =
            self.peers.iter().filter(|(_, (r, _))| r == room).map(|(&i, (_, n))| (i, n.clone())).collect();
        let mut out = vec![(id, ServerMsg::Welcome { you: id, peers: peers.clone() })];
        for (p, _) in &peers {
            out.push((*p, ServerMsg::PeerJoined { id, name: name.to_string() }));
        }
        self.peers.insert(id, (room.to_string(), name.to_string()));
        (id, out)
    }

    pub fn leave(&mut self, id: PeerId) -> Vec<(PeerId, ServerMsg)> {
        let Some((room, _)) = self.peers.remove(&id) else { return Vec::new() };
        self.peers.iter().filter(|(_, (r, _))| *r == room).map(|(&p, _)| (p, ServerMsg::PeerLeft { id })).collect()
    }

    /// `from` sent `data` to `to` (or everyone else in its room).
    pub fn relay(&self, from: PeerId, to: Option<PeerId>, data: &[u8]) -> Vec<(PeerId, ServerMsg)> {
        let Some((room, _)) = self.peers.get(&from) else { return Vec::new() };
        self.peers
            .iter()
            .filter(|(p, (r, _))| **p != from && r == room && to.is_none_or(|t| t == **p))
            .map(|(&p, _)| (p, ServerMsg::Data { from, data: data.to_vec() }))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_roundtrip() {
        for m in [
            ClientMsg::Join { version: VERSION, room: "lobby".into(), name: "Ann".into() },
            ClientMsg::Send { to: None, data: vec![1, 2, 3] },
            ClientMsg::Send { to: Some(0), data: vec![] },
        ] {
            assert_eq!(ClientMsg::from_bytes(&m.to_bytes()).unwrap(), m);
        }
        for m in [
            ServerMsg::Welcome { you: 3, peers: vec![(1, "a".into()), (2, "b".into())] },
            ServerMsg::PeerJoined { id: 4, name: "c".into() },
            ServerMsg::PeerLeft { id: 4 },
            ServerMsg::Data { from: 1, data: vec![9; 300] },
            ServerMsg::Error { message: "bad".into() },
        ] {
            assert_eq!(ServerMsg::from_bytes(&m.to_bytes()).unwrap(), m);
        }
    }

    #[test]
    fn rooms_route_messages() {
        let mut rooms = Rooms::new();
        let (a, _) = rooms.join("r1", "a");
        let (b, out) = rooms.join("r1", "b");
        assert!(out.contains(&(a, ServerMsg::PeerJoined { id: b, name: "b".into() })));
        let (c, _) = rooms.join("r2", "c");
        let sent = rooms.relay(a, None, &[1]);
        assert_eq!(sent, vec![(b, ServerMsg::Data { from: a, data: vec![1] })]);
        assert!(rooms.relay(c, None, &[1]).is_empty());
        assert_eq!(rooms.leave(a), vec![(b, ServerMsg::PeerLeft { id: a })]);
    }
}
