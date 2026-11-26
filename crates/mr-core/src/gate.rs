//! Gates between domains: the gap between two machines (or processes, or devices) placed on
//! an edge of a pipeline. Whatever crosses a gate is serialized ([`Wire`]), the bytes cross a
//! modelled network ([`Gate`]: a line of some bandwidth, latency and jitter, packets lost and
//! reordered), and the far side deserializes what arrived. The serialization's cost is part
//! of what a gate simulates, and so is loss: a lost packet's bytes arrive as zeros (or late,
//! resent, with `resend`), and the data decodes to whatever those bytes say. A decode that
//! fails (a length past the end, a tag that names nothing) loses the message; nothing guards a
//! data type against the bytes it gets.
//!
//! Times are seconds of simulated time (the host's stamps, so scripted runs are
//! reproducible); deliveries are kept in nanoseconds.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

use glam::{DVec3, IVec3, Vec3};
use serde::{Deserialize, Serialize};

use crate::chunk::CHUNK_VOLUME;
use crate::{BlockEdit, BlockId, ChunkData, ChunkKey, DenseChunk, Environment, Light, Sun, WorldUpdate};

/// A gate's knobs. All zero but the packet size is a free line: everything arrives at once,
/// in order, but still serialized and deserialized.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GateSpec {
    /// One-way delay of every packet.
    pub latency_ms: f64,
    /// Extra delay per packet, uniform in `[0, jitter_ms]`.
    pub jitter_ms: f64,
    /// The line's rate the packets share, megabits per second (0: unlimited).
    pub bandwidth_mbps: f64,
    /// Fraction of packets lost (at most 0.99).
    pub loss: f64,
    /// Messages arrive as each completes, overtaking each other; otherwise in the order sent
    /// (a late message holds back all after it).
    pub reorder: bool,
    /// A lost packet is sent again after a timeout (twice the latency plus the jitter, at
    /// least 10 ms) until it arrives; otherwise its bytes arrive as zeros.
    pub resend: bool,
    /// Bytes per packet (at least 1).
    pub mtu: u32,
    /// Seed of the jitter and the losses.
    pub seed: u64,
}

impl Default for GateSpec {
    fn default() -> Self {
        Self { latency_ms: 0.0, jitter_ms: 0.0, bandwidth_mbps: 0.0, loss: 0.0, reorder: false, resend: false, mtu: 1200, seed: 0 }
    }
}

impl GateSpec {
    /// The knobs' names, as a gate node spells them.
    pub const KNOBS: &'static [&'static str] = &["latency_ms", "jitter_ms", "bandwidth_mbps", "loss", "reorder", "resend", "mtu", "seed"];

    /// Why these knobs are not a gate's, if they are not.
    pub fn check(&self) -> Result<(), String> {
        let neg = [("latency_ms", self.latency_ms), ("jitter_ms", self.jitter_ms), ("bandwidth_mbps", self.bandwidth_mbps)];
        if let Some((k, v)) = neg.iter().find(|(_, v)| !v.is_finite() || *v < 0.0) {
            return Err(format!("{k} is at least 0 (not {v})"));
        }
        if !(0.0..=0.99).contains(&self.loss) {
            return Err(format!("loss is a fraction of packets, 0 to 0.99 (not {})", self.loss));
        }
        if self.mtu == 0 {
            return Err("mtu is at least 1 byte".into());
        }
        Ok(())
    }
}

/// A serialized form: what crosses a gate. `decode` reads what `encode` wrote, or whatever
/// bytes arrived instead; it fails only where the bytes cannot be read as the type.
pub trait Wire: Sized {
    fn encode(&self, out: &mut Vec<u8>);
    fn decode(bytes: &[u8]) -> Result<Self, WireError>;
}

/// Bytes that do not read as the type (the message is lost).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireError(pub String);

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for WireError {}

/// Little-endian writing.
pub struct Put<'a>(pub &'a mut Vec<u8>);

impl Put<'_> {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn vec3(&mut self, v: Vec3) {
        v.to_array().into_iter().for_each(|x| self.f32(x));
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
}

/// Little-endian reading, failing past the end.
pub struct Get<'a>(pub &'a [u8]);

