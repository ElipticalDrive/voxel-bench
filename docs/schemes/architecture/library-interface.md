+++
title = "The bench as a renderer library"
category = "architecture"
status = "idea"
components = []
related = ["learning-planner", "pipeline-graph"]
origin = "user"
sources = []
+++

# The bench as a renderer library

**Untested idea.**

## What it is

A simpler, library-style interface so the project can be used as a renderer: the caller's
scene, edits and meshes are the input, and the planner picks how to render them.

## How it works

The `Renderer` trait with `WorldUpdate`, mesh updates and `FrameRequest` is close to the input
side already. A library also needs: renderers in-process (`isolation = "thread"` exists); one
canonical world copy on the library's side, so a pipeline switch can re-stream the world into
the new renderer (the live session does this for hot swaps); and output into the caller's
surface: frames are read back to host memory today, so GPU back ends would have to render into
a caller-provided device and texture (devices are already handed in, `GpuPool::fixed`).

## In this project

Not built.

## Measured

Untested.
