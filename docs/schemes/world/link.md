+++
title = "A modelled network link"
category = "world"
status = "measured"
components = ["link/free", "link/net"]
related = ["client", "generator"]
origin = "project"
sources = []
+++

# A modelled network link

## What it is

The world's stream from the server to a renderer's client, passed through a simulated
network, so that a renderer can be measured as the client of a remote game server: what
arrives late or not at all shows up as holes and lost quality.

## How it works

`net` has knobs `latency_ms`, `jitter_ms`, `bandwidth_mbps`, `loss` (resent after a timeout),
`reorder` (chunks may overtake each other) and `seed`. Chunks are sent uncompressed (a dense
one is 64 KiB). The link runs in the worker, in front of the client; time is the run's
simulated clock (`run.clock_fps`), so runs repeat. `free` delivers everything at once, in
order. Each frame, the worker reports bytes sent and arrived and the chunks still missing.

## In this project

`crates/mr-core/src/link.rs`; in plans `sites = { link = "net(latency_ms=30, bandwidth_mbps=200)" }`;
examples `plans/link.toml` and `plans/stream.toml`.

## Measured

`plans/stream.toml` (`bricks` + `refloor`, terrain, radius 6, 600 frames at ~30 m/s, random
edits, unlit):

| link | holes mean / max | first complete frame | PSNR |
|---|---|---|---|
| free | 0 | #0 | 69.4 |
| lan (1 ms, 1 Gbit/s) | 13 / 640 | #21 (0.35 s) | 62.7 |
| home (30 ± 10 ms, 200 Mbit/s, 1% loss, reorder) | 66 / 642 | #111 (1.85 s) | 49.7 |
| bad (150 ± 80 ms, 50 Mbit/s, 5% loss, reorder) | 408 / 643 | never | 16.6 |

With the generator on the client, only edits cross the link (2.7–2.9 MB), and the PSNR loss
is late edits only (63.3 behind a 1 ms link).
