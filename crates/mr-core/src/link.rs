//! The link between the server and a client (PLAN step 14c): the bottleneck the server's
//! stream crosses, modelled as a network so renderers face data that has not arrived yet.
//! The worker runs it in front of its client, on the simulated time the host stamps on
//! its messages (scripted runs are reproducible); a free link ([`LinkSpec::free`]) is not
//! run at all.
//!
//! The model: one line of `bandwidth_mbps` the messages leave on in order, then
//! `latency_ms` plus a uniform `jitter_ms` each; a lost message (`loss`) arrives one
//! timeout later per loss (the resend's time on the line is not modelled). Delivery is in
//! order per stream: one stream for everything (TCP: a late message holds back all after
//! it), or with `reorder` one per chunk plus one for lights and the environment (QUIC's
//! streams: a chunk's messages stay in order, different chunks overtake each other).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use crate::chunk::CHUNK_VOLUME;
use crate::{BlockEdit, BlockId, ChunkData, ChunkKey, Environment, Light, WorldUpdate};

/// The link's knobs; all zero is the free link (everything arrives at once, in order).
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LinkSpec {
    /// One-way delay of every message.
    pub latency_ms: f64,
    /// Extra delay per message, uniform in `[0, jitter_ms]`.
    pub jitter_ms: f64,
    /// The line's rate the messages share, megabits per second (0: unlimited).
    pub bandwidth_mbps: f64,
    /// Fraction of messages lost (at most 0.99); each loss delays the message by a timeout,
    /// twice the latency plus the jitter (at least 10 ms).
    pub loss: f64,
    /// One ordered stream per chunk instead of one for everything.
    pub reorder: bool,
    /// Seed of the jitter and the losses.
    pub seed: u64,
}

impl LinkSpec {
    /// Nothing delayed or lost: the link is not run.
    pub fn free(&self) -> bool {
        self.latency_ms <= 0.0 && self.jitter_ms <= 0.0 && self.bandwidth_mbps <= 0.0 && self.loss <= 0.0
    }
}

/// The message's size on the wire, uncompressed (keys, cells, edits, lights as stored).
pub fn wire_bytes(u: &WorldUpdate) -> usize {
    const KEY: usize = 13;
    match u {
        WorldUpdate::ChunkLoad { data: ChunkData::Uniform(_), .. } => 1 + KEY + 1 + size_of::<BlockId>(),
        WorldUpdate::ChunkLoad { data: ChunkData::Dense(_), .. } => 1 + KEY + 1 + CHUNK_VOLUME * size_of::<BlockId>(),
        WorldUpdate::ChunkUnload { .. } => 1 + KEY,
        WorldUpdate::SetBlocks(e) => 5 + e.len() * size_of::<BlockEdit>(),
        WorldUpdate::LightUpsert(l) => 5 + l.len() * size_of::<Light>(),
        WorldUpdate::LightRemove(l) => 5 + l.len() * 4,
        WorldUpdate::SetEnvironment(_) => 1 + size_of::<Environment>(),
    }
}

/// What reached a client by a frame (PLAN step 14e), reported by its worker with the frame.
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrivals {
    /// Bytes sent over its link so far (`wire_bytes`, resends included), and of those the
    /// bytes that arrived (each message once).
    pub bytes: u64,
    pub arrived: u64,
    /// Chunks of its region missing on the client's side: on the link, or not generated
    /// yet (a client generating the world). The server's backlog is the host's to add.
    pub holes: u32,
}

/// What crossed the link so far.
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkStats {
    pub messages: u64,
    pub bytes: u64,
    /// Losses (each a resend).
    pub resent: u64,
    /// Bytes of the messages delivered (each once).
    pub arrived: u64,
}

struct Pending {
    at: u64,
    seq: u64,
    msg: WorldUpdate,
}

impl PartialEq for Pending {
    fn eq(&self, o: &Self) -> bool {
        (self.at, self.seq) == (o.at, o.seq)
    }
}
impl Eq for Pending {}
impl PartialOrd for Pending {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Pending {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.at, self.seq).cmp(&(o.at, o.seq))
    }
}

/// A modelled link: messages go in with the time they were sent and come out when they
/// arrive. Times are seconds; deliveries are kept in nanoseconds.
pub struct Link {
    spec: LinkSpec,
    rng: u64,
    /// When the line is free again.
    line: f64,
    /// Per stream (`None`: lights and the environment; everything without `reorder`), the
    /// latest delivery scheduled.
    last: HashMap<Option<ChunkKey>, u64>,
    queue: BinaryHeap<Reverse<Pending>>,
    seq: u64,
    stats: LinkStats,
}

