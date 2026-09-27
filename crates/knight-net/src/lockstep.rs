//! Deterministic lockstep for turn-based and fixed-tick games.
//!
//! Instead of sending game state, peers send *commands* (move this unit, attack that one). Each
//! peer schedules its commands `delay` ticks ahead; tick `t` is simulated only once commands for
//! `t` have arrived from every peer, and every peer applies them in the same order (by peer id).
//! With a deterministic simulation (fixed timestep, seeded RNG, no floats that depend on frame
//! rate) everyone stays in sync with tiny bandwidth. Hex games are a great fit: commands are
//! small ("unit 3 to hex (5, -2)").

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::{Reader, Result, Wire, Writer};
use crate::relay::PeerId;

struct Packet<C> {
    tick: u64,
    cmds: Vec<C>,
}

impl<C: Wire> Wire for Packet<C> {
    fn encode(&self, w: &mut Writer) {
        w.var(self.tick);
        self.cmds.encode(w);
    }

    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Packet { tick: r.var()?, cmds: Vec::decode(r)? })
    }
}

pub struct Lockstep<C> {
    /// Next tick to simulate.
    pub tick: u64,
    /// Input delay in ticks (hides latency; 3 ticks at 20 Hz = 150 ms).
    pub delay: u64,
    me: PeerId,
    peers: BTreeSet<PeerId>,
    inputs: BTreeMap<u64, BTreeMap<PeerId, Vec<C>>>,
    queued: Vec<C>,
    submitted_until: u64,
}

impl<C: Wire + Clone> Lockstep<C> {
    /// `peers` includes `me`.
    pub fn new(me: PeerId, peers: impl IntoIterator<Item = PeerId>, delay: u64) -> Self {
        let mut peers: BTreeSet<PeerId> = peers.into_iter().collect();
        peers.insert(me);
        Lockstep { tick: 0, delay, me, peers, inputs: BTreeMap::new(), queued: Vec::new(), submitted_until: 0 }
    }

    /// Queue a local command; it goes out with the next [`Lockstep::submit`].
    pub fn command(&mut self, c: C) {
        self.queued.push(c);
    }

    /// Package queued commands for the next unsubmitted tick. Send the bytes to all peers.
    /// Call once per tick (it returns `None` when already `delay` ticks ahead).
    pub fn submit(&mut self) -> Option<Vec<u8>> {
        let target = self.tick + self.delay;
        if self.submitted_until > target {
            return None;
        }
        let t = self.submitted_until.max(self.delay);
        self.submitted_until = t + 1;
        let cmds = std::mem::take(&mut self.queued);
        self.inputs.entry(t).or_default().insert(self.me, cmds.clone());
        Some(Packet { tick: t, cmds }.to_bytes())
    }

    /// Bytes from a peer.
    pub fn receive(&mut self, from: PeerId, bytes: &[u8]) -> Result<()> {
        let p = Packet::<C>::decode(&mut Reader::new(bytes))?;
        if p.tick >= self.tick {
            self.inputs.entry(p.tick).or_default().insert(from, p.cmds);
        }
        Ok(())
    }

    /// A peer left: stop waiting for it.
    pub fn remove_peer(&mut self, id: PeerId) {
        self.peers.remove(&id);
    }

    pub fn peers(&self) -> impl Iterator<Item = PeerId> + '_ {
        self.peers.iter().copied()
    }

    /// If every peer's commands for the current tick are in, return them (ordered by peer id)
    /// and move to the next tick. The first `delay` ticks are empty for everyone.
    pub fn advance(&mut self) -> Option<Vec<(PeerId, Vec<C>)>> {
        if self.tick < self.delay {
            self.tick += 1;
            return Some(Vec::new());
        }
        let have = self.inputs.get(&self.tick)?;
        if !self.peers.iter().all(|p| have.contains_key(p)) {
            return None;
        }
        let mut got = self.inputs.remove(&self.tick).unwrap();
        got.retain(|p, _| self.peers.contains(p));
        self.tick += 1;
        Some(got.into_iter().collect())
    }

    /// Ticks we are waiting on (for "waiting for players..." UI).
    pub fn stalled_on(&self) -> Vec<PeerId> {
        let have = self.inputs.get(&self.tick);
        self.peers.iter().copied().filter(|p| have.is_none_or(|h| !h.contains_key(p))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalHub, NetEvent};

    /// Two peers, each moving a counter; both simulations end identical.
    #[test]
    fn two_peers_stay_in_sync() {
        let hub = LocalHub::new();
        let mut ca = hub.connect("g", "a");
        let mut cb = hub.connect("g", "b");
        ca.update();
        cb.update();
        let (ia, ib) = (ca.me().unwrap(), cb.me().unwrap());
        let mut la: Lockstep<u32> = Lockstep::new(ia, [ib], 2);
        let mut lb: Lockstep<u32> = Lockstep::new(ib, [ia], 2);
        let (mut sa, mut sb) = (Vec::new(), Vec::new());
        for frame in 0..50u32 {
            if frame % 3 == 0 {
                la.command(frame);
            }
            if frame % 5 == 0 {
                lb.command(frame * 10);
            }
            if let Some(b) = la.submit() {
                ca.broadcast(b);
            }
            if let Some(b) = lb.submit() {
                cb.broadcast(b);
            }
            for e in ca.update() {
                if let NetEvent::Data { from, data } = e {
                    la.receive(from, &data).unwrap();
                }
            }
            for e in cb.update() {
                if let NetEvent::Data { from, data } = e {
                    lb.receive(from, &data).unwrap();
                }
            }
            while let Some(t) = la.advance() {
                sa.push(t.iter().map(|(p, c)| (*p, c.clone())).collect::<Vec<_>>());
            }
            while let Some(t) = lb.advance() {
                sb.push(t.iter().map(|(p, c)| (*p, c.clone())).collect::<Vec<_>>());
            }
        }
        let n = sa.len().min(sb.len());
        assert!(n > 40);
        assert_eq!(sa[..n], sb[..n]);
        assert!(sa.iter().flatten().any(|(_, c)| c.contains(&30)));
    }
}
