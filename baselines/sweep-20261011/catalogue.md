# Renderer catalogue

3047 observations from 10 runs, 17 scenarios. Machine: AMD Ryzen 9 9955HX 16-Core Processor + NVIDIA GeForce RTX 5060 Laptop GPU.

How to read: a scenario is a plan (scene, camera path, events) in one pass mode. Speed is throughput (completed frames per second; at several hundred fps and more the host's per-frame overhead compresses differences, so GPU ms is shown too). *rel* is fps relative to the fastest correct renderer of the scenario; correct means a mean PSNR against the reference of at least 35 dB (or not compared). *good at*: within 10% of the best. *stands out*: relative speed at least 1.25× the renderer's own average, i.e. where it is comparatively strong.

## Best per scenario

| scenario | best | fps | gpu ms | PSNR | runner-up | rel |
|---|---|---:|---:|---:|---|---:|
| edits [lit] | gpu-compute: tree64 + refloor | 534.7 | 0.90 | 56.4 | gpu-compute: tree64 + incremental | 0.97 |
| edits [unlit] | gpu-compute: tree64 + beam + refloor | 536.7 | 0.65 | 63.7 | gpu-compute: tree64 + refloor | 0.99 |
| far [lit] | gpu-compute: bricks + refloor | 392.8 | 1.18 | 56.7 | gpu-compute: bricks + beam + refloor | 0.93 |
| far [unlit] | gpu-compute: bricks + refloor | 428.4 | 0.88 | 57.7 | gpu-compute: bricks + beam + refloor | 0.94 |
| many-lights [lit] | gpu-rt: greedy + rayquery | 228.7 | 2.35 | 60.3 | gpu-raster: prefix + naive + visbuffer | 0.94 |
| menger [lit] | gpu-compute: hier + refloor | 1652.2 | 0.50 | 59.3 | gpu-compute: bricks + refloor | 0.96 |
| menger [unlit] | gpu-compute: hier + refloor | 2486.9 | 0.29 | 55.7 | gpu-compute: bricks + refloor | 0.95 |
| meshes [lit+meshes] | gpu-raster: prefix + greedy + raster | 3592.4 | 0.16 | 71.1 | gpu-raster: svo_dag + greedy + raster | 0.95 |
| meshes [unlit+meshes] | gpu-raster: bricks + naive + raster | 4465.1 | 0.08 | 60.2 | gpu-raster: bricks + greedy + raster | 0.93 |
| random [unlit] | gpu-compute: bricks + refloor | 1853.9 | 0.43 | 56.2 | gpu-compute: rows + refloor | 0.96 |
| room [lit] | gpu-raster: hier + greedy + raster | 3494.9 | 0.19 | 72.7 | gpu-raster: svo_dag + greedy + raster | 1.00 |
| room [unlit] | gpu-raster: bricks + greedy + raster | 5719.9 | 0.05 | 64.4 | gpu-raster: bricks + naive + raster | 0.94 |
| showcase [lit] | gpu-raster: svo_dag + naive + visbuffer | 4255.6 | 0.15 | 44.0 | gpu-raster: prefix + naive + visbuffer | 0.99 |
| showcase [unlit] | gpu-raster: bricks + naive + raster | 7870.8 | 0.03 | 44.1 | gpu-raster: bricks + naive + visbuffer | 0.87 |
| streaming [unlit] | gpu-compute: bricks + refloor | 486.1 | 0.78 | 62.1 | gpu-compute: tree64 + refloor | 0.91 |
| terrain-flight [lit] | gpu-compute: bricks + refloor | 574.2 | 1.22 | 57.8 | gpu-compute: prefix + refloor | 0.97 |
| terrain-flight [unlit] | gpu-compute: bricks + refloor | 652.4 | 0.86 | 59.5 | gpu-compute: prefix + refloor | 0.96 |

## Renderers

Sorted by the geometric mean of *rel* over the scenarios where the renderer ran correctly.

| renderer | scenarios | geomean rel | wins | good at | stands out | weak at (rel < 0.25) | incorrect | failed |
|---|---:|---:|---:|---|---|---|---|---|
| gpu-compute: bricks + refloor | 17 | 0.69 | 6 | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | far [lit] (1.5×), far [unlit] (1.5×), random [unlit] (1.5×) | room [unlit] |  |  |
| gpu-compute: hier + refloor | 17 | 0.67 | 2 | menger [lit], menger [unlit], random [unlit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] | menger [lit] (1.5×), menger [unlit] (1.5×), terrain-flight [unlit] (1.4×) |  |  |  |
| gpu-compute: prefix + refloor | 17 | 0.67 | 0 | menger [lit], menger [unlit], random [unlit], terrain-flight [lit], terrain-flight [unlit] | terrain-flight [lit] (1.5×), terrain-flight [unlit] (1.4×), menger [unlit] (1.4×) | room [unlit] |  |  |
| gpu-compute: prefix + beam + refloor | 17 | 0.60 | 0 | terrain-flight [unlit] | terrain-flight [unlit] (1.5×), many-lights [lit] (1.5×), terrain-flight [lit] (1.5×) | room [unlit] |  |  |
| gpu-compute: rows + refloor | 17 | 0.60 | 0 | menger [lit], menger [unlit], random [unlit] | random [unlit] (1.6×), menger [lit] (1.6×), menger [unlit] (1.5×) | room [unlit] |  |  |
| gpu-compute: prefix + rowrun | 17 | 0.60 | 0 |  | terrain-flight [unlit] (1.5×), terrain-flight [lit] (1.4×), far [lit] (1.3×) | room [unlit] |  |  |
| gpu-compute: tree64 + refloor | 17 | 0.60 | 1 | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit] (1.7×), edits [unlit] (1.6×), far [unlit] (1.5×) | meshes [unlit+meshes], room [unlit] |  |  |
| gpu-raster: hier + naive + visbuffer | 8 | 0.60 | 0 | room [lit] | room [lit] (1.5×), showcase [lit] (1.5×), many-lights [lit] (1.4×) |  |  |  |
| gpu-compute: hier + beam + refloor | 17 | 0.60 | 0 |  | terrain-flight [unlit] (1.5×), terrain-flight [lit] (1.4×), showcase [lit] (1.4×) | room [unlit] |  |  |
| gpu-raster: tree64 + naive + visbuffer | 8 | 0.60 | 0 |  | room [lit] (1.4×), showcase [lit] (1.4×) |  |  |  |
| gpu-compute: bricks + beam + refloor | 17 | 0.60 | 0 | edits [lit], far [lit], far [unlit], terrain-flight [lit], terrain-flight [unlit] | far [unlit] (1.6×), far [lit] (1.6×), terrain-flight [lit] (1.5×) | room [unlit] |  |  |
| gpu-compute: svo_dag + refloor | 17 | 0.59 | 0 |  | showcase [lit] (1.5×), menger [lit] (1.4×), menger [unlit] (1.4×) | room [unlit] |  |  |
| gpu-raster: prefix + naive + visbuffer | 8 | 0.59 | 0 | many-lights [lit], showcase [lit] | showcase [lit] (1.7×), many-lights [lit] (1.6×), room [lit] (1.4×) |  |  |  |
| gpu-raster: svo_dag + naive + visbuffer | 8 | 0.58 | 1 | showcase [lit] | showcase [lit] (1.7×), room [lit] (1.5×), many-lights [lit] (1.3×) |  |  |  |
| gpu-raster: clipmap + naive + visbuffer | 8 | 0.58 | 0 |  | many-lights [lit] (1.5×), showcase [lit] (1.4×), room [lit] (1.3×) |  |  |  |
| gpu-compute: rows + rowrun | 17 | 0.57 | 0 |  | menger [lit] (1.5×), menger [unlit] (1.5×), terrain-flight [unlit] (1.5×) | room [unlit] |  |  |
| gpu-raster: tree64 + naive + gbuffer | 8 | 0.56 | 0 |  | showcase [lit] (1.4×), room [lit] (1.3×) |  |  |  |
| gpu-raster: hier + naive + gbuffer | 8 | 0.56 | 0 |  | showcase [lit] (1.6×), many-lights [lit] (1.5×) |  |  |  |
| gpu-raster: bricks + naive + visbuffer | 17 | 0.56 | 0 |  | many-lights [lit] (1.6×), showcase [unlit] (1.6×), meshes [unlit+meshes] (1.6×) |  |  |  |
| gpu-compute: clipmap + refloor | 17 | 0.56 | 0 | random [unlit] | random [unlit] (1.6×), menger [unlit] (1.5×), terrain-flight [unlit] (1.4×) | meshes [lit+meshes], meshes [unlit+meshes], room [unlit] |  |  |
| gpu-raster: prefix + naive + gbuffer | 8 | 0.56 | 0 | many-lights [lit], showcase [lit] | showcase [lit] (1.6×), many-lights [lit] (1.6×), room [lit] (1.3×) |  |  |  |
| gpu-raster: prefix + greedy + visbuffer | 8 | 0.55 | 0 | room [lit] | room [lit] (1.8×), many-lights [lit] (1.6×), showcase [lit] (1.6×) | edits [lit] |  |  |
| gpu-compute: prefix + beam + rowrun | 17 | 0.55 | 0 |  | showcase [lit] (1.5×), terrain-flight [unlit] (1.4×), terrain-flight [lit] (1.4×) | room [unlit] |  |  |
| gpu-raster: svo_dag + naive + gbuffer | 8 | 0.55 | 0 |  | showcase [lit] (1.6×), many-lights [lit] (1.4×), room [lit] (1.3×) |  |  |  |
| gpu-compute: rows + beam + refloor | 17 | 0.54 | 0 |  | many-lights [lit] (1.5×), terrain-flight [unlit] (1.5×), terrain-flight [lit] (1.5×) | room [unlit] |  |  |
| gpu-raster: clipmap + naive + gbuffer | 8 | 0.54 | 0 |  | many-lights [lit] (1.6×), showcase [lit] (1.4×), terrain-flight [lit] (1.3×) |  |  |  |
| gpu-compute: bricks + rowrun | 17 | 0.53 | 0 |  | edits [unlit] (1.7×), edits [lit] (1.6×), far [unlit] (1.6×) | meshes [unlit+meshes], room [unlit] |  |  |
| gpu-raster: hier + greedy + visbuffer | 8 | 0.53 | 0 |  | room [lit] (1.7×), showcase [lit] (1.5×), many-lights [lit] (1.5×) | edits [lit] |  |  |
| gpu-raster: svo_dag + greedy + visbuffer | 8 | 0.53 | 0 | room [lit] | room [lit] (1.7×), showcase [lit] (1.6×), meshes [lit+meshes] (1.6×) | edits [lit] |  |  |
| gpu-raster: tree64 + greedy + visbuffer | 8 | 0.53 | 0 |  | room [lit] (1.6×), meshes [lit+meshes] (1.4×), showcase [lit] (1.4×) | edits [lit] |  |  |
| gpu-raster: bricks + naive + raster | 17 | 0.53 | 2 | meshes [unlit+meshes], room [unlit], showcase [unlit] | meshes [unlit+meshes] (1.9×), showcase [unlit] (1.9×), room [unlit] (1.8×) | menger [lit] |  |  |
| gpu-raster: rows + naive + gbuffer | 8 | 0.52 | 0 |  | many-lights [lit] (1.6×), showcase [lit] (1.5×), terrain-flight [lit] (1.3×) |  |  |  |
| gpu-raster: bricks + naive + gbuffer | 17 | 0.52 | 0 | many-lights [lit] | many-lights [lit] (1.7×), showcase [lit] (1.6×), showcase [unlit] (1.5×) |  |  |  |
| gpu-compute: tree64 + beam + refloor | 17 | 0.52 | 1 | edits [lit], edits [unlit] | edits [unlit] (1.9×), edits [lit] (1.8×), streaming [unlit] (1.5×) | meshes [unlit+meshes], room [unlit] |  |  |
| gpu-compute: bricks + beam + rowrun | 17 | 0.52 | 0 | edits [unlit] | edits [unlit] (1.8×), edits [lit] (1.7×), far [lit] (1.6×) | meshes [unlit+meshes], room [unlit] |  |  |
| gpu-raster: rows + greedy + visbuffer | 8 | 0.52 | 0 |  | many-lights [lit] (1.6×), room [lit] (1.5×), showcase [lit] (1.5×) | edits [lit] |  |  |
| gpu-raster: clipmap + greedy + visbuffer | 8 | 0.51 | 0 |  | many-lights [lit] (1.6×), room [lit] (1.5×), showcase [lit] (1.4×) | edits [lit] |  |  |
| gpu-rt: naive + rayquery + pathtrace | 7 | 0.51 | 0 |  |  |  | edits [lit], menger [lit], room [lit], terrain-flight [lit] | far [lit] |
| gpu-compute: rows + beam + rowrun | 17 | 0.51 | 0 |  | terrain-flight [unlit] (1.5×), terrain-flight [lit] (1.4×), far [lit] (1.4×) | room [unlit] |  |  |
| gpu-raster: rows + naive + visbuffer | 8 | 0.51 | 0 |  | many-lights [lit] (1.7×), showcase [lit] (1.7×), meshes [lit+meshes] (1.3×) |  |  |  |
| gpu-rt: greedy + rayquery + pathtrace | 7 | 0.51 | 0 |  |  |  | edits [lit], far [lit], menger [lit], room [lit], terrain-flight [lit] |  |
| gpu-rt: boxes + rayquery | 15 | 0.50 | 0 |  | terrain-flight [lit] (1.4×), terrain-flight [unlit] (1.3×) | room [unlit] |  |  |
| gpu-raster: prefix + greedy + gbuffer | 8 | 0.50 | 0 |  | many-lights [lit] (1.7×), showcase [lit] (1.6×), room [lit] (1.5×) | edits [lit] |  |  |
| gpu-raster: prefix + greedy + raster | 8 | 0.50 | 1 | meshes [lit+meshes], room [lit] | meshes [lit+meshes] (2.0×), room [lit] (2.0×), showcase [lit] (1.4×) | edits [lit] |  |  |
| gpu-compute: hier + rowrun | 17 | 0.49 | 0 |  | showcase [unlit] (1.5×), far [lit] (1.3×), terrain-flight [unlit] (1.3×) | room [unlit] |  |  |
| gpu-compute: clipmap + rowrun | 17 | 0.49 | 0 |  | edits [unlit] (1.5×), edits [lit] (1.5×), streaming [unlit] (1.5×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: clipmap + greedy + gbuffer | 8 | 0.49 | 0 |  | many-lights [lit] (1.6×), room [lit] (1.4×), showcase [lit] (1.4×) | edits [lit] |  |  |
| gpu-raster: prefix + naive + raster | 8 | 0.49 | 0 |  | room [lit] (1.6×), showcase [lit] (1.6×), meshes [lit+meshes] (1.5×) | menger [lit] |  |  |
| gpu-raster: hier + greedy + gbuffer | 8 | 0.48 | 0 |  | room [lit] (1.6×), many-lights [lit] (1.6×), showcase [lit] (1.6×) | edits [lit] |  |  |
| gpu-raster: tree64 + greedy + gbuffer | 8 | 0.48 | 0 |  | room [lit] (1.5×), showcase [lit] (1.5×), meshes [lit+meshes] (1.4×) | edits [lit] |  |  |
| gpu-raster: bricks + greedy + visbuffer | 17 | 0.48 | 0 |  | room [lit] (1.8×), room [unlit] (1.8×), many-lights [lit] (1.8×) | edits [lit], edits [unlit] |  |  |
| gpu-raster: rows + greedy + gbuffer | 8 | 0.48 | 0 |  | many-lights [lit] (1.7×), showcase [lit] (1.5×), meshes [lit+meshes] (1.3×) | edits [lit] |  |  |
| gpu-raster: hier + greedy + raster | 8 | 0.48 | 1 | meshes [lit+meshes], room [lit] | room [lit] (2.1×), meshes [lit+meshes] (1.9×), showcase [lit] (1.4×) | edits [lit] |  |  |
| gpu-compute: tree64 + incremental | 17 | 0.48 | 0 | edits [lit], edits [unlit] | edits [unlit] (2.0×), edits [lit] (2.0×), streaming [unlit] (1.7×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: clipmap + incremental | 17 | 0.48 | 0 |  | random [unlit] (1.6×), edits [unlit] (1.6×), edits [lit] (1.5×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: hier + naive + raster | 8 | 0.48 | 0 |  | room [lit] (1.7×), meshes [lit+meshes] (1.6×), showcase [lit] (1.6×) | menger [lit] |  |  |
| gpu-raster: svo_dag + greedy + gbuffer | 8 | 0.47 | 0 |  | room [lit] (1.7×), showcase [lit] (1.6×), many-lights [lit] (1.5×) | edits [lit] |  |  |
| gpu-raster: bricks + greedy + raster | 17 | 0.47 | 1 | meshes [unlit+meshes], room [unlit] | room [unlit] (2.1×), meshes [unlit+meshes] (2.0×), room [lit] (1.9×) | edits [lit], edits [unlit] |  |  |
| gpu-raster: rows + greedy + raster | 8 | 0.47 | 0 | room [lit] | room [lit] (2.0×), meshes [lit+meshes] (1.8×), many-lights [lit] (1.3×) | edits [lit] |  |  |
| gpu-compute: svo_dag + beam + refloor | 17 | 0.47 | 0 |  | showcase [lit] (1.7×), many-lights [lit] (1.5×), showcase [unlit] (1.5×) | room [unlit] |  |  |
| gpu-raster: tree64 + greedy + raster | 8 | 0.47 | 0 | room [lit] | room [lit] (2.0×), meshes [lit+meshes] (1.9×), showcase [lit] (1.3×) | edits [lit] |  |  |
| gpu-raster: tree64 + naive + raster | 8 | 0.47 | 0 |  | room [lit] (1.7×), meshes [lit+meshes] (1.7×), showcase [lit] (1.5×) | menger [lit] |  |  |
| gpu-raster: svo_dag + greedy + raster | 8 | 0.46 | 0 | meshes [lit+meshes], room [lit] | room [lit] (2.2×), meshes [lit+meshes] (2.1×), showcase [lit] (1.5×) | edits [lit] |  |  |
| gpu-raster: rows + naive + raster | 8 | 0.46 | 0 |  | meshes [lit+meshes] (1.6×), room [lit] (1.5×), showcase [lit] (1.5×) | menger [lit] |  |  |
| gpu-compute: hier + beam + rowrun | 17 | 0.46 | 0 |  | showcase [unlit] (1.5×), edits [unlit] (1.4×), edits [lit] (1.4×) | room [unlit] |  |  |
| gpu-raster: svo_dag + naive + raster | 8 | 0.45 | 0 |  | room [lit] (1.8×), meshes [lit+meshes] (1.8×), showcase [lit] (1.7×) | menger [lit] |  |  |
| gpu-raster: clipmap + naive + raster | 8 | 0.45 | 0 |  | room [lit] (1.6×), meshes [lit+meshes] (1.5×), showcase [lit] (1.4×) | menger [lit] |  |  |
| gpu-raster: bricks + greedy + gbuffer | 17 | 0.45 | 0 |  | many-lights [lit] (1.9×), meshes [unlit+meshes] (1.8×), showcase [lit] (1.7×) | edits [lit], edits [unlit], menger [unlit] |  |  |
| gpu-raster: clipmap + greedy + raster | 8 | 0.45 | 0 |  | room [lit] (1.9×), meshes [lit+meshes] (1.7×), many-lights [lit] (1.3×) | edits [lit] |  |  |
| gpu-compute: clipmap + beam + refloor | 17 | 0.44 | 0 |  | edits [lit] (1.8×), edits [unlit] (1.7×), many-lights [lit] (1.7×) | menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: tree64 + beam + incremental | 17 | 0.43 | 0 | edits [lit], edits [unlit] | edits [unlit] (2.3×), edits [lit] (2.1×), streaming [unlit] (1.7×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-discover: bricks + quadtree | 15 | 0.41 | 0 |  | edits [unlit] (2.0×), edits [lit] (1.8×), streaming [unlit] (1.8×) | menger [lit] |  |  |
| gpu-compute: clipmap + beam + rowrun | 17 | 0.40 | 0 |  | edits [unlit] (1.9×), edits [lit] (1.9×), streaming [unlit] (1.5×) | menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-discover: prefix + quadtree | 15 | 0.40 | 0 |  | edits [unlit] (1.6×), terrain-flight [unlit] (1.6×), edits [lit] (1.6×) | menger [lit] |  |  |
| gpu-discover: hier + quadtree | 15 | 0.39 | 0 |  | edits [unlit] (1.6×), terrain-flight [unlit] (1.6×), edits [lit] (1.6×) | menger [lit] |  |  |
| gpu-compute: clipmap + beam + incremental | 17 | 0.39 | 0 |  | edits [unlit] (2.0×), edits [lit] (1.9×), streaming [unlit] (1.6×) | menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: prefix + cubes + visbuffer | 8 | 0.38 | 0 |  | showcase [lit] (2.0×), room [lit] (2.0×), meshes [lit+meshes] (1.8×) | far [lit], menger [lit] |  |  |
| gpu-discover: rows + quadtree | 15 | 0.38 | 0 |  | terrain-flight [unlit] (1.6×), edits [unlit] (1.5×), streaming [unlit] (1.5×) | menger [lit] |  |  |
| gpu-raster: hier + cubes + visbuffer | 8 | 0.37 | 0 |  | showcase [lit] (2.1×), room [lit] (2.0×), meshes [lit+meshes] (1.8×) | far [lit], menger [lit] |  |  |
| gpu-raster: tree64 + cubes + visbuffer | 8 | 0.37 | 0 |  | room [lit] (2.0×), showcase [lit] (1.8×), meshes [lit+meshes] (1.8×) | far [lit], menger [lit] |  |  |
| gpu-compute: hier + sphere | 17 | 0.37 | 0 |  | random [unlit] (2.0×), streaming [unlit] (1.8×), far [unlit] (1.7×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: svo_dag + cubes + visbuffer | 8 | 0.37 | 0 |  | room [lit] (2.2×), showcase [lit] (2.1×), meshes [lit+meshes] (1.9×) | far [lit], menger [lit] |  |  |
| gpu-raster: clipmap + cubes + visbuffer | 8 | 0.37 | 0 |  | showcase [lit] (1.9×), room [lit] (1.8×), meshes [lit+meshes] (1.6×) | far [lit], menger [lit] |  |  |
| gpu-discover: tree64 + quadtree | 15 | 0.36 | 0 |  | edits [unlit] (2.4×), edits [lit] (2.1×), streaming [unlit] (1.8×) | menger [lit], menger [unlit], random [unlit], showcase [unlit] |  |  |
| gpu-raster: rows + cubes + visbuffer | 8 | 0.36 | 0 |  | showcase [lit] (2.0×), room [lit] (2.0×), meshes [lit+meshes] (1.6×) | far [lit], menger [lit] |  |  |
| gpu-compute: svo_dag + rowrun | 17 | 0.36 | 0 |  | showcase [lit] (1.7×), showcase [unlit] (1.6×), edits [lit] (1.5×) | many-lights [lit], menger [unlit], random [unlit], room [unlit] |  |  |
| gpu-compute: bricks + sphere | 17 | 0.36 | 0 |  | random [unlit] (2.4×), edits [unlit] (2.2×), streaming [unlit] (2.1×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-discover: clipmap + quadtree | 15 | 0.36 | 0 |  | edits [unlit] (2.0×), edits [lit] (1.9×), streaming [unlit] (1.7×) | menger [lit], menger [unlit], room [unlit], showcase [unlit] |  |  |
| gpu-raster: prefix + cubes + gbuffer | 8 | 0.35 | 0 |  | showcase [lit] (2.2×), many-lights [lit] (1.6×), room [lit] (1.6×) | far [lit], menger [lit] |  |  |
| gpu-compute: hier + beam + sphere | 17 | 0.35 | 0 |  | edits [unlit] (1.8×), far [unlit] (1.8×), streaming [unlit] (1.8×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: bricks + cubes + visbuffer | 17 | 0.35 | 0 |  | meshes [unlit+meshes] (2.2×), showcase [lit] (2.1×), room [lit] (2.0×) | far [lit], far [unlit], menger [lit], menger [unlit], random [unlit] |  |  |
| gpu-raster: tree64 + cubes + gbuffer | 8 | 0.34 | 0 |  | showcase [lit] (1.9×), edits [lit] (1.6×), room [lit] (1.5×) | far [lit], menger [lit] |  |  |
| gpu-compute: prefix + beam + refloor + pathtrace | 8 | 0.34 | 0 |  |  |  | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: hier + cubes + gbuffer | 8 | 0.34 | 0 |  | showcase [lit] (2.1×), many-lights [lit] (1.6×), room [lit] (1.6×) | far [lit], menger [lit] |  |  |
| gpu-compute: bricks + beam + sphere | 17 | 0.34 | 0 |  | edits [unlit] (2.3×), streaming [unlit] (2.1×), far [unlit] (2.1×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: svo_dag + cubes + gbuffer | 8 | 0.34 | 0 |  | showcase [lit] (2.1×), room [lit] (1.7×), meshes [lit+meshes] (1.6×) | far [lit], menger [lit] |  |  |
| gpu-compute: rows + sphere | 17 | 0.34 | 0 |  | random [unlit] (2.4×), far [unlit] (1.8×), edits [unlit] (1.8×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: prefix + sphere | 17 | 0.34 | 0 |  | random [unlit] (2.2×), edits [unlit] (2.0×), streaming [unlit] (1.9×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: rows + cubes + gbuffer | 8 | 0.34 | 0 |  | showcase [lit] (2.1×), room [lit] (1.7×), many-lights [lit] (1.6×) | far [lit], menger [lit] |  |  |
| gpu-compute: tree64 + rowrun | 17 | 0.34 | 0 | edits [unlit] | edits [unlit] (2.7×), edits [lit] (2.6×), far [lit] (1.6×) | many-lights [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], random [unlit], room [unlit] |  |  |
| gpu-raster: clipmap + cubes + gbuffer | 8 | 0.33 | 0 |  | showcase [lit] (1.9×), many-lights [lit] (1.6×), meshes [lit+meshes] (1.4×) | far [lit], menger [lit] |  |  |
| gpu-rt: naive + rayquery | 15 | 0.32 | 0 |  | many-lights [lit] (2.7×), showcase [lit] (2.0×), room [lit] (1.7×) | edits [lit], edits [unlit], streaming [unlit] |  | far [lit], far [unlit] |
| gpu-compute: svo_dag + beam + rowrun | 17 | 0.32 | 0 |  | edits [unlit] (1.8×), showcase [unlit] (1.8×), showcase [lit] (1.8×) | many-lights [lit], menger [lit], menger [unlit], random [unlit], room [unlit] |  |  |
| gpu-compute: prefix + refloor + pathtrace | 8 | 0.32 | 0 |  | showcase [lit] (1.3×) |  | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-discover: svo_dag + quadtree | 15 | 0.32 | 0 |  | far [unlit] (1.6×), edits [unlit] (1.6×), terrain-flight [unlit] (1.6×) | menger [lit], menger [unlit], random [unlit], showcase [unlit] |  |  |
| gpu-compute: distance + sphere + pathtrace | 8 | 0.32 | 0 |  |  |  | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: bricks + cubes + gbuffer | 17 | 0.32 | 0 |  | showcase [lit] (2.2×), showcase [unlit] (1.9×), many-lights [lit] (1.8×) | far [lit], far [unlit], menger [lit], menger [unlit], random [unlit] |  |  |
| gpu-compute: prefix + beam + sphere | 17 | 0.32 | 0 |  | edits [unlit] (2.1×), edits [lit] (2.0×), streaming [unlit] (1.9×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: hier + incremental | 17 | 0.32 | 0 |  | edits [unlit] (1.9×), showcase [unlit] (1.8×), edits [lit] (1.7×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: rows + beam + sphere | 17 | 0.32 | 0 |  | random [unlit] (1.9×), edits [unlit] (1.9×), far [unlit] (1.8×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: distance + beam + sphere + pathtrace | 8 | 0.32 | 0 |  |  |  | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + incremental | 17 | 0.31 | 0 |  | edits [unlit] (2.4×), edits [lit] (1.8×), showcase [lit] (1.7×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: hier + beam + refloor + pathtrace | 8 | 0.31 | 0 |  | showcase [lit] (1.3×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + sphere | 17 | 0.31 | 0 |  | random [unlit] (2.6×), edits [unlit] (2.3×), streaming [unlit] (2.0×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], showcase [lit], showcase [unlit] |  |  |
| gpu-compute: tree64 + beam + rowrun | 17 | 0.31 | 0 | edits [unlit] | edits [unlit] (2.9×), edits [lit] (2.7×), far [lit] (1.5×) | many-lights [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], random [unlit], room [unlit] |  |  |
| gpu-compute: svo_dag + beam + refloor + pathtrace | 8 | 0.31 | 0 |  | showcase [lit] (1.4×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-rt: greedy + rayquery | 15 | 0.31 | 1 | many-lights [lit] | many-lights [lit] (3.2×), room [lit] (1.9×), showcase [lit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit] |  |  |
| gpu-compute: svo_dag + sphere | 17 | 0.30 | 0 |  | far [unlit] (1.9×), far [lit] (1.8×), edits [unlit] (1.8×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: svo_dag + refloor + pathtrace | 8 | 0.30 | 0 |  | showcase [lit] (1.4×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: hier + refloor + pathtrace | 8 | 0.30 | 0 |  | showcase [lit] (1.3×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + incremental | 17 | 0.30 | 0 |  | edits [unlit] (1.9×), showcase [unlit] (1.7×), edits [lit] (1.7×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: hier + beam + incremental | 17 | 0.30 | 0 |  | edits [unlit] (2.0×), edits [lit] (1.8×), showcase [unlit] (1.8×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: tree64 + sphere | 17 | 0.30 | 0 |  | edits [unlit] (2.9×), edits [lit] (2.3×), streaming [unlit] (2.1×) | meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], showcase [lit], showcase [unlit] |  |  |
| gpu-compute: bricks + beam + incremental | 17 | 0.29 | 0 |  | edits [unlit] (2.5×), edits [lit] (1.9×), showcase [unlit] (1.7×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: prefix + incremental | 17 | 0.29 | 0 |  | edits [unlit] (2.2×), edits [lit] (1.9×), showcase [lit] (1.6×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: bricks + beam + refloor + pathtrace | 8 | 0.29 | 0 |  |  |  | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + beam + refloor + pathtrace | 8 | 0.29 | 0 |  |  |  | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + sphere | 17 | 0.28 | 0 |  | menger [lit] (2.4×), many-lights [lit] (2.1×), room [lit] (2.1×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-compute: prefix + beam + incremental | 17 | 0.28 | 0 |  | edits [unlit] (2.2×), edits [lit] (2.0×), showcase [lit] (1.6×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-raster: bricks + cubes + raster | 17 | 0.28 | 0 |  | meshes [unlit+meshes] (2.7×), showcase [unlit] (2.4×), room [unlit] (2.2×) | far [lit], far [unlit], menger [lit], menger [unlit], random [unlit], terrain-flight [lit] |  |  |
| gpu-raster: distance + naive + gbuffer | 8 | 0.28 | 0 |  | meshes [lit+meshes] (2.1×), room [lit] (2.1×), many-lights [lit] (2.1×) | edits [lit], far [lit], terrain-flight [lit] |  |  |
| gpu-compute: distance + beam + refloor + pathtrace | 8 | 0.28 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + refloor + pathtrace | 8 | 0.28 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + beam + incremental | 17 | 0.28 | 0 |  | edits [unlit] (2.0×), showcase [unlit] (1.7×), edits [lit] (1.7×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], terrain-flight [lit] |  |  |
| gpu-compute: svo_dag + incremental | 17 | 0.28 | 0 |  | edits [unlit] (2.0×), far [unlit] (1.9×), edits [lit] (1.9×) | many-lights [lit], menger [lit], meshes [lit+meshes], meshes [unlit+meshes], random [unlit], room [lit], room [unlit] |  |  |
| gpu-raster: distance + greedy + visbuffer | 8 | 0.28 | 0 |  | meshes [lit+meshes] (2.6×), many-lights [lit] (2.0×), room [lit] (2.0×) | edits [lit], far [lit], terrain-flight [lit] |  |  |
| gpu-compute: rows + refloor + pathtrace | 8 | 0.28 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: tree64 + beam + sphere | 17 | 0.27 | 0 |  | edits [unlit] (3.1×), edits [lit] (2.4×), random [unlit] (2.1×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], showcase [lit], showcase [unlit] |  |  |
| gpu-raster: distance + naive + visbuffer | 8 | 0.27 | 0 |  | meshes [lit+meshes] (2.6×), many-lights [lit] (2.2×), showcase [lit] (1.8×) | edits [lit], far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: distance + refloor + pathtrace | 8 | 0.27 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: distance + greedy + gbuffer | 8 | 0.27 | 0 |  | meshes [lit+meshes] (2.4×), room [lit] (2.3×), many-lights [lit] (2.0×) | edits [lit], far [lit], terrain-flight [lit] |  |  |
| gpu-compute: distance + beam + sphere | 17 | 0.27 | 0 |  | many-lights [lit] (2.2×), room [lit] (2.2×), meshes [unlit+meshes] (2.1×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-compute: prefix + rowrun + pathtrace | 8 | 0.27 | 0 |  | showcase [lit] (1.4×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: distance + greedy + raster | 8 | 0.27 | 0 |  | meshes [lit+meshes] (3.3×), room [lit] (2.5×), many-lights [lit] (1.8×) | edits [lit], far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: svo_dag + beam + sphere | 17 | 0.26 | 0 |  | edits [unlit] (2.0×), far [unlit] (2.0×), far [lit] (2.0×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit] |  |  |
| gpu-compute: clipmap + beam + sphere | 17 | 0.26 | 0 |  | edits [unlit] (2.7×), edits [lit] (2.3×), streaming [unlit] (2.0×) | menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], showcase [lit], showcase [unlit] |  |  |
| gpu-compute: tree64 + beam + refloor + pathtrace | 8 | 0.26 | 0 |  | showcase [lit] (1.5×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + refloor | 17 | 0.25 | 0 |  | menger [lit] (2.4×), many-lights [lit] (2.3×), random [unlit] (2.2×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-raster: prefix + cubes + raster | 8 | 0.25 | 0 |  | meshes [lit+meshes] (2.6×), room [lit] (2.5×), showcase [lit] (2.3×) | far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-raster: distance + naive + raster | 8 | 0.25 | 0 |  | meshes [lit+meshes] (2.8×), room [lit] (2.3×), showcase [lit] (1.7×) | edits [lit], far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: tree64 + refloor + pathtrace | 8 | 0.25 | 0 |  | showcase [lit] (1.5×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: hier + cubes + raster | 8 | 0.25 | 0 |  | room [lit] (2.5×), meshes [lit+meshes] (2.5×), showcase [lit] (2.3×) | far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-raster: rows + cubes + raster | 8 | 0.25 | 0 |  | room [lit] (2.4×), meshes [lit+meshes] (2.4×), showcase [lit] (2.2×) | far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: prefix + beam + rowrun + pathtrace | 8 | 0.25 | 0 |  | showcase [lit] (1.5×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + refloor + pathtrace | 8 | 0.25 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + beam + incremental | 17 | 0.24 | 0 |  | edits [unlit] (2.2×), edits [lit] (2.1×), far [unlit] (2.0×) | many-lights [lit], menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], random [unlit], room [lit], room [unlit] |  |  |
| gpu-compute: distance + rowrun | 17 | 0.24 | 0 |  | menger [lit] (2.3×), showcase [lit] (2.0×), random [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-raster: tree64 + cubes + raster | 8 | 0.24 | 0 |  | meshes [lit+meshes] (2.6×), room [lit] (2.6×), showcase [lit] (2.1×) | far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: clipmap + beam + refloor + pathtrace | 8 | 0.24 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: svo_dag + cubes + raster | 8 | 0.24 | 0 |  | room [lit] (2.7×), meshes [lit+meshes] (2.5×), showcase [lit] (2.3×) | far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: distance + beam + refloor | 17 | 0.24 | 0 |  | many-lights [lit] (2.4×), showcase [lit] (2.1×), menger [lit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-raster: clipmap + cubes + raster | 8 | 0.23 | 0 |  | room [lit] (2.3×), meshes [lit+meshes] (2.2×), showcase [lit] (2.2×) | far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: distance + beam + rowrun | 17 | 0.23 | 0 |  | showcase [lit] (2.1×), many-lights [lit] (2.0×), menger [lit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-compute: rows + beam + rowrun + pathtrace | 8 | 0.22 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + rowrun + pathtrace | 8 | 0.22 | 0 |  |  | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: distance + cubes + visbuffer | 8 | 0.22 | 0 |  | room [lit] (3.1×), meshes [lit+meshes] (2.8×), showcase [lit] (2.1×) | edits [lit], far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: distance + beam + rowrun + pathtrace | 8 | 0.21 | 0 |  | showcase [lit] (1.3×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + rowrun + pathtrace | 8 | 0.21 | 0 |  | showcase [lit] (1.3×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-rt: boxes + rayquery + pathtrace | 7 | 0.20 | 0 |  | showcase [lit] (1.6×) | many-lights [lit] | edits [lit], far [lit], menger [lit], room [lit], terrain-flight [lit] |  |
| gpu-raster: distance + cubes + gbuffer | 8 | 0.20 | 0 |  | meshes [lit+meshes] (2.4×), room [lit] (2.2×), showcase [lit] (2.2×) | edits [lit], far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-discover: distance + quadtree | 15 | 0.17 | 0 |  | many-lights [lit] (2.0×), room [lit] (1.8×), room [unlit] (1.7×) | edits [lit], edits [unlit], far [lit], far [unlit], menger [lit], menger [unlit], random [unlit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-compute: clipmap + incremental + pathtrace | 8 | 0.17 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-raster: distance + cubes + raster | 8 | 0.17 | 0 |  | meshes [lit+meshes] (3.6×), room [lit] (3.2×), showcase [lit] (2.2×) | edits [lit], far [lit], menger [lit], terrain-flight [lit] |  |  |
| gpu-compute: tree64 + incremental + pathtrace | 8 | 0.17 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + beam + incremental + pathtrace | 8 | 0.17 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + rowrun + pathtrace | 8 | 0.16 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + beam + rowrun + pathtrace | 8 | 0.16 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + rowrun + pathtrace | 8 | 0.16 | 0 |  | showcase [lit] (1.5×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + beam + rowrun + pathtrace | 8 | 0.15 | 0 |  | showcase [lit] (1.5×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: tree64 + beam + incremental + pathtrace | 8 | 0.15 | 0 |  | showcase [lit] (1.4×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: hier + sphere + pathtrace | 8 | 0.15 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: hier + beam + sphere + pathtrace | 8 | 0.15 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + incremental | 17 | 0.14 | 0 |  | showcase [lit] (2.3×), random [unlit] (2.1×), showcase [unlit] (1.7×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-compute: bricks + sphere + pathtrace | 8 | 0.14 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + beam + sphere + pathtrace | 8 | 0.14 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: hier + rowrun + pathtrace | 8 | 0.14 | 0 |  | showcase [lit] (1.7×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + beam + sphere + pathtrace | 8 | 0.14 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + sphere + pathtrace | 8 | 0.14 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: prefix + beam + sphere + pathtrace | 8 | 0.14 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: prefix + sphere + pathtrace | 8 | 0.14 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + beam + incremental | 17 | 0.14 | 0 |  | showcase [lit] (2.3×), random [unlit] (1.9×), showcase [unlit] (1.7×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], meshes [lit+meshes], meshes [unlit+meshes], room [lit], room [unlit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu-compute: hier + beam + rowrun + pathtrace | 8 | 0.13 | 0 |  | showcase [lit] (1.7×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + beam + sphere + pathtrace | 8 | 0.13 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + sphere + pathtrace | 8 | 0.13 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + sphere + pathtrace | 8 | 0.13 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: clipmap + beam + sphere + pathtrace | 8 | 0.12 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: hier + beam + incremental + pathtrace | 8 | 0.12 | 0 |  | showcase [lit] (1.4×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + beam + incremental + pathtrace | 8 | 0.12 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + beam + incremental + pathtrace | 8 | 0.11 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: hier + incremental + pathtrace | 8 | 0.11 | 0 |  | showcase [lit] (1.4×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: prefix + beam + incremental + pathtrace | 8 | 0.11 | 0 |  | showcase [lit] (1.4×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + rowrun + pathtrace | 8 | 0.11 | 0 |  | showcase [lit] (2.3×) | many-lights [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: bricks + incremental + pathtrace | 8 | 0.11 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: rows + incremental + pathtrace | 8 | 0.11 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + beam + rowrun + pathtrace | 8 | 0.11 | 0 |  | showcase [lit] (2.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: prefix + incremental + pathtrace | 8 | 0.11 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: tree64 + beam + sphere + pathtrace | 8 | 0.11 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: tree64 + sphere + pathtrace | 8 | 0.11 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + beam + incremental + pathtrace | 8 | 0.10 | 0 |  | showcase [lit] (1.3×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: distance + incremental + pathtrace | 8 | 0.09 | 0 |  |  | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + incremental + pathtrace | 8 | 0.09 | 0 |  | showcase [lit] (1.6×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: svo_dag + beam + incremental + pathtrace | 8 | 0.09 | 0 |  | showcase [lit] (1.6×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: tree64 + beam + rowrun + pathtrace | 8 | 0.07 | 0 |  | showcase [lit] (2.2×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| gpu-compute: tree64 + rowrun + pathtrace | 8 | 0.07 | 0 |  | showcase [lit] (2.2×) | many-lights [lit], showcase [lit] | edits [lit], far [lit], menger [lit], meshes [lit+meshes], room [lit], terrain-flight [lit] |  |
| cpu: hier + refloor | 15 | 0.05 | 0 |  | edits [unlit] (2.6×), far [unlit] (1.9×), streaming [unlit] (1.8×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu-discover: hier + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.7×), showcase [lit] (1.3×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: prefix + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.5×), edits [unlit] (1.7×), showcase [lit] (1.3×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: morton + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.6×), showcase [lit] (1.3×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: bits + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.6×), showcase [lit] (1.4×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: u8 + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.6×), showcase [lit] (1.3×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: brickmajor + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.6×), showcase [lit] (1.4×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: bricks + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.6×), showcase [lit] (1.3×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu-discover: palette + quadtree | 15 | 0.05 | 0 |  | many-lights [lit] (2.4×), edits [unlit] (1.6×), showcase [lit] (1.4×) | edits [unlit], many-lights [lit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [unlit] | edits [lit], far [lit], far [unlit], menger [lit], menger [unlit], terrain-flight [lit] |  |
| cpu: prefix + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.6×), far [unlit] (1.9×), edits [lit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bricks + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.3×), far [unlit] (2.0×), streaming [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: hier + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.6×), far [unlit] (2.2×), streaming [unlit] (2.0×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: u8 + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.3×), far [unlit] (2.0×), streaming [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bits + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.3×), far [unlit] (2.0×), streaming [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: morton + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.2×), far [unlit] (2.0×), streaming [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: brickmajor + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.2×), far [unlit] (2.0×), streaming [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: palette + refloor | 15 | 0.04 | 0 |  | edits [unlit] (2.0×), far [unlit] (2.0×), streaming [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: prefix + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.7×), far [unlit] (2.3×), streaming [unlit] (2.1×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bricks + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.4×), far [unlit] (2.2×), streaming [unlit] (2.0×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: u8 + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.4×), far [unlit] (2.2×), streaming [unlit] (2.1×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bits + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.4×), far [unlit] (2.2×), streaming [unlit] (2.1×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: morton + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.3×), far [unlit] (2.1×), streaming [unlit] (2.0×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: brickmajor + incremental | 15 | 0.04 | 0 |  | edits [unlit] (2.3×), far [unlit] (2.2×), streaming [unlit] (2.1×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: palette + incremental | 15 | 0.04 | 0 |  | far [unlit] (2.2×), edits [unlit] (2.1×), streaming [unlit] (2.0×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu-raster: greedy + raster | 15 | 0.03 | 0 |  | streaming [unlit] (2.5×), edits [unlit] (2.0×), terrain-flight [unlit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu-raster: naive + raster | 15 | 0.02 | 0 |  | showcase [unlit] (2.3×), room [unlit] (1.9×), many-lights [lit] (1.9×) | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu-discover: prefix + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (4.0×), edits [unlit] (3.1×), room [lit] (2.2×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: hier + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (4.0×), edits [unlit] (3.0×), room [lit] (2.0×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: morton + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (4.0×), edits [unlit] (2.8×), room [lit] (2.1×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: u8 + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (3.9×), edits [unlit] (2.8×), room [lit] (2.1×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: brickmajor + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (4.0×), edits [unlit] (2.8×), room [lit] (2.2×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: bits + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (3.9×), edits [unlit] (2.9×), room [lit] (2.2×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: bricks + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (4.0×), edits [unlit] (2.8×), room [lit] (2.1×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-discover: palette + cluster | 15 | 0.02 | 0 |  | many-lights [lit] (4.0×), edits [unlit] (2.8×), room [lit] (2.2×) | edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] | edits [lit] |  |
| cpu-voxelspace: heightmap + columns | 15 | - | 0 |  |  |  | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |
| gpu-splat: bricks + points + splat | 15 | - | 0 |  |  |  | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |
| gpu-splat: clipmap + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-splat: distance + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-splat: hier + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-splat: prefix + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-splat: rows + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-splat: svo_dag + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-splat: tree64 + points + splat | 7 | - | 0 |  |  |  | edits [lit], far [lit], many-lights [lit], menger [lit], room [lit], showcase [lit], terrain-flight [lit] |  |
| gpu-voxelspace: heightmap + columns | 15 | - | 0 |  |  |  | edits [lit], edits [unlit], far [lit], far [unlit], many-lights [lit], menger [lit], menger [unlit], random [unlit], room [lit], room [unlit], showcase [lit], showcase [unlit], streaming [unlit], terrain-flight [lit], terrain-flight [unlit] |  |

## Components

The effect of a stage's component, other stages and knobs equal: the geometric mean of fps relative to the best of each such group, wins, and the number of comparisons.

**cpu · storage**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| hier | 1.00 | 25 | 30 |
| prefix | 0.90 | 2 | 30 |
| bricks | 0.88 | 3 | 30 |
| u8 | 0.86 | 0 | 30 |
| bits | 0.86 | 0 | 30 |
| morton | 0.85 | 0 | 30 |
| brickmajor | 0.84 | 0 | 30 |
| palette | 0.82 | 0 | 30 |

**cpu · visibility**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 1.00 | 114 | 120 |
| incremental | 0.89 | 6 | 120 |

**cpu-discover · storage**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| prefix | 0.99 | 6 | 23 |
| hier | 0.99 | 12 | 23 |
| morton | 0.95 | 0 | 23 |
| u8 | 0.95 | 2 | 23 |
| bits | 0.95 | 0 | 23 |
| brickmajor | 0.95 | 2 | 23 |
| bricks | 0.94 | 1 | 23 |
| palette | 0.93 | 0 | 23 |

**cpu-discover · visibility**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| quadtree | 0.98 | 64 | 72 |
| cluster | 0.49 | 8 | 72 |

**cpu-raster · geometry**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| greedy | 1.00 | 13 | 15 |
| naive | 0.71 | 2 | 15 |

**gpu-compute · storage**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| prefix | 0.72 | 16 | 152 |
| bricks | 0.71 | 31 | 152 |
| hier | 0.70 | 15 | 152 |
| rows | 0.69 | 9 | 152 |
| clipmap | 0.67 | 15 | 152 |
| tree64 | 0.63 | 27 | 152 |
| svo_dag | 0.56 | 2 | 152 |
| distance | 0.39 | 37 | 152 |

**gpu-compute · prepass**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| none | 1.00 | 510 | 608 |
| beam | 0.91 | 98 | 608 |

**gpu-compute · visibility**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.98 | 256 | 304 |
| rowrun | 0.76 | 16 | 304 |
| sphere | 0.58 | 29 | 304 |
| incremental | 0.54 | 3 | 304 |

**gpu-compute · shading**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| direct | 1.00 | 128 | 128 |
| pathtrace | 0.35 | 0 | 128 |

**gpu-discover · storage**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| bricks | 0.97 | 5 | 15 |
| prefix | 0.94 | 4 | 15 |
| hier | 0.91 | 2 | 15 |
| rows | 0.89 | 1 | 15 |
| tree64 | 0.86 | 2 | 15 |
| clipmap | 0.84 | 0 | 15 |
| svo_dag | 0.76 | 0 | 15 |
| distance | 0.40 | 1 | 15 |

**gpu-raster · storage**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| prefix | 0.95 | 31 | 72 |
| bricks | 0.94 | 16 | 72 |
| hier | 0.93 | 6 | 72 |
| tree64 | 0.92 | 9 | 72 |
| svo_dag | 0.90 | 9 | 72 |
| clipmap | 0.89 | 0 | 72 |
| rows | 0.89 | 1 | 72 |
| distance | 0.51 | 0 | 72 |

**gpu-raster · geometry**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| naive | 0.91 | 116 | 219 |
| greedy | 0.84 | 79 | 219 |
| cubes | 0.55 | 24 | 219 |

**gpu-raster · visibility**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| visbuffer | 0.98 | 133 | 219 |
| gbuffer | 0.91 | 32 | 219 |
| raster | 0.80 | 54 | 219 |

**gpu-rt · geometry**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| boxes | 0.79 | 11 | 17 |
| naive | 0.60 | 3 | 15 |
| greedy | 0.57 | 3 | 17 |

**gpu-rt · shading**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| direct | 1.00 | 6 | 6 |
| pathtrace | 0.56 | 0 | 6 |

## Scenarios

### edits [lit]

1280×720. camera 1.10 blocks/frame (max 1.1), turn 0.76°/frame; 974 chunks at start, 2.6 loads/frame; chunk fill 20%, solid 15.2%, surface 1.18%; sky 37%, hits at 148 blocks (median 136); 2090.8 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: tree64 + refloor | 534.7 | 1.00 | 4.08 | 0.90 | 1.61 | 56.4 | 0.01 | 28 | 322 |
| 2 | gpu-compute: tree64 + incremental | 518.2 | 0.97 | 4.12 | 1.33 | 1.64 | 56.4 | 0.01 | 28 | 323 |
| 3 | gpu-compute: tree64 + beam + refloor | 510.7 | 0.96 | 3.83 | 0.93 | 1.68 | 56.4 | 0.01 | 28 | 322 |
| 4 | gpu-compute: tree64 + beam + incremental | 489.2 | 0.91 | 4.39 | 1.34 | 1.70 | 56.4 | 0.01 | 28 | 322 |
| 5 | gpu-compute: bricks + refloor | 483.6 | 0.90 | 4.03 | 0.91 | 0.06 | 56.4 | 0.01 | 71 | 293 |
| 6 | gpu-compute: bricks + beam + refloor | 482.7 | 0.90 | 4.00 | 0.90 | 0.06 | 56.4 | 0.01 | 71 | 293 |
| 7 | gpu-compute: bricks + rowrun | 462.8 | 0.87 | 5.35 | 1.17 | 0.07 | 56.4 | 0.01 | 71 | 293 |
| 8 | gpu-compute: tree64 + rowrun | 459.1 | 0.86 | 3.89 | 1.97 | 1.64 | 56.4 | 0.01 | 28 | 323 |
| 9 | gpu-compute: bricks + beam + rowrun | 458.8 | 0.86 | 4.26 | 1.15 | 0.07 | 56.4 | 0.01 | 71 | 293 |
| 10 | gpu-compute: tree64 + beam + rowrun | 447.2 | 0.84 | 5.22 | 1.99 | 1.62 | 56.4 | 0.01 | 28 | 322 |
| 11 | gpu-compute: clipmap + beam + refloor | 410.4 | 0.77 | 4.61 | 1.08 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 12 | gpu-compute: clipmap + rowrun | 405.0 | 0.76 | 4.64 | 1.24 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 13 | gpu-compute: clipmap + beam + rowrun | 404.4 | 0.76 | 4.73 | 1.25 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 14 | gpu-discover: tree64 + quadtree | 401.6 | 0.75 | 4.62 | 2.23 | 1.78 | 49.8 | 0.02 | 155 | 336 |
| 15 | gpu-compute: clipmap + beam + incremental | 399.5 | 0.75 | 4.73 | 1.36 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 16 | gpu-compute: clipmap + incremental | 398.5 | 0.75 | 4.66 | 1.35 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 17 | gpu-compute: clipmap + refloor | 396.0 | 0.74 | 4.71 | 1.06 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 18 | gpu-discover: bricks + quadtree | 391.5 | 0.73 | 4.71 | 2.10 | 0.10 | 49.8 | 0.02 | 199 | 305 |
| 19 | gpu-compute: prefix + beam + rowrun | 365.8 | 0.68 | 5.33 | 0.82 | 0.06 | 56.4 | 0.01 | 76 | 291 |
| 20 | gpu-compute: prefix + beam + refloor | 364.6 | 0.68 | 5.38 | 0.74 | 0.06 | 56.4 | 0.01 | 76 | 291 |
| 21 | gpu-compute: prefix + rowrun | 363.6 | 0.68 | 5.45 | 0.82 | 0.06 | 56.4 | 0.01 | 76 | 291 |
| 22 | gpu-compute: prefix + refloor | 362.1 | 0.68 | 5.46 | 0.73 | 0.06 | 56.4 | 0.01 | 76 | 291 |
| 23 | gpu-compute: bricks + sphere | 360.1 | 0.67 | 5.03 | 2.60 | 0.07 | 56.4 | 0.01 | 71 | 293 |
| 24 | gpu-compute: tree64 + sphere | 357.6 | 0.67 | 4.31 | 2.71 | 1.74 | 56.4 | 0.01 | 28 | 322 |
| 25 | gpu-compute: bricks + beam + sphere | 355.7 | 0.67 | 4.91 | 2.62 | 0.08 | 56.4 | 0.01 | 71 | 293 |
| 26 | gpu-discover: clipmap + quadtree | 352.3 | 0.66 | 5.07 | 2.14 | 0.03 | 49.8 | 0.02 | 282 | 309 |
| 27 | gpu-compute: hier + beam + refloor | 348.6 | 0.65 | 5.75 | 0.76 | 0.06 | 56.4 | 0.01 | 71 | 293 |
| 28 | gpu-compute: hier + refloor | 347.2 | 0.65 | 5.75 | 0.76 | 0.06 | 56.4 | 0.01 | 71 | 293 |
| 29 | gpu-compute: tree64 + beam + sphere | 346.8 | 0.65 | 4.65 | 2.80 | 1.78 | 56.4 | 0.01 | 28 | 322 |
| 30 | gpu-compute: hier + beam + rowrun | 343.8 | 0.64 | 5.91 | 1.22 | 0.06 | 56.4 | 0.01 | 71 | 294 |
| 31 | gpu-compute: hier + rowrun | 341.8 | 0.64 | 6.09 | 1.20 | 0.06 | 56.4 | 0.01 | 71 | 294 |
| 32 | gpu-compute: prefix + beam + sphere | 336.6 | 0.63 | 5.80 | 2.40 | 0.06 | 56.4 | 0.01 | 76 | 291 |
| 33 | gpu-compute: hier + sphere | 332.6 | 0.62 | 5.68 | 2.21 | 0.07 | 56.4 | 0.01 | 71 | 293 |
| 34 | gpu-compute: prefix + sphere | 331.8 | 0.62 | 5.90 | 2.40 | 0.07 | 56.4 | 0.01 | 76 | 291 |
| 35 | gpu-discover: prefix + quadtree | 330.9 | 0.62 | 5.87 | 1.88 | 0.08 | 49.8 | 0.02 | 204 | 302 |
| 36 | gpu-compute: hier + beam + sphere | 327.5 | 0.61 | 5.79 | 2.24 | 0.07 | 56.4 | 0.01 | 71 | 294 |
| 37 | gpu-compute: rows + refloor | 327.2 | 0.61 | 6.10 | 0.92 | 0.06 | 56.4 | 0.01 | 75 | 291 |
| 38 | gpu-compute: rows + rowrun | 326.6 | 0.61 | 6.13 | 1.01 | 0.05 | 56.4 | 0.01 | 75 | 291 |
| 39 | gpu-discover: hier + quadtree | 326.3 | 0.61 | 5.89 | 1.96 | 0.07 | 49.8 | 0.02 | 199 | 306 |
| 40 | gpu-compute: clipmap + sphere | 322.1 | 0.60 | 6.01 | 2.82 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 41 | gpu-compute: rows + beam + rowrun | 322.0 | 0.60 | 6.18 | 1.01 | 0.06 | 56.4 | 0.01 | 75 | 291 |
| 42 | gpu-compute: rows + beam + refloor | 320.8 | 0.60 | 6.20 | 0.91 | 0.07 | 56.4 | 0.01 | 75 | 291 |
| 43 | gpu-compute: clipmap + beam + sphere | 315.0 | 0.59 | 6.09 | 2.90 | 0.01 | 56.4 | 0.01 | 154 | 298 |
| 44 | gpu-compute: bricks + incremental | 308.2 | 0.58 | 9.17 | 3.09 | 0.07 | 56.4 | 0.01 | 71 | 293 |
| 45 | gpu-compute: svo_dag + refloor | 307.2 | 0.57 | 30.72 | 1.01 | 2.93 | 56.4 | 0.01 | 12 | 341 |
| 46 | gpu-compute: bricks + beam + incremental | 306.5 | 0.57 | 9.20 | 3.11 | 0.08 | 56.4 | 0.01 | 71 | 293 |
| 47 | gpu-compute: rows + beam + sphere | 305.6 | 0.57 | 6.85 | 2.60 | 0.07 | 56.4 | 0.01 | 75 | 291 |
| 48 | gpu-compute: prefix + beam + incremental | 304.8 | 0.57 | 8.09 | 2.72 | 0.07 | 56.4 | 0.01 | 76 | 291 |
| 49 | gpu-compute: rows + sphere | 303.2 | 0.57 | 6.29 | 2.57 | 0.07 | 56.4 | 0.01 | 75 | 291 |
| 50 | gpu-compute: prefix + incremental | 300.7 | 0.56 | 8.48 | 2.68 | 0.08 | 56.4 | 0.01 | 76 | 291 |
| 51 | gpu-raster: tree64 + cubes + gbuffer | 300.7 | 0.56 | 6.62 | 3.01 | 2.77 | 49.5 | 0.02 | 64 | 327 |
| 52 | gpu-raster: tree64 + cubes + visbuffer | 300.4 | 0.56 | 6.46 | 2.95 | 2.83 | 49.6 | 0.02 | 64 | 328 |
| 53 | gpu-compute: svo_dag + beam + refloor | 300.0 | 0.56 | 29.42 | 1.05 | 2.99 | 56.4 | 0.01 | 12 | 340 |
| 54 | gpu-discover: rows + quadtree | 298.7 | 0.56 | 6.45 | 2.10 | 0.09 | 49.8 | 0.02 | 203 | 304 |
| 55 | gpu-compute: svo_dag + rowrun | 298.5 | 0.56 | 31.70 | 1.68 | 3.00 | 56.4 | 0.01 | 12 | 337 |
| 56 | gpu-compute: hier + incremental | 294.7 | 0.55 | 9.03 | 2.78 | 0.07 | 56.4 | 0.01 | 71 | 294 |
| 57 | gpu-compute: svo_dag + beam + rowrun | 292.8 | 0.55 | 31.01 | 1.65 | 3.03 | 56.4 | 0.01 | 12 | 340 |
| 58 | gpu-compute: hier + beam + incremental | 288.1 | 0.54 | 9.96 | 2.91 | 0.07 | 56.4 | 0.01 | 71 | 294 |
| 59 | gpu-rt: boxes + rayquery | 285.5 | 0.53 | 5.56 | 1.08 | 1.03 | 56.3 | 0.01 | 77 | 470 |
| 60 | gpu-compute: svo_dag + incremental | 280.2 | 0.52 | 29.73 | 2.69 | 2.99 | 56.4 | 0.01 | 12 | 340 |
| 61 | gpu-raster: bricks + cubes + gbuffer | 276.5 | 0.52 | 8.23 | 2.99 | 1.28 | 49.5 | 0.02 | 108 | 318 |
| 62 | gpu-compute: svo_dag + beam + incremental | 274.8 | 0.51 | 30.10 | 2.75 | 3.01 | 56.4 | 0.01 | 12 | 340 |
| 63 | gpu-compute: svo_dag + sphere | 274.1 | 0.51 | 29.59 | 2.93 | 3.02 | 56.4 | 0.01 | 12 | 338 |
| 64 | gpu-raster: bricks + cubes + visbuffer | 271.9 | 0.51 | 7.59 | 2.96 | 1.31 | 49.6 | 0.02 | 108 | 318 |
| 65 | gpu-compute: svo_dag + beam + sphere | 269.5 | 0.50 | 30.01 | 2.97 | 3.02 | 56.4 | 0.01 | 12 | 340 |
| 66 | gpu-compute: rows + incremental | 266.9 | 0.50 | 9.31 | 3.20 | 0.08 | 56.4 | 0.01 | 75 | 291 |
| 67 | gpu-compute: rows + beam + incremental | 257.0 | 0.48 | 10.15 | 3.43 | 0.07 | 56.4 | 0.01 | 75 | 291 |
| 68 | gpu-raster: clipmap + cubes + gbuffer | 247.5 | 0.46 | 7.77 | 3.04 | 1.24 | 49.5 | 0.02 | 191 | 331 |
| 69 | gpu-raster: clipmap + cubes + visbuffer | 245.1 | 0.46 | 7.64 | 3.00 | 1.26 | 49.6 | 0.02 | 191 | 331 |
| 70 | gpu-raster: prefix + cubes + visbuffer | 232.8 | 0.44 | 7.57 | 2.92 | 1.30 | 49.6 | 0.02 | 113 | 317 |
| 71 | gpu-raster: prefix + cubes + gbuffer | 231.4 | 0.43 | 7.70 | 2.97 | 1.31 | 49.5 | 0.02 | 113 | 317 |
| 72 | gpu-discover: svo_dag + quadtree | 228.0 | 0.43 | 30.01 | 3.69 | 3.12 | 49.8 | 0.02 | 139 | 354 |
| 73 | gpu-raster: hier + cubes + visbuffer | 223.0 | 0.42 | 7.66 | 2.94 | 1.33 | 49.6 | 0.02 | 108 | 317 |
| 74 | gpu-raster: hier + cubes + gbuffer | 221.5 | 0.41 | 8.19 | 2.99 | 1.34 | 49.5 | 0.02 | 108 | 317 |
| 75 | gpu-raster: svo_dag + cubes + visbuffer | 217.3 | 0.41 | 31.69 | 3.05 | 4.18 | 49.6 | 0.02 | 49 | 348 |
| 76 | gpu-raster: svo_dag + cubes + gbuffer | 216.7 | 0.41 | 31.75 | 3.11 | 4.19 | 49.5 | 0.02 | 49 | 345 |
| 77 | gpu-raster: rows + cubes + visbuffer | 212.0 | 0.40 | 8.03 | 2.96 | 1.36 | 49.6 | 0.02 | 112 | 317 |
| 78 | gpu-raster: tree64 + naive + raster | 211.8 | 0.40 | 9.88 | 1.30 | 4.29 | 50.3 | 0.02 | 96 | 333 |
| 79 | gpu-raster: rows + cubes + gbuffer | 211.6 | 0.40 | 8.14 | 3.00 | 1.36 | 49.5 | 0.02 | 112 | 317 |
| 80 | gpu-raster: tree64 + naive + gbuffer | 209.6 | 0.39 | 9.71 | 0.77 | 4.34 | 50.2 | 0.02 | 96 | 331 |
| 81 | gpu-raster: tree64 + naive + visbuffer | 207.3 | 0.39 | 9.90 | 0.74 | 4.39 | 50.3 | 0.02 | 96 | 333 |
| 82 | gpu-raster: bricks + naive + raster | 195.1 | 0.36 | 8.84 | 1.28 | 2.81 | 50.3 | 0.02 | 140 | 331 |
| 83 | gpu-raster: bricks + naive + gbuffer | 194.8 | 0.36 | 8.61 | 0.78 | 2.79 | 50.2 | 0.02 | 140 | 331 |
| 84 | gpu-raster: bricks + naive + visbuffer | 193.3 | 0.36 | 8.88 | 0.75 | 2.80 | 50.3 | 0.02 | 140 | 331 |
| 85 | gpu-raster: clipmap + naive + raster | 178.4 | 0.33 | 9.30 | 1.30 | 2.75 | 50.3 | 0.02 | 223 | 345 |
| 86 | gpu-raster: clipmap + naive + visbuffer | 178.2 | 0.33 | 9.25 | 0.78 | 2.77 | 50.3 | 0.02 | 223 | 344 |
| 87 | gpu-raster: clipmap + naive + gbuffer | 177.6 | 0.33 | 9.21 | 0.82 | 2.77 | 50.2 | 0.02 | 223 | 344 |
| 88 | gpu-raster: prefix + cubes + raster | 176.4 | 0.33 | 8.08 | 5.52 | 1.43 | 49.6 | 0.02 | 113 | 318 |
| 89 | gpu-raster: tree64 + cubes + raster | 176.4 | 0.33 | 6.73 | 5.55 | 2.98 | 49.6 | 0.02 | 64 | 326 |
| 90 | gpu-raster: hier + cubes + raster | 175.3 | 0.33 | 8.57 | 5.55 | 1.41 | 49.6 | 0.02 | 108 | 317 |
| 91 | gpu-raster: bricks + cubes + raster | 175.2 | 0.33 | 6.67 | 5.57 | 1.44 | 49.6 | 0.02 | 108 | 318 |
| 92 | gpu-raster: rows + cubes + raster | 174.5 | 0.33 | 8.72 | 5.57 | 1.39 | 49.6 | 0.02 | 112 | 318 |
| 93 | gpu-raster: clipmap + cubes + raster | 172.6 | 0.32 | 7.44 | 5.58 | 1.36 | 49.6 | 0.02 | 191 | 330 |
| 94 | gpu-raster: prefix + naive + visbuffer | 169.3 | 0.32 | 9.71 | 0.70 | 2.83 | 50.3 | 0.02 | 145 | 331 |
| 95 | gpu-raster: prefix + naive + raster | 169.1 | 0.32 | 9.66 | 1.21 | 2.85 | 50.3 | 0.02 | 145 | 331 |
| 96 | gpu-raster: prefix + naive + gbuffer | 168.9 | 0.32 | 9.44 | 0.75 | 2.83 | 50.2 | 0.02 | 145 | 330 |
| 97 | gpu-raster: hier + naive + visbuffer | 165.9 | 0.31 | 9.68 | 0.72 | 2.83 | 50.3 | 0.02 | 140 | 331 |
| 98 | gpu-raster: hier + naive + gbuffer | 165.3 | 0.31 | 9.85 | 0.77 | 2.84 | 50.2 | 0.02 | 140 | 331 |
| 99 | gpu-raster: hier + naive + raster | 164.1 | 0.31 | 9.81 | 1.25 | 2.86 | 50.3 | 0.02 | 140 | 332 |
| 100 | gpu-raster: rows + naive + raster | 158.9 | 0.30 | 10.04 | 1.29 | 2.85 | 50.3 | 0.02 | 144 | 330 |
| 101 | gpu-raster: rows + naive + visbuffer | 158.5 | 0.30 | 10.21 | 0.75 | 2.89 | 50.3 | 0.02 | 144 | 331 |
| 102 | gpu-raster: svo_dag + naive + gbuffer | 158.2 | 0.30 | 34.43 | 0.89 | 5.89 | 50.2 | 0.02 | 81 | 358 |
| 103 | gpu-raster: rows + naive + gbuffer | 157.0 | 0.29 | 10.26 | 0.79 | 2.91 | 50.2 | 0.02 | 144 | 332 |
| 104 | gpu-raster: svo_dag + naive + raster | 156.8 | 0.29 | 33.42 | 1.85 | 5.94 | 50.3 | 0.02 | 81 | 359 |
| 105 | gpu-raster: svo_dag + cubes + raster | 153.3 | 0.29 | 32.11 | 5.87 | 4.44 | 49.6 | 0.02 | 49 | 348 |
| 106 | gpu-raster: svo_dag + naive + visbuffer | 153.3 | 0.29 | 34.57 | 0.83 | 6.08 | 50.3 | 0.02 | 81 | 358 |
| 107 | gpu-raster: tree64 + greedy + raster | 128.5 | 0.24 | 16.72 | 0.93 | 7.34 | 49.2 | 0.02 | 48 | 324 |
| 108 | gpu-raster: tree64 + greedy + visbuffer | 126.0 | 0.24 | 16.96 | 0.60 | 7.50 | 49.2 | 0.02 | 48 | 327 |
| 109 | gpu-raster: tree64 + greedy + gbuffer | 124.9 | 0.23 | 17.44 | 0.67 | 7.54 | 49.1 | 0.02 | 48 | 323 |
| 110 | gpu-raster: bricks + greedy + visbuffer | 119.6 | 0.22 | 16.23 | 0.60 | 5.98 | 49.2 | 0.02 | 92 | 315 |
| 111 | gpu-raster: bricks + greedy + raster | 118.8 | 0.22 | 16.67 | 0.93 | 6.03 | 49.2 | 0.02 | 92 | 315 |
| 112 | gpu-raster: bricks + greedy + gbuffer | 118.8 | 0.22 | 16.57 | 0.67 | 5.99 | 49.1 | 0.02 | 92 | 316 |
| 113 | gpu-raster: clipmap + greedy + visbuffer | 111.7 | 0.21 | 16.61 | 0.64 | 6.02 | 49.2 | 0.02 | 175 | 325 |
| 114 | gpu-raster: clipmap + greedy + gbuffer | 111.0 | 0.21 | 17.09 | 0.74 | 6.02 | 49.1 | 0.02 | 175 | 324 |
| 115 | gpu-raster: clipmap + greedy + raster | 110.1 | 0.21 | 17.16 | 0.94 | 6.06 | 49.2 | 0.02 | 175 | 325 |
| 116 | gpu-raster: prefix + greedy + gbuffer | 109.5 | 0.20 | 16.79 | 0.64 | 6.02 | 49.1 | 0.02 | 97 | 314 |
| 117 | gpu-raster: prefix + greedy + visbuffer | 109.1 | 0.20 | 17.10 | 0.56 | 6.02 | 49.2 | 0.02 | 97 | 313 |
| 118 | gpu-raster: prefix + greedy + raster | 108.5 | 0.20 | 17.44 | 0.87 | 6.06 | 49.2 | 0.02 | 97 | 314 |
| 119 | gpu-raster: hier + greedy + raster | 107.4 | 0.20 | 17.14 | 0.89 | 6.04 | 49.2 | 0.02 | 92 | 316 |
| 120 | gpu-raster: hier + greedy + visbuffer | 106.8 | 0.20 | 17.73 | 0.58 | 6.02 | 49.2 | 0.02 | 92 | 315 |
| 121 | gpu-raster: hier + greedy + gbuffer | 106.8 | 0.20 | 17.89 | 0.67 | 6.01 | 49.1 | 0.02 | 92 | 314 |
| 122 | gpu-raster: rows + greedy + raster | 106.4 | 0.20 | 17.36 | 0.92 | 5.97 | 49.2 | 0.02 | 96 | 315 |
| 123 | gpu-raster: rows + greedy + visbuffer | 106.2 | 0.20 | 17.01 | 0.61 | 5.97 | 49.2 | 0.02 | 96 | 314 |
| 124 | gpu-raster: svo_dag + greedy + raster | 106.2 | 0.20 | 37.16 | 1.37 | 8.98 | 49.2 | 0.02 | 33 | 339 |
| 125 | gpu-raster: rows + greedy + gbuffer | 105.6 | 0.20 | 17.46 | 0.70 | 6.02 | 49.1 | 0.02 | 96 | 314 |
| 126 | gpu-raster: svo_dag + greedy + visbuffer | 105.3 | 0.20 | 37.93 | 0.70 | 9.06 | 49.2 | 0.02 | 33 | 342 |
| 127 | gpu-raster: svo_dag + greedy + gbuffer | 104.7 | 0.20 | 37.58 | 0.78 | 9.11 | 49.1 | 0.02 | 33 | 339 |
| 128 | gpu-rt: naive + rayquery | 76.0 | 0.14 | 26.46 | 5.94 | 11.88 | 56.5 | 0.01 | 325 | 703 |
| 129 | gpu-rt: greedy + rayquery | 74.1 | 0.14 | 27.28 | 4.25 | 12.32 | 56.5 | 0.01 | 125 | 629 |
| 130 | cpu: hier + refloor | 48.5 | 0.09 | 28.71 | - | 18.06 | 56.2 | 0.01 | - | 110 |
| 131 | cpu: prefix + refloor | 44.3 | 0.08 | 30.72 | - | 19.39 | 56.2 | 0.01 | - | 122 |
| 132 | cpu: hier + incremental | 42.7 | 0.08 | 36.48 | - | 20.81 | 56.2 | 0.01 | - | 110 |
| 133 | cpu: prefix + incremental | 39.6 | 0.07 | 38.16 | - | 21.75 | 56.2 | 0.01 | - | 122 |
| 134 | cpu: bricks + refloor | 39.2 | 0.07 | 34.79 | - | 23.39 | 56.2 | 0.01 | - | 106 |
| 135 | cpu: u8 + refloor | 38.4 | 0.07 | 34.50 | - | 23.86 | 56.2 | 0.01 | - | 91 |
| 136 | cpu: bits + refloor | 37.1 | 0.07 | 38.52 | - | 24.37 | 56.2 | 0.01 | - | 110 |
| 137 | gpu-discover: distance + quadtree | 36.4 | 0.07 | 56.46 | 2.17 | 0.09 | 49.8 | 0.02 | 231 | 299 |
| 138 | cpu: u8 + incremental | 36.4 | 0.07 | 40.84 | - | 25.19 | 56.2 | 0.01 | - | 91 |
| 139 | cpu: bricks + incremental | 36.3 | 0.07 | 39.41 | - | 25.29 | 56.2 | 0.01 | - | 106 |
| 140 | cpu: morton + refloor | 36.1 | 0.07 | 36.76 | - | 23.68 | 56.2 | 0.01 | - | 132 |
| 141 | gpu-compute: distance + incremental | 36.0 | 0.07 | 58.84 | 4.83 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 142 | gpu-compute: distance + beam + incremental | 35.9 | 0.07 | 58.32 | 5.07 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 143 | cpu: brickmajor + refloor | 35.3 | 0.07 | 37.47 | - | 23.86 | 56.2 | 0.01 | - | 132 |
| 144 | gpu-compute: distance + rowrun | 35.1 | 0.07 | 61.29 | 0.88 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 145 | gpu-compute: distance + sphere | 35.1 | 0.07 | 61.60 | 0.62 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 146 | gpu-compute: distance + beam + sphere | 34.9 | 0.07 | 61.27 | 0.64 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 147 | gpu-compute: distance + beam + refloor | 34.9 | 0.07 | 62.50 | 0.77 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 148 | gpu-compute: distance + beam + rowrun | 34.8 | 0.07 | 62.04 | 0.90 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 149 | gpu-compute: distance + refloor | 34.8 | 0.07 | 61.87 | 0.77 | 0.06 | 56.4 | 0.01 | 103 | 291 |
| 150 | cpu: morton + incremental | 34.6 | 0.06 | 41.48 | - | 24.79 | 56.2 | 0.01 | - | 131 |
| 151 | cpu: bits + incremental | 33.2 | 0.06 | 44.68 | - | 27.47 | 56.2 | 0.01 | - | 110 |
| 152 | cpu: palette + refloor | 33.1 | 0.06 | 40.81 | - | 23.91 | 56.2 | 0.01 | - | 84 |
| 153 | gpu-raster: distance + cubes + raster | 33.0 | 0.06 | 61.61 | 5.70 | 2.08 | 49.6 | 0.02 | 140 | 317 |
| 154 | gpu-raster: distance + cubes + gbuffer | 32.8 | 0.06 | 62.18 | 3.48 | 2.07 | 49.5 | 0.02 | 140 | 318 |
| 155 | gpu-raster: distance + cubes + visbuffer | 32.7 | 0.06 | 61.79 | 3.08 | 2.17 | 49.6 | 0.02 | 140 | 318 |
| 156 | cpu: brickmajor + incremental | 32.4 | 0.06 | 43.97 | - | 26.32 | 56.2 | 0.01 | - | 133 |
| 157 | cpu: palette + incremental | 31.6 | 0.06 | 44.44 | - | 25.34 | 56.2 | 0.01 | - | 84 |
| 158 | gpu-raster: distance + naive + raster | 31.2 | 0.06 | 63.48 | 1.38 | 3.97 | 50.3 | 0.02 | 172 | 331 |
| 159 | gpu-raster: distance + naive + visbuffer | 30.7 | 0.06 | 64.00 | 0.90 | 4.25 | 50.3 | 0.02 | 172 | 331 |
| 160 | gpu-raster: distance + naive + gbuffer | 30.5 | 0.06 | 63.70 | 1.31 | 4.35 | 50.2 | 0.02 | 172 | 332 |
| 161 | gpu-raster: distance + greedy + visbuffer | 28.8 | 0.05 | 65.52 | 0.63 | 6.55 | 49.2 | 0.02 | 124 | 316 |
| 162 | gpu-raster: distance + greedy + raster | 28.7 | 0.05 | 65.14 | 0.97 | 6.58 | 49.2 | 0.02 | 124 | 316 |
| 163 | gpu-raster: distance + greedy + gbuffer | 28.5 | 0.05 | 65.79 | 0.96 | 6.62 | 49.1 | 0.02 | 124 | 315 |
| 164 | cpu-raster: greedy + raster | 24.1 | 0.05 | 52.82 | - | 41.09 | 71.8 | 0.00 | - | 375 |
| 165 | cpu-raster: naive + raster | 17.8 | 0.03 | 66.35 | - | 54.33 | 77.5 | 0.00 | - | 473 |
| 166 | gpu-compute: prefix + refloor + pathtrace (incorrect) | 337.5 | 0.63 | 5.46 | 2.47 | 0.06 | 27.0 | 19.96 | 90 | 291 |
| 167 | gpu-compute: bricks + beam + refloor + pathtrace (incorrect) | 332.6 | 0.62 | 6.02 | 2.86 | 0.08 | 27.0 | 19.96 | 85 | 293 |
| 168 | gpu-compute: prefix + beam + refloor + pathtrace (incorrect) | 329.0 | 0.62 | 5.90 | 2.46 | 0.07 | 27.0 | 19.96 | 90 | 291 |
| 169 | gpu-compute: tree64 + beam + refloor + pathtrace (incorrect) | 324.8 | 0.61 | 5.28 | 3.01 | 1.81 | 27.0 | 19.96 | 42 | 332 |
| 170 | gpu-voxelspace: heightmap + columns (incorrect) | 323.6 | 0.61 | 7.84 | 2.88 | 0.01 | 14.4 | 44.70 | 8 | 302 |
| 171 | gpu-compute: bricks + refloor + pathtrace (incorrect) | 320.7 | 0.60 | 6.45 | 3.00 | 0.07 | 27.0 | 19.96 | 85 | 293 |
| 172 | gpu-compute: hier + beam + refloor + pathtrace (incorrect) | 317.9 | 0.59 | 6.02 | 2.58 | 0.07 | 27.0 | 19.96 | 85 | 294 |
| 173 | gpu-compute: tree64 + refloor + pathtrace (incorrect) | 317.6 | 0.59 | 5.41 | 3.07 | 1.83 | 27.0 | 19.96 | 42 | 322 |
| 174 | gpu-compute: hier + refloor + pathtrace (incorrect) | 311.4 | 0.58 | 6.22 | 2.63 | 0.07 | 27.0 | 19.96 | 85 | 294 |
| 175 | gpu-compute: rows + beam + refloor + pathtrace (incorrect) | 297.0 | 0.56 | 6.71 | 2.91 | 0.07 | 27.0 | 19.96 | 89 | 291 |
| 176 | gpu-compute: prefix + beam + rowrun + pathtrace (incorrect) | 296.5 | 0.55 | 6.41 | 3.14 | 0.05 | 27.0 | 19.96 | 90 | 291 |
| 177 | gpu-compute: prefix + rowrun + pathtrace (incorrect) | 296.2 | 0.55 | 6.64 | 3.13 | 0.06 | 27.0 | 19.96 | 90 | 291 |
| 178 | gpu-compute: rows + refloor + pathtrace (incorrect) | 290.1 | 0.54 | 6.62 | 3.06 | 0.07 | 27.0 | 19.96 | 89 | 291 |
| 179 | gpu-compute: clipmap + refloor + pathtrace (incorrect) | 281.2 | 0.53 | 6.79 | 3.33 | 0.01 | 27.0 | 19.96 | 168 | 298 |
| 180 | gpu-compute: clipmap + beam + refloor + pathtrace (incorrect) | 279.8 | 0.52 | 6.94 | 3.38 | 0.01 | 27.0 | 19.96 | 168 | 298 |
| 181 | cpu-voxelspace: heightmap + columns (incorrect) | 251.2 | 0.47 | 4.74 | - | 2.62 | 14.4 | 44.70 | - | 110 |
| 182 | gpu-rt: boxes + rayquery + pathtrace (incorrect) | 248.9 | 0.47 | 7.79 | 3.63 | 1.06 | 27.0 | 19.96 | 91 | 477 |
| 183 | gpu-compute: rows + beam + rowrun + pathtrace (incorrect) | 248.2 | 0.46 | 7.96 | 3.83 | 0.08 | 27.0 | 19.96 | 89 | 291 |
| 184 | gpu-compute: rows + rowrun + pathtrace (incorrect) | 240.8 | 0.45 | 7.72 | 3.98 | 0.07 | 27.0 | 19.96 | 89 | 291 |
| 185 | gpu-compute: bricks + rowrun + pathtrace (incorrect) | 209.2 | 0.39 | 8.06 | 4.70 | 0.10 | 27.0 | 19.96 | 85 | 293 |
| 186 | gpu-compute: svo_dag + beam + refloor + pathtrace (incorrect) | 208.0 | 0.39 | 30.13 | 4.27 | 3.16 | 27.0 | 19.96 | 26 | 340 |
| 187 | gpu-compute: clipmap + beam + incremental + pathtrace (incorrect) | 200.7 | 0.38 | 8.22 | 4.81 | 0.02 | 27.0 | 19.96 | 168 | 298 |
| 188 | gpu-compute: bricks + beam + rowrun + pathtrace (incorrect) | 199.2 | 0.37 | 8.49 | 4.94 | 0.11 | 27.0 | 19.96 | 85 | 293 |
| 189 | gpu-compute: clipmap + beam + rowrun + pathtrace (incorrect) | 199.2 | 0.37 | 8.01 | 4.81 | 0.02 | 27.0 | 19.96 | 168 | 298 |
| 190 | gpu-compute: svo_dag + refloor + pathtrace (incorrect) | 198.6 | 0.37 | 31.04 | 4.52 | 3.16 | 27.0 | 19.96 | 26 | 337 |
| 191 | gpu-compute: clipmap + incremental + pathtrace (incorrect) | 197.7 | 0.37 | 8.39 | 4.86 | 0.02 | 27.0 | 19.96 | 168 | 298 |
| 192 | gpu-compute: clipmap + rowrun + pathtrace (incorrect) | 195.6 | 0.37 | 8.32 | 4.90 | 0.02 | 27.0 | 19.96 | 168 | 298 |
| 193 | gpu-compute: tree64 + incremental + pathtrace (incorrect) | 193.9 | 0.36 | 9.44 | 5.11 | 2.14 | 27.0 | 19.96 | 42 | 323 |
| 194 | gpu-compute: tree64 + beam + incremental + pathtrace (incorrect) | 187.1 | 0.35 | 9.93 | 5.29 | 2.11 | 27.0 | 19.96 | 42 | 322 |
| 195 | gpu-compute: hier + rowrun + pathtrace (incorrect) | 179.5 | 0.34 | 10.36 | 5.49 | 0.09 | 27.0 | 19.96 | 85 | 294 |
| 196 | gpu-compute: hier + beam + rowrun + pathtrace (incorrect) | 170.9 | 0.32 | 11.28 | 5.76 | 0.10 | 27.0 | 19.96 | 85 | 294 |
| 197 | gpu-compute: hier + beam + sphere + pathtrace (incorrect) | 151.1 | 0.28 | 9.69 | 6.54 | 0.11 | 27.0 | 19.96 | 85 | 294 |
| 198 | gpu-compute: hier + sphere + pathtrace (incorrect) | 148.9 | 0.28 | 9.61 | 6.64 | 0.12 | 27.0 | 19.96 | 85 | 294 |
| 199 | gpu-compute: prefix + beam + sphere + pathtrace (incorrect) | 136.4 | 0.26 | 10.49 | 7.25 | 0.12 | 27.0 | 19.96 | 90 | 292 |
| 200 | gpu-compute: prefix + sphere + pathtrace (incorrect) | 136.3 | 0.25 | 10.51 | 7.25 | 0.11 | 27.0 | 19.96 | 90 | 291 |
| 201 | gpu-compute: bricks + beam + sphere + pathtrace (incorrect) | 133.2 | 0.25 | 10.46 | 7.43 | 0.13 | 27.0 | 19.96 | 85 | 293 |
| 202 | gpu-compute: rows + beam + sphere + pathtrace (incorrect) | 132.8 | 0.25 | 10.31 | 7.44 | 0.11 | 27.0 | 19.96 | 89 | 291 |
| 203 | gpu-compute: bricks + sphere + pathtrace (incorrect) | 131.6 | 0.25 | 10.51 | 7.52 | 0.13 | 27.0 | 19.96 | 85 | 293 |
| 204 | gpu-compute: rows + sphere + pathtrace (incorrect) | 130.3 | 0.24 | 10.43 | 7.59 | 0.12 | 27.0 | 19.96 | 89 | 291 |
| 205 | gpu-compute: clipmap + sphere + pathtrace (incorrect) | 122.2 | 0.23 | 10.94 | 8.01 | 0.03 | 27.0 | 19.96 | 168 | 298 |
| 206 | gpu-compute: clipmap + beam + sphere + pathtrace (incorrect) | 120.6 | 0.23 | 11.04 | 8.13 | 0.03 | 27.0 | 19.96 | 168 | 298 |
| 207 | gpu-compute: tree64 + beam + sphere + pathtrace (incorrect) | 113.7 | 0.21 | 11.85 | 8.74 | 2.21 | 27.0 | 19.96 | 42 | 332 |
| 208 | gpu-compute: tree64 + sphere + pathtrace (incorrect) | 108.3 | 0.20 | 12.52 | 9.18 | 2.09 | 27.0 | 19.96 | 42 | 323 |
| 209 | gpu-compute: svo_dag + beam + rowrun + pathtrace (incorrect) | 103.0 | 0.19 | 29.92 | 9.30 | 3.32 | 27.0 | 19.96 | 26 | 340 |
| 210 | gpu-compute: svo_dag + rowrun + pathtrace (incorrect) | 101.9 | 0.19 | 30.38 | 9.43 | 3.31 | 27.0 | 19.96 | 26 | 341 |
| 211 | gpu-compute: prefix + beam + incremental + pathtrace (incorrect) | 97.3 | 0.18 | 22.05 | 10.19 | 0.12 | 27.0 | 19.96 | 90 | 291 |
| 212 | gpu-compute: tree64 + beam + rowrun + pathtrace (incorrect) | 96.4 | 0.18 | 20.53 | 10.32 | 2.20 | 27.0 | 19.96 | 42 | 323 |
| 213 | gpu-compute: hier + beam + incremental + pathtrace (incorrect) | 94.5 | 0.18 | 25.18 | 10.50 | 0.13 | 27.0 | 19.96 | 85 | 294 |
| 214 | gpu-compute: svo_dag + beam + sphere + pathtrace (incorrect) | 92.0 | 0.17 | 29.24 | 10.51 | 3.27 | 27.0 | 19.96 | 26 | 340 |
| 215 | gpu-compute: prefix + incremental + pathtrace (incorrect) | 91.4 | 0.17 | 23.76 | 10.86 | 0.13 | 27.0 | 19.96 | 90 | 291 |
| 216 | gpu-compute: svo_dag + sphere + pathtrace (incorrect) | 91.3 | 0.17 | 30.59 | 10.57 | 3.33 | 27.0 | 19.96 | 26 | 340 |
| 217 | gpu-compute: svo_dag + incremental + pathtrace (incorrect) | 91.0 | 0.17 | 32.77 | 10.57 | 3.47 | 27.0 | 19.96 | 26 | 340 |
| 218 | gpu-compute: tree64 + rowrun + pathtrace (incorrect) | 89.8 | 0.17 | 22.61 | 11.09 | 2.17 | 27.0 | 19.96 | 42 | 322 |
| 219 | gpu-compute: hier + incremental + pathtrace (incorrect) | 88.7 | 0.17 | 27.43 | 11.19 | 0.12 | 27.0 | 19.96 | 85 | 294 |
| 220 | gpu-compute: bricks + beam + incremental + pathtrace (incorrect) | 86.8 | 0.16 | 24.11 | 11.43 | 0.13 | 27.0 | 19.96 | 85 | 293 |
| 221 | gpu-compute: svo_dag + beam + incremental + pathtrace (incorrect) | 86.0 | 0.16 | 29.85 | 11.27 | 3.30 | 27.0 | 19.96 | 26 | 341 |
| 222 | gpu-compute: rows + beam + incremental + pathtrace (incorrect) | 85.3 | 0.16 | 24.42 | 11.65 | 0.13 | 27.0 | 19.96 | 89 | 291 |
| 223 | gpu-compute: rows + incremental + pathtrace (incorrect) | 81.1 | 0.15 | 26.32 | 12.25 | 0.13 | 27.0 | 19.96 | 89 | 291 |
| 224 | gpu-compute: bricks + incremental + pathtrace (incorrect) | 80.7 | 0.15 | 26.35 | 12.30 | 0.12 | 27.0 | 19.96 | 85 | 293 |
| 225 | gpu-rt: naive + rayquery + pathtrace (incorrect) | 74.9 | 0.14 | 26.42 | 6.27 | 11.95 | 27.0 | 19.96 | 339 | 703 |
| 226 | gpu-rt: greedy + rayquery + pathtrace (incorrect) | 74.7 | 0.14 | 26.72 | 4.58 | 12.20 | 27.0 | 19.96 | 139 | 624 |
| 227 | cpu-discover: prefix + quadtree (incorrect) | 44.5 | 0.08 | 30.29 | 0.94 | 15.32 | 32.0 | 2.17 | 75 | 699 |
| 228 | cpu-discover: hier + quadtree (incorrect) | 43.5 | 0.08 | 30.38 | 0.95 | 16.31 | 32.0 | 2.17 | 75 | 633 |
| 229 | gpu-splat: bricks + points + splat (incorrect) | 41.0 | 0.08 | 53.60 | 1.66 | 0.08 | 17.3 | 22.43 | 122 | 379 |
| 230 | gpu-splat: clipmap + points + splat (incorrect) | 40.2 | 0.08 | 55.09 | 1.64 | 0.02 | 17.3 | 22.43 | 205 | 383 |
| 231 | cpu-discover: bits + quadtree (incorrect) | 40.0 | 0.07 | 33.91 | 0.96 | 18.68 | 32.0 | 2.17 | 75 | 639 |
| 232 | gpu-splat: tree64 + points + splat (incorrect) | 39.9 | 0.07 | 51.82 | 1.63 | 2.26 | 17.3 | 22.43 | 78 | 407 |
| 233 | cpu-discover: bricks + quadtree (incorrect) | 39.8 | 0.07 | 34.47 | 0.94 | 18.87 | 32.0 | 2.17 | 75 | 642 |
| 234 | cpu-discover: brickmajor + quadtree (incorrect) | 39.8 | 0.07 | 34.91 | 0.95 | 15.94 | 32.0 | 2.17 | 75 | 694 |
| 235 | cpu-discover: u8 + quadtree (incorrect) | 39.7 | 0.07 | 36.06 | 0.95 | 19.54 | 32.0 | 2.17 | 75 | 627 |
| 236 | cpu-discover: morton + quadtree (incorrect) | 39.3 | 0.07 | 34.07 | 0.95 | 15.96 | 32.0 | 2.17 | 75 | 691 |
| 237 | gpu-splat: rows + points + splat (incorrect) | 39.2 | 0.07 | 55.65 | 1.66 | 0.08 | 17.3 | 22.43 | 126 | 378 |
| 238 | gpu-splat: hier + points + splat (incorrect) | 39.1 | 0.07 | 55.72 | 1.63 | 0.08 | 17.3 | 22.43 | 122 | 378 |
| 239 | gpu-splat: prefix + points + splat (incorrect) | 39.1 | 0.07 | 56.27 | 1.63 | 0.08 | 17.3 | 22.43 | 127 | 379 |
| 240 | cpu-discover: palette + quadtree (incorrect) | 38.3 | 0.07 | 36.42 | 0.96 | 14.27 | 32.0 | 2.17 | 75 | 640 |
| 241 | gpu-compute: distance + incremental + pathtrace (incorrect) | 37.0 | 0.07 | 55.01 | 15.48 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 242 | gpu-compute: distance + beam + incremental + pathtrace (incorrect) | 36.9 | 0.07 | 55.80 | 14.45 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 243 | gpu-compute: distance + beam + refloor + pathtrace (incorrect) | 35.9 | 0.07 | 59.40 | 2.61 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 244 | gpu-compute: distance + rowrun + pathtrace (incorrect) | 35.9 | 0.07 | 58.86 | 3.65 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 245 | gpu-splat: svo_dag + points + splat (incorrect) | 35.8 | 0.07 | 55.54 | 1.74 | 4.92 | 17.3 | 22.42 | 62 | 427 |
| 246 | gpu-compute: distance + beam + rowrun + pathtrace (incorrect) | 35.7 | 0.07 | 59.08 | 3.59 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 247 | gpu-compute: distance + sphere + pathtrace (incorrect) | 35.5 | 0.07 | 61.36 | 2.11 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 248 | gpu-compute: distance + refloor + pathtrace (incorrect) | 35.4 | 0.07 | 60.62 | 2.71 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 249 | gpu-compute: distance + beam + sphere + pathtrace (incorrect) | 35.1 | 0.07 | 61.46 | 2.09 | 0.06 | 27.0 | 19.96 | 117 | 291 |
| 250 | cpu-discover: hier + cluster (incorrect) | 31.7 | 0.06 | 45.41 | 0.97 | 25.52 | 32.4 | 2.16 | 75 | 424 |
| 251 | cpu-discover: prefix + cluster (incorrect) | 31.6 | 0.06 | 46.50 | 0.97 | 25.03 | 32.4 | 2.16 | 75 | 491 |
| 252 | cpu-discover: brickmajor + cluster (incorrect) | 28.3 | 0.05 | 50.27 | 0.97 | 27.43 | 32.4 | 2.16 | 75 | 481 |
| 253 | cpu-discover: bricks + cluster (incorrect) | 28.1 | 0.05 | 51.17 | 0.98 | 30.35 | 32.4 | 2.16 | 75 | 425 |
| 254 | cpu-discover: morton + cluster (incorrect) | 28.0 | 0.05 | 49.28 | 0.97 | 27.17 | 32.4 | 2.16 | 75 | 477 |
| 255 | cpu-discover: palette + cluster (incorrect) | 27.7 | 0.05 | 49.53 | 0.97 | 26.44 | 32.4 | 2.16 | 75 | 411 |
| 256 | cpu-discover: bits + cluster (incorrect) | 27.4 | 0.05 | 53.20 | 0.97 | 30.11 | 32.4 | 2.16 | 75 | 423 |
| 257 | cpu-discover: u8 + cluster (incorrect) | 27.0 | 0.05 | 55.57 | 0.97 | 30.85 | 32.4 | 2.16 | 75 | 450 |
| 258 | gpu-splat: distance + points + splat (incorrect) | 20.1 | 0.04 | 106.16 | 1.74 | 0.08 | 17.2 | 22.44 | 154 | 378 |

### edits [unlit]

1280×720. camera 1.10 blocks/frame (max 1.1), turn 0.76°/frame; 974 chunks at start, 2.6 loads/frame; chunk fill 20%, solid 15.2%, surface 1.18%; sky 37%, hits at 148 blocks (median 136); 2090.8 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: tree64 + beam + refloor | 536.7 | 1.00 | 3.89 | 0.65 | 1.62 | 63.7 | 0.00 | 28 | 322 |
| 2 | gpu-compute: tree64 + refloor | 530.7 | 0.99 | 4.39 | 0.64 | 1.60 | 63.7 | 0.00 | 28 | 322 |
| 3 | gpu-compute: tree64 + incremental | 525.9 | 0.98 | 4.18 | 0.90 | 1.64 | 63.6 | 0.00 | 28 | 321 |
| 4 | gpu-compute: tree64 + beam + incremental | 522.7 | 0.97 | 4.00 | 0.93 | 1.66 | 63.6 | 0.00 | 28 | 323 |
| 5 | gpu-compute: tree64 + beam + rowrun | 490.2 | 0.91 | 3.82 | 1.73 | 1.62 | 63.7 | 0.00 | 28 | 323 |
| 6 | gpu-compute: tree64 + rowrun | 490.0 | 0.91 | 3.97 | 1.69 | 1.63 | 63.7 | 0.00 | 28 | 321 |
| 7 | gpu-compute: bricks + refloor | 489.1 | 0.91 | 4.06 | 0.64 | 0.06 | 63.7 | 0.00 | 71 | 293 |
| 8 | gpu-compute: bricks + beam + rowrun | 485.8 | 0.91 | 3.94 | 0.85 | 0.06 | 63.7 | 0.00 | 71 | 293 |
| 9 | gpu-compute: bricks + rowrun | 481.3 | 0.90 | 4.17 | 0.86 | 0.07 | 63.7 | 0.00 | 71 | 293 |
| 10 | gpu-compute: bricks + beam + refloor | 475.6 | 0.89 | 4.15 | 0.65 | 0.07 | 63.7 | 0.00 | 71 | 293 |
| 11 | gpu-discover: tree64 + quadtree | 462.2 | 0.86 | 4.75 | 1.41 | 1.73 | 55.4 | 0.01 | 155 | 335 |
| 12 | gpu-compute: tree64 + sphere | 456.0 | 0.85 | 3.72 | 2.03 | 1.70 | 63.9 | 0.00 | 28 | 321 |
| 13 | gpu-compute: tree64 + beam + sphere | 450.9 | 0.84 | 3.89 | 2.07 | 1.64 | 63.9 | 0.00 | 28 | 323 |
| 14 | gpu-compute: bricks + sphere | 433.1 | 0.81 | 4.56 | 1.97 | 0.06 | 63.9 | 0.00 | 71 | 293 |
| 15 | gpu-discover: bricks + quadtree | 430.1 | 0.80 | 4.14 | 1.29 | 0.10 | 55.4 | 0.01 | 199 | 302 |
| 16 | gpu-compute: bricks + beam + sphere | 414.9 | 0.77 | 4.98 | 2.01 | 0.08 | 63.9 | 0.00 | 71 | 293 |
| 17 | gpu-compute: clipmap + refloor | 412.4 | 0.77 | 4.69 | 0.75 | 0.01 | 63.7 | 0.00 | 154 | 298 |
| 18 | gpu-compute: clipmap + beam + refloor | 407.6 | 0.76 | 4.73 | 0.76 | 0.01 | 63.7 | 0.00 | 154 | 298 |
| 19 | gpu-compute: clipmap + beam + rowrun | 407.4 | 0.76 | 4.59 | 0.91 | 0.01 | 63.7 | 0.00 | 154 | 298 |
| 20 | gpu-compute: clipmap + rowrun | 407.2 | 0.76 | 4.58 | 0.91 | 0.01 | 63.7 | 0.00 | 154 | 298 |
| 21 | gpu-compute: clipmap + incremental | 406.7 | 0.76 | 4.65 | 0.92 | 0.01 | 63.6 | 0.00 | 154 | 298 |
| 22 | gpu-compute: clipmap + beam + incremental | 405.9 | 0.76 | 4.65 | 0.92 | 0.01 | 63.6 | 0.00 | 154 | 298 |
| 23 | gpu-compute: bricks + beam + incremental | 399.0 | 0.74 | 6.70 | 2.12 | 0.07 | 63.6 | 0.00 | 71 | 293 |
| 24 | gpu-compute: bricks + incremental | 398.8 | 0.74 | 6.61 | 2.10 | 0.08 | 63.6 | 0.00 | 71 | 293 |
| 25 | gpu-discover: clipmap + quadtree | 384.1 | 0.72 | 4.87 | 1.31 | 0.03 | 55.4 | 0.01 | 282 | 306 |
| 26 | gpu-compute: clipmap + sphere | 377.4 | 0.70 | 4.76 | 2.11 | 0.01 | 63.9 | 0.00 | 154 | 298 |
| 27 | gpu-compute: clipmap + beam + sphere | 372.0 | 0.69 | 4.86 | 2.18 | 0.01 | 63.9 | 0.00 | 154 | 298 |
| 28 | gpu-compute: prefix + beam + rowrun | 369.4 | 0.69 | 5.38 | 0.57 | 0.06 | 63.6 | 0.00 | 76 | 291 |
| 29 | gpu-compute: prefix + beam + refloor | 368.7 | 0.69 | 5.38 | 0.51 | 0.06 | 63.6 | 0.00 | 76 | 291 |
| 30 | gpu-compute: prefix + refloor | 368.5 | 0.69 | 5.30 | 0.50 | 0.06 | 63.6 | 0.00 | 76 | 291 |
| 31 | gpu-compute: prefix + beam + sphere | 354.5 | 0.66 | 5.46 | 1.82 | 0.07 | 63.9 | 0.00 | 76 | 291 |
| 32 | gpu-compute: prefix + rowrun | 353.6 | 0.66 | 5.70 | 0.58 | 0.07 | 63.6 | 0.00 | 76 | 291 |
| 33 | gpu-compute: hier + beam + refloor | 353.3 | 0.66 | 5.64 | 0.53 | 0.05 | 63.7 | 0.00 | 71 | 293 |
| 34 | gpu-compute: prefix + sphere | 352.7 | 0.66 | 5.72 | 1.81 | 0.06 | 63.9 | 0.00 | 76 | 291 |
| 35 | gpu-compute: hier + refloor | 351.7 | 0.66 | 5.68 | 0.52 | 0.05 | 63.7 | 0.00 | 71 | 293 |
| 36 | gpu-compute: hier + beam + rowrun | 350.3 | 0.65 | 5.66 | 0.94 | 0.06 | 63.7 | 0.00 | 71 | 294 |
| 37 | gpu-compute: hier + rowrun | 347.5 | 0.65 | 5.72 | 0.92 | 0.06 | 63.7 | 0.00 | 71 | 294 |
| 38 | gpu-compute: prefix + incremental | 345.7 | 0.64 | 5.68 | 1.72 | 0.06 | 63.6 | 0.00 | 76 | 291 |
| 39 | gpu-compute: hier + beam + sphere | 344.0 | 0.64 | 5.81 | 1.70 | 0.06 | 63.9 | 0.00 | 71 | 294 |
| 40 | gpu-compute: hier + sphere | 343.9 | 0.64 | 5.64 | 1.67 | 0.07 | 63.9 | 0.00 | 71 | 293 |
| 41 | gpu-discover: prefix + quadtree | 341.4 | 0.64 | 5.67 | 1.18 | 0.08 | 55.4 | 0.01 | 204 | 301 |
| 42 | gpu-compute: prefix + beam + incremental | 338.7 | 0.63 | 5.93 | 1.75 | 0.06 | 63.6 | 0.00 | 76 | 291 |
| 43 | gpu-discover: hier + quadtree | 330.9 | 0.62 | 5.78 | 1.23 | 0.08 | 55.4 | 0.01 | 199 | 302 |
| 44 | gpu-compute: rows + beam + rowrun | 330.3 | 0.62 | 6.02 | 0.72 | 0.05 | 63.7 | 0.00 | 75 | 291 |
| 45 | gpu-compute: rows + rowrun | 327.0 | 0.61 | 6.14 | 0.72 | 0.05 | 63.7 | 0.00 | 75 | 291 |
| 46 | gpu-compute: rows + refloor | 326.8 | 0.61 | 6.11 | 0.65 | 0.06 | 63.7 | 0.00 | 75 | 291 |
| 47 | gpu-compute: hier + beam + incremental | 325.9 | 0.61 | 6.65 | 1.92 | 0.06 | 63.6 | 0.00 | 71 | 294 |
| 48 | gpu-compute: rows + beam + refloor | 325.7 | 0.61 | 6.18 | 0.66 | 0.06 | 63.7 | 0.00 | 75 | 291 |
| 49 | gpu-compute: hier + incremental | 323.5 | 0.60 | 6.34 | 1.85 | 0.07 | 63.6 | 0.00 | 71 | 294 |
| 50 | gpu-compute: rows + sphere | 319.3 | 0.59 | 6.14 | 1.95 | 0.07 | 63.9 | 0.00 | 75 | 291 |
| 51 | gpu-compute: rows + beam + sphere | 316.7 | 0.59 | 6.26 | 1.99 | 0.07 | 63.9 | 0.00 | 75 | 291 |
| 52 | gpu-discover: rows + quadtree | 312.6 | 0.58 | 6.25 | 1.29 | 0.07 | 55.4 | 0.01 | 203 | 301 |
| 53 | gpu-compute: svo_dag + refloor | 308.0 | 0.57 | 29.73 | 0.66 | 2.94 | 63.6 | 0.00 | 12 | 340 |
| 54 | gpu-compute: svo_dag + beam + refloor | 307.7 | 0.57 | 29.55 | 0.68 | 2.92 | 63.6 | 0.00 | 12 | 340 |
| 55 | gpu-compute: svo_dag + beam + rowrun | 306.6 | 0.57 | 30.17 | 1.33 | 2.94 | 63.6 | 0.00 | 12 | 340 |
| 56 | gpu-compute: rows + incremental | 299.5 | 0.56 | 6.53 | 2.11 | 0.07 | 63.6 | 0.00 | 75 | 291 |
| 57 | gpu-compute: rows + beam + incremental | 298.1 | 0.56 | 7.40 | 2.25 | 0.07 | 63.6 | 0.00 | 75 | 291 |
| 58 | gpu-compute: svo_dag + incremental | 297.8 | 0.55 | 30.15 | 1.78 | 2.98 | 63.6 | 0.00 | 12 | 339 |
| 59 | gpu-compute: svo_dag + rowrun | 295.7 | 0.55 | 30.91 | 1.31 | 3.03 | 63.6 | 0.00 | 12 | 340 |
| 60 | gpu-compute: svo_dag + beam + incremental | 292.9 | 0.55 | 31.25 | 1.75 | 3.02 | 63.6 | 0.00 | 12 | 340 |
| 61 | gpu-compute: svo_dag + sphere | 291.8 | 0.54 | 30.33 | 2.22 | 3.02 | 63.9 | 0.00 | 12 | 340 |
| 62 | gpu-compute: svo_dag + beam + sphere | 288.4 | 0.54 | 32.57 | 2.19 | 3.08 | 63.9 | 0.00 | 12 | 340 |
| 63 | gpu-rt: boxes + rayquery | 284.3 | 0.53 | 5.57 | 0.85 | 1.03 | 63.7 | 0.00 | 77 | 470 |
| 64 | gpu-raster: bricks + cubes + visbuffer | 280.5 | 0.52 | 7.16 | 2.68 | 1.27 | 55.9 | 0.01 | 108 | 318 |
| 65 | gpu-discover: svo_dag + quadtree | 280.4 | 0.52 | 30.25 | 2.11 | 3.03 | 55.4 | 0.01 | 139 | 349 |
| 66 | gpu-raster: bricks + cubes + raster | 279.8 | 0.52 | 7.30 | 2.68 | 1.29 | 55.8 | 0.01 | 108 | 318 |
| 67 | gpu-raster: bricks + cubes + gbuffer | 279.0 | 0.52 | 7.44 | 2.73 | 1.28 | 55.7 | 0.01 | 108 | 318 |
| 68 | gpu-raster: bricks + naive + gbuffer | 195.6 | 0.36 | 8.99 | 0.51 | 2.78 | 56.1 | 0.01 | 140 | 331 |
| 69 | gpu-raster: bricks + naive + visbuffer | 193.3 | 0.36 | 8.90 | 0.47 | 2.82 | 56.3 | 0.01 | 140 | 330 |
| 70 | gpu-raster: bricks + naive + raster | 193.3 | 0.36 | 8.75 | 0.45 | 2.82 | 56.2 | 0.01 | 140 | 331 |
| 71 | gpu-raster: bricks + greedy + raster | 120.4 | 0.22 | 16.26 | 0.32 | 5.93 | 54.5 | 0.01 | 92 | 315 |
| 72 | gpu-raster: bricks + greedy + gbuffer | 119.7 | 0.22 | 16.32 | 0.40 | 5.96 | 54.5 | 0.01 | 92 | 314 |
| 73 | gpu-raster: bricks + greedy + visbuffer | 118.1 | 0.22 | 16.36 | 0.33 | 6.04 | 54.6 | 0.01 | 92 | 315 |
| 74 | gpu-rt: naive + rayquery | 75.6 | 0.14 | 26.60 | 5.85 | 11.82 | 64.1 | 0.00 | 325 | 697 |
| 75 | gpu-rt: greedy + rayquery | 74.1 | 0.14 | 26.67 | 4.17 | 12.26 | 63.9 | 0.00 | 125 | 634 |
| 76 | cpu: hier + refloor | 69.3 | 0.13 | 19.43 | - | 11.99 | 64.6 | 0.00 | - | 110 |
| 77 | cpu: prefix + refloor | 60.6 | 0.11 | 21.49 | - | 13.38 | 64.6 | 0.00 | - | 123 |
| 78 | cpu: hier + incremental | 57.8 | 0.11 | 25.00 | - | 14.71 | 64.6 | 0.00 | - | 110 |
| 79 | cpu: prefix + incremental | 55.6 | 0.10 | 24.56 | - | 14.69 | 64.6 | 0.00 | - | 124 |
| 80 | cpu: bricks + refloor | 52.7 | 0.10 | 23.68 | - | 16.81 | 64.6 | 0.00 | - | 106 |
| 81 | cpu: bits + refloor | 51.0 | 0.09 | 24.62 | - | 17.05 | 64.6 | 0.00 | - | 110 |
| 82 | cpu: u8 + refloor | 50.7 | 0.09 | 26.77 | - | 17.48 | 64.6 | 0.00 | - | 89 |
| 83 | cpu: bricks + incremental | 48.9 | 0.09 | 29.12 | - | 18.13 | 64.6 | 0.00 | - | 106 |
| 84 | cpu: u8 + incremental | 48.8 | 0.09 | 29.33 | - | 18.28 | 64.6 | 0.00 | - | 92 |
| 85 | cpu: morton + refloor | 48.6 | 0.09 | 25.99 | - | 16.76 | 64.6 | 0.00 | - | 130 |
| 86 | cpu: bits + incremental | 48.3 | 0.09 | 28.26 | - | 18.00 | 64.6 | 0.00 | - | 110 |
| 87 | cpu: brickmajor + refloor | 47.1 | 0.09 | 25.93 | - | 16.92 | 64.6 | 0.00 | - | 133 |
| 88 | cpu: morton + incremental | 45.3 | 0.08 | 30.10 | - | 18.08 | 64.6 | 0.00 | - | 132 |
| 89 | cpu: brickmajor + incremental | 44.4 | 0.08 | 30.37 | - | 18.12 | 64.6 | 0.00 | - | 133 |
| 90 | cpu-discover: hier + quadtree | 43.9 | 0.08 | 30.99 | 0.07 | 16.23 | 37.6 | 2.10 | 75 | 608 |
| 91 | cpu-discover: prefix + quadtree | 43.4 | 0.08 | 31.79 | 0.07 | 15.76 | 37.6 | 2.10 | 75 | 669 |
| 92 | cpu: palette + refloor | 43.2 | 0.08 | 29.14 | - | 17.15 | 64.6 | 0.00 | - | 84 |
| 93 | cpu: palette + incremental | 40.8 | 0.08 | 32.41 | - | 18.21 | 64.6 | 0.00 | - | 84 |
| 94 | cpu-discover: morton + quadtree | 40.4 | 0.08 | 34.27 | 0.07 | 15.87 | 37.6 | 2.10 | 75 | 686 |
| 95 | cpu-discover: bricks + quadtree | 40.1 | 0.07 | 34.37 | 0.07 | 18.85 | 37.6 | 2.10 | 75 | 635 |
| 96 | cpu-discover: bits + quadtree | 39.2 | 0.07 | 34.72 | 0.08 | 18.66 | 37.6 | 2.10 | 75 | 628 |
| 97 | cpu-discover: brickmajor + quadtree | 38.8 | 0.07 | 35.70 | 0.07 | 15.89 | 37.6 | 2.10 | 75 | 701 |
| 98 | cpu-discover: u8 + quadtree | 38.6 | 0.07 | 34.71 | 0.07 | 19.81 | 37.6 | 2.10 | 75 | 657 |
| 99 | cpu-discover: palette + quadtree | 38.3 | 0.07 | 35.75 | 0.08 | 14.54 | 37.6 | 2.10 | 75 | 623 |
| 100 | gpu-discover: distance + quadtree | 35.9 | 0.07 | 56.72 | 1.51 | 0.10 | 55.4 | 0.01 | 231 | 298 |
| 101 | gpu-compute: distance + beam + incremental | 35.4 | 0.07 | 62.70 | 3.52 | 0.06 | 63.7 | 0.00 | 103 | 291 |
| 102 | gpu-compute: distance + sphere | 35.4 | 0.07 | 61.46 | 0.41 | 0.06 | 63.9 | 0.00 | 103 | 291 |
| 103 | gpu-compute: distance + incremental | 35.2 | 0.07 | 62.37 | 3.37 | 0.06 | 63.7 | 0.00 | 103 | 291 |
| 104 | gpu-compute: distance + rowrun | 35.2 | 0.07 | 61.44 | 0.61 | 0.06 | 63.9 | 0.00 | 103 | 291 |
| 105 | gpu-compute: distance + beam + refloor | 35.1 | 0.07 | 61.50 | 0.53 | 0.06 | 63.9 | 0.00 | 103 | 291 |
| 106 | gpu-compute: distance + beam + sphere | 35.0 | 0.07 | 61.44 | 0.42 | 0.06 | 63.9 | 0.00 | 103 | 291 |
| 107 | gpu-compute: distance + beam + rowrun | 34.9 | 0.07 | 61.55 | 0.63 | 0.06 | 63.9 | 0.00 | 103 | 291 |
| 108 | gpu-compute: distance + refloor | 34.7 | 0.06 | 61.33 | 0.52 | 0.06 | 63.9 | 0.00 | 103 | 291 |
| 109 | cpu-discover: prefix + cluster | 32.1 | 0.06 | 44.00 | 0.07 | 24.30 | 38.4 | 2.09 | 75 | 481 |
| 110 | cpu-discover: hier + cluster | 31.2 | 0.06 | 45.30 | 0.07 | 25.76 | 38.4 | 2.09 | 75 | 429 |
| 111 | cpu-raster: greedy + raster | 29.1 | 0.05 | 46.58 | - | 33.93 | 74.1 | 0.00 | - | 368 |
| 112 | cpu-discover: morton + cluster | 28.6 | 0.05 | 48.79 | 0.07 | 27.48 | 38.4 | 2.09 | 75 | 471 |
| 113 | cpu-discover: bits + cluster | 28.4 | 0.05 | 55.42 | 0.07 | 30.03 | 38.4 | 2.09 | 75 | 404 |
| 114 | cpu-discover: palette + cluster | 28.0 | 0.05 | 50.94 | 0.07 | 26.74 | 38.4 | 2.09 | 75 | 419 |
| 115 | cpu-discover: brickmajor + cluster | 27.9 | 0.05 | 50.16 | 0.07 | 27.17 | 38.4 | 2.09 | 75 | 468 |
| 116 | cpu-discover: u8 + cluster | 27.8 | 0.05 | 64.23 | 0.07 | 30.63 | 38.4 | 2.09 | 75 | 449 |
| 117 | cpu-discover: bricks + cluster | 27.5 | 0.05 | 50.42 | 0.07 | 30.15 | 38.4 | 2.09 | 75 | 421 |
| 118 | cpu-raster: naive + raster | 20.1 | 0.04 | 60.94 | - | 47.58 | 77.9 | 0.00 | - | 451 |
| 119 | gpu-voxelspace: heightmap + columns (incorrect) | 458.3 | 0.85 | 3.87 | 1.42 | 0.01 | 17.0 | 38.43 | 8 | 302 |
| 120 | cpu-voxelspace: heightmap + columns (incorrect) | 379.3 | 0.71 | 3.27 | - | 1.32 | 17.0 | 38.43 | - | 110 |
| 121 | gpu-splat: bricks + points + splat (incorrect) | 40.2 | 0.07 | 54.81 | 1.36 | 0.09 | 23.2 | 17.93 | 122 | 378 |

### far [lit]

1920×1080. camera 4.78 blocks/frame (max 6.1), turn 0.52°/frame; 9141 chunks at start, 35.3 loads/frame; chunk fill 8%, solid 5.8%, surface 0.44%; sky 69%, hits at 465 blocks (median 455); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: bricks + refloor | 392.8 | 1.00 | 12.55 | 1.18 | 0.08 | 56.7 | 0.01 | 1045 | 826 |
| 2 | gpu-compute: bricks + beam + refloor | 366.8 | 0.93 | 12.61 | 1.41 | 0.08 | 56.7 | 0.01 | 1045 | 826 |
| 3 | gpu-compute: tree64 + refloor | 361.3 | 0.92 | 12.89 | 1.30 | 1.11 | 56.7 | 0.01 | 191 | 994 |
| 4 | gpu-compute: bricks + rowrun | 338.1 | 0.86 | 11.63 | 1.75 | 0.08 | 56.7 | 0.01 | 1045 | 826 |
| 5 | gpu-compute: prefix + refloor | 331.4 | 0.84 | 18.80 | 1.04 | 0.08 | 56.7 | 0.01 | 1125 | 830 |
| 6 | gpu-compute: hier + refloor | 322.0 | 0.82 | 18.89 | 1.08 | 0.08 | 56.7 | 0.01 | 1045 | 835 |
| 7 | gpu-compute: prefix + beam + refloor | 321.5 | 0.82 | 18.28 | 1.27 | 0.08 | 56.7 | 0.01 | 1125 | 830 |
| 8 | gpu-compute: bricks + beam + rowrun | 318.2 | 0.81 | 11.41 | 1.96 | 0.09 | 56.7 | 0.01 | 1045 | 826 |
| 9 | gpu-compute: prefix + rowrun | 315.3 | 0.80 | 18.17 | 1.30 | 0.08 | 56.7 | 0.01 | 1125 | 830 |
| 10 | gpu-compute: hier + beam + refloor | 311.6 | 0.79 | 18.53 | 1.30 | 0.08 | 56.7 | 0.01 | 1045 | 835 |
| 11 | gpu-compute: tree64 + beam + refloor | 310.1 | 0.79 | 12.93 | 1.93 | 1.11 | 56.7 | 0.01 | 191 | 995 |
| 12 | gpu-compute: svo_dag + refloor | 303.8 | 0.77 | 18.01 | 1.20 | 1.73 | 56.7 | 0.01 | 71 | 1061 |
| 13 | gpu-compute: rows + refloor | 301.3 | 0.77 | 19.86 | 1.20 | 0.08 | 56.7 | 0.01 | 1109 | 830 |
| 14 | gpu-compute: clipmap + refloor | 298.1 | 0.76 | 12.91 | 1.94 | 0.02 | 56.7 | 0.01 | 1239 | 886 |
| 15 | gpu-compute: tree64 + incremental | 294.4 | 0.75 | 12.02 | 2.20 | 1.11 | 56.7 | 0.01 | 191 | 996 |
| 16 | gpu-compute: prefix + beam + rowrun | 288.7 | 0.73 | 18.34 | 1.53 | 0.09 | 56.7 | 0.01 | 1125 | 830 |
| 17 | gpu-compute: rows + rowrun | 287.6 | 0.73 | 19.48 | 1.41 | 0.08 | 56.7 | 0.01 | 1109 | 830 |
| 18 | gpu-compute: rows + beam + refloor | 285.3 | 0.73 | 20.80 | 1.42 | 0.08 | 56.7 | 0.01 | 1109 | 830 |
| 19 | gpu-compute: rows + beam + rowrun | 275.3 | 0.70 | 19.97 | 1.64 | 0.09 | 56.7 | 0.01 | 1109 | 830 |
| 20 | gpu-compute: clipmap + rowrun | 273.1 | 0.70 | 11.50 | 2.37 | 0.02 | 56.7 | 0.01 | 1239 | 884 |
| 21 | gpu-compute: bricks + sphere | 273.0 | 0.70 | 10.82 | 2.74 | 0.08 | 56.7 | 0.01 | 1045 | 826 |
| 22 | gpu-compute: svo_dag + beam + refloor | 272.2 | 0.69 | 17.53 | 1.86 | 1.69 | 56.7 | 0.01 | 71 | 1061 |
| 23 | gpu-compute: bricks + beam + sphere | 264.9 | 0.67 | 10.12 | 2.96 | 0.09 | 56.7 | 0.01 | 1045 | 826 |
| 24 | gpu-compute: clipmap + incremental | 263.5 | 0.67 | 11.85 | 2.59 | 0.02 | 56.7 | 0.01 | 1239 | 887 |
| 25 | gpu-compute: tree64 + beam + incremental | 262.4 | 0.67 | 10.25 | 2.82 | 1.11 | 56.7 | 0.01 | 191 | 993 |
| 26 | gpu-compute: hier + rowrun | 258.4 | 0.66 | 15.73 | 2.16 | 0.08 | 56.7 | 0.01 | 1045 | 837 |
| 27 | gpu-compute: clipmap + beam + refloor | 252.7 | 0.64 | 11.55 | 2.79 | 0.02 | 56.7 | 0.01 | 1239 | 887 |
| 28 | gpu-compute: hier + beam + rowrun | 249.1 | 0.63 | 14.88 | 2.43 | 0.09 | 56.7 | 0.01 | 1045 | 838 |
| 29 | gpu-compute: hier + sphere | 248.3 | 0.63 | 13.35 | 2.47 | 0.08 | 56.7 | 0.01 | 1045 | 837 |
| 30 | gpu-compute: prefix + sphere | 238.6 | 0.61 | 14.45 | 2.76 | 0.08 | 56.7 | 0.01 | 1125 | 834 |
| 31 | gpu-compute: clipmap + beam + rowrun | 236.8 | 0.60 | 11.23 | 3.20 | 0.02 | 56.7 | 0.01 | 1239 | 887 |
| 32 | gpu-compute: hier + beam + sphere | 236.0 | 0.60 | 14.36 | 2.69 | 0.09 | 56.7 | 0.01 | 1045 | 838 |
| 33 | gpu-compute: prefix + beam + sphere | 231.7 | 0.59 | 13.73 | 2.97 | 0.08 | 56.7 | 0.01 | 1125 | 835 |
| 34 | gpu-compute: rows + sphere | 229.6 | 0.58 | 14.97 | 2.73 | 0.08 | 56.7 | 0.01 | 1109 | 834 |
| 35 | gpu-compute: clipmap + beam + incremental | 222.9 | 0.57 | 11.73 | 3.49 | 0.02 | 56.7 | 0.01 | 1239 | 887 |
| 36 | gpu-compute: tree64 + sphere | 220.1 | 0.56 | 9.81 | 3.85 | 1.13 | 56.7 | 0.01 | 191 | 992 |
| 37 | gpu-rt: boxes + rayquery | 219.6 | 0.56 | 22.00 | 1.46 | 1.70 | 56.6 | 0.01 | 1130 | 1351 |
| 38 | gpu-compute: svo_dag + sphere | 218.8 | 0.56 | 12.58 | 3.12 | 1.71 | 56.7 | 0.01 | 71 | 1062 |
| 39 | gpu-compute: rows + beam + sphere | 217.8 | 0.55 | 16.27 | 2.94 | 0.09 | 56.7 | 0.01 | 1109 | 834 |
| 40 | gpu-compute: clipmap + sphere | 209.4 | 0.53 | 13.68 | 3.83 | 0.02 | 56.7 | 0.01 | 1239 | 884 |
| 41 | gpu-compute: svo_dag + rowrun | 208.4 | 0.53 | 13.14 | 3.48 | 1.66 | 56.7 | 0.01 | 71 | 1060 |
| 42 | gpu-compute: tree64 + rowrun | 205.2 | 0.52 | 9.78 | 4.30 | 1.12 | 56.7 | 0.01 | 191 | 995 |
| 43 | gpu-compute: svo_dag + beam + sphere | 201.3 | 0.51 | 11.77 | 3.75 | 1.72 | 56.7 | 0.01 | 71 | 1062 |
| 44 | gpu-compute: tree64 + beam + sphere | 200.2 | 0.51 | 9.18 | 4.51 | 1.13 | 56.7 | 0.01 | 191 | 996 |
| 45 | gpu-compute: svo_dag + incremental | 193.8 | 0.49 | 11.35 | 4.02 | 1.70 | 56.7 | 0.01 | 71 | 1061 |
| 46 | gpu-compute: svo_dag + beam + rowrun | 191.6 | 0.49 | 11.73 | 4.08 | 1.71 | 56.7 | 0.01 | 71 | 1061 |
| 47 | gpu-raster: bricks + naive + gbuffer | 188.8 | 0.48 | 24.10 | 1.82 | 2.44 | 50.3 | 0.02 | 1310 | 1056 |
| 48 | gpu-compute: clipmap + beam + sphere | 188.6 | 0.48 | 12.15 | 4.70 | 0.03 | 56.7 | 0.01 | 1239 | 887 |
| 49 | gpu-raster: bricks + naive + visbuffer | 187.0 | 0.48 | 22.98 | 1.83 | 2.44 | 50.3 | 0.02 | 1310 | 1055 |
| 50 | gpu-compute: tree64 + beam + rowrun | 185.2 | 0.47 | 9.14 | 4.97 | 1.12 | 56.7 | 0.01 | 191 | 997 |
| 51 | gpu-discover: bricks + quadtree | 184.5 | 0.47 | 11.01 | 5.12 | 0.16 | 48.0 | 0.02 | 1227 | 835 |
| 52 | gpu-raster: tree64 + naive + gbuffer | 183.8 | 0.47 | 34.84 | 1.86 | 3.42 | 50.3 | 0.02 | 456 | 1040 |
| 53 | gpu-raster: tree64 + naive + visbuffer | 183.1 | 0.47 | 35.05 | 1.86 | 3.46 | 50.3 | 0.02 | 456 | 1056 |
| 54 | gpu-discover: prefix + quadtree | 178.7 | 0.45 | 14.12 | 4.78 | 0.15 | 48.0 | 0.02 | 1307 | 839 |
| 55 | gpu-compute: svo_dag + beam + incremental | 177.5 | 0.45 | 10.42 | 4.71 | 1.72 | 56.7 | 0.01 | 71 | 1062 |
| 56 | gpu-raster: clipmap + naive + visbuffer | 175.1 | 0.45 | 26.28 | 2.01 | 2.39 | 50.3 | 0.02 | 1505 | 1163 |
| 57 | gpu-raster: clipmap + naive + gbuffer | 173.6 | 0.44 | 25.11 | 2.01 | 2.41 | 50.3 | 0.02 | 1505 | 1163 |
| 58 | gpu-discover: hier + quadtree | 173.2 | 0.44 | 14.81 | 4.93 | 0.15 | 48.0 | 0.02 | 1227 | 838 |
| 59 | gpu-raster: prefix + naive + gbuffer | 171.7 | 0.44 | 29.82 | 1.78 | 2.44 | 50.3 | 0.02 | 1390 | 1053 |
| 60 | gpu-raster: prefix + naive + visbuffer | 170.7 | 0.43 | 30.83 | 1.78 | 2.45 | 50.3 | 0.02 | 1390 | 1053 |
| 61 | gpu-raster: hier + naive + gbuffer | 169.6 | 0.43 | 28.26 | 1.81 | 2.44 | 50.3 | 0.02 | 1310 | 1060 |
| 62 | gpu-raster: hier + naive + visbuffer | 167.7 | 0.43 | 30.11 | 1.81 | 2.48 | 50.3 | 0.02 | 1310 | 1057 |
| 63 | gpu-discover: tree64 + quadtree | 167.2 | 0.43 | 9.38 | 5.59 | 1.19 | 48.0 | 0.02 | 373 | 1000 |
| 64 | gpu-raster: svo_dag + naive + visbuffer | 165.6 | 0.42 | 41.64 | 1.86 | 4.07 | 50.3 | 0.02 | 337 | 1126 |
| 65 | gpu-discover: rows + quadtree | 165.5 | 0.42 | 15.45 | 5.13 | 0.15 | 48.0 | 0.02 | 1291 | 833 |
| 66 | gpu-raster: rows + naive + gbuffer | 165.0 | 0.42 | 31.81 | 1.82 | 2.45 | 50.3 | 0.02 | 1374 | 1054 |
| 67 | gpu-raster: rows + naive + visbuffer | 164.6 | 0.42 | 31.27 | 1.82 | 2.44 | 50.3 | 0.02 | 1374 | 1056 |
| 68 | gpu-raster: svo_dag + naive + gbuffer | 164.6 | 0.42 | 42.22 | 1.87 | 4.09 | 50.3 | 0.02 | 337 | 1139 |
| 69 | gpu-raster: bricks + naive + raster | 158.7 | 0.40 | 18.52 | 3.27 | 2.50 | 50.4 | 0.02 | 1310 | 1056 |
| 70 | gpu-raster: tree64 + naive + raster | 152.0 | 0.39 | 22.46 | 3.52 | 3.50 | 50.4 | 0.02 | 456 | 1046 |
| 71 | gpu-discover: svo_dag + quadtree | 149.4 | 0.38 | 10.74 | 5.90 | 1.82 | 48.0 | 0.02 | 253 | 1069 |
| 72 | gpu-raster: prefix + naive + raster | 148.3 | 0.38 | 23.62 | 3.16 | 2.49 | 50.4 | 0.02 | 1390 | 1053 |
| 73 | gpu-raster: clipmap + naive + raster | 147.6 | 0.38 | 20.59 | 3.68 | 2.44 | 50.4 | 0.02 | 1505 | 1162 |
| 74 | gpu-raster: hier + naive + raster | 146.1 | 0.37 | 23.82 | 3.25 | 2.49 | 50.4 | 0.02 | 1310 | 1059 |
| 75 | gpu-discover: clipmap + quadtree | 145.4 | 0.37 | 12.66 | 6.61 | 0.09 | 48.0 | 0.02 | 1421 | 893 |
| 76 | gpu-compute: prefix + incremental | 143.2 | 0.36 | 14.28 | 6.67 | 0.09 | 56.7 | 0.01 | 1125 | 830 |
| 77 | gpu-raster: rows + naive + raster | 142.0 | 0.36 | 28.29 | 3.25 | 2.48 | 50.4 | 0.02 | 1374 | 1055 |
| 78 | gpu-compute: hier + incremental | 141.8 | 0.36 | 15.58 | 6.75 | 0.09 | 56.7 | 0.01 | 1045 | 835 |
| 79 | gpu-compute: prefix + beam + incremental | 140.3 | 0.36 | 14.73 | 6.91 | 0.09 | 56.7 | 0.01 | 1125 | 830 |
| 80 | gpu-raster: svo_dag + naive + raster | 138.3 | 0.35 | 24.21 | 3.65 | 4.10 | 50.4 | 0.02 | 337 | 1118 |
| 81 | gpu-compute: bricks + incremental | 136.9 | 0.35 | 13.22 | 7.21 | 0.09 | 56.7 | 0.01 | 1045 | 826 |
| 82 | gpu-compute: hier + beam + incremental | 134.9 | 0.34 | 16.61 | 7.16 | 0.09 | 56.7 | 0.01 | 1045 | 835 |
| 83 | gpu-compute: bricks + beam + incremental | 133.5 | 0.34 | 10.92 | 7.40 | 0.10 | 56.7 | 0.01 | 1045 | 826 |
| 84 | gpu-raster: bricks + greedy + visbuffer | 129.0 | 0.33 | 41.15 | 0.95 | 5.49 | 48.7 | 0.02 | 1182 | 945 |
| 85 | gpu-compute: rows + incremental | 128.2 | 0.33 | 16.71 | 7.45 | 0.09 | 56.7 | 0.01 | 1109 | 830 |
| 86 | gpu-raster: tree64 + greedy + gbuffer | 127.5 | 0.32 | 65.49 | 0.99 | 6.47 | 48.7 | 0.02 | 328 | 1006 |
| 87 | gpu-raster: tree64 + greedy + visbuffer | 127.2 | 0.32 | 65.89 | 0.98 | 6.47 | 48.7 | 0.02 | 328 | 1013 |
| 88 | gpu-raster: bricks + greedy + gbuffer | 124.5 | 0.32 | 42.93 | 0.95 | 5.61 | 48.7 | 0.02 | 1182 | 945 |
| 89 | gpu-raster: clipmap + greedy + gbuffer | 123.1 | 0.31 | 44.75 | 1.14 | 5.40 | 48.7 | 0.02 | 1377 | 1020 |
| 90 | gpu-raster: clipmap + greedy + visbuffer | 123.1 | 0.31 | 44.23 | 1.13 | 5.43 | 48.7 | 0.02 | 1377 | 1022 |
| 91 | gpu-raster: bricks + greedy + raster | 122.2 | 0.31 | 40.23 | 1.61 | 5.51 | 48.8 | 0.02 | 1182 | 945 |
| 92 | gpu-raster: prefix + greedy + visbuffer | 120.7 | 0.31 | 47.69 | 0.90 | 5.47 | 48.7 | 0.02 | 1262 | 941 |
| 93 | gpu-raster: prefix + greedy + gbuffer | 120.7 | 0.31 | 48.60 | 0.92 | 5.48 | 48.7 | 0.02 | 1262 | 940 |
| 94 | gpu-compute: rows + beam + incremental | 120.0 | 0.31 | 16.72 | 8.08 | 0.10 | 56.7 | 0.01 | 1109 | 830 |
| 95 | gpu-raster: tree64 + greedy + raster | 119.8 | 0.31 | 64.23 | 1.81 | 6.43 | 48.8 | 0.02 | 328 | 1007 |
| 96 | gpu-raster: hier + greedy + gbuffer | 119.3 | 0.30 | 48.40 | 0.93 | 5.50 | 48.7 | 0.02 | 1182 | 948 |
| 97 | gpu-raster: rows + greedy + visbuffer | 118.6 | 0.30 | 48.81 | 0.95 | 5.42 | 48.7 | 0.02 | 1246 | 941 |
| 98 | gpu-raster: hier + greedy + visbuffer | 118.5 | 0.30 | 48.25 | 0.95 | 5.49 | 48.7 | 0.02 | 1182 | 948 |
| 99 | gpu-raster: svo_dag + greedy + gbuffer | 118.5 | 0.30 | 73.17 | 1.01 | 7.09 | 48.7 | 0.02 | 209 | 1079 |
| 100 | gpu-raster: svo_dag + greedy + visbuffer | 118.2 | 0.30 | 72.04 | 0.98 | 7.10 | 48.7 | 0.02 | 209 | 1085 |
| 101 | gpu-raster: rows + greedy + gbuffer | 118.1 | 0.30 | 49.37 | 0.97 | 5.45 | 48.7 | 0.02 | 1246 | 942 |
| 102 | gpu-raster: prefix + greedy + raster | 115.8 | 0.29 | 45.92 | 1.52 | 5.45 | 48.8 | 0.02 | 1262 | 943 |
| 103 | gpu-raster: clipmap + greedy + raster | 115.3 | 0.29 | 41.27 | 1.98 | 5.40 | 48.8 | 0.02 | 1377 | 1019 |
| 104 | gpu-raster: hier + greedy + raster | 114.1 | 0.29 | 44.83 | 1.60 | 5.47 | 48.8 | 0.02 | 1182 | 948 |
| 105 | gpu-raster: rows + greedy + raster | 112.3 | 0.29 | 47.31 | 1.62 | 5.45 | 48.8 | 0.02 | 1246 | 941 |
| 106 | gpu-raster: svo_dag + greedy + raster | 109.4 | 0.28 | 72.26 | 1.88 | 7.12 | 48.8 | 0.02 | 209 | 1085 |
| 107 | gpu-raster: prefix + cubes + visbuffer | 78.2 | 0.20 | 17.14 | 12.59 | 1.73 | 50.2 | 0.02 | 1390 | 986 |
| 108 | gpu-raster: prefix + cubes + gbuffer | 78.1 | 0.20 | 16.39 | 12.59 | 1.72 | 50.2 | 0.02 | 1390 | 987 |
| 109 | gpu-raster: hier + cubes + visbuffer | 78.1 | 0.20 | 17.03 | 12.62 | 1.72 | 50.2 | 0.02 | 1310 | 991 |
| 110 | gpu-raster: bricks + cubes + visbuffer | 78.0 | 0.20 | 15.25 | 12.66 | 1.75 | 50.2 | 0.02 | 1310 | 986 |
| 111 | gpu-raster: bricks + cubes + gbuffer | 78.0 | 0.20 | 15.38 | 12.66 | 1.75 | 50.2 | 0.02 | 1310 | 987 |
| 112 | gpu-raster: hier + cubes + gbuffer | 77.8 | 0.20 | 16.89 | 12.64 | 1.71 | 50.2 | 0.02 | 1310 | 988 |
| 113 | gpu-raster: rows + cubes + visbuffer | 77.8 | 0.20 | 18.07 | 12.64 | 1.72 | 50.2 | 0.02 | 1374 | 984 |
| 114 | gpu-raster: rows + cubes + gbuffer | 77.6 | 0.20 | 24.46 | 12.64 | 1.73 | 50.2 | 0.02 | 1374 | 984 |
| 115 | gpu-raster: clipmap + cubes + visbuffer | 76.5 | 0.19 | 16.41 | 12.82 | 1.68 | 50.2 | 0.02 | 1505 | 1093 |
| 116 | gpu-raster: clipmap + cubes + gbuffer | 76.4 | 0.19 | 16.33 | 12.83 | 1.70 | 50.2 | 0.02 | 1505 | 1094 |
| 117 | gpu-raster: tree64 + cubes + gbuffer | 76.3 | 0.19 | 16.05 | 12.70 | 2.75 | 50.2 | 0.02 | 456 | 1014 |
| 118 | gpu-raster: tree64 + cubes + visbuffer | 76.1 | 0.19 | 16.61 | 12.68 | 2.82 | 50.2 | 0.02 | 456 | 1018 |
| 119 | gpu-raster: svo_dag + cubes + visbuffer | 73.7 | 0.19 | 18.37 | 12.67 | 3.37 | 50.2 | 0.02 | 337 | 1124 |
| 120 | gpu-raster: svo_dag + cubes + gbuffer | 73.7 | 0.19 | 18.81 | 12.68 | 3.36 | 50.2 | 0.02 | 337 | 1114 |
| 121 | gpu-rt: greedy + rayquery | 65.9 | 0.17 | 132.92 | 1.61 | 11.09 | 56.7 | 0.01 | 770 | 2081 |
| 122 | gpu-compute: distance + refloor | 49.6 | 0.13 | 201.46 | 1.09 | 0.08 | 56.7 | 0.01 | 1557 | 835 |
| 123 | gpu-compute: distance + beam + refloor | 48.8 | 0.12 | 198.34 | 1.37 | 0.08 | 56.7 | 0.01 | 1557 | 835 |
| 124 | gpu-compute: distance + sphere | 48.0 | 0.12 | 203.14 | 1.00 | 0.08 | 56.7 | 0.01 | 1557 | 835 |
| 125 | gpu-compute: distance + beam + rowrun | 46.2 | 0.12 | 185.05 | 1.79 | 0.09 | 56.7 | 0.01 | 1557 | 835 |
| 126 | gpu-compute: distance + rowrun | 46.0 | 0.12 | 208.43 | 1.58 | 0.09 | 56.7 | 0.01 | 1557 | 831 |
| 127 | gpu-compute: distance + beam + sphere | 45.3 | 0.12 | 212.17 | 1.56 | 0.09 | 56.7 | 0.01 | 1557 | 835 |
| 128 | gpu-raster: distance + naive + gbuffer | 44.1 | 0.11 | 201.18 | 1.93 | 2.43 | 50.3 | 0.02 | 1822 | 1064 |
| 129 | gpu-raster: distance + naive + visbuffer | 43.7 | 0.11 | 197.68 | 1.88 | 2.44 | 50.3 | 0.02 | 1822 | 1063 |
| 130 | gpu-discover: distance + quadtree | 43.7 | 0.11 | 118.72 | 5.13 | 0.16 | 48.0 | 0.02 | 1739 | 834 |
| 131 | gpu-raster: distance + naive + raster | 41.4 | 0.11 | 168.21 | 3.50 | 2.48 | 50.4 | 0.02 | 1822 | 1052 |
| 132 | gpu-raster: prefix + cubes + raster | 40.4 | 0.10 | 28.11 | 24.58 | 1.78 | 50.2 | 0.02 | 1390 | 986 |
| 133 | gpu-raster: rows + cubes + raster | 40.4 | 0.10 | 28.09 | 24.60 | 1.77 | 50.2 | 0.02 | 1374 | 987 |
| 134 | gpu-raster: hier + cubes + raster | 40.4 | 0.10 | 27.89 | 24.62 | 1.78 | 50.2 | 0.02 | 1310 | 989 |
| 135 | gpu-raster: bricks + cubes + raster | 40.3 | 0.10 | 28.24 | 24.65 | 1.79 | 50.2 | 0.02 | 1310 | 987 |
| 136 | gpu-raster: tree64 + cubes + raster | 40.3 | 0.10 | 27.77 | 24.70 | 2.82 | 50.2 | 0.02 | 456 | 1012 |
| 137 | gpu-raster: svo_dag + cubes + raster | 40.3 | 0.10 | 27.76 | 24.69 | 3.41 | 50.2 | 0.02 | 337 | 1121 |
| 138 | gpu-raster: clipmap + cubes + raster | 40.1 | 0.10 | 28.42 | 24.69 | 1.71 | 50.2 | 0.02 | 1505 | 1094 |
| 139 | gpu-compute: distance + incremental | 38.0 | 0.10 | 115.45 | 10.06 | 0.10 | 56.7 | 0.01 | 1557 | 835 |
| 140 | gpu-compute: distance + beam + incremental | 37.3 | 0.10 | 120.47 | 10.55 | 0.10 | 56.7 | 0.01 | 1557 | 835 |
| 141 | gpu-raster: distance + greedy + visbuffer | 37.1 | 0.09 | 230.16 | 1.70 | 5.61 | 48.7 | 0.02 | 1694 | 952 |
| 142 | gpu-raster: distance + greedy + gbuffer | 36.8 | 0.09 | 212.78 | 2.24 | 5.61 | 48.7 | 0.02 | 1694 | 948 |
| 143 | gpu-raster: distance + greedy + raster | 36.8 | 0.09 | 199.26 | 2.42 | 5.57 | 48.8 | 0.02 | 1694 | 952 |
| 144 | gpu-raster: distance + cubes + visbuffer | 34.9 | 0.09 | 115.62 | 12.58 | 1.72 | 50.2 | 0.02 | 1822 | 1015 |
| 145 | gpu-raster: distance + cubes + gbuffer | 34.8 | 0.09 | 116.46 | 12.57 | 1.71 | 50.2 | 0.02 | 1822 | 993 |
| 146 | cpu: hier + refloor | 32.6 | 0.08 | 43.90 | - | 29.37 | 58.2 | 0.00 | - | 667 |
| 147 | cpu: hier + incremental | 31.3 | 0.08 | 43.81 | - | 30.77 | 58.3 | 0.00 | - | 667 |
| 148 | cpu: prefix + incremental | 30.5 | 0.08 | 45.94 | - | 31.46 | 58.2 | 0.00 | - | 882 |
| 149 | cpu: prefix + refloor | 29.0 | 0.07 | 49.26 | - | 33.08 | 58.2 | 0.00 | - | 879 |
| 150 | cpu: bricks + refloor | 28.9 | 0.07 | 46.53 | - | 33.80 | 58.2 | 0.00 | - | 632 |
| 151 | cpu: u8 + refloor | 28.7 | 0.07 | 46.22 | - | 33.97 | 58.2 | 0.00 | - | 546 |
| 152 | cpu: bits + incremental | 28.2 | 0.07 | 47.32 | - | 34.23 | 58.3 | 0.00 | - | 667 |
| 153 | cpu: bits + refloor | 28.1 | 0.07 | 49.07 | - | 34.24 | 58.2 | 0.00 | - | 667 |
| 154 | cpu: u8 + incremental | 28.1 | 0.07 | 45.84 | - | 34.69 | 58.3 | 0.00 | - | 544 |
| 155 | cpu: morton + refloor | 28.0 | 0.07 | 50.03 | - | 34.22 | 58.2 | 0.00 | - | 961 |
| 156 | cpu: morton + incremental | 27.8 | 0.07 | 48.59 | - | 34.50 | 58.3 | 0.00 | - | 964 |
| 157 | cpu: brickmajor + refloor | 27.8 | 0.07 | 50.00 | - | 34.42 | 58.2 | 0.00 | - | 964 |
| 158 | cpu: bricks + incremental | 27.6 | 0.07 | 47.34 | - | 35.44 | 58.3 | 0.00 | - | 632 |
| 159 | gpu-raster: distance + cubes + raster | 27.6 | 0.07 | 117.12 | 24.49 | 1.73 | 50.2 | 0.02 | 1822 | 993 |
| 160 | cpu: palette + refloor | 27.4 | 0.07 | 55.49 | - | 33.97 | 58.2 | 0.00 | - | 534 |
| 161 | cpu: palette + incremental | 26.7 | 0.07 | 55.24 | - | 35.02 | 58.3 | 0.00 | - | 525 |
| 162 | cpu: brickmajor + incremental | 26.7 | 0.07 | 51.30 | - | 35.87 | 58.3 | 0.00 | - | 974 |
| 163 | cpu-raster: greedy + raster | 14.2 | 0.04 | 96.52 | - | 67.72 | 63.2 | 0.00 | - | 1024 |
| 164 | cpu-raster: naive + raster | 7.5 | 0.02 | 151.52 | - | 126.44 | 64.8 | 0.00 | - | 1606 |
| 165 | cpu-discover: prefix + cluster | 5.7 | 0.01 | 421.67 | 3.46 | 171.23 | 35.1 | 0.13 | 1053 | 1942 |
| 166 | cpu-discover: hier + cluster | 5.7 | 0.01 | 427.53 | 3.47 | 173.78 | 35.1 | 0.13 | 1053 | 1449 |
| 167 | cpu-discover: u8 + cluster | 5.4 | 0.01 | 443.29 | 3.46 | 181.49 | 35.1 | 0.13 | 1053 | 1727 |
| 168 | cpu-discover: morton + cluster | 5.4 | 0.01 | 438.58 | 3.47 | 181.41 | 35.1 | 0.13 | 1053 | 2004 |
| 169 | cpu-discover: brickmajor + cluster | 5.4 | 0.01 | 451.18 | 3.47 | 182.44 | 35.1 | 0.13 | 1053 | 2039 |
| 170 | cpu-discover: palette + cluster | 5.4 | 0.01 | 437.82 | 3.46 | 181.95 | 35.1 | 0.13 | 1053 | 1488 |
| 171 | cpu-discover: bricks + cluster | 5.2 | 0.01 | 467.35 | 3.48 | 188.61 | 35.1 | 0.13 | 1053 | 1453 |
| 172 | cpu-discover: bits + cluster | 5.0 | 0.01 | 571.06 | 3.48 | 196.79 | 35.1 | 0.13 | 1053 | 1467 |
| 173 | gpu-compute: bricks + refloor + pathtrace (incorrect) | 239.5 | 0.61 | 10.50 | 3.49 | 0.09 | 32.2 | 7.12 | 1076 | 826 |
| 174 | gpu-voxelspace: heightmap + columns (incorrect) | 238.5 | 0.61 | 13.21 | 3.49 | 0.02 | 15.1 | 25.50 | 28 | 999 |
| 175 | gpu-compute: bricks + beam + refloor + pathtrace (incorrect) | 234.6 | 0.60 | 10.63 | 3.63 | 0.09 | 32.2 | 7.12 | 1076 | 826 |
| 176 | gpu-compute: prefix + refloor + pathtrace (incorrect) | 229.1 | 0.58 | 13.27 | 3.02 | 0.08 | 32.2 | 7.12 | 1157 | 834 |
| 177 | gpu-compute: prefix + beam + refloor + pathtrace (incorrect) | 223.0 | 0.57 | 14.65 | 3.19 | 0.09 | 32.2 | 7.12 | 1157 | 830 |
| 178 | gpu-compute: hier + refloor + pathtrace (incorrect) | 222.7 | 0.57 | 14.31 | 3.12 | 0.09 | 32.2 | 7.12 | 1077 | 835 |
| 179 | gpu-compute: tree64 + refloor + pathtrace (incorrect) | 219.0 | 0.56 | 9.47 | 3.94 | 1.12 | 32.2 | 7.12 | 223 | 996 |
| 180 | gpu-compute: hier + beam + refloor + pathtrace (incorrect) | 214.3 | 0.55 | 14.85 | 3.29 | 0.09 | 32.2 | 7.12 | 1077 | 835 |
| 181 | gpu-compute: tree64 + beam + refloor + pathtrace (incorrect) | 202.7 | 0.52 | 9.02 | 4.40 | 1.12 | 32.2 | 7.12 | 223 | 992 |
| 182 | gpu-compute: rows + beam + refloor + pathtrace (incorrect) | 201.2 | 0.51 | 15.86 | 3.62 | 0.08 | 32.2 | 7.12 | 1140 | 834 |
| 183 | gpu-compute: rows + refloor + pathtrace (incorrect) | 200.3 | 0.51 | 16.28 | 3.51 | 0.09 | 32.2 | 7.12 | 1140 | 830 |
| 184 | gpu-compute: prefix + rowrun + pathtrace (incorrect) | 197.8 | 0.50 | 13.83 | 4.01 | 0.08 | 32.2 | 7.12 | 1157 | 830 |
| 185 | gpu-compute: svo_dag + refloor + pathtrace (incorrect) | 197.8 | 0.50 | 12.82 | 3.78 | 1.74 | 32.2 | 7.12 | 103 | 1061 |
| 186 | gpu-compute: prefix + beam + rowrun + pathtrace (incorrect) | 192.9 | 0.49 | 13.68 | 4.26 | 0.09 | 32.2 | 7.12 | 1157 | 830 |
| 187 | cpu-voxelspace: heightmap + columns (incorrect) | 188.0 | 0.48 | 8.88 | - | 4.78 | 15.1 | 25.50 | - | 653 |
| 188 | gpu-compute: svo_dag + beam + refloor + pathtrace (incorrect) | 186.2 | 0.47 | 11.63 | 4.30 | 1.73 | 32.2 | 7.12 | 103 | 1062 |
| 189 | gpu-compute: rows + rowrun + pathtrace (incorrect) | 172.9 | 0.44 | 16.29 | 4.70 | 0.09 | 32.2 | 7.12 | 1140 | 830 |
| 190 | gpu-compute: rows + beam + rowrun + pathtrace (incorrect) | 172.8 | 0.44 | 15.00 | 4.86 | 0.09 | 32.2 | 7.12 | 1140 | 830 |
| 191 | gpu-compute: clipmap + refloor + pathtrace (incorrect) | 172.2 | 0.44 | 12.52 | 5.40 | 0.03 | 32.2 | 7.12 | 1271 | 887 |
| 192 | gpu-compute: bricks + rowrun + pathtrace (incorrect) | 166.2 | 0.42 | 10.76 | 5.86 | 0.09 | 32.2 | 7.12 | 1076 | 826 |
| 193 | gpu-compute: bricks + beam + rowrun + pathtrace (incorrect) | 155.8 | 0.40 | 11.37 | 6.26 | 0.09 | 32.2 | 7.12 | 1076 | 826 |
| 194 | gpu-compute: clipmap + beam + refloor + pathtrace (incorrect) | 154.2 | 0.39 | 13.35 | 6.26 | 0.03 | 32.2 | 7.12 | 1271 | 887 |
| 195 | gpu-rt: boxes + rayquery + pathtrace (incorrect) | 153.6 | 0.39 | 18.15 | 4.69 | 1.92 | 32.2 | 7.12 | 1161 | 1349 |
| 196 | gpu-compute: tree64 + incremental + pathtrace (incorrect) | 144.1 | 0.37 | 9.34 | 6.75 | 1.14 | 32.2 | 7.12 | 223 | 997 |
| 197 | gpu-compute: hier + rowrun + pathtrace (incorrect) | 137.0 | 0.35 | 14.26 | 7.02 | 0.09 | 32.2 | 7.12 | 1077 | 835 |
| 198 | gpu-compute: tree64 + beam + incremental + pathtrace (incorrect) | 130.3 | 0.33 | 9.99 | 7.53 | 1.15 | 32.2 | 7.12 | 223 | 998 |
| 199 | gpu-compute: hier + beam + rowrun + pathtrace (incorrect) | 130.1 | 0.33 | 14.48 | 7.50 | 0.10 | 32.2 | 7.12 | 1077 | 835 |
| 200 | gpu-compute: clipmap + rowrun + pathtrace (incorrect) | 129.4 | 0.33 | 12.80 | 7.52 | 0.03 | 32.2 | 7.12 | 1271 | 887 |
| 201 | gpu-compute: clipmap + incremental + pathtrace (incorrect) | 128.8 | 0.33 | 12.28 | 7.59 | 0.03 | 32.2 | 7.12 | 1271 | 887 |
| 202 | gpu-compute: hier + sphere + pathtrace (incorrect) | 126.8 | 0.32 | 15.61 | 7.72 | 0.09 | 32.2 | 7.12 | 1077 | 836 |
| 203 | gpu-compute: hier + beam + sphere + pathtrace (incorrect) | 125.5 | 0.32 | 14.81 | 7.81 | 0.10 | 32.2 | 7.12 | 1077 | 835 |
| 204 | gpu-compute: clipmap + beam + rowrun + pathtrace (incorrect) | 118.8 | 0.30 | 13.96 | 8.22 | 0.03 | 32.2 | 7.12 | 1271 | 887 |
| 205 | gpu-compute: clipmap + beam + incremental + pathtrace (incorrect) | 116.5 | 0.30 | 12.48 | 8.40 | 0.03 | 32.2 | 7.12 | 1271 | 887 |
| 206 | gpu-compute: bricks + sphere + pathtrace (incorrect) | 114.2 | 0.29 | 11.16 | 8.67 | 0.10 | 32.2 | 7.12 | 1076 | 826 |
| 207 | gpu-compute: rows + sphere + pathtrace (incorrect) | 113.0 | 0.29 | 15.81 | 8.69 | 0.09 | 32.2 | 7.12 | 1140 | 830 |
| 208 | gpu-compute: prefix + sphere + pathtrace (incorrect) | 112.8 | 0.29 | 13.93 | 8.74 | 0.09 | 32.2 | 7.12 | 1157 | 830 |
| 209 | gpu-compute: rows + beam + sphere + pathtrace (incorrect) | 112.5 | 0.29 | 15.50 | 8.75 | 0.10 | 32.2 | 7.12 | 1140 | 830 |
| 210 | gpu-compute: bricks + beam + sphere + pathtrace (incorrect) | 112.3 | 0.29 | 12.25 | 8.81 | 0.10 | 32.2 | 7.12 | 1076 | 826 |
| 211 | gpu-compute: prefix + beam + sphere + pathtrace (incorrect) | 110.3 | 0.28 | 16.79 | 8.93 | 0.09 | 32.2 | 7.12 | 1157 | 830 |
| 212 | gpu-compute: svo_dag + sphere + pathtrace (incorrect) | 99.4 | 0.25 | 13.31 | 9.91 | 1.74 | 32.2 | 7.12 | 103 | 1060 |
| 213 | gpu-compute: svo_dag + beam + sphere + pathtrace (incorrect) | 95.4 | 0.24 | 13.62 | 10.35 | 1.74 | 32.2 | 7.12 | 103 | 1061 |
| 214 | gpu-compute: clipmap + sphere + pathtrace (incorrect) | 87.4 | 0.22 | 14.77 | 11.28 | 0.04 | 32.2 | 7.12 | 1271 | 887 |
| 215 | gpu-compute: svo_dag + rowrun + pathtrace (incorrect) | 87.4 | 0.22 | 14.52 | 11.32 | 1.76 | 32.2 | 7.12 | 103 | 1061 |
| 216 | gpu-compute: svo_dag + incremental + pathtrace (incorrect) | 87.4 | 0.22 | 14.63 | 11.33 | 1.77 | 32.2 | 7.12 | 103 | 1062 |
| 217 | gpu-compute: svo_dag + beam + rowrun + pathtrace (incorrect) | 84.7 | 0.22 | 14.87 | 11.70 | 1.78 | 32.2 | 7.12 | 103 | 1061 |
| 218 | gpu-compute: tree64 + sphere + pathtrace (incorrect) | 80.8 | 0.21 | 14.93 | 12.30 | 1.16 | 32.2 | 7.12 | 223 | 996 |
| 219 | gpu-compute: clipmap + beam + sphere + pathtrace (incorrect) | 80.8 | 0.21 | 15.56 | 12.22 | 0.04 | 32.2 | 7.12 | 1271 | 887 |
| 220 | gpu-compute: svo_dag + beam + incremental + pathtrace (incorrect) | 80.3 | 0.20 | 15.35 | 12.36 | 1.77 | 32.2 | 7.12 | 103 | 1062 |
| 221 | gpu-compute: tree64 + beam + sphere + pathtrace (incorrect) | 80.0 | 0.20 | 15.13 | 12.43 | 1.16 | 32.2 | 7.12 | 223 | 995 |
| 222 | gpu-compute: tree64 + beam + rowrun + pathtrace (incorrect) | 67.7 | 0.17 | 18.19 | 14.69 | 1.17 | 32.2 | 7.12 | 223 | 1007 |
| 223 | gpu-compute: tree64 + rowrun + pathtrace (incorrect) | 66.4 | 0.17 | 18.87 | 14.99 | 1.16 | 32.2 | 7.12 | 223 | 1001 |
| 224 | gpu-rt: greedy + rayquery + pathtrace (incorrect) | 62.9 | 0.16 | 136.30 | 2.04 | 11.86 | 32.2 | 7.12 | 801 | 2139 |
| 225 | gpu-compute: prefix + beam + incremental + pathtrace (incorrect) | 59.6 | 0.15 | 20.12 | 16.69 | 0.14 | 32.2 | 7.12 | 1157 | 830 |
| 226 | gpu-compute: hier + beam + incremental + pathtrace (incorrect) | 57.4 | 0.15 | 20.84 | 17.33 | 0.14 | 32.2 | 7.12 | 1077 | 835 |
| 227 | gpu-compute: prefix + incremental + pathtrace (incorrect) | 56.5 | 0.14 | 21.43 | 17.61 | 0.14 | 32.2 | 7.12 | 1157 | 830 |
| 228 | gpu-compute: bricks + beam + incremental + pathtrace (incorrect) | 55.7 | 0.14 | 21.18 | 17.87 | 0.16 | 32.2 | 7.12 | 1076 | 826 |
| 229 | gpu-compute: hier + incremental + pathtrace (incorrect) | 54.8 | 0.14 | 21.69 | 18.18 | 0.15 | 32.2 | 7.12 | 1077 | 835 |
| 230 | gpu-compute: rows + beam + incremental + pathtrace (incorrect) | 54.4 | 0.14 | 21.60 | 18.28 | 0.14 | 32.2 | 7.12 | 1140 | 830 |
| 231 | gpu-compute: bricks + incremental + pathtrace (incorrect) | 52.5 | 0.13 | 22.26 | 18.97 | 0.15 | 32.2 | 7.12 | 1076 | 826 |
| 232 | gpu-compute: rows + incremental + pathtrace (incorrect) | 52.4 | 0.13 | 23.01 | 18.98 | 0.15 | 32.2 | 7.12 | 1140 | 830 |
| 233 | gpu-compute: distance + sphere + pathtrace (incorrect) | 47.4 | 0.12 | 153.54 | 2.62 | 0.08 | 32.2 | 7.12 | 1588 | 835 |
| 234 | gpu-compute: distance + beam + sphere + pathtrace (incorrect) | 47.1 | 0.12 | 153.24 | 2.76 | 0.09 | 32.2 | 7.12 | 1588 | 835 |
| 235 | gpu-compute: distance + refloor + pathtrace (incorrect) | 45.7 | 0.12 | 160.63 | 3.34 | 0.09 | 32.2 | 7.12 | 1588 | 835 |
| 236 | gpu-compute: distance + beam + refloor + pathtrace (incorrect) | 45.7 | 0.12 | 151.40 | 3.57 | 0.09 | 32.2 | 7.12 | 1588 | 831 |
| 237 | gpu-compute: distance + beam + rowrun + pathtrace (incorrect) | 44.7 | 0.11 | 153.04 | 4.65 | 0.10 | 32.2 | 7.12 | 1588 | 831 |
| 238 | gpu-compute: distance + rowrun + pathtrace (incorrect) | 44.6 | 0.11 | 155.24 | 4.48 | 0.09 | 32.2 | 7.12 | 1588 | 835 |
| 239 | gpu-compute: distance + beam + incremental + pathtrace (incorrect) | 29.6 | 0.08 | 115.82 | 22.13 | 0.11 | 32.2 | 7.12 | 1588 | 831 |
| 240 | gpu-compute: distance + incremental + pathtrace (incorrect) | 28.8 | 0.07 | 115.14 | 23.56 | 0.10 | 32.2 | 7.12 | 1588 | 831 |
| 241 | gpu-splat: tree64 + points + splat (incorrect) | 26.2 | 0.07 | 189.58 | 2.99 | 1.17 | 23.5 | 9.69 | 613 | 1711 |
| 242 | gpu-splat: clipmap + points + splat (incorrect) | 25.7 | 0.07 | 233.93 | 3.06 | 0.03 | 23.5 | 9.69 | 1661 | 1592 |
| 243 | gpu-splat: svo_dag + points + splat (incorrect) | 25.7 | 0.07 | 236.20 | 2.99 | 1.84 | 23.5 | 9.69 | 493 | 1776 |
| 244 | gpu-splat: hier + points + splat (incorrect) | 25.7 | 0.07 | 241.79 | 2.87 | 0.09 | 23.5 | 9.69 | 1467 | 1539 |
| 245 | gpu-splat: bricks + points + splat (incorrect) | 25.5 | 0.06 | 232.39 | 2.84 | 0.10 | 23.5 | 9.69 | 1466 | 1535 |
| 246 | gpu-splat: prefix + points + splat (incorrect) | 25.5 | 0.06 | 241.39 | 2.86 | 0.09 | 23.5 | 9.69 | 1546 | 1540 |
| 247 | gpu-splat: rows + points + splat (incorrect) | 25.4 | 0.06 | 232.25 | 2.86 | 0.10 | 23.5 | 9.69 | 1530 | 1537 |
| 248 | cpu-discover: prefix + quadtree (incorrect) | 21.3 | 0.05 | 90.24 | 3.42 | 43.08 | 32.0 | 0.22 | 1053 | 1912 |
| 249 | cpu-discover: hier + quadtree (incorrect) | 21.3 | 0.05 | 90.93 | 3.40 | 43.64 | 32.0 | 0.22 | 1053 | 1491 |
| 250 | cpu-discover: morton + quadtree (incorrect) | 20.5 | 0.05 | 92.61 | 3.40 | 44.94 | 32.0 | 0.22 | 1053 | 2008 |
| 251 | cpu-discover: u8 + quadtree (incorrect) | 20.3 | 0.05 | 98.00 | 3.41 | 46.21 | 32.0 | 0.22 | 1053 | 1716 |
| 252 | cpu-discover: brickmajor + quadtree (incorrect) | 20.0 | 0.05 | 95.12 | 3.41 | 45.94 | 32.0 | 0.22 | 1053 | 2017 |
| 253 | cpu-discover: palette + quadtree (incorrect) | 20.0 | 0.05 | 89.79 | 3.41 | 44.84 | 32.0 | 0.22 | 1053 | 1480 |
| 254 | cpu-discover: bricks + quadtree (incorrect) | 19.8 | 0.05 | 101.89 | 3.40 | 47.50 | 32.0 | 0.22 | 1053 | 1439 |
| 255 | cpu-discover: bits + quadtree (incorrect) | 19.8 | 0.05 | 98.76 | 3.41 | 47.03 | 32.0 | 0.22 | 1053 | 1452 |
| 256 | gpu-splat: distance + points + splat (incorrect) | 17.9 | 0.05 | 378.92 | 2.85 | 0.09 | 23.5 | 9.69 | 1978 | 1543 |
| 257 | gpu-rt: naive + rayquery | FAILED: [gpu-rt: naive + rayquery]: worker error: wgpu error: Out of Memory | | | | | | | | |
| 258 | gpu-rt: naive + rayquery + pathtrace | FAILED: [gpu-rt: naive + rayquery + pathtrace]: worker error: wgpu error: Out of Memory | | | | | | | | |

### far [unlit]

1920×1080. camera 4.78 blocks/frame (max 6.1), turn 0.52°/frame; 9141 chunks at start, 35.3 loads/frame; chunk fill 8%, solid 5.8%, surface 0.44%; sky 69%, hits at 465 blocks (median 455); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: bricks + refloor | 428.4 | 1.00 | 13.11 | 0.88 | 0.08 | 57.7 | 0.02 | 1045 | 826 |
| 2 | gpu-compute: bricks + beam + refloor | 403.5 | 0.94 | 13.25 | 1.11 | 0.08 | 57.8 | 0.02 | 1045 | 826 |
| 3 | gpu-compute: tree64 + refloor | 397.1 | 0.93 | 12.24 | 0.95 | 1.11 | 57.7 | 0.02 | 191 | 993 |
| 4 | gpu-compute: bricks + rowrun | 370.8 | 0.87 | 12.54 | 1.38 | 0.08 | 57.7 | 0.02 | 1045 | 826 |
| 5 | gpu-compute: prefix + refloor | 354.6 | 0.83 | 19.49 | 0.77 | 0.08 | 57.7 | 0.02 | 1125 | 834 |
| 6 | gpu-compute: bricks + beam + rowrun | 344.0 | 0.80 | 12.48 | 1.60 | 0.09 | 57.8 | 0.02 | 1045 | 826 |
| 7 | gpu-compute: tree64 + beam + refloor | 340.1 | 0.79 | 12.55 | 1.57 | 1.10 | 57.8 | 0.02 | 191 | 1001 |
| 8 | gpu-compute: prefix + beam + refloor | 339.9 | 0.79 | 18.55 | 0.99 | 0.08 | 57.8 | 0.02 | 1125 | 830 |
| 9 | gpu-compute: tree64 + incremental | 337.7 | 0.79 | 12.78 | 1.58 | 1.11 | 57.7 | 0.02 | 191 | 995 |
| 10 | gpu-compute: hier + refloor | 337.5 | 0.79 | 20.10 | 0.80 | 0.08 | 57.7 | 0.02 | 1045 | 835 |
| 11 | gpu-compute: prefix + rowrun | 335.8 | 0.78 | 18.57 | 1.00 | 0.08 | 57.7 | 0.02 | 1125 | 835 |
| 12 | gpu-compute: clipmap + refloor | 332.7 | 0.78 | 15.67 | 1.44 | 0.02 | 57.7 | 0.02 | 1239 | 886 |
| 13 | gpu-compute: rows + refloor | 324.4 | 0.76 | 20.45 | 0.90 | 0.08 | 57.7 | 0.02 | 1109 | 830 |
| 14 | gpu-compute: hier + beam + refloor | 320.9 | 0.75 | 20.06 | 1.02 | 0.09 | 57.8 | 0.02 | 1045 | 835 |
| 15 | gpu-compute: svo_dag + refloor | 318.1 | 0.74 | 19.82 | 0.87 | 1.76 | 57.7 | 0.02 | 71 | 1061 |
| 16 | gpu-compute: bricks + sphere | 317.7 | 0.74 | 10.22 | 2.07 | 0.09 | 57.7 | 0.02 | 1045 | 826 |
| 17 | gpu-compute: prefix + beam + rowrun | 317.2 | 0.74 | 18.51 | 1.22 | 0.08 | 57.8 | 0.02 | 1125 | 830 |
| 18 | gpu-compute: clipmap + rowrun | 309.5 | 0.72 | 14.88 | 1.81 | 0.02 | 57.7 | 0.02 | 1239 | 887 |
| 19 | gpu-compute: rows + beam + refloor | 308.9 | 0.72 | 20.99 | 1.12 | 0.08 | 57.8 | 0.02 | 1109 | 830 |
| 20 | gpu-compute: rows + rowrun | 306.9 | 0.72 | 20.43 | 1.05 | 0.08 | 57.7 | 0.02 | 1109 | 830 |
| 21 | gpu-compute: bricks + beam + sphere | 300.4 | 0.70 | 11.31 | 2.28 | 0.09 | 57.8 | 0.02 | 1045 | 826 |
| 22 | gpu-compute: clipmap + incremental | 299.3 | 0.70 | 13.39 | 1.88 | 0.02 | 57.7 | 0.02 | 1239 | 887 |
| 23 | gpu-compute: rows + beam + rowrun | 293.8 | 0.69 | 20.41 | 1.29 | 0.08 | 57.8 | 0.02 | 1109 | 834 |
| 24 | gpu-compute: tree64 + beam + incremental | 293.8 | 0.69 | 12.43 | 2.20 | 1.11 | 57.8 | 0.02 | 191 | 996 |
| 25 | gpu-compute: svo_dag + beam + refloor | 292.1 | 0.68 | 17.57 | 1.51 | 1.68 | 57.8 | 0.02 | 71 | 1061 |
| 26 | gpu-compute: clipmap + beam + refloor | 279.7 | 0.65 | 11.64 | 2.28 | 0.02 | 57.8 | 0.02 | 1239 | 887 |
| 27 | gpu-discover: bricks + quadtree | 278.8 | 0.65 | 9.18 | 2.51 | 0.14 | 50.2 | 0.02 | 1227 | 832 |
| 28 | gpu-compute: hier + sphere | 274.9 | 0.64 | 18.62 | 1.86 | 0.08 | 57.7 | 0.02 | 1045 | 835 |
| 29 | gpu-compute: hier + rowrun | 274.0 | 0.64 | 18.12 | 1.82 | 0.09 | 57.7 | 0.02 | 1045 | 838 |
| 30 | gpu-compute: prefix + sphere | 269.6 | 0.63 | 15.08 | 2.06 | 0.08 | 57.7 | 0.02 | 1125 | 830 |
| 31 | gpu-compute: hier + beam + sphere | 266.7 | 0.62 | 14.78 | 2.08 | 0.08 | 57.8 | 0.02 | 1045 | 835 |
| 32 | gpu-compute: hier + beam + rowrun | 265.2 | 0.62 | 16.29 | 2.09 | 0.08 | 57.8 | 0.02 | 1045 | 836 |
| 33 | gpu-compute: prefix + beam + sphere | 257.4 | 0.60 | 14.15 | 2.28 | 0.08 | 57.8 | 0.02 | 1125 | 830 |
| 34 | gpu-compute: rows + sphere | 257.3 | 0.60 | 16.08 | 2.06 | 0.08 | 57.7 | 0.02 | 1109 | 830 |
| 35 | gpu-compute: tree64 + sphere | 256.1 | 0.60 | 11.51 | 2.93 | 1.12 | 57.7 | 0.02 | 191 | 996 |
| 36 | gpu-discover: tree64 + quadtree | 255.7 | 0.60 | 11.52 | 2.78 | 1.17 | 50.2 | 0.02 | 373 | 998 |
| 37 | gpu-compute: clipmap + beam + incremental | 253.7 | 0.59 | 12.42 | 2.76 | 0.02 | 57.8 | 0.02 | 1239 | 887 |
| 38 | gpu-compute: clipmap + sphere | 253.4 | 0.59 | 10.36 | 2.90 | 0.02 | 57.7 | 0.02 | 1239 | 886 |
| 39 | gpu-compute: clipmap + beam + rowrun | 252.8 | 0.59 | 11.53 | 2.67 | 0.02 | 57.8 | 0.02 | 1239 | 887 |
| 40 | gpu-compute: svo_dag + sphere | 249.9 | 0.58 | 17.28 | 2.31 | 1.71 | 57.7 | 0.02 | 71 | 1062 |
| 41 | gpu-compute: rows + beam + sphere | 247.0 | 0.58 | 15.56 | 2.28 | 0.08 | 57.8 | 0.02 | 1109 | 830 |
| 42 | gpu-discover: prefix + quadtree | 244.0 | 0.57 | 14.85 | 2.42 | 0.15 | 50.2 | 0.02 | 1307 | 838 |
| 43 | gpu-discover: hier + quadtree | 243.2 | 0.57 | 14.63 | 2.45 | 0.14 | 50.2 | 0.02 | 1227 | 838 |
| 44 | gpu-discover: clipmap + quadtree | 239.2 | 0.56 | 11.91 | 3.00 | 0.09 | 50.2 | 0.02 | 1421 | 893 |
| 45 | gpu-compute: tree64 + beam + sphere | 232.5 | 0.54 | 10.31 | 3.53 | 1.11 | 57.8 | 0.02 | 191 | 992 |
| 46 | gpu-discover: rows + quadtree | 229.7 | 0.54 | 15.70 | 2.52 | 0.14 | 50.2 | 0.02 | 1291 | 833 |
| 47 | gpu-compute: svo_dag + incremental | 229.2 | 0.54 | 13.96 | 2.79 | 1.73 | 57.7 | 0.02 | 71 | 1061 |
| 48 | gpu-compute: svo_dag + beam + sphere | 226.1 | 0.53 | 12.98 | 2.93 | 1.70 | 57.8 | 0.02 | 71 | 1061 |
| 49 | gpu-rt: boxes + rayquery | 225.4 | 0.53 | 23.63 | 1.15 | 1.69 | 57.7 | 0.02 | 1130 | 1349 |
| 50 | gpu-discover: svo_dag + quadtree | 224.8 | 0.52 | 14.47 | 2.79 | 1.77 | 50.2 | 0.02 | 253 | 1067 |
| 51 | gpu-compute: svo_dag + rowrun | 218.6 | 0.51 | 16.77 | 3.09 | 1.69 | 57.7 | 0.02 | 71 | 1060 |
| 52 | gpu-compute: clipmap + beam + sphere | 214.7 | 0.50 | 12.14 | 3.76 | 0.02 | 57.8 | 0.02 | 1239 | 887 |
| 53 | gpu-compute: tree64 + rowrun | 213.9 | 0.50 | 10.60 | 3.93 | 1.11 | 57.7 | 0.02 | 191 | 997 |
| 54 | gpu-compute: svo_dag + beam + incremental | 208.6 | 0.49 | 11.78 | 3.47 | 1.70 | 57.8 | 0.02 | 71 | 1061 |
| 55 | gpu-compute: svo_dag + beam + rowrun | 199.7 | 0.47 | 12.64 | 3.75 | 1.71 | 57.8 | 0.02 | 71 | 1062 |
| 56 | gpu-raster: bricks + naive + raster | 196.5 | 0.46 | 25.06 | 1.45 | 2.44 | 53.2 | 0.02 | 1310 | 1056 |
| 57 | gpu-raster: bricks + naive + visbuffer | 194.7 | 0.45 | 24.13 | 1.51 | 2.44 | 53.2 | 0.02 | 1310 | 1056 |
| 58 | gpu-raster: bricks + naive + gbuffer | 194.1 | 0.45 | 24.94 | 1.50 | 2.45 | 53.1 | 0.02 | 1310 | 1056 |
| 59 | gpu-compute: tree64 + beam + rowrun | 193.4 | 0.45 | 9.80 | 4.61 | 1.13 | 57.8 | 0.02 | 191 | 996 |
| 60 | gpu-compute: prefix + incremental | 175.3 | 0.41 | 13.03 | 4.96 | 0.09 | 57.7 | 0.02 | 1125 | 830 |
| 61 | gpu-compute: bricks + incremental | 173.1 | 0.40 | 11.75 | 5.59 | 0.09 | 57.7 | 0.02 | 1045 | 826 |
| 62 | gpu-compute: bricks + beam + incremental | 169.5 | 0.40 | 11.04 | 5.74 | 0.09 | 57.8 | 0.02 | 1045 | 826 |
| 63 | gpu-compute: prefix + beam + incremental | 169.4 | 0.40 | 14.75 | 5.18 | 0.09 | 57.8 | 0.02 | 1125 | 830 |
| 64 | gpu-compute: hier + incremental | 167.1 | 0.39 | 14.19 | 5.13 | 0.09 | 57.7 | 0.02 | 1045 | 835 |
| 65 | gpu-compute: hier + beam + incremental | 166.5 | 0.39 | 13.93 | 5.41 | 0.09 | 57.8 | 0.02 | 1045 | 835 |
| 66 | gpu-compute: rows + incremental | 157.8 | 0.37 | 14.81 | 5.59 | 0.09 | 57.7 | 0.02 | 1109 | 830 |
| 67 | gpu-compute: rows + beam + incremental | 150.7 | 0.35 | 15.39 | 6.09 | 0.09 | 57.8 | 0.02 | 1109 | 830 |
| 68 | gpu-raster: bricks + greedy + raster | 131.8 | 0.31 | 42.46 | 0.57 | 5.49 | 51.8 | 0.03 | 1182 | 945 |
| 69 | gpu-raster: bricks + greedy + gbuffer | 131.5 | 0.31 | 41.62 | 0.63 | 5.49 | 51.7 | 0.03 | 1182 | 946 |
| 70 | gpu-raster: bricks + greedy + visbuffer | 129.4 | 0.30 | 42.46 | 0.63 | 5.54 | 51.8 | 0.03 | 1182 | 945 |
| 71 | gpu-raster: bricks + cubes + raster | 80.4 | 0.19 | 15.05 | 12.26 | 1.73 | 53.2 | 0.02 | 1310 | 987 |
| 72 | gpu-raster: bricks + cubes + visbuffer | 80.1 | 0.19 | 14.87 | 12.32 | 1.76 | 53.1 | 0.02 | 1310 | 987 |
| 73 | gpu-raster: bricks + cubes + gbuffer | 80.0 | 0.19 | 15.05 | 12.32 | 1.74 | 53.1 | 0.02 | 1310 | 988 |
| 74 | gpu-rt: greedy + rayquery | 65.4 | 0.15 | 132.54 | 1.53 | 11.11 | 57.6 | 0.02 | 770 | 2076 |
| 75 | gpu-compute: distance + beam + refloor | 49.3 | 0.12 | 199.49 | 1.06 | 0.08 | 57.8 | 0.02 | 1557 | 835 |
| 76 | gpu-compute: distance + beam + sphere | 48.9 | 0.11 | 199.57 | 1.01 | 0.08 | 57.8 | 0.02 | 1557 | 835 |
| 77 | gpu-compute: distance + refloor | 48.2 | 0.11 | 204.04 | 0.91 | 0.08 | 57.7 | 0.02 | 1557 | 835 |
| 78 | gpu-compute: distance + rowrun | 47.6 | 0.11 | 204.68 | 1.20 | 0.08 | 57.7 | 0.02 | 1557 | 835 |
| 79 | gpu-compute: distance + beam + rowrun | 47.4 | 0.11 | 201.10 | 1.50 | 0.09 | 57.8 | 0.02 | 1557 | 835 |
| 80 | gpu-discover: distance + quadtree | 47.2 | 0.11 | 139.44 | 2.61 | 0.15 | 50.2 | 0.02 | 1739 | 834 |
| 81 | gpu-compute: distance + sphere | 46.1 | 0.11 | 213.14 | 0.98 | 0.09 | 57.7 | 0.02 | 1557 | 835 |
| 82 | cpu: hier + refloor | 41.2 | 0.10 | 34.06 | - | 23.03 | 61.7 | 0.01 | - | 667 |
| 83 | gpu-compute: distance + incremental | 39.6 | 0.09 | 122.20 | 7.88 | 0.10 | 57.7 | 0.02 | 1557 | 831 |
| 84 | gpu-compute: distance + beam + incremental | 39.4 | 0.09 | 121.05 | 8.22 | 0.10 | 57.8 | 0.02 | 1557 | 831 |
| 85 | cpu: prefix + incremental | 38.9 | 0.09 | 35.24 | - | 24.37 | 61.7 | 0.01 | - | 883 |
| 86 | cpu: hier + incremental | 38.9 | 0.09 | 34.45 | - | 24.52 | 61.7 | 0.01 | - | 667 |
| 87 | cpu: bricks + refloor | 36.8 | 0.09 | 34.45 | - | 26.41 | 61.7 | 0.01 | - | 632 |
| 88 | cpu: prefix + refloor | 36.0 | 0.08 | 42.34 | - | 26.46 | 61.7 | 0.01 | - | 885 |
| 89 | cpu: u8 + refloor | 35.7 | 0.08 | 36.36 | - | 27.12 | 61.7 | 0.01 | - | 540 |
| 90 | cpu: bricks + incremental | 35.4 | 0.08 | 35.31 | - | 27.49 | 61.7 | 0.01 | - | 631 |
| 91 | cpu: morton + refloor | 35.2 | 0.08 | 39.27 | - | 26.88 | 61.7 | 0.01 | - | 965 |
| 92 | cpu: bits + refloor | 35.2 | 0.08 | 37.55 | - | 27.15 | 61.7 | 0.01 | - | 667 |
| 93 | cpu: brickmajor + refloor | 35.1 | 0.08 | 39.61 | - | 26.90 | 61.7 | 0.01 | - | 970 |
| 94 | cpu: bits + incremental | 34.7 | 0.08 | 37.93 | - | 27.60 | 61.7 | 0.01 | - | 667 |
| 95 | cpu: u8 + incremental | 34.7 | 0.08 | 36.70 | - | 27.89 | 61.7 | 0.01 | - | 538 |
| 96 | cpu: palette + refloor | 34.0 | 0.08 | 45.34 | - | 26.95 | 61.7 | 0.01 | - | 532 |
| 97 | cpu: morton + incremental | 34.0 | 0.08 | 39.46 | - | 27.98 | 61.7 | 0.01 | - | 965 |
| 98 | cpu: brickmajor + incremental | 33.7 | 0.08 | 39.07 | - | 28.16 | 61.7 | 0.01 | - | 972 |
| 99 | cpu: palette + incremental | 33.2 | 0.08 | 44.24 | - | 27.77 | 61.7 | 0.01 | - | 525 |
| 100 | cpu-raster: greedy + raster | 15.9 | 0.04 | 88.85 | - | 60.18 | 66.7 | 0.00 | - | 1024 |
| 101 | cpu-raster: naive + raster | 8.0 | 0.02 | 143.32 | - | 118.66 | 68.1 | 0.00 | - | 1612 |
| 102 | cpu-discover: hier + cluster | 5.7 | 0.01 | 423.46 | 0.27 | 172.12 | 37.1 | 0.13 | 1053 | 1430 |
| 103 | cpu-discover: prefix + cluster | 5.7 | 0.01 | 418.99 | 0.27 | 171.95 | 37.1 | 0.13 | 1053 | 1913 |
| 104 | cpu-discover: morton + cluster | 5.5 | 0.01 | 437.20 | 0.27 | 179.44 | 37.1 | 0.13 | 1053 | 2020 |
| 105 | cpu-discover: u8 + cluster | 5.4 | 0.01 | 437.80 | 0.27 | 181.71 | 37.1 | 0.13 | 1053 | 1692 |
| 106 | cpu-discover: brickmajor + cluster | 5.4 | 0.01 | 445.57 | 0.27 | 182.12 | 37.1 | 0.13 | 1053 | 2043 |
| 107 | cpu-discover: palette + cluster | 5.4 | 0.01 | 444.14 | 0.27 | 181.48 | 37.1 | 0.13 | 1053 | 1500 |
| 108 | cpu-discover: bits + cluster | 5.3 | 0.01 | 465.45 | 0.27 | 185.71 | 37.1 | 0.13 | 1053 | 1466 |
| 109 | cpu-discover: bricks + cluster | 5.3 | 0.01 | 458.57 | 0.27 | 186.73 | 37.1 | 0.13 | 1053 | 1430 |
| 110 | gpu-voxelspace: heightmap + columns (incorrect) | 321.2 | 0.75 | 12.73 | 1.89 | 0.02 | 18.3 | 21.45 | 28 | 999 |
| 111 | cpu-voxelspace: heightmap + columns (incorrect) | 266.9 | 0.62 | 7.44 | - | 3.13 | 18.3 | 21.45 | - | 648 |
| 112 | gpu-splat: bricks + points + splat (incorrect) | 26.0 | 0.06 | 285.89 | 2.57 | 0.10 | 26.6 | 8.67 | 1466 | 1536 |
| 113 | cpu-discover: prefix + quadtree (incorrect) | 21.3 | 0.05 | 96.81 | 0.26 | 43.15 | 33.9 | 0.22 | 1053 | 1905 |
| 114 | cpu-discover: hier + quadtree (incorrect) | 21.0 | 0.05 | 95.47 | 0.26 | 44.16 | 33.9 | 0.22 | 1053 | 1433 |
| 115 | cpu-discover: bricks + quadtree (incorrect) | 20.4 | 0.05 | 99.55 | 0.26 | 46.25 | 33.9 | 0.22 | 1053 | 1417 |
| 116 | cpu-discover: bits + quadtree (incorrect) | 20.3 | 0.05 | 96.19 | 0.26 | 45.81 | 33.9 | 0.22 | 1053 | 1451 |
| 117 | cpu-discover: morton + quadtree (incorrect) | 20.3 | 0.05 | 99.79 | 0.26 | 45.63 | 33.9 | 0.22 | 1053 | 1982 |
| 118 | cpu-discover: u8 + quadtree (incorrect) | 20.2 | 0.05 | 100.56 | 0.26 | 46.48 | 33.9 | 0.22 | 1053 | 1721 |
| 119 | cpu-discover: brickmajor + quadtree (incorrect) | 20.2 | 0.05 | 103.24 | 0.26 | 45.53 | 33.9 | 0.22 | 1053 | 1948 |
| 120 | cpu-discover: palette + quadtree (incorrect) | 19.7 | 0.05 | 100.56 | 0.26 | 45.55 | 33.9 | 0.22 | 1053 | 1492 |
| 121 | gpu-rt: naive + rayquery | FAILED: [gpu-rt: naive + rayquery]: worker error: wgpu error: Out of Memory | | | | | | | | |

### many-lights [lit]

1920×1080. camera 1.42 blocks/frame (max 1.4), turn 0.53°/frame; 2209 chunks at start, 6.0 loads/frame; chunk fill 15%, solid 11.6%, surface 0.87%; sky 31%, hits at 110 blocks (median 81); 0.0 edits/frame; 4000 lights (animated); 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-rt: greedy + rayquery | 228.7 | 1.00 | 50.04 | 2.35 | 3.44 | 60.3 | 0.01 | 218 | 845 |
| 2 | gpu-raster: prefix + naive + visbuffer | 214.8 | 0.94 | 19.50 | 3.98 | 2.53 | 55.0 | 0.02 | 367 | 459 |
| 3 | gpu-raster: prefix + naive + gbuffer | 208.3 | 0.91 | 19.14 | 4.15 | 2.52 | 55.0 | 0.02 | 367 | 460 |
| 4 | gpu-raster: bricks + naive + gbuffer | 206.9 | 0.90 | 18.24 | 4.27 | 2.61 | 55.0 | 0.02 | 347 | 461 |
| 5 | gpu-compute: bricks + refloor | 206.3 | 0.90 | 9.25 | 4.68 | 2.23 | 60.3 | 0.01 | 274 | 391 |
| 6 | gpu-compute: prefix + refloor | 205.3 | 0.90 | 11.63 | 4.59 | 2.25 | 60.4 | 0.01 | 294 | 398 |
| 7 | gpu-raster: bricks + naive + visbuffer | 204.4 | 0.89 | 19.05 | 4.34 | 2.59 | 55.0 | 0.02 | 347 | 459 |
| 8 | gpu-raster: rows + naive + visbuffer | 203.7 | 0.89 | 20.23 | 4.23 | 2.59 | 55.0 | 0.02 | 363 | 459 |
| 9 | gpu-raster: prefix + greedy + visbuffer | 203.4 | 0.89 | 35.86 | 3.77 | 2.98 | 53.4 | 0.02 | 335 | 419 |
| 10 | gpu-compute: prefix + beam + refloor | 202.6 | 0.89 | 11.90 | 4.67 | 2.26 | 60.4 | 0.01 | 294 | 392 |
| 11 | gpu-rt: naive + rayquery | 202.0 | 0.88 | 52.96 | 2.66 | 3.58 | 60.3 | 0.01 | 564 | 1010 |
| 12 | gpu-raster: bricks + greedy + visbuffer | 198.7 | 0.87 | 33.72 | 4.00 | 3.05 | 53.4 | 0.02 | 315 | 420 |
| 13 | gpu-compute: rows + refloor | 198.5 | 0.87 | 12.73 | 4.75 | 2.26 | 60.3 | 0.01 | 290 | 391 |
| 14 | gpu-raster: prefix + greedy + gbuffer | 198.1 | 0.87 | 34.32 | 3.91 | 3.03 | 53.3 | 0.02 | 335 | 420 |
| 15 | gpu-raster: clipmap + naive + gbuffer | 194.4 | 0.85 | 19.22 | 4.57 | 2.63 | 55.0 | 0.02 | 408 | 486 |
| 16 | gpu-raster: clipmap + naive + visbuffer | 194.4 | 0.85 | 20.12 | 4.55 | 2.62 | 55.0 | 0.02 | 408 | 486 |
| 17 | gpu-compute: bricks + beam + refloor | 193.7 | 0.85 | 9.60 | 4.99 | 2.26 | 60.4 | 0.01 | 274 | 391 |
| 18 | gpu-compute: hier + refloor | 193.4 | 0.85 | 12.43 | 4.92 | 2.31 | 60.3 | 0.01 | 274 | 392 |
| 19 | gpu-raster: rows + greedy + visbuffer | 192.8 | 0.84 | 35.95 | 4.02 | 3.00 | 53.4 | 0.02 | 331 | 419 |
| 20 | gpu-raster: bricks + greedy + gbuffer | 192.4 | 0.84 | 33.61 | 4.14 | 3.08 | 53.3 | 0.02 | 315 | 421 |
| 21 | gpu-raster: hier + naive + visbuffer | 192.1 | 0.84 | 19.25 | 4.57 | 2.63 | 55.0 | 0.02 | 347 | 460 |
| 22 | gpu-raster: rows + naive + gbuffer | 190.6 | 0.83 | 20.68 | 4.57 | 2.65 | 55.0 | 0.02 | 363 | 462 |
| 23 | gpu-raster: hier + naive + gbuffer | 189.4 | 0.83 | 19.45 | 4.65 | 2.66 | 55.0 | 0.02 | 347 | 461 |
| 24 | gpu-compute: rows + beam + refloor | 188.7 | 0.83 | 13.49 | 5.00 | 2.28 | 60.4 | 0.01 | 290 | 392 |
| 25 | gpu-raster: clipmap + greedy + visbuffer | 187.8 | 0.82 | 35.00 | 4.22 | 3.08 | 53.4 | 0.02 | 376 | 444 |
| 26 | gpu-raster: rows + greedy + gbuffer | 187.8 | 0.82 | 35.23 | 4.17 | 3.01 | 53.3 | 0.02 | 331 | 419 |
| 27 | gpu-raster: hier + greedy + visbuffer | 183.7 | 0.80 | 35.74 | 4.35 | 3.07 | 53.4 | 0.02 | 315 | 421 |
| 28 | gpu-raster: clipmap + greedy + gbuffer | 182.5 | 0.80 | 34.54 | 4.41 | 3.10 | 53.3 | 0.02 | 376 | 445 |
| 29 | gpu-compute: hier + beam + refloor | 181.7 | 0.79 | 12.64 | 5.25 | 2.26 | 60.4 | 0.01 | 274 | 393 |
| 30 | gpu-raster: hier + greedy + gbuffer | 177.7 | 0.78 | 34.98 | 4.53 | 3.13 | 53.3 | 0.02 | 315 | 424 |
| 31 | gpu-raster: svo_dag + naive + visbuffer | 176.4 | 0.77 | 21.41 | 5.05 | 2.93 | 55.0 | 0.02 | 102 | 495 |
| 32 | gpu-raster: svo_dag + naive + gbuffer | 173.5 | 0.76 | 21.23 | 5.14 | 2.94 | 55.0 | 0.02 | 102 | 493 |
| 33 | gpu-compute: clipmap + refloor | 173.4 | 0.76 | 11.29 | 5.60 | 2.26 | 60.3 | 0.01 | 335 | 405 |
| 34 | gpu-compute: svo_dag + refloor | 172.4 | 0.75 | 11.26 | 5.54 | 2.54 | 60.4 | 0.01 | 29 | 448 |
| 35 | gpu-raster: svo_dag + greedy + visbuffer | 171.5 | 0.75 | 37.26 | 4.74 | 3.36 | 53.4 | 0.02 | 70 | 456 |
| 36 | gpu-compute: clipmap + beam + refloor | 166.9 | 0.73 | 11.08 | 5.84 | 2.38 | 60.4 | 0.01 | 335 | 411 |
| 37 | gpu-raster: svo_dag + greedy + gbuffer | 166.8 | 0.73 | 37.58 | 4.91 | 3.35 | 53.3 | 0.02 | 70 | 456 |
| 38 | gpu-compute: svo_dag + beam + refloor | 166.6 | 0.73 | 10.84 | 5.75 | 2.58 | 60.4 | 0.01 | 29 | 448 |
| 39 | gpu-raster: tree64 + naive + visbuffer | 163.4 | 0.71 | 20.68 | 5.61 | 2.88 | 55.0 | 0.02 | 139 | 470 |
| 40 | gpu-raster: tree64 + greedy + visbuffer | 159.0 | 0.70 | 36.44 | 5.30 | 3.32 | 53.4 | 0.02 | 107 | 453 |
| 41 | gpu-raster: tree64 + naive + gbuffer | 158.7 | 0.69 | 20.35 | 5.81 | 2.86 | 55.0 | 0.02 | 139 | 473 |
| 42 | gpu-compute: rows + rowrun | 158.6 | 0.69 | 13.31 | 6.11 | 2.32 | 60.3 | 0.01 | 290 | 391 |
| 43 | gpu-raster: tree64 + greedy + gbuffer | 153.1 | 0.67 | 36.44 | 5.54 | 3.33 | 53.3 | 0.02 | 107 | 451 |
| 44 | gpu-raster: prefix + greedy + raster | 151.7 | 0.66 | 35.90 | 5.57 | 3.21 | 53.4 | 0.02 | 335 | 420 |
| 45 | gpu-compute: rows + beam + rowrun | 151.1 | 0.66 | 13.36 | 6.44 | 2.33 | 60.4 | 0.01 | 290 | 391 |
| 46 | gpu-raster: bricks + greedy + raster | 149.7 | 0.65 | 29.54 | 5.76 | 3.26 | 53.4 | 0.02 | 315 | 423 |
| 47 | gpu-compute: tree64 + refloor | 148.7 | 0.65 | 10.61 | 6.58 | 2.52 | 60.3 | 0.01 | 66 | 448 |
| 48 | gpu-raster: rows + greedy + raster | 145.1 | 0.63 | 36.36 | 5.87 | 3.12 | 53.4 | 0.02 | 331 | 421 |
| 49 | gpu-compute: tree64 + beam + refloor | 141.3 | 0.62 | 10.85 | 6.95 | 2.50 | 60.3 | 0.01 | 66 | 449 |
| 50 | gpu-compute: prefix + rowrun | 139.9 | 0.61 | 12.26 | 7.04 | 2.37 | 60.4 | 0.01 | 294 | 398 |
| 51 | gpu-compute: prefix + beam + rowrun | 137.5 | 0.60 | 12.16 | 7.17 | 2.34 | 60.4 | 0.01 | 294 | 391 |
| 52 | gpu-compute: distance + sphere | 137.1 | 0.60 | 106.74 | 4.11 | 2.20 | 60.3 | 0.01 | 402 | 391 |
| 53 | gpu-compute: distance + beam + sphere | 136.4 | 0.60 | 107.23 | 4.17 | 2.19 | 60.4 | 0.01 | 402 | 392 |
| 54 | gpu-compute: clipmap + incremental | 135.1 | 0.59 | 11.97 | 7.31 | 2.30 | 60.4 | 0.01 | 335 | 405 |
| 55 | gpu-raster: hier + greedy + raster | 135.0 | 0.59 | 36.03 | 6.44 | 3.23 | 53.4 | 0.02 | 315 | 422 |
| 56 | gpu-compute: distance + refloor | 134.1 | 0.59 | 108.10 | 4.26 | 2.24 | 60.3 | 0.01 | 402 | 392 |
| 57 | gpu-raster: distance + naive + visbuffer | 133.0 | 0.58 | 109.27 | 3.98 | 2.54 | 55.0 | 0.02 | 475 | 459 |
| 58 | gpu-compute: clipmap + beam + incremental | 132.2 | 0.58 | 12.02 | 7.46 | 2.33 | 60.4 | 0.01 | 335 | 405 |
| 59 | gpu-raster: prefix + cubes + visbuffer | 131.6 | 0.58 | 13.04 | 7.32 | 2.61 | 54.9 | 0.02 | 367 | 440 |
| 60 | gpu-raster: distance + naive + gbuffer | 131.3 | 0.57 | 109.85 | 4.05 | 2.57 | 55.0 | 0.02 | 475 | 458 |
| 61 | gpu-raster: clipmap + greedy + raster | 131.2 | 0.57 | 29.08 | 6.72 | 3.22 | 53.4 | 0.02 | 376 | 443 |
| 62 | gpu-raster: prefix + cubes + gbuffer | 129.9 | 0.57 | 12.89 | 7.42 | 2.56 | 54.9 | 0.02 | 367 | 440 |
| 63 | gpu-compute: rows + beam + sphere | 129.8 | 0.57 | 13.15 | 7.61 | 2.37 | 60.4 | 0.01 | 290 | 391 |
| 64 | gpu-compute: distance + beam + refloor | 129.7 | 0.57 | 108.78 | 4.57 | 2.22 | 60.4 | 0.01 | 402 | 391 |
| 65 | gpu-compute: bricks + sphere | 129.7 | 0.57 | 11.73 | 7.64 | 2.35 | 60.3 | 0.01 | 274 | 392 |
| 66 | gpu-compute: bricks + beam + sphere | 129.6 | 0.57 | 11.70 | 7.65 | 2.36 | 60.4 | 0.01 | 274 | 392 |
| 67 | gpu-raster: bricks + cubes + visbuffer | 129.5 | 0.57 | 11.66 | 7.54 | 2.62 | 54.9 | 0.02 | 347 | 442 |
| 68 | gpu-compute: rows + sphere | 129.1 | 0.56 | 13.34 | 7.65 | 2.34 | 60.3 | 0.01 | 290 | 391 |
| 69 | gpu-raster: prefix + naive + raster | 129.0 | 0.56 | 13.82 | 7.33 | 2.75 | 55.0 | 0.02 | 367 | 459 |
| 70 | gpu-raster: bricks + cubes + gbuffer | 128.2 | 0.56 | 11.39 | 7.61 | 2.54 | 54.9 | 0.02 | 347 | 441 |
| 71 | gpu-raster: distance + greedy + visbuffer | 128.1 | 0.56 | 108.70 | 3.77 | 3.03 | 53.4 | 0.02 | 443 | 421 |
| 72 | gpu-compute: hier + beam + sphere | 127.8 | 0.56 | 12.43 | 7.75 | 2.33 | 60.4 | 0.01 | 274 | 393 |
| 73 | gpu-raster: rows + cubes + visbuffer | 127.4 | 0.56 | 13.87 | 7.55 | 2.60 | 54.9 | 0.02 | 363 | 440 |
| 74 | gpu-compute: hier + sphere | 127.3 | 0.56 | 12.61 | 7.78 | 2.34 | 60.3 | 0.01 | 274 | 393 |
| 75 | gpu-raster: bricks + naive + raster | 126.5 | 0.55 | 13.57 | 7.60 | 2.75 | 55.0 | 0.02 | 347 | 459 |
| 76 | gpu-raster: rows + cubes + gbuffer | 126.0 | 0.55 | 14.47 | 7.66 | 2.59 | 54.9 | 0.02 | 363 | 440 |
| 77 | gpu-raster: clipmap + cubes + visbuffer | 125.8 | 0.55 | 12.03 | 7.73 | 2.62 | 54.9 | 0.02 | 408 | 467 |
| 78 | gpu-raster: distance + greedy + gbuffer | 125.4 | 0.55 | 107.77 | 3.91 | 3.00 | 53.3 | 0.02 | 443 | 421 |
| 79 | gpu-raster: clipmap + cubes + gbuffer | 123.1 | 0.54 | 12.12 | 7.91 | 2.63 | 54.9 | 0.02 | 408 | 465 |
| 80 | gpu-raster: hier + cubes + visbuffer | 122.9 | 0.54 | 13.63 | 7.87 | 2.59 | 54.9 | 0.02 | 347 | 442 |
| 81 | gpu-raster: rows + naive + raster | 122.2 | 0.53 | 14.22 | 7.77 | 2.78 | 55.0 | 0.02 | 363 | 459 |
| 82 | gpu-raster: hier + cubes + gbuffer | 121.0 | 0.53 | 13.01 | 8.00 | 2.59 | 54.9 | 0.02 | 347 | 442 |
| 83 | gpu-rt: boxes + rayquery | 120.8 | 0.53 | 11.87 | 8.15 | 2.68 | 60.3 | 0.01 | 291 | 631 |
| 84 | gpu-rt: greedy + rayquery + pathtrace | 119.2 | 0.52 | 49.57 | 7.03 | 3.83 | 35.6 | 2.54 | 249 | 837 |
| 85 | gpu-raster: svo_dag + greedy + raster | 116.6 | 0.51 | 37.15 | 7.59 | 3.49 | 53.4 | 0.02 | 70 | 456 |
| 86 | gpu-raster: svo_dag + cubes + visbuffer | 116.4 | 0.51 | 16.79 | 8.24 | 2.80 | 54.9 | 0.02 | 102 | 474 |
| 87 | gpu-raster: tree64 + greedy + raster | 115.7 | 0.51 | 36.49 | 7.76 | 3.37 | 53.4 | 0.02 | 107 | 454 |
| 88 | gpu-compute: prefix + sphere | 115.0 | 0.50 | 13.24 | 8.63 | 2.32 | 60.3 | 0.01 | 294 | 391 |
| 89 | gpu-raster: svo_dag + cubes + gbuffer | 114.6 | 0.50 | 17.01 | 8.39 | 2.85 | 54.9 | 0.02 | 102 | 475 |
| 90 | gpu-compute: clipmap + sphere | 114.2 | 0.50 | 13.03 | 8.67 | 2.40 | 60.3 | 0.01 | 335 | 410 |
| 91 | gpu-compute: prefix + beam + sphere | 113.6 | 0.50 | 13.42 | 8.73 | 2.33 | 60.4 | 0.01 | 294 | 391 |
| 92 | gpu-raster: hier + naive + raster | 112.7 | 0.49 | 13.76 | 8.53 | 2.79 | 55.0 | 0.02 | 347 | 461 |
| 93 | gpu-compute: clipmap + rowrun | 111.5 | 0.49 | 12.94 | 8.89 | 2.33 | 60.3 | 0.01 | 335 | 405 |
| 94 | gpu-rt: naive + rayquery + pathtrace | 110.6 | 0.48 | 49.69 | 7.55 | 3.87 | 35.6 | 2.54 | 596 | 1016 |
| 95 | gpu-compute: tree64 + incremental | 110.5 | 0.48 | 12.79 | 8.96 | 2.49 | 60.4 | 0.01 | 66 | 447 |
| 96 | gpu-raster: tree64 + cubes + visbuffer | 110.3 | 0.48 | 15.71 | 8.82 | 2.79 | 54.9 | 0.02 | 139 | 454 |
| 97 | gpu-raster: tree64 + cubes + gbuffer | 109.1 | 0.48 | 16.27 | 8.93 | 2.77 | 54.9 | 0.02 | 139 | 456 |
| 98 | gpu-compute: clipmap + beam + rowrun | 108.7 | 0.48 | 13.07 | 9.11 | 2.34 | 60.4 | 0.01 | 335 | 405 |
| 99 | gpu-raster: clipmap + naive + raster | 108.4 | 0.47 | 13.73 | 8.93 | 2.82 | 55.0 | 0.02 | 408 | 485 |
| 100 | gpu-compute: clipmap + beam + sphere | 107.9 | 0.47 | 13.32 | 9.18 | 2.37 | 60.4 | 0.01 | 335 | 405 |
| 101 | gpu-raster: distance + greedy + raster | 107.6 | 0.47 | 109.48 | 5.39 | 3.18 | 53.4 | 0.02 | 443 | 420 |
| 102 | gpu-compute: distance + rowrun | 105.1 | 0.46 | 109.73 | 6.49 | 2.33 | 60.3 | 0.01 | 402 | 391 |
| 103 | gpu-compute: tree64 + beam + incremental | 104.6 | 0.46 | 12.95 | 9.47 | 2.56 | 60.4 | 0.01 | 66 | 447 |
| 104 | gpu-compute: distance + beam + rowrun | 104.0 | 0.45 | 109.46 | 6.60 | 2.34 | 60.4 | 0.01 | 402 | 392 |
| 105 | gpu-raster: svo_dag + naive + raster | 96.4 | 0.42 | 21.21 | 9.95 | 3.00 | 55.0 | 0.02 | 102 | 491 |
| 106 | gpu-raster: distance + naive + raster | 95.9 | 0.42 | 109.18 | 7.13 | 2.71 | 55.0 | 0.02 | 475 | 458 |
| 107 | gpu-raster: distance + cubes + visbuffer | 95.8 | 0.42 | 108.45 | 7.30 | 2.55 | 54.9 | 0.02 | 475 | 440 |
| 108 | gpu-raster: distance + cubes + gbuffer | 94.9 | 0.41 | 108.90 | 7.40 | 2.59 | 54.9 | 0.02 | 475 | 440 |
| 109 | gpu-raster: tree64 + naive + raster | 94.8 | 0.41 | 20.46 | 10.22 | 2.89 | 55.0 | 0.02 | 139 | 468 |
| 110 | gpu-compute: svo_dag + sphere | 94.5 | 0.41 | 15.02 | 10.49 | 2.65 | 60.3 | 0.01 | 29 | 447 |
| 111 | gpu-discover: prefix + quadtree | 93.9 | 0.41 | 14.69 | 10.55 | 2.42 | 54.9 | 0.02 | 476 | 400 |
| 112 | gpu-compute: bricks + beam + rowrun | 91.1 | 0.40 | 14.46 | 10.91 | 2.29 | 60.4 | 0.01 | 274 | 391 |
| 113 | gpu-compute: svo_dag + beam + sphere | 89.8 | 0.39 | 15.82 | 11.04 | 2.60 | 60.4 | 0.01 | 29 | 448 |
| 114 | gpu-discover: rows + quadtree | 88.9 | 0.39 | 15.62 | 11.15 | 2.39 | 54.9 | 0.02 | 472 | 402 |
| 115 | gpu-discover: bricks + quadtree | 88.9 | 0.39 | 15.29 | 11.16 | 2.37 | 54.9 | 0.02 | 456 | 397 |
| 116 | gpu-compute: tree64 + sphere | 85.0 | 0.37 | 15.36 | 11.70 | 2.55 | 60.4 | 0.01 | 66 | 447 |
| 117 | gpu-compute: tree64 + beam + sphere | 82.8 | 0.36 | 16.19 | 12.01 | 2.60 | 60.4 | 0.01 | 66 | 447 |
| 118 | gpu-discover: hier + quadtree | 81.2 | 0.36 | 17.21 | 12.22 | 2.40 | 54.9 | 0.02 | 456 | 398 |
| 119 | gpu-discover: clipmap + quadtree | 77.5 | 0.34 | 17.24 | 12.78 | 2.41 | 54.9 | 0.02 | 517 | 414 |
| 120 | gpu-compute: bricks + beam + incremental | 76.7 | 0.34 | 17.41 | 12.97 | 2.34 | 60.4 | 0.01 | 274 | 391 |
| 121 | gpu-compute: rows + incremental | 76.5 | 0.33 | 16.86 | 13.01 | 2.37 | 60.4 | 0.01 | 290 | 391 |
| 122 | gpu-discover: distance + quadtree | 75.8 | 0.33 | 110.93 | 10.37 | 2.33 | 54.9 | 0.02 | 584 | 399 |
| 123 | gpu-compute: bricks + rowrun | 74.8 | 0.33 | 17.26 | 13.30 | 2.34 | 60.3 | 0.01 | 274 | 392 |
| 124 | gpu-compute: bricks + incremental | 74.3 | 0.32 | 17.41 | 13.40 | 2.42 | 60.4 | 0.01 | 274 | 394 |
| 125 | gpu-compute: rows + beam + incremental | 73.3 | 0.32 | 18.22 | 13.56 | 2.34 | 60.4 | 0.01 | 290 | 393 |
| 126 | gpu-raster: prefix + cubes + raster | 72.1 | 0.32 | 19.02 | 13.77 | 2.66 | 55.0 | 0.02 | 367 | 440 |
| 127 | gpu-compute: hier + rowrun | 71.1 | 0.31 | 18.02 | 14.00 | 2.45 | 60.3 | 0.01 | 274 | 395 |
| 128 | gpu-compute: hier + beam + incremental | 71.0 | 0.31 | 18.25 | 14.01 | 2.40 | 60.4 | 0.01 | 274 | 393 |
| 129 | gpu-raster: bricks + cubes + raster | 70.7 | 0.31 | 18.61 | 14.05 | 2.62 | 55.0 | 0.02 | 347 | 445 |
| 130 | gpu-compute: hier + incremental | 70.2 | 0.31 | 18.92 | 14.17 | 2.33 | 60.4 | 0.01 | 274 | 393 |
| 131 | gpu-raster: rows + cubes + raster | 70.1 | 0.31 | 18.84 | 14.16 | 2.73 | 55.0 | 0.02 | 363 | 443 |
| 132 | gpu-discover: svo_dag + quadtree | 68.6 | 0.30 | 18.95 | 14.48 | 2.66 | 54.9 | 0.02 | 211 | 459 |
| 133 | gpu-compute: hier + beam + rowrun | 68.2 | 0.30 | 19.06 | 14.59 | 2.39 | 60.4 | 0.01 | 274 | 393 |
| 134 | gpu-discover: tree64 + quadtree | 67.4 | 0.29 | 18.70 | 14.75 | 2.58 | 54.9 | 0.02 | 248 | 457 |
| 135 | gpu-raster: hier + cubes + raster | 67.0 | 0.29 | 19.50 | 14.84 | 2.67 | 55.0 | 0.02 | 347 | 444 |
| 136 | gpu-compute: prefix + beam + incremental | 66.8 | 0.29 | 19.72 | 14.90 | 2.37 | 60.4 | 0.01 | 294 | 391 |
| 137 | gpu-compute: prefix + incremental | 66.8 | 0.29 | 19.71 | 14.91 | 2.42 | 60.3 | 0.01 | 294 | 391 |
| 138 | gpu-raster: clipmap + cubes + raster | 65.6 | 0.29 | 19.80 | 15.15 | 2.65 | 55.0 | 0.02 | 408 | 469 |
| 139 | gpu-compute: prefix + beam + refloor + pathtrace | 63.4 | 0.28 | 20.53 | 15.71 | 2.43 | 35.6 | 2.54 | 325 | 391 |
| 140 | gpu-compute: distance + sphere + pathtrace | 61.3 | 0.27 | 109.17 | 13.75 | 2.37 | 35.6 | 2.54 | 433 | 391 |
| 141 | gpu-raster: distance + cubes + raster | 61.3 | 0.27 | 108.44 | 13.57 | 2.60 | 55.0 | 0.02 | 475 | 440 |
| 142 | gpu-compute: distance + beam + sphere + pathtrace | 60.8 | 0.27 | 108.97 | 13.89 | 2.38 | 35.6 | 2.54 | 433 | 391 |
| 143 | gpu-raster: svo_dag + cubes + raster | 59.9 | 0.26 | 21.88 | 16.56 | 2.91 | 55.0 | 0.02 | 102 | 477 |
| 144 | gpu-raster: tree64 + cubes + raster | 58.7 | 0.26 | 21.99 | 16.96 | 2.84 | 55.0 | 0.02 | 139 | 455 |
| 145 | gpu-compute: prefix + refloor + pathtrace | 58.5 | 0.26 | 22.69 | 17.03 | 2.42 | 35.6 | 2.54 | 325 | 391 |
| 146 | gpu-compute: rows + beam + refloor + pathtrace | 58.4 | 0.26 | 21.87 | 17.03 | 2.46 | 35.6 | 2.54 | 321 | 391 |
| 147 | gpu-compute: bricks + beam + refloor + pathtrace | 58.2 | 0.25 | 22.41 | 17.10 | 2.39 | 35.6 | 2.54 | 305 | 391 |
| 148 | gpu-compute: hier + beam + refloor + pathtrace | 55.8 | 0.24 | 23.06 | 17.86 | 2.47 | 35.6 | 2.54 | 305 | 393 |
| 149 | gpu-compute: distance + beam + refloor + pathtrace | 55.6 | 0.24 | 109.46 | 15.56 | 2.37 | 35.6 | 2.54 | 433 | 391 |
| 150 | gpu-compute: rows + refloor + pathtrace | 55.1 | 0.24 | 23.53 | 18.09 | 2.41 | 35.6 | 2.54 | 321 | 391 |
| 151 | gpu-compute: bricks + refloor + pathtrace | 55.0 | 0.24 | 23.48 | 18.09 | 2.46 | 35.6 | 2.54 | 305 | 391 |
| 152 | gpu-compute: hier + refloor + pathtrace | 53.3 | 0.23 | 24.56 | 18.68 | 2.40 | 35.6 | 2.54 | 305 | 393 |
| 153 | gpu-compute: distance + refloor + pathtrace | 52.5 | 0.23 | 108.75 | 16.65 | 2.39 | 35.6 | 2.54 | 433 | 391 |
| 154 | gpu-compute: clipmap + refloor + pathtrace | 52.0 | 0.23 | 25.22 | 19.15 | 2.39 | 35.6 | 2.54 | 366 | 405 |
| 155 | gpu-compute: distance + incremental | 51.0 | 0.22 | 109.10 | 17.27 | 2.36 | 60.4 | 0.01 | 402 | 391 |
| 156 | gpu-compute: svo_dag + beam + refloor + pathtrace | 50.9 | 0.22 | 25.08 | 19.58 | 2.66 | 35.6 | 2.54 | 61 | 448 |
| 157 | gpu-compute: distance + beam + incremental | 50.1 | 0.22 | 110.66 | 17.58 | 2.38 | 60.4 | 0.01 | 402 | 391 |
| 158 | gpu-compute: clipmap + beam + refloor + pathtrace | 49.8 | 0.22 | 25.67 | 19.97 | 2.42 | 35.6 | 2.54 | 366 | 406 |
| 159 | gpu-compute: svo_dag + refloor + pathtrace | 49.6 | 0.22 | 26.16 | 20.08 | 2.67 | 35.6 | 2.54 | 61 | 449 |
| 160 | gpu-compute: svo_dag + incremental | 48.5 | 0.21 | 26.43 | 20.55 | 2.62 | 60.3 | 0.01 | 29 | 448 |
| 161 | gpu-compute: svo_dag + beam + incremental | 44.9 | 0.20 | 28.53 | 22.18 | 2.74 | 60.4 | 0.01 | 29 | 448 |
| 162 | gpu-compute: prefix + rowrun + pathtrace | 44.0 | 0.19 | 29.61 | 22.66 | 2.40 | 35.6 | 2.54 | 325 | 391 |
| 163 | gpu-compute: svo_dag + rowrun | 41.7 | 0.18 | 31.80 | 23.91 | 2.66 | 60.4 | 0.01 | 29 | 448 |
| 164 | gpu-compute: rows + rowrun + pathtrace | 41.1 | 0.18 | 31.27 | 24.27 | 2.41 | 35.6 | 2.54 | 321 | 391 |
| 165 | gpu-compute: svo_dag + beam + rowrun | 41.0 | 0.18 | 32.08 | 24.31 | 2.66 | 60.4 | 0.01 | 29 | 451 |
| 166 | gpu-compute: rows + beam + rowrun + pathtrace | 41.0 | 0.18 | 31.18 | 24.34 | 2.41 | 35.6 | 2.54 | 321 | 394 |
| 167 | gpu-compute: tree64 + beam + refloor + pathtrace | 40.8 | 0.18 | 32.60 | 24.45 | 2.59 | 35.6 | 2.54 | 97 | 450 |
| 168 | gpu-compute: prefix + beam + rowrun + pathtrace | 38.6 | 0.17 | 33.24 | 25.84 | 2.40 | 35.6 | 2.54 | 325 | 391 |
| 169 | gpu-compute: tree64 + refloor + pathtrace | 38.4 | 0.17 | 35.05 | 25.95 | 2.65 | 35.6 | 2.54 | 97 | 449 |
| 170 | gpu-compute: distance + rowrun + pathtrace | 36.9 | 0.16 | 109.90 | 25.22 | 2.38 | 35.6 | 2.54 | 433 | 392 |
| 171 | gpu-compute: distance + beam + rowrun + pathtrace | 36.8 | 0.16 | 108.17 | 25.37 | 2.38 | 35.6 | 2.54 | 433 | 391 |
| 172 | gpu-compute: bricks + sphere + pathtrace | 35.1 | 0.15 | 35.97 | 28.39 | 2.41 | 35.6 | 2.54 | 305 | 391 |
| 173 | gpu-compute: rows + sphere + pathtrace | 35.1 | 0.15 | 36.17 | 28.39 | 2.44 | 35.6 | 2.54 | 321 | 391 |
| 174 | gpu-compute: rows + beam + sphere + pathtrace | 34.9 | 0.15 | 36.28 | 28.57 | 2.43 | 35.6 | 2.54 | 321 | 391 |
| 175 | gpu-compute: bricks + beam + sphere + pathtrace | 34.9 | 0.15 | 36.30 | 28.59 | 2.42 | 35.6 | 2.54 | 305 | 391 |
| 176 | gpu-compute: hier + sphere + pathtrace | 34.8 | 0.15 | 36.46 | 28.64 | 2.42 | 35.6 | 2.54 | 305 | 396 |
| 177 | gpu-compute: hier + beam + sphere + pathtrace | 34.5 | 0.15 | 36.62 | 28.89 | 2.45 | 35.6 | 2.54 | 305 | 397 |
| 178 | gpu-compute: clipmap + incremental + pathtrace | 34.5 | 0.15 | 37.31 | 28.90 | 2.40 | 35.6 | 2.54 | 366 | 405 |
| 179 | gpu-compute: clipmap + beam + incremental + pathtrace | 34.2 | 0.15 | 36.97 | 29.14 | 2.38 | 35.6 | 2.54 | 366 | 405 |
| 180 | gpu-compute: clipmap + sphere + pathtrace | 33.0 | 0.14 | 38.07 | 30.18 | 2.48 | 35.6 | 2.54 | 366 | 405 |
| 181 | gpu-compute: prefix + beam + sphere + pathtrace | 32.0 | 0.14 | 39.51 | 31.21 | 2.42 | 35.6 | 2.54 | 325 | 391 |
| 182 | gpu-compute: prefix + sphere + pathtrace | 31.6 | 0.14 | 40.28 | 31.53 | 2.44 | 35.6 | 2.54 | 325 | 391 |
| 183 | gpu-compute: clipmap + beam + sphere + pathtrace | 29.8 | 0.13 | 41.86 | 33.47 | 2.47 | 35.6 | 2.54 | 366 | 405 |
| 184 | gpu-compute: tree64 + beam + rowrun | 29.7 | 0.13 | 42.86 | 33.60 | 2.62 | 60.3 | 0.01 | 66 | 448 |
| 185 | gpu-rt: boxes + rayquery + pathtrace | 28.7 | 0.13 | 45.76 | 34.79 | 2.87 | 35.6 | 2.54 | 322 | 622 |
| 186 | gpu-compute: clipmap + rowrun + pathtrace | 28.6 | 0.13 | 44.64 | 34.90 | 2.40 | 35.6 | 2.54 | 366 | 408 |
| 187 | gpu-compute: clipmap + beam + rowrun + pathtrace | 28.6 | 0.12 | 44.17 | 34.92 | 2.43 | 35.6 | 2.54 | 366 | 407 |
| 188 | gpu-compute: tree64 + incremental + pathtrace | 28.5 | 0.12 | 46.32 | 34.97 | 2.62 | 35.6 | 2.54 | 97 | 447 |
| 189 | gpu-compute: tree64 + rowrun | 28.4 | 0.12 | 45.21 | 35.11 | 2.63 | 60.3 | 0.01 | 66 | 449 |
| 190 | cpu-discover: prefix + quadtree | 26.9 | 0.12 | 62.49 | 10.87 | 35.97 | 46.5 | 2.48 | 278 | 1155 |
| 191 | cpu-discover: hier + quadtree | 26.5 | 0.12 | 67.32 | 10.84 | 36.52 | 46.5 | 2.48 | 278 | 1003 |
| 192 | gpu-compute: svo_dag + beam + sphere + pathtrace | 26.1 | 0.11 | 48.43 | 38.28 | 2.68 | 35.6 | 2.54 | 61 | 449 |
| 193 | gpu-compute: svo_dag + sphere + pathtrace | 26.1 | 0.11 | 48.99 | 38.31 | 2.68 | 35.6 | 2.54 | 61 | 448 |
| 194 | cpu-discover: morton + quadtree | 25.5 | 0.11 | 64.73 | 10.72 | 37.93 | 46.5 | 2.48 | 278 | 1169 |
| 195 | cpu-discover: bits + quadtree | 25.4 | 0.11 | 72.00 | 10.76 | 38.12 | 46.5 | 2.48 | 278 | 1083 |
| 196 | cpu-discover: bricks + quadtree | 25.3 | 0.11 | 74.10 | 10.77 | 38.28 | 46.5 | 2.48 | 278 | 1047 |
| 197 | cpu-discover: u8 + quadtree | 24.9 | 0.11 | 75.00 | 10.83 | 38.92 | 46.5 | 2.48 | 278 | 1117 |
| 198 | cpu-discover: brickmajor + quadtree | 24.9 | 0.11 | 66.03 | 10.80 | 38.76 | 46.5 | 2.48 | 278 | 1234 |
| 199 | gpu-compute: tree64 + beam + incremental + pathtrace | 24.9 | 0.11 | 52.95 | 40.08 | 2.64 | 35.6 | 2.54 | 97 | 453 |
| 200 | cpu-discover: palette + quadtree | 24.6 | 0.11 | 62.64 | 10.71 | 39.03 | 46.5 | 2.48 | 278 | 1066 |
| 201 | gpu-compute: bricks + rowrun + pathtrace | 24.5 | 0.11 | 52.27 | 40.75 | 2.46 | 35.6 | 2.54 | 305 | 393 |
| 202 | gpu-compute: bricks + beam + rowrun + pathtrace | 24.2 | 0.11 | 52.56 | 41.25 | 2.43 | 35.6 | 2.54 | 305 | 392 |
| 203 | gpu-compute: tree64 + sphere + pathtrace | 23.1 | 0.10 | 57.18 | 43.23 | 2.65 | 35.6 | 2.54 | 97 | 453 |
| 204 | gpu-compute: tree64 + beam + sphere + pathtrace | 22.9 | 0.10 | 57.43 | 43.55 | 2.65 | 35.6 | 2.54 | 97 | 447 |
| 205 | gpu-compute: rows + beam + incremental + pathtrace | 20.9 | 0.09 | 60.13 | 47.70 | 2.44 | 35.6 | 2.54 | 321 | 391 |
| 206 | gpu-compute: rows + incremental + pathtrace | 20.5 | 0.09 | 61.34 | 48.68 | 2.43 | 35.6 | 2.54 | 321 | 392 |
| 207 | gpu-compute: bricks + beam + incremental + pathtrace | 20.0 | 0.09 | 63.05 | 49.94 | 2.45 | 35.6 | 2.54 | 305 | 394 |
| 208 | gpu-compute: bricks + incremental + pathtrace | 19.5 | 0.09 | 64.66 | 51.13 | 2.42 | 35.6 | 2.54 | 305 | 391 |
| 209 | gpu-compute: hier + beam + incremental + pathtrace | 19.4 | 0.08 | 65.29 | 51.38 | 2.43 | 35.6 | 2.54 | 305 | 393 |
| 210 | gpu-compute: hier + incremental + pathtrace | 18.8 | 0.08 | 67.62 | 53.10 | 2.46 | 35.6 | 2.54 | 305 | 393 |
| 211 | gpu-compute: prefix + beam + incremental + pathtrace | 18.6 | 0.08 | 68.72 | 53.78 | 2.43 | 35.6 | 2.54 | 325 | 391 |
| 212 | gpu-compute: prefix + incremental + pathtrace | 18.3 | 0.08 | 69.67 | 54.43 | 2.45 | 35.6 | 2.54 | 325 | 391 |
| 213 | gpu-compute: hier + rowrun + pathtrace | 18.3 | 0.08 | 70.25 | 54.57 | 2.44 | 35.6 | 2.54 | 305 | 397 |
| 214 | cpu-discover: prefix + cluster | 17.9 | 0.08 | 112.06 | 10.57 | 54.84 | 48.7 | 1.72 | 278 | 796 |
| 215 | cpu-discover: hier + cluster | 17.9 | 0.08 | 111.86 | 10.55 | 55.03 | 48.7 | 1.72 | 278 | 650 |
| 216 | gpu-compute: hier + beam + rowrun + pathtrace | 17.4 | 0.08 | 73.43 | 57.29 | 2.49 | 35.6 | 2.54 | 305 | 400 |
| 217 | gpu-compute: distance + beam + incremental + pathtrace | 17.3 | 0.08 | 108.99 | 57.52 | 2.42 | 35.6 | 2.54 | 433 | 392 |
| 218 | cpu-discover: morton + cluster | 17.2 | 0.08 | 123.28 | 10.53 | 57.23 | 48.7 | 1.72 | 278 | 815 |
| 219 | gpu-compute: distance + incremental + pathtrace | 16.9 | 0.07 | 109.14 | 58.97 | 2.44 | 35.6 | 2.54 | 433 | 391 |
| 220 | cpu-discover: palette + cluster | 16.9 | 0.07 | 115.67 | 10.52 | 58.00 | 48.7 | 1.72 | 278 | 667 |
| 221 | cpu-discover: bricks + cluster | 16.8 | 0.07 | 124.99 | 10.52 | 58.52 | 48.7 | 1.72 | 278 | 659 |
| 222 | cpu-discover: u8 + cluster | 16.8 | 0.07 | 119.84 | 10.51 | 58.56 | 48.7 | 1.72 | 278 | 750 |
| 223 | cpu-discover: brickmajor + cluster | 16.8 | 0.07 | 121.21 | 10.53 | 58.39 | 48.7 | 1.72 | 278 | 802 |
| 224 | cpu-discover: bits + cluster | 16.6 | 0.07 | 123.21 | 10.51 | 59.35 | 48.7 | 1.72 | 278 | 649 |
| 225 | gpu-compute: svo_dag + incremental + pathtrace | 13.0 | 0.06 | 100.05 | 77.13 | 2.71 | 35.6 | 2.54 | 61 | 448 |
| 226 | gpu-compute: svo_dag + beam + incremental + pathtrace | 12.4 | 0.05 | 104.00 | 80.38 | 2.76 | 35.6 | 2.54 | 61 | 449 |
| 227 | gpu-compute: svo_dag + rowrun + pathtrace | 11.2 | 0.05 | 115.21 | 89.38 | 2.71 | 35.6 | 2.54 | 61 | 452 |
| 228 | gpu-compute: svo_dag + beam + rowrun + pathtrace | 11.1 | 0.05 | 115.03 | 89.64 | 2.73 | 35.6 | 2.54 | 61 | 450 |
| 229 | cpu-raster: greedy + raster | 10.6 | 0.05 | 124.96 | - | 93.47 | 65.3 | 0.00 | - | 365 |
| 230 | cpu: bricks + refloor | 9.1 | 0.04 | 134.19 | - | 109.52 | 61.6 | 0.01 | - | 216 |
| 231 | cpu: hier + refloor | 9.0 | 0.04 | 141.26 | - | 110.08 | 61.6 | 0.01 | - | 226 |
| 232 | cpu: bits + refloor | 9.0 | 0.04 | 138.09 | - | 110.33 | 61.6 | 0.01 | - | 226 |
| 233 | cpu: u8 + refloor | 8.9 | 0.04 | 140.20 | - | 111.70 | 61.6 | 0.01 | - | 162 |
| 234 | cpu: brickmajor + refloor | 8.8 | 0.04 | 138.91 | - | 112.92 | 61.6 | 0.01 | - | 258 |
| 235 | cpu: morton + refloor | 8.8 | 0.04 | 138.54 | - | 113.43 | 61.6 | 0.01 | - | 256 |
| 236 | cpu: palette + refloor | 8.6 | 0.04 | 142.07 | - | 115.26 | 61.6 | 0.01 | - | 153 |
| 237 | cpu-raster: naive + raster | 8.6 | 0.04 | 149.12 | - | 114.27 | 68.6 | 0.00 | - | 571 |
| 238 | cpu: prefix + refloor | 8.4 | 0.04 | 149.57 | - | 118.72 | 61.7 | 0.01 | - | 237 |
| 239 | cpu: hier + incremental | 7.8 | 0.03 | 166.19 | - | 128.53 | 61.6 | 0.01 | - | 226 |
| 240 | cpu: bricks + incremental | 7.6 | 0.03 | 165.90 | - | 131.52 | 61.6 | 0.01 | - | 216 |
| 241 | cpu: u8 + incremental | 7.6 | 0.03 | 167.74 | - | 131.95 | 61.6 | 0.01 | - | 161 |
| 242 | cpu: morton + incremental | 7.5 | 0.03 | 168.99 | - | 132.14 | 61.6 | 0.01 | - | 260 |
| 243 | cpu: prefix + incremental | 7.3 | 0.03 | 182.09 | - | 136.20 | 61.6 | 0.01 | - | 239 |
| 244 | gpu-compute: tree64 + beam + rowrun + pathtrace | 7.2 | 0.03 | 178.06 | 138.32 | 2.64 | 35.6 | 2.54 | 97 | 452 |
| 245 | cpu: bits + incremental | 7.2 | 0.03 | 181.59 | - | 139.11 | 61.6 | 0.01 | - | 226 |
| 246 | cpu: palette + incremental | 7.2 | 0.03 | 180.21 | - | 139.08 | 61.6 | 0.01 | - | 153 |
| 247 | cpu: brickmajor + incremental | 7.1 | 0.03 | 178.72 | - | 140.32 | 61.6 | 0.01 | - | 259 |
| 248 | gpu-compute: tree64 + rowrun + pathtrace | 7.1 | 0.03 | 182.46 | 140.75 | 2.69 | 35.6 | 2.54 | 97 | 449 |
| 249 | cpu-voxelspace: heightmap + columns (incorrect) | 122.1 | 0.53 | 11.16 | - | 7.95 | 14.8 | 54.06 | - | 259 |
| 250 | gpu-splat: prefix + points + splat (incorrect) | 111.6 | 0.49 | 107.27 | 5.18 | 2.30 | 18.9 | 29.98 | 406 | 575 |
| 251 | gpu-splat: bricks + points + splat (incorrect) | 110.4 | 0.48 | 105.49 | 5.33 | 2.31 | 18.9 | 29.98 | 386 | 574 |
| 252 | gpu-splat: clipmap + points + splat (incorrect) | 108.8 | 0.48 | 106.24 | 5.47 | 2.36 | 18.9 | 29.98 | 447 | 585 |
| 253 | gpu-splat: rows + points + splat (incorrect) | 107.9 | 0.47 | 111.41 | 5.40 | 2.34 | 18.9 | 29.98 | 402 | 574 |
| 254 | gpu-splat: hier + points + splat (incorrect) | 105.3 | 0.46 | 108.42 | 5.69 | 2.32 | 18.9 | 29.98 | 386 | 575 |
| 255 | gpu-splat: svo_dag + points + splat (incorrect) | 100.1 | 0.44 | 100.45 | 6.17 | 2.57 | 18.9 | 29.98 | 141 | 634 |
| 256 | gpu-splat: tree64 + points + splat (incorrect) | 95.5 | 0.42 | 98.79 | 6.91 | 2.50 | 18.9 | 29.98 | 178 | 633 |
| 257 | gpu-splat: distance + points + splat (incorrect) | 84.5 | 0.37 | 203.23 | 5.14 | 2.31 | 18.9 | 29.98 | 514 | 575 |
| 258 | gpu-voxelspace: heightmap + columns (incorrect) | 65.6 | 0.29 | 21.33 | 15.19 | 2.41 | 17.0 | 50.74 | 13 | 422 |

### menger [lit]

1920×1080. camera 3.46 blocks/frame (max 3.5), turn 0.56°/frame; 312 chunks at start, 0.8 loads/frame; chunk fill 2%, solid 0.4%, surface 0.40%; sky 72%, hits at 259 blocks (median 248); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: hier + refloor | 1652.2 | 1.00 | 3.17 | 0.50 | 0.01 | 59.3 | 0.00 | 48 | 257 |
| 2 | gpu-compute: bricks + refloor | 1579.6 | 0.96 | 2.34 | 0.53 | 0.01 | 59.3 | 0.00 | 48 | 257 |
| 3 | gpu-compute: rows + refloor | 1566.5 | 0.95 | 2.80 | 0.54 | 0.01 | 59.3 | 0.00 | 50 | 257 |
| 4 | gpu-compute: prefix + refloor | 1527.5 | 0.92 | 2.89 | 0.54 | 0.01 | 59.3 | 0.00 | 51 | 262 |
| 5 | gpu-compute: rows + rowrun | 1419.3 | 0.86 | 3.25 | 0.61 | 0.01 | 59.3 | 0.00 | 50 | 257 |
| 6 | gpu-compute: svo_dag + refloor | 1392.8 | 0.84 | 3.33 | 0.62 | 0.05 | 59.3 | 0.00 | 17 | 269 |
| 7 | gpu-compute: prefix + rowrun | 1322.1 | 0.80 | 2.63 | 0.66 | 0.01 | 59.3 | 0.00 | 51 | 257 |
| 8 | gpu-compute: clipmap + refloor | 1267.8 | 0.77 | 2.43 | 0.71 | 0.01 | 59.3 | 0.00 | 334 | 259 |
| 9 | gpu-compute: bricks + rowrun | 1248.1 | 0.76 | 2.48 | 0.73 | 0.01 | 59.3 | 0.00 | 48 | 257 |
| 10 | gpu-compute: tree64 + refloor | 1205.4 | 0.73 | 2.65 | 0.73 | 0.05 | 59.3 | 0.00 | 23 | 274 |
| 11 | gpu-compute: distance + sphere | 1137.7 | 0.69 | 33.15 | 0.38 | 0.01 | 59.2 | 0.00 | 64 | 257 |
| 12 | gpu-compute: hier + beam + refloor | 1080.2 | 0.65 | 2.72 | 0.83 | 0.01 | 59.2 | 0.00 | 48 | 262 |
| 13 | gpu-compute: bricks + beam + refloor | 1066.1 | 0.65 | 2.56 | 0.87 | 0.01 | 59.2 | 0.00 | 48 | 257 |
| 14 | gpu-compute: clipmap + rowrun | 1057.2 | 0.64 | 2.67 | 0.87 | 0.01 | 59.3 | 0.00 | 334 | 259 |
| 15 | gpu-compute: hier + rowrun | 1052.9 | 0.64 | 2.69 | 0.87 | 0.01 | 59.3 | 0.00 | 48 | 257 |
| 16 | gpu-compute: rows + beam + refloor | 1042.1 | 0.63 | 2.79 | 0.87 | 0.01 | 59.2 | 0.00 | 50 | 257 |
| 17 | gpu-compute: distance + refloor | 1025.5 | 0.62 | 32.36 | 0.48 | 0.01 | 59.2 | 0.00 | 64 | 257 |
| 18 | gpu-compute: prefix + beam + refloor | 1022.9 | 0.62 | 2.88 | 0.89 | 0.01 | 59.2 | 0.00 | 51 | 257 |
| 19 | gpu-rt: boxes + rayquery | 1022.5 | 0.62 | 2.76 | 0.87 | 0.11 | 59.3 | 0.00 | 37 | 405 |
| 20 | gpu-compute: rows + beam + rowrun | 962.6 | 0.58 | 2.90 | 0.95 | 0.01 | 59.2 | 0.00 | 50 | 257 |
| 21 | gpu-compute: clipmap + incremental | 944.7 | 0.57 | 2.83 | 0.98 | 0.01 | 59.2 | 0.00 | 334 | 259 |
| 22 | gpu-compute: distance + rowrun | 923.9 | 0.56 | 31.29 | 0.59 | 0.01 | 59.2 | 0.00 | 64 | 257 |
| 23 | gpu-compute: prefix + beam + rowrun | 921.6 | 0.56 | 2.93 | 1.00 | 0.01 | 59.2 | 0.00 | 51 | 257 |
| 24 | gpu-compute: bricks + beam + rowrun | 890.8 | 0.54 | 2.78 | 1.06 | 0.01 | 59.2 | 0.00 | 48 | 257 |
| 25 | gpu-rt: greedy + rayquery | 869.5 | 0.53 | 30.52 | 0.62 | 0.48 | 59.4 | 0.00 | 288 | 586 |
| 26 | gpu-compute: distance + beam + sphere | 817.3 | 0.49 | 31.64 | 0.72 | 0.01 | 59.1 | 0.00 | 64 | 257 |
| 27 | gpu-compute: tree64 + incremental | 813.1 | 0.49 | 3.03 | 1.15 | 0.05 | 59.2 | 0.00 | 23 | 272 |
| 28 | gpu-compute: hier + beam + rowrun | 770.2 | 0.47 | 3.11 | 1.23 | 0.01 | 59.2 | 0.00 | 48 | 257 |
| 29 | gpu-compute: distance + beam + refloor | 761.1 | 0.46 | 31.44 | 0.82 | 0.01 | 59.2 | 0.00 | 64 | 257 |
| 30 | gpu-raster: bricks + greedy + visbuffer | 747.5 | 0.45 | 7.87 | 1.15 | 0.15 | 53.7 | 0.01 | 122 | 287 |
| 31 | gpu-raster: prefix + greedy + visbuffer | 741.3 | 0.45 | 7.52 | 1.15 | 0.15 | 53.7 | 0.01 | 124 | 288 |
| 32 | gpu-raster: hier + greedy + visbuffer | 739.9 | 0.45 | 7.78 | 1.14 | 0.14 | 53.7 | 0.01 | 122 | 289 |
| 33 | gpu-raster: rows + greedy + visbuffer | 738.4 | 0.45 | 7.63 | 1.15 | 0.15 | 53.7 | 0.01 | 124 | 289 |
| 34 | gpu-raster: bricks + greedy + gbuffer | 738.0 | 0.45 | 7.73 | 1.15 | 0.16 | 53.5 | 0.01 | 122 | 289 |
| 35 | gpu-compute: hier + sphere | 732.2 | 0.44 | 2.99 | 1.30 | 0.01 | 59.2 | 0.00 | 48 | 257 |
| 36 | gpu-compute: tree64 + beam + refloor | 728.1 | 0.44 | 3.29 | 1.30 | 0.05 | 59.2 | 0.00 | 23 | 273 |
| 37 | gpu-raster: rows + greedy + gbuffer | 728.0 | 0.44 | 7.89 | 1.16 | 0.15 | 53.5 | 0.01 | 124 | 289 |
| 38 | gpu-raster: svo_dag + greedy + visbuffer | 713.4 | 0.43 | 8.53 | 1.20 | 0.18 | 53.7 | 0.01 | 90 | 288 |
| 39 | gpu-compute: distance + beam + rowrun | 708.9 | 0.43 | 31.18 | 0.93 | 0.01 | 59.2 | 0.00 | 64 | 257 |
| 40 | gpu-raster: clipmap + greedy + visbuffer | 703.6 | 0.43 | 7.63 | 1.24 | 0.14 | 53.7 | 0.01 | 407 | 296 |
| 41 | gpu-raster: clipmap + greedy + gbuffer | 701.5 | 0.42 | 7.80 | 1.25 | 0.15 | 53.5 | 0.01 | 407 | 296 |
| 42 | gpu-raster: tree64 + greedy + visbuffer | 700.3 | 0.42 | 7.73 | 1.23 | 0.18 | 53.7 | 0.01 | 96 | 291 |
| 43 | gpu-compute: bricks + sphere | 685.2 | 0.41 | 3.52 | 1.40 | 0.01 | 59.2 | 0.00 | 48 | 257 |
| 44 | gpu-compute: rows + sphere | 685.1 | 0.41 | 3.18 | 1.40 | 0.01 | 59.2 | 0.00 | 50 | 257 |
| 45 | gpu-rt: naive + rayquery | 672.3 | 0.41 | 47.63 | 0.71 | 0.66 | 59.5 | 0.00 | 411 | 708 |
| 46 | gpu-compute: prefix + sphere | 590.0 | 0.36 | 3.41 | 1.62 | 0.01 | 59.2 | 0.00 | 51 | 257 |
| 47 | gpu-compute: hier + beam + sphere | 589.3 | 0.36 | 3.53 | 1.63 | 0.01 | 59.2 | 0.00 | 48 | 257 |
| 48 | gpu-compute: svo_dag + beam + refloor | 584.9 | 0.35 | 3.98 | 1.63 | 0.05 | 59.2 | 0.00 | 17 | 269 |
| 49 | gpu-compute: tree64 + beam + incremental | 579.4 | 0.35 | 3.65 | 1.67 | 0.05 | 59.2 | 0.00 | 23 | 273 |
| 50 | gpu-raster: hier + naive + visbuffer | 574.9 | 0.35 | 4.92 | 1.62 | 0.09 | 55.3 | 0.01 | 122 | 302 |
| 51 | gpu-raster: hier + naive + gbuffer | 573.1 | 0.35 | 5.25 | 1.63 | 0.09 | 55.1 | 0.01 | 122 | 303 |
| 52 | gpu-raster: rows + naive + gbuffer | 568.4 | 0.34 | 5.34 | 1.64 | 0.09 | 55.1 | 0.01 | 124 | 305 |
| 53 | gpu-raster: rows + naive + visbuffer | 568.1 | 0.34 | 5.11 | 1.63 | 0.09 | 55.3 | 0.01 | 124 | 303 |
| 54 | gpu-compute: bricks + beam + sphere | 564.7 | 0.34 | 3.44 | 1.73 | 0.02 | 59.2 | 0.00 | 48 | 257 |
| 55 | gpu-compute: clipmap + sphere | 561.1 | 0.34 | 3.65 | 1.74 | 0.01 | 59.2 | 0.00 | 334 | 259 |
| 56 | gpu-compute: rows + beam + sphere | 560.6 | 0.34 | 4.14 | 1.73 | 0.01 | 59.2 | 0.00 | 50 | 257 |
| 57 | gpu-raster: svo_dag + naive + visbuffer | 557.6 | 0.34 | 5.52 | 1.68 | 0.12 | 55.3 | 0.01 | 90 | 302 |
| 58 | gpu-raster: svo_dag + naive + gbuffer | 554.3 | 0.34 | 5.64 | 1.68 | 0.13 | 55.1 | 0.01 | 90 | 302 |
| 59 | gpu-raster: tree64 + naive + visbuffer | 549.5 | 0.33 | 4.81 | 1.71 | 0.12 | 55.3 | 0.01 | 96 | 306 |
| 60 | gpu-raster: clipmap + naive + visbuffer | 548.2 | 0.33 | 4.09 | 1.72 | 0.09 | 55.3 | 0.01 | 407 | 311 |
| 61 | gpu-raster: clipmap + naive + gbuffer | 547.8 | 0.33 | 4.11 | 1.72 | 0.09 | 55.1 | 0.01 | 407 | 310 |
| 62 | gpu-raster: tree64 + naive + gbuffer | 547.0 | 0.33 | 4.63 | 1.71 | 0.12 | 55.1 | 0.01 | 96 | 302 |
| 63 | gpu-raster: distance + greedy + visbuffer | 546.6 | 0.33 | 31.76 | 1.23 | 0.15 | 53.7 | 0.01 | 138 | 289 |
| 64 | gpu-raster: bricks + naive + visbuffer | 543.5 | 0.33 | 3.85 | 1.74 | 0.09 | 55.3 | 0.01 | 122 | 303 |
| 65 | gpu-compute: tree64 + rowrun | 534.6 | 0.32 | 3.65 | 1.81 | 0.05 | 59.3 | 0.00 | 23 | 273 |
| 66 | gpu-compute: svo_dag + sphere | 531.9 | 0.32 | 3.89 | 1.81 | 0.06 | 59.2 | 0.00 | 17 | 269 |
| 67 | gpu-compute: svo_dag + rowrun | 521.0 | 0.32 | 4.20 | 1.86 | 0.05 | 59.3 | 0.00 | 17 | 269 |
| 68 | gpu-raster: prefix + greedy + gbuffer | 515.5 | 0.31 | 8.44 | 1.75 | 0.15 | 53.5 | 0.01 | 124 | 289 |
| 69 | gpu-raster: hier + greedy + gbuffer | 510.3 | 0.31 | 8.58 | 1.77 | 0.15 | 53.5 | 0.01 | 122 | 289 |
| 70 | gpu-compute: tree64 + sphere | 504.0 | 0.31 | 3.47 | 1.93 | 0.05 | 59.2 | 0.00 | 23 | 274 |
| 71 | gpu-raster: svo_dag + greedy + gbuffer | 496.2 | 0.30 | 8.58 | 1.83 | 0.19 | 53.5 | 0.01 | 90 | 288 |
| 72 | gpu-compute: prefix + beam + sphere | 494.4 | 0.30 | 4.11 | 1.97 | 0.02 | 59.2 | 0.00 | 51 | 257 |
| 73 | gpu-raster: tree64 + greedy + gbuffer | 489.8 | 0.30 | 7.59 | 1.85 | 0.20 | 53.5 | 0.01 | 96 | 292 |
| 74 | gpu-compute: clipmap + beam + refloor | 486.5 | 0.29 | 3.81 | 2.01 | 0.02 | 59.2 | 0.00 | 334 | 259 |
| 75 | gpu-raster: distance + naive + gbuffer | 469.8 | 0.28 | 31.93 | 1.62 | 0.09 | 55.1 | 0.01 | 138 | 303 |
| 76 | gpu-raster: bricks + greedy + raster | 460.5 | 0.28 | 6.79 | 2.02 | 0.15 | 53.5 | 0.01 | 122 | 288 |
| 77 | gpu-raster: prefix + greedy + raster | 458.0 | 0.28 | 6.02 | 2.03 | 0.15 | 53.5 | 0.01 | 124 | 289 |
| 78 | gpu-raster: rows + greedy + raster | 456.0 | 0.28 | 8.67 | 2.03 | 0.15 | 53.5 | 0.01 | 124 | 288 |
| 79 | gpu-raster: hier + greedy + raster | 454.7 | 0.28 | 8.63 | 2.03 | 0.15 | 53.5 | 0.01 | 122 | 289 |
| 80 | gpu-compute: clipmap + beam + rowrun | 452.8 | 0.27 | 4.22 | 2.16 | 0.02 | 59.2 | 0.00 | 334 | 259 |
| 81 | gpu-raster: bricks + naive + gbuffer | 440.6 | 0.27 | 4.83 | 2.18 | 0.09 | 55.1 | 0.01 | 122 | 304 |
| 82 | gpu-raster: distance + greedy + gbuffer | 439.6 | 0.27 | 31.43 | 1.70 | 0.15 | 53.5 | 0.01 | 138 | 289 |
| 83 | gpu-compute: clipmap + beam + incremental | 426.9 | 0.26 | 4.18 | 2.30 | 0.02 | 59.2 | 0.00 | 334 | 259 |
| 84 | gpu-raster: clipmap + greedy + raster | 425.7 | 0.26 | 6.21 | 2.20 | 0.15 | 53.5 | 0.01 | 407 | 296 |
| 85 | gpu-raster: prefix + naive + gbuffer | 422.3 | 0.26 | 4.51 | 2.27 | 0.09 | 55.1 | 0.01 | 124 | 303 |
| 86 | gpu-raster: svo_dag + greedy + raster | 420.1 | 0.25 | 8.19 | 2.21 | 0.20 | 53.5 | 0.01 | 90 | 288 |
| 87 | gpu-raster: prefix + naive + visbuffer | 418.3 | 0.25 | 4.82 | 2.30 | 0.09 | 55.3 | 0.01 | 124 | 303 |
| 88 | gpu-raster: tree64 + greedy + raster | 418.2 | 0.25 | 6.89 | 2.23 | 0.19 | 53.5 | 0.01 | 96 | 292 |
| 89 | gpu-compute: tree64 + beam + rowrun | 416.1 | 0.25 | 4.32 | 2.35 | 0.06 | 59.2 | 0.00 | 23 | 274 |
| 90 | gpu-compute: svo_dag + incremental | 405.9 | 0.25 | 3.90 | 2.42 | 0.06 | 59.2 | 0.00 | 17 | 269 |
| 91 | gpu-compute: tree64 + beam + sphere | 397.0 | 0.24 | 4.27 | 2.47 | 0.05 | 59.2 | 0.00 | 23 | 274 |
| 92 | gpu-raster: distance + greedy + raster | 392.1 | 0.24 | 32.81 | 1.98 | 0.15 | 53.5 | 0.01 | 138 | 289 |
| 93 | gpu-raster: distance + naive + visbuffer | 365.3 | 0.22 | 32.55 | 2.23 | 0.10 | 55.3 | 0.01 | 138 | 302 |
| 94 | gpu-compute: svo_dag + beam + sphere | 350.6 | 0.21 | 4.31 | 2.80 | 0.06 | 59.2 | 0.00 | 17 | 269 |
| 95 | gpu-discover: hier + quadtree | 347.7 | 0.21 | 4.65 | 2.81 | 0.05 | 53.1 | 0.01 | 230 | 266 |
| 96 | gpu-discover: rows + quadtree | 347.4 | 0.21 | 4.64 | 2.81 | 0.05 | 53.1 | 0.01 | 232 | 266 |
| 97 | gpu-discover: bricks + quadtree | 346.2 | 0.21 | 4.88 | 2.82 | 0.04 | 53.1 | 0.01 | 230 | 266 |
| 98 | gpu-discover: prefix + quadtree | 344.3 | 0.21 | 4.98 | 2.83 | 0.05 | 53.1 | 0.01 | 233 | 270 |
| 99 | gpu-compute: svo_dag + beam + rowrun | 342.5 | 0.21 | 5.14 | 2.86 | 0.06 | 59.2 | 0.00 | 17 | 269 |
| 100 | gpu-raster: prefix + naive + raster | 326.3 | 0.20 | 5.09 | 2.99 | 0.10 | 55.1 | 0.01 | 124 | 306 |
| 101 | gpu-compute: clipmap + beam + sphere | 323.5 | 0.20 | 4.77 | 3.05 | 0.02 | 59.2 | 0.00 | 334 | 259 |
| 102 | gpu-raster: hier + naive + raster | 323.3 | 0.20 | 5.15 | 3.02 | 0.09 | 55.1 | 0.01 | 122 | 303 |
| 103 | gpu-raster: rows + naive + raster | 323.2 | 0.20 | 5.20 | 3.02 | 0.09 | 55.1 | 0.01 | 124 | 302 |
| 104 | gpu-raster: bricks + naive + raster | 322.5 | 0.20 | 5.06 | 3.03 | 0.10 | 55.1 | 0.01 | 122 | 303 |
| 105 | gpu-compute: bricks + incremental | 320.0 | 0.19 | 4.40 | 3.09 | 0.02 | 59.2 | 0.00 | 48 | 256 |
| 106 | gpu-discover: distance + quadtree | 313.0 | 0.19 | 32.66 | 2.74 | 0.05 | 53.1 | 0.01 | 246 | 266 |
| 107 | gpu-compute: rows + incremental | 309.6 | 0.19 | 5.15 | 3.19 | 0.02 | 59.2 | 0.00 | 50 | 257 |
| 108 | gpu-raster: clipmap + naive + raster | 308.3 | 0.19 | 5.24 | 3.18 | 0.09 | 55.1 | 0.01 | 407 | 310 |
| 109 | gpu-raster: svo_dag + naive + raster | 308.2 | 0.19 | 5.63 | 3.15 | 0.14 | 55.1 | 0.01 | 90 | 303 |
| 110 | gpu-raster: tree64 + naive + raster | 302.5 | 0.18 | 5.46 | 3.23 | 0.13 | 55.1 | 0.01 | 96 | 301 |
| 111 | gpu-compute: hier + incremental | 300.2 | 0.18 | 5.02 | 3.28 | 0.02 | 59.2 | 0.00 | 48 | 257 |
| 112 | gpu-raster: distance + naive + raster | 289.7 | 0.18 | 32.78 | 2.97 | 0.10 | 55.1 | 0.01 | 138 | 303 |
| 113 | gpu-compute: bricks + beam + incremental | 287.0 | 0.17 | 5.08 | 3.45 | 0.02 | 59.2 | 0.00 | 48 | 257 |
| 114 | gpu-discover: clipmap + quadtree | 286.1 | 0.17 | 5.02 | 3.43 | 0.05 | 53.1 | 0.01 | 516 | 267 |
| 115 | gpu-compute: svo_dag + beam + incremental | 282.1 | 0.17 | 5.47 | 3.50 | 0.06 | 59.2 | 0.00 | 17 | 269 |
| 116 | gpu-discover: tree64 + quadtree | 272.6 | 0.16 | 5.23 | 3.61 | 0.10 | 53.1 | 0.01 | 205 | 285 |
| 117 | gpu-discover: svo_dag + quadtree | 269.8 | 0.16 | 5.88 | 3.64 | 0.09 | 53.1 | 0.01 | 199 | 281 |
| 118 | gpu-compute: rows + beam + incremental | 268.9 | 0.16 | 5.90 | 3.68 | 0.02 | 59.2 | 0.00 | 50 | 257 |
| 119 | gpu-compute: hier + beam + incremental | 267.4 | 0.16 | 5.36 | 3.71 | 0.02 | 59.2 | 0.00 | 48 | 257 |
| 120 | gpu-compute: prefix + incremental | 249.3 | 0.15 | 5.78 | 3.97 | 0.02 | 59.2 | 0.00 | 51 | 257 |
| 121 | gpu-compute: prefix + beam + incremental | 229.1 | 0.14 | 6.20 | 4.33 | 0.03 | 59.2 | 0.00 | 51 | 257 |
| 122 | gpu-compute: distance + incremental | 174.8 | 0.11 | 32.22 | 5.40 | 0.02 | 59.2 | 0.00 | 64 | 257 |
| 123 | gpu-compute: distance + beam + incremental | 161.7 | 0.10 | 33.98 | 5.86 | 0.02 | 59.2 | 0.00 | 64 | 257 |
| 124 | gpu-raster: prefix + cubes + visbuffer | 135.3 | 0.08 | 9.82 | 7.35 | 0.09 | 55.2 | 0.01 | 92 | 280 |
| 125 | gpu-raster: rows + cubes + visbuffer | 135.2 | 0.08 | 10.12 | 7.35 | 0.09 | 55.2 | 0.01 | 92 | 279 |
| 126 | gpu-raster: hier + cubes + visbuffer | 135.2 | 0.08 | 10.02 | 7.34 | 0.10 | 55.2 | 0.01 | 90 | 282 |
| 127 | gpu-raster: bricks + cubes + visbuffer | 135.1 | 0.08 | 10.19 | 7.36 | 0.09 | 55.2 | 0.01 | 90 | 280 |
| 128 | gpu-raster: hier + cubes + gbuffer | 135.1 | 0.08 | 9.91 | 7.36 | 0.09 | 55.0 | 0.01 | 90 | 281 |
| 129 | gpu-raster: bricks + cubes + gbuffer | 135.0 | 0.08 | 10.22 | 7.36 | 0.09 | 55.0 | 0.01 | 90 | 278 |
| 130 | gpu-raster: rows + cubes + gbuffer | 134.9 | 0.08 | 10.08 | 7.37 | 0.09 | 55.0 | 0.01 | 92 | 280 |
| 131 | gpu-raster: prefix + cubes + gbuffer | 134.8 | 0.08 | 10.16 | 7.37 | 0.08 | 55.0 | 0.01 | 92 | 280 |
| 132 | gpu-raster: svo_dag + cubes + visbuffer | 134.5 | 0.08 | 10.13 | 7.39 | 0.12 | 55.2 | 0.01 | 58 | 285 |
| 133 | gpu-raster: svo_dag + cubes + gbuffer | 134.0 | 0.08 | 10.24 | 7.42 | 0.12 | 55.0 | 0.01 | 58 | 285 |
| 134 | gpu-raster: tree64 + cubes + visbuffer | 133.8 | 0.08 | 10.23 | 7.43 | 0.12 | 55.2 | 0.01 | 64 | 285 |
| 135 | gpu-raster: clipmap + cubes + visbuffer | 133.6 | 0.08 | 10.23 | 7.44 | 0.09 | 55.2 | 0.01 | 375 | 288 |
| 136 | gpu-raster: tree64 + cubes + gbuffer | 133.4 | 0.08 | 10.18 | 7.45 | 0.13 | 55.0 | 0.01 | 64 | 285 |
| 137 | gpu-raster: clipmap + cubes + gbuffer | 133.4 | 0.08 | 10.17 | 7.46 | 0.09 | 55.0 | 0.01 | 375 | 287 |
| 138 | gpu-raster: distance + cubes + gbuffer | 129.7 | 0.08 | 32.89 | 7.36 | 0.08 | 55.0 | 0.01 | 106 | 278 |
| 139 | gpu-raster: distance + cubes + visbuffer | 129.6 | 0.08 | 33.86 | 7.34 | 0.10 | 55.2 | 0.01 | 106 | 278 |
| 140 | cpu: hier + refloor | 75.1 | 0.05 | 18.72 | - | 13.27 | 61.7 | 0.00 | - | 74 |
| 141 | gpu-raster: prefix + cubes + raster | 70.2 | 0.04 | 17.93 | 14.20 | 0.09 | 55.1 | 0.01 | 92 | 279 |
| 142 | gpu-raster: bricks + cubes + raster | 70.2 | 0.04 | 18.00 | 14.20 | 0.09 | 55.1 | 0.01 | 90 | 279 |
| 143 | gpu-raster: hier + cubes + raster | 70.2 | 0.04 | 17.75 | 14.20 | 0.10 | 55.1 | 0.01 | 90 | 281 |
| 144 | gpu-raster: rows + cubes + raster | 70.1 | 0.04 | 17.87 | 14.22 | 0.10 | 55.1 | 0.01 | 92 | 278 |
| 145 | gpu-raster: svo_dag + cubes + raster | 69.8 | 0.04 | 18.15 | 14.28 | 0.12 | 55.1 | 0.01 | 58 | 285 |
| 146 | gpu-raster: tree64 + cubes + raster | 69.7 | 0.04 | 18.04 | 14.30 | 0.13 | 55.1 | 0.01 | 64 | 286 |
| 147 | gpu-raster: clipmap + cubes + raster | 69.7 | 0.04 | 18.07 | 14.30 | 0.08 | 55.1 | 0.01 | 375 | 287 |
| 148 | gpu-raster: distance + cubes + raster | 69.4 | 0.04 | 32.97 | 14.19 | 0.09 | 55.1 | 0.01 | 106 | 279 |
| 149 | cpu: bricks + refloor | 67.0 | 0.04 | 21.02 | - | 14.88 | 61.7 | 0.00 | - | 72 |
| 150 | cpu: prefix + refloor | 66.8 | 0.04 | 21.23 | - | 14.91 | 61.7 | 0.00 | - | 54 |
| 151 | cpu: u8 + refloor | 65.6 | 0.04 | 21.63 | - | 15.20 | 61.7 | 0.00 | - | 59 |
| 152 | cpu: bits + refloor | 65.4 | 0.04 | 21.43 | - | 15.26 | 61.7 | 0.00 | - | 74 |
| 153 | cpu: morton + refloor | 65.3 | 0.04 | 21.50 | - | 15.24 | 61.7 | 0.00 | - | 72 |
| 154 | cpu: hier + incremental | 64.4 | 0.04 | 21.48 | - | 15.49 | 61.6 | 0.00 | - | 74 |
| 155 | cpu: palette + refloor | 64.4 | 0.04 | 22.96 | - | 15.45 | 61.7 | 0.00 | - | 52 |
| 156 | cpu: brickmajor + refloor | 64.1 | 0.04 | 22.15 | - | 15.53 | 61.7 | 0.00 | - | 72 |
| 157 | cpu: bricks + incremental | 61.8 | 0.04 | 21.80 | - | 16.13 | 61.6 | 0.00 | - | 72 |
| 158 | cpu: morton + incremental | 61.4 | 0.04 | 22.44 | - | 16.21 | 61.6 | 0.00 | - | 72 |
| 159 | cpu: u8 + incremental | 61.1 | 0.04 | 22.48 | - | 16.32 | 61.6 | 0.00 | - | 59 |
| 160 | cpu: palette + incremental | 58.8 | 0.04 | 23.45 | - | 16.93 | 61.6 | 0.00 | - | 52 |
| 161 | cpu: prefix + incremental | 56.6 | 0.03 | 24.25 | - | 17.59 | 61.6 | 0.00 | - | 54 |
| 162 | cpu: bits + incremental | 56.4 | 0.03 | 24.53 | - | 17.69 | 61.6 | 0.00 | - | 74 |
| 163 | cpu: brickmajor + incremental | 56.1 | 0.03 | 24.46 | - | 17.74 | 61.6 | 0.00 | - | 72 |
| 164 | cpu-discover: prefix + cluster | 22.3 | 0.01 | 86.90 | 1.76 | 44.47 | 44.7 | 0.02 | 52 | 395 |
| 165 | cpu-discover: morton + cluster | 22.0 | 0.01 | 86.21 | 1.76 | 45.06 | 44.7 | 0.02 | 52 | 431 |
| 166 | cpu-discover: bits + cluster | 21.8 | 0.01 | 95.54 | 1.76 | 45.45 | 44.7 | 0.02 | 52 | 395 |
| 167 | cpu-discover: hier + cluster | 21.7 | 0.01 | 91.38 | 1.76 | 45.66 | 44.7 | 0.02 | 52 | 386 |
| 168 | cpu-discover: bricks + cluster | 21.7 | 0.01 | 92.79 | 1.76 | 45.73 | 44.7 | 0.02 | 52 | 400 |
| 169 | cpu-discover: u8 + cluster | 21.6 | 0.01 | 93.40 | 1.76 | 45.95 | 44.7 | 0.02 | 52 | 417 |
| 170 | cpu-discover: brickmajor + cluster | 21.1 | 0.01 | 91.59 | 1.76 | 46.92 | 44.7 | 0.02 | 52 | 424 |
| 171 | cpu-discover: palette + cluster | 20.8 | 0.01 | 94.79 | 1.76 | 47.71 | 44.7 | 0.02 | 52 | 393 |
| 172 | cpu-raster: greedy + raster | 10.2 | 0.01 | 112.35 | - | 93.31 | 69.5 | 0.00 | - | 563 |
| 173 | cpu-raster: naive + raster | 7.4 | 0.00 | 155.98 | - | 128.39 | 75.1 | 0.00 | - | 857 |
| 174 | gpu-compute: hier + refloor + pathtrace (incorrect) | 746.1 | 0.45 | 3.23 | 1.26 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 175 | gpu-compute: bricks + refloor + pathtrace (incorrect) | 723.5 | 0.44 | 3.29 | 1.32 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 176 | gpu-compute: prefix + refloor + pathtrace (incorrect) | 718.3 | 0.43 | 3.16 | 1.33 | 0.01 | 32.4 | 5.79 | 82 | 257 |
| 177 | gpu-compute: rows + refloor + pathtrace (incorrect) | 709.0 | 0.43 | 3.08 | 1.34 | 0.01 | 32.4 | 5.79 | 82 | 257 |
| 178 | gpu-compute: distance + sphere + pathtrace (incorrect) | 703.0 | 0.43 | 31.19 | 0.95 | 0.01 | 32.4 | 5.79 | 96 | 257 |
| 179 | gpu-rt: greedy + rayquery + pathtrace (incorrect) | 682.3 | 0.41 | 19.43 | 0.95 | 0.47 | 32.4 | 5.79 | 319 | 585 |
| 180 | gpu-compute: hier + beam + refloor + pathtrace (incorrect) | 603.3 | 0.37 | 3.63 | 1.58 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 181 | gpu-compute: bricks + beam + refloor + pathtrace (incorrect) | 598.3 | 0.36 | 3.44 | 1.63 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 182 | gpu-compute: distance + refloor + pathtrace (incorrect) | 594.1 | 0.36 | 31.56 | 1.23 | 0.01 | 32.4 | 5.79 | 96 | 257 |
| 183 | gpu-compute: rows + beam + refloor + pathtrace (incorrect) | 585.9 | 0.35 | 3.74 | 1.64 | 0.01 | 32.4 | 5.79 | 82 | 257 |
| 184 | gpu-compute: svo_dag + refloor + pathtrace (incorrect) | 585.3 | 0.35 | 3.44 | 1.64 | 0.05 | 32.4 | 5.79 | 48 | 269 |
| 185 | gpu-compute: prefix + beam + refloor + pathtrace (incorrect) | 575.2 | 0.35 | 3.79 | 1.68 | 0.02 | 32.4 | 5.79 | 82 | 257 |
| 186 | gpu-compute: distance + beam + sphere + pathtrace (incorrect) | 571.5 | 0.35 | 31.73 | 1.29 | 0.01 | 32.4 | 5.79 | 96 | 257 |
| 187 | gpu-rt: naive + rayquery + pathtrace (incorrect) | 570.1 | 0.35 | 28.05 | 1.06 | 0.62 | 32.4 | 5.79 | 443 | 710 |
| 188 | gpu-compute: rows + rowrun + pathtrace (incorrect) | 566.1 | 0.34 | 3.68 | 1.71 | 0.01 | 32.4 | 5.79 | 82 | 257 |
| 189 | gpu-compute: clipmap + refloor + pathtrace (incorrect) | 560.0 | 0.34 | 3.39 | 1.74 | 0.01 | 32.4 | 5.79 | 365 | 258 |
| 190 | gpu-compute: prefix + rowrun + pathtrace (incorrect) | 559.5 | 0.34 | 3.47 | 1.74 | 0.01 | 32.4 | 5.79 | 82 | 257 |
| 191 | gpu-compute: tree64 + refloor + pathtrace (incorrect) | 518.8 | 0.31 | 3.77 | 1.88 | 0.05 | 32.4 | 5.79 | 54 | 278 |
| 192 | gpu-compute: distance + beam + refloor + pathtrace (incorrect) | 497.4 | 0.30 | 31.12 | 1.56 | 0.01 | 32.4 | 5.79 | 96 | 257 |
| 193 | gpu-compute: rows + beam + rowrun + pathtrace (incorrect) | 480.8 | 0.29 | 3.94 | 2.03 | 0.02 | 32.4 | 5.79 | 82 | 257 |
| 194 | gpu-compute: bricks + rowrun + pathtrace (incorrect) | 465.6 | 0.28 | 3.73 | 2.10 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 195 | gpu-compute: prefix + beam + rowrun + pathtrace (incorrect) | 464.3 | 0.28 | 4.10 | 2.11 | 0.01 | 32.4 | 5.79 | 82 | 257 |
| 196 | gpu-compute: distance + rowrun + pathtrace (incorrect) | 463.6 | 0.28 | 31.42 | 1.69 | 0.01 | 32.4 | 5.79 | 96 | 257 |
| 197 | gpu-compute: tree64 + beam + refloor + pathtrace (incorrect) | 420.4 | 0.25 | 4.19 | 2.33 | 0.06 | 32.4 | 5.79 | 54 | 273 |
| 198 | gpu-compute: distance + beam + rowrun + pathtrace (incorrect) | 408.1 | 0.25 | 32.59 | 2.02 | 0.01 | 32.4 | 5.79 | 96 | 257 |
| 199 | gpu-compute: clipmap + rowrun + pathtrace (incorrect) | 388.7 | 0.24 | 4.43 | 2.53 | 0.01 | 32.4 | 5.79 | 365 | 259 |
| 200 | gpu-rt: boxes + rayquery + pathtrace (incorrect) | 385.0 | 0.23 | 4.55 | 2.55 | 0.16 | 32.4 | 5.79 | 68 | 407 |
| 201 | gpu-compute: bricks + beam + rowrun + pathtrace (incorrect) | 383.6 | 0.23 | 4.32 | 2.57 | 0.02 | 32.4 | 5.79 | 80 | 257 |
| 202 | gpu-compute: svo_dag + beam + refloor + pathtrace (incorrect) | 382.1 | 0.23 | 4.18 | 2.56 | 0.06 | 32.4 | 5.79 | 48 | 270 |
| 203 | gpu-compute: clipmap + incremental + pathtrace (incorrect) | 367.8 | 0.22 | 4.44 | 2.67 | 0.01 | 32.4 | 5.79 | 365 | 259 |
| 204 | cpu-voxelspace: heightmap + columns (incorrect) | 366.1 | 0.22 | 4.55 | - | 2.64 | 14.8 | 27.66 | - | 82 |
| 205 | gpu-compute: hier + rowrun + pathtrace (incorrect) | 365.0 | 0.22 | 5.01 | 2.69 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 206 | gpu-splat: bricks + points + splat (incorrect) | 340.2 | 0.21 | 16.53 | 2.44 | 0.01 | 24.2 | 11.25 | 102 | 354 |
| 207 | gpu-splat: rows + points + splat (incorrect) | 338.4 | 0.20 | 15.13 | 2.44 | 0.01 | 24.2 | 11.25 | 104 | 354 |
| 208 | gpu-splat: prefix + points + splat (incorrect) | 329.6 | 0.20 | 14.64 | 2.54 | 0.01 | 24.2 | 11.25 | 104 | 355 |
| 209 | gpu-splat: tree64 + points + splat (incorrect) | 327.9 | 0.20 | 19.31 | 2.51 | 0.07 | 24.2 | 11.25 | 76 | 369 |
| 210 | gpu-compute: clipmap + beam + refloor + pathtrace (incorrect) | 323.9 | 0.20 | 5.04 | 3.04 | 0.01 | 32.4 | 5.79 | 365 | 259 |
| 211 | gpu-splat: hier + points + splat (incorrect) | 320.1 | 0.19 | 16.25 | 2.59 | 0.01 | 24.2 | 11.25 | 102 | 354 |
| 212 | gpu-splat: clipmap + points + splat (incorrect) | 317.9 | 0.19 | 15.85 | 2.65 | 0.01 | 24.2 | 11.25 | 387 | 355 |
| 213 | gpu-splat: svo_dag + points + splat (incorrect) | 317.5 | 0.19 | 18.76 | 2.66 | 0.06 | 24.2 | 11.25 | 70 | 403 |
| 214 | gpu-compute: tree64 + incremental + pathtrace (incorrect) | 314.5 | 0.19 | 4.88 | 3.14 | 0.07 | 32.4 | 5.79 | 54 | 272 |
| 215 | gpu-compute: hier + beam + rowrun + pathtrace (incorrect) | 311.4 | 0.19 | 5.24 | 3.17 | 0.02 | 32.4 | 5.79 | 80 | 257 |
| 216 | gpu-compute: hier + sphere + pathtrace (incorrect) | 308.0 | 0.19 | 5.00 | 3.21 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 217 | gpu-compute: bricks + sphere + pathtrace (incorrect) | 292.1 | 0.18 | 4.98 | 3.39 | 0.01 | 32.4 | 5.79 | 80 | 257 |
| 218 | gpu-splat: distance + points + splat (incorrect) | 290.8 | 0.18 | 40.68 | 2.53 | 0.01 | 24.2 | 11.25 | 118 | 356 |
| 219 | gpu-compute: rows + sphere + pathtrace (incorrect) | 286.9 | 0.17 | 5.63 | 3.43 | 0.02 | 32.4 | 5.79 | 82 | 257 |
| 220 | gpu-compute: hier + beam + sphere + pathtrace (incorrect) | 282.0 | 0.17 | 5.24 | 3.50 | 0.02 | 32.4 | 5.79 | 80 | 257 |
| 221 | gpu-compute: bricks + beam + sphere + pathtrace (incorrect) | 269.3 | 0.16 | 5.14 | 3.68 | 0.02 | 32.4 | 5.79 | 80 | 257 |
| 222 | gpu-compute: rows + beam + sphere + pathtrace (incorrect) | 267.6 | 0.16 | 5.78 | 3.68 | 0.02 | 32.4 | 5.79 | 82 | 257 |
| 223 | gpu-compute: clipmap + beam + rowrun + pathtrace (incorrect) | 261.7 | 0.16 | 5.87 | 3.78 | 0.02 | 32.4 | 5.79 | 365 | 259 |
| 224 | gpu-compute: tree64 + beam + incremental + pathtrace (incorrect) | 258.1 | 0.16 | 5.95 | 3.84 | 0.06 | 32.4 | 5.79 | 54 | 274 |
| 225 | gpu-compute: prefix + sphere + pathtrace (incorrect) | 257.3 | 0.16 | 6.24 | 3.85 | 0.03 | 32.4 | 5.79 | 82 | 257 |
| 226 | gpu-compute: clipmap + beam + incremental + pathtrace (incorrect) | 251.2 | 0.15 | 6.09 | 3.94 | 0.02 | 32.4 | 5.79 | 365 | 259 |
| 227 | gpu-compute: clipmap + sphere + pathtrace (incorrect) | 249.5 | 0.15 | 5.69 | 3.97 | 0.02 | 32.4 | 5.79 | 365 | 259 |
| 228 | gpu-compute: prefix + beam + sphere + pathtrace (incorrect) | 235.9 | 0.14 | 5.77 | 4.20 | 0.02 | 32.4 | 5.79 | 82 | 257 |
| 229 | gpu-compute: svo_dag + sphere + pathtrace (incorrect) | 220.1 | 0.13 | 6.03 | 4.51 | 0.06 | 32.4 | 5.79 | 48 | 269 |
| 230 | gpu-compute: tree64 + sphere + pathtrace (incorrect) | 195.7 | 0.12 | 12.99 | 5.05 | 0.07 | 32.4 | 5.79 | 54 | 273 |
| 231 | gpu-compute: svo_dag + rowrun + pathtrace (incorrect) | 189.7 | 0.11 | 7.06 | 5.22 | 0.07 | 32.4 | 5.79 | 48 | 269 |
| 232 | gpu-compute: clipmap + beam + sphere + pathtrace (incorrect) | 187.9 | 0.11 | 7.17 | 5.29 | 0.02 | 32.4 | 5.79 | 365 | 259 |
| 233 | gpu-compute: tree64 + beam + sphere + pathtrace (incorrect) | 187.4 | 0.11 | 7.22 | 5.30 | 0.07 | 32.4 | 5.79 | 54 | 273 |
| 234 | gpu-compute: svo_dag + beam + sphere + pathtrace (incorrect) | 183.2 | 0.11 | 7.13 | 5.41 | 0.07 | 32.4 | 5.79 | 48 | 270 |
| 235 | gpu-compute: svo_dag + incremental + pathtrace (incorrect) | 163.8 | 0.10 | 8.38 | 6.07 | 0.07 | 32.4 | 5.79 | 48 | 269 |
| 236 | gpu-compute: tree64 + rowrun + pathtrace (incorrect) | 163.7 | 0.10 | 8.06 | 6.07 | 0.07 | 32.4 | 5.79 | 54 | 275 |
| 237 | gpu-compute: svo_dag + beam + rowrun + pathtrace (incorrect) | 162.3 | 0.10 | 8.68 | 6.13 | 0.06 | 32.4 | 5.79 | 48 | 269 |
| 238 | gpu-compute: tree64 + beam + rowrun + pathtrace (incorrect) | 161.8 | 0.10 | 8.19 | 6.14 | 0.08 | 32.4 | 5.79 | 54 | 274 |
| 239 | gpu-compute: rows + beam + incremental + pathtrace (incorrect) | 141.9 | 0.09 | 9.34 | 7.00 | 0.02 | 32.4 | 5.79 | 82 | 257 |
| 240 | gpu-compute: rows + incremental + pathtrace (incorrect) | 141.7 | 0.09 | 9.55 | 7.03 | 0.04 | 32.4 | 5.79 | 82 | 257 |
| 241 | gpu-compute: bricks + beam + incremental + pathtrace (incorrect) | 137.7 | 0.08 | 9.66 | 7.22 | 0.03 | 32.4 | 5.79 | 80 | 257 |
| 242 | gpu-compute: svo_dag + beam + incremental + pathtrace (incorrect) | 137.3 | 0.08 | 9.41 | 7.25 | 0.07 | 32.4 | 5.79 | 48 | 269 |
| 243 | gpu-compute: bricks + incremental + pathtrace (incorrect) | 134.9 | 0.08 | 9.70 | 7.38 | 0.03 | 32.4 | 5.79 | 80 | 257 |
| 244 | gpu-compute: hier + beam + incremental + pathtrace (incorrect) | 134.3 | 0.08 | 9.74 | 7.41 | 0.03 | 32.4 | 5.79 | 80 | 262 |
| 245 | gpu-compute: hier + incremental + pathtrace (incorrect) | 133.2 | 0.08 | 10.00 | 7.47 | 0.03 | 32.4 | 5.79 | 80 | 262 |
| 246 | gpu-compute: prefix + beam + incremental + pathtrace (incorrect) | 118.7 | 0.07 | 10.84 | 8.39 | 0.03 | 32.4 | 5.79 | 82 | 257 |
| 247 | gpu-compute: prefix + incremental + pathtrace (incorrect) | 115.6 | 0.07 | 11.30 | 8.60 | 0.03 | 32.4 | 5.79 | 82 | 262 |
| 248 | gpu-compute: distance + beam + incremental + pathtrace (incorrect) | 94.2 | 0.06 | 32.76 | 10.43 | 0.03 | 32.4 | 5.79 | 96 | 257 |
| 249 | gpu-voxelspace: heightmap + columns (incorrect) | 91.7 | 0.06 | 17.09 | 10.87 | 0.02 | 14.8 | 27.66 | 12 | 259 |
| 250 | gpu-compute: distance + incremental + pathtrace (incorrect) | 91.2 | 0.06 | 32.51 | 10.80 | 0.02 | 32.4 | 5.79 | 96 | 258 |
| 251 | cpu-discover: hier + quadtree (incorrect) | 59.4 | 0.04 | 33.24 | 1.70 | 16.40 | 24.9 | 0.95 | 52 | 526 |
| 252 | cpu-discover: u8 + quadtree (incorrect) | 58.8 | 0.04 | 33.37 | 1.70 | 16.58 | 24.9 | 0.95 | 52 | 558 |
| 253 | cpu-discover: brickmajor + quadtree (incorrect) | 58.5 | 0.04 | 33.39 | 1.70 | 16.68 | 24.9 | 0.95 | 52 | 558 |
| 254 | cpu-discover: bits + quadtree (incorrect) | 58.4 | 0.04 | 34.23 | 1.71 | 16.73 | 24.9 | 0.95 | 52 | 536 |
| 255 | cpu-discover: prefix + quadtree (incorrect) | 58.4 | 0.04 | 34.84 | 1.70 | 16.69 | 24.9 | 0.95 | 52 | 548 |
| 256 | cpu-discover: bricks + quadtree (incorrect) | 58.1 | 0.04 | 34.88 | 1.71 | 16.80 | 24.9 | 0.95 | 52 | 548 |
| 257 | cpu-discover: palette + quadtree (incorrect) | 57.9 | 0.04 | 35.60 | 1.70 | 16.79 | 24.9 | 0.95 | 52 | 529 |
| 258 | cpu-discover: morton + quadtree (incorrect) | 57.6 | 0.03 | 33.79 | 1.70 | 16.92 | 24.9 | 0.95 | 52 | 566 |

### menger [unlit]

1920×1080. camera 3.46 blocks/frame (max 3.5), turn 0.56°/frame; 312 chunks at start, 0.8 loads/frame; chunk fill 2%, solid 0.4%, surface 0.40%; sky 72%, hits at 259 blocks (median 248); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: hier + refloor | 2486.9 | 1.00 | 3.05 | 0.29 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 2 | gpu-compute: bricks + refloor | 2373.8 | 0.95 | 3.14 | 0.31 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 3 | gpu-compute: prefix + refloor | 2317.5 | 0.93 | 3.38 | 0.31 | 0.01 | 55.7 | 0.01 | 51 | 257 |
| 4 | gpu-compute: rows + refloor | 2312.7 | 0.93 | 3.47 | 0.31 | 0.01 | 55.7 | 0.01 | 50 | 257 |
| 5 | gpu-compute: rows + rowrun | 2083.8 | 0.84 | 3.17 | 0.37 | 0.01 | 55.7 | 0.01 | 50 | 257 |
| 6 | gpu-compute: svo_dag + refloor | 2082.0 | 0.84 | 3.82 | 0.36 | 0.05 | 55.7 | 0.01 | 17 | 269 |
| 7 | gpu-compute: clipmap + refloor | 2039.8 | 0.82 | 2.70 | 0.40 | 0.01 | 55.7 | 0.01 | 334 | 259 |
| 8 | gpu-compute: prefix + rowrun | 1926.8 | 0.77 | 3.12 | 0.41 | 0.01 | 55.7 | 0.01 | 51 | 257 |
| 9 | gpu-compute: tree64 + refloor | 1826.3 | 0.73 | 3.10 | 0.42 | 0.05 | 55.7 | 0.01 | 23 | 273 |
| 10 | gpu-compute: bricks + rowrun | 1786.1 | 0.72 | 2.28 | 0.47 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 11 | gpu-compute: clipmap + incremental | 1615.3 | 0.65 | 2.33 | 0.53 | 0.01 | 55.7 | 0.01 | 334 | 259 |
| 12 | gpu-compute: clipmap + rowrun | 1597.9 | 0.64 | 2.38 | 0.53 | 0.01 | 55.7 | 0.01 | 334 | 259 |
| 13 | gpu-rt: boxes + rayquery | 1413.2 | 0.57 | 3.36 | 0.60 | 0.10 | 55.7 | 0.01 | 37 | 405 |
| 14 | gpu-compute: hier + beam + refloor | 1369.8 | 0.55 | 3.16 | 0.63 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 15 | gpu-compute: bricks + beam + refloor | 1365.5 | 0.55 | 2.52 | 0.65 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 16 | gpu-compute: distance + sphere | 1362.6 | 0.55 | 32.30 | 0.21 | 0.01 | 55.7 | 0.01 | 64 | 257 |
| 17 | gpu-compute: hier + rowrun | 1361.3 | 0.55 | 2.81 | 0.64 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 18 | gpu-compute: tree64 + incremental | 1339.4 | 0.54 | 2.79 | 0.64 | 0.05 | 55.7 | 0.01 | 23 | 274 |
| 19 | gpu-compute: rows + beam + refloor | 1312.8 | 0.53 | 2.75 | 0.66 | 0.01 | 55.6 | 0.01 | 50 | 257 |
| 20 | gpu-compute: prefix + beam + refloor | 1310.7 | 0.53 | 2.50 | 0.66 | 0.01 | 55.6 | 0.01 | 51 | 257 |
| 21 | gpu-compute: distance + refloor | 1274.8 | 0.51 | 32.22 | 0.27 | 0.01 | 55.7 | 0.01 | 64 | 257 |
| 22 | gpu-compute: rows + beam + rowrun | 1233.6 | 0.50 | 2.79 | 0.71 | 0.01 | 55.6 | 0.01 | 50 | 257 |
| 23 | gpu-compute: hier + sphere | 1203.8 | 0.48 | 2.72 | 0.74 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 24 | gpu-compute: prefix + beam + rowrun | 1191.7 | 0.48 | 2.53 | 0.75 | 0.01 | 55.6 | 0.01 | 51 | 257 |
| 25 | gpu-compute: bricks + sphere | 1131.9 | 0.46 | 2.61 | 0.80 | 0.01 | 55.7 | 0.01 | 48 | 256 |
| 26 | gpu-compute: distance + rowrun | 1127.6 | 0.45 | 32.43 | 0.36 | 0.01 | 55.7 | 0.01 | 64 | 257 |
| 27 | gpu-compute: bricks + beam + rowrun | 1127.6 | 0.45 | 2.55 | 0.81 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 28 | gpu-compute: rows + sphere | 1125.1 | 0.45 | 2.76 | 0.80 | 0.01 | 55.7 | 0.01 | 50 | 257 |
| 29 | gpu-compute: prefix + sphere | 997.1 | 0.40 | 2.81 | 0.92 | 0.01 | 55.7 | 0.01 | 51 | 257 |
| 30 | gpu-compute: tree64 + beam + refloor | 964.2 | 0.39 | 2.87 | 0.94 | 0.05 | 55.7 | 0.01 | 23 | 273 |
| 31 | gpu-compute: distance + beam + sphere | 953.6 | 0.38 | 32.71 | 0.56 | 0.01 | 55.5 | 0.01 | 64 | 257 |
| 32 | gpu-compute: clipmap + sphere | 944.7 | 0.38 | 3.04 | 0.98 | 0.01 | 55.7 | 0.01 | 334 | 259 |
| 33 | gpu-raster: bricks + greedy + raster | 932.0 | 0.37 | 7.43 | 0.88 | 0.14 | 51.5 | 0.01 | 122 | 286 |
| 34 | gpu-compute: hier + beam + rowrun | 926.3 | 0.37 | 3.20 | 1.00 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 35 | gpu-rt: greedy + rayquery | 907.4 | 0.36 | 29.86 | 0.54 | 0.47 | 55.7 | 0.01 | 288 | 586 |
| 36 | gpu-compute: distance + beam + refloor | 893.8 | 0.36 | 31.29 | 0.62 | 0.01 | 55.6 | 0.01 | 64 | 257 |
| 37 | gpu-raster: bricks + greedy + visbuffer | 891.4 | 0.36 | 7.72 | 0.92 | 0.14 | 51.4 | 0.01 | 122 | 286 |
| 38 | gpu-compute: svo_dag + sphere | 884.5 | 0.36 | 3.50 | 1.04 | 0.05 | 55.7 | 0.01 | 17 | 269 |
| 39 | gpu-compute: hier + beam + sphere | 859.3 | 0.35 | 3.10 | 1.09 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 40 | gpu-compute: distance + beam + rowrun | 836.4 | 0.34 | 30.93 | 0.71 | 0.01 | 55.6 | 0.01 | 64 | 257 |
| 41 | gpu-compute: tree64 + sphere | 835.7 | 0.34 | 3.08 | 1.11 | 0.06 | 55.7 | 0.01 | 23 | 273 |
| 42 | gpu-compute: bricks + beam + sphere | 834.0 | 0.34 | 2.87 | 1.14 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 43 | gpu-compute: rows + beam + sphere | 815.5 | 0.33 | 3.26 | 1.14 | 0.01 | 55.6 | 0.01 | 50 | 257 |
| 44 | gpu-compute: tree64 + beam + incremental | 808.1 | 0.32 | 3.06 | 1.16 | 0.05 | 55.7 | 0.01 | 23 | 274 |
| 45 | gpu-compute: prefix + beam + sphere | 752.2 | 0.30 | 3.24 | 1.26 | 0.01 | 55.6 | 0.01 | 51 | 257 |
| 46 | gpu-compute: svo_dag + incremental | 722.2 | 0.29 | 3.13 | 1.30 | 0.05 | 55.7 | 0.01 | 17 | 269 |
| 47 | gpu-discover: rows + quadtree | 704.3 | 0.28 | 3.26 | 1.32 | 0.04 | 52.0 | 0.01 | 232 | 267 |
| 48 | gpu-rt: naive + rayquery | 703.0 | 0.28 | 43.26 | 0.64 | 0.65 | 55.8 | 0.01 | 411 | 709 |
| 49 | gpu-compute: svo_dag + beam + refloor | 692.9 | 0.28 | 3.69 | 1.36 | 0.05 | 55.6 | 0.01 | 17 | 269 |
| 50 | gpu-discover: hier + quadtree | 688.1 | 0.28 | 2.99 | 1.35 | 0.03 | 52.0 | 0.01 | 230 | 268 |
| 51 | gpu-raster: bricks + naive + raster | 680.4 | 0.27 | 4.72 | 1.35 | 0.08 | 52.6 | 0.01 | 122 | 302 |
| 52 | gpu-discover: prefix + quadtree | 680.3 | 0.27 | 3.28 | 1.37 | 0.03 | 52.0 | 0.01 | 233 | 266 |
| 53 | gpu-raster: bricks + naive + visbuffer | 661.7 | 0.27 | 4.96 | 1.40 | 0.08 | 52.6 | 0.01 | 122 | 303 |
| 54 | gpu-raster: bricks + naive + gbuffer | 661.0 | 0.27 | 4.18 | 1.40 | 0.09 | 52.4 | 0.01 | 122 | 304 |
| 55 | gpu-discover: bricks + quadtree | 634.7 | 0.26 | 3.24 | 1.48 | 0.03 | 52.0 | 0.01 | 230 | 267 |
| 56 | gpu-compute: tree64 + rowrun | 615.0 | 0.25 | 3.58 | 1.56 | 0.05 | 55.7 | 0.01 | 23 | 273 |
| 57 | gpu-compute: rows + incremental | 603.1 | 0.24 | 3.51 | 1.59 | 0.01 | 55.7 | 0.01 | 50 | 257 |
| 58 | gpu-compute: bricks + incremental | 602.9 | 0.24 | 3.44 | 1.60 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 59 | gpu-discover: clipmap + quadtree | 602.0 | 0.24 | 3.32 | 1.57 | 0.04 | 52.0 | 0.01 | 516 | 271 |
| 60 | gpu-compute: tree64 + beam + sphere | 578.5 | 0.23 | 3.39 | 1.67 | 0.06 | 55.7 | 0.01 | 23 | 273 |
| 61 | gpu-compute: clipmap + beam + refloor | 574.8 | 0.23 | 3.45 | 1.69 | 0.01 | 55.6 | 0.01 | 334 | 258 |
| 62 | gpu-raster: bricks + greedy + gbuffer | 574.0 | 0.23 | 7.91 | 1.57 | 0.15 | 51.3 | 0.01 | 122 | 288 |
| 63 | gpu-compute: svo_dag + rowrun | 566.7 | 0.23 | 4.05 | 1.69 | 0.06 | 55.7 | 0.01 | 17 | 268 |
| 64 | gpu-discover: tree64 + quadtree | 551.5 | 0.22 | 3.64 | 1.71 | 0.08 | 52.0 | 0.01 | 205 | 285 |
| 65 | gpu-compute: hier + incremental | 543.1 | 0.22 | 3.69 | 1.78 | 0.01 | 55.7 | 0.01 | 48 | 257 |
| 66 | gpu-discover: distance + quadtree | 537.5 | 0.22 | 31.63 | 1.36 | 0.04 | 52.0 | 0.01 | 246 | 266 |
| 67 | gpu-compute: clipmap + beam + rowrun | 535.1 | 0.22 | 3.78 | 1.82 | 0.01 | 55.6 | 0.01 | 334 | 259 |
| 68 | gpu-compute: clipmap + beam + incremental | 530.7 | 0.21 | 3.63 | 1.84 | 0.01 | 55.6 | 0.01 | 334 | 259 |
| 69 | gpu-discover: svo_dag + quadtree | 529.1 | 0.21 | 3.68 | 1.79 | 0.08 | 52.0 | 0.01 | 199 | 278 |
| 70 | gpu-compute: bricks + beam + incremental | 508.0 | 0.20 | 3.82 | 1.92 | 0.01 | 55.6 | 0.01 | 48 | 257 |
| 71 | gpu-compute: rows + beam + incremental | 490.1 | 0.20 | 4.12 | 1.99 | 0.01 | 55.6 | 0.01 | 50 | 257 |
| 72 | gpu-compute: prefix + incremental | 477.4 | 0.19 | 3.66 | 2.04 | 0.01 | 55.7 | 0.01 | 51 | 257 |
| 73 | gpu-compute: svo_dag + beam + sphere | 474.0 | 0.19 | 3.69 | 2.04 | 0.06 | 55.6 | 0.01 | 17 | 269 |
| 74 | gpu-compute: tree64 + beam + rowrun | 462.9 | 0.19 | 4.16 | 2.10 | 0.06 | 55.7 | 0.01 | 23 | 274 |
| 75 | gpu-compute: hier + beam + incremental | 460.2 | 0.19 | 4.63 | 2.12 | 0.02 | 55.6 | 0.01 | 48 | 257 |
| 76 | gpu-compute: clipmap + beam + sphere | 430.0 | 0.17 | 4.04 | 2.28 | 0.01 | 55.6 | 0.01 | 334 | 259 |
| 77 | gpu-compute: svo_dag + beam + incremental | 421.7 | 0.17 | 3.88 | 2.32 | 0.05 | 55.6 | 0.01 | 17 | 269 |
| 78 | gpu-compute: prefix + beam + incremental | 412.1 | 0.17 | 4.20 | 2.38 | 0.02 | 55.6 | 0.01 | 51 | 257 |
| 79 | gpu-compute: svo_dag + beam + rowrun | 368.7 | 0.15 | 5.30 | 2.65 | 0.06 | 55.6 | 0.01 | 17 | 269 |
| 80 | gpu-compute: distance + incremental | 296.0 | 0.12 | 33.26 | 2.94 | 0.02 | 55.7 | 0.01 | 64 | 257 |
| 81 | gpu-compute: distance + beam + incremental | 269.5 | 0.11 | 32.02 | 3.30 | 0.02 | 55.6 | 0.01 | 64 | 258 |
| 82 | gpu-raster: bricks + cubes + raster | 140.3 | 0.06 | 9.78 | 7.08 | 0.08 | 52.5 | 0.01 | 90 | 279 |
| 83 | gpu-raster: bricks + cubes + visbuffer | 139.5 | 0.06 | 9.76 | 7.13 | 0.09 | 52.4 | 0.01 | 90 | 279 |
| 84 | gpu-raster: bricks + cubes + gbuffer | 139.1 | 0.06 | 9.96 | 7.14 | 0.09 | 52.2 | 0.01 | 90 | 279 |
| 85 | cpu: hier + refloor | 137.7 | 0.06 | 11.21 | - | 7.23 | 59.8 | 0.00 | - | 74 |
| 86 | cpu: bricks + refloor | 117.4 | 0.05 | 12.76 | - | 8.48 | 59.8 | 0.00 | - | 72 |
| 87 | cpu: prefix + refloor | 115.5 | 0.05 | 13.17 | - | 8.59 | 59.8 | 0.00 | - | 54 |
| 88 | cpu: morton + refloor | 113.8 | 0.05 | 13.32 | - | 8.73 | 59.8 | 0.00 | - | 72 |
| 89 | cpu: u8 + refloor | 113.6 | 0.05 | 13.13 | - | 8.76 | 59.8 | 0.00 | - | 59 |
| 90 | cpu: brickmajor + refloor | 113.5 | 0.05 | 13.37 | - | 8.75 | 59.8 | 0.00 | - | 72 |
| 91 | cpu: bits + refloor | 112.6 | 0.05 | 13.54 | - | 8.84 | 59.8 | 0.00 | - | 74 |
| 92 | cpu: palette + refloor | 109.7 | 0.04 | 13.45 | - | 9.03 | 59.8 | 0.00 | - | 52 |
| 93 | cpu: hier + incremental | 108.3 | 0.04 | 13.55 | - | 9.19 | 59.7 | 0.00 | - | 74 |
| 94 | cpu: bricks + incremental | 103.3 | 0.04 | 13.90 | - | 9.64 | 59.7 | 0.00 | - | 72 |
| 95 | cpu: bits + incremental | 102.6 | 0.04 | 14.31 | - | 9.70 | 59.7 | 0.00 | - | 74 |
| 96 | cpu: u8 + incremental | 101.6 | 0.04 | 14.51 | - | 9.81 | 59.7 | 0.00 | - | 59 |
| 97 | cpu: brickmajor + incremental | 100.6 | 0.04 | 13.75 | - | 9.87 | 59.7 | 0.00 | - | 72 |
| 98 | cpu: morton + incremental | 100.6 | 0.04 | 14.59 | - | 9.88 | 59.7 | 0.00 | - | 72 |
| 99 | cpu: palette + incremental | 97.7 | 0.04 | 14.69 | - | 10.15 | 59.7 | 0.00 | - | 52 |
| 100 | cpu: prefix + incremental | 95.4 | 0.04 | 14.76 | - | 10.41 | 59.7 | 0.00 | - | 54 |
| 101 | cpu-discover: u8 + cluster | 22.3 | 0.01 | 100.40 | 0.12 | 44.54 | 47.4 | 0.02 | 52 | 422 |
| 102 | cpu-discover: bits + cluster | 22.1 | 0.01 | 98.17 | 0.12 | 44.90 | 47.4 | 0.02 | 52 | 394 |
| 103 | cpu-discover: brickmajor + cluster | 22.1 | 0.01 | 99.20 | 0.12 | 44.91 | 47.4 | 0.02 | 52 | 426 |
| 104 | cpu-discover: prefix + cluster | 22.0 | 0.01 | 100.97 | 0.12 | 45.08 | 47.4 | 0.02 | 52 | 395 |
| 105 | cpu-discover: hier + cluster | 21.9 | 0.01 | 100.17 | 0.12 | 45.22 | 47.4 | 0.02 | 52 | 405 |
| 106 | cpu-discover: morton + cluster | 21.8 | 0.01 | 101.11 | 0.12 | 45.57 | 47.4 | 0.02 | 52 | 433 |
| 107 | cpu-discover: bricks + cluster | 21.6 | 0.01 | 102.04 | 0.12 | 45.87 | 47.4 | 0.02 | 52 | 388 |
| 108 | cpu-discover: palette + cluster | 21.3 | 0.01 | 101.12 | 0.12 | 46.44 | 47.4 | 0.02 | 52 | 415 |
| 109 | cpu-raster: greedy + raster | 11.0 | 0.00 | 105.16 | - | 86.17 | 68.3 | 0.00 | - | 581 |
| 110 | cpu-raster: naive + raster | 7.9 | 0.00 | 146.11 | - | 120.01 | 74.6 | 0.00 | - | 838 |
| 111 | cpu-voxelspace: heightmap + columns (incorrect) | 683.2 | 0.27 | 2.82 | - | 1.37 | 15.8 | 28.55 | - | 82 |
| 112 | gpu-splat: bricks + points + splat (incorrect) | 366.0 | 0.15 | 16.24 | 2.21 | 0.01 | 21.8 | 10.44 | 102 | 354 |
| 113 | gpu-voxelspace: heightmap + columns (incorrect) | 190.6 | 0.08 | 9.06 | 5.21 | 0.02 | 15.8 | 28.55 | 12 | 259 |
| 114 | cpu-discover: u8 + quadtree (incorrect) | 59.1 | 0.02 | 36.19 | 0.12 | 16.52 | 29.0 | 0.96 | 52 | 550 |
| 115 | cpu-discover: prefix + quadtree (incorrect) | 59.1 | 0.02 | 36.30 | 0.12 | 16.54 | 29.0 | 0.96 | 52 | 538 |
| 116 | cpu-discover: hier + quadtree (incorrect) | 59.0 | 0.02 | 35.61 | 0.12 | 16.60 | 29.0 | 0.96 | 52 | 552 |
| 117 | cpu-discover: morton + quadtree (incorrect) | 58.9 | 0.02 | 36.31 | 0.12 | 16.57 | 29.0 | 0.96 | 52 | 569 |
| 118 | cpu-discover: brickmajor + quadtree (incorrect) | 58.6 | 0.02 | 36.06 | 0.12 | 16.66 | 29.0 | 0.96 | 52 | 559 |
| 119 | cpu-discover: bricks + quadtree (incorrect) | 58.6 | 0.02 | 36.39 | 0.12 | 16.70 | 29.0 | 0.96 | 52 | 537 |
| 120 | cpu-discover: palette + quadtree (incorrect) | 58.2 | 0.02 | 36.60 | 0.12 | 16.78 | 29.0 | 0.96 | 52 | 547 |
| 121 | cpu-discover: bits + quadtree (incorrect) | 58.1 | 0.02 | 37.26 | 0.12 | 16.83 | 29.0 | 0.96 | 52 | 553 |

### meshes [lit+meshes]

1920×1080. camera 0.00 blocks/frame (max 0.0), turn 0.00°/frame; 27 chunks at start, 0.0 loads/frame; chunk fill 4%, solid 0.1%, surface 0.06%; sky 47%, hits at 51 blocks (median 43); 0.0 edits/frame; 2 lights; 5520 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-raster: prefix + greedy + raster | 3592.4 | 1.00 | 2.00 | 0.16 | 0.09 | 71.1 | 0.00 | 31 | 217 |
| 2 | gpu-raster: svo_dag + greedy + raster | 3406.2 | 0.95 | 1.97 | 0.17 | 0.10 | 71.1 | 0.00 | 26 | 218 |
| 3 | gpu-raster: hier + greedy + raster | 3357.0 | 0.93 | 1.76 | 0.19 | 0.09 | 71.1 | 0.00 | 30 | 218 |
| 4 | gpu-raster: tree64 + greedy + raster | 3182.2 | 0.89 | 1.82 | 0.21 | 0.10 | 71.1 | 0.00 | 26 | 217 |
| 5 | gpu-raster: distance + greedy + raster | 3135.0 | 0.87 | 1.78 | 0.21 | 0.09 | 71.1 | 0.00 | 32 | 217 |
| 6 | gpu-raster: rows + greedy + raster | 3123.4 | 0.87 | 1.84 | 0.21 | 0.08 | 71.1 | 0.00 | 30 | 217 |
| 7 | gpu-raster: bricks + greedy + raster | 3120.6 | 0.87 | 1.97 | 0.20 | 0.10 | 71.1 | 0.00 | 30 | 218 |
| 8 | gpu-raster: svo_dag + greedy + visbuffer | 2970.7 | 0.83 | 1.95 | 0.23 | 0.09 | 71.1 | 0.00 | 26 | 217 |
| 9 | gpu-raster: svo_dag + naive + raster | 2847.6 | 0.79 | 1.97 | 0.25 | 0.10 | 61.1 | 0.00 | 26 | 217 |
| 10 | gpu-raster: prefix + greedy + visbuffer | 2834.1 | 0.79 | 1.85 | 0.24 | 0.10 | 71.1 | 0.00 | 31 | 217 |
| 11 | gpu-raster: prefix + naive + visbuffer | 2811.2 | 0.78 | 2.13 | 0.25 | 0.09 | 61.1 | 0.00 | 31 | 217 |
| 12 | gpu-raster: hier + greedy + visbuffer | 2791.2 | 0.78 | 2.01 | 0.25 | 0.10 | 71.1 | 0.00 | 30 | 217 |
| 13 | gpu-raster: tree64 + naive + raster | 2769.5 | 0.77 | 2.11 | 0.25 | 0.09 | 61.1 | 0.00 | 26 | 217 |
| 14 | gpu-raster: clipmap + greedy + raster | 2755.8 | 0.77 | 1.92 | 0.25 | 0.10 | 71.1 | 0.00 | 49 | 218 |
| 15 | gpu-raster: hier + naive + visbuffer | 2742.4 | 0.76 | 2.23 | 0.26 | 0.10 | 61.1 | 0.00 | 30 | 217 |
| 16 | gpu-raster: hier + naive + raster | 2738.1 | 0.76 | 2.00 | 0.26 | 0.10 | 61.1 | 0.00 | 30 | 217 |
| 17 | gpu-raster: svo_dag + naive + visbuffer | 2727.8 | 0.76 | 2.01 | 0.26 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 18 | gpu-raster: tree64 + greedy + visbuffer | 2707.0 | 0.75 | 2.05 | 0.26 | 0.10 | 71.1 | 0.00 | 26 | 218 |
| 19 | gpu-raster: prefix + naive + raster | 2706.1 | 0.75 | 1.89 | 0.27 | 0.09 | 61.1 | 0.00 | 31 | 218 |
| 20 | gpu-raster: tree64 + naive + visbuffer | 2621.9 | 0.73 | 2.21 | 0.27 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 21 | gpu-raster: rows + greedy + visbuffer | 2613.1 | 0.73 | 2.13 | 0.28 | 0.09 | 71.1 | 0.00 | 30 | 217 |
| 22 | gpu-raster: rows + naive + raster | 2596.5 | 0.72 | 1.76 | 0.29 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 23 | gpu-raster: distance + greedy + visbuffer | 2595.8 | 0.72 | 1.89 | 0.28 | 0.10 | 71.1 | 0.00 | 32 | 218 |
| 24 | gpu-raster: distance + naive + raster | 2548.3 | 0.71 | 2.15 | 0.28 | 0.08 | 61.1 | 0.00 | 32 | 218 |
| 25 | gpu-raster: prefix + greedy + gbuffer | 2546.6 | 0.71 | 2.13 | 0.29 | 0.10 | 71.1 | 0.00 | 31 | 218 |
| 26 | gpu-raster: distance + naive + visbuffer | 2532.0 | 0.70 | 2.32 | 0.28 | 0.10 | 61.1 | 0.00 | 32 | 218 |
| 27 | gpu-raster: clipmap + greedy + visbuffer | 2518.9 | 0.70 | 2.08 | 0.29 | 0.10 | 71.1 | 0.00 | 49 | 217 |
| 28 | gpu-raster: tree64 + greedy + gbuffer | 2512.9 | 0.70 | 1.97 | 0.29 | 0.10 | 71.1 | 0.00 | 26 | 218 |
| 29 | gpu-raster: svo_dag + cubes + visbuffer | 2503.6 | 0.70 | 2.15 | 0.29 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 30 | gpu-raster: prefix + cubes + visbuffer | 2484.0 | 0.69 | 2.01 | 0.29 | 0.11 | 61.1 | 0.00 | 31 | 217 |
| 31 | gpu-raster: bricks + greedy + visbuffer | 2451.4 | 0.68 | 2.21 | 0.29 | 0.10 | 71.1 | 0.00 | 30 | 218 |
| 32 | gpu-raster: bricks + naive + visbuffer | 2433.8 | 0.68 | 2.00 | 0.30 | 0.10 | 61.1 | 0.00 | 30 | 217 |
| 33 | gpu-raster: bricks + naive + raster | 2417.5 | 0.67 | 2.24 | 0.31 | 0.09 | 61.1 | 0.00 | 30 | 218 |
| 34 | gpu-raster: clipmap + naive + visbuffer | 2415.3 | 0.67 | 2.02 | 0.31 | 0.09 | 61.1 | 0.00 | 49 | 218 |
| 35 | gpu-raster: hier + cubes + visbuffer | 2393.1 | 0.67 | 2.11 | 0.31 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 36 | gpu-raster: prefix + cubes + raster | 2389.2 | 0.67 | 1.86 | 0.32 | 0.09 | 61.1 | 0.00 | 31 | 218 |
| 37 | gpu-raster: tree64 + cubes + visbuffer | 2386.8 | 0.66 | 2.08 | 0.31 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 38 | gpu-raster: svo_dag + greedy + gbuffer | 2380.0 | 0.66 | 2.19 | 0.32 | 0.10 | 71.1 | 0.00 | 26 | 218 |
| 39 | gpu-raster: rows + naive + visbuffer | 2379.9 | 0.66 | 2.11 | 0.31 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 40 | gpu-raster: clipmap + naive + raster | 2360.6 | 0.66 | 2.00 | 0.32 | 0.09 | 61.1 | 0.00 | 49 | 218 |
| 41 | gpu-raster: hier + greedy + gbuffer | 2350.0 | 0.65 | 2.18 | 0.32 | 0.10 | 71.1 | 0.00 | 30 | 218 |
| 42 | gpu-raster: prefix + naive + gbuffer | 2332.4 | 0.65 | 2.17 | 0.32 | 0.10 | 61.1 | 0.00 | 31 | 218 |
| 43 | gpu-raster: distance + greedy + gbuffer | 2299.7 | 0.64 | 2.30 | 0.32 | 0.10 | 71.1 | 0.00 | 32 | 217 |
| 44 | gpu-raster: rows + greedy + gbuffer | 2275.6 | 0.63 | 2.01 | 0.33 | 0.10 | 71.1 | 0.00 | 30 | 217 |
| 45 | gpu-raster: tree64 + cubes + raster | 2271.6 | 0.63 | 2.22 | 0.33 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 46 | gpu-raster: svo_dag + naive + gbuffer | 2259.4 | 0.63 | 2.09 | 0.34 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 47 | gpu-raster: tree64 + naive + gbuffer | 2258.1 | 0.63 | 2.21 | 0.34 | 0.10 | 61.1 | 0.00 | 26 | 219 |
| 48 | gpu-raster: hier + naive + gbuffer | 2225.5 | 0.62 | 2.19 | 0.34 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 49 | gpu-raster: distance + cubes + visbuffer | 2209.5 | 0.62 | 2.37 | 0.35 | 0.10 | 61.1 | 0.00 | 32 | 218 |
| 50 | gpu-raster: hier + cubes + raster | 2192.1 | 0.61 | 2.33 | 0.34 | 0.10 | 61.1 | 0.00 | 30 | 217 |
| 51 | gpu-raster: svo_dag + cubes + raster | 2179.0 | 0.61 | 2.23 | 0.35 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 52 | gpu-raster: bricks + cubes + raster | 2177.7 | 0.61 | 1.95 | 0.36 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 53 | gpu-raster: clipmap + cubes + visbuffer | 2159.4 | 0.60 | 2.14 | 0.35 | 0.11 | 61.1 | 0.00 | 49 | 218 |
| 54 | gpu-raster: clipmap + greedy + gbuffer | 2150.2 | 0.60 | 2.07 | 0.36 | 0.09 | 71.1 | 0.00 | 49 | 217 |
| 55 | gpu-raster: distance + cubes + raster | 2147.2 | 0.60 | 2.20 | 0.36 | 0.09 | 61.1 | 0.00 | 32 | 218 |
| 56 | gpu-raster: rows + cubes + raster | 2132.0 | 0.59 | 2.07 | 0.37 | 0.09 | 61.1 | 0.00 | 30 | 217 |
| 57 | gpu-raster: bricks + greedy + gbuffer | 2124.5 | 0.59 | 2.97 | 0.35 | 0.11 | 71.1 | 0.00 | 30 | 218 |
| 58 | gpu-raster: distance + naive + gbuffer | 2123.2 | 0.59 | 1.97 | 0.37 | 0.10 | 61.1 | 0.00 | 32 | 218 |
| 59 | gpu-raster: bricks + cubes + visbuffer | 2110.1 | 0.59 | 3.33 | 0.35 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 60 | gpu-raster: rows + cubes + visbuffer | 2097.1 | 0.58 | 2.14 | 0.37 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 61 | gpu-raster: bricks + naive + gbuffer | 2070.2 | 0.58 | 2.20 | 0.37 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 62 | gpu-raster: rows + naive + gbuffer | 1954.1 | 0.54 | 3.21 | 0.37 | 0.09 | 61.1 | 0.00 | 30 | 219 |
| 63 | gpu-raster: svo_dag + cubes + gbuffer | 1904.2 | 0.53 | 3.45 | 0.39 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 64 | gpu-raster: clipmap + naive + gbuffer | 1902.5 | 0.53 | 4.22 | 0.39 | 0.11 | 61.1 | 0.00 | 49 | 219 |
| 65 | gpu-raster: clipmap + cubes + raster | 1825.2 | 0.51 | 4.12 | 0.42 | 0.10 | 61.1 | 0.00 | 49 | 218 |
| 66 | gpu-compute: distance + beam + sphere | 1790.0 | 0.50 | 3.32 | 0.44 | 0.10 | 73.5 | 0.00 | 22 | 217 |
| 67 | gpu-raster: tree64 + cubes + gbuffer | 1768.6 | 0.49 | 3.67 | 0.43 | 0.10 | 61.1 | 0.00 | 26 | 218 |
| 68 | gpu-raster: distance + cubes + gbuffer | 1730.7 | 0.48 | 4.10 | 0.44 | 0.10 | 61.1 | 0.00 | 32 | 218 |
| 69 | gpu-compute: distance + sphere | 1729.4 | 0.48 | 4.69 | 0.43 | 0.09 | 73.5 | 0.00 | 22 | 216 |
| 70 | gpu-raster: clipmap + cubes + gbuffer | 1707.8 | 0.48 | 4.04 | 0.45 | 0.10 | 61.1 | 0.00 | 49 | 218 |
| 71 | gpu-raster: prefix + cubes + gbuffer | 1685.7 | 0.47 | 4.44 | 0.45 | 0.09 | 61.1 | 0.00 | 31 | 218 |
| 72 | gpu-raster: bricks + cubes + gbuffer | 1668.5 | 0.46 | 5.00 | 0.46 | 0.09 | 61.1 | 0.00 | 30 | 218 |
| 73 | gpu-raster: hier + cubes + gbuffer | 1664.9 | 0.46 | 4.10 | 0.46 | 0.10 | 61.1 | 0.00 | 30 | 218 |
| 74 | gpu-raster: rows + cubes + gbuffer | 1480.1 | 0.41 | 4.62 | 0.53 | 0.09 | 61.1 | 0.00 | 30 | 218 |
| 75 | gpu-compute: distance + refloor | 1287.1 | 0.36 | 4.70 | 0.64 | 0.09 | 73.5 | 0.00 | 22 | 217 |
| 76 | gpu-compute: hier + refloor | 1269.6 | 0.35 | 2.40 | 0.72 | 0.09 | 73.5 | 0.00 | 20 | 217 |
| 77 | gpu-compute: hier + beam + refloor | 1260.0 | 0.35 | 2.25 | 0.72 | 0.09 | 73.5 | 0.00 | 20 | 216 |
| 78 | gpu-compute: distance + beam + refloor | 1239.4 | 0.35 | 4.54 | 0.67 | 0.09 | 73.5 | 0.00 | 22 | 217 |
| 79 | gpu-compute: prefix + refloor | 1234.2 | 0.34 | 2.40 | 0.73 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 80 | gpu-compute: distance + rowrun | 1232.2 | 0.34 | 4.47 | 0.68 | 0.10 | 73.5 | 0.00 | 22 | 217 |
| 81 | gpu-compute: prefix + beam + refloor | 1218.7 | 0.34 | 2.29 | 0.74 | 0.09 | 73.5 | 0.00 | 20 | 218 |
| 82 | gpu-compute: distance + beam + rowrun | 1204.8 | 0.34 | 4.75 | 0.70 | 0.10 | 73.5 | 0.00 | 22 | 218 |
| 83 | gpu-compute: hier + rowrun | 1188.5 | 0.33 | 2.40 | 0.77 | 0.09 | 73.5 | 0.00 | 20 | 217 |
| 84 | gpu-compute: svo_dag + refloor | 1175.3 | 0.33 | 5.06 | 0.71 | 0.09 | 73.5 | 0.00 | 16 | 217 |
| 85 | gpu-compute: prefix + rowrun | 1173.8 | 0.33 | 2.42 | 0.76 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 86 | gpu-compute: svo_dag + rowrun | 1166.7 | 0.32 | 4.71 | 0.73 | 0.09 | 73.5 | 0.00 | 16 | 218 |
| 87 | gpu-compute: hier + beam + rowrun | 1149.6 | 0.32 | 2.62 | 0.80 | 0.10 | 73.5 | 0.00 | 20 | 216 |
| 88 | gpu-compute: svo_dag + beam + rowrun | 1138.5 | 0.32 | 4.48 | 0.75 | 0.09 | 73.5 | 0.00 | 16 | 218 |
| 89 | gpu-compute: svo_dag + beam + refloor | 1132.7 | 0.32 | 4.51 | 0.73 | 0.10 | 73.5 | 0.00 | 16 | 217 |
| 90 | gpu-compute: prefix + beam + rowrun | 1097.0 | 0.31 | 4.66 | 0.77 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 91 | gpu-compute: bricks + beam + refloor | 1027.7 | 0.29 | 2.46 | 0.90 | 0.09 | 73.5 | 0.00 | 20 | 217 |
| 92 | gpu-compute: bricks + refloor | 1022.6 | 0.28 | 2.52 | 0.91 | 0.09 | 73.5 | 0.00 | 20 | 216 |
| 93 | gpu-compute: rows + beam + refloor | 968.4 | 0.27 | 4.70 | 0.90 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 94 | gpu-compute: bricks + beam + rowrun | 968.2 | 0.27 | 2.59 | 0.97 | 0.09 | 73.5 | 0.00 | 20 | 218 |
| 95 | gpu-compute: rows + refloor | 960.0 | 0.27 | 4.76 | 0.90 | 0.10 | 73.5 | 0.00 | 20 | 216 |
| 96 | gpu-compute: bricks + rowrun | 950.7 | 0.26 | 2.75 | 0.97 | 0.10 | 73.5 | 0.00 | 20 | 216 |
| 97 | gpu-compute: rows + rowrun | 949.2 | 0.26 | 4.89 | 0.92 | 0.11 | 73.5 | 0.00 | 20 | 217 |
| 98 | gpu-compute: rows + beam + rowrun | 934.9 | 0.26 | 4.65 | 0.94 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 99 | gpu-compute: tree64 + refloor | 919.2 | 0.26 | 4.80 | 0.96 | 0.10 | 73.5 | 0.00 | 16 | 218 |
| 100 | gpu-compute: tree64 + beam + refloor | 912.2 | 0.25 | 4.87 | 0.96 | 0.10 | 73.5 | 0.00 | 16 | 218 |
| 101 | gpu-compute: tree64 + rowrun | 898.0 | 0.25 | 4.68 | 0.99 | 0.09 | 73.5 | 0.00 | 16 | 218 |
| 102 | gpu-compute: tree64 + beam + rowrun | 894.6 | 0.25 | 5.21 | 0.98 | 0.09 | 73.5 | 0.00 | 16 | 217 |
| 103 | gpu-compute: clipmap + refloor | 837.9 | 0.23 | 5.09 | 1.07 | 0.10 | 73.5 | 0.00 | 39 | 216 |
| 104 | gpu-compute: clipmap + beam + refloor | 825.0 | 0.23 | 5.14 | 1.07 | 0.10 | 73.5 | 0.00 | 39 | 217 |
| 105 | gpu-compute: clipmap + rowrun | 813.7 | 0.23 | 4.88 | 1.10 | 0.09 | 73.5 | 0.00 | 39 | 217 |
| 106 | gpu-compute: clipmap + beam + rowrun | 813.5 | 0.23 | 4.87 | 1.11 | 0.10 | 73.5 | 0.00 | 39 | 217 |
| 107 | gpu-compute: clipmap + incremental | 738.8 | 0.21 | 5.23 | 1.22 | 0.09 | 73.5 | 0.00 | 39 | 216 |
| 108 | gpu-compute: clipmap + beam + incremental | 737.6 | 0.21 | 5.26 | 1.23 | 0.10 | 73.5 | 0.00 | 39 | 217 |
| 109 | gpu-compute: tree64 + incremental | 729.6 | 0.20 | 5.54 | 1.24 | 0.09 | 73.5 | 0.00 | 16 | 218 |
| 110 | gpu-compute: hier + incremental | 708.5 | 0.20 | 3.12 | 1.35 | 0.09 | 73.5 | 0.00 | 20 | 216 |
| 111 | gpu-compute: tree64 + beam + incremental | 707.9 | 0.20 | 5.68 | 1.28 | 0.10 | 73.5 | 0.00 | 16 | 218 |
| 112 | gpu-compute: hier + beam + incremental | 676.0 | 0.19 | 3.20 | 1.42 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 113 | gpu-compute: distance + incremental | 663.2 | 0.18 | 5.32 | 1.37 | 0.09 | 73.5 | 0.00 | 22 | 217 |
| 114 | gpu-compute: bricks + incremental | 637.1 | 0.18 | 3.28 | 1.51 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 115 | gpu-compute: bricks + beam + incremental | 635.3 | 0.18 | 3.27 | 1.52 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 116 | gpu-compute: distance + beam + incremental | 632.5 | 0.18 | 5.60 | 1.46 | 0.10 | 73.5 | 0.00 | 22 | 217 |
| 117 | gpu-compute: prefix + incremental | 629.8 | 0.18 | 3.39 | 1.52 | 0.10 | 73.5 | 0.00 | 20 | 218 |
| 118 | gpu-compute: prefix + beam + incremental | 622.5 | 0.17 | 3.19 | 1.55 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 119 | gpu-compute: rows + incremental | 604.8 | 0.17 | 5.65 | 1.53 | 0.10 | 73.5 | 0.00 | 20 | 217 |
| 120 | gpu-compute: rows + beam + incremental | 568.6 | 0.16 | 5.58 | 1.64 | 0.11 | 73.5 | 0.00 | 20 | 217 |
| 121 | gpu-compute: svo_dag + incremental | 433.8 | 0.12 | 6.48 | 2.19 | 0.14 | 73.5 | 0.00 | 16 | 218 |
| 122 | gpu-compute: svo_dag + beam + incremental | 432.8 | 0.12 | 6.22 | 2.20 | 0.13 | 73.5 | 0.00 | 16 | 218 |
| 123 | gpu-compute: hier + sphere | 427.3 | 0.12 | 4.10 | 2.29 | 0.15 | 73.5 | 0.00 | 20 | 216 |
| 124 | gpu-compute: hier + beam + sphere | 421.5 | 0.12 | 4.04 | 2.32 | 0.14 | 73.5 | 0.00 | 20 | 217 |
| 125 | gpu-compute: svo_dag + sphere | 379.0 | 0.11 | 6.66 | 2.53 | 0.16 | 73.5 | 0.00 | 16 | 218 |
| 126 | gpu-compute: svo_dag + beam + sphere | 371.4 | 0.10 | 6.54 | 2.59 | 0.14 | 73.5 | 0.00 | 16 | 218 |
| 127 | gpu-compute: prefix + sphere | 363.5 | 0.10 | 4.51 | 2.70 | 0.16 | 73.5 | 0.00 | 20 | 217 |
| 128 | gpu-compute: bricks + sphere | 356.9 | 0.10 | 4.49 | 2.76 | 0.16 | 73.5 | 0.00 | 20 | 216 |
| 129 | gpu-compute: prefix + beam + sphere | 352.2 | 0.10 | 6.60 | 2.74 | 0.14 | 73.5 | 0.00 | 20 | 218 |
| 130 | gpu-compute: bricks + beam + sphere | 352.0 | 0.10 | 4.56 | 2.79 | 0.16 | 73.5 | 0.00 | 20 | 217 |
| 131 | gpu-compute: rows + sphere | 350.7 | 0.10 | 6.72 | 2.75 | 0.15 | 73.5 | 0.00 | 20 | 217 |
| 132 | gpu-compute: rows + beam + sphere | 348.7 | 0.10 | 6.55 | 2.76 | 0.16 | 73.5 | 0.00 | 20 | 217 |
| 133 | gpu-compute: tree64 + sphere | 340.0 | 0.09 | 6.32 | 2.84 | 0.16 | 73.5 | 0.00 | 16 | 218 |
| 134 | gpu-compute: tree64 + beam + sphere | 337.7 | 0.09 | 6.85 | 2.86 | 0.17 | 73.5 | 0.00 | 16 | 218 |
| 135 | gpu-compute: clipmap + sphere | 322.1 | 0.09 | 7.08 | 3.01 | 0.15 | 73.5 | 0.00 | 39 | 216 |
| 136 | gpu-compute: clipmap + beam + sphere | 312.5 | 0.09 | 6.74 | 3.11 | 0.16 | 73.5 | 0.00 | 39 | 217 |
| 137 | gpu-compute: distance + sphere + pathtrace (incorrect) | 628.8 | 0.18 | 5.35 | 1.47 | 0.10 | 31.0 | 7.28 | 54 | 216 |
| 138 | gpu-compute: distance + beam + sphere + pathtrace (incorrect) | 625.2 | 0.17 | 5.17 | 1.48 | 0.09 | 31.0 | 7.28 | 54 | 217 |
| 139 | gpu-compute: distance + beam + refloor + pathtrace (incorrect) | 472.5 | 0.13 | 6.10 | 2.01 | 0.12 | 31.0 | 7.28 | 54 | 217 |
| 140 | gpu-compute: prefix + beam + refloor + pathtrace (incorrect) | 469.5 | 0.13 | 3.87 | 2.08 | 0.13 | 31.0 | 7.28 | 52 | 217 |
| 141 | gpu-compute: distance + refloor + pathtrace (incorrect) | 465.4 | 0.13 | 6.19 | 2.03 | 0.12 | 31.0 | 7.28 | 54 | 217 |
| 142 | gpu-compute: svo_dag + beam + refloor + pathtrace (incorrect) | 458.9 | 0.13 | 6.31 | 2.07 | 0.12 | 31.0 | 7.28 | 48 | 218 |
| 143 | gpu-compute: hier + beam + refloor + pathtrace (incorrect) | 451.3 | 0.13 | 4.05 | 2.16 | 0.13 | 31.0 | 7.28 | 52 | 217 |
| 144 | gpu-compute: svo_dag + refloor + pathtrace (incorrect) | 451.1 | 0.13 | 6.23 | 2.08 | 0.12 | 31.0 | 7.28 | 48 | 218 |
| 145 | gpu-compute: prefix + refloor + pathtrace (incorrect) | 440.9 | 0.12 | 4.26 | 2.21 | 0.13 | 31.0 | 7.28 | 52 | 217 |
| 146 | gpu-compute: hier + refloor + pathtrace (incorrect) | 435.2 | 0.12 | 3.90 | 2.24 | 0.14 | 31.0 | 7.28 | 52 | 217 |
| 147 | gpu-compute: bricks + beam + refloor + pathtrace (incorrect) | 403.0 | 0.11 | 4.29 | 2.43 | 0.15 | 31.0 | 7.28 | 52 | 217 |
| 148 | gpu-compute: bricks + refloor + pathtrace (incorrect) | 395.2 | 0.11 | 4.42 | 2.48 | 0.16 | 31.0 | 7.28 | 52 | 217 |
| 149 | gpu-compute: rows + beam + refloor + pathtrace (incorrect) | 388.5 | 0.11 | 6.61 | 2.47 | 0.15 | 31.0 | 7.28 | 52 | 218 |
| 150 | gpu-compute: prefix + rowrun + pathtrace (incorrect) | 379.0 | 0.11 | 4.19 | 2.59 | 0.15 | 31.0 | 7.28 | 52 | 217 |
| 151 | gpu-compute: rows + refloor + pathtrace (incorrect) | 378.7 | 0.11 | 7.04 | 2.53 | 0.15 | 31.0 | 7.28 | 52 | 217 |
| 152 | gpu-compute: tree64 + beam + refloor + pathtrace (incorrect) | 368.8 | 0.10 | 7.11 | 2.60 | 0.16 | 31.0 | 7.28 | 48 | 218 |
| 153 | gpu-compute: tree64 + refloor + pathtrace (incorrect) | 366.7 | 0.10 | 6.97 | 2.63 | 0.16 | 31.0 | 7.28 | 48 | 218 |
| 154 | gpu-compute: distance + beam + rowrun + pathtrace (incorrect) | 366.5 | 0.10 | 6.34 | 2.62 | 0.14 | 31.0 | 7.28 | 54 | 217 |
| 155 | gpu-compute: prefix + beam + rowrun + pathtrace (incorrect) | 364.9 | 0.10 | 6.50 | 2.64 | 0.14 | 31.0 | 7.28 | 52 | 218 |
| 156 | gpu-compute: distance + rowrun + pathtrace (incorrect) | 353.5 | 0.10 | 6.76 | 2.72 | 0.15 | 31.0 | 7.28 | 54 | 217 |
| 157 | gpu-compute: clipmap + refloor + pathtrace (incorrect) | 336.0 | 0.09 | 6.61 | 2.87 | 0.15 | 31.0 | 7.28 | 71 | 217 |
| 158 | gpu-compute: clipmap + beam + refloor + pathtrace (incorrect) | 330.4 | 0.09 | 7.16 | 2.93 | 0.16 | 31.0 | 7.28 | 71 | 218 |
| 159 | gpu-compute: rows + beam + rowrun + pathtrace (incorrect) | 308.5 | 0.09 | 7.34 | 3.15 | 0.15 | 31.0 | 7.28 | 52 | 217 |
| 160 | gpu-compute: rows + rowrun + pathtrace (incorrect) | 305.7 | 0.09 | 7.32 | 3.17 | 0.15 | 31.0 | 7.28 | 52 | 217 |
| 161 | gpu-compute: bricks + rowrun + pathtrace (incorrect) | 258.4 | 0.07 | 5.77 | 3.82 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 162 | gpu-compute: bricks + beam + rowrun + pathtrace (incorrect) | 241.0 | 0.07 | 5.92 | 4.10 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 163 | gpu-compute: clipmap + beam + incremental + pathtrace (incorrect) | 238.8 | 0.07 | 8.26 | 4.11 | 0.17 | 31.0 | 7.28 | 71 | 218 |
| 164 | gpu-compute: clipmap + beam + rowrun + pathtrace (incorrect) | 237.8 | 0.07 | 8.35 | 4.12 | 0.16 | 31.0 | 7.28 | 71 | 217 |
| 165 | gpu-compute: clipmap + rowrun + pathtrace (incorrect) | 236.5 | 0.07 | 8.41 | 4.14 | 0.17 | 31.0 | 7.28 | 71 | 217 |
| 166 | gpu-compute: clipmap + incremental + pathtrace (incorrect) | 235.4 | 0.07 | 8.46 | 4.16 | 0.17 | 31.0 | 7.28 | 71 | 216 |
| 167 | gpu-compute: tree64 + incremental + pathtrace (incorrect) | 216.6 | 0.06 | 9.32 | 4.54 | 0.18 | 31.0 | 7.28 | 48 | 219 |
| 168 | gpu-compute: hier + rowrun + pathtrace (incorrect) | 212.2 | 0.06 | 6.31 | 4.66 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 169 | gpu-compute: tree64 + beam + incremental + pathtrace (incorrect) | 201.9 | 0.06 | 9.59 | 4.88 | 0.18 | 31.0 | 7.28 | 48 | 219 |
| 170 | gpu-compute: hier + beam + rowrun + pathtrace (incorrect) | 200.9 | 0.06 | 7.16 | 4.93 | 0.15 | 31.0 | 7.28 | 52 | 217 |
| 171 | gpu-compute: hier + sphere + pathtrace (incorrect) | 176.2 | 0.05 | 7.40 | 5.64 | 0.17 | 31.0 | 7.28 | 52 | 216 |
| 172 | gpu-compute: hier + beam + sphere + pathtrace (incorrect) | 175.4 | 0.05 | 7.59 | 5.66 | 0.16 | 31.0 | 7.28 | 52 | 217 |
| 173 | gpu-compute: svo_dag + beam + rowrun + pathtrace (incorrect) | 169.8 | 0.05 | 10.60 | 5.82 | 0.18 | 31.0 | 7.28 | 48 | 218 |
| 174 | gpu-compute: svo_dag + rowrun + pathtrace (incorrect) | 166.9 | 0.05 | 10.78 | 5.93 | 0.18 | 31.0 | 7.28 | 48 | 219 |
| 175 | gpu-compute: svo_dag + beam + sphere + pathtrace (incorrect) | 157.6 | 0.04 | 10.12 | 6.29 | 0.17 | 31.0 | 7.28 | 48 | 218 |
| 176 | gpu-compute: bricks + sphere + pathtrace (incorrect) | 157.3 | 0.04 | 8.05 | 6.31 | 0.18 | 31.0 | 7.28 | 52 | 217 |
| 177 | gpu-compute: bricks + beam + sphere + pathtrace (incorrect) | 156.5 | 0.04 | 8.11 | 6.34 | 0.18 | 31.0 | 7.28 | 52 | 218 |
| 178 | gpu-compute: rows + beam + sphere + pathtrace (incorrect) | 155.4 | 0.04 | 10.51 | 6.37 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 179 | gpu-compute: prefix + beam + sphere + pathtrace (incorrect) | 155.3 | 0.04 | 8.11 | 6.38 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 180 | gpu-compute: prefix + sphere + pathtrace (incorrect) | 154.8 | 0.04 | 8.42 | 6.42 | 0.18 | 31.0 | 7.28 | 52 | 217 |
| 181 | gpu-compute: rows + sphere + pathtrace (incorrect) | 154.6 | 0.04 | 11.58 | 6.38 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 182 | gpu-compute: svo_dag + sphere + pathtrace (incorrect) | 154.5 | 0.04 | 10.44 | 6.41 | 0.18 | 31.0 | 7.28 | 48 | 218 |
| 183 | gpu-compute: tree64 + beam + sphere + pathtrace (incorrect) | 149.2 | 0.04 | 11.74 | 6.64 | 0.18 | 31.0 | 7.28 | 48 | 218 |
| 184 | gpu-compute: tree64 + sphere + pathtrace (incorrect) | 143.7 | 0.04 | 11.85 | 6.89 | 0.19 | 31.0 | 7.28 | 48 | 218 |
| 185 | gpu-compute: clipmap + sphere + pathtrace (incorrect) | 142.0 | 0.04 | 11.32 | 6.98 | 0.19 | 31.0 | 7.28 | 71 | 217 |
| 186 | gpu-compute: distance + beam + incremental + pathtrace (incorrect) | 141.9 | 0.04 | 11.22 | 6.99 | 0.17 | 31.0 | 7.28 | 54 | 218 |
| 187 | gpu-compute: clipmap + beam + sphere + pathtrace (incorrect) | 137.1 | 0.04 | 11.59 | 7.24 | 0.21 | 31.0 | 7.28 | 71 | 217 |
| 188 | gpu-compute: svo_dag + incremental + pathtrace (incorrect) | 134.4 | 0.04 | 11.66 | 7.39 | 0.17 | 31.0 | 7.28 | 48 | 217 |
| 189 | gpu-compute: distance + incremental + pathtrace (incorrect) | 133.5 | 0.04 | 12.04 | 7.43 | 0.17 | 31.0 | 7.28 | 54 | 217 |
| 190 | gpu-compute: prefix + beam + incremental + pathtrace (incorrect) | 132.0 | 0.04 | 9.16 | 7.53 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 191 | gpu-compute: hier + beam + incremental + pathtrace (incorrect) | 128.7 | 0.04 | 9.62 | 7.73 | 0.18 | 31.0 | 7.28 | 52 | 217 |
| 192 | gpu-compute: svo_dag + beam + incremental + pathtrace (incorrect) | 128.2 | 0.04 | 12.33 | 7.74 | 0.19 | 31.0 | 7.28 | 48 | 218 |
| 193 | gpu-compute: prefix + incremental + pathtrace (incorrect) | 123.2 | 0.03 | 9.76 | 8.08 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 194 | gpu-compute: hier + incremental + pathtrace (incorrect) | 120.0 | 0.03 | 9.94 | 8.29 | 0.18 | 31.0 | 7.28 | 52 | 217 |
| 195 | gpu-compute: rows + beam + incremental + pathtrace (incorrect) | 117.3 | 0.03 | 12.73 | 8.46 | 0.17 | 31.0 | 7.28 | 52 | 216 |
| 196 | gpu-compute: bricks + beam + incremental + pathtrace (incorrect) | 117.1 | 0.03 | 10.12 | 8.50 | 0.18 | 31.0 | 7.28 | 52 | 217 |
| 197 | gpu-compute: tree64 + beam + rowrun + pathtrace (incorrect) | 114.2 | 0.03 | 13.12 | 8.70 | 0.18 | 31.0 | 7.28 | 48 | 219 |
| 198 | gpu-compute: rows + incremental + pathtrace (incorrect) | 110.2 | 0.03 | 12.76 | 9.02 | 0.17 | 31.0 | 7.28 | 52 | 217 |
| 199 | gpu-compute: bricks + incremental + pathtrace (incorrect) | 109.2 | 0.03 | 11.02 | 9.11 | 0.17 | 31.0 | 7.28 | 52 | 216 |
| 200 | gpu-compute: tree64 + rowrun + pathtrace (incorrect) | 104.2 | 0.03 | 13.48 | 9.54 | 0.17 | 31.0 | 7.28 | 48 | 219 |

### meshes [unlit+meshes]

1920×1080. camera 0.00 blocks/frame (max 0.0), turn 0.00°/frame; 27 chunks at start, 0.0 loads/frame; chunk fill 4%, solid 0.1%, surface 0.06%; sky 47%, hits at 51 blocks (median 43); 0.0 edits/frame; 2 lights; 5520 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-raster: bricks + naive + raster | 4465.1 | 1.00 | 3.16 | 0.08 | 0.09 | 60.2 | 0.00 | 30 | 218 |
| 2 | gpu-raster: bricks + greedy + raster | 4166.4 | 0.93 | 3.19 | 0.06 | 0.09 | 63.4 | 0.00 | 30 | 217 |
| 3 | gpu-raster: bricks + naive + visbuffer | 3880.1 | 0.87 | 2.38 | 0.11 | 0.09 | 60.1 | 0.00 | 30 | 218 |
| 4 | gpu-raster: bricks + greedy + gbuffer | 3534.6 | 0.79 | 2.00 | 0.16 | 0.09 | 63.4 | 0.00 | 30 | 217 |
| 5 | gpu-raster: bricks + greedy + visbuffer | 3526.6 | 0.79 | 2.14 | 0.10 | 0.11 | 63.4 | 0.00 | 30 | 218 |
| 6 | gpu-raster: bricks + cubes + visbuffer | 3469.0 | 0.78 | 1.96 | 0.16 | 0.09 | 60.1 | 0.00 | 30 | 217 |
| 7 | gpu-raster: bricks + cubes + raster | 3434.7 | 0.77 | 1.88 | 0.16 | 0.10 | 60.2 | 0.00 | 30 | 217 |
| 8 | gpu-raster: bricks + naive + gbuffer | 3227.3 | 0.72 | 1.92 | 0.20 | 0.10 | 60.2 | 0.00 | 30 | 218 |
| 9 | gpu-compute: distance + sphere | 2522.0 | 0.56 | 2.05 | 0.30 | 0.09 | 71.3 | 0.00 | 22 | 217 |
| 10 | gpu-compute: distance + beam + sphere | 2460.0 | 0.55 | 2.04 | 0.31 | 0.09 | 71.3 | 0.00 | 22 | 218 |
| 11 | gpu-raster: bricks + cubes + gbuffer | 2339.8 | 0.52 | 1.99 | 0.33 | 0.09 | 60.2 | 0.00 | 30 | 218 |
| 12 | gpu-compute: distance + beam + refloor | 1594.8 | 0.36 | 4.10 | 0.50 | 0.09 | 71.3 | 0.00 | 22 | 217 |
| 13 | gpu-compute: distance + refloor | 1584.7 | 0.35 | 4.74 | 0.49 | 0.09 | 71.3 | 0.00 | 22 | 216 |
| 14 | gpu-compute: distance + rowrun | 1566.3 | 0.35 | 4.32 | 0.51 | 0.09 | 71.3 | 0.00 | 22 | 217 |
| 15 | gpu-compute: distance + beam + rowrun | 1519.7 | 0.34 | 4.26 | 0.52 | 0.10 | 71.3 | 0.00 | 22 | 217 |
| 16 | gpu-compute: hier + refloor | 1419.7 | 0.32 | 4.39 | 0.57 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 17 | gpu-compute: hier + beam + refloor | 1409.7 | 0.32 | 4.19 | 0.58 | 0.09 | 71.3 | 0.00 | 20 | 217 |
| 18 | gpu-compute: prefix + refloor | 1374.3 | 0.31 | 4.15 | 0.60 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 19 | gpu-compute: prefix + beam + refloor | 1342.4 | 0.30 | 4.46 | 0.62 | 0.09 | 71.3 | 0.00 | 20 | 217 |
| 20 | gpu-compute: hier + rowrun | 1339.6 | 0.30 | 4.65 | 0.62 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 21 | gpu-compute: svo_dag + refloor | 1338.2 | 0.30 | 4.73 | 0.59 | 0.10 | 71.3 | 0.00 | 16 | 218 |
| 22 | gpu-compute: svo_dag + rowrun | 1330.4 | 0.30 | 4.77 | 0.61 | 0.10 | 71.3 | 0.00 | 16 | 218 |
| 23 | gpu-compute: svo_dag + beam + refloor | 1327.7 | 0.30 | 4.57 | 0.60 | 0.09 | 71.3 | 0.00 | 16 | 218 |
| 24 | gpu-compute: prefix + beam + rowrun | 1315.3 | 0.29 | 4.54 | 0.63 | 0.09 | 71.3 | 0.00 | 20 | 218 |
| 25 | gpu-compute: prefix + rowrun | 1300.6 | 0.29 | 4.57 | 0.63 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 26 | gpu-compute: hier + beam + rowrun | 1299.3 | 0.29 | 4.16 | 0.64 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 27 | gpu-compute: svo_dag + beam + rowrun | 1283.1 | 0.29 | 4.54 | 0.64 | 0.10 | 71.3 | 0.00 | 16 | 218 |
| 28 | gpu-compute: bricks + refloor | 1188.0 | 0.27 | 4.45 | 0.72 | 0.09 | 71.3 | 0.00 | 20 | 217 |
| 29 | gpu-compute: bricks + beam + refloor | 1186.8 | 0.27 | 3.77 | 0.73 | 0.09 | 71.3 | 0.00 | 20 | 216 |
| 30 | gpu-compute: rows + refloor | 1159.5 | 0.26 | 4.72 | 0.73 | 0.10 | 71.3 | 0.00 | 20 | 216 |
| 31 | gpu-compute: rows + beam + refloor | 1138.4 | 0.25 | 4.77 | 0.74 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 32 | gpu-compute: rows + rowrun | 1123.8 | 0.25 | 5.08 | 0.75 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 33 | gpu-compute: rows + beam + rowrun | 1121.3 | 0.25 | 4.74 | 0.76 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 34 | gpu-compute: bricks + rowrun | 1087.5 | 0.24 | 4.28 | 0.79 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 35 | gpu-compute: bricks + beam + rowrun | 1086.6 | 0.24 | 5.09 | 0.78 | 0.11 | 71.3 | 0.00 | 20 | 217 |
| 36 | gpu-compute: tree64 + refloor | 1043.7 | 0.23 | 4.81 | 0.82 | 0.10 | 71.3 | 0.00 | 16 | 218 |
| 37 | gpu-compute: tree64 + rowrun | 1037.0 | 0.23 | 4.82 | 0.83 | 0.09 | 71.3 | 0.00 | 16 | 218 |
| 38 | gpu-compute: tree64 + beam + refloor | 1035.3 | 0.23 | 5.28 | 0.83 | 0.10 | 71.3 | 0.00 | 16 | 218 |
| 39 | gpu-compute: clipmap + refloor | 1018.9 | 0.23 | 4.70 | 0.85 | 0.10 | 71.3 | 0.00 | 39 | 216 |
| 40 | gpu-compute: tree64 + beam + rowrun | 1017.9 | 0.23 | 5.41 | 0.84 | 0.10 | 71.3 | 0.00 | 16 | 219 |
| 41 | gpu-compute: clipmap + beam + refloor | 1008.6 | 0.23 | 4.55 | 0.86 | 0.10 | 71.3 | 0.00 | 39 | 216 |
| 42 | gpu-compute: clipmap + rowrun | 975.2 | 0.22 | 4.63 | 0.90 | 0.09 | 71.3 | 0.00 | 39 | 217 |
| 43 | gpu-compute: clipmap + beam + rowrun | 970.5 | 0.22 | 4.67 | 0.91 | 0.10 | 71.3 | 0.00 | 39 | 217 |
| 44 | gpu-compute: clipmap + incremental | 886.0 | 0.20 | 5.13 | 1.00 | 0.10 | 71.3 | 0.00 | 39 | 217 |
| 45 | gpu-compute: clipmap + beam + incremental | 868.5 | 0.19 | 5.78 | 1.01 | 0.09 | 71.3 | 0.00 | 39 | 217 |
| 46 | gpu-compute: tree64 + incremental | 832.1 | 0.19 | 5.23 | 1.07 | 0.10 | 71.3 | 0.00 | 16 | 218 |
| 47 | gpu-compute: tree64 + beam + incremental | 828.0 | 0.19 | 5.18 | 1.08 | 0.10 | 71.3 | 0.00 | 16 | 219 |
| 48 | gpu-compute: distance + incremental | 813.9 | 0.18 | 4.74 | 1.11 | 0.09 | 71.3 | 0.00 | 22 | 217 |
| 49 | gpu-compute: hier + incremental | 786.0 | 0.18 | 4.42 | 1.15 | 0.10 | 71.3 | 0.00 | 20 | 216 |
| 50 | gpu-compute: distance + beam + incremental | 764.9 | 0.17 | 4.90 | 1.19 | 0.10 | 71.3 | 0.00 | 22 | 217 |
| 51 | gpu-compute: hier + beam + incremental | 749.8 | 0.17 | 5.15 | 1.21 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 52 | gpu-compute: bricks + incremental | 727.3 | 0.16 | 4.90 | 1.26 | 0.10 | 71.3 | 0.00 | 20 | 216 |
| 53 | gpu-compute: bricks + beam + incremental | 713.8 | 0.16 | 4.89 | 1.29 | 0.10 | 71.3 | 0.00 | 20 | 216 |
| 54 | gpu-compute: prefix + incremental | 709.8 | 0.16 | 5.45 | 1.29 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 55 | gpu-compute: rows + incremental | 703.3 | 0.16 | 5.63 | 1.30 | 0.09 | 71.3 | 0.00 | 20 | 217 |
| 56 | gpu-compute: prefix + beam + incremental | 693.2 | 0.16 | 5.16 | 1.33 | 0.10 | 71.3 | 0.00 | 20 | 217 |
| 57 | gpu-compute: rows + beam + incremental | 661.3 | 0.15 | 5.37 | 1.39 | 0.10 | 71.3 | 0.00 | 20 | 218 |
| 58 | gpu-compute: svo_dag + beam + incremental | 498.5 | 0.11 | 6.20 | 1.89 | 0.13 | 71.3 | 0.00 | 16 | 217 |
| 59 | gpu-compute: svo_dag + incremental | 492.3 | 0.11 | 5.93 | 1.91 | 0.12 | 71.3 | 0.00 | 16 | 218 |
| 60 | gpu-compute: hier + sphere | 479.0 | 0.11 | 6.17 | 1.98 | 0.12 | 71.3 | 0.00 | 20 | 217 |
| 61 | gpu-compute: hier + beam + sphere | 470.6 | 0.11 | 5.97 | 2.01 | 0.12 | 71.3 | 0.00 | 20 | 218 |
| 62 | gpu-compute: svo_dag + sphere | 428.0 | 0.10 | 6.50 | 2.23 | 0.14 | 71.3 | 0.00 | 16 | 218 |
| 63 | gpu-compute: svo_dag + beam + sphere | 415.0 | 0.09 | 6.62 | 2.28 | 0.15 | 71.3 | 0.00 | 16 | 218 |
| 64 | gpu-compute: rows + sphere | 407.4 | 0.09 | 6.32 | 2.35 | 0.15 | 71.3 | 0.00 | 20 | 216 |
| 65 | gpu-compute: bricks + sphere | 403.8 | 0.09 | 5.97 | 2.37 | 0.15 | 71.3 | 0.00 | 20 | 216 |
| 66 | gpu-compute: rows + beam + sphere | 400.5 | 0.09 | 6.61 | 2.39 | 0.15 | 71.3 | 0.00 | 20 | 216 |
| 67 | gpu-compute: prefix + sphere | 398.8 | 0.09 | 6.26 | 2.41 | 0.15 | 71.3 | 0.00 | 20 | 217 |
| 68 | gpu-compute: bricks + beam + sphere | 397.4 | 0.09 | 5.83 | 2.42 | 0.14 | 71.3 | 0.00 | 20 | 217 |
| 69 | gpu-compute: prefix + beam + sphere | 396.0 | 0.09 | 6.53 | 2.42 | 0.15 | 71.3 | 0.00 | 20 | 217 |
| 70 | gpu-compute: tree64 + sphere | 379.6 | 0.09 | 6.18 | 2.53 | 0.15 | 71.3 | 0.00 | 16 | 217 |
| 71 | gpu-compute: tree64 + beam + sphere | 374.3 | 0.08 | 6.70 | 2.54 | 0.16 | 71.3 | 0.00 | 16 | 218 |
| 72 | gpu-compute: clipmap + sphere | 368.9 | 0.08 | 6.86 | 2.60 | 0.17 | 71.3 | 0.00 | 39 | 216 |
| 73 | gpu-compute: clipmap + beam + sphere | 360.9 | 0.08 | 6.68 | 2.67 | 0.16 | 71.3 | 0.00 | 39 | 217 |

### random [unlit]

1920×1080. camera 3.77 blocks/frame (max 3.8), turn 1.14°/frame; 144 chunks at start, 0.9 loads/frame; chunk fill 7%, solid 1.0%, surface 0.98%; sky 16%, hits at 113 blocks (median 111); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: bricks + refloor | 1853.9 | 1.00 | 2.23 | 0.43 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 2 | gpu-compute: rows + refloor | 1774.4 | 0.96 | 2.47 | 0.44 | 0.01 | 56.2 | 0.01 | 33 | 235 |
| 3 | gpu-compute: prefix + refloor | 1721.9 | 0.93 | 2.26 | 0.46 | 0.01 | 56.2 | 0.01 | 33 | 236 |
| 4 | gpu-compute: clipmap + refloor | 1701.1 | 0.92 | 2.44 | 0.47 | 0.01 | 56.2 | 0.01 | 86 | 237 |
| 5 | gpu-compute: hier + refloor | 1688.1 | 0.91 | 2.29 | 0.48 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 6 | gpu-compute: bricks + sphere | 1587.2 | 0.86 | 2.18 | 0.53 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 7 | gpu-compute: rows + rowrun | 1514.1 | 0.82 | 2.32 | 0.55 | 0.01 | 56.2 | 0.01 | 33 | 236 |
| 8 | gpu-compute: clipmap + sphere | 1511.2 | 0.82 | 2.45 | 0.55 | 0.01 | 56.2 | 0.01 | 86 | 237 |
| 9 | gpu-compute: rows + sphere | 1505.5 | 0.81 | 2.67 | 0.55 | 0.01 | 56.2 | 0.01 | 33 | 236 |
| 10 | gpu-compute: clipmap + incremental | 1465.8 | 0.79 | 2.51 | 0.57 | 0.01 | 56.2 | 0.01 | 86 | 237 |
| 11 | gpu-compute: hier + sphere | 1404.5 | 0.76 | 2.35 | 0.59 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 12 | gpu-compute: prefix + sphere | 1355.6 | 0.73 | 2.43 | 0.63 | 0.01 | 56.2 | 0.01 | 33 | 235 |
| 13 | gpu-compute: bricks + beam + refloor | 1298.4 | 0.70 | 2.29 | 0.68 | 0.01 | 56.5 | 0.01 | 32 | 235 |
| 14 | gpu-compute: tree64 + refloor | 1289.9 | 0.70 | 2.36 | 0.66 | 0.08 | 56.2 | 0.01 | 18 | 245 |
| 15 | gpu-compute: rows + beam + refloor | 1260.2 | 0.68 | 2.60 | 0.69 | 0.01 | 56.5 | 0.01 | 33 | 235 |
| 16 | gpu-compute: tree64 + beam + refloor | 1259.8 | 0.68 | 2.64 | 0.66 | 0.09 | 56.2 | 0.01 | 18 | 246 |
| 17 | gpu-compute: prefix + beam + refloor | 1223.4 | 0.66 | 2.62 | 0.72 | 0.01 | 56.5 | 0.01 | 33 | 236 |
| 18 | gpu-compute: prefix + rowrun | 1220.3 | 0.66 | 2.50 | 0.72 | 0.01 | 56.2 | 0.01 | 33 | 235 |
| 19 | gpu-compute: hier + beam + refloor | 1213.8 | 0.65 | 2.55 | 0.73 | 0.01 | 56.5 | 0.01 | 32 | 235 |
| 20 | gpu-compute: svo_dag + refloor | 1159.6 | 0.63 | 6.89 | 0.63 | 0.19 | 56.2 | 0.01 | 22 | 271 |
| 21 | gpu-compute: clipmap + rowrun | 1153.5 | 0.62 | 2.64 | 0.77 | 0.01 | 56.2 | 0.01 | 86 | 237 |
| 22 | gpu-compute: bricks + beam + sphere | 1151.2 | 0.62 | 2.42 | 0.79 | 0.01 | 56.5 | 0.01 | 32 | 235 |
| 23 | gpu-compute: bricks + rowrun | 1126.1 | 0.61 | 2.46 | 0.81 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 24 | gpu-compute: rows + beam + sphere | 1117.8 | 0.60 | 2.80 | 0.80 | 0.01 | 56.5 | 0.01 | 33 | 236 |
| 25 | gpu-compute: rows + beam + rowrun | 1106.1 | 0.60 | 2.85 | 0.80 | 0.01 | 56.5 | 0.01 | 33 | 236 |
| 26 | gpu-compute: tree64 + incremental | 1102.6 | 0.59 | 2.56 | 0.80 | 0.08 | 56.2 | 0.01 | 18 | 246 |
| 27 | gpu-compute: tree64 + beam + incremental | 1086.8 | 0.59 | 2.41 | 0.81 | 0.08 | 56.2 | 0.01 | 18 | 246 |
| 28 | gpu-compute: tree64 + sphere | 1081.8 | 0.58 | 2.72 | 0.82 | 0.08 | 56.2 | 0.01 | 18 | 246 |
| 29 | gpu-compute: hier + beam + sphere | 1079.0 | 0.58 | 2.57 | 0.84 | 0.01 | 56.5 | 0.01 | 32 | 236 |
| 30 | gpu-compute: tree64 + beam + sphere | 1067.3 | 0.58 | 2.34 | 0.83 | 0.08 | 56.2 | 0.01 | 18 | 246 |
| 31 | gpu-compute: prefix + beam + sphere | 1037.3 | 0.56 | 2.58 | 0.88 | 0.01 | 56.5 | 0.01 | 33 | 235 |
| 32 | gpu-compute: distance + refloor | 1027.5 | 0.55 | 19.69 | 0.41 | 0.01 | 56.2 | 0.01 | 40 | 236 |
| 33 | gpu-compute: distance + sphere | 1007.6 | 0.54 | 19.73 | 0.43 | 0.01 | 56.2 | 0.01 | 40 | 236 |
| 34 | gpu-compute: prefix + beam + rowrun | 943.6 | 0.51 | 2.73 | 0.97 | 0.01 | 56.5 | 0.01 | 33 | 235 |
| 35 | gpu-compute: rows + incremental | 884.1 | 0.48 | 2.86 | 1.05 | 0.01 | 56.2 | 0.01 | 33 | 235 |
| 36 | gpu-compute: bricks + beam + rowrun | 871.9 | 0.47 | 2.75 | 1.08 | 0.01 | 56.5 | 0.01 | 32 | 236 |
| 37 | gpu-compute: distance + rowrun | 854.4 | 0.46 | 19.84 | 0.63 | 0.01 | 56.2 | 0.01 | 40 | 236 |
| 38 | gpu-compute: svo_dag + sphere | 839.3 | 0.45 | 7.11 | 0.97 | 0.19 | 56.2 | 0.01 | 22 | 271 |
| 39 | gpu-compute: bricks + incremental | 828.0 | 0.45 | 2.90 | 1.15 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 40 | gpu-compute: distance + beam + sphere | 825.1 | 0.45 | 19.98 | 0.69 | 0.01 | 56.5 | 0.01 | 40 | 236 |
| 41 | gpu-compute: distance + beam + refloor | 823.6 | 0.44 | 19.77 | 0.67 | 0.01 | 56.5 | 0.01 | 40 | 236 |
| 42 | gpu-rt: boxes + rayquery | 807.7 | 0.44 | 2.92 | 1.15 | 0.10 | 56.1 | 0.01 | 20 | 377 |
| 43 | gpu-compute: clipmap + beam + refloor | 794.7 | 0.43 | 3.07 | 1.19 | 0.01 | 56.5 | 0.01 | 86 | 237 |
| 44 | gpu-compute: hier + incremental | 787.6 | 0.42 | 2.97 | 1.20 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 45 | gpu-compute: clipmap + beam + sphere | 745.0 | 0.40 | 3.04 | 1.28 | 0.01 | 56.5 | 0.01 | 86 | 237 |
| 46 | gpu-compute: clipmap + beam + incremental | 735.7 | 0.40 | 3.06 | 1.30 | 0.01 | 56.5 | 0.01 | 86 | 237 |
| 47 | gpu-raster: bricks + naive + raster | 713.8 | 0.39 | 3.86 | 1.24 | 0.13 | 46.8 | 0.02 | 105 | 266 |
| 48 | gpu-compute: distance + beam + rowrun | 708.0 | 0.38 | 19.80 | 0.89 | 0.01 | 56.5 | 0.01 | 40 | 236 |
| 49 | gpu-compute: rows + beam + incremental | 700.3 | 0.38 | 3.16 | 1.37 | 0.01 | 56.5 | 0.01 | 33 | 235 |
| 50 | gpu-compute: bricks + beam + incremental | 699.8 | 0.38 | 3.00 | 1.38 | 0.01 | 56.5 | 0.01 | 32 | 235 |
| 51 | gpu-raster: bricks + naive + visbuffer | 689.8 | 0.37 | 4.50 | 1.28 | 0.14 | 46.8 | 0.02 | 105 | 266 |
| 52 | gpu-compute: hier + rowrun | 687.9 | 0.37 | 2.99 | 1.39 | 0.01 | 56.2 | 0.01 | 32 | 235 |
| 53 | gpu-raster: bricks + greedy + raster | 673.8 | 0.36 | 7.68 | 1.21 | 0.24 | 46.8 | 0.02 | 105 | 265 |
| 54 | gpu-compute: hier + beam + incremental | 657.2 | 0.35 | 3.35 | 1.46 | 0.01 | 56.5 | 0.01 | 32 | 236 |
| 55 | gpu-raster: bricks + greedy + visbuffer | 657.0 | 0.35 | 7.14 | 1.26 | 0.24 | 46.8 | 0.02 | 105 | 265 |
| 56 | gpu-compute: clipmap + beam + rowrun | 645.8 | 0.35 | 3.33 | 1.49 | 0.01 | 56.5 | 0.01 | 86 | 237 |
| 57 | gpu-raster: bricks + naive + gbuffer | 614.9 | 0.33 | 3.53 | 1.49 | 0.13 | 46.8 | 0.02 | 105 | 266 |
| 58 | gpu-compute: hier + beam + rowrun | 577.0 | 0.31 | 3.38 | 1.68 | 0.01 | 56.5 | 0.01 | 32 | 236 |
| 59 | gpu-raster: bricks + greedy + gbuffer | 567.0 | 0.31 | 6.88 | 1.51 | 0.24 | 46.8 | 0.02 | 105 | 265 |
| 60 | gpu-compute: svo_dag + beam + refloor | 562.7 | 0.30 | 7.44 | 1.60 | 0.19 | 56.5 | 0.01 | 22 | 270 |
| 61 | gpu-compute: prefix + incremental | 544.2 | 0.29 | 3.97 | 1.78 | 0.01 | 56.2 | 0.01 | 33 | 236 |
| 62 | gpu-compute: distance + incremental | 540.3 | 0.29 | 20.23 | 1.32 | 0.01 | 56.2 | 0.01 | 40 | 236 |
| 63 | gpu-discover: bricks + quadtree | 538.1 | 0.29 | 3.41 | 1.78 | 0.04 | 46.7 | 0.02 | 214 | 247 |
| 64 | gpu-discover: prefix + quadtree | 536.2 | 0.29 | 3.47 | 1.79 | 0.04 | 46.7 | 0.02 | 215 | 247 |
| 65 | gpu-discover: rows + quadtree | 534.6 | 0.29 | 3.57 | 1.79 | 0.04 | 46.7 | 0.02 | 215 | 247 |
| 66 | gpu-discover: hier + quadtree | 523.1 | 0.28 | 3.58 | 1.83 | 0.05 | 46.7 | 0.02 | 214 | 247 |
| 67 | gpu-discover: clipmap + quadtree | 505.8 | 0.27 | 3.53 | 1.90 | 0.04 | 46.7 | 0.02 | 268 | 245 |
| 68 | gpu-compute: svo_dag + beam + sphere | 489.6 | 0.26 | 7.19 | 1.87 | 0.19 | 56.5 | 0.01 | 22 | 271 |
| 69 | gpu-compute: prefix + beam + incremental | 482.9 | 0.26 | 3.96 | 2.02 | 0.02 | 56.5 | 0.01 | 33 | 236 |
| 70 | gpu-compute: distance + beam + incremental | 477.9 | 0.26 | 20.10 | 1.60 | 0.02 | 56.5 | 0.01 | 40 | 236 |
| 71 | gpu-rt: greedy + rayquery | 470.8 | 0.25 | 35.09 | 0.94 | 1.07 | 56.4 | 0.01 | 307 | 613 |
| 72 | gpu-rt: naive + rayquery | 463.9 | 0.25 | 33.54 | 0.96 | 1.06 | 56.4 | 0.01 | 313 | 618 |
| 73 | gpu-discover: tree64 + quadtree | 442.0 | 0.24 | 3.88 | 2.17 | 0.12 | 46.7 | 0.02 | 200 | 258 |
| 74 | gpu-discover: distance + quadtree | 441.4 | 0.24 | 20.09 | 1.75 | 0.04 | 46.7 | 0.02 | 222 | 246 |
| 75 | gpu-raster: bricks + cubes + visbuffer | 421.6 | 0.23 | 4.44 | 2.32 | 0.07 | 46.8 | 0.02 | 49 | 244 |
| 76 | gpu-raster: bricks + cubes + raster | 414.0 | 0.22 | 4.39 | 2.37 | 0.07 | 46.7 | 0.02 | 49 | 245 |
| 77 | gpu-discover: svo_dag + quadtree | 390.1 | 0.21 | 7.19 | 2.37 | 0.22 | 46.7 | 0.02 | 204 | 277 |
| 78 | gpu-raster: bricks + cubes + gbuffer | 389.9 | 0.21 | 4.71 | 2.52 | 0.07 | 46.7 | 0.02 | 49 | 244 |
| 79 | gpu-compute: svo_dag + incremental | 387.8 | 0.21 | 6.94 | 2.42 | 0.19 | 56.2 | 0.01 | 22 | 271 |
| 80 | gpu-compute: tree64 + rowrun | 277.7 | 0.15 | 6.90 | 3.55 | 0.10 | 56.2 | 0.01 | 18 | 246 |
| 81 | gpu-compute: svo_dag + beam + incremental | 277.6 | 0.15 | 7.55 | 3.47 | 0.20 | 56.5 | 0.01 | 22 | 271 |
| 82 | gpu-compute: tree64 + beam + rowrun | 273.4 | 0.15 | 7.00 | 3.61 | 0.10 | 56.2 | 0.01 | 18 | 247 |
| 83 | gpu-compute: svo_dag + rowrun | 251.9 | 0.14 | 7.50 | 3.84 | 0.19 | 56.2 | 0.01 | 22 | 270 |
| 84 | gpu-compute: svo_dag + beam + rowrun | 205.2 | 0.11 | 8.36 | 4.75 | 0.21 | 56.5 | 0.01 | 22 | 271 |
| 85 | cpu: bricks + refloor | 140.6 | 0.08 | 10.97 | - | 7.05 | 59.7 | 0.00 | - | 51 |
| 86 | cpu: u8 + refloor | 136.3 | 0.07 | 10.93 | - | 7.26 | 59.7 | 0.00 | - | 45 |
| 87 | cpu: bits + refloor | 134.8 | 0.07 | 11.25 | - | 7.35 | 59.7 | 0.00 | - | 52 |
| 88 | cpu: morton + refloor | 134.7 | 0.07 | 11.34 | - | 7.34 | 59.7 | 0.00 | - | 52 |
| 89 | cpu: hier + refloor | 133.3 | 0.07 | 11.27 | - | 7.44 | 59.7 | 0.00 | - | 52 |
| 90 | cpu: brickmajor + refloor | 133.0 | 0.07 | 11.40 | - | 7.43 | 59.7 | 0.00 | - | 51 |
| 91 | cpu: palette + refloor | 123.9 | 0.07 | 11.92 | - | 7.96 | 59.7 | 0.00 | - | 42 |
| 92 | cpu: prefix + refloor | 114.3 | 0.06 | 12.70 | - | 8.67 | 59.9 | 0.00 | - | 41 |
| 93 | cpu: bricks + incremental | 97.5 | 0.05 | 14.35 | - | 10.19 | 59.7 | 0.00 | - | 51 |
| 94 | cpu: bits + incremental | 97.4 | 0.05 | 14.81 | - | 10.19 | 59.7 | 0.00 | - | 52 |
| 95 | cpu: u8 + incremental | 95.3 | 0.05 | 14.96 | - | 10.42 | 59.7 | 0.00 | - | 45 |
| 96 | cpu: hier + incremental | 94.9 | 0.05 | 14.96 | - | 10.47 | 59.7 | 0.00 | - | 52 |
| 97 | cpu: morton + incremental | 94.5 | 0.05 | 14.72 | - | 10.49 | 59.7 | 0.00 | - | 52 |
| 98 | cpu: brickmajor + incremental | 94.4 | 0.05 | 14.84 | - | 10.50 | 59.7 | 0.00 | - | 51 |
| 99 | cpu: palette + incremental | 88.6 | 0.05 | 15.67 | - | 11.17 | 59.7 | 0.00 | - | 41 |
| 100 | cpu: prefix + incremental | 80.4 | 0.04 | 17.12 | - | 12.35 | 59.7 | 0.00 | - | 41 |
| 101 | cpu-discover: u8 + cluster | 30.9 | 0.02 | 64.09 | 0.19 | 31.97 | 41.3 | 0.06 | 36 | 544 |
| 102 | cpu-discover: morton + cluster | 30.7 | 0.02 | 61.41 | 0.19 | 32.12 | 41.3 | 0.06 | 36 | 557 |
| 103 | cpu-discover: bricks + cluster | 30.5 | 0.02 | 63.10 | 0.19 | 32.43 | 41.3 | 0.06 | 36 | 529 |
| 104 | cpu-discover: brickmajor + cluster | 30.3 | 0.02 | 59.21 | 0.19 | 32.54 | 41.3 | 0.06 | 36 | 530 |
| 105 | cpu-discover: prefix + cluster | 30.2 | 0.02 | 62.06 | 0.19 | 32.71 | 41.3 | 0.06 | 36 | 562 |
| 106 | cpu-discover: palette + cluster | 30.1 | 0.02 | 63.25 | 0.19 | 32.75 | 41.3 | 0.06 | 36 | 511 |
| 107 | cpu-discover: bits + cluster | 29.6 | 0.02 | 70.54 | 0.19 | 33.46 | 41.3 | 0.06 | 36 | 525 |
| 108 | cpu-discover: hier + cluster | 29.4 | 0.02 | 63.45 | 0.19 | 33.66 | 41.3 | 0.06 | 36 | 572 |
| 109 | cpu-discover: bricks + quadtree | 26.1 | 0.01 | 78.99 | 0.19 | 37.86 | 40.5 | 0.07 | 36 | 981 |
| 110 | cpu-discover: u8 + quadtree | 26.1 | 0.01 | 71.34 | 0.19 | 37.93 | 40.5 | 0.07 | 36 | 1006 |
| 111 | cpu-discover: hier + quadtree | 26.0 | 0.01 | 70.81 | 0.19 | 37.99 | 40.5 | 0.07 | 36 | 981 |
| 112 | cpu-discover: bits + quadtree | 26.0 | 0.01 | 80.54 | 0.19 | 38.00 | 40.5 | 0.07 | 36 | 929 |
| 113 | cpu-discover: brickmajor + quadtree | 26.0 | 0.01 | 75.99 | 0.19 | 37.99 | 40.5 | 0.07 | 36 | 988 |
| 114 | cpu-discover: morton + quadtree | 25.8 | 0.01 | 86.45 | 0.19 | 38.32 | 40.5 | 0.07 | 36 | 1054 |
| 115 | cpu-discover: prefix + quadtree | 25.7 | 0.01 | 71.88 | 0.19 | 38.37 | 40.5 | 0.07 | 36 | 1023 |
| 116 | cpu-discover: palette + quadtree | 25.6 | 0.01 | 90.34 | 0.19 | 38.56 | 40.5 | 0.07 | 36 | 965 |
| 117 | cpu-raster: naive + raster | 8.6 | 0.00 | 139.15 | - | 111.08 | 61.5 | 0.00 | - | 634 |
| 118 | cpu-raster: greedy + raster | 8.5 | 0.00 | 138.58 | - | 112.60 | 61.7 | 0.00 | - | 532 |
| 119 | cpu-voxelspace: heightmap + columns (incorrect) | 491.1 | 0.26 | 3.50 | - | 1.97 | 10.4 | 69.49 | - | 60 |
| 120 | gpu-splat: bricks + points + splat (incorrect) | 319.5 | 0.17 | 10.76 | 2.87 | 0.02 | 14.7 | 28.66 | 56 | 265 |
| 121 | gpu-voxelspace: heightmap + columns (incorrect) | 225.4 | 0.12 | 9.87 | 4.39 | 0.02 | 10.4 | 69.49 | 9 | 235 |

### room [lit]

1920×1080. camera 0.00 blocks/frame (max 0.0), turn 0.00°/frame; 27 chunks at start, 0.0 loads/frame; chunk fill 4%, solid 0.1%, surface 0.06%; sky 47%, hits at 51 blocks (median 43); 0.0 edits/frame; 2 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-raster: hier + greedy + raster | 3494.9 | 1.00 | 1.92 | 0.19 | 0.01 | 72.7 | 0.00 | 30 | 216 |
| 2 | gpu-raster: svo_dag + greedy + raster | 3493.6 | 1.00 | 2.18 | 0.18 | 0.01 | 72.7 | 0.00 | 26 | 217 |
| 3 | gpu-raster: prefix + greedy + raster | 3469.0 | 0.99 | 2.01 | 0.18 | 0.01 | 72.7 | 0.00 | 31 | 217 |
| 4 | gpu-raster: prefix + greedy + visbuffer | 3437.7 | 0.98 | 1.82 | 0.19 | 0.01 | 72.6 | 0.00 | 31 | 217 |
| 5 | gpu-raster: rows + greedy + raster | 3264.7 | 0.93 | 1.70 | 0.20 | 0.01 | 72.7 | 0.00 | 30 | 217 |
| 6 | gpu-raster: tree64 + greedy + raster | 3242.2 | 0.93 | 2.33 | 0.19 | 0.01 | 72.7 | 0.00 | 26 | 217 |
| 7 | gpu-raster: hier + naive + visbuffer | 3192.6 | 0.91 | 2.00 | 0.22 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 8 | gpu-raster: svo_dag + greedy + visbuffer | 3180.2 | 0.91 | 2.00 | 0.21 | 0.01 | 72.6 | 0.00 | 26 | 217 |
| 9 | gpu-raster: hier + greedy + visbuffer | 3120.0 | 0.89 | 2.01 | 0.22 | 0.02 | 72.6 | 0.00 | 30 | 216 |
| 10 | gpu-raster: bricks + greedy + raster | 3105.5 | 0.89 | 2.00 | 0.21 | 0.01 | 72.7 | 0.00 | 30 | 216 |
| 11 | gpu-raster: bricks + greedy + visbuffer | 3064.2 | 0.88 | 1.82 | 0.23 | 0.01 | 72.6 | 0.00 | 30 | 216 |
| 12 | gpu-raster: clipmap + greedy + raster | 3025.3 | 0.87 | 2.05 | 0.23 | 0.01 | 72.7 | 0.00 | 49 | 217 |
| 13 | gpu-raster: tree64 + naive + visbuffer | 3018.1 | 0.86 | 2.00 | 0.23 | 0.02 | 61.2 | 0.00 | 26 | 217 |
| 14 | gpu-raster: tree64 + greedy + visbuffer | 3012.9 | 0.86 | 1.97 | 0.23 | 0.01 | 72.6 | 0.00 | 26 | 217 |
| 15 | gpu-raster: svo_dag + naive + visbuffer | 2994.0 | 0.86 | 1.93 | 0.23 | 0.01 | 61.2 | 0.00 | 26 | 216 |
| 16 | gpu-raster: prefix + naive + visbuffer | 2861.1 | 0.82 | 2.46 | 0.24 | 0.01 | 61.2 | 0.00 | 31 | 217 |
| 17 | gpu-raster: hier + naive + raster | 2839.1 | 0.81 | 2.14 | 0.26 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 18 | gpu-raster: svo_dag + greedy + gbuffer | 2819.7 | 0.81 | 1.99 | 0.26 | 0.01 | 72.6 | 0.00 | 26 | 217 |
| 19 | gpu-raster: svo_dag + cubes + visbuffer | 2793.1 | 0.80 | 1.76 | 0.27 | 0.01 | 61.2 | 0.00 | 26 | 217 |
| 20 | gpu-raster: svo_dag + naive + raster | 2780.7 | 0.80 | 1.96 | 0.26 | 0.01 | 61.2 | 0.00 | 26 | 217 |
| 21 | gpu-raster: clipmap + greedy + visbuffer | 2776.4 | 0.79 | 2.33 | 0.25 | 0.01 | 72.6 | 0.00 | 49 | 217 |
| 22 | gpu-raster: prefix + naive + raster | 2766.1 | 0.79 | 1.85 | 0.26 | 0.01 | 61.2 | 0.00 | 31 | 217 |
| 23 | gpu-raster: tree64 + naive + raster | 2761.3 | 0.79 | 1.76 | 0.27 | 0.01 | 61.2 | 0.00 | 26 | 217 |
| 24 | gpu-raster: hier + greedy + gbuffer | 2752.2 | 0.79 | 2.28 | 0.26 | 0.02 | 72.6 | 0.00 | 30 | 217 |
| 25 | gpu-raster: rows + greedy + visbuffer | 2702.1 | 0.77 | 3.12 | 0.24 | 0.02 | 72.6 | 0.00 | 30 | 217 |
| 26 | gpu-raster: clipmap + naive + visbuffer | 2694.7 | 0.77 | 2.22 | 0.27 | 0.01 | 61.2 | 0.00 | 49 | 217 |
| 27 | gpu-raster: prefix + greedy + gbuffer | 2689.0 | 0.77 | 1.91 | 0.27 | 0.01 | 72.6 | 0.00 | 31 | 217 |
| 28 | gpu-raster: prefix + cubes + visbuffer | 2655.6 | 0.76 | 2.20 | 0.28 | 0.02 | 61.2 | 0.00 | 31 | 217 |
| 29 | gpu-raster: tree64 + greedy + gbuffer | 2588.1 | 0.74 | 2.19 | 0.28 | 0.01 | 72.6 | 0.00 | 26 | 216 |
| 30 | gpu-raster: hier + cubes + visbuffer | 2580.0 | 0.74 | 2.10 | 0.29 | 0.02 | 61.2 | 0.00 | 30 | 216 |
| 31 | gpu-raster: tree64 + cubes + visbuffer | 2561.7 | 0.73 | 2.27 | 0.29 | 0.01 | 61.2 | 0.00 | 26 | 216 |
| 32 | gpu-raster: bricks + naive + raster | 2550.0 | 0.73 | 2.09 | 0.30 | 0.01 | 61.2 | 0.00 | 30 | 216 |
| 33 | gpu-raster: svo_dag + naive + gbuffer | 2535.9 | 0.73 | 2.36 | 0.29 | 0.01 | 61.2 | 0.00 | 26 | 216 |
| 34 | gpu-raster: bricks + greedy + gbuffer | 2525.6 | 0.72 | 2.19 | 0.29 | 0.02 | 72.6 | 0.00 | 30 | 217 |
| 35 | gpu-raster: bricks + naive + visbuffer | 2512.0 | 0.72 | 2.37 | 0.27 | 0.02 | 61.2 | 0.00 | 30 | 217 |
| 36 | gpu-raster: rows + cubes + visbuffer | 2510.4 | 0.72 | 1.71 | 0.30 | 0.01 | 61.2 | 0.00 | 30 | 216 |
| 37 | gpu-raster: prefix + naive + gbuffer | 2494.1 | 0.71 | 2.20 | 0.29 | 0.01 | 61.2 | 0.00 | 31 | 217 |
| 38 | gpu-raster: clipmap + naive + raster | 2485.6 | 0.71 | 1.99 | 0.30 | 0.01 | 61.2 | 0.00 | 49 | 216 |
| 39 | gpu-raster: bricks + cubes + visbuffer | 2475.8 | 0.71 | 1.88 | 0.31 | 0.01 | 61.2 | 0.00 | 30 | 216 |
| 40 | gpu-raster: tree64 + naive + gbuffer | 2455.2 | 0.70 | 2.39 | 0.31 | 0.01 | 61.2 | 0.00 | 26 | 217 |
| 41 | gpu-raster: rows + naive + raster | 2437.1 | 0.70 | 2.42 | 0.30 | 0.01 | 61.2 | 0.00 | 30 | 216 |
| 42 | gpu-raster: hier + naive + gbuffer | 2410.4 | 0.69 | 2.27 | 0.32 | 0.02 | 61.2 | 0.00 | 30 | 216 |
| 43 | gpu-raster: clipmap + greedy + gbuffer | 2408.1 | 0.69 | 2.55 | 0.31 | 0.02 | 72.6 | 0.00 | 49 | 217 |
| 44 | gpu-raster: distance + cubes + visbuffer | 2353.8 | 0.67 | 2.14 | 0.31 | 0.02 | 61.2 | 0.00 | 32 | 217 |
| 45 | gpu-raster: clipmap + cubes + visbuffer | 2352.8 | 0.67 | 2.31 | 0.33 | 0.02 | 61.2 | 0.00 | 49 | 217 |
| 46 | gpu-raster: distance + greedy + raster | 2333.0 | 0.67 | 3.44 | 0.21 | 0.02 | 72.7 | 0.00 | 32 | 217 |
| 47 | gpu-raster: bricks + naive + gbuffer | 2276.1 | 0.65 | 2.50 | 0.34 | 0.02 | 61.2 | 0.00 | 30 | 217 |
| 48 | gpu-raster: svo_dag + cubes + raster | 2248.9 | 0.64 | 2.25 | 0.35 | 0.01 | 61.2 | 0.00 | 26 | 217 |
| 49 | gpu-raster: clipmap + naive + gbuffer | 2213.1 | 0.63 | 2.93 | 0.34 | 0.02 | 61.2 | 0.00 | 49 | 217 |
| 50 | gpu-raster: hier + cubes + raster | 2208.8 | 0.63 | 2.20 | 0.35 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 51 | gpu-raster: prefix + cubes + raster | 2176.6 | 0.62 | 2.32 | 0.35 | 0.01 | 61.2 | 0.00 | 31 | 217 |
| 52 | gpu-raster: tree64 + cubes + raster | 2160.1 | 0.62 | 2.35 | 0.36 | 0.01 | 61.2 | 0.00 | 26 | 216 |
| 53 | gpu-raster: distance + greedy + gbuffer | 2142.2 | 0.61 | 2.96 | 0.31 | 0.02 | 72.6 | 0.00 | 32 | 217 |
| 54 | gpu-raster: rows + cubes + raster | 2096.8 | 0.60 | 2.21 | 0.36 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 55 | gpu-rt: greedy + rayquery | 2095.1 | 0.60 | 2.21 | 0.38 | 0.06 | 76.3 | 0.00 | 2 | 350 |
| 56 | gpu-compute: distance + sphere | 2081.2 | 0.60 | 2.39 | 0.39 | 0.01 | 76.3 | 0.00 | 22 | 215 |
| 57 | gpu-compute: distance + beam + sphere | 2047.8 | 0.59 | 2.03 | 0.42 | 0.01 | 76.3 | 0.00 | 22 | 216 |
| 58 | gpu-raster: distance + naive + gbuffer | 2041.4 | 0.58 | 3.63 | 0.34 | 0.02 | 61.2 | 0.00 | 32 | 217 |
| 59 | gpu-raster: distance + naive + raster | 2031.3 | 0.58 | 3.65 | 0.29 | 0.02 | 61.2 | 0.00 | 32 | 217 |
| 60 | gpu-raster: rows + greedy + gbuffer | 2017.3 | 0.58 | 4.43 | 0.31 | 0.03 | 72.6 | 0.00 | 30 | 217 |
| 61 | gpu-raster: rows + cubes + gbuffer | 2000.1 | 0.57 | 1.84 | 0.41 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 62 | gpu-rt: naive + rayquery | 1964.1 | 0.56 | 3.50 | 0.39 | 0.06 | 76.3 | 0.00 | 4 | 352 |
| 63 | gpu-raster: svo_dag + cubes + gbuffer | 1955.2 | 0.56 | 3.33 | 0.40 | 0.01 | 61.2 | 0.00 | 26 | 216 |
| 64 | gpu-raster: rows + naive + gbuffer | 1952.6 | 0.56 | 4.36 | 0.35 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 65 | gpu-raster: bricks + cubes + raster | 1929.7 | 0.55 | 4.49 | 0.39 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 66 | gpu-raster: distance + greedy + visbuffer | 1918.8 | 0.55 | 3.50 | 0.26 | 0.03 | 72.6 | 0.00 | 32 | 216 |
| 67 | gpu-raster: clipmap + cubes + raster | 1910.7 | 0.55 | 2.09 | 0.41 | 0.01 | 61.2 | 0.00 | 49 | 217 |
| 68 | gpu-raster: prefix + cubes + gbuffer | 1901.1 | 0.54 | 2.90 | 0.40 | 0.01 | 61.2 | 0.00 | 31 | 217 |
| 69 | gpu-raster: distance + cubes + raster | 1852.6 | 0.53 | 2.76 | 0.42 | 0.02 | 61.2 | 0.00 | 32 | 217 |
| 70 | gpu-raster: hier + cubes + gbuffer | 1844.5 | 0.53 | 4.15 | 0.41 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 71 | gpu-raster: tree64 + cubes + gbuffer | 1840.9 | 0.53 | 2.98 | 0.42 | 0.01 | 61.2 | 0.00 | 26 | 217 |
| 72 | gpu-raster: bricks + cubes + gbuffer | 1794.8 | 0.51 | 4.30 | 0.43 | 0.01 | 61.2 | 0.00 | 30 | 217 |
| 73 | gpu-raster: clipmap + cubes + gbuffer | 1578.1 | 0.45 | 4.16 | 0.50 | 0.01 | 61.2 | 0.00 | 49 | 217 |
| 74 | gpu-raster: distance + naive + visbuffer | 1558.4 | 0.45 | 5.32 | 0.28 | 0.03 | 61.2 | 0.00 | 32 | 216 |
| 75 | gpu-raster: distance + cubes + gbuffer | 1520.9 | 0.44 | 3.74 | 0.49 | 0.01 | 61.2 | 0.00 | 32 | 217 |
| 76 | gpu-compute: distance + refloor | 1438.6 | 0.41 | 2.26 | 0.63 | 0.01 | 76.3 | 0.00 | 22 | 216 |
| 77 | gpu-compute: distance + rowrun | 1407.3 | 0.40 | 2.26 | 0.64 | 0.01 | 76.3 | 0.00 | 22 | 216 |
| 78 | gpu-compute: hier + refloor | 1352.4 | 0.39 | 2.13 | 0.67 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 79 | gpu-compute: distance + beam + refloor | 1332.4 | 0.38 | 2.22 | 0.68 | 0.01 | 76.3 | 0.00 | 22 | 216 |
| 80 | gpu-compute: distance + beam + rowrun | 1328.5 | 0.38 | 2.24 | 0.69 | 0.01 | 76.3 | 0.00 | 22 | 216 |
| 81 | gpu-compute: hier + beam + refloor | 1307.7 | 0.37 | 2.34 | 0.70 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 82 | gpu-compute: prefix + refloor | 1305.2 | 0.37 | 2.32 | 0.70 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 83 | gpu-compute: prefix + beam + rowrun | 1242.1 | 0.36 | 2.36 | 0.72 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 84 | gpu-compute: prefix + rowrun | 1228.7 | 0.35 | 2.40 | 0.75 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 85 | gpu-compute: hier + rowrun | 1217.0 | 0.35 | 2.50 | 0.76 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 86 | gpu-compute: prefix + beam + refloor | 1213.1 | 0.35 | 2.52 | 0.76 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 87 | gpu-compute: hier + beam + rowrun | 1205.1 | 0.34 | 2.40 | 0.76 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 88 | gpu-compute: svo_dag + refloor | 1201.9 | 0.34 | 5.00 | 0.69 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 89 | gpu-compute: svo_dag + beam + refloor | 1193.6 | 0.34 | 4.50 | 0.72 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 90 | gpu-compute: svo_dag + rowrun | 1164.6 | 0.33 | 5.00 | 0.73 | 0.01 | 76.3 | 0.00 | 16 | 216 |
| 91 | gpu-discover: prefix + quadtree | 1162.2 | 0.33 | 4.82 | 0.70 | 0.03 | 61.2 | 0.00 | 202 | 224 |
| 92 | gpu-compute: svo_dag + beam + rowrun | 1146.5 | 0.33 | 4.76 | 0.74 | 0.01 | 76.3 | 0.00 | 16 | 216 |
| 93 | gpu-rt: boxes + rayquery | 1141.1 | 0.33 | 4.64 | 0.75 | 0.06 | 74.5 | 0.00 | 6 | 350 |
| 94 | gpu-raster: rows + naive + visbuffer | 1135.6 | 0.32 | 11.79 | 0.27 | 0.02 | 61.2 | 0.00 | 30 | 217 |
| 95 | gpu-discover: hier + quadtree | 1109.0 | 0.32 | 4.78 | 0.75 | 0.02 | 61.2 | 0.00 | 202 | 224 |
| 96 | gpu-compute: bricks + refloor | 1077.9 | 0.31 | 2.72 | 0.86 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 97 | gpu-discover: distance + quadtree | 1075.0 | 0.31 | 4.98 | 0.77 | 0.03 | 61.2 | 0.00 | 204 | 224 |
| 98 | gpu-compute: rows + refloor | 1063.6 | 0.30 | 2.64 | 0.88 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 99 | gpu-discover: bricks + quadtree | 1058.2 | 0.30 | 5.02 | 0.79 | 0.03 | 61.2 | 0.00 | 202 | 224 |
| 100 | gpu-compute: rows + beam + refloor | 1045.8 | 0.30 | 2.77 | 0.89 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 101 | gpu-discover: rows + quadtree | 1042.9 | 0.30 | 4.74 | 0.81 | 0.02 | 61.2 | 0.00 | 202 | 224 |
| 102 | gpu-compute: bricks + beam + refloor | 1040.5 | 0.30 | 2.55 | 0.89 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 103 | gpu-compute: rows + rowrun | 1033.0 | 0.30 | 2.74 | 0.90 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 104 | gpu-discover: svo_dag + quadtree | 1005.7 | 0.29 | 4.81 | 0.83 | 0.03 | 61.2 | 0.00 | 198 | 225 |
| 105 | gpu-compute: rows + beam + rowrun | 1004.1 | 0.29 | 2.78 | 0.91 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 106 | gpu-discover: tree64 + quadtree | 998.9 | 0.29 | 4.75 | 0.85 | 0.03 | 61.2 | 0.00 | 198 | 225 |
| 107 | gpu-compute: bricks + beam + rowrun | 975.2 | 0.28 | 2.50 | 0.96 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 108 | gpu-compute: bricks + rowrun | 973.5 | 0.28 | 2.65 | 0.97 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 109 | gpu-discover: clipmap + quadtree | 925.4 | 0.26 | 4.78 | 0.93 | 0.03 | 61.2 | 0.00 | 221 | 224 |
| 110 | gpu-compute: tree64 + rowrun | 920.4 | 0.26 | 4.85 | 0.96 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 111 | gpu-compute: tree64 + beam + rowrun | 902.1 | 0.26 | 4.79 | 0.98 | 0.01 | 76.3 | 0.00 | 16 | 216 |
| 112 | gpu-compute: tree64 + beam + refloor | 898.6 | 0.26 | 5.09 | 0.99 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 113 | gpu-compute: tree64 + refloor | 897.4 | 0.26 | 4.80 | 0.96 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 114 | gpu-compute: clipmap + refloor | 885.5 | 0.25 | 5.05 | 1.00 | 0.01 | 76.3 | 0.00 | 39 | 215 |
| 115 | gpu-compute: clipmap + beam + refloor | 873.7 | 0.25 | 5.07 | 1.02 | 0.01 | 76.3 | 0.00 | 39 | 216 |
| 116 | gpu-compute: clipmap + rowrun | 838.3 | 0.24 | 5.27 | 1.07 | 0.01 | 76.3 | 0.00 | 39 | 215 |
| 117 | gpu-compute: clipmap + beam + rowrun | 833.2 | 0.24 | 4.98 | 1.08 | 0.01 | 76.3 | 0.00 | 39 | 216 |
| 118 | gpu-compute: clipmap + incremental | 759.4 | 0.22 | 5.53 | 1.20 | 0.01 | 76.3 | 0.00 | 39 | 216 |
| 119 | gpu-compute: clipmap + beam + incremental | 743.7 | 0.21 | 5.77 | 1.21 | 0.01 | 76.3 | 0.00 | 39 | 216 |
| 120 | gpu-compute: tree64 + incremental | 731.7 | 0.21 | 5.27 | 1.25 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 121 | gpu-compute: tree64 + beam + incremental | 730.2 | 0.21 | 5.11 | 1.25 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 122 | gpu-compute: hier + incremental | 698.7 | 0.20 | 3.12 | 1.38 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 123 | gpu-compute: distance + incremental | 690.7 | 0.20 | 3.21 | 1.38 | 0.01 | 76.3 | 0.00 | 22 | 215 |
| 124 | gpu-compute: hier + beam + incremental | 677.8 | 0.19 | 3.30 | 1.41 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 125 | gpu-compute: distance + beam + incremental | 654.1 | 0.19 | 3.30 | 1.46 | 0.01 | 76.3 | 0.00 | 22 | 215 |
| 126 | gpu-compute: bricks + incremental | 644.6 | 0.18 | 3.49 | 1.49 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 127 | gpu-compute: bricks + beam + incremental | 631.7 | 0.18 | 3.35 | 1.53 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 128 | gpu-compute: prefix + incremental | 628.6 | 0.18 | 3.40 | 1.54 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 129 | gpu-compute: rows + incremental | 619.1 | 0.18 | 3.31 | 1.56 | 0.01 | 76.3 | 0.00 | 20 | 215 |
| 130 | gpu-compute: prefix + beam + incremental | 612.5 | 0.18 | 3.42 | 1.57 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 131 | gpu-compute: rows + beam + incremental | 582.9 | 0.17 | 3.59 | 1.65 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 132 | gpu-compute: svo_dag + beam + incremental | 430.0 | 0.12 | 6.10 | 2.22 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 133 | gpu-compute: svo_dag + incremental | 429.1 | 0.12 | 6.84 | 2.22 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 134 | gpu-compute: hier + sphere | 424.9 | 0.12 | 4.15 | 2.30 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 135 | gpu-compute: hier + beam + sphere | 420.1 | 0.12 | 4.09 | 2.33 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 136 | gpu-compute: svo_dag + sphere | 377.5 | 0.11 | 6.97 | 2.55 | 0.02 | 76.3 | 0.00 | 16 | 216 |
| 137 | gpu-compute: svo_dag + beam + sphere | 369.1 | 0.11 | 6.58 | 2.60 | 0.01 | 76.3 | 0.00 | 16 | 217 |
| 138 | gpu-compute: rows + sphere | 359.0 | 0.10 | 4.35 | 2.74 | 0.02 | 76.3 | 0.00 | 20 | 216 |
| 139 | gpu-compute: prefix + sphere | 358.7 | 0.10 | 5.81 | 2.74 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 140 | gpu-compute: prefix + beam + sphere | 357.9 | 0.10 | 4.63 | 2.75 | 0.02 | 76.3 | 0.00 | 20 | 216 |
| 141 | gpu-compute: bricks + sphere | 355.2 | 0.10 | 4.91 | 2.77 | 0.01 | 76.3 | 0.00 | 20 | 216 |
| 142 | gpu-compute: rows + beam + sphere | 354.9 | 0.10 | 4.73 | 2.77 | 0.02 | 76.3 | 0.00 | 20 | 216 |
| 143 | gpu-compute: bricks + beam + sphere | 351.5 | 0.10 | 4.67 | 2.80 | 0.02 | 76.3 | 0.00 | 20 | 215 |
| 144 | gpu-compute: tree64 + beam + sphere | 336.0 | 0.10 | 7.42 | 2.87 | 0.02 | 76.3 | 0.00 | 16 | 217 |
| 145 | gpu-compute: tree64 + sphere | 334.4 | 0.10 | 6.77 | 2.87 | 0.02 | 76.3 | 0.00 | 16 | 217 |
| 146 | gpu-compute: clipmap + sphere | 317.2 | 0.09 | 7.20 | 3.04 | 0.01 | 76.3 | 0.00 | 39 | 216 |
| 147 | gpu-compute: clipmap + beam + sphere | 310.5 | 0.09 | 7.72 | 3.12 | 0.01 | 76.3 | 0.00 | 39 | 215 |
| 148 | cpu-discover: brickmajor + quadtree | 205.9 | 0.06 | 9.30 | 0.42 | 4.60 | 61.2 | 0.00 | 24 | 332 |
| 149 | cpu-discover: u8 + quadtree | 203.1 | 0.06 | 9.58 | 0.41 | 4.67 | 61.2 | 0.00 | 24 | 327 |
| 150 | cpu-discover: prefix + quadtree | 203.0 | 0.06 | 9.57 | 0.41 | 4.66 | 61.2 | 0.00 | 24 | 338 |
| 151 | cpu-discover: morton + quadtree | 202.7 | 0.06 | 9.53 | 0.42 | 4.68 | 61.2 | 0.00 | 24 | 327 |
| 152 | cpu-discover: bits + quadtree | 202.0 | 0.06 | 9.95 | 0.41 | 4.69 | 61.2 | 0.00 | 24 | 331 |
| 153 | cpu-discover: palette + quadtree | 200.4 | 0.06 | 9.84 | 0.41 | 4.73 | 61.2 | 0.00 | 24 | 328 |
| 154 | cpu-discover: hier + quadtree | 199.3 | 0.06 | 10.01 | 0.41 | 4.75 | 61.2 | 0.00 | 24 | 335 |
| 155 | cpu-discover: bricks + quadtree | 192.1 | 0.05 | 9.94 | 0.42 | 4.93 | 61.2 | 0.00 | 24 | 335 |
| 156 | cpu-discover: prefix + cluster | 147.1 | 0.04 | 11.09 | 0.42 | 6.55 | 61.2 | 0.00 | 24 | 245 |
| 157 | cpu-discover: morton + cluster | 140.9 | 0.04 | 11.11 | 0.41 | 6.84 | 61.2 | 0.00 | 24 | 247 |
| 158 | cpu-discover: bits + cluster | 139.9 | 0.04 | 11.45 | 0.42 | 6.89 | 61.2 | 0.00 | 24 | 246 |
| 159 | cpu-discover: u8 + cluster | 139.9 | 0.04 | 12.00 | 0.40 | 6.89 | 61.2 | 0.00 | 24 | 245 |
| 160 | cpu-discover: brickmajor + cluster | 139.8 | 0.04 | 11.40 | 0.41 | 6.88 | 61.2 | 0.00 | 24 | 249 |
| 161 | cpu-discover: hier + cluster | 139.1 | 0.04 | 11.03 | 0.41 | 6.92 | 61.2 | 0.00 | 24 | 249 |
| 162 | cpu-discover: palette + cluster | 138.4 | 0.04 | 11.82 | 0.41 | 6.96 | 61.2 | 0.00 | 24 | 248 |
| 163 | cpu-discover: bricks + cluster | 135.9 | 0.04 | 11.51 | 0.42 | 7.08 | 61.2 | 0.00 | 24 | 245 |
| 164 | cpu-raster: greedy + raster | 82.1 | 0.02 | 16.82 | - | 12.05 | 99.0 | 0.00 | - | 51 |
| 165 | cpu-raster: naive + raster | 75.5 | 0.02 | 16.49 | - | 13.08 | 99.0 | 0.00 | - | 68 |
| 166 | cpu: hier + refloor | 43.9 | 0.01 | 27.40 | - | 22.75 | 76.5 | 0.00 | - | 33 |
| 167 | cpu: prefix + refloor | 37.8 | 0.01 | 31.11 | - | 26.38 | 76.5 | 0.00 | - | 31 |
| 168 | cpu: hier + incremental | 35.3 | 0.01 | 31.93 | - | 28.27 | 76.5 | 0.00 | - | 33 |
| 169 | cpu: bricks + refloor | 34.5 | 0.01 | 33.29 | - | 28.92 | 76.5 | 0.00 | - | 32 |
| 170 | cpu: u8 + refloor | 33.7 | 0.01 | 34.01 | - | 29.64 | 76.5 | 0.00 | - | 32 |
| 171 | cpu: bits + refloor | 33.6 | 0.01 | 33.99 | - | 29.72 | 76.5 | 0.00 | - | 33 |
| 172 | cpu: brickmajor + refloor | 33.4 | 0.01 | 34.43 | - | 29.89 | 76.5 | 0.00 | - | 32 |
| 173 | cpu: morton + refloor | 33.3 | 0.01 | 36.22 | - | 30.01 | 76.5 | 0.00 | - | 33 |
| 174 | cpu: palette + refloor | 32.9 | 0.01 | 34.89 | - | 30.32 | 76.5 | 0.00 | - | 31 |
| 175 | cpu: prefix + incremental | 32.2 | 0.01 | 35.24 | - | 31.05 | 76.5 | 0.00 | - | 32 |
| 176 | cpu: bricks + incremental | 31.4 | 0.01 | 37.57 | - | 31.83 | 76.5 | 0.00 | - | 31 |
| 177 | cpu: u8 + incremental | 31.0 | 0.01 | 37.02 | - | 32.17 | 76.5 | 0.00 | - | 32 |
| 178 | cpu: morton + incremental | 30.7 | 0.01 | 37.51 | - | 32.57 | 76.5 | 0.00 | - | 33 |
| 179 | cpu: palette + incremental | 30.4 | 0.01 | 37.98 | - | 32.88 | 76.5 | 0.00 | - | 31 |
| 180 | cpu: brickmajor + incremental | 30.3 | 0.01 | 37.95 | - | 32.98 | 76.5 | 0.00 | - | 33 |
| 181 | cpu: bits + incremental | 29.9 | 0.01 | 41.49 | - | 33.41 | 76.5 | 0.00 | - | 33 |
| 182 | gpu-splat: prefix + points + splat (incorrect) | 1926.6 | 0.55 | 3.70 | 0.39 | 0.01 | 10.7 | 30.42 | 28 | 216 |
| 183 | gpu-splat: svo_dag + points + splat (incorrect) | 1908.0 | 0.55 | 3.22 | 0.41 | 0.00 | 10.7 | 30.42 | 24 | 216 |
| 184 | gpu-splat: hier + points + splat (incorrect) | 1759.9 | 0.50 | 4.26 | 0.43 | 0.01 | 10.7 | 30.42 | 28 | 215 |
| 185 | gpu-splat: tree64 + points + splat (incorrect) | 1700.1 | 0.49 | 4.27 | 0.46 | 0.01 | 10.7 | 30.42 | 24 | 217 |
| 186 | gpu-splat: distance + points + splat (incorrect) | 1664.8 | 0.48 | 4.49 | 0.47 | 0.01 | 10.7 | 30.42 | 30 | 215 |
| 187 | gpu-splat: bricks + points + splat (incorrect) | 1543.0 | 0.44 | 4.31 | 0.52 | 0.01 | 10.7 | 30.42 | 28 | 215 |
| 188 | gpu-splat: rows + points + splat (incorrect) | 1535.6 | 0.44 | 4.42 | 0.52 | 0.01 | 10.7 | 30.42 | 28 | 216 |
| 189 | gpu-splat: clipmap + points + splat (incorrect) | 1464.9 | 0.42 | 4.57 | 0.55 | 0.01 | 10.7 | 30.42 | 47 | 216 |
| 190 | gpu-rt: greedy + rayquery + pathtrace (incorrect) | 1127.4 | 0.32 | 4.71 | 0.76 | 0.07 | 30.5 | 8.58 | 34 | 350 |
| 191 | gpu-rt: naive + rayquery + pathtrace (incorrect) | 1027.0 | 0.29 | 5.07 | 0.85 | 0.06 | 30.5 | 8.58 | 36 | 352 |
| 192 | gpu-compute: distance + sphere + pathtrace (incorrect) | 631.8 | 0.18 | 3.25 | 1.53 | 0.01 | 30.5 | 8.58 | 54 | 216 |
| 193 | gpu-compute: distance + beam + sphere + pathtrace (incorrect) | 615.5 | 0.18 | 3.52 | 1.56 | 0.01 | 30.5 | 8.58 | 54 | 216 |
| 194 | gpu-compute: distance + beam + refloor + pathtrace (incorrect) | 482.8 | 0.14 | 3.72 | 2.02 | 0.01 | 30.5 | 8.58 | 54 | 216 |
| 195 | gpu-compute: distance + refloor + pathtrace (incorrect) | 474.9 | 0.14 | 4.02 | 2.04 | 0.01 | 30.5 | 8.58 | 54 | 216 |
| 196 | gpu-compute: svo_dag + beam + refloor + pathtrace (incorrect) | 458.4 | 0.13 | 6.14 | 2.07 | 0.01 | 30.5 | 8.58 | 48 | 217 |
| 197 | gpu-compute: prefix + beam + refloor + pathtrace (incorrect) | 453.1 | 0.13 | 3.80 | 2.16 | 0.01 | 30.5 | 8.58 | 52 | 216 |
| 198 | gpu-compute: prefix + refloor + pathtrace (incorrect) | 450.8 | 0.13 | 3.93 | 2.17 | 0.01 | 30.5 | 8.58 | 52 | 216 |
| 199 | gpu-compute: svo_dag + refloor + pathtrace (incorrect) | 448.9 | 0.13 | 6.22 | 2.13 | 0.01 | 30.5 | 8.58 | 48 | 217 |
| 200 | gpu-compute: hier + beam + refloor + pathtrace (incorrect) | 446.7 | 0.13 | 4.07 | 2.19 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 201 | gpu-compute: hier + refloor + pathtrace (incorrect) | 438.2 | 0.13 | 4.00 | 2.23 | 0.01 | 30.5 | 8.58 | 52 | 216 |
| 202 | gpu-compute: bricks + beam + refloor + pathtrace (incorrect) | 403.4 | 0.12 | 4.16 | 2.43 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 203 | gpu-compute: rows + beam + refloor + pathtrace (incorrect) | 394.7 | 0.11 | 4.36 | 2.49 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 204 | gpu-compute: bricks + refloor + pathtrace (incorrect) | 385.8 | 0.11 | 4.55 | 2.54 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 205 | gpu-compute: rows + refloor + pathtrace (incorrect) | 380.7 | 0.11 | 4.49 | 2.58 | 0.01 | 30.5 | 8.58 | 52 | 216 |
| 206 | gpu-compute: prefix + rowrun + pathtrace (incorrect) | 373.3 | 0.11 | 4.70 | 2.63 | 0.01 | 30.5 | 8.58 | 52 | 217 |
| 207 | gpu-compute: distance + beam + rowrun + pathtrace (incorrect) | 368.2 | 0.11 | 4.38 | 2.67 | 0.02 | 30.5 | 8.58 | 54 | 216 |
| 208 | gpu-compute: prefix + beam + rowrun + pathtrace (incorrect) | 367.5 | 0.11 | 4.65 | 2.67 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 209 | gpu-compute: tree64 + beam + refloor + pathtrace (incorrect) | 366.0 | 0.10 | 6.77 | 2.63 | 0.02 | 30.5 | 8.58 | 48 | 217 |
| 210 | gpu-compute: distance + rowrun + pathtrace (incorrect) | 363.4 | 0.10 | 4.62 | 2.71 | 0.01 | 30.5 | 8.58 | 54 | 216 |
| 211 | gpu-compute: tree64 + refloor + pathtrace (incorrect) | 361.6 | 0.10 | 6.64 | 2.65 | 0.01 | 30.5 | 8.58 | 48 | 216 |
| 212 | gpu-rt: boxes + rayquery + pathtrace (incorrect) | 347.5 | 0.10 | 7.12 | 2.78 | 0.11 | 30.5 | 8.58 | 38 | 350 |
| 213 | gpu-compute: clipmap + refloor + pathtrace (incorrect) | 329.7 | 0.09 | 6.85 | 2.94 | 0.02 | 30.5 | 8.58 | 71 | 216 |
| 214 | gpu-compute: clipmap + beam + refloor + pathtrace (incorrect) | 327.1 | 0.09 | 7.08 | 2.97 | 0.02 | 30.5 | 8.58 | 71 | 215 |
| 215 | gpu-compute: rows + beam + rowrun + pathtrace (incorrect) | 309.4 | 0.09 | 4.95 | 3.19 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 216 | gpu-compute: rows + rowrun + pathtrace (incorrect) | 305.2 | 0.09 | 6.64 | 3.23 | 0.01 | 30.5 | 8.58 | 52 | 216 |
| 217 | cpu-voxelspace: heightmap + columns (incorrect) | 261.4 | 0.07 | 8.06 | - | 3.73 | 15.3 | 18.26 | - | 41 |
| 218 | gpu-compute: bricks + rowrun + pathtrace (incorrect) | 259.1 | 0.07 | 5.64 | 3.82 | 0.01 | 30.5 | 8.58 | 52 | 215 |
| 219 | gpu-compute: bricks + beam + rowrun + pathtrace (incorrect) | 242.5 | 0.07 | 6.00 | 4.08 | 0.01 | 30.5 | 8.58 | 52 | 216 |
| 220 | gpu-compute: clipmap + beam + rowrun + pathtrace (incorrect) | 236.1 | 0.07 | 8.32 | 4.16 | 0.01 | 30.5 | 8.58 | 71 | 216 |
| 221 | gpu-compute: clipmap + beam + incremental + pathtrace (incorrect) | 233.2 | 0.07 | 8.73 | 4.21 | 0.02 | 30.5 | 8.58 | 71 | 215 |
| 222 | gpu-compute: clipmap + rowrun + pathtrace (incorrect) | 232.5 | 0.07 | 8.73 | 4.23 | 0.02 | 30.5 | 8.58 | 71 | 216 |
| 223 | gpu-compute: clipmap + incremental + pathtrace (incorrect) | 231.6 | 0.07 | 8.69 | 4.24 | 0.02 | 30.5 | 8.58 | 71 | 215 |
| 224 | gpu-voxelspace: heightmap + columns (incorrect) | 217.9 | 0.06 | 8.90 | 4.52 | 0.01 | 15.3 | 18.26 | 9 | 215 |
| 225 | gpu-compute: tree64 + incremental + pathtrace (incorrect) | 211.9 | 0.06 | 9.30 | 4.65 | 0.02 | 30.5 | 8.58 | 48 | 216 |
| 226 | gpu-compute: hier + rowrun + pathtrace (incorrect) | 210.0 | 0.06 | 6.21 | 4.72 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 227 | gpu-compute: hier + beam + rowrun + pathtrace (incorrect) | 198.9 | 0.06 | 6.55 | 4.97 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 228 | gpu-compute: tree64 + beam + incremental + pathtrace (incorrect) | 197.6 | 0.06 | 10.16 | 4.99 | 0.02 | 30.5 | 8.58 | 48 | 217 |
| 229 | gpu-compute: hier + sphere + pathtrace (incorrect) | 172.6 | 0.05 | 7.58 | 5.76 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 230 | gpu-compute: hier + beam + sphere + pathtrace (incorrect) | 171.8 | 0.05 | 7.45 | 5.78 | 0.03 | 30.5 | 8.58 | 52 | 216 |
| 231 | gpu-compute: svo_dag + beam + rowrun + pathtrace (incorrect) | 169.2 | 0.05 | 10.36 | 5.85 | 0.03 | 30.5 | 8.58 | 48 | 217 |
| 232 | gpu-compute: svo_dag + rowrun + pathtrace (incorrect) | 162.5 | 0.05 | 10.83 | 6.06 | 0.03 | 30.5 | 8.58 | 48 | 217 |
| 233 | gpu-compute: bricks + sphere + pathtrace (incorrect) | 152.2 | 0.04 | 8.22 | 6.53 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 234 | gpu-compute: prefix + beam + sphere + pathtrace (incorrect) | 152.2 | 0.04 | 8.06 | 6.53 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 235 | gpu-compute: rows + sphere + pathtrace (incorrect) | 151.6 | 0.04 | 8.21 | 6.56 | 0.03 | 30.5 | 8.58 | 52 | 216 |
| 236 | gpu-compute: prefix + sphere + pathtrace (incorrect) | 151.5 | 0.04 | 8.25 | 6.56 | 0.03 | 30.5 | 8.58 | 52 | 215 |
| 237 | gpu-compute: rows + beam + sphere + pathtrace (incorrect) | 151.2 | 0.04 | 8.22 | 6.55 | 0.03 | 30.5 | 8.58 | 52 | 216 |
| 238 | gpu-compute: svo_dag + sphere + pathtrace (incorrect) | 150.5 | 0.04 | 11.47 | 6.58 | 0.02 | 30.5 | 8.58 | 48 | 216 |
| 239 | gpu-compute: bricks + beam + sphere + pathtrace (incorrect) | 150.2 | 0.04 | 8.43 | 6.59 | 0.02 | 30.5 | 8.58 | 52 | 215 |
| 240 | gpu-compute: svo_dag + beam + sphere + pathtrace (incorrect) | 149.1 | 0.04 | 11.29 | 6.61 | 0.03 | 30.5 | 8.58 | 48 | 216 |
| 241 | gpu-compute: tree64 + beam + sphere + pathtrace (incorrect) | 145.4 | 0.04 | 11.00 | 6.82 | 0.03 | 30.5 | 8.58 | 48 | 216 |
| 242 | gpu-compute: tree64 + sphere + pathtrace (incorrect) | 138.4 | 0.04 | 11.72 | 7.13 | 0.03 | 30.5 | 8.58 | 48 | 216 |
| 243 | gpu-compute: clipmap + sphere + pathtrace (incorrect) | 137.2 | 0.04 | 11.52 | 7.24 | 0.03 | 30.5 | 8.58 | 71 | 216 |
| 244 | gpu-compute: distance + beam + incremental + pathtrace (incorrect) | 135.8 | 0.04 | 9.15 | 7.32 | 0.03 | 30.5 | 8.58 | 54 | 216 |
| 245 | gpu-compute: clipmap + beam + sphere + pathtrace (incorrect) | 132.3 | 0.04 | 12.25 | 7.51 | 0.03 | 30.5 | 8.58 | 71 | 216 |
| 246 | gpu-compute: svo_dag + incremental + pathtrace (incorrect) | 130.6 | 0.04 | 12.45 | 7.60 | 0.04 | 30.5 | 8.58 | 48 | 216 |
| 247 | gpu-compute: distance + incremental + pathtrace (incorrect) | 129.2 | 0.04 | 9.43 | 7.67 | 0.02 | 30.5 | 8.58 | 54 | 215 |
| 248 | gpu-compute: prefix + beam + incremental + pathtrace (incorrect) | 127.6 | 0.04 | 9.46 | 7.80 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 249 | gpu-compute: hier + beam + incremental + pathtrace (incorrect) | 125.8 | 0.04 | 9.73 | 7.91 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 250 | gpu-compute: svo_dag + beam + incremental + pathtrace (incorrect) | 125.1 | 0.04 | 12.41 | 7.94 | 0.03 | 30.5 | 8.58 | 48 | 216 |
| 251 | gpu-compute: prefix + incremental + pathtrace (incorrect) | 119.5 | 0.03 | 10.08 | 8.33 | 0.04 | 30.5 | 8.58 | 52 | 216 |
| 252 | gpu-compute: hier + incremental + pathtrace (incorrect) | 117.6 | 0.03 | 9.94 | 8.47 | 0.02 | 30.5 | 8.58 | 52 | 216 |
| 253 | gpu-compute: rows + beam + incremental + pathtrace (incorrect) | 113.8 | 0.03 | 10.53 | 8.72 | 0.04 | 30.5 | 8.58 | 52 | 216 |
| 254 | gpu-compute: bricks + beam + incremental + pathtrace (incorrect) | 113.0 | 0.03 | 10.51 | 8.78 | 0.03 | 30.5 | 8.58 | 52 | 216 |
| 255 | gpu-compute: tree64 + beam + rowrun + pathtrace (incorrect) | 111.7 | 0.03 | 13.98 | 8.86 | 0.03 | 30.5 | 8.58 | 48 | 216 |
| 256 | gpu-compute: rows + incremental + pathtrace (incorrect) | 107.4 | 0.03 | 11.15 | 9.25 | 0.03 | 30.5 | 8.58 | 52 | 216 |
| 257 | gpu-compute: bricks + incremental + pathtrace (incorrect) | 106.1 | 0.03 | 11.11 | 9.38 | 0.03 | 30.5 | 8.58 | 52 | 215 |
| 258 | gpu-compute: tree64 + rowrun + pathtrace (incorrect) | 103.4 | 0.03 | 14.38 | 9.62 | 0.02 | 30.5 | 8.58 | 48 | 217 |

### room [unlit]

1920×1080. camera 0.00 blocks/frame (max 0.0), turn 0.00°/frame; 27 chunks at start, 0.0 loads/frame; chunk fill 4%, solid 0.1%, surface 0.06%; sky 47%, hits at 51 blocks (median 43); 0.0 edits/frame; 2 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-raster: bricks + greedy + raster | 5719.9 | 1.00 | 3.11 | 0.05 | 0.01 | 64.4 | 0.00 | 30 | 217 |
| 2 | gpu-raster: bricks + naive + raster | 5387.0 | 0.94 | 2.98 | 0.07 | 0.01 | 60.5 | 0.00 | 30 | 217 |
| 3 | gpu-raster: bricks + greedy + visbuffer | 4975.4 | 0.87 | 3.13 | 0.09 | 0.01 | 64.4 | 0.00 | 30 | 216 |
| 4 | gpu-raster: bricks + naive + visbuffer | 4656.5 | 0.81 | 3.16 | 0.10 | 0.02 | 60.5 | 0.00 | 30 | 217 |
| 5 | gpu-raster: bricks + greedy + gbuffer | 4052.4 | 0.71 | 1.97 | 0.14 | 0.01 | 64.4 | 0.00 | 30 | 217 |
| 6 | gpu-raster: bricks + cubes + visbuffer | 3774.3 | 0.66 | 2.08 | 0.15 | 0.01 | 60.5 | 0.00 | 30 | 217 |
| 7 | gpu-raster: bricks + naive + gbuffer | 3519.2 | 0.62 | 2.07 | 0.18 | 0.02 | 60.5 | 0.00 | 30 | 217 |
| 8 | gpu-raster: bricks + cubes + raster | 3505.8 | 0.61 | 1.98 | 0.17 | 0.01 | 60.5 | 0.00 | 30 | 217 |
| 9 | gpu-compute: distance + sphere | 2615.6 | 0.46 | 2.25 | 0.28 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 10 | gpu-raster: bricks + cubes + gbuffer | 2576.6 | 0.45 | 1.96 | 0.29 | 0.02 | 60.5 | 0.00 | 30 | 217 |
| 11 | gpu-compute: distance + beam + sphere | 2571.0 | 0.45 | 1.92 | 0.30 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 12 | gpu-rt: naive + rayquery | 2331.7 | 0.41 | 2.25 | 0.33 | 0.06 | 79.0 | 0.00 | 4 | 352 |
| 13 | gpu-rt: greedy + rayquery | 2259.7 | 0.40 | 2.23 | 0.34 | 0.06 | 79.0 | 0.00 | 2 | 350 |
| 14 | gpu-compute: distance + refloor | 1642.3 | 0.29 | 4.25 | 0.48 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 15 | gpu-discover: distance + quadtree | 1630.2 | 0.29 | 4.49 | 0.45 | 0.02 | 60.5 | 0.00 | 204 | 226 |
| 16 | gpu-compute: distance + beam + refloor | 1624.7 | 0.28 | 4.30 | 0.49 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 17 | gpu-discover: hier + quadtree | 1611.0 | 0.28 | 4.29 | 0.47 | 0.02 | 60.5 | 0.00 | 202 | 224 |
| 18 | gpu-discover: prefix + quadtree | 1599.4 | 0.28 | 4.54 | 0.46 | 0.03 | 60.5 | 0.00 | 202 | 224 |
| 19 | gpu-compute: distance + rowrun | 1596.5 | 0.28 | 4.59 | 0.49 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 20 | gpu-compute: distance + beam + rowrun | 1583.4 | 0.28 | 4.30 | 0.51 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 21 | gpu-discover: bricks + quadtree | 1575.9 | 0.28 | 4.50 | 0.48 | 0.02 | 60.5 | 0.00 | 202 | 223 |
| 22 | gpu-discover: rows + quadtree | 1558.9 | 0.27 | 4.26 | 0.48 | 0.03 | 60.5 | 0.00 | 202 | 226 |
| 23 | gpu-discover: svo_dag + quadtree | 1496.2 | 0.26 | 4.36 | 0.52 | 0.03 | 60.5 | 0.00 | 198 | 225 |
| 24 | gpu-discover: tree64 + quadtree | 1470.6 | 0.26 | 4.50 | 0.52 | 0.03 | 60.5 | 0.00 | 198 | 228 |
| 25 | gpu-compute: hier + refloor | 1437.6 | 0.25 | 4.88 | 0.56 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 26 | gpu-compute: svo_dag + refloor | 1414.0 | 0.25 | 4.84 | 0.57 | 0.01 | 78.9 | 0.00 | 16 | 216 |
| 27 | gpu-compute: hier + beam + refloor | 1411.3 | 0.25 | 5.04 | 0.57 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 28 | gpu-compute: prefix + refloor | 1409.3 | 0.25 | 4.27 | 0.59 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 29 | gpu-compute: svo_dag + beam + refloor | 1384.4 | 0.24 | 4.34 | 0.59 | 0.01 | 78.9 | 0.00 | 16 | 216 |
| 30 | gpu-compute: hier + rowrun | 1383.1 | 0.24 | 4.37 | 0.60 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 31 | gpu-compute: prefix + beam + refloor | 1380.3 | 0.24 | 4.20 | 0.60 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 32 | gpu-discover: clipmap + quadtree | 1374.0 | 0.24 | 4.42 | 0.56 | 0.02 | 60.5 | 0.00 | 221 | 223 |
| 33 | gpu-compute: svo_dag + rowrun | 1351.1 | 0.24 | 4.72 | 0.60 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 34 | gpu-compute: prefix + beam + rowrun | 1349.8 | 0.24 | 4.52 | 0.61 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 35 | gpu-compute: prefix + rowrun | 1337.4 | 0.23 | 4.47 | 0.62 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 36 | gpu-compute: hier + beam + rowrun | 1329.5 | 0.23 | 4.76 | 0.62 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 37 | gpu-compute: svo_dag + beam + rowrun | 1327.2 | 0.23 | 4.31 | 0.61 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 38 | gpu-rt: boxes + rayquery | 1319.8 | 0.23 | 4.11 | 0.63 | 0.06 | 78.9 | 0.00 | 6 | 350 |
| 39 | gpu-compute: bricks + refloor | 1194.6 | 0.21 | 4.54 | 0.71 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 40 | gpu-compute: rows + refloor | 1177.7 | 0.21 | 4.43 | 0.72 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 41 | gpu-compute: rows + beam + refloor | 1160.6 | 0.20 | 4.83 | 0.73 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 42 | gpu-compute: rows + rowrun | 1143.2 | 0.20 | 4.92 | 0.74 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 43 | gpu-compute: rows + beam + rowrun | 1142.7 | 0.20 | 4.53 | 0.75 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 44 | gpu-compute: bricks + beam + rowrun | 1134.5 | 0.20 | 4.33 | 0.77 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 45 | gpu-compute: tree64 + refloor | 1066.1 | 0.19 | 4.87 | 0.81 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 46 | gpu-compute: tree64 + beam + rowrun | 1056.6 | 0.18 | 4.49 | 0.82 | 0.01 | 78.9 | 0.00 | 16 | 216 |
| 47 | gpu-compute: tree64 + rowrun | 1054.7 | 0.18 | 4.64 | 0.82 | 0.01 | 78.9 | 0.00 | 16 | 216 |
| 48 | gpu-compute: tree64 + beam + refloor | 1052.0 | 0.18 | 4.95 | 0.82 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 49 | gpu-compute: clipmap + refloor | 1020.9 | 0.18 | 4.80 | 0.84 | 0.01 | 78.9 | 0.00 | 39 | 215 |
| 50 | gpu-compute: clipmap + rowrun | 988.2 | 0.17 | 4.84 | 0.89 | 0.01 | 78.9 | 0.00 | 39 | 216 |
| 51 | gpu-compute: clipmap + beam + rowrun | 975.0 | 0.17 | 4.85 | 0.90 | 0.01 | 78.9 | 0.00 | 39 | 216 |
| 52 | gpu-compute: clipmap + beam + refloor | 970.8 | 0.17 | 5.67 | 0.85 | 0.01 | 78.9 | 0.00 | 39 | 215 |
| 53 | gpu-compute: clipmap + incremental | 896.0 | 0.16 | 4.80 | 0.99 | 0.01 | 78.9 | 0.00 | 39 | 216 |
| 54 | gpu-compute: clipmap + beam + incremental | 889.1 | 0.16 | 4.94 | 1.00 | 0.01 | 78.9 | 0.00 | 39 | 216 |
| 55 | gpu-compute: tree64 + incremental | 834.8 | 0.15 | 5.45 | 1.07 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 56 | gpu-compute: tree64 + beam + incremental | 828.8 | 0.14 | 5.33 | 1.08 | 0.01 | 78.9 | 0.00 | 16 | 216 |
| 57 | gpu-compute: distance + incremental | 813.1 | 0.14 | 4.92 | 1.11 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 58 | gpu-compute: hier + incremental | 779.7 | 0.14 | 5.58 | 1.15 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 59 | gpu-compute: distance + beam + incremental | 767.5 | 0.13 | 5.06 | 1.19 | 0.01 | 78.9 | 0.00 | 22 | 216 |
| 60 | gpu-compute: hier + beam + incremental | 754.8 | 0.13 | 5.69 | 1.20 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 61 | gpu-compute: bricks + incremental | 728.7 | 0.13 | 5.51 | 1.26 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 62 | gpu-compute: prefix + incremental | 716.5 | 0.13 | 5.19 | 1.28 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 63 | gpu-compute: rows + incremental | 704.4 | 0.12 | 6.03 | 1.29 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 64 | gpu-compute: bricks + beam + refloor | 697.4 | 0.12 | 5.92 | 1.29 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 65 | gpu-compute: prefix + beam + incremental | 696.0 | 0.12 | 5.34 | 1.32 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 66 | gpu-compute: rows + beam + incremental | 666.4 | 0.12 | 5.43 | 1.38 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 67 | gpu-compute: bricks + rowrun | 644.8 | 0.11 | 6.27 | 1.41 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 68 | gpu-compute: svo_dag + beam + incremental | 497.8 | 0.09 | 6.19 | 1.89 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 69 | gpu-compute: svo_dag + incremental | 495.6 | 0.09 | 6.10 | 1.91 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 70 | gpu-compute: hier + sphere | 479.0 | 0.08 | 6.40 | 1.98 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 71 | gpu-compute: hier + beam + sphere | 470.2 | 0.08 | 6.43 | 2.02 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 72 | gpu-compute: svo_dag + sphere | 427.7 | 0.07 | 6.21 | 2.24 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 73 | gpu-compute: svo_dag + beam + sphere | 416.7 | 0.07 | 6.65 | 2.28 | 0.01 | 78.9 | 0.00 | 16 | 217 |
| 74 | gpu-compute: bricks + beam + incremental | 416.4 | 0.07 | 6.73 | 2.25 | 0.02 | 78.9 | 0.00 | 20 | 216 |
| 75 | gpu-compute: rows + sphere | 407.9 | 0.07 | 6.47 | 2.35 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 76 | gpu-compute: rows + beam + sphere | 402.9 | 0.07 | 6.15 | 2.39 | 0.02 | 78.9 | 0.00 | 20 | 216 |
| 77 | gpu-compute: bricks + beam + sphere | 395.9 | 0.07 | 6.11 | 2.42 | 0.01 | 78.9 | 0.00 | 20 | 216 |
| 78 | gpu-compute: prefix + sphere | 395.0 | 0.07 | 6.86 | 2.42 | 0.01 | 78.9 | 0.00 | 20 | 215 |
| 79 | gpu-compute: prefix + beam + sphere | 393.8 | 0.07 | 6.71 | 2.43 | 0.02 | 78.9 | 0.00 | 20 | 216 |
| 80 | gpu-compute: tree64 + sphere | 376.1 | 0.07 | 6.91 | 2.55 | 0.01 | 78.9 | 0.00 | 16 | 216 |
| 81 | gpu-compute: tree64 + beam + sphere | 374.7 | 0.07 | 6.60 | 2.56 | 0.02 | 78.9 | 0.00 | 16 | 217 |
| 82 | gpu-compute: clipmap + sphere | 370.4 | 0.06 | 6.64 | 2.60 | 0.01 | 78.9 | 0.00 | 39 | 216 |
| 83 | gpu-compute: clipmap + beam + sphere | 354.5 | 0.06 | 6.90 | 2.69 | 0.01 | 78.9 | 0.00 | 39 | 216 |
| 84 | gpu-compute: bricks + sphere | 328.6 | 0.06 | 6.49 | 2.94 | 0.02 | 78.9 | 0.00 | 20 | 215 |
| 85 | cpu-raster: greedy + raster | 273.9 | 0.05 | 7.62 | - | 3.52 | 99.0 | 0.00 | - | 50 |
| 86 | cpu-raster: naive + raster | 215.6 | 0.04 | 8.85 | - | 4.48 | 99.0 | 0.00 | - | 67 |
| 87 | cpu-discover: hier + quadtree | 211.3 | 0.04 | 9.81 | 0.06 | 4.50 | 60.5 | 0.00 | 24 | 333 |
| 88 | cpu-discover: prefix + quadtree | 211.2 | 0.04 | 10.24 | 0.06 | 4.49 | 60.5 | 0.00 | 24 | 325 |
| 89 | cpu-discover: u8 + quadtree | 203.9 | 0.04 | 10.05 | 0.05 | 4.66 | 60.5 | 0.00 | 24 | 325 |
| 90 | cpu-discover: bits + quadtree | 202.0 | 0.04 | 10.48 | 0.06 | 4.70 | 60.5 | 0.00 | 24 | 326 |
| 91 | cpu-discover: morton + quadtree | 201.9 | 0.04 | 10.16 | 0.05 | 4.70 | 60.5 | 0.00 | 24 | 328 |
| 92 | cpu-discover: brickmajor + quadtree | 201.2 | 0.04 | 10.43 | 0.05 | 4.72 | 60.5 | 0.00 | 24 | 330 |
| 93 | cpu-discover: palette + quadtree | 200.5 | 0.04 | 10.39 | 0.05 | 4.74 | 60.5 | 0.00 | 24 | 327 |
| 94 | cpu-discover: bricks + quadtree | 198.4 | 0.03 | 10.59 | 0.05 | 4.79 | 60.5 | 0.00 | 24 | 334 |
| 95 | cpu-discover: hier + cluster | 146.6 | 0.03 | 14.54 | 0.05 | 6.59 | 60.5 | 0.00 | 24 | 246 |
| 96 | cpu-discover: prefix + cluster | 146.3 | 0.03 | 14.61 | 0.05 | 6.59 | 60.5 | 0.00 | 24 | 249 |
| 97 | cpu-discover: brickmajor + cluster | 139.8 | 0.02 | 14.74 | 0.05 | 6.91 | 60.5 | 0.00 | 24 | 248 |
| 98 | cpu-discover: bits + cluster | 139.7 | 0.02 | 15.55 | 0.05 | 6.90 | 60.5 | 0.00 | 24 | 243 |
| 99 | cpu-discover: bricks + cluster | 139.0 | 0.02 | 15.34 | 0.05 | 6.95 | 60.5 | 0.00 | 24 | 245 |
| 100 | cpu-discover: morton + cluster | 139.0 | 0.02 | 15.62 | 0.05 | 6.96 | 60.5 | 0.00 | 24 | 248 |
| 101 | cpu-discover: u8 + cluster | 138.6 | 0.02 | 15.01 | 0.05 | 6.98 | 60.5 | 0.00 | 24 | 246 |
| 102 | cpu-discover: palette + cluster | 137.7 | 0.02 | 15.31 | 0.05 | 7.02 | 60.5 | 0.00 | 24 | 247 |
| 103 | cpu: hier + refloor | 62.1 | 0.01 | 20.44 | - | 16.06 | 85.6 | 0.00 | - | 33 |
| 104 | cpu: prefix + refloor | 49.6 | 0.01 | 23.45 | - | 20.10 | 85.6 | 0.00 | - | 31 |
| 105 | cpu: bricks + refloor | 46.8 | 0.01 | 25.68 | - | 21.33 | 85.6 | 0.00 | - | 32 |
| 106 | cpu: hier + incremental | 46.2 | 0.01 | 25.24 | - | 21.61 | 85.6 | 0.00 | - | 32 |
| 107 | cpu: morton + refloor | 45.1 | 0.01 | 26.80 | - | 22.10 | 85.6 | 0.00 | - | 33 |
| 108 | cpu: brickmajor + refloor | 45.0 | 0.01 | 26.22 | - | 22.20 | 85.6 | 0.00 | - | 33 |
| 109 | cpu: bits + refloor | 44.9 | 0.01 | 26.89 | - | 22.21 | 85.6 | 0.00 | - | 33 |
| 110 | cpu: u8 + refloor | 44.8 | 0.01 | 26.17 | - | 22.29 | 85.6 | 0.00 | - | 32 |
| 111 | cpu: palette + refloor | 44.1 | 0.01 | 28.03 | - | 22.62 | 85.6 | 0.00 | - | 31 |
| 112 | cpu: prefix + incremental | 40.5 | 0.01 | 28.32 | - | 24.63 | 85.6 | 0.00 | - | 31 |
| 113 | cpu: bricks + incremental | 40.5 | 0.01 | 29.24 | - | 24.64 | 85.6 | 0.00 | - | 33 |
| 114 | cpu: bits + incremental | 40.2 | 0.01 | 29.40 | - | 24.85 | 85.6 | 0.00 | - | 33 |
| 115 | cpu: brickmajor + incremental | 39.9 | 0.01 | 29.28 | - | 24.99 | 85.6 | 0.00 | - | 32 |
| 116 | cpu: u8 + incremental | 39.7 | 0.01 | 29.54 | - | 25.12 | 85.6 | 0.00 | - | 32 |
| 117 | cpu: palette + incremental | 39.4 | 0.01 | 29.73 | - | 25.32 | 85.6 | 0.00 | - | 31 |
| 118 | cpu: morton + incremental | 37.9 | 0.01 | 36.16 | - | 26.31 | 85.6 | 0.00 | - | 33 |
| 119 | gpu-splat: bricks + points + splat (incorrect) | 3495.0 | 0.61 | 1.88 | 0.19 | 0.01 | 13.5 | 26.42 | 28 | 215 |
| 120 | gpu-voxelspace: heightmap + columns (incorrect) | 946.2 | 0.17 | 4.79 | 0.94 | 0.01 | 14.5 | 15.31 | 9 | 214 |
| 121 | cpu-voxelspace: heightmap + columns (incorrect) | 568.1 | 0.10 | 5.81 | - | 1.64 | 14.5 | 15.31 | - | 41 |

### showcase [lit]

960×540. camera 1.36 blocks/frame (max 1.4), turn 2.72°/frame; 84 chunks at start, 0.5 loads/frame; chunk fill 12%, solid 0.3%, surface 0.35%; sky 28%, hits at 38 blocks (median 29); 0.0 edits/frame; 2 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-raster: svo_dag + naive + visbuffer | 4255.6 | 1.00 | 1.20 | 0.15 | 0.06 | 44.0 | 0.03 | 9 | 214 |
| 2 | gpu-raster: prefix + naive + visbuffer | 4212.0 | 0.99 | 0.97 | 0.16 | 0.05 | 44.0 | 0.03 | 18 | 215 |
| 3 | gpu-raster: prefix + naive + gbuffer | 3894.7 | 0.92 | 1.14 | 0.18 | 0.05 | 44.0 | 0.03 | 18 | 215 |
| 4 | gpu-compute: hier + refloor | 3833.7 | 0.90 | 0.55 | 0.22 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 5 | gpu-raster: svo_dag + naive + gbuffer | 3828.1 | 0.90 | 1.27 | 0.17 | 0.07 | 44.0 | 0.03 | 9 | 213 |
| 6 | gpu-raster: hier + naive + visbuffer | 3802.5 | 0.89 | 1.24 | 0.17 | 0.05 | 44.0 | 0.03 | 17 | 215 |
| 7 | gpu-compute: prefix + refloor | 3799.2 | 0.89 | 0.63 | 0.23 | 0.01 | 58.3 | 0.00 | 13 | 212 |
| 8 | gpu-raster: hier + naive + gbuffer | 3710.3 | 0.87 | 1.10 | 0.18 | 0.05 | 44.0 | 0.03 | 17 | 215 |
| 9 | gpu-raster: rows + naive + visbuffer | 3706.7 | 0.87 | 1.03 | 0.19 | 0.05 | 44.0 | 0.03 | 18 | 215 |
| 10 | gpu-compute: svo_dag + refloor | 3678.8 | 0.86 | 0.79 | 0.22 | 0.03 | 58.3 | 0.00 | 4 | 213 |
| 11 | gpu-compute: prefix + beam + refloor | 3675.6 | 0.86 | 0.58 | 0.24 | 0.01 | 58.3 | 0.00 | 13 | 212 |
| 12 | gpu-raster: prefix + greedy + visbuffer | 3659.4 | 0.86 | 1.76 | 0.16 | 0.08 | 44.0 | 0.03 | 18 | 215 |
| 13 | gpu-raster: tree64 + naive + visbuffer | 3646.8 | 0.86 | 1.45 | 0.18 | 0.07 | 44.0 | 0.03 | 10 | 215 |
| 14 | gpu-raster: bricks + naive + visbuffer | 3639.6 | 0.86 | 1.33 | 0.19 | 0.04 | 44.0 | 0.03 | 17 | 215 |
| 15 | gpu-raster: bricks + naive + gbuffer | 3579.3 | 0.84 | 0.99 | 0.20 | 0.05 | 44.0 | 0.03 | 17 | 215 |
| 16 | gpu-raster: svo_dag + greedy + visbuffer | 3574.7 | 0.84 | 1.95 | 0.15 | 0.10 | 44.0 | 0.03 | 9 | 214 |
| 17 | gpu-compute: hier + beam + refloor | 3560.1 | 0.84 | 0.68 | 0.24 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 18 | gpu-raster: clipmap + naive + visbuffer | 3504.7 | 0.82 | 0.93 | 0.21 | 0.05 | 44.0 | 0.03 | 32 | 217 |
| 19 | gpu-compute: prefix + beam + rowrun | 3464.1 | 0.81 | 0.56 | 0.25 | 0.01 | 58.3 | 0.00 | 13 | 212 |
| 20 | gpu-raster: hier + greedy + visbuffer | 3457.4 | 0.81 | 1.62 | 0.17 | 0.08 | 44.0 | 0.03 | 17 | 215 |
| 21 | gpu-raster: prefix + greedy + gbuffer | 3446.7 | 0.81 | 1.83 | 0.17 | 0.08 | 44.0 | 0.03 | 18 | 214 |
| 22 | gpu-raster: rows + naive + gbuffer | 3431.2 | 0.81 | 1.08 | 0.21 | 0.05 | 44.0 | 0.03 | 18 | 215 |
| 23 | gpu-compute: svo_dag + beam + refloor | 3430.8 | 0.81 | 0.87 | 0.23 | 0.04 | 58.3 | 0.00 | 4 | 213 |
| 24 | gpu-raster: clipmap + naive + gbuffer | 3334.9 | 0.78 | 0.85 | 0.23 | 0.04 | 44.0 | 0.03 | 32 | 217 |
| 25 | gpu-compute: prefix + rowrun | 3323.3 | 0.78 | 0.75 | 0.25 | 0.01 | 58.3 | 0.00 | 13 | 212 |
| 26 | gpu-raster: svo_dag + greedy + gbuffer | 3314.7 | 0.78 | 2.19 | 0.17 | 0.11 | 44.0 | 0.03 | 9 | 214 |
| 27 | gpu-raster: prefix + cubes + visbuffer | 3300.1 | 0.78 | 0.86 | 0.23 | 0.03 | 44.0 | 0.03 | 17 | 214 |
| 28 | gpu-raster: svo_dag + cubes + visbuffer | 3284.4 | 0.77 | 1.16 | 0.23 | 0.06 | 44.0 | 0.03 | 8 | 213 |
| 29 | gpu-raster: tree64 + naive + gbuffer | 3279.4 | 0.77 | 1.75 | 0.20 | 0.09 | 44.0 | 0.03 | 10 | 214 |
| 30 | gpu-raster: hier + cubes + visbuffer | 3277.4 | 0.77 | 0.69 | 0.25 | 0.03 | 44.0 | 0.03 | 16 | 214 |
| 31 | gpu-raster: bricks + greedy + visbuffer | 3265.1 | 0.77 | 1.80 | 0.19 | 0.08 | 44.0 | 0.03 | 17 | 215 |
| 32 | gpu-raster: hier + greedy + gbuffer | 3263.9 | 0.77 | 1.70 | 0.18 | 0.08 | 44.0 | 0.03 | 17 | 214 |
| 33 | gpu-raster: prefix + naive + raster | 3263.1 | 0.77 | 1.09 | 0.23 | 0.04 | 44.0 | 0.03 | 18 | 214 |
| 34 | gpu-raster: prefix + cubes + gbuffer | 3262.0 | 0.77 | 0.79 | 0.25 | 0.03 | 44.0 | 0.03 | 17 | 214 |
| 35 | gpu-raster: hier + naive + raster | 3226.2 | 0.76 | 1.09 | 0.23 | 0.04 | 44.0 | 0.03 | 17 | 214 |
| 36 | gpu-raster: svo_dag + naive + raster | 3219.1 | 0.76 | 1.43 | 0.22 | 0.07 | 44.0 | 0.03 | 9 | 214 |
| 37 | gpu-compute: bricks + refloor | 3197.6 | 0.75 | 0.66 | 0.28 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 38 | gpu-raster: rows + greedy + visbuffer | 3182.0 | 0.75 | 1.83 | 0.19 | 0.08 | 44.0 | 0.03 | 18 | 214 |
| 39 | gpu-raster: tree64 + greedy + visbuffer | 3177.6 | 0.75 | 2.32 | 0.18 | 0.11 | 44.0 | 0.03 | 10 | 214 |
| 40 | gpu-compute: rows + refloor | 3166.4 | 0.74 | 0.59 | 0.28 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 41 | gpu-raster: bricks + greedy + gbuffer | 3161.2 | 0.74 | 1.96 | 0.21 | 0.08 | 44.0 | 0.03 | 17 | 215 |
| 42 | gpu-compute: bricks + beam + refloor | 3113.9 | 0.73 | 0.59 | 0.29 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 43 | gpu-raster: bricks + cubes + visbuffer | 3111.1 | 0.73 | 0.82 | 0.27 | 0.03 | 44.0 | 0.03 | 16 | 214 |
| 44 | gpu-raster: rows + greedy + gbuffer | 3084.4 | 0.72 | 1.93 | 0.21 | 0.08 | 44.0 | 0.03 | 18 | 215 |
| 45 | gpu-compute: rows + beam + refloor | 3081.3 | 0.72 | 0.65 | 0.29 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 46 | gpu-raster: rows + cubes + visbuffer | 3065.2 | 0.72 | 0.83 | 0.27 | 0.04 | 44.0 | 0.03 | 17 | 214 |
| 47 | gpu-raster: hier + cubes + gbuffer | 3041.0 | 0.71 | 0.84 | 0.26 | 0.03 | 44.0 | 0.03 | 16 | 214 |
| 48 | gpu-raster: svo_dag + cubes + gbuffer | 3027.6 | 0.71 | 1.42 | 0.25 | 0.06 | 44.0 | 0.03 | 8 | 213 |
| 49 | gpu-raster: tree64 + greedy + gbuffer | 3026.3 | 0.71 | 2.41 | 0.20 | 0.11 | 44.0 | 0.03 | 10 | 214 |
| 50 | gpu-raster: bricks + naive + raster | 3017.3 | 0.71 | 0.83 | 0.26 | 0.05 | 44.0 | 0.03 | 17 | 214 |
| 51 | gpu-raster: bricks + cubes + gbuffer | 3006.2 | 0.71 | 0.91 | 0.28 | 0.03 | 44.0 | 0.03 | 16 | 213 |
| 52 | gpu-raster: clipmap + greedy + visbuffer | 3005.3 | 0.71 | 1.90 | 0.21 | 0.09 | 44.0 | 0.03 | 32 | 216 |
| 53 | gpu-raster: clipmap + cubes + visbuffer | 2989.5 | 0.70 | 0.62 | 0.29 | 0.03 | 44.0 | 0.03 | 31 | 216 |
| 54 | gpu-raster: prefix + greedy + raster | 2966.9 | 0.70 | 1.79 | 0.23 | 0.08 | 44.0 | 0.03 | 18 | 215 |
| 55 | gpu-raster: rows + cubes + gbuffer | 2964.9 | 0.70 | 0.82 | 0.28 | 0.03 | 44.0 | 0.03 | 17 | 214 |
| 56 | gpu-compute: tree64 + refloor | 2948.5 | 0.69 | 1.12 | 0.28 | 0.04 | 58.3 | 0.00 | 4 | 213 |
| 57 | gpu-compute: rows + rowrun | 2931.3 | 0.69 | 0.93 | 0.30 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 58 | gpu-raster: svo_dag + greedy + raster | 2907.3 | 0.68 | 2.08 | 0.22 | 0.10 | 44.0 | 0.03 | 9 | 214 |
| 59 | gpu-raster: tree64 + naive + raster | 2898.9 | 0.68 | 1.55 | 0.25 | 0.07 | 44.0 | 0.03 | 10 | 215 |
| 60 | gpu-raster: tree64 + cubes + visbuffer | 2894.0 | 0.68 | 1.40 | 0.26 | 0.07 | 44.0 | 0.03 | 9 | 213 |
| 61 | gpu-raster: clipmap + greedy + gbuffer | 2885.9 | 0.68 | 1.81 | 0.23 | 0.09 | 44.0 | 0.03 | 32 | 217 |
| 62 | gpu-raster: hier + greedy + raster | 2872.2 | 0.67 | 1.83 | 0.23 | 0.07 | 44.0 | 0.03 | 17 | 214 |
| 63 | gpu-compute: rows + beam + rowrun | 2844.5 | 0.67 | 0.87 | 0.32 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 64 | gpu-raster: rows + naive + raster | 2834.9 | 0.67 | 1.20 | 0.27 | 0.05 | 44.0 | 0.03 | 18 | 215 |
| 65 | gpu-rt: naive + rayquery | 2784.3 | 0.65 | 1.86 | 0.24 | 0.13 | 58.0 | 0.00 | 15 | 364 |
| 66 | gpu-raster: clipmap + cubes + gbuffer | 2757.4 | 0.65 | 0.86 | 0.31 | 0.03 | 44.0 | 0.03 | 31 | 215 |
| 67 | gpu-raster: bricks + greedy + raster | 2741.2 | 0.64 | 1.89 | 0.26 | 0.08 | 44.0 | 0.03 | 17 | 214 |
| 68 | gpu-compute: tree64 + beam + refloor | 2727.2 | 0.64 | 1.28 | 0.30 | 0.06 | 58.3 | 0.00 | 4 | 213 |
| 69 | gpu-raster: clipmap + naive + raster | 2720.9 | 0.64 | 0.96 | 0.31 | 0.04 | 44.0 | 0.03 | 32 | 217 |
| 70 | gpu-raster: tree64 + cubes + gbuffer | 2705.9 | 0.64 | 1.89 | 0.28 | 0.08 | 44.0 | 0.03 | 9 | 214 |
| 71 | gpu-raster: tree64 + greedy + raster | 2680.9 | 0.63 | 2.17 | 0.25 | 0.10 | 44.0 | 0.03 | 10 | 214 |
| 72 | gpu-raster: rows + greedy + raster | 2645.2 | 0.62 | 2.10 | 0.26 | 0.08 | 44.0 | 0.03 | 18 | 214 |
| 73 | gpu-compute: hier + beam + rowrun | 2615.5 | 0.61 | 0.73 | 0.35 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 74 | gpu-compute: hier + rowrun | 2613.1 | 0.61 | 0.78 | 0.33 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 75 | gpu-compute: svo_dag + rowrun | 2592.3 | 0.61 | 0.86 | 0.34 | 0.04 | 58.3 | 0.00 | 4 | 212 |
| 76 | gpu-compute: clipmap + refloor | 2533.8 | 0.60 | 0.69 | 0.36 | 0.01 | 58.3 | 0.00 | 27 | 214 |
| 77 | gpu-rt: greedy + rayquery | 2518.3 | 0.59 | 2.79 | 0.24 | 0.17 | 58.0 | 0.00 | 15 | 364 |
| 78 | gpu-raster: prefix + cubes + raster | 2484.9 | 0.58 | 0.88 | 0.35 | 0.03 | 44.0 | 0.03 | 17 | 214 |
| 79 | gpu-compute: bricks + beam + rowrun | 2472.1 | 0.58 | 0.99 | 0.37 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 80 | gpu-compute: clipmap + beam + refloor | 2450.3 | 0.58 | 0.69 | 0.38 | 0.01 | 58.3 | 0.00 | 27 | 214 |
| 81 | gpu-raster: hier + cubes + raster | 2434.7 | 0.57 | 0.94 | 0.36 | 0.03 | 44.0 | 0.03 | 16 | 214 |
| 82 | gpu-compute: svo_dag + beam + rowrun | 2418.0 | 0.57 | 0.85 | 0.35 | 0.04 | 58.3 | 0.00 | 4 | 213 |
| 83 | gpu-raster: clipmap + greedy + raster | 2384.7 | 0.56 | 1.87 | 0.31 | 0.09 | 44.0 | 0.03 | 32 | 217 |
| 84 | gpu-compute: bricks + rowrun | 2371.8 | 0.56 | 0.89 | 0.39 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 85 | gpu-raster: svo_dag + cubes + raster | 2359.4 | 0.55 | 1.21 | 0.35 | 0.06 | 44.0 | 0.03 | 8 | 213 |
| 86 | gpu-rt: naive + rayquery + pathtrace | 2310.6 | 0.54 | 2.02 | 0.32 | 0.14 | 36.2 | 0.88 | 22 | 365 |
| 87 | gpu-compute: hier + incremental | 2302.8 | 0.54 | 0.71 | 0.40 | 0.01 | 58.1 | 0.00 | 12 | 212 |
| 88 | gpu-raster: bricks + cubes + raster | 2285.3 | 0.54 | 0.92 | 0.39 | 0.03 | 44.0 | 0.03 | 16 | 214 |
| 89 | gpu-compute: distance + sphere | 2284.6 | 0.54 | 4.97 | 0.16 | 0.01 | 58.3 | 0.00 | 16 | 213 |
| 90 | gpu-raster: rows + cubes + raster | 2283.2 | 0.54 | 0.93 | 0.39 | 0.03 | 44.0 | 0.03 | 17 | 214 |
| 91 | gpu-compute: distance + beam + sphere | 2274.5 | 0.53 | 4.82 | 0.17 | 0.01 | 58.3 | 0.00 | 16 | 213 |
| 92 | gpu-compute: clipmap + rowrun | 2253.0 | 0.53 | 0.80 | 0.41 | 0.01 | 58.3 | 0.00 | 27 | 214 |
| 93 | gpu-compute: bricks + incremental | 2206.1 | 0.52 | 0.90 | 0.42 | 0.01 | 58.1 | 0.00 | 12 | 212 |
| 94 | gpu-compute: hier + beam + incremental | 2177.5 | 0.51 | 0.86 | 0.42 | 0.01 | 58.1 | 0.00 | 12 | 212 |
| 95 | gpu-compute: clipmap + beam + rowrun | 2161.1 | 0.51 | 1.03 | 0.42 | 0.01 | 58.3 | 0.00 | 27 | 214 |
| 96 | gpu-raster: tree64 + cubes + raster | 2156.6 | 0.51 | 1.49 | 0.38 | 0.07 | 44.0 | 0.03 | 9 | 213 |
| 97 | gpu-raster: clipmap + cubes + raster | 2150.5 | 0.51 | 0.86 | 0.42 | 0.03 | 44.0 | 0.03 | 31 | 215 |
| 98 | gpu-rt: greedy + rayquery + pathtrace | 2126.9 | 0.50 | 2.67 | 0.32 | 0.17 | 36.2 | 0.88 | 22 | 364 |
| 99 | gpu-compute: bricks + beam + incremental | 2122.3 | 0.50 | 0.83 | 0.44 | 0.01 | 58.1 | 0.00 | 12 | 212 |
| 100 | gpu-compute: distance + refloor | 2120.3 | 0.50 | 4.78 | 0.20 | 0.01 | 58.3 | 0.00 | 16 | 213 |
| 101 | gpu-rt: boxes + rayquery | 2103.3 | 0.49 | 1.03 | 0.44 | 0.08 | 58.0 | 0.00 | 10 | 353 |
| 102 | gpu-compute: distance + beam + refloor | 2094.2 | 0.49 | 4.84 | 0.21 | 0.01 | 58.3 | 0.00 | 16 | 213 |
| 103 | gpu-compute: tree64 + incremental | 2093.5 | 0.49 | 1.12 | 0.43 | 0.04 | 58.1 | 0.00 | 4 | 214 |
| 104 | gpu-compute: distance + rowrun | 2083.0 | 0.49 | 4.79 | 0.22 | 0.01 | 58.3 | 0.00 | 16 | 213 |
| 105 | gpu-compute: rows + incremental | 2074.0 | 0.49 | 0.76 | 0.45 | 0.01 | 58.1 | 0.00 | 12 | 212 |
| 106 | gpu-compute: distance + beam + rowrun | 2058.1 | 0.48 | 4.79 | 0.23 | 0.01 | 58.3 | 0.00 | 16 | 213 |
| 107 | gpu-raster: distance + naive + visbuffer | 2053.5 | 0.48 | 5.48 | 0.15 | 0.05 | 44.0 | 0.03 | 21 | 215 |
| 108 | gpu-compute: prefix + incremental | 2052.7 | 0.48 | 0.78 | 0.46 | 0.01 | 58.1 | 0.00 | 13 | 212 |
| 109 | gpu-raster: distance + naive + gbuffer | 2038.9 | 0.48 | 5.18 | 0.17 | 0.05 | 44.0 | 0.03 | 21 | 215 |
| 110 | gpu-compute: tree64 + beam + incremental | 1997.3 | 0.47 | 1.29 | 0.45 | 0.05 | 58.1 | 0.00 | 4 | 214 |
| 111 | gpu-compute: clipmap + incremental | 1980.0 | 0.47 | 0.80 | 0.47 | 0.01 | 58.1 | 0.00 | 27 | 214 |
| 112 | gpu-compute: prefix + beam + incremental | 1968.5 | 0.46 | 0.80 | 0.48 | 0.01 | 58.1 | 0.00 | 13 | 212 |
| 113 | gpu-compute: rows + beam + incremental | 1953.6 | 0.46 | 0.81 | 0.48 | 0.01 | 58.1 | 0.00 | 12 | 212 |
| 114 | gpu-raster: distance + greedy + gbuffer | 1950.0 | 0.46 | 5.05 | 0.17 | 0.08 | 44.0 | 0.03 | 21 | 215 |
| 115 | gpu-raster: distance + greedy + visbuffer | 1930.2 | 0.45 | 5.20 | 0.15 | 0.09 | 44.0 | 0.03 | 21 | 215 |
| 116 | gpu-compute: clipmap + beam + incremental | 1918.2 | 0.45 | 0.82 | 0.49 | 0.01 | 58.1 | 0.00 | 27 | 214 |
| 117 | gpu-raster: distance + cubes + visbuffer | 1899.5 | 0.45 | 5.45 | 0.23 | 0.04 | 44.0 | 0.03 | 20 | 214 |
| 118 | gpu-raster: distance + naive + raster | 1870.6 | 0.44 | 4.97 | 0.21 | 0.05 | 44.0 | 0.03 | 21 | 215 |
| 119 | gpu-raster: distance + cubes + gbuffer | 1848.4 | 0.43 | 5.15 | 0.25 | 0.04 | 44.0 | 0.03 | 20 | 214 |
| 120 | gpu-compute: svo_dag + beam + refloor + pathtrace | 1835.1 | 0.43 | 0.91 | 0.50 | 0.04 | 36.2 | 0.88 | 12 | 212 |
| 121 | gpu-compute: svo_dag + refloor + pathtrace | 1817.4 | 0.43 | 0.90 | 0.52 | 0.04 | 36.2 | 0.88 | 12 | 212 |
| 122 | gpu-compute: prefix + beam + refloor + pathtrace | 1795.3 | 0.42 | 0.84 | 0.53 | 0.01 | 36.2 | 0.88 | 21 | 212 |
| 123 | gpu-raster: distance + greedy + raster | 1765.5 | 0.41 | 5.06 | 0.22 | 0.08 | 44.0 | 0.03 | 21 | 215 |
| 124 | gpu-compute: prefix + refloor + pathtrace | 1729.4 | 0.41 | 0.88 | 0.55 | 0.01 | 36.2 | 0.88 | 21 | 213 |
| 125 | gpu-compute: hier + beam + refloor + pathtrace | 1697.0 | 0.40 | 1.12 | 0.55 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 126 | gpu-compute: hier + refloor + pathtrace | 1667.1 | 0.39 | 0.89 | 0.57 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 127 | gpu-compute: distance + sphere + pathtrace | 1600.7 | 0.38 | 4.80 | 0.38 | 0.01 | 36.2 | 0.88 | 24 | 212 |
| 128 | gpu-compute: tree64 + beam + refloor + pathtrace | 1595.6 | 0.37 | 1.43 | 0.58 | 0.05 | 36.2 | 0.88 | 12 | 214 |
| 129 | gpu-compute: distance + beam + sphere + pathtrace | 1591.9 | 0.37 | 4.82 | 0.39 | 0.01 | 36.2 | 0.88 | 24 | 213 |
| 130 | gpu-raster: distance + cubes + raster | 1588.4 | 0.37 | 5.03 | 0.34 | 0.04 | 44.0 | 0.03 | 20 | 215 |
| 131 | gpu-compute: tree64 + refloor + pathtrace | 1575.8 | 0.37 | 1.25 | 0.59 | 0.05 | 36.2 | 0.88 | 12 | 214 |
| 132 | gpu-compute: prefix + rowrun + pathtrace | 1575.7 | 0.37 | 0.99 | 0.60 | 0.01 | 36.2 | 0.88 | 21 | 212 |
| 133 | gpu-compute: tree64 + beam + rowrun | 1548.4 | 0.36 | 1.43 | 0.60 | 0.05 | 58.3 | 0.00 | 4 | 214 |
| 134 | gpu-compute: tree64 + rowrun | 1529.4 | 0.36 | 1.37 | 0.61 | 0.05 | 58.3 | 0.00 | 4 | 214 |
| 135 | gpu-compute: prefix + beam + rowrun + pathtrace | 1518.2 | 0.36 | 0.96 | 0.63 | 0.01 | 36.2 | 0.88 | 21 | 213 |
| 136 | gpu-compute: hier + sphere | 1449.0 | 0.34 | 1.02 | 0.66 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 137 | gpu-discover: prefix + quadtree | 1431.0 | 0.34 | 1.11 | 0.65 | 0.03 | 44.0 | 0.03 | 120 | 223 |
| 138 | gpu-compute: hier + beam + sphere | 1430.2 | 0.34 | 1.03 | 0.67 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 139 | gpu-compute: bricks + beam + refloor + pathtrace | 1418.6 | 0.33 | 1.10 | 0.67 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 140 | gpu-discover: hier + quadtree | 1399.8 | 0.33 | 1.02 | 0.67 | 0.04 | 44.0 | 0.03 | 120 | 221 |
| 141 | gpu-compute: rows + beam + refloor + pathtrace | 1388.9 | 0.33 | 1.02 | 0.69 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 142 | gpu-compute: svo_dag + incremental | 1387.8 | 0.33 | 1.35 | 0.69 | 0.04 | 58.1 | 0.00 | 4 | 213 |
| 143 | gpu-compute: distance + incremental | 1374.6 | 0.32 | 4.80 | 0.50 | 0.01 | 58.1 | 0.00 | 16 | 213 |
| 144 | gpu-rt: boxes + rayquery + pathtrace | 1374.4 | 0.32 | 1.21 | 0.70 | 0.08 | 36.2 | 0.88 | 18 | 353 |
| 145 | gpu-compute: distance + beam + refloor + pathtrace | 1360.7 | 0.32 | 4.88 | 0.51 | 0.01 | 36.2 | 0.88 | 24 | 213 |
| 146 | gpu-compute: bricks + refloor + pathtrace | 1354.6 | 0.32 | 1.13 | 0.70 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 147 | gpu-compute: svo_dag + beam + incremental | 1346.5 | 0.32 | 1.29 | 0.71 | 0.05 | 58.1 | 0.00 | 4 | 213 |
| 148 | gpu-compute: svo_dag + sphere | 1343.8 | 0.32 | 1.07 | 0.71 | 0.04 | 58.3 | 0.00 | 4 | 213 |
| 149 | gpu-compute: distance + refloor + pathtrace | 1342.0 | 0.32 | 4.88 | 0.52 | 0.01 | 36.2 | 0.88 | 24 | 212 |
| 150 | gpu-compute: rows + refloor + pathtrace | 1340.2 | 0.31 | 1.11 | 0.72 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 151 | gpu-compute: distance + beam + incremental | 1323.9 | 0.31 | 4.84 | 0.53 | 0.01 | 58.1 | 0.00 | 16 | 212 |
| 152 | gpu-discover: bricks + quadtree | 1318.7 | 0.31 | 1.06 | 0.71 | 0.04 | 44.0 | 0.03 | 120 | 221 |
| 153 | gpu-discover: rows + quadtree | 1314.6 | 0.31 | 1.11 | 0.71 | 0.04 | 44.0 | 0.03 | 120 | 223 |
| 154 | gpu-compute: svo_dag + beam + sphere | 1309.8 | 0.31 | 1.33 | 0.73 | 0.04 | 58.3 | 0.00 | 4 | 213 |
| 155 | gpu-discover: svo_dag + quadtree | 1283.3 | 0.30 | 1.36 | 0.72 | 0.07 | 44.0 | 0.03 | 112 | 224 |
| 156 | gpu-compute: prefix + sphere | 1256.1 | 0.30 | 1.14 | 0.77 | 0.01 | 58.3 | 0.00 | 13 | 212 |
| 157 | gpu-compute: prefix + beam + sphere | 1242.0 | 0.29 | 1.25 | 0.77 | 0.01 | 58.3 | 0.00 | 13 | 212 |
| 158 | gpu-discover: tree64 + quadtree | 1229.7 | 0.29 | 1.83 | 0.75 | 0.07 | 44.0 | 0.03 | 112 | 225 |
| 159 | gpu-compute: rows + sphere | 1208.4 | 0.28 | 1.19 | 0.80 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 160 | gpu-compute: distance + beam + rowrun + pathtrace | 1207.2 | 0.28 | 4.92 | 0.62 | 0.01 | 36.2 | 0.88 | 24 | 213 |
| 161 | gpu-compute: rows + beam + sphere | 1201.5 | 0.28 | 1.15 | 0.80 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 162 | gpu-compute: bricks + sphere | 1199.1 | 0.28 | 1.34 | 0.81 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 163 | gpu-compute: distance + rowrun + pathtrace | 1192.4 | 0.28 | 4.94 | 0.62 | 0.01 | 36.2 | 0.88 | 24 | 213 |
| 164 | gpu-compute: bricks + beam + sphere | 1186.5 | 0.28 | 1.19 | 0.81 | 0.01 | 58.3 | 0.00 | 12 | 212 |
| 165 | gpu-compute: rows + beam + rowrun + pathtrace | 1160.2 | 0.27 | 1.24 | 0.83 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 166 | gpu-discover: clipmap + quadtree | 1147.3 | 0.27 | 1.30 | 0.83 | 0.04 | 44.0 | 0.03 | 135 | 223 |
| 167 | gpu-compute: rows + rowrun + pathtrace | 1143.1 | 0.27 | 1.24 | 0.84 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 168 | gpu-discover: distance + quadtree | 1136.2 | 0.27 | 4.82 | 0.63 | 0.04 | 44.0 | 0.03 | 124 | 221 |
| 169 | gpu-compute: clipmap + refloor + pathtrace | 1124.9 | 0.26 | 1.23 | 0.85 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 170 | gpu-compute: clipmap + beam + refloor + pathtrace | 1115.9 | 0.26 | 1.25 | 0.86 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 171 | gpu-compute: svo_dag + rowrun + pathtrace | 1083.4 | 0.25 | 1.35 | 0.90 | 0.04 | 36.2 | 0.88 | 12 | 212 |
| 172 | gpu-compute: svo_dag + beam + rowrun + pathtrace | 1061.0 | 0.25 | 1.46 | 0.91 | 0.04 | 36.2 | 0.88 | 12 | 213 |
| 173 | gpu-compute: clipmap + sphere | 1055.3 | 0.25 | 1.26 | 0.92 | 0.01 | 58.3 | 0.00 | 27 | 214 |
| 174 | gpu-compute: hier + rowrun + pathtrace | 1029.8 | 0.24 | 1.41 | 0.94 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 175 | gpu-compute: clipmap + beam + sphere | 1017.8 | 0.24 | 1.33 | 0.95 | 0.01 | 58.3 | 0.00 | 27 | 214 |
| 176 | gpu-compute: bricks + rowrun + pathtrace | 987.3 | 0.23 | 1.50 | 0.99 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 177 | gpu-compute: hier + beam + rowrun + pathtrace | 984.6 | 0.23 | 1.43 | 0.99 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 178 | gpu-compute: tree64 + sphere | 975.1 | 0.23 | 1.44 | 0.99 | 0.06 | 58.3 | 0.00 | 4 | 213 |
| 179 | gpu-compute: bricks + beam + rowrun + pathtrace | 958.7 | 0.23 | 1.47 | 1.01 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 180 | gpu-compute: tree64 + incremental + pathtrace | 957.4 | 0.22 | 2.21 | 1.00 | 0.07 | 36.2 | 0.88 | 12 | 214 |
| 181 | gpu-compute: tree64 + beam + sphere | 955.4 | 0.22 | 1.82 | 1.01 | 0.05 | 58.3 | 0.00 | 4 | 214 |
| 182 | gpu-compute: tree64 + beam + incremental + pathtrace | 924.3 | 0.22 | 1.53 | 1.05 | 0.06 | 36.2 | 0.88 | 12 | 214 |
| 183 | gpu-compute: clipmap + rowrun + pathtrace | 910.3 | 0.21 | 1.46 | 1.07 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 184 | gpu-compute: clipmap + beam + rowrun + pathtrace | 905.7 | 0.21 | 1.82 | 1.07 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 185 | gpu-compute: clipmap + incremental + pathtrace | 808.3 | 0.19 | 1.69 | 1.21 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 186 | gpu-compute: clipmap + beam + incremental + pathtrace | 798.1 | 0.19 | 1.81 | 1.22 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 187 | gpu-compute: hier + beam + incremental + pathtrace | 685.1 | 0.16 | 2.01 | 1.43 | 0.01 | 36.2 | 0.88 | 20 | 213 |
| 188 | gpu-compute: prefix + beam + incremental + pathtrace | 656.1 | 0.15 | 2.03 | 1.50 | 0.01 | 36.2 | 0.88 | 21 | 213 |
| 189 | gpu-compute: hier + incremental + pathtrace | 654.7 | 0.15 | 2.00 | 1.50 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 190 | gpu-compute: bricks + beam + incremental + pathtrace | 648.9 | 0.15 | 2.03 | 1.51 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 191 | gpu-compute: tree64 + beam + rowrun + pathtrace | 646.5 | 0.15 | 2.40 | 1.52 | 0.06 | 36.2 | 0.88 | 12 | 214 |
| 192 | gpu-compute: hier + beam + sphere + pathtrace | 639.8 | 0.15 | 1.93 | 1.54 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 193 | gpu-compute: hier + sphere + pathtrace | 635.2 | 0.15 | 2.17 | 1.55 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 194 | gpu-compute: tree64 + rowrun + pathtrace | 631.2 | 0.15 | 2.40 | 1.56 | 0.06 | 36.2 | 0.88 | 12 | 214 |
| 195 | gpu-compute: prefix + incremental + pathtrace | 620.8 | 0.15 | 2.06 | 1.59 | 0.01 | 36.2 | 0.88 | 21 | 212 |
| 196 | gpu-compute: bricks + incremental + pathtrace | 618.3 | 0.15 | 2.10 | 1.59 | 0.01 | 36.2 | 0.88 | 20 | 211 |
| 197 | gpu-compute: svo_dag + incremental + pathtrace | 611.6 | 0.14 | 2.15 | 1.61 | 0.04 | 36.2 | 0.88 | 12 | 212 |
| 198 | gpu-compute: rows + beam + incremental + pathtrace | 607.7 | 0.14 | 2.10 | 1.62 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 199 | gpu-compute: svo_dag + beam + sphere + pathtrace | 603.7 | 0.14 | 2.17 | 1.63 | 0.05 | 36.2 | 0.88 | 12 | 213 |
| 200 | gpu-compute: svo_dag + sphere + pathtrace | 598.9 | 0.14 | 2.05 | 1.64 | 0.04 | 36.2 | 0.88 | 12 | 213 |
| 201 | gpu-compute: rows + incremental + pathtrace | 585.2 | 0.14 | 2.16 | 1.68 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 202 | gpu-compute: svo_dag + beam + incremental + pathtrace | 583.7 | 0.14 | 2.11 | 1.68 | 0.05 | 36.2 | 0.88 | 12 | 213 |
| 203 | gpu-compute: prefix + sphere + pathtrace | 577.1 | 0.14 | 2.08 | 1.71 | 0.01 | 36.2 | 0.88 | 21 | 212 |
| 204 | gpu-compute: prefix + beam + sphere + pathtrace | 574.6 | 0.14 | 2.25 | 1.71 | 0.01 | 36.2 | 0.88 | 21 | 212 |
| 205 | gpu-compute: bricks + beam + sphere + pathtrace | 540.3 | 0.13 | 2.34 | 1.82 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 206 | gpu-compute: rows + beam + sphere + pathtrace | 538.0 | 0.13 | 2.36 | 1.83 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 207 | gpu-compute: bricks + sphere + pathtrace | 537.4 | 0.13 | 2.34 | 1.83 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 208 | gpu-compute: rows + sphere + pathtrace | 526.8 | 0.12 | 4.37 | 1.86 | 0.01 | 36.2 | 0.88 | 20 | 212 |
| 209 | gpu-compute: distance + beam + incremental + pathtrace | 519.9 | 0.12 | 5.44 | 1.80 | 0.01 | 36.2 | 0.88 | 24 | 213 |
| 210 | gpu-compute: distance + incremental + pathtrace | 488.2 | 0.11 | 5.38 | 1.92 | 0.01 | 36.2 | 0.88 | 24 | 212 |
| 211 | gpu-compute: tree64 + beam + sphere + pathtrace | 482.1 | 0.11 | 3.01 | 2.02 | 0.08 | 36.2 | 0.88 | 12 | 214 |
| 212 | gpu-compute: tree64 + sphere + pathtrace | 477.8 | 0.11 | 2.60 | 2.07 | 0.07 | 36.2 | 0.88 | 12 | 214 |
| 213 | gpu-compute: clipmap + sphere + pathtrace | 472.0 | 0.11 | 5.06 | 2.07 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 214 | gpu-compute: clipmap + beam + sphere + pathtrace | 460.6 | 0.11 | 2.72 | 2.14 | 0.01 | 36.2 | 0.88 | 35 | 214 |
| 215 | cpu-discover: brickmajor + quadtree | 269.6 | 0.06 | 7.56 | 0.40 | 3.56 | 36.8 | 0.35 | 16 | 282 |
| 216 | cpu-discover: prefix + quadtree | 267.6 | 0.06 | 7.32 | 0.40 | 3.58 | 36.8 | 0.35 | 16 | 283 |
| 217 | cpu-discover: hier + quadtree | 266.5 | 0.06 | 6.51 | 0.40 | 3.59 | 36.8 | 0.35 | 16 | 279 |
| 218 | cpu-discover: bits + quadtree | 266.1 | 0.06 | 7.13 | 0.40 | 3.60 | 36.8 | 0.35 | 16 | 280 |
| 219 | cpu-discover: u8 + quadtree | 264.7 | 0.06 | 7.78 | 0.40 | 3.63 | 36.8 | 0.35 | 16 | 286 |
| 220 | cpu-discover: palette + quadtree | 264.6 | 0.06 | 7.53 | 0.40 | 3.60 | 36.8 | 0.35 | 16 | 281 |
| 221 | cpu-discover: morton + quadtree | 264.1 | 0.06 | 7.67 | 0.40 | 3.62 | 36.8 | 0.35 | 16 | 289 |
| 222 | cpu-discover: bricks + quadtree | 257.6 | 0.06 | 7.56 | 0.40 | 3.69 | 36.8 | 0.35 | 16 | 280 |
| 223 | cpu: hier + refloor | 115.5 | 0.03 | 11.97 | - | 8.64 | 59.6 | 0.00 | - | 28 |
| 224 | cpu: prefix + refloor | 105.8 | 0.02 | 13.41 | - | 9.42 | 59.6 | 0.00 | - | 22 |
| 225 | cpu-raster: greedy + raster | 102.4 | 0.02 | 11.90 | - | 9.69 | 71.0 | 0.00 | - | 119 |
| 226 | cpu-raster: naive + raster | 102.2 | 0.02 | 11.38 | - | 9.70 | 71.0 | 0.00 | - | 110 |
| 227 | cpu: bricks + refloor | 95.8 | 0.02 | 13.85 | - | 10.41 | 59.6 | 0.00 | - | 28 |
| 228 | cpu: hier + incremental | 95.3 | 0.02 | 14.26 | - | 10.46 | 59.3 | 0.00 | - | 28 |
| 229 | cpu: morton + refloor | 94.2 | 0.02 | 15.16 | - | 10.59 | 59.6 | 0.00 | - | 28 |
| 230 | cpu: bits + refloor | 94.0 | 0.02 | 14.83 | - | 10.61 | 59.6 | 0.00 | - | 28 |
| 231 | cpu: u8 + refloor | 94.0 | 0.02 | 15.31 | - | 10.61 | 59.6 | 0.00 | - | 25 |
| 232 | cpu: brickmajor + refloor | 93.0 | 0.02 | 15.51 | - | 10.72 | 59.6 | 0.00 | - | 28 |
| 233 | cpu: palette + refloor | 88.3 | 0.02 | 14.63 | - | 11.28 | 59.6 | 0.00 | - | 24 |
| 234 | cpu: bricks + incremental | 87.4 | 0.02 | 15.45 | - | 11.42 | 59.3 | 0.00 | - | 28 |
| 235 | cpu: prefix + incremental | 87.0 | 0.02 | 14.99 | - | 11.46 | 59.3 | 0.00 | - | 23 |
| 236 | cpu: morton + incremental | 86.3 | 0.02 | 15.47 | - | 11.56 | 59.3 | 0.00 | - | 28 |
| 237 | cpu: bits + incremental | 83.8 | 0.02 | 15.88 | - | 11.91 | 59.3 | 0.00 | - | 28 |
| 238 | cpu: palette + incremental | 80.3 | 0.02 | 15.29 | - | 12.41 | 59.3 | 0.00 | - | 22 |
| 239 | cpu: brickmajor + incremental | 79.9 | 0.02 | 16.48 | - | 12.48 | 59.3 | 0.00 | - | 28 |
| 240 | cpu: u8 + incremental | 79.4 | 0.02 | 17.41 | - | 12.57 | 59.3 | 0.00 | - | 25 |
| 241 | cpu-discover: hier + cluster | 57.6 | 0.01 | 36.04 | 0.40 | 17.18 | 44.0 | 0.03 | 16 | 258 |
| 242 | cpu-discover: prefix + cluster | 55.2 | 0.01 | 38.48 | 0.40 | 17.92 | 44.0 | 0.03 | 16 | 258 |
| 243 | cpu-discover: bricks + cluster | 54.9 | 0.01 | 35.64 | 0.40 | 18.06 | 44.0 | 0.03 | 16 | 259 |
| 244 | cpu-discover: u8 + cluster | 54.6 | 0.01 | 37.96 | 0.40 | 18.14 | 44.0 | 0.03 | 16 | 260 |
| 245 | cpu-discover: bits + cluster | 54.3 | 0.01 | 37.73 | 0.40 | 18.26 | 44.0 | 0.03 | 16 | 260 |
| 246 | cpu-discover: morton + cluster | 54.0 | 0.01 | 40.34 | 0.41 | 18.35 | 44.0 | 0.03 | 16 | 263 |
| 247 | cpu-discover: brickmajor + cluster | 53.6 | 0.01 | 40.78 | 0.40 | 18.49 | 44.0 | 0.03 | 16 | 271 |
| 248 | cpu-discover: palette + cluster | 53.0 | 0.01 | 39.47 | 0.40 | 18.68 | 44.0 | 0.03 | 16 | 259 |
| 249 | gpu-splat: prefix + points + splat (incorrect) | 3352.8 | 0.79 | 1.33 | 0.21 | 0.01 | 10.9 | 60.30 | 17 | 216 |
| 250 | gpu-splat: bricks + points + splat (incorrect) | 3333.3 | 0.78 | 1.16 | 0.22 | 0.01 | 10.9 | 60.30 | 16 | 214 |
| 251 | gpu-splat: hier + points + splat (incorrect) | 3283.6 | 0.77 | 1.42 | 0.21 | 0.01 | 10.9 | 60.30 | 16 | 215 |
| 252 | gpu-splat: svo_dag + points + splat (incorrect) | 3231.6 | 0.76 | 1.19 | 0.21 | 0.03 | 10.9 | 60.30 | 8 | 215 |
| 253 | gpu-splat: rows + points + splat (incorrect) | 3181.6 | 0.75 | 1.38 | 0.22 | 0.01 | 10.9 | 60.30 | 16 | 214 |
| 254 | gpu-splat: clipmap + points + splat (incorrect) | 3055.9 | 0.72 | 1.33 | 0.24 | 0.01 | 10.9 | 60.30 | 31 | 216 |
| 255 | gpu-splat: tree64 + points + splat (incorrect) | 2723.7 | 0.64 | 1.91 | 0.22 | 0.06 | 10.9 | 60.30 | 8 | 218 |
| 256 | gpu-splat: distance + points + splat (incorrect) | 1952.7 | 0.46 | 5.72 | 0.20 | 0.01 | 10.9 | 60.30 | 20 | 215 |
| 257 | gpu-voxelspace: heightmap + columns (incorrect) | 785.1 | 0.18 | 1.99 | 1.25 | 0.01 | 11.0 | 53.75 | 3 | 215 |
| 258 | cpu-voxelspace: heightmap + columns (incorrect) | 624.3 | 0.15 | 2.02 | - | 1.54 | 11.0 | 55.99 | - | 31 |

### showcase [unlit]

960×540. camera 1.36 blocks/frame (max 1.4), turn 2.72°/frame; 84 chunks at start, 0.5 loads/frame; chunk fill 12%, solid 0.3%, surface 0.35%; sky 28%, hits at 38 blocks (median 29); 0.0 edits/frame; 2 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-raster: bricks + naive + raster | 7870.8 | 1.00 | 0.94 | 0.03 | 0.03 | 44.1 | 0.03 | 17 | 215 |
| 2 | gpu-raster: bricks + naive + visbuffer | 6851.2 | 0.87 | 0.99 | 0.04 | 0.04 | 44.1 | 0.03 | 17 | 215 |
| 3 | gpu-compute: hier + refloor | 6394.0 | 0.81 | 0.55 | 0.11 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 4 | gpu-raster: bricks + naive + gbuffer | 6341.6 | 0.81 | 1.03 | 0.06 | 0.04 | 44.1 | 0.03 | 17 | 214 |
| 5 | gpu-compute: prefix + refloor | 5989.1 | 0.76 | 0.59 | 0.11 | 0.01 | 60.9 | 0.00 | 13 | 212 |
| 6 | gpu-compute: prefix + rowrun | 5976.2 | 0.76 | 0.55 | 0.12 | 0.01 | 60.9 | 0.00 | 13 | 212 |
| 7 | gpu-raster: bricks + greedy + raster | 5896.4 | 0.75 | 1.84 | 0.03 | 0.08 | 44.1 | 0.03 | 17 | 215 |
| 8 | gpu-compute: svo_dag + refloor | 5889.0 | 0.75 | 0.96 | 0.11 | 0.03 | 60.9 | 0.00 | 4 | 213 |
| 9 | gpu-compute: prefix + beam + refloor | 5785.5 | 0.74 | 0.48 | 0.12 | 0.01 | 60.9 | 0.00 | 13 | 212 |
| 10 | gpu-compute: hier + beam + refloor | 5746.9 | 0.73 | 0.52 | 0.11 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 11 | gpu-compute: hier + rowrun | 5695.6 | 0.72 | 0.54 | 0.13 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 12 | gpu-compute: prefix + beam + rowrun | 5672.3 | 0.72 | 0.41 | 0.13 | 0.01 | 60.9 | 0.00 | 13 | 213 |
| 13 | gpu-compute: svo_dag + beam + refloor | 5627.9 | 0.72 | 0.75 | 0.12 | 0.03 | 60.9 | 0.00 | 4 | 213 |
| 14 | gpu-raster: bricks + greedy + visbuffer | 5581.6 | 0.71 | 1.67 | 0.04 | 0.08 | 44.1 | 0.03 | 17 | 215 |
| 15 | gpu-raster: bricks + cubes + raster | 5444.3 | 0.69 | 0.71 | 0.11 | 0.02 | 44.1 | 0.03 | 16 | 214 |
| 16 | gpu-compute: hier + beam + rowrun | 5277.2 | 0.67 | 0.53 | 0.14 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 17 | gpu-raster: bricks + greedy + gbuffer | 5246.8 | 0.67 | 1.80 | 0.06 | 0.08 | 44.1 | 0.03 | 17 | 215 |
| 18 | gpu-compute: bricks + beam + refloor | 5234.3 | 0.67 | 0.54 | 0.15 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 19 | gpu-compute: bricks + refloor | 5225.0 | 0.66 | 0.60 | 0.14 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 20 | gpu-raster: bricks + cubes + visbuffer | 5212.8 | 0.66 | 0.72 | 0.12 | 0.03 | 44.1 | 0.03 | 16 | 214 |
| 21 | gpu-compute: rows + refloor | 5205.6 | 0.66 | 0.51 | 0.14 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 22 | gpu-compute: rows + rowrun | 5192.4 | 0.66 | 0.45 | 0.15 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 23 | gpu-compute: bricks + rowrun | 5106.4 | 0.65 | 0.51 | 0.16 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 24 | gpu-compute: rows + beam + refloor | 5064.4 | 0.64 | 0.50 | 0.15 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 25 | gpu-compute: rows + beam + rowrun | 4857.4 | 0.62 | 0.54 | 0.15 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 26 | gpu-compute: bricks + beam + rowrun | 4832.1 | 0.61 | 0.53 | 0.16 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 27 | gpu-raster: bricks + cubes + gbuffer | 4700.6 | 0.60 | 0.66 | 0.14 | 0.03 | 44.1 | 0.03 | 16 | 213 |
| 28 | gpu-compute: svo_dag + rowrun | 4615.8 | 0.59 | 0.83 | 0.15 | 0.03 | 60.9 | 0.00 | 4 | 212 |
| 29 | gpu-compute: tree64 + refloor | 4515.1 | 0.57 | 1.24 | 0.14 | 0.05 | 60.9 | 0.00 | 4 | 213 |
| 30 | gpu-compute: svo_dag + beam + rowrun | 4481.0 | 0.57 | 0.95 | 0.16 | 0.04 | 60.9 | 0.00 | 4 | 213 |
| 31 | gpu-compute: hier + incremental | 4444.4 | 0.56 | 0.55 | 0.18 | 0.01 | 60.6 | 0.00 | 12 | 212 |
| 32 | gpu-compute: clipmap + refloor | 4269.3 | 0.54 | 0.50 | 0.20 | 0.01 | 60.9 | 0.00 | 27 | 214 |
| 33 | gpu-compute: hier + beam + incremental | 4232.1 | 0.54 | 0.53 | 0.19 | 0.01 | 60.6 | 0.00 | 12 | 212 |
| 34 | gpu-compute: tree64 + beam + refloor | 4156.5 | 0.53 | 1.52 | 0.15 | 0.06 | 60.9 | 0.00 | 4 | 214 |
| 35 | gpu-compute: clipmap + rowrun | 4060.4 | 0.52 | 0.51 | 0.21 | 0.01 | 60.9 | 0.00 | 27 | 214 |
| 36 | gpu-compute: rows + incremental | 4058.2 | 0.52 | 0.67 | 0.21 | 0.01 | 60.6 | 0.00 | 12 | 212 |
| 37 | gpu-compute: clipmap + beam + refloor | 4036.8 | 0.51 | 0.58 | 0.20 | 0.01 | 60.9 | 0.00 | 27 | 214 |
| 38 | gpu-compute: bricks + beam + incremental | 3978.9 | 0.51 | 0.65 | 0.21 | 0.01 | 60.6 | 0.00 | 12 | 212 |
| 39 | gpu-compute: clipmap + beam + rowrun | 3952.9 | 0.50 | 0.52 | 0.22 | 0.01 | 60.9 | 0.00 | 27 | 214 |
| 40 | gpu-compute: bricks + incremental | 3886.9 | 0.49 | 0.95 | 0.20 | 0.02 | 60.6 | 0.00 | 12 | 212 |
| 41 | gpu-compute: rows + beam + incremental | 3786.2 | 0.48 | 0.65 | 0.23 | 0.01 | 60.6 | 0.00 | 12 | 212 |
| 42 | gpu-compute: prefix + incremental | 3746.4 | 0.48 | 0.67 | 0.22 | 0.01 | 60.6 | 0.00 | 13 | 212 |
| 43 | gpu-compute: tree64 + rowrun | 3657.4 | 0.46 | 1.11 | 0.21 | 0.05 | 60.9 | 0.00 | 4 | 213 |
| 44 | gpu-rt: boxes + rayquery | 3628.8 | 0.46 | 0.68 | 0.23 | 0.08 | 60.9 | 0.00 | 10 | 353 |
| 45 | gpu-compute: prefix + beam + incremental | 3547.2 | 0.45 | 0.69 | 0.24 | 0.01 | 60.6 | 0.00 | 13 | 212 |
| 46 | gpu-compute: clipmap + beam + incremental | 3526.9 | 0.45 | 0.57 | 0.25 | 0.01 | 60.6 | 0.00 | 27 | 214 |
| 47 | gpu-compute: tree64 + incremental | 3522.2 | 0.45 | 1.28 | 0.22 | 0.05 | 60.6 | 0.00 | 4 | 213 |
| 48 | gpu-rt: naive + rayquery | 3498.9 | 0.44 | 1.95 | 0.17 | 0.13 | 60.3 | 0.00 | 15 | 363 |
| 49 | gpu-compute: clipmap + incremental | 3492.5 | 0.44 | 0.55 | 0.24 | 0.01 | 60.6 | 0.00 | 27 | 214 |
| 50 | gpu-compute: tree64 + beam + rowrun | 3397.2 | 0.43 | 1.14 | 0.22 | 0.06 | 60.9 | 0.00 | 4 | 214 |
| 51 | gpu-compute: tree64 + beam + incremental | 3300.3 | 0.42 | 1.24 | 0.23 | 0.05 | 60.6 | 0.00 | 4 | 214 |
| 52 | gpu-rt: greedy + rayquery | 3024.4 | 0.38 | 2.87 | 0.17 | 0.17 | 60.3 | 0.00 | 15 | 365 |
| 53 | gpu-compute: distance + sphere | 2807.7 | 0.36 | 4.99 | 0.08 | 0.01 | 60.9 | 0.00 | 16 | 213 |
| 54 | gpu-compute: distance + beam + sphere | 2717.0 | 0.35 | 4.99 | 0.09 | 0.01 | 60.9 | 0.00 | 16 | 213 |
| 55 | gpu-compute: hier + sphere | 2681.3 | 0.34 | 0.78 | 0.34 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 56 | gpu-compute: distance + refloor | 2643.3 | 0.34 | 5.03 | 0.10 | 0.01 | 60.9 | 0.00 | 16 | 213 |
| 57 | gpu-compute: distance + beam + refloor | 2633.3 | 0.33 | 4.88 | 0.11 | 0.01 | 60.9 | 0.00 | 16 | 213 |
| 58 | gpu-compute: distance + rowrun | 2633.1 | 0.33 | 5.07 | 0.10 | 0.01 | 60.9 | 0.00 | 16 | 212 |
| 59 | gpu-compute: svo_dag + incremental | 2593.9 | 0.33 | 0.96 | 0.33 | 0.04 | 60.6 | 0.00 | 4 | 213 |
| 60 | gpu-compute: hier + beam + sphere | 2588.4 | 0.33 | 0.76 | 0.35 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 61 | gpu-compute: svo_dag + beam + incremental | 2554.1 | 0.32 | 0.97 | 0.34 | 0.04 | 60.6 | 0.00 | 4 | 213 |
| 62 | gpu-compute: distance + beam + rowrun | 2501.4 | 0.32 | 5.52 | 0.11 | 0.01 | 60.9 | 0.00 | 16 | 212 |
| 63 | gpu-compute: svo_dag + sphere | 2291.4 | 0.29 | 1.12 | 0.39 | 0.04 | 60.9 | 0.00 | 4 | 212 |
| 64 | gpu-compute: prefix + sphere | 2274.5 | 0.29 | 0.86 | 0.41 | 0.01 | 60.9 | 0.00 | 13 | 212 |
| 65 | gpu-compute: svo_dag + beam + sphere | 2254.2 | 0.29 | 1.11 | 0.40 | 0.04 | 60.9 | 0.00 | 4 | 212 |
| 66 | gpu-compute: prefix + beam + sphere | 2217.5 | 0.28 | 0.82 | 0.42 | 0.01 | 60.9 | 0.00 | 13 | 212 |
| 67 | gpu-compute: rows + sphere | 2202.1 | 0.28 | 0.78 | 0.43 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 68 | gpu-compute: bricks + sphere | 2173.8 | 0.28 | 1.05 | 0.43 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 69 | gpu-discover: hier + quadtree | 2171.4 | 0.28 | 0.79 | 0.40 | 0.04 | 44.1 | 0.03 | 120 | 224 |
| 70 | gpu-discover: bricks + quadtree | 2148.5 | 0.27 | 0.87 | 0.41 | 0.04 | 44.1 | 0.03 | 120 | 222 |
| 71 | gpu-discover: prefix + quadtree | 2148.0 | 0.27 | 0.76 | 0.40 | 0.04 | 44.1 | 0.03 | 120 | 225 |
| 72 | gpu-compute: rows + beam + sphere | 2133.7 | 0.27 | 0.86 | 0.44 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 73 | gpu-discover: rows + quadtree | 2108.8 | 0.27 | 0.81 | 0.41 | 0.04 | 44.1 | 0.03 | 120 | 222 |
| 74 | gpu-compute: bricks + beam + sphere | 2099.5 | 0.27 | 1.05 | 0.44 | 0.01 | 60.9 | 0.00 | 12 | 212 |
| 75 | gpu-compute: distance + incremental | 1929.4 | 0.25 | 5.13 | 0.25 | 0.01 | 60.6 | 0.00 | 16 | 212 |
| 76 | gpu-compute: clipmap + sphere | 1893.5 | 0.24 | 0.84 | 0.50 | 0.01 | 60.9 | 0.00 | 27 | 214 |
| 77 | gpu-discover: clipmap + quadtree | 1881.3 | 0.24 | 0.88 | 0.48 | 0.04 | 44.1 | 0.03 | 135 | 224 |
| 78 | gpu-compute: distance + beam + incremental | 1852.1 | 0.24 | 4.93 | 0.27 | 0.01 | 60.6 | 0.00 | 16 | 213 |
| 79 | gpu-discover: svo_dag + quadtree | 1827.4 | 0.23 | 1.24 | 0.46 | 0.06 | 44.1 | 0.03 | 112 | 222 |
| 80 | gpu-compute: clipmap + beam + sphere | 1818.8 | 0.23 | 0.86 | 0.52 | 0.01 | 60.9 | 0.00 | 27 | 214 |
| 81 | gpu-discover: tree64 + quadtree | 1810.0 | 0.23 | 1.54 | 0.46 | 0.08 | 44.1 | 0.03 | 112 | 223 |
| 82 | gpu-compute: tree64 + sphere | 1610.0 | 0.20 | 1.26 | 0.58 | 0.05 | 60.9 | 0.00 | 4 | 213 |
| 83 | gpu-compute: tree64 + beam + sphere | 1600.6 | 0.20 | 1.27 | 0.58 | 0.05 | 60.9 | 0.00 | 4 | 214 |
| 84 | gpu-discover: distance + quadtree | 1467.9 | 0.19 | 5.11 | 0.40 | 0.03 | 44.1 | 0.03 | 124 | 222 |
| 85 | cpu-raster: naive + raster | 352.7 | 0.04 | 3.85 | - | 2.76 | 71.0 | 0.00 | - | 107 |
| 86 | cpu-raster: greedy + raster | 347.6 | 0.04 | 3.79 | - | 2.80 | 71.0 | 0.00 | - | 110 |
| 87 | cpu: hier + refloor | 276.6 | 0.04 | 7.05 | - | 3.59 | 64.8 | 0.00 | - | 28 |
| 88 | cpu-discover: hier + quadtree | 272.9 | 0.03 | 7.64 | 0.04 | 3.54 | 37.7 | 0.34 | 16 | 283 |
| 89 | cpu-discover: bits + quadtree | 264.7 | 0.03 | 7.84 | 0.04 | 3.65 | 37.7 | 0.34 | 16 | 281 |
| 90 | cpu-discover: morton + quadtree | 263.7 | 0.03 | 7.13 | 0.04 | 3.65 | 37.7 | 0.34 | 16 | 285 |
| 91 | cpu-discover: u8 + quadtree | 262.9 | 0.03 | 9.26 | 0.04 | 3.67 | 37.7 | 0.34 | 16 | 281 |
| 92 | cpu-discover: prefix + quadtree | 261.0 | 0.03 | 7.89 | 0.04 | 3.68 | 37.7 | 0.34 | 16 | 279 |
| 93 | cpu-discover: bricks + quadtree | 260.8 | 0.03 | 8.50 | 0.04 | 3.70 | 37.7 | 0.34 | 16 | 279 |
| 94 | cpu-discover: palette + quadtree | 259.9 | 0.03 | 7.71 | 0.04 | 3.68 | 37.7 | 0.34 | 16 | 279 |
| 95 | cpu-discover: brickmajor + quadtree | 257.7 | 0.03 | 8.25 | 0.04 | 3.72 | 37.7 | 0.34 | 16 | 288 |
| 96 | cpu: prefix + refloor | 226.5 | 0.03 | 7.54 | - | 4.38 | 64.8 | 0.00 | - | 23 |
| 97 | cpu: hier + incremental | 212.7 | 0.03 | 7.69 | - | 4.68 | 63.9 | 0.00 | - | 28 |
| 98 | cpu: bricks + refloor | 211.8 | 0.03 | 8.35 | - | 4.70 | 64.8 | 0.00 | - | 28 |
| 99 | cpu: u8 + refloor | 207.8 | 0.03 | 8.13 | - | 4.79 | 64.8 | 0.00 | - | 25 |
| 100 | cpu: brickmajor + refloor | 207.6 | 0.03 | 8.37 | - | 4.78 | 64.8 | 0.00 | - | 28 |
| 101 | cpu: bits + refloor | 205.5 | 0.03 | 8.57 | - | 4.84 | 64.8 | 0.00 | - | 28 |
| 102 | cpu: morton + refloor | 205.0 | 0.03 | 8.35 | - | 4.85 | 64.8 | 0.00 | - | 28 |
| 103 | cpu: palette + refloor | 203.5 | 0.03 | 8.73 | - | 4.87 | 64.8 | 0.00 | - | 23 |
| 104 | cpu: bricks + incremental | 179.1 | 0.02 | 8.65 | - | 5.56 | 63.9 | 0.00 | - | 27 |
| 105 | cpu: bits + incremental | 177.8 | 0.02 | 9.00 | - | 5.60 | 63.9 | 0.00 | - | 28 |
| 106 | cpu: u8 + incremental | 177.1 | 0.02 | 8.96 | - | 5.62 | 63.9 | 0.00 | - | 25 |
| 107 | cpu: morton + incremental | 177.0 | 0.02 | 9.16 | - | 5.61 | 63.9 | 0.00 | - | 28 |
| 108 | cpu: brickmajor + incremental | 176.8 | 0.02 | 9.20 | - | 5.62 | 63.9 | 0.00 | - | 28 |
| 109 | cpu: palette + incremental | 176.1 | 0.02 | 8.77 | - | 5.63 | 63.9 | 0.00 | - | 23 |
| 110 | cpu: prefix + incremental | 175.6 | 0.02 | 9.09 | - | 5.66 | 63.9 | 0.00 | - | 23 |
| 111 | cpu-discover: hier + cluster | 56.7 | 0.01 | 37.12 | 0.04 | 17.49 | 44.1 | 0.03 | 16 | 260 |
| 112 | cpu-discover: prefix + cluster | 55.2 | 0.01 | 40.17 | 0.04 | 17.95 | 44.1 | 0.03 | 16 | 261 |
| 113 | cpu-discover: bits + cluster | 54.8 | 0.01 | 38.70 | 0.04 | 18.10 | 44.1 | 0.03 | 16 | 261 |
| 114 | cpu-discover: bricks + cluster | 54.7 | 0.01 | 39.95 | 0.04 | 18.13 | 44.1 | 0.03 | 16 | 256 |
| 115 | cpu-discover: morton + cluster | 54.0 | 0.01 | 44.92 | 0.04 | 18.34 | 44.1 | 0.03 | 16 | 265 |
| 116 | cpu-discover: u8 + cluster | 53.9 | 0.01 | 40.80 | 0.04 | 18.40 | 44.1 | 0.03 | 16 | 259 |
| 117 | cpu-discover: brickmajor + cluster | 53.6 | 0.01 | 40.64 | 0.04 | 18.50 | 44.1 | 0.03 | 16 | 261 |
| 118 | cpu-discover: palette + cluster | 53.2 | 0.01 | 40.29 | 0.04 | 18.63 | 44.1 | 0.03 | 16 | 266 |
| 119 | gpu-splat: bricks + points + splat (incorrect) | 4753.8 | 0.60 | 1.35 | 0.12 | 0.01 | 12.3 | 55.70 | 16 | 214 |
| 120 | gpu-voxelspace: heightmap + columns (incorrect) | 2426.1 | 0.31 | 0.97 | 0.36 | 0.01 | 12.1 | 47.79 | 3 | 215 |
| 121 | cpu-voxelspace: heightmap + columns (incorrect) | 1198.5 | 0.15 | 1.50 | - | 0.77 | 12.1 | 47.79 | - | 31 |

### streaming [unlit]

1920×1080. camera 6.87 blocks/frame (max 6.9), turn 0.00°/frame; 3753 chunks at start, 30.5 loads/frame; chunk fill 11%, solid 8.2%, surface 0.63%; sky 71%, hits at 346 blocks (median 329); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: bricks + refloor | 486.1 | 1.00 | 8.07 | 0.78 | 0.08 | 62.1 | 0.01 | 530 | 505 |
| 2 | gpu-compute: tree64 + refloor | 443.4 | 0.91 | 9.93 | 0.84 | 1.22 | 62.1 | 0.01 | 99 | 531 |
| 3 | gpu-compute: bricks + beam + refloor | 435.7 | 0.90 | 8.68 | 1.04 | 0.08 | 62.2 | 0.01 | 530 | 506 |
| 4 | gpu-compute: bricks + rowrun | 409.4 | 0.84 | 7.75 | 1.33 | 0.08 | 62.1 | 0.01 | 530 | 506 |
| 5 | gpu-compute: tree64 + incremental | 390.2 | 0.80 | 8.52 | 1.36 | 1.21 | 62.0 | 0.01 | 99 | 530 |
| 6 | gpu-compute: tree64 + beam + refloor | 386.9 | 0.80 | 10.03 | 1.34 | 1.23 | 62.2 | 0.01 | 99 | 531 |
| 7 | gpu-compute: bricks + beam + rowrun | 384.2 | 0.79 | 7.87 | 1.59 | 0.08 | 62.2 | 0.01 | 530 | 506 |
| 8 | gpu-compute: clipmap + refloor | 376.7 | 0.77 | 9.17 | 1.25 | 0.02 | 62.1 | 0.01 | 571 | 504 |
| 9 | gpu-compute: hier + refloor | 375.6 | 0.77 | 12.00 | 0.71 | 0.07 | 62.1 | 0.01 | 530 | 503 |
| 10 | gpu-compute: bricks + sphere | 372.1 | 0.77 | 6.53 | 1.84 | 0.08 | 62.1 | 0.01 | 530 | 506 |
| 11 | gpu-compute: prefix + beam + refloor | 368.8 | 0.76 | 11.15 | 0.95 | 0.08 | 62.2 | 0.01 | 570 | 503 |
| 12 | gpu-compute: prefix + rowrun | 366.7 | 0.75 | 11.63 | 0.94 | 0.07 | 62.0 | 0.01 | 570 | 503 |
| 13 | gpu-compute: prefix + refloor | 365.9 | 0.75 | 12.33 | 0.69 | 0.08 | 62.0 | 0.01 | 570 | 503 |
| 14 | gpu-compute: clipmap + incremental | 359.5 | 0.74 | 7.87 | 1.62 | 0.02 | 62.0 | 0.01 | 571 | 505 |
| 15 | gpu-compute: hier + beam + refloor | 355.0 | 0.73 | 11.57 | 0.98 | 0.08 | 62.2 | 0.01 | 530 | 503 |
| 16 | gpu-discover: bricks + quadtree | 353.3 | 0.73 | 6.54 | 1.87 | 0.14 | 55.3 | 0.02 | 712 | 520 |
| 17 | gpu-compute: clipmap + rowrun | 352.1 | 0.72 | 7.79 | 1.68 | 0.02 | 62.1 | 0.01 | 571 | 505 |
| 18 | gpu-compute: prefix + beam + rowrun | 351.8 | 0.72 | 10.99 | 1.19 | 0.07 | 62.2 | 0.01 | 570 | 504 |
| 19 | gpu-compute: bricks + beam + sphere | 348.2 | 0.72 | 6.61 | 2.08 | 0.08 | 62.2 | 0.01 | 530 | 506 |
| 20 | gpu-compute: rows + refloor | 347.4 | 0.71 | 13.39 | 0.80 | 0.08 | 62.1 | 0.01 | 562 | 503 |
| 21 | gpu-compute: tree64 + beam + incremental | 342.7 | 0.70 | 8.25 | 1.85 | 1.25 | 62.2 | 0.01 | 99 | 532 |
| 22 | gpu-compute: rows + rowrun | 336.6 | 0.69 | 12.60 | 0.98 | 0.07 | 62.1 | 0.01 | 562 | 503 |
| 23 | gpu-compute: rows + beam + refloor | 334.4 | 0.69 | 11.95 | 1.05 | 0.07 | 62.2 | 0.01 | 562 | 503 |
| 24 | gpu-compute: clipmap + beam + refloor | 324.5 | 0.67 | 7.25 | 2.14 | 0.02 | 62.2 | 0.01 | 571 | 503 |
| 25 | gpu-compute: rows + beam + rowrun | 319.2 | 0.66 | 11.69 | 1.25 | 0.07 | 62.2 | 0.01 | 562 | 503 |
| 26 | gpu-compute: hier + sphere | 317.8 | 0.65 | 9.42 | 1.64 | 0.07 | 62.1 | 0.01 | 530 | 503 |
| 27 | gpu-discover: tree64 + quadtree | 317.1 | 0.65 | 7.91 | 2.14 | 1.30 | 55.3 | 0.02 | 281 | 540 |
| 28 | gpu-compute: svo_dag + refloor | 312.7 | 0.64 | 14.23 | 0.77 | 2.24 | 62.0 | 0.01 | 40 | 624 |
| 29 | gpu-compute: prefix + sphere | 308.2 | 0.63 | 9.23 | 1.82 | 0.08 | 62.1 | 0.01 | 570 | 506 |
| 30 | gpu-compute: clipmap + beam + incremental | 303.3 | 0.62 | 7.64 | 2.53 | 0.02 | 62.2 | 0.01 | 571 | 505 |
| 31 | gpu-discover: clipmap + quadtree | 302.7 | 0.62 | 7.31 | 2.25 | 0.08 | 55.3 | 0.02 | 753 | 512 |
| 32 | gpu-compute: clipmap + sphere | 301.9 | 0.62 | 7.90 | 2.52 | 0.02 | 62.1 | 0.01 | 571 | 505 |
| 33 | gpu-compute: tree64 + sphere | 300.6 | 0.62 | 7.25 | 2.63 | 1.24 | 62.1 | 0.01 | 99 | 533 |
| 34 | gpu-compute: hier + beam + sphere | 300.5 | 0.62 | 9.34 | 1.89 | 0.08 | 62.2 | 0.01 | 530 | 503 |
| 35 | gpu-compute: hier + rowrun | 297.9 | 0.61 | 9.19 | 1.90 | 0.07 | 62.1 | 0.01 | 530 | 503 |
| 36 | gpu-compute: clipmap + beam + rowrun | 296.5 | 0.61 | 7.57 | 2.57 | 0.02 | 62.2 | 0.01 | 571 | 504 |
| 37 | gpu-compute: prefix + beam + sphere | 295.0 | 0.61 | 8.89 | 2.07 | 0.08 | 62.2 | 0.01 | 570 | 504 |
| 38 | gpu-discover: prefix + quadtree | 294.7 | 0.61 | 9.21 | 1.84 | 0.13 | 55.3 | 0.02 | 752 | 515 |
| 39 | gpu-discover: hier + quadtree | 292.1 | 0.60 | 9.79 | 1.83 | 0.14 | 55.3 | 0.02 | 712 | 515 |
| 40 | gpu-compute: rows + sphere | 288.6 | 0.59 | 10.39 | 1.83 | 0.07 | 62.1 | 0.01 | 562 | 503 |
| 41 | gpu-rt: boxes + rayquery | 286.9 | 0.59 | 14.08 | 1.06 | 1.09 | 62.0 | 0.01 | 563 | 679 |
| 42 | gpu-compute: svo_dag + beam + refloor | 285.4 | 0.59 | 12.83 | 1.43 | 2.23 | 62.2 | 0.01 | 40 | 623 |
| 43 | gpu-compute: hier + beam + rowrun | 280.8 | 0.58 | 8.74 | 2.22 | 0.07 | 62.2 | 0.01 | 530 | 506 |
| 44 | gpu-discover: rows + quadtree | 279.6 | 0.58 | 9.45 | 1.86 | 0.13 | 55.3 | 0.02 | 744 | 515 |
| 45 | gpu-compute: rows + beam + sphere | 276.7 | 0.57 | 9.38 | 2.08 | 0.08 | 62.2 | 0.01 | 562 | 503 |
| 46 | gpu-compute: tree64 + beam + sphere | 272.1 | 0.56 | 7.23 | 3.11 | 1.26 | 62.2 | 0.01 | 99 | 532 |
| 47 | gpu-compute: clipmap + beam + sphere | 257.2 | 0.53 | 9.11 | 3.42 | 0.02 | 62.2 | 0.01 | 571 | 505 |
| 48 | gpu-compute: svo_dag + sphere | 254.3 | 0.52 | 12.46 | 2.00 | 2.27 | 62.1 | 0.01 | 40 | 624 |
| 49 | gpu-compute: svo_dag + incremental | 240.1 | 0.49 | 12.50 | 2.44 | 2.26 | 62.0 | 0.01 | 40 | 622 |
| 50 | gpu-discover: svo_dag + quadtree | 238.6 | 0.49 | 12.64 | 2.18 | 2.35 | 55.3 | 0.02 | 222 | 628 |
| 51 | gpu-raster: bricks + naive + raster | 237.3 | 0.49 | 18.62 | 0.63 | 2.19 | 56.7 | 0.02 | 668 | 532 |
| 52 | gpu-raster: bricks + naive + gbuffer | 234.1 | 0.48 | 18.42 | 0.69 | 2.22 | 56.6 | 0.02 | 668 | 534 |
| 53 | gpu-raster: bricks + naive + visbuffer | 232.8 | 0.48 | 18.35 | 0.69 | 2.22 | 56.6 | 0.02 | 668 | 533 |
| 54 | gpu-compute: svo_dag + beam + sphere | 228.2 | 0.47 | 10.80 | 2.64 | 2.30 | 62.2 | 0.01 | 40 | 625 |
| 55 | gpu-compute: bricks + incremental | 226.7 | 0.47 | 8.69 | 4.23 | 0.08 | 62.0 | 0.01 | 530 | 506 |
| 56 | gpu-compute: prefix + incremental | 224.6 | 0.46 | 10.67 | 3.85 | 0.08 | 62.0 | 0.01 | 570 | 504 |
| 57 | gpu-compute: prefix + beam + incremental | 220.8 | 0.45 | 10.21 | 4.09 | 0.08 | 62.2 | 0.01 | 570 | 504 |
| 58 | gpu-compute: svo_dag + beam + incremental | 218.8 | 0.45 | 11.49 | 3.12 | 2.26 | 62.2 | 0.01 | 40 | 623 |
| 59 | gpu-compute: bricks + beam + incremental | 217.0 | 0.45 | 8.53 | 4.43 | 0.08 | 62.2 | 0.01 | 530 | 506 |
| 60 | gpu-compute: svo_dag + rowrun | 217.0 | 0.45 | 10.65 | 3.15 | 2.25 | 62.0 | 0.01 | 40 | 623 |
| 61 | gpu-compute: tree64 + rowrun | 216.3 | 0.45 | 6.69 | 4.28 | 1.27 | 62.1 | 0.01 | 99 | 532 |
| 62 | gpu-compute: hier + incremental | 214.9 | 0.44 | 10.82 | 4.08 | 0.08 | 62.0 | 0.01 | 530 | 503 |
| 63 | gpu-compute: hier + beam + incremental | 207.4 | 0.43 | 11.35 | 4.36 | 0.09 | 62.2 | 0.01 | 530 | 504 |
| 64 | gpu-compute: rows + incremental | 202.1 | 0.42 | 11.73 | 4.22 | 0.09 | 62.0 | 0.01 | 562 | 504 |
| 65 | gpu-compute: tree64 + beam + rowrun | 196.9 | 0.41 | 7.15 | 4.84 | 1.28 | 62.2 | 0.01 | 99 | 532 |
| 66 | gpu-compute: rows + beam + incremental | 191.5 | 0.39 | 11.88 | 4.67 | 0.08 | 62.2 | 0.01 | 562 | 504 |
| 67 | gpu-compute: svo_dag + beam + rowrun | 191.4 | 0.39 | 11.19 | 3.78 | 2.41 | 62.2 | 0.01 | 40 | 625 |
| 68 | gpu-raster: bricks + cubes + raster | 164.7 | 0.34 | 9.31 | 5.41 | 1.42 | 56.7 | 0.02 | 668 | 527 |
| 69 | gpu-raster: bricks + cubes + visbuffer | 163.2 | 0.34 | 9.41 | 5.48 | 1.44 | 56.6 | 0.02 | 668 | 528 |
| 70 | gpu-raster: bricks + cubes + gbuffer | 162.5 | 0.33 | 9.50 | 5.47 | 1.45 | 56.6 | 0.02 | 668 | 527 |
| 71 | gpu-raster: bricks + greedy + gbuffer | 142.2 | 0.29 | 32.72 | 0.34 | 5.15 | 55.2 | 0.02 | 604 | 514 |
| 72 | gpu-raster: bricks + greedy + raster | 140.6 | 0.29 | 32.75 | 0.28 | 5.15 | 55.3 | 0.02 | 604 | 517 |
| 73 | gpu-raster: bricks + greedy + visbuffer | 139.1 | 0.29 | 34.31 | 0.31 | 5.19 | 55.3 | 0.02 | 604 | 515 |
| 74 | gpu-rt: greedy + rayquery | 83.5 | 0.17 | 81.81 | 1.43 | 9.45 | 62.0 | 0.01 | 373 | 790 |
| 75 | gpu-rt: naive + rayquery | 70.4 | 0.14 | 91.95 | 3.00 | 10.26 | 62.1 | 0.01 | 963 | 913 |
| 76 | gpu-discover: distance + quadtree | 48.3 | 0.10 | 115.72 | 1.93 | 0.15 | 55.3 | 0.02 | 968 | 515 |
| 77 | gpu-compute: distance + beam + sphere | 47.9 | 0.10 | 130.35 | 0.82 | 0.08 | 62.2 | 0.01 | 786 | 504 |
| 78 | gpu-compute: distance + sphere | 47.4 | 0.10 | 133.58 | 0.56 | 0.08 | 62.1 | 0.01 | 786 | 505 |
| 79 | gpu-compute: distance + refloor | 47.2 | 0.10 | 133.17 | 0.67 | 0.08 | 62.1 | 0.01 | 786 | 504 |
| 80 | gpu-compute: distance + rowrun | 47.1 | 0.10 | 128.42 | 0.95 | 0.08 | 62.1 | 0.01 | 786 | 504 |
| 81 | gpu-compute: distance + beam + refloor | 47.0 | 0.10 | 130.68 | 0.94 | 0.08 | 62.2 | 0.01 | 786 | 505 |
| 82 | gpu-compute: distance + beam + rowrun | 46.6 | 0.10 | 128.79 | 1.21 | 0.08 | 62.2 | 0.01 | 786 | 505 |
| 83 | cpu: hier + refloor | 44.4 | 0.09 | 36.25 | - | 21.26 | 66.4 | 0.00 | - | 338 |
| 84 | gpu-compute: distance + incremental | 42.4 | 0.09 | 86.73 | 6.34 | 0.09 | 62.1 | 0.01 | 786 | 504 |
| 85 | gpu-compute: distance + beam + incremental | 42.1 | 0.09 | 86.89 | 6.65 | 0.09 | 62.2 | 0.01 | 786 | 504 |
| 86 | cpu: prefix + incremental | 40.3 | 0.08 | 37.11 | - | 23.38 | 66.3 | 0.00 | - | 301 |
| 87 | cpu: bricks + refloor | 40.2 | 0.08 | 37.15 | - | 24.11 | 66.4 | 0.00 | - | 322 |
| 88 | cpu: hier + incremental | 39.9 | 0.08 | 37.75 | - | 23.78 | 66.3 | 0.00 | - | 338 |
| 89 | cpu: prefix + refloor | 38.6 | 0.08 | 41.20 | - | 24.43 | 66.4 | 0.00 | - | 294 |
| 90 | cpu: u8 + refloor | 38.4 | 0.08 | 38.25 | - | 25.07 | 66.4 | 0.00 | - | 190 |
| 91 | cpu: bits + refloor | 38.4 | 0.08 | 39.53 | - | 24.73 | 66.4 | 0.00 | - | 338 |
| 92 | cpu: morton + refloor | 37.8 | 0.08 | 40.75 | - | 24.85 | 66.4 | 0.00 | - | 328 |
| 93 | cpu: bricks + incremental | 37.8 | 0.08 | 38.21 | - | 25.61 | 66.3 | 0.00 | - | 322 |
| 94 | cpu: u8 + incremental | 37.5 | 0.08 | 39.00 | - | 25.70 | 66.3 | 0.00 | - | 190 |
| 95 | cpu: brickmajor + refloor | 37.4 | 0.08 | 41.85 | - | 25.07 | 66.4 | 0.00 | - | 328 |
| 96 | cpu: bits + incremental | 37.4 | 0.08 | 41.18 | - | 25.39 | 66.3 | 0.00 | - | 338 |
| 97 | cpu: morton + incremental | 36.6 | 0.08 | 40.85 | - | 25.70 | 66.3 | 0.00 | - | 328 |
| 98 | cpu: brickmajor + incremental | 36.5 | 0.08 | 40.81 | - | 25.69 | 66.3 | 0.00 | - | 328 |
| 99 | cpu: palette + refloor | 36.3 | 0.07 | 44.30 | - | 24.99 | 66.4 | 0.00 | - | 97 |
| 100 | cpu: palette + incremental | 35.4 | 0.07 | 43.78 | - | 25.81 | 66.3 | 0.00 | - | 101 |
| 101 | cpu-raster: greedy + raster | 33.6 | 0.07 | 49.14 | - | 29.31 | 70.8 | 0.00 | - | 573 |
| 102 | cpu-discover: hier + quadtree | 26.1 | 0.05 | 84.94 | 0.23 | 34.94 | 49.8 | 0.11 | 534 | 1119 |
| 103 | cpu-discover: prefix + quadtree | 25.6 | 0.05 | 84.82 | 0.22 | 35.10 | 49.8 | 0.11 | 534 | 1350 |
| 104 | cpu-discover: brickmajor + quadtree | 24.6 | 0.05 | 86.68 | 0.23 | 36.47 | 49.8 | 0.11 | 534 | 1409 |
| 105 | cpu-discover: u8 + quadtree | 24.5 | 0.05 | 90.86 | 0.23 | 37.75 | 49.8 | 0.11 | 534 | 1282 |
| 106 | cpu-discover: bricks + quadtree | 24.5 | 0.05 | 89.60 | 0.23 | 37.82 | 49.8 | 0.11 | 534 | 1138 |
| 107 | cpu-discover: morton + quadtree | 24.5 | 0.05 | 87.37 | 0.23 | 36.83 | 49.8 | 0.11 | 534 | 1390 |
| 108 | cpu-discover: bits + quadtree | 24.5 | 0.05 | 89.46 | 0.23 | 37.44 | 49.8 | 0.11 | 534 | 1145 |
| 109 | cpu-discover: palette + quadtree | 24.1 | 0.05 | 84.01 | 0.23 | 36.20 | 49.8 | 0.11 | 534 | 1187 |
| 110 | cpu-raster: naive + raster | 17.7 | 0.04 | 73.01 | - | 54.25 | 72.1 | 0.00 | - | 803 |
| 111 | cpu-discover: hier + cluster | 10.1 | 0.02 | 245.21 | 0.20 | 95.95 | 50.6 | 0.11 | 534 | 870 |
| 112 | cpu-discover: prefix + cluster | 10.0 | 0.02 | 249.67 | 0.18 | 96.17 | 50.6 | 0.11 | 534 | 1096 |
| 113 | cpu-discover: bricks + cluster | 9.5 | 0.02 | 265.41 | 0.18 | 102.75 | 50.6 | 0.11 | 534 | 854 |
| 114 | cpu-discover: brickmajor + cluster | 9.5 | 0.02 | 264.56 | 0.20 | 101.66 | 50.6 | 0.11 | 534 | 1116 |
| 115 | cpu-discover: u8 + cluster | 9.4 | 0.02 | 270.55 | 0.19 | 104.14 | 50.6 | 0.11 | 534 | 1008 |
| 116 | cpu-discover: morton + cluster | 9.4 | 0.02 | 267.60 | 0.20 | 103.47 | 50.6 | 0.11 | 534 | 1130 |
| 117 | cpu-discover: bits + cluster | 9.3 | 0.02 | 277.44 | 0.19 | 104.66 | 50.6 | 0.11 | 534 | 887 |
| 118 | cpu-discover: palette + cluster | 8.8 | 0.02 | 291.63 | 0.18 | 109.56 | 50.6 | 0.11 | 534 | 973 |
| 119 | gpu-voxelspace: heightmap + columns (incorrect) | 421.7 | 0.87 | 8.44 | 1.02 | 0.02 | 31.3 | 9.71 | 26 | 523 |
| 120 | cpu-voxelspace: heightmap + columns (incorrect) | 300.3 | 0.62 | 6.20 | - | 2.63 | 31.3 | 9.71 | - | 339 |
| 121 | gpu-splat: bricks + points + splat (incorrect) | 33.8 | 0.07 | 169.91 | 1.27 | 0.09 | 32.0 | 8.63 | 683 | 865 |

### terrain-flight [lit]

1920×1080. camera 2.25 blocks/frame (max 3.0), turn 0.35°/frame; 3985 chunks at start, 11.1 loads/frame; chunk fill 11%, solid 8.5%, surface 0.65%; sky 64%, hits at 268 blocks (median 235); 0.0 edits/frame; 400 lights (animated); 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: bricks + refloor | 574.2 | 1.00 | 7.82 | 1.22 | 0.74 | 57.8 | 0.01 | 531 | 508 |
| 2 | gpu-compute: prefix + refloor | 557.7 | 0.97 | 11.27 | 1.08 | 0.75 | 57.8 | 0.01 | 571 | 508 |
| 3 | gpu-compute: hier + refloor | 532.6 | 0.93 | 11.01 | 1.13 | 0.77 | 57.8 | 0.01 | 531 | 509 |
| 4 | gpu-compute: bricks + beam + refloor | 525.3 | 0.91 | 7.42 | 1.43 | 0.75 | 57.7 | 0.01 | 531 | 508 |
| 5 | gpu-compute: prefix + beam + refloor | 508.1 | 0.88 | 10.29 | 1.29 | 0.76 | 57.8 | 0.01 | 571 | 508 |
| 6 | gpu-compute: rows + refloor | 501.8 | 0.87 | 10.00 | 1.24 | 0.74 | 57.8 | 0.01 | 563 | 508 |
| 7 | gpu-compute: tree64 + refloor | 495.7 | 0.86 | 8.49 | 1.50 | 1.15 | 57.8 | 0.01 | 94 | 597 |
| 8 | gpu-compute: prefix + rowrun | 490.5 | 0.85 | 8.76 | 1.38 | 0.77 | 57.8 | 0.01 | 571 | 508 |
| 9 | gpu-compute: hier + beam + refloor | 482.9 | 0.84 | 11.58 | 1.33 | 0.80 | 57.7 | 0.01 | 531 | 509 |
| 10 | gpu-compute: rows + beam + refloor | 461.0 | 0.80 | 9.51 | 1.45 | 0.77 | 57.7 | 0.01 | 563 | 506 |
| 11 | gpu-compute: rows + rowrun | 458.5 | 0.80 | 9.72 | 1.46 | 0.78 | 57.8 | 0.01 | 563 | 506 |
| 12 | gpu-compute: prefix + beam + rowrun | 452.9 | 0.79 | 8.80 | 1.59 | 0.77 | 57.8 | 0.01 | 571 | 506 |
| 13 | gpu-compute: svo_dag + refloor | 448.6 | 0.78 | 9.33 | 1.39 | 1.46 | 57.8 | 0.01 | 30 | 617 |
| 14 | gpu-compute: rows + beam + rowrun | 422.3 | 0.74 | 9.86 | 1.69 | 0.77 | 57.7 | 0.01 | 563 | 506 |
| 15 | gpu-compute: bricks + rowrun | 419.4 | 0.73 | 6.64 | 1.98 | 0.85 | 57.8 | 0.01 | 531 | 508 |
| 16 | gpu-compute: clipmap + refloor | 415.6 | 0.72 | 7.37 | 1.93 | 0.83 | 57.8 | 0.01 | 571 | 531 |
| 17 | gpu-raster: bricks + naive + visbuffer | 412.8 | 0.72 | 16.87 | 1.09 | 1.55 | 51.9 | 0.01 | 668 | 629 |
| 18 | gpu-raster: bricks + naive + gbuffer | 410.2 | 0.71 | 17.68 | 1.06 | 1.57 | 51.9 | 0.02 | 668 | 627 |
| 19 | gpu-raster: tree64 + naive + visbuffer | 403.0 | 0.70 | 16.83 | 1.18 | 1.88 | 51.9 | 0.01 | 232 | 636 |
| 20 | gpu-compute: bricks + beam + rowrun | 402.6 | 0.70 | 6.99 | 2.10 | 0.85 | 57.7 | 0.01 | 531 | 508 |
| 21 | gpu-rt: boxes + rayquery | 397.6 | 0.69 | 12.31 | 1.44 | 1.35 | 57.6 | 0.01 | 561 | 809 |
| 22 | gpu-raster: tree64 + naive + gbuffer | 397.3 | 0.69 | 17.48 | 1.19 | 1.90 | 51.9 | 0.02 | 232 | 633 |
| 23 | gpu-raster: prefix + naive + gbuffer | 395.1 | 0.69 | 20.41 | 1.02 | 1.53 | 51.9 | 0.02 | 708 | 628 |
| 24 | gpu-raster: prefix + naive + visbuffer | 393.4 | 0.69 | 21.08 | 1.02 | 1.54 | 51.9 | 0.01 | 708 | 628 |
| 25 | gpu-raster: clipmap + naive + visbuffer | 390.3 | 0.68 | 18.65 | 1.22 | 1.51 | 51.9 | 0.01 | 709 | 663 |
| 26 | gpu-raster: clipmap + naive + gbuffer | 390.2 | 0.68 | 16.49 | 1.22 | 1.53 | 51.9 | 0.02 | 709 | 663 |
| 27 | gpu-raster: hier + naive + visbuffer | 389.9 | 0.68 | 20.91 | 1.06 | 1.52 | 51.9 | 0.01 | 668 | 624 |
| 28 | gpu-raster: hier + naive + gbuffer | 388.6 | 0.68 | 20.57 | 1.06 | 1.53 | 51.9 | 0.02 | 668 | 629 |
| 29 | gpu-raster: rows + naive + gbuffer | 379.2 | 0.66 | 21.35 | 1.07 | 1.53 | 51.9 | 0.02 | 700 | 629 |
| 30 | gpu-raster: rows + naive + visbuffer | 377.9 | 0.66 | 21.72 | 1.06 | 1.55 | 51.9 | 0.01 | 700 | 629 |
| 31 | gpu-compute: tree64 + incremental | 367.0 | 0.64 | 8.24 | 2.34 | 1.27 | 57.7 | 0.01 | 94 | 599 |
| 32 | gpu-compute: clipmap + rowrun | 362.6 | 0.63 | 7.09 | 2.41 | 0.85 | 57.8 | 0.01 | 571 | 529 |
| 33 | gpu-compute: tree64 + beam + refloor | 357.2 | 0.62 | 7.84 | 2.38 | 1.25 | 57.7 | 0.01 | 94 | 600 |
| 34 | gpu-raster: svo_dag + naive + gbuffer | 355.5 | 0.62 | 17.52 | 1.18 | 2.23 | 51.9 | 0.02 | 167 | 690 |
| 35 | gpu-raster: svo_dag + naive + visbuffer | 355.2 | 0.62 | 17.83 | 1.16 | 2.22 | 51.9 | 0.01 | 167 | 690 |
| 36 | gpu-compute: hier + rowrun | 348.7 | 0.61 | 9.13 | 2.35 | 0.89 | 57.8 | 0.01 | 531 | 509 |
| 37 | gpu-compute: clipmap + incremental | 345.4 | 0.60 | 7.34 | 2.54 | 0.93 | 57.7 | 0.01 | 571 | 531 |
| 38 | gpu-compute: clipmap + beam + refloor | 328.9 | 0.57 | 7.31 | 2.72 | 0.94 | 57.7 | 0.01 | 571 | 528 |
| 39 | gpu-compute: hier + sphere | 328.7 | 0.57 | 9.43 | 2.51 | 0.91 | 57.7 | 0.01 | 531 | 511 |
| 40 | gpu-compute: bricks + sphere | 327.4 | 0.57 | 7.17 | 2.79 | 0.96 | 57.7 | 0.01 | 531 | 508 |
| 41 | gpu-compute: hier + beam + rowrun | 324.5 | 0.57 | 9.16 | 2.61 | 0.95 | 57.7 | 0.01 | 531 | 509 |
| 42 | gpu-compute: svo_dag + beam + refloor | 321.1 | 0.56 | 8.23 | 2.44 | 1.60 | 57.8 | 0.01 | 30 | 617 |
| 43 | gpu-raster: bricks + greedy + visbuffer | 319.6 | 0.56 | 28.32 | 0.69 | 2.50 | 50.0 | 0.02 | 604 | 560 |
| 44 | gpu-raster: bricks + naive + raster | 319.6 | 0.56 | 14.76 | 1.99 | 1.63 | 52.0 | 0.01 | 668 | 627 |
| 45 | gpu-compute: hier + beam + sphere | 317.2 | 0.55 | 9.03 | 2.71 | 0.96 | 57.7 | 0.01 | 531 | 509 |
| 46 | gpu-raster: bricks + greedy + gbuffer | 316.4 | 0.55 | 28.75 | 0.71 | 2.50 | 50.0 | 0.02 | 604 | 561 |
| 47 | gpu-raster: tree64 + greedy + gbuffer | 311.9 | 0.54 | 29.04 | 0.84 | 2.83 | 50.0 | 0.02 | 168 | 603 |
| 48 | gpu-compute: bricks + beam + sphere | 311.8 | 0.54 | 7.19 | 2.98 | 0.96 | 57.7 | 0.01 | 531 | 508 |
| 49 | gpu-compute: prefix + sphere | 309.9 | 0.54 | 9.09 | 2.81 | 0.97 | 57.7 | 0.01 | 571 | 508 |
| 50 | gpu-raster: tree64 + greedy + visbuffer | 309.8 | 0.54 | 28.76 | 0.81 | 2.86 | 50.0 | 0.02 | 168 | 606 |
| 51 | gpu-raster: clipmap + greedy + visbuffer | 308.5 | 0.54 | 30.03 | 0.84 | 2.46 | 50.0 | 0.02 | 645 | 597 |
| 52 | gpu-raster: prefix + naive + raster | 307.7 | 0.54 | 16.87 | 1.90 | 1.60 | 52.0 | 0.01 | 708 | 629 |
| 53 | gpu-raster: prefix + greedy + visbuffer | 306.0 | 0.53 | 31.30 | 0.64 | 2.47 | 50.0 | 0.02 | 644 | 563 |
| 54 | gpu-raster: clipmap + greedy + gbuffer | 305.3 | 0.53 | 29.31 | 0.88 | 2.47 | 50.0 | 0.02 | 645 | 597 |
| 55 | gpu-compute: rows + sphere | 305.1 | 0.53 | 9.80 | 2.78 | 0.99 | 57.7 | 0.01 | 563 | 506 |
| 56 | gpu-raster: prefix + greedy + gbuffer | 302.3 | 0.53 | 31.91 | 0.68 | 2.49 | 50.0 | 0.02 | 644 | 563 |
| 57 | gpu-raster: hier + greedy + visbuffer | 301.9 | 0.53 | 32.37 | 0.68 | 2.49 | 50.0 | 0.02 | 604 | 564 |
| 58 | gpu-raster: hier + greedy + gbuffer | 300.9 | 0.52 | 31.23 | 0.72 | 2.48 | 50.0 | 0.02 | 604 | 564 |
| 59 | gpu-raster: hier + naive + raster | 300.6 | 0.52 | 15.91 | 2.03 | 1.62 | 52.0 | 0.01 | 668 | 629 |
| 60 | gpu-raster: rows + greedy + visbuffer | 299.7 | 0.52 | 32.45 | 0.69 | 2.47 | 50.0 | 0.02 | 636 | 563 |
| 61 | gpu-raster: rows + greedy + gbuffer | 297.7 | 0.52 | 32.27 | 0.76 | 2.45 | 50.0 | 0.02 | 636 | 563 |
| 62 | gpu-raster: rows + naive + raster | 296.7 | 0.52 | 16.68 | 2.00 | 1.62 | 52.0 | 0.01 | 700 | 628 |
| 63 | gpu-compute: prefix + beam + sphere | 295.0 | 0.51 | 9.08 | 3.01 | 0.99 | 57.7 | 0.01 | 571 | 508 |
| 64 | gpu-raster: bricks + greedy + raster | 293.2 | 0.51 | 28.69 | 1.11 | 2.53 | 50.0 | 0.02 | 604 | 560 |
| 65 | gpu-compute: rows + beam + sphere | 291.5 | 0.51 | 9.73 | 2.97 | 0.98 | 57.7 | 0.01 | 563 | 506 |
| 66 | gpu-compute: clipmap + beam + rowrun | 290.5 | 0.51 | 7.44 | 3.19 | 0.96 | 57.7 | 0.01 | 571 | 531 |
| 67 | gpu-compute: tree64 + beam + incremental | 288.4 | 0.50 | 7.05 | 3.19 | 1.32 | 57.7 | 0.01 | 94 | 597 |
| 68 | gpu-raster: svo_dag + greedy + visbuffer | 286.4 | 0.50 | 30.17 | 0.78 | 3.15 | 50.0 | 0.02 | 103 | 626 |
| 69 | gpu-raster: prefix + greedy + raster | 284.7 | 0.50 | 31.40 | 1.06 | 2.50 | 50.0 | 0.02 | 644 | 562 |
| 70 | gpu-raster: clipmap + naive + raster | 283.5 | 0.49 | 14.93 | 2.40 | 1.66 | 52.0 | 0.01 | 709 | 661 |
| 71 | gpu-raster: hier + greedy + raster | 278.7 | 0.49 | 31.63 | 1.13 | 2.49 | 50.0 | 0.02 | 604 | 563 |
| 72 | gpu-raster: rows + greedy + raster | 278.0 | 0.48 | 32.26 | 1.12 | 2.47 | 50.0 | 0.02 | 636 | 562 |
| 73 | gpu-compute: clipmap + beam + incremental | 277.0 | 0.48 | 8.26 | 3.37 | 1.01 | 57.7 | 0.01 | 571 | 529 |
| 74 | gpu-raster: svo_dag + greedy + gbuffer | 274.9 | 0.48 | 31.54 | 0.83 | 3.26 | 50.0 | 0.02 | 103 | 628 |
| 75 | gpu-raster: clipmap + greedy + raster | 274.0 | 0.48 | 26.74 | 1.40 | 2.47 | 50.0 | 0.02 | 645 | 595 |
| 76 | gpu-raster: tree64 + naive + raster | 267.0 | 0.46 | 15.63 | 2.71 | 2.06 | 52.0 | 0.01 | 232 | 631 |
| 77 | gpu-raster: tree64 + greedy + raster | 266.2 | 0.46 | 28.37 | 1.59 | 2.87 | 50.0 | 0.02 | 168 | 604 |
| 78 | gpu-compute: svo_dag + sphere | 254.6 | 0.44 | 8.04 | 3.35 | 1.69 | 57.7 | 0.01 | 30 | 616 |
| 79 | gpu-raster: svo_dag + naive + raster | 252.9 | 0.44 | 16.78 | 2.58 | 2.39 | 52.0 | 0.01 | 167 | 690 |
| 80 | gpu-raster: svo_dag + greedy + raster | 251.1 | 0.44 | 29.66 | 1.47 | 3.18 | 50.0 | 0.02 | 103 | 627 |
| 81 | gpu-discover: prefix + quadtree | 250.5 | 0.44 | 9.83 | 3.65 | 1.03 | 50.9 | 0.01 | 753 | 512 |
| 82 | gpu-compute: clipmap + sphere | 250.4 | 0.44 | 8.35 | 3.80 | 1.04 | 57.7 | 0.01 | 571 | 529 |
| 83 | gpu-discover: bricks + quadtree | 243.5 | 0.42 | 8.13 | 3.94 | 1.07 | 50.8 | 0.01 | 713 | 525 |
| 84 | gpu-discover: hier + quadtree | 236.5 | 0.41 | 10.01 | 3.92 | 1.08 | 50.8 | 0.01 | 713 | 514 |
| 85 | gpu-discover: rows + quadtree | 233.5 | 0.41 | 10.27 | 3.93 | 1.01 | 50.8 | 0.01 | 745 | 515 |
| 86 | gpu-compute: svo_dag + rowrun | 230.3 | 0.40 | 7.73 | 3.83 | 1.71 | 57.8 | 0.01 | 30 | 616 |
| 87 | gpu-compute: tree64 + sphere | 218.4 | 0.38 | 7.60 | 4.38 | 1.42 | 57.7 | 0.01 | 94 | 599 |
| 88 | gpu-compute: clipmap + beam + sphere | 210.9 | 0.37 | 8.22 | 4.63 | 1.08 | 57.7 | 0.01 | 571 | 529 |
| 89 | gpu-compute: svo_dag + incremental | 209.4 | 0.36 | 8.10 | 4.30 | 1.73 | 57.7 | 0.01 | 30 | 617 |
| 90 | gpu-compute: svo_dag + beam + sphere | 206.6 | 0.36 | 8.03 | 4.36 | 1.77 | 57.7 | 0.01 | 30 | 616 |
| 91 | gpu-rt: greedy + rayquery | 196.6 | 0.34 | 45.75 | 0.86 | 3.92 | 57.7 | 0.01 | 350 | 1117 |
| 92 | gpu-compute: tree64 + rowrun | 196.3 | 0.34 | 7.97 | 4.94 | 1.45 | 57.8 | 0.01 | 94 | 596 |
| 93 | gpu-discover: clipmap + quadtree | 194.6 | 0.34 | 8.39 | 5.00 | 1.11 | 50.8 | 0.01 | 754 | 537 |
| 94 | gpu-compute: svo_dag + beam + rowrun | 190.2 | 0.33 | 8.27 | 4.82 | 1.78 | 57.8 | 0.01 | 30 | 617 |
| 95 | gpu-discover: tree64 + quadtree | 187.5 | 0.33 | 8.06 | 5.13 | 1.47 | 50.8 | 0.01 | 276 | 611 |
| 96 | gpu-compute: tree64 + beam + sphere | 183.5 | 0.32 | 9.18 | 5.30 | 1.41 | 57.7 | 0.01 | 94 | 597 |
| 97 | gpu-discover: svo_dag + quadtree | 179.1 | 0.31 | 8.44 | 5.12 | 1.83 | 50.9 | 0.01 | 212 | 625 |
| 98 | gpu-rt: naive + rayquery | 178.4 | 0.31 | 51.66 | 1.37 | 3.61 | 57.7 | 0.01 | 912 | 1559 |
| 99 | gpu-compute: svo_dag + beam + incremental | 172.8 | 0.30 | 8.77 | 5.39 | 1.83 | 57.7 | 0.01 | 30 | 617 |
| 100 | gpu-compute: prefix + incremental | 169.5 | 0.30 | 10.04 | 5.78 | 1.11 | 57.7 | 0.01 | 571 | 508 |
| 101 | gpu-compute: tree64 + beam + rowrun | 167.6 | 0.29 | 8.96 | 5.84 | 1.49 | 57.7 | 0.01 | 94 | 600 |
| 102 | gpu-compute: hier + incremental | 166.4 | 0.29 | 10.38 | 5.90 | 1.08 | 57.7 | 0.01 | 531 | 511 |
| 103 | gpu-raster: bricks + cubes + visbuffer | 166.1 | 0.29 | 10.19 | 5.77 | 1.59 | 51.9 | 0.02 | 668 | 592 |
| 104 | gpu-raster: bricks + cubes + gbuffer | 165.6 | 0.29 | 9.77 | 5.76 | 1.61 | 51.8 | 0.02 | 668 | 592 |
| 105 | gpu-compute: prefix + beam + incremental | 164.7 | 0.29 | 9.93 | 5.97 | 1.11 | 57.7 | 0.01 | 571 | 508 |
| 106 | gpu-raster: prefix + cubes + visbuffer | 163.4 | 0.28 | 10.86 | 5.72 | 1.59 | 51.9 | 0.02 | 708 | 597 |
| 107 | gpu-raster: prefix + cubes + gbuffer | 163.2 | 0.28 | 10.93 | 5.72 | 1.59 | 51.8 | 0.02 | 708 | 596 |
| 108 | gpu-raster: hier + cubes + visbuffer | 162.4 | 0.28 | 11.04 | 5.76 | 1.54 | 51.9 | 0.02 | 668 | 597 |
| 109 | gpu-raster: hier + cubes + gbuffer | 161.4 | 0.28 | 11.17 | 5.76 | 1.60 | 51.8 | 0.02 | 668 | 597 |
| 110 | gpu-raster: clipmap + cubes + visbuffer | 161.4 | 0.28 | 9.95 | 5.91 | 1.60 | 51.9 | 0.02 | 709 | 629 |
| 111 | gpu-compute: bricks + incremental | 161.0 | 0.28 | 8.78 | 6.13 | 1.10 | 57.7 | 0.01 | 531 | 507 |
| 112 | gpu-raster: clipmap + cubes + gbuffer | 160.9 | 0.28 | 9.85 | 5.92 | 1.61 | 51.8 | 0.02 | 709 | 630 |
| 113 | gpu-raster: rows + cubes + visbuffer | 160.8 | 0.28 | 11.75 | 5.76 | 1.61 | 51.9 | 0.02 | 700 | 597 |
| 114 | gpu-raster: rows + cubes + gbuffer | 160.8 | 0.28 | 12.57 | 5.76 | 1.60 | 51.8 | 0.02 | 700 | 597 |
| 115 | gpu-raster: tree64 + cubes + visbuffer | 158.1 | 0.28 | 12.10 | 5.87 | 1.98 | 51.9 | 0.02 | 232 | 614 |
| 116 | gpu-raster: tree64 + cubes + gbuffer | 158.1 | 0.28 | 12.06 | 5.88 | 1.93 | 51.8 | 0.02 | 232 | 610 |
| 117 | gpu-compute: hier + beam + incremental | 157.6 | 0.27 | 10.26 | 6.24 | 1.13 | 57.7 | 0.01 | 531 | 511 |
| 118 | gpu-compute: bricks + beam + incremental | 157.2 | 0.27 | 9.14 | 6.29 | 1.12 | 57.7 | 0.01 | 531 | 508 |
| 119 | gpu-compute: rows + incremental | 154.7 | 0.27 | 11.26 | 6.35 | 1.12 | 57.7 | 0.01 | 563 | 508 |
| 120 | gpu-raster: svo_dag + cubes + visbuffer | 150.8 | 0.26 | 13.46 | 5.86 | 2.28 | 51.9 | 0.02 | 167 | 660 |
| 121 | gpu-raster: svo_dag + cubes + gbuffer | 150.1 | 0.26 | 13.24 | 5.88 | 2.28 | 51.8 | 0.02 | 167 | 658 |
| 122 | gpu-compute: rows + beam + incremental | 143.0 | 0.25 | 11.07 | 6.89 | 1.11 | 57.7 | 0.01 | 563 | 508 |
| 123 | gpu-compute: distance + refloor | 137.2 | 0.24 | 87.75 | 1.04 | 0.82 | 57.7 | 0.01 | 787 | 508 |
| 124 | gpu-compute: distance + sphere | 136.4 | 0.24 | 86.65 | 0.89 | 0.87 | 57.7 | 0.01 | 787 | 509 |
| 125 | gpu-compute: distance + beam + sphere | 135.0 | 0.24 | 83.69 | 1.11 | 0.86 | 57.7 | 0.01 | 787 | 508 |
| 126 | gpu-compute: distance + beam + refloor | 133.8 | 0.23 | 120.79 | 1.27 | 0.82 | 57.7 | 0.01 | 787 | 508 |
| 127 | gpu-compute: distance + rowrun | 133.0 | 0.23 | 75.55 | 1.37 | 0.80 | 57.7 | 0.01 | 787 | 506 |
| 128 | gpu-compute: distance + beam + rowrun | 130.6 | 0.23 | 77.64 | 1.59 | 0.82 | 57.7 | 0.01 | 787 | 507 |
| 129 | gpu-raster: distance + naive + visbuffer | 126.7 | 0.22 | 93.58 | 1.07 | 1.53 | 51.9 | 0.01 | 924 | 625 |
| 130 | gpu-raster: distance + naive + gbuffer | 126.0 | 0.22 | 126.57 | 1.06 | 1.54 | 51.9 | 0.02 | 924 | 630 |
| 131 | gpu-raster: distance + greedy + gbuffer | 115.6 | 0.20 | 138.62 | 0.72 | 2.48 | 50.0 | 0.02 | 860 | 564 |
| 132 | gpu-raster: distance + naive + raster | 113.2 | 0.20 | 78.44 | 2.13 | 1.70 | 52.0 | 0.01 | 924 | 630 |
| 133 | gpu-raster: distance + greedy + visbuffer | 112.5 | 0.20 | 141.68 | 0.73 | 2.56 | 50.0 | 0.02 | 860 | 565 |
| 134 | gpu-raster: distance + greedy + raster | 106.4 | 0.19 | 98.34 | 1.13 | 2.66 | 50.0 | 0.02 | 860 | 564 |
| 135 | gpu-discover: distance + quadtree | 106.3 | 0.19 | 77.13 | 3.73 | 1.10 | 50.8 | 0.01 | 969 | 513 |
| 136 | gpu-raster: prefix + cubes + raster | 90.8 | 0.16 | 14.51 | 10.87 | 1.70 | 51.8 | 0.01 | 708 | 597 |
| 137 | gpu-raster: bricks + cubes + raster | 90.6 | 0.16 | 14.48 | 10.92 | 1.62 | 51.8 | 0.01 | 668 | 592 |
| 138 | gpu-raster: rows + cubes + raster | 90.5 | 0.16 | 14.36 | 10.89 | 1.59 | 51.8 | 0.01 | 700 | 597 |
| 139 | gpu-raster: hier + cubes + raster | 90.3 | 0.16 | 14.47 | 10.93 | 1.66 | 51.8 | 0.01 | 668 | 597 |
| 140 | gpu-raster: clipmap + cubes + raster | 88.8 | 0.15 | 14.98 | 11.12 | 1.68 | 51.8 | 0.01 | 709 | 630 |
| 141 | gpu-raster: distance + cubes + gbuffer | 88.1 | 0.15 | 75.72 | 5.71 | 1.60 | 51.8 | 0.02 | 924 | 597 |
| 142 | gpu-raster: distance + cubes + visbuffer | 87.9 | 0.15 | 77.78 | 5.70 | 1.60 | 51.9 | 0.02 | 924 | 597 |
| 143 | gpu-raster: tree64 + cubes + raster | 87.7 | 0.15 | 14.59 | 11.25 | 2.03 | 51.8 | 0.01 | 232 | 612 |
| 144 | gpu-raster: svo_dag + cubes + raster | 86.3 | 0.15 | 14.79 | 11.21 | 2.30 | 51.8 | 0.01 | 167 | 659 |
| 145 | gpu-compute: distance + incremental | 73.4 | 0.13 | 78.24 | 8.91 | 1.14 | 57.7 | 0.01 | 787 | 508 |
| 146 | gpu-compute: distance + beam + incremental | 71.6 | 0.12 | 78.59 | 9.29 | 1.10 | 57.7 | 0.01 | 787 | 508 |
| 147 | gpu-raster: distance + cubes + raster | 64.0 | 0.11 | 77.31 | 10.84 | 1.61 | 51.8 | 0.01 | 924 | 597 |
| 148 | cpu: hier + refloor | 30.5 | 0.05 | 48.88 | - | 32.37 | 59.1 | 0.00 | - | 337 |
| 149 | cpu: hier + incremental | 27.8 | 0.05 | 54.93 | - | 35.53 | 59.1 | 0.00 | - | 337 |
| 150 | cpu: prefix + incremental | 27.7 | 0.05 | 55.58 | - | 35.62 | 59.0 | 0.00 | - | 407 |
| 151 | cpu: bricks + refloor | 27.3 | 0.05 | 51.92 | - | 36.36 | 59.1 | 0.00 | - | 321 |
| 152 | cpu: prefix + refloor | 26.9 | 0.05 | 56.63 | - | 36.68 | 59.1 | 0.00 | - | 406 |
| 153 | cpu: bits + refloor | 26.9 | 0.05 | 53.52 | - | 36.79 | 59.1 | 0.00 | - | 337 |
| 154 | cpu: u8 + refloor | 26.9 | 0.05 | 53.01 | - | 36.91 | 59.1 | 0.00 | - | 261 |
| 155 | cpu: morton + refloor | 26.7 | 0.05 | 54.17 | - | 36.92 | 59.1 | 0.00 | - | 439 |
| 156 | cpu: brickmajor + refloor | 26.4 | 0.05 | 55.94 | - | 37.36 | 59.1 | 0.00 | - | 432 |
| 157 | cpu: bricks + incremental | 26.2 | 0.05 | 54.75 | - | 37.94 | 59.1 | 0.00 | - | 321 |
| 158 | cpu: palette + refloor | 26.1 | 0.05 | 57.54 | - | 37.52 | 59.1 | 0.00 | - | 251 |
| 159 | cpu: morton + incremental | 25.8 | 0.04 | 56.91 | - | 38.20 | 59.1 | 0.00 | - | 446 |
| 160 | cpu: bits + incremental | 25.5 | 0.04 | 57.13 | - | 38.73 | 59.1 | 0.00 | - | 337 |
| 161 | cpu-raster: greedy + raster | 25.3 | 0.04 | 69.74 | - | 38.69 | 64.1 | 0.00 | - | 619 |
| 162 | cpu: palette + incremental | 25.2 | 0.04 | 60.95 | - | 38.79 | 59.1 | 0.00 | - | 249 |
| 163 | cpu: brickmajor + incremental | 24.8 | 0.04 | 60.60 | - | 39.74 | 59.1 | 0.00 | - | 446 |
| 164 | cpu: u8 + incremental | 24.4 | 0.04 | 59.36 | - | 40.62 | 59.1 | 0.00 | - | 257 |
| 165 | cpu-raster: naive + raster | 14.7 | 0.03 | 94.73 | - | 65.27 | 64.7 | 0.00 | - | 833 |
| 166 | cpu-discover: hier + cluster | 11.2 | 0.02 | 210.07 | 2.47 | 87.79 | 37.1 | 0.18 | 535 | 880 |
| 167 | cpu-discover: prefix + cluster | 11.2 | 0.02 | 209.89 | 2.47 | 88.01 | 37.1 | 0.18 | 535 | 1110 |
| 168 | cpu-discover: u8 + cluster | 10.7 | 0.02 | 219.22 | 2.47 | 92.40 | 37.1 | 0.18 | 535 | 991 |
| 169 | cpu-discover: bits + cluster | 10.6 | 0.02 | 217.28 | 2.47 | 93.11 | 37.1 | 0.18 | 535 | 867 |
| 170 | cpu-discover: morton + cluster | 10.6 | 0.02 | 223.40 | 2.46 | 93.48 | 37.1 | 0.18 | 535 | 1137 |
| 171 | cpu-discover: brickmajor + cluster | 10.5 | 0.02 | 225.96 | 2.47 | 93.70 | 37.1 | 0.18 | 535 | 1127 |
| 172 | cpu-discover: bricks + cluster | 10.4 | 0.02 | 226.49 | 2.48 | 95.14 | 37.1 | 0.18 | 535 | 892 |
| 173 | cpu-discover: palette + cluster | 10.4 | 0.02 | 217.49 | 2.47 | 94.61 | 37.1 | 0.18 | 535 | 877 |
| 174 | gpu-voxelspace: heightmap + columns (incorrect) | 331.4 | 0.58 | 6.88 | 2.79 | 0.96 | 17.7 | 24.68 | 27 | 574 |
| 175 | gpu-compute: prefix + refloor + pathtrace (incorrect) | 288.7 | 0.50 | 9.10 | 3.10 | 0.99 | 31.7 | 8.00 | 603 | 509 |
| 176 | gpu-compute: prefix + beam + refloor + pathtrace (incorrect) | 281.3 | 0.49 | 9.49 | 3.22 | 0.98 | 31.7 | 8.00 | 603 | 508 |
| 177 | gpu-compute: hier + refloor + pathtrace (incorrect) | 277.1 | 0.48 | 9.87 | 3.23 | 0.98 | 31.7 | 8.00 | 563 | 509 |
| 178 | gpu-compute: bricks + refloor + pathtrace (incorrect) | 269.0 | 0.47 | 7.46 | 3.53 | 1.03 | 31.7 | 8.00 | 563 | 508 |
| 179 | gpu-compute: hier + beam + refloor + pathtrace (incorrect) | 268.6 | 0.47 | 9.94 | 3.37 | 0.97 | 31.7 | 8.00 | 563 | 509 |
| 180 | gpu-compute: bricks + beam + refloor + pathtrace (incorrect) | 263.1 | 0.46 | 7.60 | 3.64 | 1.03 | 31.7 | 8.00 | 563 | 508 |
| 181 | gpu-compute: rows + refloor + pathtrace (incorrect) | 254.0 | 0.44 | 10.45 | 3.57 | 1.01 | 31.7 | 8.00 | 595 | 506 |
| 182 | gpu-compute: rows + beam + refloor + pathtrace (incorrect) | 248.8 | 0.43 | 10.70 | 3.64 | 1.03 | 31.7 | 8.00 | 595 | 506 |
| 183 | gpu-compute: prefix + rowrun + pathtrace (incorrect) | 229.9 | 0.40 | 9.72 | 4.13 | 0.99 | 31.7 | 8.00 | 603 | 508 |
| 184 | gpu-compute: prefix + beam + rowrun + pathtrace (incorrect) | 215.7 | 0.38 | 9.57 | 4.44 | 1.06 | 31.7 | 8.00 | 603 | 506 |
| 185 | gpu-compute: tree64 + refloor + pathtrace (incorrect) | 205.0 | 0.36 | 7.59 | 4.69 | 1.43 | 31.7 | 8.00 | 126 | 597 |
| 186 | gpu-compute: svo_dag + refloor + pathtrace (incorrect) | 203.6 | 0.35 | 8.30 | 4.46 | 1.74 | 31.7 | 8.00 | 61 | 617 |
| 187 | cpu-voxelspace: heightmap + columns (incorrect) | 202.4 | 0.35 | 7.50 | - | 4.71 | 17.6 | 24.69 | - | 383 |
| 188 | gpu-compute: rows + rowrun + pathtrace (incorrect) | 199.9 | 0.35 | 10.72 | 4.78 | 1.05 | 31.7 | 8.00 | 595 | 508 |
| 189 | gpu-compute: rows + beam + rowrun + pathtrace (incorrect) | 195.0 | 0.34 | 10.86 | 4.94 | 1.04 | 31.7 | 8.00 | 595 | 508 |
| 190 | gpu-rt: greedy + rayquery + pathtrace (incorrect) | 193.0 | 0.34 | 46.30 | 1.38 | 3.95 | 31.7 | 8.00 | 381 | 1126 |
| 191 | gpu-rt: boxes + rayquery + pathtrace (incorrect) | 187.7 | 0.33 | 9.62 | 4.99 | 1.61 | 31.7 | 8.00 | 593 | 813 |
| 192 | gpu-compute: clipmap + refloor + pathtrace (incorrect) | 187.3 | 0.33 | 9.00 | 5.20 | 1.09 | 31.7 | 8.00 | 603 | 529 |
| 193 | gpu-compute: tree64 + beam + refloor + pathtrace (incorrect) | 182.6 | 0.32 | 10.07 | 5.31 | 1.44 | 31.7 | 8.00 | 126 | 596 |
| 194 | gpu-rt: naive + rayquery + pathtrace (incorrect) | 178.0 | 0.31 | 50.78 | 1.94 | 3.59 | 31.7 | 8.00 | 943 | 1562 |
| 195 | gpu-compute: svo_dag + beam + refloor + pathtrace (incorrect) | 173.7 | 0.30 | 8.99 | 5.33 | 1.81 | 31.7 | 8.00 | 61 | 618 |
| 196 | gpu-compute: clipmap + beam + refloor + pathtrace (incorrect) | 163.7 | 0.29 | 8.99 | 6.01 | 1.10 | 31.7 | 8.00 | 603 | 531 |
| 197 | gpu-compute: bricks + rowrun + pathtrace (incorrect) | 159.1 | 0.28 | 9.49 | 6.18 | 1.03 | 31.7 | 8.00 | 563 | 507 |
| 198 | gpu-compute: bricks + beam + rowrun + pathtrace (incorrect) | 149.1 | 0.26 | 10.24 | 6.62 | 1.11 | 31.7 | 8.00 | 563 | 508 |
| 199 | gpu-compute: clipmap + incremental + pathtrace (incorrect) | 131.8 | 0.23 | 11.64 | 7.48 | 1.17 | 31.7 | 8.00 | 603 | 529 |
| 200 | gpu-compute: tree64 + incremental + pathtrace (incorrect) | 131.4 | 0.23 | 11.53 | 7.51 | 1.50 | 31.7 | 8.00 | 126 | 600 |
| 201 | gpu-compute: clipmap + rowrun + pathtrace (incorrect) | 131.0 | 0.23 | 11.51 | 7.53 | 1.10 | 31.7 | 8.00 | 603 | 529 |
| 202 | gpu-compute: hier + sphere + pathtrace (incorrect) | 129.1 | 0.22 | 11.29 | 7.64 | 1.11 | 31.7 | 8.00 | 563 | 511 |
| 203 | gpu-compute: hier + beam + sphere + pathtrace (incorrect) | 127.7 | 0.22 | 11.28 | 7.73 | 1.12 | 31.7 | 8.00 | 563 | 511 |
| 204 | gpu-compute: hier + rowrun + pathtrace (incorrect) | 127.5 | 0.22 | 11.86 | 7.74 | 1.13 | 31.7 | 8.00 | 563 | 511 |
| 205 | gpu-compute: distance + sphere + pathtrace (incorrect) | 121.4 | 0.21 | 77.15 | 2.42 | 0.95 | 31.7 | 8.00 | 819 | 506 |
| 206 | gpu-compute: clipmap + beam + rowrun + pathtrace (incorrect) | 120.5 | 0.21 | 11.67 | 8.21 | 1.14 | 31.7 | 8.00 | 603 | 529 |
| 207 | gpu-compute: clipmap + beam + incremental + pathtrace (incorrect) | 119.3 | 0.21 | 11.79 | 8.28 | 1.11 | 31.7 | 8.00 | 603 | 529 |
| 208 | gpu-compute: hier + beam + rowrun + pathtrace (incorrect) | 119.3 | 0.21 | 12.38 | 8.27 | 1.13 | 31.7 | 8.00 | 563 | 511 |
| 209 | gpu-compute: distance + beam + sphere + pathtrace (incorrect) | 118.8 | 0.21 | 74.59 | 2.63 | 0.93 | 31.7 | 8.00 | 819 | 507 |
| 210 | gpu-compute: bricks + sphere + pathtrace (incorrect) | 116.8 | 0.20 | 12.23 | 8.49 | 1.09 | 31.7 | 8.00 | 563 | 508 |
| 211 | gpu-compute: rows + sphere + pathtrace (incorrect) | 115.9 | 0.20 | 12.32 | 8.52 | 1.15 | 31.7 | 8.00 | 595 | 508 |
| 212 | gpu-compute: rows + beam + sphere + pathtrace (incorrect) | 115.0 | 0.20 | 12.24 | 8.59 | 1.13 | 31.7 | 8.00 | 595 | 508 |
| 213 | gpu-compute: bricks + beam + sphere + pathtrace (incorrect) | 114.9 | 0.20 | 12.35 | 8.64 | 1.13 | 31.7 | 8.00 | 563 | 508 |
| 214 | gpu-compute: prefix + sphere + pathtrace (incorrect) | 114.5 | 0.20 | 12.64 | 8.64 | 1.13 | 31.7 | 8.00 | 603 | 508 |
| 215 | gpu-compute: tree64 + beam + incremental + pathtrace (incorrect) | 114.3 | 0.20 | 14.07 | 8.66 | 1.53 | 31.7 | 8.00 | 126 | 600 |
| 216 | gpu-compute: distance + beam + refloor + pathtrace (incorrect) | 112.8 | 0.20 | 73.24 | 3.21 | 0.99 | 31.7 | 8.00 | 819 | 506 |
| 217 | gpu-compute: prefix + beam + sphere + pathtrace (incorrect) | 112.4 | 0.20 | 12.91 | 8.80 | 1.15 | 31.7 | 8.00 | 603 | 508 |
| 218 | gpu-compute: distance + refloor + pathtrace (incorrect) | 112.2 | 0.20 | 77.90 | 3.09 | 0.98 | 31.7 | 8.00 | 819 | 506 |
| 219 | gpu-compute: distance + rowrun + pathtrace (incorrect) | 101.6 | 0.18 | 77.52 | 4.39 | 1.02 | 31.7 | 8.00 | 819 | 506 |
| 220 | gpu-compute: distance + beam + rowrun + pathtrace (incorrect) | 99.0 | 0.17 | 77.63 | 4.63 | 1.07 | 31.7 | 8.00 | 819 | 509 |
| 221 | gpu-compute: clipmap + sphere + pathtrace (incorrect) | 92.1 | 0.16 | 15.69 | 10.78 | 1.16 | 31.7 | 8.00 | 603 | 529 |
| 222 | gpu-compute: svo_dag + sphere + pathtrace (incorrect) | 90.6 | 0.16 | 16.51 | 10.78 | 1.93 | 31.7 | 8.00 | 61 | 618 |
| 223 | gpu-splat: prefix + points + splat (incorrect) | 87.8 | 0.15 | 117.82 | 1.71 | 0.81 | 23.5 | 9.26 | 766 | 823 |
| 224 | gpu-splat: rows + points + splat (incorrect) | 86.7 | 0.15 | 116.12 | 1.80 | 0.84 | 23.5 | 9.26 | 758 | 821 |
| 225 | gpu-splat: bricks + points + splat (incorrect) | 86.3 | 0.15 | 119.57 | 1.92 | 0.88 | 23.5 | 9.26 | 726 | 820 |
| 226 | gpu-splat: hier + points + splat (incorrect) | 86.0 | 0.15 | 109.33 | 1.91 | 0.86 | 23.5 | 9.26 | 726 | 824 |
| 227 | gpu-splat: clipmap + points + splat (incorrect) | 85.6 | 0.15 | 109.12 | 2.16 | 0.90 | 23.5 | 9.26 | 766 | 843 |
| 228 | gpu-compute: svo_dag + beam + sphere + pathtrace (incorrect) | 84.7 | 0.15 | 17.12 | 11.56 | 1.83 | 31.7 | 8.00 | 61 | 618 |
| 229 | gpu-compute: clipmap + beam + sphere + pathtrace (incorrect) | 84.6 | 0.15 | 16.31 | 11.74 | 1.19 | 31.7 | 8.00 | 603 | 529 |
| 230 | gpu-splat: svo_dag + points + splat (incorrect) | 83.4 | 0.15 | 113.32 | 1.85 | 1.70 | 23.5 | 9.26 | 225 | 942 |
| 231 | gpu-splat: tree64 + points + splat (incorrect) | 77.9 | 0.14 | 149.74 | 1.90 | 1.58 | 23.5 | 9.26 | 289 | 919 |
| 232 | gpu-compute: svo_dag + incremental + pathtrace (incorrect) | 75.8 | 0.13 | 20.38 | 12.96 | 1.86 | 31.7 | 8.00 | 61 | 619 |
| 233 | gpu-compute: svo_dag + rowrun + pathtrace (incorrect) | 74.8 | 0.13 | 19.87 | 13.13 | 1.90 | 31.7 | 8.00 | 61 | 619 |
| 234 | gpu-compute: tree64 + sphere + pathtrace (incorrect) | 74.3 | 0.13 | 19.98 | 13.40 | 1.59 | 31.7 | 8.00 | 126 | 597 |
| 235 | gpu-compute: tree64 + beam + sphere + pathtrace (incorrect) | 73.1 | 0.13 | 19.98 | 13.61 | 1.55 | 31.7 | 8.00 | 126 | 600 |
| 236 | gpu-compute: svo_dag + beam + rowrun + pathtrace (incorrect) | 70.8 | 0.12 | 20.42 | 13.90 | 1.92 | 31.7 | 8.00 | 61 | 617 |
| 237 | gpu-compute: svo_dag + beam + incremental + pathtrace (incorrect) | 68.2 | 0.12 | 22.02 | 14.43 | 1.89 | 31.7 | 8.00 | 61 | 618 |
| 238 | gpu-compute: prefix + beam + incremental + pathtrace (incorrect) | 64.5 | 0.11 | 22.77 | 15.44 | 1.17 | 31.7 | 8.00 | 603 | 508 |
| 239 | gpu-compute: hier + beam + incremental + pathtrace (incorrect) | 62.4 | 0.11 | 23.07 | 15.96 | 1.18 | 31.7 | 8.00 | 563 | 511 |
| 240 | gpu-compute: prefix + incremental + pathtrace (incorrect) | 61.4 | 0.11 | 24.05 | 16.22 | 1.11 | 31.7 | 8.00 | 603 | 508 |
| 241 | gpu-compute: bricks + beam + incremental + pathtrace (incorrect) | 61.3 | 0.11 | 22.66 | 16.25 | 1.14 | 31.7 | 8.00 | 563 | 508 |
| 242 | gpu-compute: rows + beam + incremental + pathtrace (incorrect) | 60.2 | 0.10 | 22.82 | 16.56 | 1.20 | 31.7 | 8.00 | 595 | 508 |
| 243 | gpu-compute: hier + incremental + pathtrace (incorrect) | 59.7 | 0.10 | 24.18 | 16.68 | 1.20 | 31.7 | 8.00 | 563 | 511 |
| 244 | gpu-splat: distance + points + splat (incorrect) | 59.1 | 0.10 | 181.59 | 1.74 | 0.83 | 23.5 | 9.26 | 982 | 822 |
| 245 | gpu-compute: bricks + incremental + pathtrace (incorrect) | 57.9 | 0.10 | 23.96 | 17.20 | 1.22 | 31.7 | 8.00 | 563 | 508 |
| 246 | gpu-compute: rows + incremental + pathtrace (incorrect) | 57.8 | 0.10 | 23.99 | 17.24 | 1.20 | 31.7 | 8.00 | 595 | 508 |
| 247 | gpu-compute: tree64 + beam + rowrun + pathtrace (incorrect) | 56.1 | 0.10 | 25.40 | 17.76 | 1.59 | 31.7 | 8.00 | 126 | 597 |
| 248 | gpu-compute: tree64 + rowrun + pathtrace (incorrect) | 55.6 | 0.10 | 26.46 | 17.92 | 1.62 | 31.7 | 8.00 | 126 | 600 |
| 249 | gpu-compute: distance + beam + incremental + pathtrace (incorrect) | 43.2 | 0.08 | 78.73 | 20.49 | 1.13 | 31.7 | 8.00 | 819 | 509 |
| 250 | gpu-compute: distance + incremental + pathtrace (incorrect) | 41.3 | 0.07 | 78.35 | 21.81 | 1.14 | 31.7 | 8.00 | 819 | 508 |
| 251 | cpu-discover: hier + quadtree (incorrect) | 27.8 | 0.05 | 80.45 | 2.47 | 34.64 | 34.4 | 0.23 | 535 | 1126 |
| 252 | cpu-discover: prefix + quadtree (incorrect) | 27.8 | 0.05 | 79.96 | 2.54 | 34.52 | 34.4 | 0.23 | 535 | 1336 |
| 253 | cpu-discover: morton + quadtree (incorrect) | 26.2 | 0.05 | 83.02 | 2.53 | 36.65 | 34.4 | 0.23 | 535 | 1380 |
| 254 | cpu-discover: u8 + quadtree (incorrect) | 25.9 | 0.05 | 86.87 | 2.52 | 37.26 | 34.4 | 0.23 | 535 | 1250 |
| 255 | cpu-discover: brickmajor + quadtree (incorrect) | 25.8 | 0.04 | 85.42 | 2.51 | 37.10 | 34.4 | 0.23 | 535 | 1395 |
| 256 | cpu-discover: bricks + quadtree (incorrect) | 25.8 | 0.04 | 84.29 | 2.52 | 37.50 | 34.4 | 0.23 | 535 | 1116 |
| 257 | cpu-discover: bits + quadtree (incorrect) | 25.6 | 0.04 | 85.43 | 2.53 | 37.57 | 34.4 | 0.23 | 535 | 1142 |
| 258 | cpu-discover: palette + quadtree (incorrect) | 25.4 | 0.04 | 84.06 | 2.50 | 37.43 | 34.4 | 0.23 | 535 | 1156 |

### terrain-flight [unlit]

1920×1080. camera 2.25 blocks/frame (max 3.0), turn 0.35°/frame; 3985 chunks at start, 11.1 loads/frame; chunk fill 11%, solid 8.5%, surface 0.65%; sky 64%, hits at 268 blocks (median 235); 0.0 edits/frame; 400 lights (animated); 0 mesh triangles.

| # | renderer | fps | rel | p99 ms | gpu ms | cpu ms | PSNR | bad % | GPU MB | RSS MB |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu-compute: bricks + refloor | 652.4 | 1.00 | 9.02 | 0.86 | 0.81 | 59.5 | 0.01 | 531 | 507 |
| 2 | gpu-compute: prefix + refloor | 627.8 | 0.96 | 12.75 | 0.76 | 0.75 | 59.5 | 0.01 | 571 | 508 |
| 3 | gpu-compute: hier + refloor | 617.9 | 0.95 | 13.26 | 0.78 | 0.74 | 59.5 | 0.01 | 531 | 510 |
| 4 | gpu-compute: tree64 + refloor | 597.3 | 0.92 | 9.45 | 1.00 | 1.11 | 59.5 | 0.01 | 94 | 600 |
| 5 | gpu-compute: bricks + beam + refloor | 596.0 | 0.91 | 9.32 | 1.07 | 0.78 | 59.5 | 0.01 | 531 | 508 |
| 6 | gpu-compute: prefix + beam + refloor | 587.8 | 0.90 | 12.09 | 0.97 | 0.77 | 59.5 | 0.01 | 571 | 508 |
| 7 | gpu-compute: rows + refloor | 583.5 | 0.89 | 11.00 | 0.88 | 0.75 | 59.5 | 0.01 | 563 | 508 |
| 8 | gpu-compute: prefix + rowrun | 579.2 | 0.89 | 12.97 | 0.96 | 0.77 | 59.5 | 0.01 | 571 | 508 |
| 9 | gpu-compute: hier + beam + refloor | 569.8 | 0.87 | 12.02 | 1.00 | 0.75 | 59.5 | 0.01 | 531 | 509 |
| 10 | gpu-compute: rows + rowrun | 540.0 | 0.83 | 11.72 | 1.02 | 0.76 | 59.5 | 0.01 | 563 | 508 |
| 11 | gpu-compute: bricks + rowrun | 536.6 | 0.82 | 7.96 | 1.30 | 0.76 | 59.5 | 0.01 | 531 | 508 |
| 12 | gpu-compute: rows + beam + refloor | 530.2 | 0.81 | 13.11 | 1.10 | 0.74 | 59.5 | 0.01 | 563 | 506 |
| 13 | gpu-compute: svo_dag + refloor | 528.8 | 0.81 | 9.68 | 0.93 | 1.45 | 59.5 | 0.01 | 30 | 618 |
| 14 | gpu-compute: prefix + beam + rowrun | 518.1 | 0.79 | 12.14 | 1.15 | 0.77 | 59.5 | 0.01 | 571 | 509 |
| 15 | gpu-compute: clipmap + refloor | 507.0 | 0.78 | 8.26 | 1.41 | 0.76 | 59.5 | 0.01 | 571 | 531 |
| 16 | gpu-compute: bricks + beam + rowrun | 501.1 | 0.77 | 6.82 | 1.53 | 0.76 | 59.5 | 0.01 | 531 | 508 |
| 17 | gpu-compute: rows + beam + rowrun | 497.5 | 0.76 | 9.60 | 1.25 | 0.76 | 59.5 | 0.01 | 563 | 506 |
| 18 | gpu-raster: bricks + naive + raster | 468.5 | 0.72 | 17.45 | 0.63 | 1.51 | 55.2 | 0.01 | 668 | 627 |
| 19 | gpu-compute: tree64 + incremental | 468.4 | 0.72 | 8.95 | 1.59 | 1.14 | 59.5 | 0.01 | 94 | 600 |
| 20 | gpu-raster: bricks + naive + visbuffer | 459.9 | 0.70 | 17.33 | 0.69 | 1.53 | 55.0 | 0.02 | 668 | 627 |
| 21 | gpu-raster: bricks + naive + gbuffer | 452.4 | 0.69 | 17.43 | 0.69 | 1.55 | 55.0 | 0.02 | 668 | 628 |
| 22 | gpu-compute: clipmap + incremental | 439.9 | 0.67 | 7.45 | 1.79 | 0.80 | 59.5 | 0.01 | 571 | 531 |
| 23 | gpu-rt: boxes + rayquery | 439.5 | 0.67 | 13.13 | 1.00 | 1.31 | 59.5 | 0.01 | 561 | 814 |
| 24 | gpu-compute: clipmap + rowrun | 437.5 | 0.67 | 7.79 | 1.75 | 0.81 | 59.5 | 0.01 | 571 | 529 |
| 25 | gpu-discover: bricks + quadtree | 431.1 | 0.66 | 6.87 | 1.83 | 0.82 | 53.6 | 0.02 | 713 | 521 |
| 26 | gpu-compute: tree64 + beam + refloor | 429.1 | 0.66 | 8.48 | 1.85 | 1.18 | 59.5 | 0.01 | 94 | 601 |
| 27 | gpu-compute: hier + rowrun | 427.8 | 0.66 | 8.78 | 1.72 | 0.79 | 59.5 | 0.01 | 531 | 509 |
| 28 | gpu-compute: hier + sphere | 411.4 | 0.63 | 8.56 | 1.84 | 0.81 | 59.5 | 0.01 | 531 | 509 |
| 29 | gpu-compute: bricks + sphere | 408.8 | 0.63 | 6.78 | 2.07 | 0.86 | 59.5 | 0.01 | 531 | 508 |
| 30 | gpu-discover: prefix + quadtree | 405.7 | 0.62 | 8.46 | 1.81 | 0.84 | 53.6 | 0.02 | 753 | 522 |
| 31 | gpu-discover: hier + quadtree | 398.2 | 0.61 | 9.05 | 1.84 | 0.85 | 53.6 | 0.02 | 713 | 514 |
| 32 | gpu-discover: rows + quadtree | 391.7 | 0.60 | 9.82 | 1.83 | 0.83 | 53.6 | 0.02 | 745 | 513 |
| 33 | gpu-compute: hier + beam + rowrun | 389.5 | 0.60 | 8.71 | 1.96 | 0.83 | 59.5 | 0.01 | 531 | 509 |
| 34 | gpu-compute: prefix + sphere | 388.6 | 0.60 | 8.33 | 2.04 | 0.84 | 59.5 | 0.01 | 571 | 508 |
| 35 | gpu-compute: hier + beam + sphere | 384.4 | 0.59 | 8.72 | 2.04 | 0.85 | 59.5 | 0.01 | 531 | 509 |
| 36 | gpu-compute: clipmap + beam + refloor | 383.6 | 0.59 | 6.71 | 2.19 | 0.81 | 59.5 | 0.01 | 571 | 529 |
| 37 | gpu-compute: bricks + beam + sphere | 380.2 | 0.58 | 7.32 | 2.25 | 0.88 | 59.5 | 0.01 | 531 | 508 |
| 38 | gpu-discover: tree64 + quadtree | 375.9 | 0.58 | 8.37 | 2.20 | 1.26 | 53.6 | 0.02 | 276 | 609 |
| 39 | gpu-discover: clipmap + quadtree | 373.3 | 0.57 | 6.84 | 2.23 | 0.90 | 53.6 | 0.02 | 754 | 536 |
| 40 | gpu-compute: rows + sphere | 370.8 | 0.57 | 9.57 | 2.06 | 0.84 | 59.5 | 0.01 | 563 | 506 |
| 41 | gpu-compute: svo_dag + beam + refloor | 368.2 | 0.56 | 8.53 | 1.96 | 1.51 | 59.5 | 0.01 | 30 | 617 |
| 42 | gpu-compute: prefix + beam + sphere | 362.4 | 0.56 | 8.60 | 2.24 | 0.89 | 59.5 | 0.01 | 571 | 508 |
| 43 | gpu-compute: tree64 + beam + incremental | 354.1 | 0.54 | 7.73 | 2.41 | 1.24 | 59.5 | 0.01 | 94 | 600 |
| 44 | gpu-compute: rows + beam + sphere | 351.3 | 0.54 | 9.53 | 2.26 | 0.87 | 59.5 | 0.01 | 563 | 506 |
| 45 | gpu-compute: clipmap + beam + rowrun | 344.2 | 0.53 | 6.98 | 2.53 | 0.90 | 59.5 | 0.01 | 571 | 531 |
| 46 | gpu-compute: clipmap + beam + incremental | 339.1 | 0.52 | 6.94 | 2.60 | 0.94 | 59.5 | 0.01 | 571 | 531 |
| 47 | gpu-discover: svo_dag + quadtree | 334.2 | 0.51 | 9.17 | 2.21 | 1.59 | 53.6 | 0.02 | 212 | 626 |
| 48 | gpu-compute: svo_dag + sphere | 328.2 | 0.50 | 8.52 | 2.36 | 1.58 | 59.5 | 0.01 | 30 | 616 |
| 49 | gpu-raster: bricks + greedy + visbuffer | 319.3 | 0.49 | 28.86 | 0.31 | 2.53 | 53.1 | 0.02 | 604 | 561 |
| 50 | gpu-raster: bricks + greedy + raster | 318.9 | 0.49 | 29.37 | 0.26 | 2.54 | 53.2 | 0.02 | 604 | 561 |
| 51 | gpu-raster: bricks + greedy + gbuffer | 317.8 | 0.49 | 28.22 | 0.35 | 2.54 | 53.1 | 0.02 | 604 | 561 |
| 52 | gpu-compute: clipmap + sphere | 315.1 | 0.48 | 7.75 | 2.85 | 0.97 | 59.5 | 0.01 | 571 | 529 |
| 53 | gpu-compute: svo_dag + incremental | 298.8 | 0.46 | 8.33 | 2.69 | 1.62 | 59.5 | 0.01 | 30 | 618 |
| 54 | gpu-compute: svo_dag + rowrun | 291.6 | 0.45 | 8.15 | 2.80 | 1.65 | 59.5 | 0.01 | 30 | 617 |
| 55 | gpu-compute: tree64 + sphere | 279.7 | 0.43 | 7.38 | 3.30 | 1.38 | 59.5 | 0.01 | 94 | 600 |
| 56 | gpu-compute: clipmap + beam + sphere | 259.7 | 0.40 | 8.13 | 3.65 | 1.01 | 59.5 | 0.01 | 571 | 531 |
| 57 | gpu-compute: svo_dag + beam + sphere | 255.2 | 0.39 | 8.17 | 3.36 | 1.71 | 59.5 | 0.01 | 30 | 617 |
| 58 | gpu-compute: tree64 + rowrun | 253.9 | 0.39 | 7.59 | 3.70 | 1.40 | 59.5 | 0.01 | 94 | 599 |
| 59 | gpu-compute: svo_dag + beam + incremental | 236.0 | 0.36 | 7.85 | 3.71 | 1.74 | 59.5 | 0.01 | 30 | 618 |
| 60 | gpu-compute: prefix + incremental | 233.4 | 0.36 | 9.61 | 4.06 | 1.01 | 59.5 | 0.01 | 571 | 508 |
| 61 | gpu-compute: svo_dag + beam + rowrun | 232.4 | 0.36 | 7.67 | 3.79 | 1.77 | 59.5 | 0.01 | 30 | 618 |
| 62 | gpu-compute: tree64 + beam + sphere | 229.0 | 0.35 | 7.65 | 4.16 | 1.38 | 59.5 | 0.01 | 94 | 600 |
| 63 | gpu-compute: prefix + beam + incremental | 225.1 | 0.35 | 9.68 | 4.24 | 1.08 | 59.5 | 0.01 | 571 | 508 |
| 64 | gpu-compute: hier + incremental | 222.9 | 0.34 | 10.31 | 4.27 | 1.03 | 59.5 | 0.01 | 531 | 509 |
| 65 | gpu-compute: hier + beam + incremental | 213.1 | 0.33 | 10.08 | 4.50 | 1.03 | 59.5 | 0.01 | 531 | 509 |
| 66 | gpu-compute: tree64 + beam + rowrun | 210.7 | 0.32 | 7.58 | 4.57 | 1.42 | 59.5 | 0.01 | 94 | 598 |
| 67 | gpu-compute: bricks + beam + incremental | 208.9 | 0.32 | 8.05 | 4.69 | 1.10 | 59.5 | 0.01 | 531 | 508 |
| 68 | gpu-compute: rows + incremental | 207.8 | 0.32 | 10.74 | 4.59 | 1.02 | 59.5 | 0.01 | 563 | 508 |
| 69 | gpu-rt: greedy + rayquery | 197.6 | 0.30 | 46.58 | 0.76 | 3.95 | 59.5 | 0.01 | 350 | 1127 |
| 70 | gpu-compute: bricks + incremental | 194.2 | 0.30 | 9.49 | 5.05 | 1.11 | 59.5 | 0.01 | 531 | 508 |
| 71 | gpu-compute: rows + beam + incremental | 191.8 | 0.29 | 10.73 | 5.02 | 1.06 | 59.5 | 0.01 | 563 | 508 |
| 72 | gpu-rt: naive + rayquery | 177.9 | 0.27 | 51.29 | 1.27 | 3.63 | 59.5 | 0.01 | 912 | 1556 |
| 73 | gpu-raster: bricks + cubes + raster | 177.2 | 0.27 | 10.22 | 5.33 | 1.52 | 55.1 | 0.01 | 668 | 594 |
| 74 | gpu-raster: bricks + cubes + gbuffer | 175.8 | 0.27 | 9.64 | 5.39 | 1.58 | 54.9 | 0.02 | 668 | 592 |
| 75 | gpu-raster: bricks + cubes + visbuffer | 175.8 | 0.27 | 9.38 | 5.39 | 1.61 | 55.0 | 0.02 | 668 | 592 |
| 76 | gpu-compute: distance + sphere | 141.8 | 0.22 | 121.17 | 0.60 | 0.81 | 59.5 | 0.01 | 787 | 508 |
| 77 | gpu-compute: distance + refloor | 141.7 | 0.22 | 122.66 | 0.71 | 0.79 | 59.5 | 0.01 | 787 | 509 |
| 78 | gpu-compute: distance + beam + sphere | 138.4 | 0.21 | 120.41 | 0.81 | 0.84 | 59.5 | 0.01 | 787 | 509 |
| 79 | gpu-compute: distance + beam + refloor | 137.1 | 0.21 | 119.33 | 0.94 | 0.83 | 59.5 | 0.01 | 787 | 509 |
| 80 | gpu-compute: distance + rowrun | 137.0 | 0.21 | 119.65 | 0.94 | 0.85 | 59.5 | 0.01 | 787 | 509 |
| 81 | gpu-compute: distance + beam + rowrun | 134.6 | 0.21 | 83.42 | 1.15 | 0.85 | 59.5 | 0.01 | 787 | 508 |
| 82 | gpu-discover: distance + quadtree | 127.1 | 0.19 | 76.18 | 1.81 | 0.84 | 53.6 | 0.02 | 969 | 513 |
| 83 | gpu-compute: distance + incremental | 84.3 | 0.13 | 78.35 | 6.73 | 1.14 | 59.5 | 0.01 | 787 | 508 |
| 84 | gpu-compute: distance + beam + incremental | 82.9 | 0.13 | 77.25 | 7.00 | 1.12 | 59.5 | 0.01 | 787 | 508 |
| 85 | cpu: hier + refloor | 40.4 | 0.06 | 36.06 | - | 24.33 | 63.2 | 0.00 | - | 337 |
| 86 | cpu: hier + incremental | 37.5 | 0.06 | 38.49 | - | 26.25 | 63.2 | 0.00 | - | 336 |
| 87 | cpu: prefix + incremental | 37.4 | 0.06 | 39.41 | - | 26.29 | 63.1 | 0.00 | - | 405 |
| 88 | cpu: bricks + refloor | 35.9 | 0.06 | 39.82 | - | 27.58 | 63.2 | 0.00 | - | 321 |
| 89 | cpu: prefix + refloor | 35.1 | 0.05 | 44.21 | - | 28.05 | 63.2 | 0.00 | - | 397 |
| 90 | cpu: u8 + refloor | 34.8 | 0.05 | 40.90 | - | 28.42 | 63.2 | 0.00 | - | 252 |
| 91 | cpu: morton + refloor | 34.7 | 0.05 | 41.28 | - | 28.30 | 63.2 | 0.00 | - | 439 |
| 92 | cpu: bits + refloor | 34.6 | 0.05 | 41.07 | - | 28.49 | 63.2 | 0.00 | - | 337 |
| 93 | cpu: brickmajor + refloor | 34.3 | 0.05 | 42.18 | - | 28.60 | 63.2 | 0.00 | - | 437 |
| 94 | cpu: palette + refloor | 34.2 | 0.05 | 43.50 | - | 28.44 | 63.2 | 0.00 | - | 250 |
| 95 | cpu: bricks + incremental | 34.1 | 0.05 | 40.62 | - | 29.09 | 63.2 | 0.00 | - | 321 |
| 96 | cpu: bits + incremental | 33.8 | 0.05 | 41.68 | - | 29.12 | 63.2 | 0.00 | - | 337 |
| 97 | cpu: u8 + incremental | 33.5 | 0.05 | 44.76 | - | 29.50 | 63.2 | 0.00 | - | 261 |
| 98 | cpu: brickmajor + incremental | 33.5 | 0.05 | 42.72 | - | 29.31 | 63.2 | 0.00 | - | 441 |
| 99 | cpu-raster: greedy + raster | 33.2 | 0.05 | 52.71 | - | 29.33 | 67.2 | 0.00 | - | 580 |
| 100 | cpu: morton + incremental | 33.1 | 0.05 | 42.47 | - | 29.63 | 63.2 | 0.00 | - | 436 |
| 101 | cpu: palette + incremental | 33.1 | 0.05 | 44.77 | - | 29.35 | 63.2 | 0.00 | - | 246 |
| 102 | cpu-discover: hier + quadtree | 27.6 | 0.04 | 88.77 | 0.19 | 34.85 | 36.3 | 0.22 | 535 | 1159 |
| 103 | cpu-discover: prefix + quadtree | 27.3 | 0.04 | 84.56 | 0.18 | 35.18 | 36.3 | 0.22 | 535 | 1371 |
| 104 | cpu-discover: morton + quadtree | 26.0 | 0.04 | 92.55 | 0.19 | 36.86 | 36.3 | 0.22 | 535 | 1377 |
| 105 | cpu-discover: u8 + quadtree | 25.8 | 0.04 | 91.56 | 0.20 | 37.48 | 36.3 | 0.22 | 535 | 1279 |
| 106 | cpu-discover: bricks + quadtree | 25.8 | 0.04 | 95.61 | 0.16 | 37.63 | 36.3 | 0.22 | 535 | 1088 |
| 107 | cpu-discover: bits + quadtree | 25.8 | 0.04 | 91.44 | 0.16 | 37.46 | 36.3 | 0.22 | 535 | 1099 |
| 108 | cpu-discover: brickmajor + quadtree | 25.7 | 0.04 | 95.49 | 0.18 | 37.29 | 36.3 | 0.22 | 535 | 1395 |
| 109 | cpu-discover: palette + quadtree | 25.3 | 0.04 | 96.03 | 0.21 | 37.57 | 36.3 | 0.22 | 535 | 1151 |
| 110 | cpu-raster: naive + raster | 17.0 | 0.03 | 78.02 | - | 55.99 | 67.9 | 0.00 | - | 834 |
| 111 | cpu-discover: hier + cluster | 11.2 | 0.02 | 211.13 | 0.17 | 88.06 | 39.1 | 0.18 | 535 | 885 |
| 112 | cpu-discover: prefix + cluster | 11.0 | 0.02 | 223.86 | 0.17 | 89.79 | 39.1 | 0.18 | 535 | 1081 |
| 113 | cpu-discover: morton + cluster | 10.6 | 0.02 | 227.65 | 0.17 | 93.35 | 39.1 | 0.18 | 535 | 1143 |
| 114 | cpu-discover: palette + cluster | 10.5 | 0.02 | 221.47 | 0.17 | 93.66 | 39.1 | 0.18 | 535 | 863 |
| 115 | cpu-discover: bits + cluster | 10.5 | 0.02 | 226.40 | 0.16 | 94.13 | 39.1 | 0.18 | 535 | 885 |
| 116 | cpu-discover: u8 + cluster | 10.4 | 0.02 | 228.75 | 0.17 | 94.72 | 39.1 | 0.18 | 535 | 987 |
| 117 | cpu-discover: brickmajor + cluster | 10.2 | 0.02 | 231.91 | 0.17 | 96.95 | 39.1 | 0.18 | 535 | 1113 |
| 118 | cpu-discover: bricks + cluster | 10.2 | 0.02 | 235.28 | 0.17 | 97.54 | 39.1 | 0.18 | 535 | 854 |
| 119 | gpu-voxelspace: heightmap + columns (incorrect) | 607.7 | 0.93 | 8.44 | 1.11 | 0.76 | 20.9 | 19.96 | 27 | 573 |
| 120 | cpu-voxelspace: heightmap + columns (incorrect) | 296.5 | 0.45 | 5.38 | - | 3.16 | 20.9 | 19.96 | - | 383 |
| 121 | gpu-splat: bricks + points + splat (incorrect) | 90.2 | 0.14 | 119.48 | 1.38 | 0.79 | 26.9 | 8.32 | 726 | 821 |

