+++
title = "A planner that learns the machine"
category = "architecture"
status = "idea"
components = []
related = ["library-interface", "compile-on-demand", "device-inventory"]
origin = "user"
sources = ["M. Frigo, S. G. Johnson, The design and implementation of FFTW3, Proceedings of the IEEE 2005 (wisdom: plans measured on the machine and kept)"]
+++

# A planner that learns the machine

**Untested idea.**

## What it is

Something like the auto-tuner, but with memory across sessions about the user's machine: it
uses all the information it can to pick a decent render plan (pipeline and knobs) very
cheaply, and keeps learning, the whole time the user interacts with it, how best to render
things on that machine. It replaces or extends the tuner. Sweeps over every combination give
it data about every pipeline on this machine.

## How it works

A sketch:
- **Features, not scenario names:** a cheap feature vector of the scene and request
  (resolution, view radius, solid and surface fractions, camera speed and turn rate, edits per
  frame, lights, mesh triangles, mode), so it generalises from the sweep's scenarios. The
  observations already record these features.
- **Constraints, then the objective:** mode support, a quality floor, memory budgets filter;
  frame time (mean, p99), latency and later power are the objective.
- **Two levels:** a pipeline change costs a hot swap (seconds), a knob change is live; so a
  learned model picks the pipeline rarely (at scene load, or on large drift) and the existing
  Bayesian tuner tunes its knobs continuously.
- **Model:** small and honest about uncertainty (per-pipeline regression of log frame time on
  the features, or nearest neighbours); exploration on a hidden standby lane or in idle time,
  so the user sees no stutter. Learning during use is a contextual bandit.
- **Noise and drift:** observations keyed by machine and driver, recent ones weighted more,
  the incumbent re-measured now and then.
- Per-stage timings let it predict a pipeline as a sum of stages instead of measuring every
  combination.

## In this project

Not built. What exists for it: observations with machine, scenario features, pipeline key and
per-stage times (`observations.jsonl`), sweeps, the catalogue, the tuner.

## Measured

Untested.
