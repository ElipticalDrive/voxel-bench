+++
title = "A cached inventory of every device and API"
category = "architecture"
status = "idea"
components = []
related = ["device-per-node", "learning-planner"]
origin = "user"
sources = []
+++

# A cached inventory of every device and API

**Untested idea.**

## What it is

Enumerate every available graphics API at start-up and cache the result, so that a node's API
becomes a real choice per device, offered from what the machine has, instead of fixed to
Vulkan. (Raised after a Windows guest without Vulkan found no adapter.)

## How it works

`wgpu::Instance` with `Backends::all()` and `enumerate_adapters` lists every adapter on every
API (Vulkan, DX12 and GL on Windows; Vulkan and GL on Linux; Metal on macOS); one GPU appears
once per API. Adapters are live driver handles and cannot be cached, but the *inventory* can,
per machine, on disk: per adapter its name, API, device type, driver version, features and
limits. The GUI, the planner and plan validation read it without touching a driver; a worker
opens only the API it was assigned. Invalidation: driver versions plus one cheap API's
adapter list.

## In this project

Not built. Today each worker creates its own instance and picks Vulkan unless `WGPU_BACKEND`
says otherwise.

## Measured

Untested. The cost of bringing up every API (GL may create a hidden context) is not known;
likely tens of milliseconds.

## Open questions

- Several Vulkan instances on threads of one process are not safe with debug labels, so
  enumerating more APIs needs the same care.
- Measurements must record the API used (they record the adapter's backend already).
- DX12 needs its shader compiler (DXC) shipped; it is linked in on Windows builds.