impl<'a> Get<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if n > self.0.len() {
            return Err(WireError(format!("{n} bytes wanted, {} left", self.0.len())));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.arr::<1>()?[0])
    }
    pub fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    pub fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    pub fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(self.arr()?))
    }
    pub fn i32(&mut self) -> Result<i32, WireError> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    pub fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    pub fn f64(&mut self) -> Result<f64, WireError> {
        Ok(f64::from_le_bytes(self.arr()?))
    }
    pub fn vec3(&mut self) -> Result<Vec3, WireError> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    /// A count of items of `each` bytes, checked against what is left (a count garbled by
    /// loss reads as a failure, not as an allocation).
    pub fn count(&mut self, each: usize) -> Result<usize, WireError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(each.max(1)) > self.0.len() {
            return Err(WireError(format!("{n} items of {each} bytes, {} bytes left", self.0.len())));
        }
        Ok(n)
    }
    pub fn end(&self) -> Result<(), WireError> {
        if self.0.is_empty() { Ok(()) } else { Err(WireError(format!("{} bytes left over", self.0.len()))) }
    }
}

/// A slice of plain values, as bytes (a length, then the values as laid out in memory).
pub fn encode_pod<T: bytemuck::Pod>(v: &[T], out: &mut Vec<u8>) {
    Put(out).u32(v.len() as u32);
    out.extend_from_slice(bytemuck::cast_slice(v));
}

/// What [`encode_pod`] wrote.
pub fn decode_pod<T: bytemuck::Pod>(bytes: &[u8]) -> Result<Vec<T>, WireError> {
    let mut g = Get(bytes);
    let n = g.count(size_of::<T>())?;
    let body = g.take(n * size_of::<T>())?;
    g.end()?;
    let mut v = vec![T::zeroed(); n];
    bytemuck::cast_slice_mut::<T, u8>(&mut v).copy_from_slice(body);
    Ok(v)
}

