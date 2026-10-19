+++
title = "World generation on the server or the client"
category = "world"
status = "measured"
components = ["generator/procedural"]
related = ["link", "client"]
origin = "project"
sources = []
+++

# World generation on the server or the client

## What it is

Where the procedural world is generated: on the server, which then streams chunks, or on the
client's CPU from the world's spec (a shared seed), so that only edits cross the link.

## How it works

The generator node feeds the world (server) or the client (`cpu`). A generating client makes
its region itself, nearest chunks first, within a budget per frame; joining late, it receives
the edits made so far.

## In this project

`crates/mr-world/src/generators.rs` (terrain, flat, menger, room, random, showcase, empty, and the stress worlds city, islands,
lattice, gyroid, checker; see `docs/plans.md` § Worlds)
and `stream.rs` (`Generating`); in plans `sites = { generator = "cpu" }`.

## Measured

Generation costs wall time on the client: the first frame after 144 ms (radius 6), 610 ms
(`derive`, radius 3 + 2 levels), 1.45 s (`pick` with coarse levels: the generator making
coarse chunks); updates 1.0–5.5 ms per frame against 0.25–1.1 ms streamed. In exchange the
link carries edits only.
