+++
title = "The served split: server, link and client"
category = "world"
status = "built"
components = []
related = ["link", "client", "generator"]
origin = "user"
sources = []
+++

# The served split: server, link and client

## What it is

A strict line between what is local to the renderer and what is not, as if the bench were a
served game: a **server** (the authoritative world, the generator, simulation), a **link**
(a modelled network), and a **client** (the renderer and everything it derives). Imposing
that line on every renderer tests techniques in a practical setting: they must tolerate data
that has not arrived yet. The point is the separation and the bottleneck between server and
client; the link's knobs are a way to stress an algorithm's resilience, not a goal of
realistic networking.

## How it works

The host is the server and sends one stream for every client. In each worker the link delays
it, the client derives its renderer's region, and the generator may run there instead of on
the server. All three are graph nodes, so the network is one more edge between devices. See
[link](link.md), [client](client.md) and [generator](generator.md).

## In this project

`crates/mr-world/src/stream.rs` (the server's stream), `crates/mr-core/src/{link,client}.rs`,
`crates/mr-serve` (the worker side). Plans `link.toml`, `stream.toml`.

## Measured

See the link's table: uncompressed 64 KiB chunks make bandwidth the bottleneck.

## Open questions

- Compression, and a server that paces itself to the link.
- A GPU generator; a server sending edits by region instead of all of them.