/// The world's changes: the stream from the world to a client.
impl Wire for Vec<WorldUpdate> {
    fn encode(&self, out: &mut Vec<u8>) {
        let mut p = Put(out);
        p.u32(self.len() as u32);
        let key = |p: &mut Put, k: &ChunkKey| {
            k.pos.to_array().into_iter().for_each(|x| p.i32(x));
            p.u8(k.lod);
        };
        for u in self {
            match u {
                WorldUpdate::ChunkLoad { key: k, data } => {
                    p.u8(0);
                    key(&mut p, k);
                    match data {
                        ChunkData::Uniform(b) => {
                            p.u8(0);
                            p.u16(b.0);
                        }
                        ChunkData::Dense(d) => {
                            p.u8(1);
                            d.0.iter().for_each(|b| p.u16(b.0));
                        }
                    }
                }
                WorldUpdate::ChunkUnload { key: k } => {
                    p.u8(1);
                    key(&mut p, k);
                }
                WorldUpdate::SetBlocks(e) => {
                    p.u8(2);
                    p.u32(e.len() as u32);
                    for e in e {
                        e.pos.to_array().into_iter().for_each(|x| p.i32(x));
                        p.u16(e.block.0);
                    }
                }
                WorldUpdate::LightUpsert(l) => {
                    p.u8(3);
                    p.u32(l.len() as u32);
                    for l in l {
                        p.u32(l.id);
                        l.position.to_array().into_iter().for_each(|x| p.f64(x));
                        p.vec3(l.color);
                        p.f32(l.intensity);
                        p.f32(l.radius);
                    }
                }
                WorldUpdate::LightRemove(l) => {
                    p.u8(4);
                    p.u32(l.len() as u32);
                    l.iter().for_each(|x| p.u32(*x));
                }
                WorldUpdate::SetEnvironment(e) => {
                    p.u8(5);
                    p.vec3(e.sky);
                    p.vec3(e.ambient);
                    match e.sun {
                        None => p.u8(0),
                        Some(s) => {
                            p.u8(1);
                            p.vec3(s.direction);
                            p.vec3(s.color);
                        }
                    }
                    p.f32(e.exposure);
                }
            }
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut g = Get(bytes);
        let n = g.count(1)?;
        let key = |g: &mut Get| -> Result<ChunkKey, WireError> { Ok(ChunkKey { pos: IVec3::new(g.i32()?, g.i32()?, g.i32()?), lod: g.u8()? }) };
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(match g.u8()? {
                0 => {
                    let k = key(&mut g)?;
                    let data = match g.u8()? {
                        0 => ChunkData::Uniform(BlockId(g.u16()?)),
                        1 => {
                            let mut d = DenseChunk::filled(BlockId::AIR);
                            let body = g.take(CHUNK_VOLUME * 2)?;
                            for (b, c) in d.0.iter_mut().zip(body.as_chunks::<2>().0) {
                                *b = BlockId(u16::from_le_bytes(*c));
                            }
                            ChunkData::Dense(d.into())
                        }
                        t => return Err(WireError(format!("chunk data tag {t}"))),
                    };
                    WorldUpdate::ChunkLoad { key: k, data }
                }
                1 => WorldUpdate::ChunkUnload { key: key(&mut g)? },
                2 => {
                    let n = g.count(14)?;
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n {
                        e.push(BlockEdit { pos: IVec3::new(g.i32()?, g.i32()?, g.i32()?), block: BlockId(g.u16()?) });
                    }
                    WorldUpdate::SetBlocks(e)
                }
                3 => {
                    let n = g.count(48)?;
                    let mut l = Vec::with_capacity(n);
                    for _ in 0..n {
                        l.push(Light { id: g.u32()?, position: DVec3::new(g.f64()?, g.f64()?, g.f64()?), color: g.vec3()?, intensity: g.f32()?, radius: g.f32()? });
                    }
                    WorldUpdate::LightUpsert(l)
                }
                4 => {
                    let n = g.count(4)?;
                    WorldUpdate::LightRemove((0..n).map(|_| g.u32()).collect::<Result<_, _>>()?)
                }
                5 => {
                    let (sky, ambient) = (g.vec3()?, g.vec3()?);
                    let sun = match g.u8()? {
                        0 => None,
                        1 => Some(Sun { direction: g.vec3()?, color: g.vec3()? }),
                        t => return Err(WireError(format!("sun tag {t}"))),
                    };
                    WorldUpdate::SetEnvironment(Environment { sky, ambient, sun, exposure: g.f32()? })
                }
                t => return Err(WireError(format!("world update tag {t}"))),
            });
        }
        g.end()?;
        Ok(out)
    }
}

/// An image: its size and pixels (any format: the bytes per pixel follow from the size).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Wire for Frame {
    fn encode(&self, out: &mut Vec<u8>) {
        let mut p = Put(out);
        p.u32(self.width);
        p.u32(self.height);
        p.u32(self.pixels.len() as u32);
        p.bytes(&self.pixels);
    }

    fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut g = Get(bytes);
        let (width, height) = (g.u32()?, g.u32()?);
        let n = g.count(1)?;
        let pixels = g.take(n)?.to_vec();
        g.end()?;
        Ok(Frame { width, height, pixels })
    }
}

/// What crossed a gate so far, the serialization included.
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GateStats {
    /// Messages sent and messages delivered (whole, loss or not).
    pub messages: u64,
    pub delivered: u64,
    /// Delivered messages whose bytes did not decode (lost).
    pub failed: u64,
    /// Bytes on the line, resends included, and the packets.
    pub bytes: u64,
    pub packets: u64,
    /// Packets lost: arriving as zeros, or (`resend`) sent again.
    pub lost_packets: u64,
    pub lost_bytes: u64,
    /// Time spent serializing and deserializing, milliseconds of wall clock.
    pub encode_ms: f64,
    pub decode_ms: f64,
}

struct InFlight {
    at: u64,
    seq: u64,
    bytes: Vec<u8>,
}

impl PartialEq for InFlight {
    fn eq(&self, o: &Self) -> bool {
        (self.at, self.seq) == (o.at, o.seq)
    }
}
impl Eq for InFlight {}
impl PartialOrd for InFlight {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for InFlight {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.at, self.seq).cmp(&(o.at, o.seq))
    }
}

