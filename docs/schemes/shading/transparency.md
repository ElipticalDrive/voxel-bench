+++
title = "Transparency and surface effects"
category = "shading"
status = "idea"
components = []
related = ["deferred-shading", "direct", "pathtrace"]
origin = "user"
sources = []
+++

# Transparency and surface effects

**Untested idea.**

## What it is

Translucent blocks, and possibly shading effects on transparent surfaces such as ripples (a
surface's normal or appearance perturbed by a function, for water and the like). In the
project owner's view the major hurdle left once the architecture is done; after it, the
requirements on renderers are largely set.

## How it works

Transparency needs layers (several hits per pixel, or a continuation: a ray that goes on past
a translucent hit) and an opaque-only query for shadow rays. Surface effects fit as a
component on the shading's surface input (a perturbation of normal and texel), so every
strategy gets them. The rule: the feature must cost other pipelines nothing at run time, so
layers, continued rays, the opaque-only query and surface effects are optional capabilities
of the hit record, the ray queries and the shading, compiled in only where a graph chooses
them: no runtime flag in a hot path, no wider record, no extra pass for opaque pipelines.

## In this project

Not built. The hit record reserves its high bits for optional capabilities (bits 23–31), so
layers can be added without widening it for everyone.

## Measured

Untested. A measured reason for the rule: a shared shading call with a runtime lit flag cost
4–6% on NVIDIA.
