+++
title = "Docking panes"
category = "tools"
status = "idea"
components = []
related = ["graph-editor"]
origin = "user"
sources = []
+++

# Docking panes

**Untested idea.**

## What it is

As in common GUI tools: every pane (workflow form, TOML, metrics, compare, benchmark, keys,
log, the pipeline editor, ...) can be torn off into its own OS window or rearranged in the main
one. Fullscreen stays on the display the main window is on, also with panes torn off.

## How it works

Three docks: left, right and bottom; the bottom spans the full width, left and right run down
to its upper edge; the middle is the main view (the renderers' viewport, or the canvas). An
empty dock collapses; a full one may be collapsed by the user; collapsed, a dock is a thin
border at the screen's edge (click or hover to open). Candidates: `egui_dock` (tabs, splits,
tear-off into viewports) if it fits the three-dock layout, else a thin dock layer over egui's
multi-viewport support.

## In this project

Not built.

## Measured

Untested.
