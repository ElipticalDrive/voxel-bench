# MultiRenderer

A test bench for voxel renderers. A renderer is assembled from components: a storage,
a way of finding what is visible, a ray walk, a shading model, and so on. These pieces are
connected as a pipeline graph, and each piece can run on the CPU or the GPU. Many such
renderers draw the same scenes side by side, each in its own worker process, and are
measured against a reference renderer and against each other.

The world is a lattice of unit cubes: each cell is air or an opaque block with a 16×16
texture per face. Point lights take up no space and cannot be seen. Worlds are unbounded and
streamed in chunks of 32³ cells.

## Install

Linux and Windows, with Python 3.9+. On Windows, install Python from python.org (for example
`winget install Python.Python.3.13`) and type `py` instead of `python3` below. `python3` on
Windows usually starts the Microsoft Store's Python, which hides the files it writes under
AppData from other programs (the build cannot find its sources); the installer detects this
and stops.

```sh
python3 install.py              # a wizard: install, update or uninstall
python3 install.py install --yes
python3 install.py update       # new sources, rebuild; plans you edited are kept
python3 install.py uninstall    # --purge also removes your plans, runs and the prerequisites it installed
```

The installer asks before it installs anything that is missing: the Rust toolchain (rustup,
with `llvm-tools`) and a C compiler (on Windows, Visual Studio Build Tools with C++ and ATL).
It then builds from this checkout (or from `--source <git URL or directory>`) into
`~/.local/share/multirenderer` (Windows: `%LOCALAPPDATA%\multirenderer`). It puts
`multirenderer` on your PATH and adds a desktop or Start menu entry. The sources and the
toolchain stay installed, because pipelines are compiled against them when they are needed.

To build by hand, run `cargo build --release`. On Windows this also needs ATL, because DX12's
shader compiler is linked into the binary.

## Run

```sh
multirenderer                                    # the GUI (or: gui plans/terrain.toml)
multirenderer list                               # back ends, their slots and the components
multirenderer run plans/terrain.toml             # benchmark, compare with the reference, report
multirenderer view plans/terrain.toml -r gpu-compute:prefix+incremental -r gpu-raster:bricks+greedy+raster
multirenderer graph gpu-compute:prefix+incremental   # a pipeline as a graph
multirenderer diff a.png b.png --out diff.png
```

`plans/` holds example run plans: terrain, menger, room, edits, many lights, meshes,
streaming, a modelled network link, two GPUs and others.

## The GUI

- **Left:** the plan, edited as a form or as raw TOML: the world, camera path, lights,
  scripted events, the renderers and the run settings.
  It starts plain (terrain, the sun, one renderer); **🎬 Demos** loads showier set-ups:
  many lights, path tracing on discovery, meshes, a modelled network, two GPUs.
- **Centre:** every renderer in the plan runs live, side by side, each as fast as it can.
  Fly with WASD, Space and Shift, and look around with the mouse. F11 switches to fullscreen.
  The ❓ tab lists every key.
- **Right:** live metrics and each renderer's knobs. **Compare** scores the current view
  against the reference (PSNR, SSIM, diff images). **Benchmark** runs the whole plan and
  shows its report.
- **✏ graph** opens the pipeline editor: the renderer drawn as a graph from the world to the
  image. Click a node to change its component, device, inputs or knobs. A choice that cannot
  run is greyed out, and hovering over it says why. In the live view, edits take effect
  immediately.

Each renderer has an auto-tuner that adjusts its knobs towards a goal, for example the best
quality at 120 fps.

## Documentation

- [docs/gui.md](docs/gui.md): the GUI, keys, the pipeline editor, knobs, quality probes and
  the auto-tuner.
- [docs/pipelines.md](docs/pipelines.md): pipelines as graphs: nodes, components, back ends,
  devices, timing, and pipelines compiled for the local machine.
- [docs/plans.md](docs/plans.md): run plans, sweeps, observations, the catalogue, and the
  scripts.
- [docs/architecture.md](docs/architecture.md): processes, streaming, measurement, the
  conventions renderers follow, the code layout, and how to add a component.
- [docs/design.md](docs/design.md): the requirements behind the architecture and how each is
  met: pipelines as graphs, composition at no cost, a device per node, compiling on demand.
- [docs/schemes/](docs/schemes/README.md): the encyclopedia of volume-rendering schemes, one
  entry per technique: what it is, how it works, where it is implemented, what was measured.
- [ROADMAP.md](ROADMAP.md): ideas for the bench itself, not built yet.
- [docs/findings.md](docs/findings.md): what has been measured so far, in brief.
