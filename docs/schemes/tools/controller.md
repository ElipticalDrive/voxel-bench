+++
title = "Game controller support"
category = "tools"
status = "idea"
components = []
related = ["graph-editor", "docking"]
origin = "user"
sources = []
+++

# Game controller support

**Untested idea.**

## What it is

Flying the camera with a gamepad (sticks for move and look, triggers for speed, buttons for
the mode and lane toggles) in the GUI's viewport and the view and presenter windows, beside
the keyboard and mouse.

## How it works

A gamepad library (e.g. `gilrs`, cross-platform) polled once per frame into the same camera
control the keys drive, with a dead zone and a response curve; the bindings shown in the keys
pane. Analog input makes recorded flights smoother, so a recorded controller path could also
serve as a camera path for benchmarks.

## In this project

Not built.

## Measured

Untested.