/// A modelled network the bytes of messages cross: each message in packets of `mtu`
/// bytes on one line, each packet late by the latency and its jitter, lost by chance; a
/// message arrives when its last packet does (a lost one's bytes zeros, or resent).
pub struct Gate {
    spec: GateSpec,
    rng: u64,
    line: f64,
    last: u64,
    seq: u64,
    queue: BinaryHeap<Reverse<InFlight>>,
    stats: GateStats,
}

impl Gate {
    pub fn new(spec: GateSpec) -> Self {
        Self { spec, rng: spec.seed ^ 0x6a09_e667_f3bc_c908, line: 0.0, last: 0, seq: 0, queue: BinaryHeap::new(), stats: GateStats::default() }
    }

    pub fn spec(&self) -> GateSpec {
        self.spec
    }

    /// Changes the knobs for messages sent from now on.
    pub fn set(&mut self, spec: GateSpec) {
        self.spec = spec;
    }

    pub fn stats(&self) -> GateStats {
        self.stats
    }

    /// Messages still on the way.
    pub fn in_flight(&self) -> usize {
        self.queue.len()
    }

    /// Uniform in [0, 1) (splitmix64).
    fn uniform(&mut self) -> f64 {
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as f64 / (u64::MAX as f64 + 1.0)
    }

    /// Sends a message's bytes at time `now`.
    pub fn send(&mut self, now: f64, mut bytes: Vec<u8>) {
        let s = self.spec;
        let rate = |n: usize| if s.bandwidth_mbps > 0.0 { n as f64 * 8.0 / (s.bandwidth_mbps * 1e6) } else { 0.0 };
        let timeout = (2.0 * s.latency_ms + s.jitter_ms).max(10.0) * 1e-3;
        let mut done = now;
        let len = bytes.len();
        let mtu = s.mtu.max(1) as usize;
        // An empty message is one empty packet.
        let packets = len.div_ceil(mtu).max(1);
        for i in 0..packets {
            let range = i * mtu..((i + 1) * mtu).min(len);
            let n = range.len();
            let depart = now.max(self.line);
            self.line = depart + rate(n);
            let mut arrive = self.line + (s.latency_ms + s.jitter_ms * self.uniform()) * 1e-3;
            self.stats.packets += 1;
            self.stats.bytes += n as u64;
            while s.loss > 0.0 && self.uniform() < s.loss.min(0.99) {
                self.stats.lost_packets += 1;
                self.stats.lost_bytes += n as u64;
                if !s.resend {
                    bytes[range.clone()].fill(0);
                    break;
                }
                arrive += timeout;
                self.line += rate(n);
                self.stats.packets += 1;
                self.stats.bytes += n as u64;
            }
            done = done.max(arrive);
        }
        self.stats.messages += 1;
        let mut at = (done * 1e9) as u64;
        if !s.reorder {
            at = at.max(self.last);
            self.last = at;
        }
        self.queue.push(Reverse(InFlight { at, seq: self.seq, bytes }));
        self.seq += 1;
    }

    /// The messages arrived by time `now`, in arrival order, with their arrival times.
    pub fn deliver(&mut self, now: f64, out: &mut Vec<(f64, Vec<u8>)>) {
        let now = (now * 1e9) as u64;
        while self.queue.peek().is_some_and(|p| p.0.at <= now) {
            let m = self.queue.pop().unwrap().0;
            out.push((m.at as f64 * 1e-9, m.bytes));
            self.stats.delivered += 1;
        }
    }

    /// Serializes `value` and sends it at `sent`, then hands out what arrived by `now`,
    /// decoded, with the arrival times (the messages that failed to decode counted and
    /// dropped); both ends timed.
    pub fn pass<T: Wire>(&mut self, sent: f64, value: Option<&T>, now: f64) -> Vec<(f64, T)> {
        if let Some(v) = value {
            let t = Instant::now();
            let mut bytes = Vec::new();
            v.encode(&mut bytes);
            self.stats.encode_ms += t.elapsed().as_secs_f64() * 1e3;
            self.send(sent, bytes);
        }
        let mut got = Vec::new();
        self.deliver(now, &mut got);
        let t = Instant::now();
        let out: Vec<(f64, T)> = got
            .iter()
            .filter_map(|(at, b)| match T::decode(b) {
                Ok(v) => Some((*at, v)),
                Err(_) => {
                    self.stats.failed += 1;
                    None
                }
            })
            .collect();
        self.stats.decode_ms += t.elapsed().as_secs_f64() * 1e3;
        out
    }
}

