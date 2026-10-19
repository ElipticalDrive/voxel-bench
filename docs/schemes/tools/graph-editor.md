+++
title = "A graph-first pipeline editor"
category = "tools"
status = "built"
components = []
related = ["docking", "pipeline-graph"]
origin = "user"
sources = []
+++

# A graph-first pipeline editor

## What it is

A GUI for building pipelines that reflects the graph, not the old monolithic back ends: start
from the world and the image and bind ports; device and API a choice on every node; the back
end derived from the graph and only shown; shared nodes shown as shared; edge timings visible
and editable; every choice tried first and greyed with the reason when the result cannot run.

## How it works

See [docs/gui.md](../../gui.md#the-pipeline-editor): a canvas laid out from the world to the
image, an inspector for the selected node, every listed choice completed into candidate graphs
and tried before it is offered, live edits hot-swapping the lane.

## In this project

`src/gui/graph/` (`canvas.rs`, `model.rs`, `mod.rs`).

## Measured

Every choice the editor lists on every preset (1,100) is answered in 0.26 ms each: 930 run,
170 refused with genuine reasons.

## Open questions

The owner's feedback, not built yet: dragging a node out of view zooms awkwardly (the fit is
recomputed every frame); mouse-centred zoom by default, with an optional mode that re-frames
the graph when a dragged node is released; the editor in its own OS window.