impl Link {
    pub fn new(spec: LinkSpec) -> Self {
        Self { spec, rng: spec.seed ^ 0x9e37_79b9_7f4a_7c15, line: 0.0, last: HashMap::new(), queue: BinaryHeap::new(), seq: 0, stats: LinkStats::default() }
    }

    pub fn spec(&self) -> LinkSpec {
        self.spec
    }

    /// Changes the knobs for messages sent from now on.
    pub fn set(&mut self, spec: LinkSpec) {
        self.spec = spec;
    }

    pub fn stats(&self) -> LinkStats {
        self.stats
    }

    /// Messages still on the way.
    pub fn in_flight(&self) -> usize {
        self.queue.len()
    }

    /// Chunks still on the way (holes at the client until they arrive).
    pub fn chunks_in_flight(&self) -> usize {
        self.queue.iter().filter(|p| matches!(p.0.msg, WorldUpdate::ChunkLoad { .. })).count()
    }

    /// Uniform in [0, 1) (splitmix64).
    fn uniform(&mut self) -> f64 {
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as f64 / (u64::MAX as f64 + 1.0)
    }

    /// Sends the server's messages at time `now`; with `reorder`, edits travel per chunk.
    pub fn send(&mut self, now: f64, updates: Vec<WorldUpdate>) {
        for u in updates {
            match u {
                WorldUpdate::SetBlocks(edits) if self.spec.reorder => {
                    let mut by: Vec<(ChunkKey, Vec<BlockEdit>)> = vec![];
                    for e in edits {
                        let k = ChunkKey::of_block(e.pos);
                        match by.iter_mut().find(|(c, _)| *c == k) {
                            Some((_, v)) => v.push(e),
                            None => by.push((k, vec![e])),
                        }
                    }
                    for (k, v) in by {
                        self.one(now, Some(k), WorldUpdate::SetBlocks(v));
                    }
                }
                u => {
                    let key = match &u {
                        WorldUpdate::ChunkLoad { key, .. } | WorldUpdate::ChunkUnload { key } if self.spec.reorder => Some(*key),
                        _ => None,
                    };
                    self.one(now, key, u);
                }
            }
        }
    }

    fn one(&mut self, now: f64, stream: Option<ChunkKey>, msg: WorldUpdate) {
        let s = self.spec;
        let bytes = wire_bytes(&msg);
        let depart = now.max(self.line);
        self.line = depart + if s.bandwidth_mbps > 0.0 { bytes as f64 * 8.0 / (s.bandwidth_mbps * 1e6) } else { 0.0 };
        let mut arrive = self.line + (s.latency_ms + s.jitter_ms * self.uniform()) * 1e-3;
        let timeout = (2.0 * s.latency_ms + s.jitter_ms).max(10.0) * 1e-3;
        while s.loss > 0.0 && self.uniform() < s.loss.min(0.99) {
            arrive += timeout;
            self.stats.resent += 1;
            self.stats.bytes += bytes as u64;
        }
        self.stats.messages += 1;
        self.stats.bytes += bytes as u64;
        let at = (arrive * 1e9) as u64;
        let last = self.last.entry(stream).or_insert(0);
        *last = (*last).max(at);
        let at = *last;
        self.queue.push(Reverse(Pending { at, seq: self.seq, msg }));
        self.seq += 1;
    }