/// Several gates on one edge, in order from the producer: each a domain of its own, so what
/// crosses is serialized and deserialized at every gate, and enters the next gate when it
/// left the one before.
pub struct Gates(pub Vec<Gate>);

impl Gates {
    pub fn new(specs: &[GateSpec]) -> Self {
        Self(specs.iter().map(|s| Gate::new(*s)).collect())
    }

    /// What `value` sent at `now` (if any) and the values sent before it come to on the far
    /// side by `now`, in arrival order.
    pub fn pass<T: Wire>(&mut self, now: f64, value: Option<T>) -> Vec<T> {
        let mut vals: Vec<(f64, T)> = value.map(|v| (now, v)).into_iter().collect();
        for g in &mut self.0 {
            let mut next = g.pass::<T>(now, None, now);
            for (at, v) in &vals {
                next.extend(g.pass(*at, Some(v), now));
            }
            vals = next;
        }
        vals.into_iter().map(|(_, v)| v).collect()
    }

    pub fn stats(&self) -> Vec<GateStats> {
        self.0.iter().map(Gate::stats).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Vec<WorldUpdate> {
        let mut d = DenseChunk::filled(BlockId(3));
        d.0[17] = BlockId(9);
        vec![
            WorldUpdate::ChunkLoad { key: ChunkKey { pos: IVec3::new(1, -2, 3), lod: 1 }, data: ChunkData::Dense(d.into()) },
            WorldUpdate::ChunkLoad { key: ChunkKey::lod0(IVec3::ZERO), data: ChunkData::Uniform(BlockId(2)) },
            WorldUpdate::ChunkUnload { key: ChunkKey::lod0(IVec3::X) },
            WorldUpdate::SetBlocks(vec![BlockEdit { pos: IVec3::new(5, 6, -7), block: BlockId(4) }]),
            WorldUpdate::LightUpsert(vec![Light { id: 7, position: DVec3::new(1.5, 2.0, -3.0), color: Vec3::ONE, intensity: 2.0, radius: 9.0 }]),
            WorldUpdate::LightRemove(vec![1, 2]),
            WorldUpdate::SetEnvironment(Environment::default()),
        ]
    }

    fn same(a: &[WorldUpdate], b: &[WorldUpdate]) -> bool {
        format!("{a:?}") == format!("{b:?}")
    }

    /// Every serialized form reads back what was written, and garbage fails instead of
    /// allocating what a garbled count says.
    #[test]
    fn round_trips() {
        let w = world();
        let mut b = vec![];
        w.encode(&mut b);
        assert!(same(&Vec::<WorldUpdate>::decode(&b).unwrap(), &w));
        let f = Frame { width: 3, height: 2, pixels: (0..24).collect() };
        let mut b = vec![];
        f.encode(&mut b);
        assert_eq!(Frame::decode(&b).unwrap(), f);
        let hits: Vec<[f32; 4]> = (0..10).map(|i| [i as f32, 1.0, 2.0, 3.0]).collect();
        let mut b = vec![];
        encode_pod(&hits, &mut b);
        assert_eq!(decode_pod::<[f32; 4]>(&b).unwrap(), hits);
        assert!(Vec::<WorldUpdate>::decode(&[0xff, 0xff, 0xff, 0xff, 0]).is_err());
        assert!(decode_pod::<[f32; 4]>(&[0xff; 9]).is_err());
    }

    /// Latency delays, bandwidth spaces out, and on a free line the bytes arrive at once.
    #[test]
    fn latency_and_bandwidth() {
        let mut g = Gate::new(GateSpec { latency_ms: 100.0, bandwidth_mbps: 8e-6 * 1000.0, mtu: 500, ..Default::default() });
        // 1000 bytes a second, two packets.
        g.send(0.0, vec![1; 1000]);
        let mut out = vec![];
        g.deliver(1.05, &mut out);
        assert!(out.is_empty());
        g.deliver(1.1, &mut out);
        assert_eq!(out.iter().map(|m| m.1.clone()).collect::<Vec<_>>(), [vec![1; 1000]]);
        assert_eq!((g.stats().packets, g.stats().bytes), (2, 1000));
        let mut free = Gate::new(GateSpec::default());
        assert_eq!(free.pass(0.0, Some(&Frame { width: 1, height: 1, pixels: vec![1, 2, 3, 4] }), 0.0).len(), 1);
    }

    /// Loss on the bytes: without resends a lost packet arrives as zeros and the data is what
    /// those bytes say (or fails to decode); with resends everything arrives, later. The same
    /// seed loses the same packets. Shown on the three kinds of edge: the world's changes,
    /// hit records and a frame.
    #[test]
    fn loss_on_three_edges() {
        let spec = GateSpec { latency_ms: 5.0, jitter_ms: 5.0, loss: 0.3, mtu: 64, seed: 11, ..Default::default() };
        // A frame: lost packets are black runs, the size intact while the header survives.
        let f = Frame { width: 64, height: 64, pixels: vec![200; 64 * 64 * 4] };
        let mut gates = Gates::new(&[spec, GateSpec { seed: 12, ..spec }]);
        let mut got = gates.pass(0.0, Some(f.clone()));
        got.extend(gates.pass(1.0, None));
        let st = gates.stats();
        assert!(st.iter().all(|s| s.messages == 1 && s.lost_packets > 0), "{st:?}");
        if let Some(g) = got.first() {
            let zeros = g.pixels.iter().filter(|p| **p == 0).count();
            assert!(zeros > 0 && zeros < g.pixels.len(), "{zeros}");
        } else {
            assert!(st.iter().any(|s| s.failed == 1));
        }
        // Hit records: same, zeroed records where packets were lost.
        let hits: Vec<[u32; 4]> = (0..400).map(|i| [i, 1, 2, 3]).collect();
        let mut g = Gate::new(spec);
        let mut b = vec![];
        encode_pod(&hits, &mut b);
        g.send(0.0, b);
        let mut out = vec![];
        g.deliver(1.0, &mut out);
        match decode_pod::<[u32; 4]>(&out[0].1) {
            Ok(h) => assert!(h != hits && h.contains(&[0; 4])),
            Err(_) => assert!(out[0].1[..4].contains(&0)),
        }
        // The world's changes: resent, everything arrives intact.
        let w = world();
        let mut g = Gate::new(GateSpec { resend: true, ..spec });
        assert!(g.pass(0.0, Some(&w), 0.0).is_empty(), "nothing arrives at once");
        let got = g.pass(10.0, None::<&Vec<WorldUpdate>>, 10.0);
        assert!(same(&got[0].1, &w));
        assert!(g.stats().lost_packets > 0 && g.stats().packets > g.stats().lost_packets);
        // Without resends, lost bytes do what they do: a different world or none.
        let mut g = Gate::new(spec);
        g.pass(0.0, Some(&w), 0.0);
        let got = g.pass(10.0, None::<&Vec<WorldUpdate>>, 10.0);
        assert!(got.is_empty() && g.stats().failed == 1 || !same(&got[0].1, &w));
        // Reproducible.
        let run = || {
            let mut g = Gate::new(spec);
            g.send(0.0, vec![9; 4000]);
            let mut o = vec![];
            g.deliver(9.0, &mut o);
            o
        };
        assert_eq!(run(), run());
    }

    /// In order unless `reorder`: a small message after a big one waits for it.
    #[test]
    fn order() {
        let run = |reorder| {
            let mut g = Gate::new(GateSpec { bandwidth_mbps: 1.0, reorder, ..Default::default() });
            g.send(0.0, vec![1; 100_000]);
            // The small one on a line of its own (as if sent over another path): quick.
            g.line = 0.0;
            g.send(0.0, vec![2; 10]);
            let mut o = vec![];
            g.deliver(9.0, &mut o);
            o.iter().map(|m| m.1[0]).collect::<Vec<u8>>()
        };
        assert_eq!(run(false), [1, 2]);
        assert_eq!(run(true), [2, 1]);
    }
}