    /// Hands out the messages arrived by time `now`, in arrival order.
    pub fn deliver(&mut self, now: f64, out: &mut Vec<WorldUpdate>) {
        let now = (now * 1e9) as u64;
        while self.queue.peek().is_some_and(|p| p.0.at <= now) {
            let msg = self.queue.pop().unwrap().0.msg;
            self.stats.arrived += wire_bytes(&msg) as u64;
            out.push(msg);
        }
        if self.queue.is_empty() {
            self.last.clear();
        } else if self.last.len() > 4 * self.queue.len() + 64 {
            // Streams with nothing on the way hold nothing back.
            self.last.retain(|_, t| *t > now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    fn load(x: i32) -> WorldUpdate {
        WorldUpdate::ChunkLoad { key: ChunkKey { pos: IVec3::new(x, 0, 0), lod: 0 }, data: ChunkData::Uniform(BlockId(1)) }
    }

    fn xs(v: &[WorldUpdate]) -> Vec<i32> {
        v.iter()
            .map(|u| match u {
                WorldUpdate::ChunkLoad { key, .. } | WorldUpdate::ChunkUnload { key } => key.pos.x,
                _ => -1,
            })
            .collect()
    }

    /// Latency delays, bandwidth spaces out, order holds on one stream.
    #[test]
    fn latency_and_bandwidth() {
        // One load per second on the line.
        let bytes = wire_bytes(&load(0));
        let mut l = Link::new(LinkSpec { latency_ms: 100.0, bandwidth_mbps: bytes as f64 * 8.0 * 1e-6, ..Default::default() });
        l.send(0.0, (0..3).map(load).collect());
        let mut out = vec![];
        l.deliver(1.05, &mut out);
        assert!(out.is_empty());
        l.deliver(1.1, &mut out);
        assert_eq!(xs(&out), [0]);
        l.deliver(3.1, &mut out);
        assert_eq!(xs(&out), [0, 1, 2]);
        assert_eq!(l.stats(), LinkStats { messages: 3, bytes: 3 * bytes as u64, resent: 0, arrived: 3 * bytes as u64 });
    }

    /// Jitter and losses reorder only across streams: one stream (or one chunk's) stays in
    /// order, and every message arrives; the same seed gives the same arrivals.
    #[test]
    fn order_per_stream() {
        let spec = LinkSpec { latency_ms: 20.0, jitter_ms: 50.0, loss: 0.3, ..Default::default() };
        let run = |spec: LinkSpec| {
            let mut l = Link::new(spec);
            for f in 0..20 {
                // Chunk x loads at frame x, unloads and loads again 3 frames later.
                let mut ups = vec![load(f)];
                if f >= 3 {
                    ups.push(WorldUpdate::ChunkUnload { key: ChunkKey { pos: IVec3::new(f - 3, 0, 0), lod: 0 } });
                }
                l.send(f as f64 / 60.0, ups);
            }
            let mut out = vec![];
            l.deliver(100.0, &mut out);
            (out, l.stats())
        };
        let (ordered, st) = run(spec);
        assert_eq!(ordered.len(), 37);
        assert!(st.resent > 0);
        let mut expect = vec![];
        for f in 0..20 {
            expect.push(f);
            if f >= 3 {
                expect.push(f - 3);
            }
        }
        assert_eq!(xs(&ordered), expect);
        let (re, _) = run(LinkSpec { reorder: true, ..spec });
        assert_ne!(xs(&re), expect, "jitter reorders chunks");
        for x in 0..17 {
            let kinds: Vec<bool> = re.iter().filter(|u| xs(std::slice::from_ref(u)) == [x]).map(|u| matches!(u, WorldUpdate::ChunkLoad { .. })).collect();
            assert_eq!(kinds, [true, false], "chunk {x}'s load before its unload");
        }
        assert_eq!(xs(&run(LinkSpec { reorder: true, ..spec }).0), xs(&re));
    }

    /// With `reorder`, an edit batch splits per chunk and each part follows its chunk.
    #[test]
    fn edits_follow_their_chunk() {
        let mut l = Link::new(LinkSpec { latency_ms: 10.0, jitter_ms: 100.0, reorder: true, seed: 3, ..Default::default() });
        let e = |x: i32| BlockEdit { pos: IVec3::new(x, 0, 0), block: BlockId(2) };
        l.send(0.0, vec![load(0), load(1)]);
        l.send(0.001, vec![WorldUpdate::SetBlocks(vec![e(1), e(40), e(2)])]);
        let mut out = vec![];
        l.deliver(10.0, &mut out);
        let pos = |p: &dyn Fn(&WorldUpdate) -> bool| out.iter().position(|u| p(u)).unwrap();
        let edits: Vec<&WorldUpdate> = out.iter().filter(|u| matches!(u, WorldUpdate::SetBlocks(_))).collect();
        assert_eq!(edits.len(), 2);
        let at0 = pos(&|u| matches!(u, WorldUpdate::SetBlocks(v) if v.len() == 2));
        let at1 = pos(&|u| matches!(u, WorldUpdate::SetBlocks(v) if v.len() == 1));
        assert!(pos(&|u| xs(std::slice::from_ref(u)) == [0]) < at0);
        assert!(pos(&|u| xs(std::slice::from_ref(u)) == [1]) < at1);
    }
}
