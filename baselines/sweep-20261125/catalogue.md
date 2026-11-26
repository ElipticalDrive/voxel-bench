# Renderer catalogue

4269 observations from 3 runs, 6 scenarios. Machine: AMD Ryzen 9 9955HX 16-Core Processor + NVIDIA GeForce RTX 5060 Laptop GPU.

How to read: a scenario is a plan (scene, camera path, events) in one pass mode. Speed is throughput (completed frames per second; at several hundred fps and more the host's per-frame overhead compresses differences, so GPU ms is shown too). *rel* is fps relative to the fastest correct renderer of the scenario; correct means a mean PSNR against the reference of at least 35 dB (or not compared). *good at*: within 10% of the best. *stands out*: relative speed at least 1.25× the renderer's own average, i.e. where it is comparatively strong.

## Below the floor

Renderers that failed or whose frames scored below 35 dB (by the settled score for a pipeline with a late edge or accumulated frames, else the score against the reference given the same, else the plain one): each a bug or a missing `accept` rule. 

| scenario | renderer | why |
|---|---|---|
| far [lit] | gpu: heightmap + columns + heightfield + direct | 15.4 dB like |
| far [lit] | cpu: heightmap + columns + heightfield + direct | 15.4 dB like |
| far [lit] | gpu: heightmap + columns + heightfield + pathtrace | 15.4 dB settled |
| far [lit] | gpu: naive@cpu + rayquery + raycast + direct | failed: [gpu: naive@cpu + rayquery + raycast + direct]: worker error: the device is out of memory (NVIDIA GeForce RTX 5060 Laptop GPU, this renderer seen holding up to 6923 MiB; the world's size and the resolution set most of it: lower the view radius or the resolution, or the storage's pool or slot knobs): wgpu error: Out of Memory |
| far [lit] | gpu: naive@cpu + rayquery + raycast + pathtrace | failed: [gpu: naive@cpu + rayquery + raycast + pathtrace]: worker error: the device is out of memory (NVIDIA GeForce RTX 5060 Laptop GPU, this renderer seen holding up to 6954 MiB; the world's size and the resolution set most of it: lower the view radius or the resolution, or the storage's pool or slot knobs): wgpu error: Out of Memory |
| far [unlit] | cpu: heightmap + columns + heightfield + direct | 18.4 dB like |
| far [unlit] | gpu: heightmap + columns + heightfield + pathtrace | 18.4 dB like |
| far [unlit] | gpu: naive@cpu + rayquery + raycast + pathtrace | failed: [gpu: naive@cpu + rayquery + raycast + pathtrace]: worker error: the device is out of memory (NVIDIA GeForce RTX 5060 Laptop GPU, this renderer seen holding up to 6954 MiB; the world's size and the resolution set most of it: lower the view radius or the resolution, or the storage's pool or slot knobs): wgpu error: Out of Memory |
| showcase [lit] | gpu: heightmap + columns + heightfield + direct | 10.9 dB like |
| showcase [lit] | cpu: heightmap + columns + heightfield + direct | 10.9 dB like |
| showcase [lit] | gpu: heightmap + columns + heightfield + pathtrace | 10.9 dB settled |
| showcase [unlit] | gpu: heightmap + columns + heightfield + pathtrace | 12.2 dB like |
| showcase [unlit] | cpu: heightmap + columns + heightfield + direct | 12.2 dB like |
| terrain-flight [lit] | gpu: heightmap + columns + heightfield + direct | 18.3 dB like |
| terrain-flight [lit] | cpu: heightmap + columns + heightfield + direct | 18.3 dB like |
| terrain-flight [lit] | gpu: heightmap + columns + heightfield + pathtrace | 18.1 dB settled |
| terrain-flight [unlit] | gpu: heightmap + columns + heightfield + pathtrace | 21.0 dB like |
| terrain-flight [unlit] | cpu: heightmap + columns + heightfield + direct | 21.0 dB like |

## Best per scenario

| scenario | best | fps | gpu ms | PSNR | runner-up | rel |
|---|---|---:|---:|---:|---|---:|
| far [lit] | gpu: bricks + refloor + composite + raycast + direct | 375.0 | 1.14 | 56.7 | gpu: bricks + refloor + beam + composite + raycast + direct | 0.98 |
| far [unlit] | gpu: bricks + refloor + composite + raycast + pathtrace | 400.5 | 0.88 | 57.7 | gpu: tree64 + refloor + composite + raycast + pathtrace | 0.96 |
| showcase [lit] | gpu: distance + sphere + composite + raycast + direct | 4434.8 | 0.15 | 58.3 | gpu: distance + sphere + beam + composite + raycast + direct | 0.94 |
| showcase [unlit] | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 6842.2 | 0.03 | 44.1 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.88 |
| terrain-flight [lit] | gpu: bricks + refloor + composite + raycast + direct | 568.7 | 1.16 | 57.8 | gpu: prefix + refloor + composite + raycast + direct | 0.96 |
| terrain-flight [unlit] | gpu: bricks + refloor + composite + raycast + pathtrace | 611.8 | 0.86 | 59.5 | gpu: tree64 + refloor + composite + raycast + pathtrace | 0.93 |

## Renderers

Sorted by the geometric mean of *rel* over the scenarios where the renderer ran correctly.

| renderer | scenarios | geomean rel | geomean mem | wins | good at | stands out | weak at (rel < 0.25) | incorrect | failed |
|---|---:|---:|---:|---:|---|---|---|---|---|
| gpu: bricks + refloor + composite + raycast + direct | 3 | 0.89 | 6.55 | 2 | far [lit], terrain-flight [lit] |  |  |  |  |
| gpu: prefix + refloor + composite + raycast + direct | 3 | 0.89 | 6.70 | 0 | terrain-flight [lit] |  |  |  |  |
| gpu: hier + refloor + composite + raycast + direct | 3 | 0.85 | 6.59 | 0 | terrain-flight [lit] |  |  |  |  |
| gpu: bricks + refloor + beam + composite + raycast + direct | 3 | 0.83 | 6.58 | 0 | far [lit] |  |  |  |  |
| gpu: prefix + refloor + beam + composite + raycast + direct | 3 | 0.83 | 6.70 | 0 |  |  |  |  |  |
| gpu: hier + refloor + beam + composite + raycast + direct | 3 | 0.82 | 6.59 | 0 |  |  |  |  |  |
| gpu: distance + sphere + composite + raycast + direct | 3 | 0.81 | 7.38 | 1 | showcase [lit] |  |  |  |  |
| gpu: prefix + rowrun + composite + raycast + direct | 3 | 0.80 | 6.70 | 0 |  |  |  |  |  |
| gpu: tree64 + refloor + composite + raycast + direct | 3 | 0.80 | 4.68 | 0 | far [lit] |  |  |  |  |
| gpu: svo_dag + refloor + composite + raycast + direct | 3 | 0.78 | 4.49 | 0 |  |  |  |  |  |
| gpu: rows + refloor + composite + raycast + direct | 3 | 0.77 | 6.67 | 0 |  |  |  |  |  |
| gpu: distance + sphere + beam + composite + raycast + direct | 3 | 0.77 | 7.38 | 0 | showcase [lit] |  |  |  |  |
| gpu: prefix + rowrun + beam + composite + raycast + direct | 3 | 0.76 | 6.70 | 0 |  |  |  |  |  |
| gpu: distance + refloor + composite + raycast + direct | 3 | 0.76 | 7.38 | 0 |  |  |  |  |  |
| gpu: rows + refloor + beam + composite + raycast + direct | 3 | 0.74 | 6.68 | 0 |  |  |  |  |  |
| gpu: rows + rowrun + composite + raycast + direct | 3 | 0.73 | 6.67 | 0 |  |  |  |  |  |
| gpu: bricks + rowrun + composite + raycast + direct | 3 | 0.72 | 6.58 | 0 |  |  |  |  |  |
| gpu: distance + refloor + beam + composite + raycast + direct | 3 | 0.70 | 7.38 | 0 |  |  |  |  |  |
| gpu: bricks + rowrun + beam + composite + raycast + direct | 3 | 0.69 | 6.58 | 0 |  |  |  |  |  |
| gpu: clipmap + refloor + composite + raycast + direct | 3 | 0.69 | 6.77 | 0 |  |  |  |  |  |
| gpu: distance + rowrun + composite + raycast + direct | 3 | 0.69 | 7.38 | 0 |  |  |  |  |  |
| gpu: rows + rowrun + beam + composite + raycast + direct | 3 | 0.69 | 6.68 | 0 |  |  |  |  |  |
| gpu: tree64 + refloor + beam + composite + raycast + direct | 3 | 0.68 | 4.72 | 0 |  |  |  |  |  |
| gpu: prefix + refloor + composite + raycast + pathtrace | 6 | 0.66 | 6.76 | 0 | terrain-flight [unlit] | terrain-flight [unlit] (1.4×), showcase [unlit] (1.3×), far [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + direct | 3 | 0.66 | 7.29 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: distance + rowrun + beam + composite + raycast + direct | 3 | 0.65 | 7.39 | 0 |  |  |  |  |  |
| gpu: svo_dag + refloor + beam + composite + raycast + direct | 3 | 0.65 | 4.49 | 0 |  |  |  |  |  |
| gpu: hier + refloor + composite + raycast + pathtrace | 6 | 0.64 | 6.65 | 0 | terrain-flight [unlit] | terrain-flight [unlit] (1.4×), showcase [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + direct | 3 | 0.64 | 7.33 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + direct | 3 | 0.63 | 7.41 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: bricks + refloor + composite + raycast + pathtrace | 6 | 0.63 | 6.64 | 2 | far [unlit], terrain-flight [unlit] | far [unlit] (1.6×), terrain-flight [unlit] (1.6×) |  |  |  |
| gpu: prefix + refloor + beam + composite + raycast + pathtrace | 6 | 0.63 | 6.76 | 0 |  | terrain-flight [unlit] (1.4×), far [unlit] (1.3×), showcase [unlit] (1.3×) |  |  |  |
| gpu: distance + sphere + composite + raycast + pathtrace | 6 | 0.63 | 7.43 | 0 |  | showcase [unlit] (1.4×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + direct | 3 | 0.62 | 7.30 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + direct | 3 | 0.62 | 7.44 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 3 | 0.62 | 5.72 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 3 | 0.62 | 5.66 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + beam + composite + raycast + pathtrace | 6 | 0.61 | 6.64 | 0 | far [unlit] | far [unlit] (1.6×), terrain-flight [unlit] (1.5×) |  |  |  |
| gpu: hier + rowrun + composite + raycast + direct | 3 | 0.61 | 6.59 | 0 |  |  |  |  |  |
| gpu: hier + refloor + beam + composite + raycast + pathtrace | 6 | 0.61 | 6.65 | 0 |  | terrain-flight [unlit] (1.4×), far [unlit] (1.3×), showcase [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 3 | 0.61 | 7.41 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 3 | 0.61 | 7.44 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + direct | 3 | 0.61 | 7.34 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: tree64 + incremental + composite + raycast + direct | 3 | 0.61 | 4.69 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: clipmap + rowrun + composite + raycast + direct | 3 | 0.60 | 6.77 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 3 | 0.60 | 5.61 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 3 | 0.60 | 7.30 | 0 |  |  |  |  |  |
| gpu: distance + sphere + beam + composite + raycast + pathtrace | 6 | 0.60 | 7.44 | 0 |  | showcase [unlit] (1.4×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 3 | 0.60 | 7.96 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 3 | 0.60 | 5.74 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: clipmap + refloor + beam + composite + raycast + direct | 3 | 0.60 | 6.78 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + direct | 3 | 0.60 | 7.41 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 3 | 0.60 | 7.34 | 0 |  |  |  |  |  |
| gpu: tree64 + refloor + composite + raycast + pathtrace | 6 | 0.59 | 4.90 | 0 | far [unlit], terrain-flight [unlit] | far [unlit] (1.6×), terrain-flight [unlit] (1.6×) |  |  |  |
| gpu: hier + rowrun + beam + composite + raycast + direct | 3 | 0.59 | 6.59 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + direct | 3 | 0.59 | 7.38 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 3 | 0.59 | 8.03 | 0 |  |  |  |  |  |
| gpu: bricks + incremental + composite + raycast + direct | 3 | 0.59 | 6.50 | 0 |  |  |  |  |  |
| gpu: rows + refloor + composite + raycast + pathtrace | 6 | 0.58 | 6.73 | 0 |  | terrain-flight [unlit] (1.5×), far [unlit] (1.3×), showcase [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + direct | 3 | 0.58 | 7.41 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + direct | 3 | 0.58 | 7.38 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 3 | 0.58 | 8.06 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + direct | 3 | 0.57 | 8.10 | 0 |  | showcase [lit] (1.6×) |  |  |  |
| gpu: distance + refloor + composite + raycast + pathtrace | 6 | 0.57 | 7.44 | 0 |  | showcase [unlit] (1.4×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: svo_dag + refloor + composite + raycast + pathtrace | 6 | 0.57 | 4.63 | 0 |  | terrain-flight [unlit] (1.4×), showcase [unlit] (1.4×), far [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 3 | 0.57 | 7.98 | 0 |  |  |  |  |  |
| gpu: clipmap + incremental + composite + raycast + direct | 3 | 0.57 | 6.77 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + direct | 3 | 0.56 | 8.05 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + direct | 3 | 0.56 | 8.06 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 3 | 0.55 | 8.06 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 3 | 0.55 | 5.84 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + direct | 3 | 0.55 | 8.09 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 3 | 0.55 | 7.98 | 0 |  |  |  |  |  |
| gpu: boxes@cpu + rayquery + raycast + direct | 3 | 0.55 | 8.48 | 0 |  |  |  |  |  |
| gpu: prefix + rowrun + composite + raycast + pathtrace | 6 | 0.55 | 6.83 | 0 |  | terrain-flight [unlit] (1.5×), far [unlit] (1.4×), showcase [unlit] (1.4×) |  |  |  |
| gpu: rows + refloor + beam + composite + raycast + pathtrace | 6 | 0.55 | 6.78 | 0 |  | terrain-flight [unlit] (1.4×), far [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + direct | 3 | 0.54 | 8.05 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 3 | 0.54 | 5.70 | 0 |  |  |  |  |  |
| gpu: distance + refloor + beam + composite + raycast + pathtrace | 6 | 0.54 | 7.50 | 0 |  | showcase [unlit] (1.4×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + direct | 3 | 0.54 | 8.10 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + direct | 3 | 0.54 | 7.31 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + quadtree + gbuffer + direct | 3 | 0.54 | 8.26 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: bricks + incremental + beam + composite + raycast + direct | 3 | 0.54 | 6.58 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + direct | 3 | 0.54 | 7.35 | 0 |  |  |  |  |  |
| gpu: clipmap + rowrun + beam + composite + raycast + direct | 3 | 0.54 | 6.78 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + quadtree + visbuffer + direct | 3 | 0.54 | 8.24 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: hier + incremental + composite + raycast + direct | 3 | 0.53 | 6.59 | 0 |  |  |  |  |  |
| gpu: tree64 + incremental + beam + composite + raycast + direct | 3 | 0.53 | 4.86 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 6 | 0.53 | 6.84 | 0 |  | terrain-flight [unlit] (1.4×), far [unlit] (1.4×), showcase [unlit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + raster + direct | 3 | 0.53 | 7.26 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + direct | 3 | 0.52 | 7.33 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + raster + direct | 3 | 0.52 | 7.36 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + direct | 3 | 0.52 | 7.48 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: prefix + refloor + quadtree + gbuffer + direct | 3 | 0.52 | 8.44 | 0 |  |  |  |  |  |
| gpu: prefix + refloor + quadtree + visbuffer + direct | 3 | 0.52 | 8.42 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + direct | 3 | 0.52 | 7.03 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 3 | 0.52 | 5.73 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: hier + refloor + quadtree + visbuffer + direct | 3 | 0.52 | 8.29 | 0 |  |  |  |  |  |
| gpu: prefix + incremental + composite + raycast + direct | 3 | 0.51 | 6.70 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 3 | 0.51 | 5.66 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 6 | 0.51 | 4.87 | 0 |  | far [unlit] (1.6×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: hier + refloor + quadtree + gbuffer + direct | 3 | 0.51 | 8.33 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + direct | 3 | 0.51 | 7.33 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + direct | 3 | 0.51 | 7.41 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + direct | 3 | 0.51 | 7.30 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 3 | 0.51 | 7.47 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: hier + incremental + beam + composite + raycast + direct | 3 | 0.51 | 6.60 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + direct | 3 | 0.51 | 7.30 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 3 | 0.51 | 7.50 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + direct | 3 | 0.51 | 7.38 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 3 | 0.51 | 5.46 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + direct | 3 | 0.51 | 7.30 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + hier + refloor + depth + raster + direct | 3 | 0.50 | 7.26 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + direct | 3 | 0.50 | 7.34 | 0 |  |  |  |  |  |
| gpu: svo_dag + rowrun + composite + raycast + direct | 3 | 0.50 | 4.50 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 3 | 0.50 | 5.35 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + direct | 3 | 0.50 | 7.31 | 0 |  |  |  |  |  |
| gpu: rows + rowrun + composite + raycast + pathtrace | 6 | 0.50 | 6.82 | 0 |  | terrain-flight [unlit] (1.5×), far [unlit] (1.5×), showcase [unlit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + direct | 3 | 0.50 | 7.33 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 3 | 0.50 | 5.15 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 3 | 0.50 | 7.33 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: hier + sphere + composite + raycast + direct | 3 | 0.50 | 6.59 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 3 | 0.50 | 5.22 | 0 |  | showcase [lit] (1.6×) |  |  |  |
| gpu: rows + incremental + composite + raycast + direct | 3 | 0.50 | 6.67 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 3 | 0.50 | 7.45 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 6 | 0.50 | 7.38 | 0 |  | showcase [unlit] (1.8×), terrain-flight [unlit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 3 | 0.50 | 7.38 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: clipmap + refloor + composite + raycast + pathtrace | 6 | 0.50 | 6.90 | 0 |  | far [unlit] (1.7×), terrain-flight [unlit] (1.6×) | showcase [lit] |  |  |
| gpu: clipmap + incremental + beam + composite + raycast + direct | 3 | 0.50 | 6.78 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + refloor + depth + raster + direct | 3 | 0.49 | 7.34 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + direct | 3 | 0.49 | 7.38 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + direct | 3 | 0.49 | 7.40 | 0 |  |  |  |  |  |
| gpu: bricks + sphere + composite + raycast + direct | 3 | 0.49 | 6.57 | 0 |  | far [lit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + direct | 3 | 0.49 | 7.44 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 6 | 0.49 | 7.44 | 0 |  | showcase [unlit] (1.7×), terrain-flight [unlit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + direct | 3 | 0.49 | 7.38 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + distance + sphere + depth + raster + direct | 3 | 0.49 | 8.02 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 6 | 0.49 | 4.64 | 0 |  | showcase [unlit] (1.5×), far [unlit] (1.3×) |  |  |  |
| gpu: rows + refloor + quadtree + visbuffer + direct | 3 | 0.49 | 8.37 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + raster + direct | 3 | 0.48 | 5.40 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: distance + rowrun + composite + raycast + pathtrace | 6 | 0.48 | 7.52 | 0 |  | showcase [unlit] (1.5×), terrain-flight [unlit] (1.4×) |  |  |  |
| gpu: rows + rowrun + beam + composite + raycast + pathtrace | 6 | 0.48 | 6.82 | 0 |  | terrain-flight [unlit] (1.5×), far [unlit] (1.4×), showcase [unlit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 3 | 0.48 | 7.53 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: distance + sphere + quadtree + gbuffer + direct | 3 | 0.48 | 9.20 | 0 |  |  |  |  |  |
| gpu: hier + sphere + beam + composite + raycast + direct | 3 | 0.48 | 6.59 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: distance + sphere + quadtree + visbuffer + direct | 3 | 0.48 | 9.18 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + direct | 3 | 0.48 | 8.26 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + refloor + depth + raster + direct | 3 | 0.48 | 8.02 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: prefix + incremental + beam + composite + raycast + direct | 3 | 0.48 | 6.70 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 3 | 0.48 | 7.60 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + direct | 3 | 0.48 | 7.41 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + raster + direct | 3 | 0.48 | 6.86 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + direct | 3 | 0.48 | 8.18 | 0 |  | showcase [lit] (1.6×) |  |  |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + direct | 3 | 0.48 | 7.46 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 3 | 0.48 | 8.03 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + direct | 3 | 0.47 | 7.38 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + direct | 3 | 0.47 | 7.41 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + direct | 3 | 0.47 | 7.35 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + direct | 3 | 0.47 | 8.10 | 0 |  | showcase [lit] (1.6×) |  |  |  |
| gpu: tree64 + refloor + quadtree + gbuffer + direct | 3 | 0.47 | 6.31 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + direct | 3 | 0.47 | 8.29 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + direct | 3 | 0.47 | 7.31 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: tree64 + refloor + quadtree + visbuffer + direct | 3 | 0.47 | 6.31 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + direct | 3 | 0.47 | 7.44 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: prefix + rowrun + quadtree + visbuffer + direct | 3 | 0.47 | 8.42 | 0 |  |  |  |  |  |
| gpu: rows + refloor + quadtree + gbuffer + direct | 3 | 0.47 | 8.40 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 3 | 0.47 | 7.99 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + direct | 3 | 0.47 | 8.09 | 0 |  | showcase [lit] (1.6×) |  |  |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + raster + direct | 3 | 0.47 | 7.83 | 0 |  |  |  |  |  |
| gpu: rows + incremental + beam + composite + raycast + direct | 3 | 0.47 | 6.68 | 0 |  |  |  |  |  |
| gpu: prefix + rowrun + quadtree + gbuffer + direct | 3 | 0.47 | 8.45 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + direct | 3 | 0.47 | 7.42 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 3 | 0.47 | 7.53 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 3 | 0.47 | 5.40 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 3 | 0.47 | 7.60 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 3 | 0.47 | 5.76 | 0 |  |  |  |  |  |
| gpu: bricks + sphere + beam + composite + raycast + direct | 3 | 0.47 | 6.58 | 0 |  | far [lit] (1.5×) |  |  |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 3 | 0.47 | 7.57 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + direct | 3 | 0.47 | 5.54 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 3 | 0.47 | 5.64 | 0 |  |  |  |  |  |
| gpu: bricks + rowrun + composite + raycast + pathtrace | 6 | 0.47 | 6.75 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.7×), showcase [unlit] (1.4×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + direct | 3 | 0.47 | 8.15 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: rows + rowrun + quadtree + gbuffer + direct | 3 | 0.46 | 8.40 | 0 |  |  |  |  |  |
| gpu: prefix + sphere + composite + raycast + direct | 3 | 0.46 | 6.70 | 0 |  | far [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + raster + direct | 3 | 0.46 | 7.12 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + direct | 3 | 0.46 | 8.19 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: distance + rowrun + beam + composite + raycast + pathtrace | 6 | 0.46 | 7.53 | 0 |  | showcase [unlit] (1.5×), terrain-flight [unlit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 3 | 0.46 | 5.97 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 3 | 0.46 | 5.69 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + direct | 3 | 0.46 | 8.11 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + direct | 3 | 0.46 | 7.31 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 3 | 0.46 | 5.44 | 0 |  |  |  |  |  |
| gpu: rows + rowrun + quadtree + visbuffer + direct | 3 | 0.46 | 8.36 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + raster + direct | 3 | 0.46 | 7.22 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 3 | 0.46 | 5.72 | 0 |  |  |  |  |  |
| gpu: distance + refloor + quadtree + gbuffer + direct | 3 | 0.46 | 9.20 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + raster + direct | 3 | 0.46 | 7.34 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: distance + refloor + quadtree + visbuffer + direct | 3 | 0.46 | 9.16 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + direct | 3 | 0.46 | 7.41 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 3 | 0.46 | 5.66 | 0 |  |  |  |  |  |
| gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 6 | 0.45 | 6.75 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.6×), showcase [unlit] (1.4×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + direct | 3 | 0.45 | 7.35 | 0 |  |  |  |  |  |
| gpu: clipmap + refloor + quadtree + visbuffer + direct | 3 | 0.45 | 8.42 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + direct | 3 | 0.45 | 7.38 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: rows + sphere + composite + raycast + direct | 3 | 0.45 | 6.67 | 0 |  | far [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + direct | 3 | 0.44 | 5.18 | 0 |  |  |  |  |  |
| gpu: prefix + sphere + beam + composite + raycast + direct | 3 | 0.44 | 6.70 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: clipmap + refloor + quadtree + gbuffer + direct | 3 | 0.44 | 8.46 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + raster + direct | 3 | 0.44 | 7.91 | 0 |  | showcase [lit] (1.5×) |  |  |  |
| gpu: svo_dag + rowrun + beam + composite + raycast + direct | 3 | 0.44 | 4.63 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + raster + direct | 3 | 0.44 | 7.20 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 3 | 0.44 | 5.17 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: distance + incremental + composite + raycast + direct | 3 | 0.44 | 7.53 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + direct | 3 | 0.44 | 5.00 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 6 | 0.44 | 6.92 | 0 |  | far [unlit] (1.6×), terrain-flight [unlit] (1.4×), showcase [unlit] (1.3×) |  |  |  |
| gpu: bricks + rowrun + quadtree + gbuffer + direct | 3 | 0.44 | 8.33 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 3 | 0.44 | 5.66 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + direct | 3 | 0.43 | 7.30 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 3 | 0.43 | 5.76 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 3 | 0.43 | 5.22 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: bricks + rowrun + quadtree + visbuffer + direct | 3 | 0.43 | 8.30 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: svo_dag + refloor + quadtree + visbuffer + direct | 3 | 0.43 | 6.01 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + raster + direct | 3 | 0.43 | 7.91 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + direct | 3 | 0.43 | 7.37 | 0 |  |  |  |  |  |
| gpu: svo_dag + refloor + quadtree + gbuffer + direct | 3 | 0.43 | 6.05 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + direct | 3 | 0.43 | 7.28 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: rows + sphere + beam + composite + raycast + direct | 3 | 0.43 | 6.67 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: distance + rowrun + quadtree + visbuffer + direct | 3 | 0.43 | 9.21 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + direct | 3 | 0.43 | 7.30 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + direct | 3 | 0.43 | 7.36 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + direct | 3 | 0.43 | 7.34 | 0 |  |  |  |  |  |
| gpu: svo_dag + sphere + composite + raycast + direct | 3 | 0.43 | 4.50 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + raster + direct | 3 | 0.43 | 7.38 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: distance + rowrun + quadtree + gbuffer + direct | 3 | 0.43 | 9.23 | 0 |  |  |  |  |  |
| gpu: naive@cpu + rayquery + raycast + direct | 3 | 0.43 | 13.22 | 0 |  | showcase [lit] (1.4×) |  |  | far [lit] |
| gpu: bricks + incremental + composite + raycast + pathtrace | 6 | 0.43 | 6.74 | 0 |  | far [unlit] (2.1×), terrain-flight [unlit] (1.9×), showcase [unlit] (1.3×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + raster + direct | 3 | 0.42 | 7.21 | 0 |  |  |  |  |  |
| gpu: hier + rowrun + composite + raycast + pathtrace | 6 | 0.42 | 6.75 | 0 |  | showcase [unlit] (1.8×), far [unlit] (1.5×), terrain-flight [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + rows + incremental + depth + raster + direct | 3 | 0.42 | 7.35 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + direct | 3 | 0.42 | 7.48 | 0 |  |  |  |  |  |
| gpu: hier + incremental + composite + raycast + pathtrace | 6 | 0.42 | 6.76 | 0 |  | terrain-flight [unlit] (1.7×), far [unlit] (1.7×), showcase [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + direct | 3 | 0.41 | 7.24 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + direct | 3 | 0.41 | 7.56 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: tree64 + incremental + composite + raycast + pathtrace | 6 | 0.41 | 5.07 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.6×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + direct | 3 | 0.41 | 7.37 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + direct | 3 | 0.41 | 7.52 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + raster + direct | 3 | 0.41 | 7.26 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + direct | 3 | 0.41 | 7.41 | 0 |  |  |  |  |  |
| gpu: distance + incremental + beam + composite + raycast + direct | 3 | 0.41 | 7.52 | 0 |  |  |  |  |  |
| gpu: bricks + incremental + beam + composite + raycast + pathtrace | 6 | 0.41 | 6.76 | 0 |  | far [unlit] (2.0×), terrain-flight [unlit] (1.8×), showcase [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + direct | 3 | 0.41 | 7.45 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + direct | 3 | 0.41 | 8.19 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + direct | 3 | 0.41 | 7.38 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 6 | 0.41 | 7.26 | 0 |  | showcase [unlit] (1.6×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: clipmap + rowrun + quadtree + visbuffer + direct | 3 | 0.41 | 8.44 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + direct | 3 | 0.41 | 8.19 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 6 | 0.41 | 7.31 | 0 |  | showcase [unlit] (1.8×), terrain-flight [unlit] (1.3×) |  |  |  |
| gpu: boxes@cpu + rayquery + raycast + pathtrace | 6 | 0.41 | 8.70 | 0 |  | terrain-flight [unlit] (1.5×) |  |  |  |
| gpu: naive@cpu + faces + hier + incremental + depth + raster + direct | 3 | 0.41 | 7.27 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 3 | 0.40 | 7.66 | 0 |  |  |  |  |  |
| gpu: clipmap + rowrun + quadtree + gbuffer + direct | 3 | 0.40 | 8.47 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 3 | 0.40 | 8.30 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + direct | 3 | 0.40 | 7.50 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + raster + direct | 3 | 0.40 | 8.03 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: clipmap + rowrun + composite + raycast + pathtrace | 6 | 0.40 | 6.93 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.6×), showcase [unlit] (1.3×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 3 | 0.40 | 7.58 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 3 | 0.40 | 8.34 | 0 |  |  |  |  |  |
| gpu: hier + incremental + beam + composite + raycast + pathtrace | 6 | 0.40 | 6.77 | 0 |  | terrain-flight [unlit] (1.7×), far [unlit] (1.6×), showcase [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix + refloor + quadtree + raster + direct | 3 | 0.40 | 8.40 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 3 | 0.40 | 7.50 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 3 | 0.40 | 5.44 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 3 | 0.40 | 5.16 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 3 | 0.40 | 5.21 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + quadtree + raster + direct | 3 | 0.40 | 8.03 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 3 | 0.40 | 7.69 | 0 |  |  |  |  |  |
| gpu: hier + rowrun + beam + composite + raycast + pathtrace | 6 | 0.40 | 6.76 | 0 |  | showcase [unlit] (1.8×), far [unlit] (1.5×), terrain-flight [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 3 | 0.40 | 5.35 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 3 | 0.40 | 5.42 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 3 | 0.39 | 5.43 | 0 |  |  |  |  |  |
| gpu: hier + rowrun + quadtree + visbuffer + direct | 3 | 0.39 | 8.38 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + direct | 3 | 0.39 | 7.51 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: clipmap + incremental + quadtree + visbuffer + direct | 3 | 0.39 | 8.44 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + direct | 3 | 0.39 | 7.58 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: hier + rowrun + quadtree + gbuffer + direct | 3 | 0.39 | 8.42 | 0 |  |  |  |  |  |
| gpu: rows + incremental + beam + composite + raycast + pathtrace | 6 | 0.39 | 6.85 | 0 |  | terrain-flight [unlit] (1.7×), far [unlit] (1.7×), showcase [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + refloor + quadtree + raster + direct | 3 | 0.39 | 8.28 | 0 |  |  |  |  |  |
| gpu: clipmap + sphere + composite + raycast + direct | 3 | 0.39 | 6.78 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 3 | 0.39 | 7.55 | 0 |  |  |  |  |  |
| gpu: prefix + incremental + composite + raycast + pathtrace | 6 | 0.39 | 6.87 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.8×), showcase [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 6 | 0.39 | 4.91 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + incremental + quadtree + gbuffer + direct | 3 | 0.39 | 8.46 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: clipmap + incremental + composite + raycast + pathtrace | 6 | 0.39 | 6.94 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×), showcase [unlit] (1.3×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + composite + raycast + direct | 3 | 0.39 | 5.00 | 0 |  | far [lit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 3 | 0.39 | 7.60 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + raster + direct | 3 | 0.39 | 7.22 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + raster + direct | 3 | 0.39 | 7.38 | 0 |  |  |  |  |  |
| gpu: distance + sphere + quadtree + raster + direct | 3 | 0.38 | 9.15 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 6 | 0.38 | 7.33 | 1 | showcase [unlit] | showcase [unlit] (2.6×), terrain-flight [unlit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + raster + direct | 3 | 0.38 | 7.92 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + raster + direct | 3 | 0.38 | 7.13 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + raster + direct | 3 | 0.38 | 7.12 | 0 |  |  |  |  |  |
| gpu: svo_dag + sphere + beam + composite + raycast + direct | 3 | 0.38 | 4.60 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: hier + sphere + quadtree + gbuffer + direct | 3 | 0.38 | 8.33 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: tree64 + incremental + quadtree + gbuffer + direct | 3 | 0.38 | 6.45 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: bricks + incremental + quadtree + visbuffer + direct | 3 | 0.38 | 8.40 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: rows + incremental + composite + raycast + pathtrace | 6 | 0.38 | 6.84 | 0 |  | terrain-flight [unlit] (1.8×), far [unlit] (1.7×), showcase [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + raster + direct | 3 | 0.38 | 7.86 | 0 |  |  |  |  |  |
| gpu: bricks + incremental + quadtree + gbuffer + direct | 3 | 0.38 | 8.46 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: tree64 + incremental + quadtree + visbuffer + direct | 3 | 0.38 | 6.40 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: svo_dag + incremental + composite + raycast + direct | 3 | 0.38 | 4.62 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: bricks + sphere + quadtree + visbuffer + direct | 3 | 0.38 | 8.29 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: prefix + incremental + beam + composite + raycast + pathtrace | 6 | 0.38 | 6.89 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.7×), showcase [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + sphere + quadtree + gbuffer + direct | 3 | 0.38 | 8.32 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 3 | 0.38 | 5.98 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 3 | 0.38 | 5.18 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 3 | 0.38 | 8.31 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + direct | 3 | 0.37 | 7.38 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 6 | 0.37 | 6.40 | 0 |  | far [unlit] (1.7×), terrain-flight [unlit] (1.5×) | showcase [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 3 | 0.37 | 6.07 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + raster + direct | 3 | 0.37 | 5.42 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + direct | 3 | 0.37 | 7.84 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 3 | 0.37 | 5.24 | 0 |  |  |  |  |  |
| gpu: hier + sphere + quadtree + visbuffer + direct | 3 | 0.37 | 8.30 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 6 | 0.37 | 6.46 | 0 |  | far [unlit] (1.7×), terrain-flight [unlit] (1.5×) | showcase [lit] |  |  |
| gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 6 | 0.37 | 8.44 | 0 |  | far [unlit] (1.6×), far [lit] (1.3×) |  |  |  |
| gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 6 | 0.37 | 8.33 | 0 |  | far [unlit] (1.8×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 6 | 0.37 | 8.48 | 0 |  | far [unlit] (1.6×), far [lit] (1.3×) |  |  |  |
| gpu: tree64 + sphere + composite + raycast + direct | 3 | 0.37 | 4.87 | 0 |  | far [lit] (1.6×) | showcase [lit] |  |  |
| gpu: hier + incremental + quadtree + visbuffer + direct | 3 | 0.37 | 8.45 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + raster + direct | 3 | 0.37 | 7.23 | 0 |  |  |  |  |  |
| gpu: distance + refloor + quadtree + raster + direct | 3 | 0.37 | 9.18 | 0 |  |  |  |  |  |
| gpu: hier + refloor + quadtree + visbuffer + pathtrace | 6 | 0.37 | 8.34 | 0 |  | far [unlit] (1.6×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + direct | 3 | 0.37 | 7.34 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 6 | 0.37 | 8.13 | 0 |  | terrain-flight [unlit] (1.6×), far [unlit] (1.5×), far [lit] (1.3×) | showcase [lit], showcase [unlit] |  |  |
| gpu: hier + incremental + quadtree + gbuffer + direct | 3 | 0.36 | 8.47 | 0 |  |  |  |  |  |
| gpu: rows + refloor + quadtree + raster + direct | 3 | 0.36 | 8.37 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + hier + sphere + depth + raster + direct | 3 | 0.36 | 7.27 | 0 |  |  |  |  |  |
| gpu: prefix + incremental + quadtree + visbuffer + direct | 3 | 0.36 | 8.58 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + direct | 3 | 0.36 | 5.28 | 0 |  |  |  |  |  |
| gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 6 | 0.36 | 8.39 | 0 |  | far [unlit] (1.9×), far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 3 | 0.36 | 8.34 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + direct | 3 | 0.36 | 7.35 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + incremental + depth + raster + direct | 3 | 0.36 | 8.03 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: hier + refloor + quadtree + gbuffer + pathtrace | 6 | 0.36 | 8.39 | 0 |  | far [unlit] (1.6×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: tree64 + rowrun + beam + composite + raycast + direct | 3 | 0.36 | 4.86 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 6 | 0.36 | 6.94 | 0 |  | far [unlit] (1.7×), showcase [unlit] (1.4×), terrain-flight [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + raster + direct | 3 | 0.36 | 7.27 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 3 | 0.36 | 5.96 | 0 |  |  |  |  |  |
| gpu: prefix + incremental + quadtree + gbuffer + direct | 3 | 0.36 | 8.69 | 0 |  |  |  |  |  |
| gpu: rows + incremental + quadtree + gbuffer + direct | 3 | 0.36 | 8.56 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 3 | 0.36 | 7.67 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 3 | 0.36 | 6.03 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + direct | 3 | 0.36 | 8.29 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: distance + sphere + quadtree + visbuffer + pathtrace | 6 | 0.36 | 9.15 | 0 |  | far [unlit] (1.3×) |  |  |  |
| gpu: prefix + sphere + quadtree + visbuffer + direct | 3 | 0.36 | 8.52 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: rows + incremental + quadtree + visbuffer + direct | 3 | 0.36 | 8.53 | 0 |  |  |  |  |  |
| gpu: prefix + sphere + quadtree + gbuffer + direct | 3 | 0.36 | 8.52 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 3 | 0.36 | 8.45 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + direct | 3 | 0.36 | 8.21 | 0 |  | showcase [lit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 6 | 0.36 | 7.10 | 0 |  | showcase [unlit] (2.4×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: distance + sphere + quadtree + gbuffer + pathtrace | 6 | 0.36 | 9.20 | 0 |  | far [unlit] (1.4×) |  |  |  |
| gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 3 | 0.35 | 7.62 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 3 | 0.35 | 7.72 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 3 | 0.35 | 8.38 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: hier + sphere + composite + raycast + pathtrace | 6 | 0.35 | 6.74 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.8×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + beam + composite + raycast + direct | 3 | 0.35 | 6.88 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 3 | 0.35 | 7.65 | 0 |  |  |  |  |  |
| gpu: rows + sphere + quadtree + visbuffer + direct | 3 | 0.35 | 8.50 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: naive@cpu + rayquery + raycast + pathtrace | 6 | 0.35 | 12.99 | 0 |  | showcase [lit] (1.3×) |  |  | far [lit], far [unlit] |
| gpu: svo_dag + incremental + beam + composite + raycast + direct | 3 | 0.35 | 4.64 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + raster + direct | 3 | 0.35 | 7.28 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + raster + direct | 3 | 0.35 | 7.26 | 0 |  |  |  |  |  |
| gpu: rows + sphere + quadtree + gbuffer + direct | 3 | 0.35 | 8.53 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 6 | 0.35 | 8.16 | 0 |  | terrain-flight [unlit] (1.7×), far [lit] (1.3×), far [unlit] (1.3×) | showcase [lit], showcase [unlit] |  |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + direct | 3 | 0.35 | 8.12 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + raster + direct | 3 | 0.35 | 7.93 | 0 |  |  |  |  |  |
| gpu: rows + refloor + quadtree + visbuffer + pathtrace | 6 | 0.35 | 8.44 | 0 |  | far [unlit] (1.6×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: tree64 + refloor + quadtree + raster + direct | 3 | 0.34 | 6.27 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + direct | 3 | 0.34 | 7.67 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: hier + sphere + beam + composite + raycast + pathtrace | 6 | 0.34 | 6.75 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + raster + direct | 3 | 0.34 | 7.12 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 3 | 0.34 | 7.59 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + raster + direct | 3 | 0.34 | 7.37 | 0 |  |  |  |  |  |
| gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 6 | 0.34 | 8.42 | 0 |  | far [unlit] (1.7×), terrain-flight [unlit] (1.7×) | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + direct | 3 | 0.34 | 7.74 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + direct | 3 | 0.34 | 7.66 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 6 | 0.34 | 6.05 | 0 |  | terrain-flight [unlit] (1.5×), far [unlit] (1.5×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + raster + direct | 3 | 0.34 | 7.11 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + direct | 3 | 0.34 | 7.69 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 3 | 0.34 | 7.68 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + direct | 3 | 0.34 | 7.57 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: distance + incremental + composite + raycast + pathtrace | 6 | 0.34 | 7.53 | 0 |  | terrain-flight [unlit] (1.8×), showcase [unlit] (1.6×), far [unlit] (1.6×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 6 | 0.34 | 6.08 | 0 |  | far [unlit] (1.5×), terrain-flight [unlit] (1.5×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 3 | 0.34 | 7.66 | 0 |  |  |  |  |  |
| gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 6 | 0.34 | 8.45 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.7×) | showcase [lit], showcase [unlit] |  |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 3 | 0.34 | 7.71 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: rows + refloor + quadtree + gbuffer + pathtrace | 6 | 0.34 | 8.48 | 0 |  | far [unlit] (1.7×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + direct | 3 | 0.34 | 7.57 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 3 | 0.34 | 7.75 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: distance + refloor + quadtree + visbuffer + pathtrace | 6 | 0.34 | 9.18 | 0 |  | far [unlit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + direct | 3 | 0.34 | 7.12 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 3 | 0.34 | 7.71 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + sphere + depth + raster + direct | 3 | 0.34 | 7.34 | 0 |  |  |  |  |  |
| gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 6 | 0.34 | 6.96 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.5×), showcase [unlit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: distance + refloor + quadtree + gbuffer + pathtrace | 6 | 0.33 | 9.21 | 0 |  | far [unlit] (1.4×) |  |  |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + raster + direct | 3 | 0.33 | 7.13 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + direct | 3 | 0.33 | 8.39 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 3 | 0.33 | 8.38 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + direct | 3 | 0.33 | 7.73 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + direct | 3 | 0.33 | 7.68 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: tree64 + sphere + beam + composite + raycast + direct | 3 | 0.33 | 4.90 | 0 |  | far [lit] (1.6×) | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 3 | 0.33 | 5.56 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + direct | 3 | 0.33 | 8.45 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: rows + rowrun + quadtree + raster + direct | 3 | 0.33 | 8.39 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + raster + direct | 3 | 0.33 | 7.23 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + direct | 3 | 0.33 | 8.39 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: distance + incremental + beam + composite + raycast + pathtrace | 6 | 0.33 | 7.54 | 0 |  | terrain-flight [unlit] (1.7×), far [unlit] (1.6×), showcase [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + direct | 3 | 0.33 | 7.69 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 3 | 0.33 | 8.02 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: bricks + sphere + composite + raycast + pathtrace | 6 | 0.33 | 6.73 | 0 |  | far [unlit] (2.3×), terrain-flight [unlit] (1.9×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + quadtree + visbuffer + direct | 3 | 0.33 | 8.51 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + direct | 3 | 0.33 | 8.46 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 3 | 0.33 | 5.67 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 3 | 0.33 | 8.40 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 3 | 0.33 | 7.95 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + raster + direct | 3 | 0.32 | 7.20 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 3 | 0.32 | 8.33 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + raster + direct | 3 | 0.32 | 7.83 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 3 | 0.32 | 5.72 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 3 | 0.32 | 5.79 | 0 |  | showcase [lit] (2.1×) | far [lit] |  |  |
| gpu: clipmap + refloor + quadtree + raster + direct | 3 | 0.32 | 8.43 | 0 |  |  |  |  |  |
| gpu: svo_dag + refloor + quadtree + raster + direct | 3 | 0.32 | 6.05 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 3 | 0.32 | 7.68 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 3 | 0.32 | 5.72 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 3 | 0.32 | 5.42 | 0 |  | showcase [lit] (1.3×) |  |  |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + direct | 3 | 0.32 | 7.73 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 3 | 0.32 | 7.64 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: prefix + sphere + composite + raycast + pathtrace | 6 | 0.32 | 6.85 | 0 |  | far [unlit] (2.0×), terrain-flight [unlit] (1.8×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 3 | 0.32 | 5.72 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + direct | 3 | 0.32 | 8.39 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 3 | 0.32 | 7.70 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: bricks + sphere + beam + composite + raycast + pathtrace | 6 | 0.32 | 6.74 | 0 |  | far [unlit] (2.3×), terrain-flight [unlit] (1.8×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + quadtree + gbuffer + direct | 3 | 0.32 | 8.54 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: prefix + rowrun + quadtree + raster + direct | 3 | 0.32 | 8.48 | 0 |  | far [lit] (1.3×) |  |  |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + direct | 3 | 0.32 | 8.44 | 0 |  | showcase [lit] (2.0×) | far [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + direct | 3 | 0.32 | 5.55 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 3 | 0.32 | 8.26 | 0 |  |  |  |  |  |
| gpu: svo_dag + rowrun + composite + raycast + pathtrace | 6 | 0.32 | 4.69 | 0 |  | showcase [unlit] (2.1×), far [unlit] (1.5×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 3 | 0.32 | 5.48 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + direct | 3 | 0.32 | 7.34 | 0 |  | terrain-flight [lit] (1.3×) |  |  |  |
| gpu: prefix + sphere + beam + composite + raycast + pathtrace | 6 | 0.31 | 6.86 | 0 |  | far [unlit] (2.0×), terrain-flight [unlit] (1.8×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 3 | 0.31 | 7.99 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: distance + incremental + quadtree + gbuffer + direct | 3 | 0.31 | 9.34 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 3 | 0.31 | 7.66 | 0 |  |  |  |  |  |
| gpu: rows + sphere + composite + raycast + pathtrace | 6 | 0.31 | 6.82 | 0 |  | far [unlit] (2.0×), terrain-flight [unlit] (1.8×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 3 | 0.31 | 8.33 | 0 |  |  |  |  |  |
| gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.31 | 8.49 | 0 |  | far [unlit] (1.8×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: distance + incremental + quadtree + visbuffer + direct | 3 | 0.31 | 9.30 | 0 |  |  |  |  |  |
| gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.31 | 8.52 | 0 |  | far [unlit] (1.8×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 3 | 0.31 | 8.07 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 3 | 0.31 | 7.74 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + rayquery + raycast + direct | 3 | 0.31 | 10.19 | 0 |  | showcase [lit] (1.7×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + direct | 3 | 0.31 | 7.58 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + direct | 3 | 0.31 | 5.01 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 3 | 0.31 | 8.35 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + direct | 3 | 0.31 | 5.20 | 0 |  |  |  |  |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 3 | 0.31 | 7.68 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 3 | 0.31 | 8.00 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.31 | 8.53 | 0 |  | far [unlit] (1.8×) | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + direct | 3 | 0.31 | 7.64 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 3 | 0.31 | 8.07 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: svo_dag + sphere + quadtree + gbuffer + direct | 3 | 0.30 | 6.15 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: svo_dag + sphere + quadtree + visbuffer + direct | 3 | 0.30 | 6.13 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.30 | 8.49 | 0 |  | far [unlit] (1.7×), far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + raster + direct | 3 | 0.30 | 5.37 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 3 | 0.30 | 7.75 | 0 |  |  |  |  |  |
| gpu: rows + sphere + beam + composite + raycast + pathtrace | 6 | 0.30 | 6.83 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.8×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 3 | 0.30 | 5.96 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 3 | 0.30 | 7.75 | 0 |  |  |  |  |  |
| gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.30 | 9.24 | 0 |  | far [unlit] (1.5×) | showcase [lit] |  |  |
| gpu: tree64 + sphere + quadtree + visbuffer + direct | 3 | 0.30 | 6.57 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: points@cpu + splat + bricks + refloor + direct | 3 | 0.30 | 8.33 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: tree64 + sphere + quadtree + gbuffer + direct | 3 | 0.30 | 6.47 | 0 |  | far [lit] (1.4×) | showcase [lit] |  |  |
| gpu: points@cpu + splat + prefix + refloor + direct | 3 | 0.30 | 8.93 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 3 | 0.30 | 5.73 | 0 |  | showcase [lit] (1.7×) | far [lit] |  |  |
| gpu: svo_dag + sphere + composite + raycast + pathtrace | 6 | 0.29 | 4.68 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 3 | 0.29 | 8.44 | 0 |  |  | far [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 3 | 0.29 | 5.74 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: rows + incremental + quadtree + raster + direct | 3 | 0.29 | 8.46 | 0 |  |  | showcase [lit] |  |  |
| gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.29 | 9.27 | 0 |  | far [unlit] (1.5×) | showcase [lit] |  |  |
| gpu: distance + rowrun + quadtree + raster + direct | 3 | 0.29 | 9.24 | 0 |  |  |  |  |  |
| gpu: points@cpu + splat + prefix + rowrun + direct | 3 | 0.29 | 8.97 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 3 | 0.29 | 5.79 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + hier + refloor + direct | 3 | 0.29 | 8.81 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: points@cpu + splat + tree64 + refloor + direct | 3 | 0.29 | 7.48 | 0 |  | showcase [lit] (1.7×) | far [lit] |  |  |
| gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 6 | 0.29 | 4.75 | 0 |  | showcase [unlit] (2.0×), far [unlit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 6 | 0.29 | 6.45 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.5×) | showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 6 | 0.29 | 6.51 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.5×) | showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + direct | 3 | 0.29 | 7.60 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + refloor + direct | 3 | 0.29 | 7.35 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + direct | 3 | 0.29 | 7.65 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + direct | 3 | 0.29 | 7.56 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.29 | 8.47 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + rows + refloor + direct | 3 | 0.29 | 8.91 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + direct | 3 | 0.29 | 7.68 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.29 | 8.51 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + distance + sphere + direct | 3 | 0.29 | 9.68 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + direct | 3 | 0.28 | 7.59 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + rows + rowrun + direct | 3 | 0.28 | 8.91 | 0 |  | showcase [lit] (1.7×) | far [lit] |  |  |
| gpu: points@cpu + splat + bricks + rowrun + direct | 3 | 0.28 | 8.86 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: points@cpu + splat + clipmap + refloor + direct | 3 | 0.28 | 9.54 | 0 |  | showcase [lit] (1.7×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + direct | 3 | 0.28 | 7.69 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 3 | 0.28 | 7.76 | 0 |  |  |  |  |  |
| gpu: hier + incremental + quadtree + raster + direct | 3 | 0.28 | 8.40 | 0 |  |  | showcase [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 3 | 0.28 | 7.83 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + direct | 3 | 0.28 | 7.57 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: points@cpu + splat + distance + refloor + direct | 3 | 0.28 | 9.68 | 0 |  | showcase [lit] (1.9×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + direct | 3 | 0.28 | 7.79 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + direct | 3 | 0.28 | 7.64 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + direct | 3 | 0.28 | 7.75 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + direct | 3 | 0.28 | 7.71 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 3 | 0.28 | 6.07 | 0 |  |  |  |  |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + direct | 3 | 0.28 | 7.68 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 3 | 0.28 | 7.58 | 0 |  |  | showcase [lit] |  |  |
| gpu: points@cpu + splat + clipmap + rowrun + direct | 3 | 0.28 | 9.56 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 3 | 0.28 | 6.00 | 0 |  |  |  |  |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 3 | 0.28 | 8.47 | 0 |  |  | far [lit] |  |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 3 | 0.28 | 7.65 | 0 |  |  | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 6 | 0.28 | 7.44 | 0 |  | showcase [unlit] (2.5×) | far [lit], far [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 3 | 0.28 | 7.73 | 0 |  |  |  |  |  |
| gpu: bricks + incremental + quadtree + raster + direct | 3 | 0.28 | 8.42 | 0 |  |  | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + direct | 3 | 0.28 | 7.74 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + direct | 3 | 0.28 | 7.70 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + direct | 3 | 0.28 | 7.69 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 3 | 0.28 | 7.69 | 0 |  |  |  |  |  |
| gpu: points@cpu + splat + distance + rowrun + direct | 3 | 0.28 | 9.68 | 0 |  | showcase [lit] (1.8×) | far [lit] |  |  |
| gpu: svo_dag + incremental + quadtree + visbuffer + direct | 3 | 0.28 | 6.24 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: svo_dag + incremental + quadtree + gbuffer + direct | 3 | 0.28 | 6.28 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 6 | 0.28 | 8.49 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + pathtrace | 6 | 0.28 | 10.35 | 0 |  | showcase [lit] (1.6×), showcase [unlit] (1.4×) | far [lit], far [unlit] |  |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + direct | 3 | 0.27 | 7.81 | 0 |  | showcase [lit] (1.8×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + clipmap + incremental + direct | 3 | 0.27 | 9.56 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 6 | 0.27 | 8.51 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 6 | 0.27 | 7.54 | 0 |  | showcase [unlit] (2.5×) | far [lit], far [unlit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + hier + rowrun + direct | 3 | 0.27 | 8.81 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: svo_dag + rowrun + quadtree + visbuffer + direct | 3 | 0.27 | 6.25 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: svo_dag + rowrun + quadtree + gbuffer + direct | 3 | 0.27 | 6.29 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: points@cpu + splat + tree64 + incremental + direct | 3 | 0.27 | 7.55 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 3 | 0.27 | 5.78 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 3 | 0.27 | 5.72 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 3 | 0.27 | 8.03 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 6 | 0.27 | 4.70 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.5×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.27 | 8.44 | 0 |  | far [unlit] (2.1×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: clipmap + sphere + composite + raycast + pathtrace | 6 | 0.27 | 6.93 | 0 |  | far [unlit] (2.3×), terrain-flight [unlit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.27 | 8.47 | 0 |  | far [unlit] (2.1×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + direct | 3 | 0.27 | 5.01 | 0 |  |  | far [lit] |  |  |
| gpu: svo_dag + incremental + composite + raycast + pathtrace | 6 | 0.27 | 4.70 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×), showcase [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 3 | 0.26 | 7.96 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + direct | 3 | 0.26 | 8.41 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: prefix + incremental + quadtree + raster + direct | 3 | 0.26 | 8.52 | 0 |  |  | showcase [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 3 | 0.26 | 8.32 | 0 |  |  | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + direct | 3 | 0.26 | 8.46 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + raster + pathtrace | 3 | 0.26 | 8.09 | 0 |  |  | far [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 3 | 0.26 | 8.40 | 0 |  |  | showcase [lit] |  |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 3 | 0.26 | 8.57 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: hier + incremental + quadtree + gbuffer + pathtrace | 6 | 0.26 | 8.51 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 3 | 0.26 | 8.64 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: hier + incremental + quadtree + visbuffer + pathtrace | 6 | 0.26 | 8.46 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 3 | 0.26 | 5.93 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 3 | 0.26 | 5.76 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: rows + incremental + quadtree + visbuffer + pathtrace | 6 | 0.26 | 8.55 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + bricks + incremental + direct | 3 | 0.26 | 8.88 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 3 | 0.26 | 5.75 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 3 | 0.26 | 5.82 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 3 | 0.26 | 7.58 | 0 |  |  | terrain-flight [lit] |  |  |
| gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 6 | 0.26 | 8.44 | 0 |  | far [unlit] (2.3×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: distance + sphere + quadtree + raster + pathtrace | 6 | 0.26 | 9.33 | 0 |  | far [unlit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 3 | 0.26 | 5.83 | 0 |  |  | terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + hier + sphere + direct | 3 | 0.26 | 8.79 | 0 |  | showcase [lit] (1.5×) | far [lit] |  |  |
| gpu: rows + incremental + quadtree + gbuffer + pathtrace | 6 | 0.26 | 8.57 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 3 | 0.26 | 5.80 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + rowrun + direct | 3 | 0.26 | 7.37 | 0 |  | showcase [lit] (1.6×) | far [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 3 | 0.25 | 5.83 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + prefix + incremental + direct | 3 | 0.25 | 9.02 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + direct | 3 | 0.25 | 5.56 | 0 |  |  | terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + distance + sphere + depth + raster + pathtrace | 3 | 0.25 | 8.19 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 6 | 0.25 | 8.47 | 0 |  | far [unlit] (2.2×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 3 | 0.25 | 5.61 | 0 |  |  | terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + raster + pathtrace | 3 | 0.25 | 7.40 | 0 |  |  | far [lit] |  |  |
| gpu: distance + incremental + quadtree + raster + direct | 3 | 0.25 | 9.27 | 0 |  |  | showcase [lit] |  |  |
| gpu: tree64 + rowrun + composite + raycast + pathtrace | 6 | 0.25 | 4.95 | 0 |  | far [unlit] (2.0×), showcase [unlit] (1.9×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 3 | 0.25 | 7.62 | 0 |  |  | terrain-flight [lit] |  |  |
| gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 6 | 0.25 | 4.72 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.5×), showcase [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + prefix + sphere + direct | 3 | 0.25 | 8.93 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: points@cpu + splat + hier + incremental + direct | 3 | 0.25 | 8.89 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 3 | 0.25 | 7.58 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + bricks + sphere + direct | 3 | 0.25 | 8.79 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 6 | 0.25 | 8.23 | 0 |  | far [unlit] (2.5×), terrain-flight [unlit] (2.5×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 3 | 0.25 | 7.69 | 0 |  |  | showcase [lit] |  |  |
| gpu: tree64 + sphere + composite + raycast + pathtrace | 6 | 0.25 | 4.93 | 0 |  | far [unlit] (2.5×), terrain-flight [unlit] (1.8×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 3 | 0.25 | 7.86 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: prefix + refloor + quadtree + raster + pathtrace | 6 | 0.25 | 8.65 | 0 |  | far [unlit] (2.4×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 3 | 0.25 | 7.62 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 6 | 0.25 | 8.59 | 0 |  | far [unlit] (2.1×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + rows + sphere + direct | 3 | 0.25 | 8.90 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 3 | 0.25 | 8.33 | 0 |  |  | showcase [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 3 | 0.25 | 8.40 | 0 |  |  | showcase [lit] |  |  |
| gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 6 | 0.25 | 8.64 | 0 |  | far [unlit] (2.1×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + rows + incremental + direct | 3 | 0.25 | 8.99 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + raster + pathtrace | 3 | 0.25 | 7.30 | 0 |  |  | far [lit] |  |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 3 | 0.25 | 7.58 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 3 | 0.25 | 7.75 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 3 | 0.25 | 7.93 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 3 | 0.25 | 7.67 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 3 | 0.24 | 8.57 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + bricks + refloor + pathtrace | 6 | 0.24 | 8.69 | 0 |  | showcase [unlit] (1.9×) | far [lit], far [unlit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 6 | 0.24 | 6.94 | 0 |  | far [unlit] (2.2×), terrain-flight [unlit] (1.7×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 3 | 0.24 | 7.61 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + sphere + direct | 3 | 0.24 | 7.36 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: tree64 + refloor + quadtree + raster + pathtrace | 6 | 0.24 | 6.58 | 0 |  | far [unlit] (2.6×), terrain-flight [unlit] (2.0×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + refloor + quadtree + raster + pathtrace | 6 | 0.24 | 8.55 | 0 |  | far [unlit] (2.4×), terrain-flight [unlit] (1.3×), showcase [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + tree64 + sphere + direct | 3 | 0.24 | 7.52 | 0 |  | showcase [lit] (1.3×) | far [lit] |  |  |
| gpu: points@cpu + splat + clipmap + sphere + direct | 3 | 0.24 | 9.54 | 0 |  | showcase [lit] (1.3×) | far [lit] |  |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 3 | 0.24 | 7.70 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + sphere + quadtree + gbuffer + pathtrace | 6 | 0.24 | 8.50 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 3 | 0.24 | 7.73 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + sphere + quadtree + visbuffer + pathtrace | 6 | 0.24 | 8.47 | 0 |  | far [unlit] (2.0×) | showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 3 | 0.24 | 7.72 | 0 |  |  | far [lit], showcase [lit] |  |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 3 | 0.24 | 7.85 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 3 | 0.24 | 7.73 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + raster + pathtrace | 3 | 0.24 | 8.09 | 0 |  |  | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + raster + pathtrace | 3 | 0.24 | 7.54 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + rowrun + quadtree + raster + direct | 3 | 0.24 | 8.53 | 0 |  | far [lit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 3 | 0.24 | 8.63 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.24 | 8.53 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + direct | 3 | 0.24 | 5.24 | 0 |  | far [lit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 6 | 0.24 | 5.05 | 0 |  | showcase [unlit] (2.0×), far [unlit] (1.9×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 3 | 0.24 | 7.56 | 0 |  |  | showcase [lit] |  |  |
| gpu: hier + sphere + quadtree + raster + direct | 3 | 0.24 | 8.42 | 0 |  | far [lit] (1.3×) | showcase [lit] |  |  |
| gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.24 | 8.51 | 0 |  | far [unlit] (2.0×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 3 | 0.24 | 7.60 | 0 |  |  | showcase [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + incremental + direct | 3 | 0.24 | 7.42 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 3 | 0.24 | 7.89 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 3 | 0.24 | 7.82 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: distance + refloor + quadtree + raster + pathtrace | 6 | 0.24 | 9.38 | 0 |  | far [unlit] (2.0×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 6 | 0.23 | 8.46 | 0 |  | far [unlit] (2.4×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 6 | 0.23 | 8.49 | 0 |  | far [unlit] (2.4×) | showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks + rowrun + quadtree + raster + direct | 3 | 0.23 | 8.42 | 0 |  | far [lit] (1.7×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + distance + incremental + direct | 3 | 0.23 | 9.75 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + incremental + quadtree + raster + direct | 3 | 0.23 | 6.74 | 0 |  | far [lit] (1.3×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + tree64 + rowrun + direct | 3 | 0.23 | 7.54 | 0 |  |  | far [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + direct | 3 | 0.23 | 5.58 | 0 |  | far [lit] (1.7×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 6 | 0.23 | 5.04 | 0 |  | far [unlit] (2.5×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 3 | 0.23 | 7.66 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 3 | 0.23 | 7.73 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + refloor + depth + raster + pathtrace | 3 | 0.23 | 7.44 | 0 |  |  | far [lit], terrain-flight [lit] |  |  |
| gpu: rows + refloor + quadtree + raster + pathtrace | 6 | 0.23 | 8.64 | 0 |  | far [unlit] (2.4×), showcase [unlit] (1.3×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + incremental + quadtree + raster + direct | 3 | 0.23 | 8.57 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + distance + sphere + pathtrace | 3 | 0.23 | 9.91 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 6 | 0.23 | 8.62 | 0 |  | far [unlit] (2.1×) | showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 3 | 0.23 | 5.91 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 3 | 0.23 | 7.87 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + refloor + quadtree + raster + pathtrace | 6 | 0.23 | 6.24 | 0 |  | far [unlit] (2.2×), terrain-flight [unlit] (2.2×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + distance + refloor + depth + raster + pathtrace | 3 | 0.23 | 8.20 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + raster + pathtrace | 3 | 0.23 | 7.38 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 3 | 0.23 | 6.14 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 6 | 0.23 | 6.21 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.6×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 6 | 0.23 | 8.59 | 0 |  | far [unlit] (2.1×) | showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 3 | 0.23 | 7.64 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + refloor + quadtree + raster + pathtrace | 6 | 0.23 | 8.40 | 0 |  | far [unlit] (3.0×), showcase [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: clipmap + refloor + quadtree + raster + pathtrace | 6 | 0.23 | 8.58 | 0 |  | far [unlit] (2.6×), terrain-flight [unlit] (2.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 6 | 0.23 | 8.52 | 0 |  | far [unlit] (2.1×), terrain-flight [unlit] (1.8×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 6 | 0.23 | 8.50 | 0 |  | far [unlit] (2.1×), terrain-flight [unlit] (1.8×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 3 | 0.23 | 5.96 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 3 | 0.23 | 7.73 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 3 | 0.23 | 5.58 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: rows + sphere + quadtree + visbuffer + pathtrace | 6 | 0.23 | 8.57 | 0 |  | far [unlit] (2.1×) | showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 3 | 0.23 | 7.67 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 3 | 0.23 | 7.83 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + prefix + refloor + pathtrace | 3 | 0.23 | 9.18 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: rows + sphere + quadtree + gbuffer + pathtrace | 6 | 0.23 | 8.59 | 0 |  | far [unlit] (2.1×) | showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: hier + rowrun + quadtree + raster + direct | 3 | 0.23 | 8.44 | 0 |  | far [lit] (1.7×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 3 | 0.23 | 7.66 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + quadtree + gbuffer + direct | 3 | 0.23 | 6.71 | 0 |  | far [lit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + quadtree + visbuffer + direct | 3 | 0.23 | 6.68 | 0 |  | far [lit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 3 | 0.23 | 5.89 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 3 | 0.23 | 7.94 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 6 | 0.23 | 6.26 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.6×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + distance + sphere + pathtrace | 3 | 0.23 | 13.07 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + refloor + pathtrace | 3 | 0.23 | 12.32 | 0 |  | showcase [lit] (1.4×) | far [lit] |  |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 3 | 0.22 | 7.81 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + direct | 3 | 0.22 | 5.59 | 0 |  | far [lit] (1.5×) | terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 3 | 0.22 | 7.74 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 3 | 0.22 | 7.90 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 3 | 0.22 | 7.56 | 0 |  | far [lit] (1.3×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 3 | 0.22 | 7.72 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 3 | 0.22 | 7.68 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 3 | 0.22 | 7.73 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + hier + refloor + pathtrace | 3 | 0.22 | 12.21 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 3 | 0.22 | 7.80 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: distance + incremental + quadtree + visbuffer + pathtrace | 6 | 0.22 | 9.34 | 0 |  | far [unlit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + hier + refloor + pathtrace | 3 | 0.22 | 9.03 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: distance + incremental + quadtree + gbuffer + pathtrace | 6 | 0.22 | 9.40 | 0 |  | far [unlit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: rows + sphere + quadtree + raster + direct | 3 | 0.22 | 8.52 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 6 | 0.22 | 6.78 | 0 |  | far [unlit] (2.1×), terrain-flight [unlit] (1.7×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 3 | 0.22 | 8.62 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 3 | 0.22 | 7.60 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 6 | 0.22 | 6.84 | 0 |  | far [unlit] (2.1×), terrain-flight [unlit] (1.7×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 3 | 0.22 | 7.60 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + distance + refloor + pathtrace | 3 | 0.22 | 9.91 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: bricks + sphere + quadtree + raster + direct | 3 | 0.22 | 8.43 | 0 |  | far [lit] (1.3×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + tree64 + refloor + pathtrace | 3 | 0.22 | 7.79 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + refloor + pathtrace | 3 | 0.22 | 10.77 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 3 | 0.22 | 7.68 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + direct | 3 | 0.22 | 5.02 | 0 |  | far [lit] (1.4×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 3 | 0.22 | 7.65 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 3 | 0.22 | 7.93 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 3 | 0.22 | 8.00 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 3 | 0.22 | 7.69 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix + sphere + quadtree + raster + direct | 3 | 0.22 | 8.55 | 0 |  |  | showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + rows + refloor + depth + raster + pathtrace | 3 | 0.22 | 7.51 | 0 |  |  | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + refloor + pathtrace | 3 | 0.21 | 12.10 | 0 |  |  | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + refloor + pathtrace | 3 | 0.21 | 10.81 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + rows + refloor + pathtrace | 3 | 0.21 | 12.35 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 3 | 0.21 | 5.18 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + rows + refloor + pathtrace | 3 | 0.21 | 9.14 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + rowrun + pathtrace | 3 | 0.21 | 12.40 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 3 | 0.21 | 8.64 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 3 | 0.21 | 5.54 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + refloor + pathtrace | 3 | 0.21 | 12.70 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + refloor + pathtrace | 3 | 0.21 | 7.63 | 0 |  | showcase [lit] (1.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + raster + direct | 3 | 0.21 | 7.44 | 0 |  | showcase [lit] (2.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + raster + direct | 3 | 0.21 | 8.11 | 0 |  | showcase [lit] (2.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + distance + refloor + pathtrace | 3 | 0.21 | 13.04 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 3 | 0.21 | 5.98 | 0 |  |  | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + raster + direct | 3 | 0.21 | 8.11 | 0 |  | showcase [lit] (2.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 3 | 0.21 | 8.38 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + raster + direct | 3 | 0.21 | 7.33 | 0 |  | showcase [lit] (2.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 3 | 0.21 | 6.10 | 0 |  |  | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 3 | 0.21 | 8.34 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 3 | 0.21 | 7.67 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 3 | 0.20 | 7.66 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 3 | 0.20 | 7.59 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct | 3 | 0.20 | 7.33 | 0 |  | showcase [lit] (2.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 6 | 0.20 | 6.29 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.6×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 6 | 0.20 | 7.30 | 0 |  | showcase [unlit] (3.8×), terrain-flight [unlit] (1.4×) | far [lit], far [unlit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 3 | 0.20 | 7.74 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + prefix + rowrun + pathtrace | 3 | 0.20 | 9.26 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 3 | 0.20 | 7.88 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 6 | 0.20 | 6.25 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.7×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + direct | 3 | 0.20 | 5.48 | 0 |  | showcase [lit] (2.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 3 | 0.20 | 7.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + raster + direct | 3 | 0.20 | 7.41 | 0 |  | showcase [lit] (2.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 3 | 0.20 | 7.91 | 0 |  | showcase [lit] (1.3×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 3 | 0.20 | 6.07 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 3 | 0.20 | 6.01 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 3 | 0.20 | 8.14 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 3 | 0.20 | 7.70 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + distance + rowrun + pathtrace | 3 | 0.20 | 13.11 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 3 | 0.20 | 7.77 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 3 | 0.20 | 5.71 | 0 |  | showcase [lit] (1.6×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 3 | 0.20 | 8.21 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: rows + rowrun + quadtree + raster + pathtrace | 6 | 0.20 | 8.49 | 0 |  | far [unlit] (2.7×), showcase [unlit] (1.5×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + direct | 3 | 0.20 | 5.48 | 0 |  | showcase [lit] (2.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + rows + rowrun + pathtrace | 3 | 0.20 | 12.37 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + direct | 3 | 0.20 | 7.71 | 0 |  | showcase [lit] (2.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + raster + direct | 3 | 0.20 | 7.42 | 0 |  | showcase [lit] (2.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + direct | 3 | 0.20 | 7.45 | 0 |  | showcase [lit] (2.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + distance + rowrun + pathtrace | 3 | 0.19 | 9.95 | 0 |  | showcase [lit] (1.4×) | far [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + sphere + quadtree + raster + direct | 3 | 0.19 | 6.28 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix + rowrun + quadtree + raster + pathtrace | 6 | 0.19 | 8.51 | 0 |  | far [unlit] (2.9×), showcase [unlit] (1.5×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.19 | 6.29 | 0 |  | far [unlit] (1.7×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 3 | 0.19 | 5.54 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 3 | 0.19 | 8.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + clipmap + refloor + pathtrace | 3 | 0.19 | 9.79 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 3 | 0.19 | 8.45 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 3 | 0.19 | 5.48 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + raster + direct | 3 | 0.19 | 8.12 | 0 |  | showcase [lit] (2.5×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + rows + rowrun + pathtrace | 3 | 0.19 | 9.16 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.19 | 6.26 | 0 |  | far [unlit] (1.8×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + raster + pathtrace | 3 | 0.19 | 7.38 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 3 | 0.19 | 5.45 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + incremental + pathtrace | 3 | 0.19 | 10.71 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 3 | 0.19 | 6.00 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 3 | 0.19 | 6.06 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 3 | 0.19 | 5.51 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + hier + incremental + pathtrace | 3 | 0.19 | 12.16 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + rows + incremental + pathtrace | 3 | 0.19 | 12.41 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 3 | 0.19 | 7.42 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + rowrun + pathtrace | 3 | 0.19 | 12.69 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + incremental + pathtrace | 3 | 0.19 | 12.26 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 3 | 0.18 | 8.35 | 0 |  | far [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 3 | 0.18 | 8.28 | 0 |  | far [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: distance + rowrun + quadtree + raster + pathtrace | 6 | 0.18 | 9.22 | 0 |  | far [unlit] (2.4×), showcase [unlit] (1.5×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 3 | 0.18 | 6.00 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + raster + direct | 3 | 0.18 | 7.34 | 0 |  | showcase [lit] (2.2×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + raster + direct | 3 | 0.18 | 7.44 | 0 |  | showcase [lit] (2.2×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 3 | 0.18 | 5.97 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + incremental + pathtrace | 3 | 0.18 | 12.74 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + incremental + pathtrace | 3 | 0.18 | 12.39 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + rowrun + pathtrace | 3 | 0.18 | 12.27 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + incremental + quadtree + raster + pathtrace | 6 | 0.18 | 6.54 | 0 |  | far [unlit] (2.9×), terrain-flight [unlit] (2.4×), showcase [unlit] (1.3×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + raster + direct | 3 | 0.18 | 7.35 | 0 |  | showcase [lit] (2.2×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + tree64 + incremental + pathtrace | 3 | 0.18 | 7.82 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 3 | 0.18 | 8.01 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + quadtree + raster + direct | 3 | 0.18 | 8.61 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 3 | 0.18 | 7.87 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 3 | 0.18 | 7.94 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + raster + pathtrace | 3 | 0.18 | 8.10 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + raster + direct | 3 | 0.18 | 7.44 | 0 |  | showcase [lit] (2.1×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + raster + direct | 3 | 0.18 | 8.13 | 0 |  | showcase [lit] (2.1×) | far [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + sphere + quadtree + raster + direct | 3 | 0.18 | 6.58 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 3 | 0.18 | 6.03 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 3 | 0.18 | 6.12 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 3 | 0.18 | 5.72 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + bricks + rowrun + pathtrace | 3 | 0.17 | 9.12 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 3 | 0.17 | 5.66 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 3 | 0.17 | 7.86 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + direct | 3 | 0.17 | 7.73 | 0 |  | showcase [lit] (2.1×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 3 | 0.17 | 7.80 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 3 | 0.17 | 8.19 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + incremental + quadtree + raster + pathtrace | 6 | 0.17 | 8.58 | 0 |  | far [unlit] (3.0×), terrain-flight [unlit] (2.8×), showcase [unlit] (1.4×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 3 | 0.17 | 7.81 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + direct | 3 | 0.17 | 5.46 | 0 |  | showcase [lit] (2.0×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 3 | 0.17 | 7.93 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + direct | 3 | 0.17 | 7.72 | 0 |  | showcase [lit] (2.0×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 3 | 0.17 | 8.27 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + rowrun + quadtree + raster + pathtrace | 6 | 0.17 | 8.43 | 0 |  | far [unlit] (3.1×), terrain-flight [unlit] (2.8×), showcase [unlit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + clipmap + rowrun + pathtrace | 3 | 0.17 | 9.83 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: rows + incremental + quadtree + raster + pathtrace | 6 | 0.17 | 8.64 | 0 |  | far [unlit] (3.0×), showcase [unlit] (1.7×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + raster + pathtrace | 3 | 0.17 | 7.50 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + distance + incremental + pathtrace | 3 | 0.17 | 13.02 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 3 | 0.17 | 7.85 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 3 | 0.17 | 7.79 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + incremental + quadtree + raster + pathtrace | 6 | 0.17 | 8.54 | 0 |  | far [unlit] (3.5×), showcase [unlit] (1.7×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + raster + direct | 3 | 0.17 | 7.34 | 0 |  | showcase [lit] (2.1×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + hier + incremental + pathtrace | 3 | 0.17 | 9.12 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + incremental + quadtree + raster + pathtrace | 6 | 0.17 | 8.55 | 0 |  | far [unlit] (3.2×), showcase [unlit] (1.6×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 3 | 0.17 | 7.92 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 3 | 0.17 | 7.99 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + hier + rowrun + pathtrace | 3 | 0.17 | 12.25 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 3 | 0.16 | 7.93 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 3 | 0.16 | 5.58 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 3 | 0.16 | 7.54 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + raster + direct | 3 | 0.16 | 7.33 | 0 |  | showcase [lit] (1.8×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 3 | 0.16 | 5.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 3 | 0.16 | 7.85 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 3 | 0.16 | 7.89 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 3 | 0.16 | 8.21 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 3 | 0.16 | 8.00 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + rows + incremental + pathtrace | 3 | 0.16 | 9.20 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 3 | 0.16 | 8.28 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 3 | 0.16 | 6.17 | 0 |  | far [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + bricks + incremental + pathtrace | 3 | 0.16 | 9.11 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + hier + rowrun + pathtrace | 3 | 0.16 | 9.09 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + clipmap + incremental + pathtrace | 3 | 0.16 | 9.84 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix + incremental + quadtree + raster + pathtrace | 6 | 0.16 | 8.67 | 0 |  | far [unlit] (3.2×), showcase [unlit] (1.7×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + hier + sphere + pathtrace | 3 | 0.16 | 12.19 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 3 | 0.16 | 6.02 | 0 |  | far [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + direct | 3 | 0.16 | 7.34 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 6 | 0.16 | 6.61 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 3 | 0.16 | 7.76 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 3 | 0.16 | 7.85 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 6 | 0.16 | 6.59 | 0 |  | far [unlit] (1.9×), terrain-flight [unlit] (1.5×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + prefix + incremental + pathtrace | 3 | 0.16 | 9.28 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + raster + direct | 3 | 0.16 | 7.44 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + raster + direct | 3 | 0.16 | 7.33 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + raster + pathtrace | 3 | 0.16 | 8.20 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + direct | 3 | 0.16 | 5.48 | 0 |  | showcase [lit] (1.8×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + raster + direct | 3 | 0.16 | 7.41 | 0 |  | showcase [lit] (1.7×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + raster + pathtrace | 3 | 0.16 | 7.40 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + hier + sphere + pathtrace | 3 | 0.16 | 9.05 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + sphere + pathtrace | 3 | 0.16 | 12.26 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + incremental + quadtree + raster + direct | 3 | 0.15 | 6.36 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + sphere + pathtrace | 3 | 0.15 | 12.36 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 3 | 0.15 | 5.71 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 3 | 0.15 | 6.00 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + rows + sphere + pathtrace | 3 | 0.15 | 12.35 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + raster + pathtrace | 3 | 0.15 | 7.33 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 3 | 0.15 | 5.93 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 3 | 0.15 | 5.36 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + incremental + pathtrace | 3 | 0.15 | 10.78 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 3 | 0.15 | 5.61 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + rowrun + pathtrace | 3 | 0.15 | 10.81 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + direct | 3 | 0.15 | 7.71 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 3 | 0.15 | 7.87 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 3 | 0.15 | 7.94 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + raster + pathtrace | 3 | 0.15 | 7.31 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 3 | 0.15 | 8.63 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 3 | 0.15 | 8.70 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 3 | 0.15 | 7.87 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + rowrun + pathtrace | 3 | 0.15 | 7.70 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + raster + pathtrace | 3 | 0.15 | 7.44 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + direct | 3 | 0.15 | 5.56 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + sphere + quadtree + raster + pathtrace | 6 | 0.15 | 8.57 | 0 |  | far [unlit] (3.4×), showcase [unlit] (1.8×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + bricks + sphere + pathtrace | 3 | 0.15 | 9.06 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 3 | 0.15 | 7.76 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 3 | 0.15 | 7.94 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + prefix + sphere + pathtrace | 3 | 0.15 | 9.19 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: distance + incremental + quadtree + raster + pathtrace | 6 | 0.15 | 9.42 | 0 |  | far [unlit] (2.8×), showcase [unlit] (1.8×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 3 | 0.15 | 7.87 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks + rowrun + quadtree + raster + pathtrace | 6 | 0.15 | 8.40 | 0 |  | far [unlit] (3.9×), showcase [unlit] (1.9×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + sphere + pathtrace | 3 | 0.15 | 10.77 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + distance + incremental + pathtrace | 3 | 0.15 | 9.99 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + rows + sphere + pathtrace | 3 | 0.14 | 9.17 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 3 | 0.14 | 5.92 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + sphere + pathtrace | 3 | 0.14 | 7.66 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 3 | 0.14 | 5.98 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + sphere + pathtrace | 3 | 0.14 | 12.66 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + raster + pathtrace | 3 | 0.14 | 8.28 | 0 |  | showcase [lit] (2.1×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + direct | 3 | 0.14 | 5.49 | 0 |  | showcase [lit] (1.9×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + sphere + pathtrace | 3 | 0.14 | 10.84 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier + rowrun + quadtree + raster + pathtrace | 6 | 0.14 | 8.43 | 0 |  | far [unlit] (3.4×), showcase [unlit] (1.9×), terrain-flight [unlit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks + sphere + quadtree + raster + pathtrace | 6 | 0.14 | 8.55 | 0 |  | far [unlit] (4.0×), showcase [unlit] (1.9×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 3 | 0.14 | 7.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: clipmap + sphere + quadtree + raster + pathtrace | 6 | 0.14 | 8.58 | 0 |  | far [unlit] (3.4×), terrain-flight [unlit] (3.1×), showcase [unlit] (1.7×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: svo_dag + sphere + quadtree + raster + pathtrace | 6 | 0.14 | 6.25 | 0 |  | far [unlit] (3.0×), terrain-flight [unlit] (2.7×), showcase [unlit] (1.6×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: prefix + sphere + quadtree + raster + pathtrace | 6 | 0.14 | 8.68 | 0 |  | far [unlit] (3.6×), showcase [unlit] (1.8×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: points@cpu + splat + tree64 + sphere + pathtrace | 3 | 0.14 | 7.81 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + raster + pathtrace | 3 | 0.14 | 8.11 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: rows + sphere + quadtree + raster + pathtrace | 6 | 0.14 | 8.65 | 0 |  | far [unlit] (3.5×), showcase [unlit] (1.9×), terrain-flight [unlit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: naive@cpu + faces + rows + incremental + depth + raster + pathtrace | 3 | 0.14 | 7.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 3 | 0.13 | 6.00 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 3 | 0.13 | 6.00 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + incremental + depth + raster + pathtrace | 3 | 0.13 | 7.42 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 3 | 0.13 | 5.53 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + sphere + quadtree + raster + pathtrace | 6 | 0.13 | 6.60 | 0 |  | far [unlit] (3.6×), terrain-flight [unlit] (2.8×), showcase [unlit] (1.6×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + raster + pathtrace | 3 | 0.13 | 7.60 | 0 |  | showcase [lit] (2.0×) | far [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + clipmap + sphere + pathtrace | 3 | 0.13 | 9.82 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + svo_dag + incremental + pathtrace | 3 | 0.13 | 7.73 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + raster + pathtrace | 3 | 0.13 | 7.29 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 3 | 0.13 | 5.96 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + raster + pathtrace | 3 | 0.13 | 8.29 | 0 |  | showcase [lit] (2.0×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 3 | 0.13 | 6.04 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + raster + pathtrace | 3 | 0.13 | 7.54 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 3 | 0.13 | 8.15 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 3 | 0.13 | 8.22 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + rowrun + quadtree + raster + direct | 3 | 0.13 | 6.24 | 0 |  | far [lit] (2.1×) | showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + raster + pathtrace | 3 | 0.13 | 7.51 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 3 | 0.13 | 7.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + raster + pathtrace | 3 | 0.13 | 7.43 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + incremental + quadtree + raster + pathtrace | 6 | 0.13 | 6.27 | 0 |  | far [unlit] (3.0×), terrain-flight [unlit] (2.7×), showcase [unlit] (1.8×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + rowrun + pathtrace | 3 | 0.12 | 10.74 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + quadtree + raster + direct | 3 | 0.12 | 6.70 | 0 |  | far [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: points@cpu + splat + tree64 + rowrun + pathtrace | 3 | 0.12 | 7.83 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + direct | 3 | 0.12 | 5.49 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 3 | 0.12 | 5.64 | 0 |  | showcase [lit] (2.2×) | far [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + raster + pathtrace | 3 | 0.12 | 7.58 | 0 |  | showcase [lit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 3 | 0.12 | 6.05 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 3 | 0.12 | 6.12 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + direct | 3 | 0.12 | 5.72 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 3 | 0.12 | 8.01 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + raster + pathtrace | 3 | 0.12 | 7.31 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 3 | 0.12 | 5.62 | 0 |  | showcase [lit] (2.2×) | far [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + raster + pathtrace | 3 | 0.12 | 7.40 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + raster + pathtrace | 3 | 0.12 | 7.38 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 3 | 0.12 | 7.30 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + distance + incremental + depth + raster + pathtrace | 3 | 0.12 | 8.20 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + raster + pathtrace | 3 | 0.11 | 7.31 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 3 | 0.11 | 7.87 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + sphere + depth + raster + pathtrace | 3 | 0.11 | 7.43 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 3 | 0.10 | 8.00 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 3 | 0.10 | 7.61 | 0 |  | showcase [lit] (2.1×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 6 | 0.10 | 6.18 | 0 |  | far [unlit] (3.3×), terrain-flight [unlit] (2.8×), showcase [unlit] (1.8×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 3 | 0.10 | 5.20 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + raster + pathtrace | 3 | 0.10 | 7.57 | 0 |  | showcase [lit] (1.7×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + raster + pathtrace | 3 | 0.10 | 8.28 | 0 |  | showcase [lit] (2.1×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 3 | 0.10 | 5.61 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 3 | 0.10 | 5.19 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + raster + pathtrace | 3 | 0.10 | 7.43 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 3 | 0.10 | 7.53 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: tree64 + rowrun + quadtree + raster + pathtrace | 6 | 0.09 | 6.60 | 0 |  | far [unlit] (3.3×), terrain-flight [unlit] (2.6×), showcase [unlit] (1.8×) | far [lit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: naive@cpu + faces + rows + sphere + depth + raster + pathtrace | 3 | 0.09 | 7.52 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + raster + pathtrace | 3 | 0.09 | 7.54 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 3 | 0.09 | 7.42 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + raster + pathtrace | 3 | 0.09 | 7.42 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + raster + pathtrace | 3 | 0.09 | 7.59 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 3 | 0.09 | 5.63 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + raster + pathtrace | 3 | 0.08 | 7.51 | 0 |  | showcase [lit] (1.7×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + raster + pathtrace | 3 | 0.08 | 7.60 | 0 |  | showcase [lit] (1.7×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 3 | 0.08 | 5.68 | 0 |  | showcase [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + raster + pathtrace | 3 | 0.08 | 7.50 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 3 | 0.08 | 5.72 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 3 | 0.08 | 5.57 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 3 | 0.08 | 7.87 | 0 |  | showcase [lit] (1.7×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 3 | 0.08 | 8.02 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + raster + pathtrace | 3 | 0.08 | 8.29 | 0 |  | showcase [lit] (1.7×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 3 | 0.07 | 7.87 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + raster + pathtrace | 3 | 0.07 | 7.49 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 3 | 0.07 | 5.18 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + raster + pathtrace | 3 | 0.07 | 7.50 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 3 | 0.07 | 5.34 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 3 | 0.07 | 7.49 | 0 |  | showcase [lit] (1.9×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + raster + pathtrace | 3 | 0.06 | 7.60 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + raster + pathtrace | 3 | 0.06 | 7.56 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + raster + pathtrace | 3 | 0.06 | 7.48 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 3 | 0.06 | 5.64 | 0 |  | showcase [lit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 3 | 0.06 | 5.64 | 0 |  | showcase [lit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 3 | 0.05 | 5.83 | 0 |  | showcase [lit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: hier + refloor + raycast + direct | 6 | 0.05 | 1.26 | 0 |  | far [unlit] (1.8×), far [lit] (1.5×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 3 | 0.05 | 7.89 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 3 | 0.05 | 5.71 | 0 |  | showcase [lit] (1.8×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 3 | 0.05 | 5.57 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: prefix + refloor + raycast + direct | 6 | 0.05 | 1.39 | 0 |  | far [unlit] (1.8×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: hier + incremental + raycast + direct | 6 | 0.05 | 1.27 | 0 |  | far [unlit] (1.9×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bricks + refloor + raycast + direct | 6 | 0.05 | 1.21 | 0 |  | far [unlit] (1.9×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: prefix + incremental + raycast + direct | 6 | 0.05 | 1.40 | 0 |  | far [unlit] (2.0×), far [lit] (1.8×), terrain-flight [unlit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bits + refloor + raycast + direct | 6 | 0.05 | 1.26 | 0 |  | far [unlit] (1.9×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: u8 + refloor + raycast + direct | 6 | 0.05 | 1.04 | 0 |  | far [unlit] (1.9×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: brickmajor + refloor + raycast + direct | 6 | 0.05 | 1.57 | 0 |  | far [unlit] (1.8×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: morton + refloor + raycast + direct | 6 | 0.04 | 1.58 | 0 |  | far [unlit] (1.8×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: palette + refloor + raycast + direct | 6 | 0.04 | 1.00 | 0 |  | far [unlit] (1.8×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bricks + incremental + raycast + direct | 6 | 0.04 | 1.22 | 0 |  | far [unlit] (1.9×), far [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: greedy + faces + raster + bricks + refloor + direct | 6 | 0.04 | 2.08 | 0 |  | terrain-flight [unlit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: bits + incremental + raycast + direct | 6 | 0.04 | 1.27 | 0 |  | far [unlit] (1.9×), far [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: brickmajor + incremental + raycast + direct | 6 | 0.04 | 1.58 | 0 |  | far [unlit] (1.9×), far [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: u8 + incremental + raycast + direct | 6 | 0.04 | 1.05 | 0 |  | far [unlit] (1.9×), far [lit] (1.6×), terrain-flight [unlit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: morton + incremental + raycast + direct | 6 | 0.04 | 1.58 | 0 |  | far [unlit] (1.9×), far [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 3 | 0.04 | 5.62 | 0 |  | showcase [lit] (2.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: palette + incremental + raycast + direct | 6 | 0.04 | 1.01 | 0 |  | far [unlit] (1.9×), far [lit] (1.7×), terrain-flight [unlit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.67 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.73 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.31 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 3 | 0.04 | 5.81 | 0 |  | showcase [lit] (2.0×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.42 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + refloor + direct | 3 | 0.04 | 10.67 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.68 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + incremental + direct | 3 | 0.04 | 8.38 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + incremental + direct | 3 | 0.04 | 10.72 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.24 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + rowrun + direct | 3 | 0.04 | 8.29 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.67 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + incremental + direct | 3 | 0.04 | 10.68 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + rowrun + direct | 3 | 0.04 | 10.46 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + refloor + direct | 3 | 0.04 | 8.28 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + sphere + direct | 3 | 0.04 | 11.34 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + sphere + direct | 3 | 0.04 | 10.62 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.18 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + rowrun + direct | 3 | 0.04 | 11.26 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + incremental + direct | 3 | 0.04 | 8.89 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.41 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + rowrun + direct | 3 | 0.04 | 10.73 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + rowrun + direct | 3 | 0.04 | 10.64 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + sphere + direct | 3 | 0.04 | 8.47 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + refloor + direct | 3 | 0.04 | 10.59 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.69 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + sphere + direct | 3 | 0.04 | 10.55 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.35 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.34 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.48 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + incremental + direct | 3 | 0.04 | 10.80 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + incremental + direct | 3 | 0.04 | 10.57 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + refloor + direct | 3 | 0.04 | 10.66 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.09 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.62 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + rowrun + direct | 3 | 0.04 | 10.64 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + refloor + direct | 3 | 0.04 | 10.51 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + sphere + direct | 3 | 0.04 | 10.77 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + incremental + direct | 3 | 0.04 | 11.32 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + rowrun + direct | 3 | 0.04 | 8.73 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + refloor + direct | 3 | 0.04 | 8.77 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + refloor + direct | 3 | 0.04 | 11.24 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + sphere + direct | 3 | 0.04 | 8.89 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + hier + refloor + direct | 3 | 0.04 | 2.14 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.08 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.77 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + prefix + refloor + direct | 3 | 0.04 | 2.71 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.76 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.81 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + hier + incremental + direct | 3 | 0.04 | 2.14 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.38 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.87 | 0 |  | showcase [lit] (1.5×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 11.21 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.78 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.04 | 10.35 | 0 |  | showcase [lit] (1.6×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + sphere + direct | 3 | 0.04 | 10.50 | 0 |  | showcase [lit] (1.4×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + bricks + incremental + direct | 3 | 0.04 | 2.12 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + prefix + incremental + direct | 3 | 0.04 | 2.73 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + u8 + refloor + direct | 3 | 0.04 | 2.48 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + u8 + incremental + direct | 3 | 0.04 | 2.51 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + brickmajor + incremental + direct | 3 | 0.04 | 2.87 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + bits + incremental + direct | 3 | 0.04 | 2.15 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + morton + refloor + direct | 3 | 0.04 | 2.87 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + morton + incremental + direct | 3 | 0.04 | 2.87 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.38 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: greedy + faces + raster + brickmajor + refloor + direct | 3 | 0.04 | 2.85 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + bits + refloor + direct | 3 | 0.04 | 2.15 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: greedy + faces + raster + palette + incremental + direct | 3 | 0.04 | 2.22 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.51 | 0 |  | showcase [lit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.55 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: greedy + faces + raster + palette + refloor + direct | 3 | 0.04 | 2.22 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.90 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 11.25 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.46 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.55 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.57 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 11.20 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 11.31 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.75 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.29 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.89 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.87 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.58 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.04 | 10.45 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 10.50 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 10.56 | 0 |  | showcase [lit] (1.7×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 11.33 | 0 |  | showcase [lit] (1.8×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 10.52 | 0 |  | showcase [lit] (1.8×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 11.40 | 0 |  | showcase [lit] (1.8×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 11.47 | 0 |  | showcase [lit] (1.8×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 11.30 | 0 |  | showcase [lit] (1.9×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: naive + faces + raster + bricks + refloor + direct | 6 | 0.03 | 2.84 | 0 |  | showcase [unlit] (1.6×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.03 | 11.35 | 0 |  | showcase [lit] (2.0×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: naive + faces + raster + hier + refloor + direct | 3 | 0.03 | 2.92 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + prefix + refloor + direct | 3 | 0.03 | 3.44 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + hier + incremental + direct | 3 | 0.03 | 2.92 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + prefix + incremental + direct | 3 | 0.03 | 3.40 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + bricks + incremental + direct | 3 | 0.03 | 2.93 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + u8 + incremental + direct | 3 | 0.03 | 3.21 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + brickmajor + incremental + direct | 3 | 0.03 | 3.57 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + u8 + refloor + direct | 3 | 0.03 | 3.22 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + bits + incremental + direct | 3 | 0.03 | 2.89 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + morton + refloor + direct | 3 | 0.03 | 3.56 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + brickmajor + refloor + direct | 3 | 0.03 | 3.58 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + morton + incremental + direct | 3 | 0.03 | 3.58 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + bits + refloor + direct | 3 | 0.03 | 2.90 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + palette + refloor + direct | 3 | 0.03 | 2.96 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| cpu: naive + faces + raster + palette + incremental + direct | 3 | 0.03 | 2.97 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.08 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.22 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.65 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.98 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.21 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 8.92 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 10.36 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.42 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.65 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.95 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.54 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 8.86 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 10.31 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.53 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.59 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.16 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 10.40 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.89 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.74 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.83 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 10.49 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 9.63 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 10.37 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 3 | 0.01 | 10.49 | 0 |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.16 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.10 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 8.95 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.06 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.16 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.20 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.06 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 10.07 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.27 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.64 | 0 |  |  | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.14 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 10.08 | 0 |  |  | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.14 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.04 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.61 | 0 |  |  | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.92 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.43 | 0 |  |  | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.15 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.99 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.84 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 9.87 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 10.08 | 0 |  | terrain-flight [lit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 10.09 | 0 |  | terrain-flight [lit] (1.4×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 6 | 0.01 | 10.20 | 0 |  | terrain-flight [lit] (1.4×), terrain-flight [unlit] (1.3×) | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |  |
| cpu: heightmap + columns + heightfield + direct | 6 | - | - | 0 |  |  |  | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |
| gpu: heightmap + columns + heightfield + direct | 3 | - | - | 0 |  |  |  | far [lit], showcase [lit], terrain-flight [lit] |  |
| gpu: heightmap + columns + heightfield + pathtrace | 6 | - | - | 0 |  |  |  | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |  |

## Memory

The footprint is the peak RSS plus the GPU memory (measured by the device's allocator where it reports one, else what the renderer counts; RSS may hold mapped GPU memory too, so read it as an upper bound). *mem* is the footprint relative to the leanest correct renderer of the scenario. The frontier (◆ in the scenario tables): correct renderers no other correct one beats on speed and footprint both.

| scenario | leanest | MB | fastest's MB | frontier (fps, MB) |
|---|---|---:|---:|---|
| far [lit] | cpu: palette + incremental + raycast + direct | 525 | 2770 | gpu: bricks + refloor + composite + raycast + direct (375, 2770); gpu: bricks + refloor + beam + composite + raycast + direct (367, 2766); gpu: tree64 + refloor + composite + raycast + direct (362, 1517); gpu: tree64 + refloor + beam + composite + raycast + direct (314, 1499); gpu: svo_dag + refloor + composite + raycast + direct (283, 1336); gpu: svo_dag + rowrun + composite + raycast + direct (201, 1336); cpu: hier + refloor + raycast + direct (32, 670); cpu: hier + incremental + raycast + direct (30, 670); cpu: bricks + refloor + raycast + direct (28, 635); cpu: u8 + refloor + raycast + direct (27, 534); cpu: palette + incremental + raycast + direct (26, 525) |
| far [unlit] | cpu: palette + refloor + raycast + direct | 541 | 2799 | gpu: bricks + refloor + composite + raycast + pathtrace (401, 2799); gpu: tree64 + refloor + composite + raycast + pathtrace (385, 1587); gpu: tree64 + refloor + beam + composite + raycast + pathtrace (332, 1534); gpu: svo_dag + refloor + composite + raycast + pathtrace (295, 1367); cpu: hier + refloor + raycast + direct (39, 670); cpu: bricks + refloor + raycast + direct (35, 635); cpu: bricks + incremental + raycast + direct (34, 635); cpu: u8 + refloor + raycast + direct (34, 552); cpu: palette + refloor + raycast + direct (32, 541) |
| showcase [lit] | cpu: prefix + refloor + raycast + direct | 26 | 320 | gpu: distance + sphere + composite + raycast + direct (4435, 320); gpu: distance + sphere + beam + composite + raycast + direct (4175, 320); gpu: distance + refloor + composite + raycast + direct (3820, 320); gpu: prefix + refloor + composite + raycast + direct (3767, 316); gpu: hier + refloor + composite + raycast + direct (3570, 315); gpu: svo_dag + refloor + composite + raycast + direct (3523, 309); gpu: svo_dag + refloor + beam + composite + raycast + direct (3260, 309); gpu: tree64 + refloor + composite + raycast + direct (2735, 298); cpu: greedy + faces + raster + hier + refloor + direct (123, 56); cpu: naive + faces + raster + hier + refloor + direct (122, 54); cpu: hier + refloor + raycast + direct (112, 31); cpu: prefix + refloor + raycast + direct (103, 26) |
| showcase [unlit] | cpu: prefix + refloor + raycast + direct | 26 | 332 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace (6842, 332); gpu: prefix + refloor + composite + raycast + pathtrace (5895, 325); gpu: hier + refloor + composite + raycast + pathtrace (5605, 324); gpu: svo_dag + refloor + composite + raycast + pathtrace (5401, 318); gpu: svo_dag + sphere + beam + composite + raycast + pathtrace (2176, 318); cpu: naive + faces + raster + bricks + refloor + direct (328, 56); cpu: greedy + faces + raster + bricks + refloor + direct (326, 56); cpu: hier + refloor + raycast + direct (249, 31); cpu: prefix + refloor + raycast + direct (213, 26) |
| terrain-flight [lit] | cpu: palette + incremental + raycast + direct | 252 | 1118 | gpu: bricks + refloor + composite + raycast + direct (569, 1118); gpu: hier + refloor + composite + raycast + direct (522, 1116); gpu: bricks + refloor + beam + composite + raycast + direct (505, 1115); gpu: tree64 + refloor + composite + raycast + direct (486, 775); gpu: svo_dag + refloor + composite + raycast + direct (450, 749); cpu: hier + refloor + raycast + direct (31, 339); cpu: bits + refloor + raycast + direct (27, 339); cpu: bricks + refloor + raycast + direct (27, 323); cpu: u8 + refloor + raycast + direct (26, 259); cpu: palette + refloor + raycast + direct (26, 253); cpu: palette + incremental + raycast + direct (25, 252) |
| terrain-flight [unlit] | cpu: palette + incremental + raycast + direct | 252 | 1146 | gpu: bricks + refloor + composite + raycast + pathtrace (612, 1146); gpu: tree64 + refloor + composite + raycast + pathtrace (568, 803); gpu: svo_dag + refloor + composite + raycast + pathtrace (487, 781); cpu: hier + refloor + raycast + direct (40, 340); cpu: bits + refloor + raycast + direct (35, 339); cpu: bricks + refloor + raycast + direct (35, 324); cpu: u8 + refloor + raycast + direct (34, 257); cpu: palette + refloor + raycast + direct (33, 252); cpu: palette + incremental + raycast + direct (32, 252) |

Leanest first (geometric mean of *mem* over the scenarios where it ran correctly):

| renderer | geomean mem | geomean rel | on the frontier |
|---|---:|---:|---|
| cpu: palette + refloor + raycast + direct | 1.00 | 0.04 | far [unlit], terrain-flight [lit], terrain-flight [unlit] |
| cpu: palette + incremental + raycast + direct | 1.01 | 0.04 | far [lit], terrain-flight [lit], terrain-flight [unlit] |
| cpu: u8 + refloor + raycast + direct | 1.04 | 0.05 | far [lit], far [unlit], terrain-flight [lit], terrain-flight [unlit] |
| cpu: u8 + incremental + raycast + direct | 1.05 | 0.04 |  |
| cpu: bricks + refloor + raycast + direct | 1.21 | 0.05 | far [lit], far [unlit], terrain-flight [lit], terrain-flight [unlit] |
| cpu: bricks + incremental + raycast + direct | 1.22 | 0.04 | far [unlit] |
| cpu: bits + refloor + raycast + direct | 1.26 | 0.05 | terrain-flight [lit], terrain-flight [unlit] |
| cpu: hier + refloor + raycast + direct | 1.26 | 0.05 | far [lit], far [unlit], showcase [lit], showcase [unlit], terrain-flight [lit], terrain-flight [unlit] |
| cpu: hier + incremental + raycast + direct | 1.27 | 0.05 | far [lit] |
| cpu: bits + incremental + raycast + direct | 1.27 | 0.04 |  |
| cpu: prefix + refloor + raycast + direct | 1.39 | 0.05 | showcase [lit], showcase [unlit] |
| cpu: prefix + incremental + raycast + direct | 1.40 | 0.05 |  |
| cpu: brickmajor + refloor + raycast + direct | 1.57 | 0.05 |  |
| cpu: morton + refloor + raycast + direct | 1.58 | 0.04 |  |
| cpu: brickmajor + incremental + raycast + direct | 1.58 | 0.04 |  |
| cpu: morton + incremental + raycast + direct | 1.58 | 0.04 |  |
| cpu: greedy + faces + raster + bricks + refloor + direct | 2.08 | 0.04 | showcase [unlit] |
| cpu: greedy + faces + raster + bricks + incremental + direct | 2.12 | 0.04 |  |
| cpu: greedy + faces + raster + hier + incremental + direct | 2.14 | 0.04 |  |
| cpu: greedy + faces + raster + hier + refloor + direct | 2.14 | 0.04 | showcase [lit] |
| cpu: greedy + faces + raster + bits + incremental + direct | 2.15 | 0.04 |  |
| cpu: greedy + faces + raster + bits + refloor + direct | 2.15 | 0.04 |  |
| cpu: greedy + faces + raster + palette + incremental + direct | 2.22 | 0.04 |  |
| cpu: greedy + faces + raster + palette + refloor + direct | 2.22 | 0.04 |  |
| cpu: greedy + faces + raster + u8 + refloor + direct | 2.48 | 0.04 |  |
| cpu: greedy + faces + raster + u8 + incremental + direct | 2.51 | 0.04 |  |
| cpu: greedy + faces + raster + prefix + refloor + direct | 2.71 | 0.04 |  |
| cpu: greedy + faces + raster + prefix + incremental + direct | 2.73 | 0.04 |  |
| cpu: naive + faces + raster + bricks + refloor + direct | 2.84 | 0.03 | showcase [unlit] |
| cpu: greedy + faces + raster + brickmajor + refloor + direct | 2.85 | 0.04 |  |
| cpu: greedy + faces + raster + morton + refloor + direct | 2.87 | 0.04 |  |
| cpu: greedy + faces + raster + brickmajor + incremental + direct | 2.87 | 0.04 |  |
| cpu: greedy + faces + raster + morton + incremental + direct | 2.87 | 0.04 |  |
| cpu: naive + faces + raster + bits + incremental + direct | 2.89 | 0.03 |  |
| cpu: naive + faces + raster + bits + refloor + direct | 2.90 | 0.03 |  |
| cpu: naive + faces + raster + hier + incremental + direct | 2.92 | 0.03 |  |
| cpu: naive + faces + raster + hier + refloor + direct | 2.92 | 0.03 | showcase [lit] |
| cpu: naive + faces + raster + bricks + incremental + direct | 2.93 | 0.03 |  |
| cpu: naive + faces + raster + palette + refloor + direct | 2.96 | 0.03 |  |
| cpu: naive + faces + raster + palette + incremental + direct | 2.97 | 0.03 |  |
| cpu: naive + faces + raster + u8 + incremental + direct | 3.21 | 0.03 |  |
| cpu: naive + faces + raster + u8 + refloor + direct | 3.22 | 0.03 |  |
| cpu: naive + faces + raster + prefix + incremental + direct | 3.40 | 0.03 |  |
| cpu: naive + faces + raster + prefix + refloor + direct | 3.44 | 0.03 |  |
| cpu: naive + faces + raster + morton + refloor + direct | 3.56 | 0.03 |  |
| cpu: naive + faces + raster + brickmajor + incremental + direct | 3.57 | 0.03 |  |
| cpu: naive + faces + raster + morton + incremental + direct | 3.58 | 0.03 |  |
| cpu: naive + faces + raster + brickmajor + refloor + direct | 3.58 | 0.03 |  |
| gpu: svo_dag + refloor + composite + raycast + direct | 4.49 | 0.78 | far [lit], showcase [lit], terrain-flight [lit] |
| gpu: svo_dag + refloor + beam + composite + raycast + direct | 4.49 | 0.65 | showcase [lit] |
| gpu: svo_dag + rowrun + composite + raycast + direct | 4.50 | 0.50 | far [lit] |
| gpu: svo_dag + sphere + composite + raycast + direct | 4.50 | 0.43 |  |
| gpu: svo_dag + sphere + beam + composite + raycast + direct | 4.60 | 0.38 |  |
| gpu: svo_dag + incremental + composite + raycast + direct | 4.62 | 0.38 |  |
| gpu: svo_dag + rowrun + beam + composite + raycast + direct | 4.63 | 0.44 |  |
| gpu: svo_dag + refloor + composite + raycast + pathtrace | 4.63 | 0.57 | far [unlit], showcase [unlit], terrain-flight [unlit] |
| gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 4.64 | 0.49 |  |
| gpu: svo_dag + incremental + beam + composite + raycast + direct | 4.64 | 0.35 |  |
| gpu: tree64 + refloor + composite + raycast + direct | 4.68 | 0.80 | far [lit], showcase [lit], terrain-flight [lit] |
| gpu: svo_dag + sphere + composite + raycast + pathtrace | 4.68 | 0.29 |  |
| gpu: tree64 + incremental + composite + raycast + direct | 4.69 | 0.61 |  |
| gpu: svo_dag + rowrun + composite + raycast + pathtrace | 4.69 | 0.32 |  |
| gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 4.70 | 0.27 | showcase [unlit] |
| gpu: svo_dag + incremental + composite + raycast + pathtrace | 4.70 | 0.27 |  |
| gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 4.72 | 0.25 |  |
| gpu: tree64 + refloor + beam + composite + raycast + direct | 4.72 | 0.68 | far [lit] |
| gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 4.75 | 0.29 |  |
| gpu: tree64 + incremental + beam + composite + raycast + direct | 4.86 | 0.53 |  |
| gpu: tree64 + rowrun + beam + composite + raycast + direct | 4.86 | 0.36 |  |
| gpu: tree64 + sphere + composite + raycast + direct | 4.87 | 0.37 |  |
| gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 4.87 | 0.51 | far [unlit] |
| gpu: tree64 + sphere + beam + composite + raycast + direct | 4.90 | 0.33 |  |
| gpu: tree64 + refloor + composite + raycast + pathtrace | 4.90 | 0.59 | far [unlit], terrain-flight [unlit] |
| gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 4.91 | 0.39 |  |
| gpu: tree64 + sphere + composite + raycast + pathtrace | 4.93 | 0.25 |  |
| gpu: tree64 + rowrun + composite + raycast + pathtrace | 4.95 | 0.25 |  |
| gpu: tree64 + rowrun + composite + raycast + direct | 5.00 | 0.39 |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + direct | 5.00 | 0.44 |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + direct | 5.01 | 0.31 |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + direct | 5.01 | 0.27 |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + direct | 5.02 | 0.22 |  |
| gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 5.04 | 0.23 |  |
| gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 5.05 | 0.24 |  |
| gpu: tree64 + incremental + composite + raycast + pathtrace | 5.07 | 0.41 |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 5.15 | 0.50 |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 5.16 | 0.40 |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 5.17 | 0.44 |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 5.18 | 0.21 |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 5.18 | 0.38 |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + direct | 5.18 | 0.44 |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 5.18 | 0.07 |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 5.19 | 0.10 |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + direct | 5.20 | 0.31 |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 5.20 | 0.10 |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 5.21 | 0.40 |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 5.22 | 0.43 |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 5.22 | 0.50 |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 5.24 | 0.37 |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + direct | 5.24 | 0.24 |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + direct | 5.28 | 0.36 |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 5.34 | 0.07 |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 5.35 | 0.50 |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 5.35 | 0.40 |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 5.36 | 0.15 |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + raster + direct | 5.37 | 0.30 |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + raster + direct | 5.40 | 0.48 |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 5.40 | 0.47 |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 5.42 | 0.32 |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + raster + direct | 5.42 | 0.37 |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 5.42 | 0.40 |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 5.43 | 0.39 |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 5.44 | 0.40 |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 5.44 | 0.46 |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 5.45 | 0.19 |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + direct | 5.46 | 0.17 |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 5.46 | 0.51 |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 5.48 | 0.19 |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + direct | 5.48 | 0.16 |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + direct | 5.48 | 0.20 |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + direct | 5.48 | 0.20 |  |
| gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 5.48 | 0.32 |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + direct | 5.49 | 0.12 |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + direct | 5.49 | 0.14 |  |
| gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 5.51 | 0.19 |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 5.52 | 0.16 |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 5.53 | 0.13 |  |
| gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 5.54 | 0.19 |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + direct | 5.54 | 0.47 |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 5.54 | 0.21 |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + direct | 5.55 | 0.32 |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + direct | 5.56 | 0.15 |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 5.56 | 0.33 |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + direct | 5.56 | 0.25 |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 5.57 | 0.08 |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 5.57 | 0.05 |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 5.58 | 0.23 |  |
| gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 5.58 | 0.16 |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + direct | 5.58 | 0.23 |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + direct | 5.59 | 0.22 |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 5.61 | 0.15 |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 5.61 | 0.10 |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 5.61 | 0.25 |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 5.61 | 0.60 |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 5.62 | 0.04 |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 5.62 | 0.12 |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 5.63 | 0.09 |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 5.64 | 0.12 |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 5.64 | 0.47 |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 5.64 | 0.06 |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 5.64 | 0.06 |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 5.66 | 0.62 |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 5.66 | 0.51 |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 5.66 | 0.17 |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 5.66 | 0.46 |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 5.66 | 0.44 |  |
| gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 5.67 | 0.33 |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 5.68 | 0.08 |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 5.69 | 0.46 |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 5.70 | 0.54 |  |
| gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 5.71 | 0.15 |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 5.71 | 0.20 |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 5.71 | 0.05 |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 5.72 | 0.32 |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 5.72 | 0.46 |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 5.72 | 0.62 |  |
| gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 5.72 | 0.18 |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 5.72 | 0.32 |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 5.72 | 0.27 |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + direct | 5.72 | 0.12 |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 5.72 | 0.08 |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 5.72 | 0.32 |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 5.73 | 0.30 |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 5.73 | 0.52 |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 5.74 | 0.29 |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 5.74 | 0.60 |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 5.75 | 0.26 |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 5.76 | 0.47 |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 5.76 | 0.43 |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 5.76 | 0.26 |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 5.78 | 0.27 |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 5.79 | 0.32 |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 5.79 | 0.29 |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 5.80 | 0.26 |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 5.81 | 0.04 |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 5.82 | 0.26 |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 5.83 | 0.25 |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 5.83 | 0.05 |  |
| gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 5.83 | 0.26 |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 5.84 | 0.55 |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 5.89 | 0.23 |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 5.91 | 0.23 |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 5.92 | 0.14 |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 5.93 | 0.26 |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 5.93 | 0.15 |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 5.96 | 0.30 |  |
| gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 5.96 | 0.23 |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 5.96 | 0.36 |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 5.96 | 0.13 |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 5.97 | 0.46 |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 5.97 | 0.18 |  |
| gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 5.98 | 0.14 |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 5.98 | 0.21 |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 5.98 | 0.38 |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 6.00 | 0.13 |  |
| gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 6.00 | 0.18 |  |
| gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 6.00 | 0.15 |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 6.00 | 0.28 |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 6.00 | 0.19 |  |
| gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 6.00 | 0.13 |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 6.01 | 0.20 |  |
| gpu: svo_dag + refloor + quadtree + visbuffer + direct | 6.01 | 0.43 |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 6.02 | 0.16 |  |
| gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 6.03 | 0.36 |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 6.03 | 0.18 |  |
| gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 6.04 | 0.13 |  |
| gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 6.05 | 0.34 |  |
| gpu: svo_dag + refloor + quadtree + raster + direct | 6.05 | 0.32 |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 6.05 | 0.12 |  |
| gpu: svo_dag + refloor + quadtree + gbuffer + direct | 6.05 | 0.43 |  |
| gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 6.06 | 0.19 |  |
| gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 6.07 | 0.20 |  |
| gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 6.07 | 0.28 |  |
| gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 6.07 | 0.37 |  |
| gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 6.08 | 0.34 |  |
| gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 6.10 | 0.21 |  |
| gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 6.12 | 0.18 |  |
| gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 6.12 | 0.12 |  |
| gpu: svo_dag + sphere + quadtree + visbuffer + direct | 6.13 | 0.30 |  |
| gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 6.14 | 0.23 |  |
| gpu: svo_dag + sphere + quadtree + gbuffer + direct | 6.15 | 0.30 |  |
| gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 6.17 | 0.16 |  |
| gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 6.18 | 0.10 |  |
| gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 6.21 | 0.23 |  |
| gpu: svo_dag + incremental + quadtree + visbuffer + direct | 6.24 | 0.28 |  |
| gpu: svo_dag + rowrun + quadtree + raster + direct | 6.24 | 0.13 |  |
| gpu: svo_dag + refloor + quadtree + raster + pathtrace | 6.24 | 0.23 |  |
| gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 6.25 | 0.20 |  |
| gpu: svo_dag + rowrun + quadtree + visbuffer + direct | 6.25 | 0.27 |  |
| gpu: svo_dag + sphere + quadtree + raster + pathtrace | 6.25 | 0.14 |  |
| gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 6.26 | 0.23 |  |
| gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 6.26 | 0.19 |  |
| gpu: svo_dag + incremental + quadtree + raster + pathtrace | 6.27 | 0.13 |  |
| gpu: tree64 + refloor + quadtree + raster + direct | 6.27 | 0.34 |  |
| gpu: svo_dag + sphere + quadtree + raster + direct | 6.28 | 0.19 |  |
| gpu: svo_dag + incremental + quadtree + gbuffer + direct | 6.28 | 0.28 |  |
| gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 6.29 | 0.20 |  |
| gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 6.29 | 0.19 |  |
| gpu: svo_dag + rowrun + quadtree + gbuffer + direct | 6.29 | 0.27 |  |
| gpu: tree64 + refloor + quadtree + gbuffer + direct | 6.31 | 0.47 |  |
| gpu: tree64 + refloor + quadtree + visbuffer + direct | 6.31 | 0.47 |  |
| gpu: svo_dag + incremental + quadtree + raster + direct | 6.36 | 0.15 |  |
| gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 6.40 | 0.37 |  |
| gpu: tree64 + incremental + quadtree + visbuffer + direct | 6.40 | 0.38 |  |
| gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 6.45 | 0.29 |  |
| gpu: tree64 + incremental + quadtree + gbuffer + direct | 6.45 | 0.38 |  |
| gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 6.46 | 0.37 |  |
| gpu: tree64 + sphere + quadtree + gbuffer + direct | 6.47 | 0.30 |  |
| gpu: bricks + incremental + composite + raycast + direct | 6.50 | 0.59 |  |
| gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 6.51 | 0.29 |  |
| gpu: tree64 + incremental + quadtree + raster + pathtrace | 6.54 | 0.18 |  |
| gpu: bricks + refloor + composite + raycast + direct | 6.55 | 0.89 | far [lit], terrain-flight [lit] |
| gpu: tree64 + sphere + quadtree + visbuffer + direct | 6.57 | 0.30 |  |
| gpu: bricks + sphere + composite + raycast + direct | 6.57 | 0.49 |  |
| gpu: bricks + sphere + beam + composite + raycast + direct | 6.58 | 0.47 |  |
| gpu: bricks + rowrun + composite + raycast + direct | 6.58 | 0.72 |  |
| gpu: bricks + refloor + beam + composite + raycast + direct | 6.58 | 0.83 | far [lit], terrain-flight [lit] |
| gpu: tree64 + refloor + quadtree + raster + pathtrace | 6.58 | 0.24 |  |
| gpu: bricks + incremental + beam + composite + raycast + direct | 6.58 | 0.54 |  |
| gpu: tree64 + sphere + quadtree + raster + direct | 6.58 | 0.18 |  |
| gpu: bricks + rowrun + beam + composite + raycast + direct | 6.58 | 0.69 |  |
| gpu: hier + refloor + composite + raycast + direct | 6.59 | 0.85 | showcase [lit], terrain-flight [lit] |
| gpu: hier + sphere + composite + raycast + direct | 6.59 | 0.50 |  |
| gpu: hier + rowrun + composite + raycast + direct | 6.59 | 0.61 |  |
| gpu: hier + incremental + composite + raycast + direct | 6.59 | 0.53 |  |
| gpu: hier + sphere + beam + composite + raycast + direct | 6.59 | 0.48 |  |
| gpu: hier + refloor + beam + composite + raycast + direct | 6.59 | 0.82 |  |
| gpu: hier + rowrun + beam + composite + raycast + direct | 6.59 | 0.59 |  |
| gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 6.59 | 0.16 |  |
| gpu: hier + incremental + beam + composite + raycast + direct | 6.60 | 0.51 |  |
| gpu: tree64 + sphere + quadtree + raster + pathtrace | 6.60 | 0.13 |  |
| gpu: tree64 + rowrun + quadtree + raster + pathtrace | 6.60 | 0.09 |  |
| gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 6.61 | 0.16 |  |
| gpu: bricks + refloor + beam + composite + raycast + pathtrace | 6.64 | 0.61 |  |
| gpu: bricks + refloor + composite + raycast + pathtrace | 6.64 | 0.63 | far [unlit], terrain-flight [unlit] |
| gpu: hier + refloor + composite + raycast + pathtrace | 6.65 | 0.64 | showcase [unlit] |
| gpu: hier + refloor + beam + composite + raycast + pathtrace | 6.65 | 0.61 |  |
| gpu: rows + incremental + composite + raycast + direct | 6.67 | 0.50 |  |
| gpu: rows + rowrun + composite + raycast + direct | 6.67 | 0.73 |  |
| gpu: rows + sphere + beam + composite + raycast + direct | 6.67 | 0.43 |  |
| gpu: rows + refloor + composite + raycast + direct | 6.67 | 0.77 |  |
| gpu: rows + sphere + composite + raycast + direct | 6.67 | 0.45 |  |
| gpu: rows + refloor + beam + composite + raycast + direct | 6.68 | 0.74 |  |
| gpu: rows + rowrun + beam + composite + raycast + direct | 6.68 | 0.69 |  |
| gpu: tree64 + rowrun + quadtree + visbuffer + direct | 6.68 | 0.23 |  |
| gpu: rows + incremental + beam + composite + raycast + direct | 6.68 | 0.47 |  |
| gpu: prefix + sphere + composite + raycast + direct | 6.70 | 0.46 |  |
| gpu: prefix + rowrun + composite + raycast + direct | 6.70 | 0.80 |  |
| gpu: prefix + refloor + beam + composite + raycast + direct | 6.70 | 0.83 |  |
| gpu: prefix + refloor + composite + raycast + direct | 6.70 | 0.89 | showcase [lit] |
| gpu: tree64 + rowrun + quadtree + raster + direct | 6.70 | 0.12 |  |
| gpu: prefix + sphere + beam + composite + raycast + direct | 6.70 | 0.44 |  |
| gpu: prefix + incremental + composite + raycast + direct | 6.70 | 0.51 |  |
| gpu: prefix + incremental + beam + composite + raycast + direct | 6.70 | 0.48 |  |
| gpu: prefix + rowrun + beam + composite + raycast + direct | 6.70 | 0.76 |  |
| gpu: tree64 + rowrun + quadtree + gbuffer + direct | 6.71 | 0.23 |  |
| gpu: bricks + sphere + composite + raycast + pathtrace | 6.73 | 0.33 |  |
| gpu: rows + refloor + composite + raycast + pathtrace | 6.73 | 0.58 |  |
| gpu: bricks + sphere + beam + composite + raycast + pathtrace | 6.74 | 0.32 |  |
| gpu: bricks + incremental + composite + raycast + pathtrace | 6.74 | 0.43 |  |
| gpu: hier + sphere + composite + raycast + pathtrace | 6.74 | 0.35 |  |
| gpu: tree64 + incremental + quadtree + raster + direct | 6.74 | 0.23 |  |
| gpu: bricks + rowrun + composite + raycast + pathtrace | 6.75 | 0.47 |  |
| gpu: hier + sphere + beam + composite + raycast + pathtrace | 6.75 | 0.34 |  |
| gpu: hier + rowrun + composite + raycast + pathtrace | 6.75 | 0.42 |  |
| gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 6.75 | 0.45 |  |
| gpu: hier + incremental + composite + raycast + pathtrace | 6.76 | 0.42 |  |
| gpu: bricks + incremental + beam + composite + raycast + pathtrace | 6.76 | 0.41 |  |
| gpu: prefix + refloor + composite + raycast + pathtrace | 6.76 | 0.66 | showcase [unlit] |
| gpu: hier + rowrun + beam + composite + raycast + pathtrace | 6.76 | 0.40 |  |
| gpu: prefix + refloor + beam + composite + raycast + pathtrace | 6.76 | 0.63 |  |
| gpu: hier + incremental + beam + composite + raycast + pathtrace | 6.77 | 0.40 |  |
| gpu: clipmap + refloor + composite + raycast + direct | 6.77 | 0.69 |  |
| gpu: clipmap + rowrun + composite + raycast + direct | 6.77 | 0.60 |  |
| gpu: clipmap + incremental + composite + raycast + direct | 6.77 | 0.57 |  |
| gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 6.78 | 0.22 |  |
| gpu: clipmap + sphere + composite + raycast + direct | 6.78 | 0.39 |  |
| gpu: clipmap + rowrun + beam + composite + raycast + direct | 6.78 | 0.54 |  |
| gpu: rows + refloor + beam + composite + raycast + pathtrace | 6.78 | 0.55 |  |
| gpu: clipmap + refloor + beam + composite + raycast + direct | 6.78 | 0.60 |  |
| gpu: clipmap + incremental + beam + composite + raycast + direct | 6.78 | 0.50 |  |
| gpu: rows + rowrun + beam + composite + raycast + pathtrace | 6.82 | 0.48 |  |
| gpu: rows + sphere + composite + raycast + pathtrace | 6.82 | 0.31 |  |
| gpu: rows + rowrun + composite + raycast + pathtrace | 6.82 | 0.50 |  |
| gpu: rows + sphere + beam + composite + raycast + pathtrace | 6.83 | 0.30 |  |
| gpu: prefix + rowrun + composite + raycast + pathtrace | 6.83 | 0.55 |  |
| gpu: rows + incremental + composite + raycast + pathtrace | 6.84 | 0.38 |  |
| gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 6.84 | 0.53 |  |
| gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 6.84 | 0.22 |  |
| gpu: rows + incremental + beam + composite + raycast + pathtrace | 6.85 | 0.39 |  |
| gpu: prefix + sphere + composite + raycast + pathtrace | 6.85 | 0.32 |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + raster + direct | 6.86 | 0.48 |  |
| gpu: prefix + sphere + beam + composite + raycast + pathtrace | 6.86 | 0.31 |  |
| gpu: prefix + incremental + composite + raycast + pathtrace | 6.87 | 0.39 |  |
| gpu: clipmap + sphere + beam + composite + raycast + direct | 6.88 | 0.35 |  |
| gpu: prefix + incremental + beam + composite + raycast + pathtrace | 6.89 | 0.38 |  |
| gpu: clipmap + refloor + composite + raycast + pathtrace | 6.90 | 0.50 |  |
| gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 6.92 | 0.44 |  |
| gpu: clipmap + sphere + composite + raycast + pathtrace | 6.93 | 0.27 |  |
| gpu: clipmap + rowrun + composite + raycast + pathtrace | 6.93 | 0.40 |  |
| gpu: clipmap + incremental + composite + raycast + pathtrace | 6.94 | 0.39 |  |
| gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 6.94 | 0.24 |  |
| gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 6.94 | 0.36 |  |
| gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 6.96 | 0.34 |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + direct | 7.03 | 0.52 |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 7.10 | 0.36 |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + raster + direct | 7.11 | 0.34 |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + raster + direct | 7.12 | 0.46 |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + raster + direct | 7.12 | 0.34 |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + direct | 7.12 | 0.34 |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + raster + direct | 7.12 | 0.38 |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + raster + direct | 7.13 | 0.33 |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + raster + direct | 7.13 | 0.38 |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + raster + direct | 7.20 | 0.44 |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + raster + direct | 7.20 | 0.32 |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + raster + direct | 7.21 | 0.42 |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + raster + direct | 7.22 | 0.39 |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + raster + direct | 7.22 | 0.46 |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + raster + direct | 7.23 | 0.33 |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + raster + direct | 7.23 | 0.37 |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + direct | 7.24 | 0.41 |  |
| gpu: naive@cpu + faces + hier + refloor + depth + raster + direct | 7.26 | 0.50 |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 7.26 | 0.41 |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + raster + direct | 7.26 | 0.35 |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + raster + direct | 7.26 | 0.53 |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + raster + direct | 7.26 | 0.41 |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + raster + direct | 7.27 | 0.36 |  |
| gpu: naive@cpu + faces + hier + sphere + depth + raster + direct | 7.27 | 0.36 |  |
| gpu: naive@cpu + faces + hier + incremental + depth + raster + direct | 7.27 | 0.41 |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + raster + direct | 7.28 | 0.35 |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + direct | 7.28 | 0.43 |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + raster + pathtrace | 7.29 | 0.13 |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + direct | 7.29 | 0.66 |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + direct | 7.30 | 0.43 |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + direct | 7.30 | 0.51 |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + direct | 7.30 | 0.51 |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + direct | 7.30 | 0.51 |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + raster + pathtrace | 7.30 | 0.25 |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 7.30 | 0.60 |  |
| gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + direct | 7.30 | 0.62 |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 7.30 | 0.20 |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + direct | 7.30 | 0.43 |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 7.30 | 0.12 |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + raster + pathtrace | 7.31 | 0.12 |  |
| gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + direct | 7.31 | 0.50 |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + direct | 7.31 | 0.46 |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + direct | 7.31 | 0.54 |  |
| gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + direct | 7.31 | 0.47 |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + raster + pathtrace | 7.31 | 0.11 |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 7.31 | 0.41 |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + raster + pathtrace | 7.31 | 0.15 |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + raster + pathtrace | 7.33 | 0.15 |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + direct | 7.33 | 0.64 |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct | 7.33 | 0.20 |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 7.33 | 0.50 |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + direct | 7.33 | 0.51 |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 7.33 | 0.38 | showcase [unlit] |
| gpu: cubes@cpu + faces + hier + refloor + depth + raster + direct | 7.33 | 0.21 |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + raster + direct | 7.33 | 0.16 |  |
| gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + direct | 7.33 | 0.52 |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + direct | 7.33 | 0.50 |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + raster + direct | 7.33 | 0.16 |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + direct | 7.34 | 0.32 |  |
| gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + direct | 7.34 | 0.61 |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + direct | 7.34 | 0.43 |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + raster + direct | 7.34 | 0.18 |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 7.34 | 0.60 |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + direct | 7.34 | 0.16 |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + raster + direct | 7.34 | 0.17 |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + direct | 7.34 | 0.37 |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + raster + direct | 7.34 | 0.46 |  |
| gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + direct | 7.34 | 0.50 |  |
| gpu: naive@cpu + faces + rows + sphere + depth + raster + direct | 7.34 | 0.34 |  |
| gpu: naive@cpu + faces + rows + refloor + depth + raster + direct | 7.34 | 0.49 |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + direct | 7.35 | 0.45 |  |
| gpu: naive@cpu + faces + rows + incremental + depth + raster + direct | 7.35 | 0.42 |  |
| gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + direct | 7.35 | 0.47 |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + direct | 7.35 | 0.36 |  |
| gpu: points@cpu + splat + svo_dag + refloor + direct | 7.35 | 0.29 |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + raster + direct | 7.35 | 0.18 |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + direct | 7.35 | 0.54 |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + direct | 7.36 | 0.43 |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + raster + direct | 7.36 | 0.52 |  |
| gpu: points@cpu + splat + svo_dag + sphere + direct | 7.36 | 0.24 |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + direct | 7.37 | 0.41 |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + direct | 7.37 | 0.43 |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + raster + direct | 7.37 | 0.34 |  |
| gpu: points@cpu + splat + svo_dag + rowrun + direct | 7.37 | 0.26 |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + direct | 7.38 | 0.49 |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + direct | 7.38 | 0.37 |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + raster + pathtrace | 7.38 | 0.12 |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + raster + direct | 7.38 | 0.39 |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + direct | 7.38 | 0.58 |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 7.38 | 0.50 |  |
| gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + direct | 7.38 | 0.45 |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + direct | 7.38 | 0.41 |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + raster + direct | 7.38 | 0.43 |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + raster + pathtrace | 7.38 | 0.19 |  |
| gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + direct | 7.38 | 0.59 |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + direct | 7.38 | 0.49 |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 7.38 | 0.50 |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + direct | 7.38 | 0.51 |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + raster + pathtrace | 7.38 | 0.23 |  |
| gpu: distance + refloor + beam + composite + raycast + direct | 7.38 | 0.70 |  |
| gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + direct | 7.38 | 0.47 |  |
| gpu: distance + refloor + composite + raycast + direct | 7.38 | 0.76 | showcase [lit] |
| gpu: distance + sphere + composite + raycast + direct | 7.38 | 0.81 | showcase [lit] |
| gpu: distance + rowrun + composite + raycast + direct | 7.38 | 0.69 |  |
| gpu: distance + sphere + beam + composite + raycast + direct | 7.38 | 0.77 | showcase [lit] |
| gpu: distance + rowrun + beam + composite + raycast + direct | 7.39 | 0.65 |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + direct | 7.40 | 0.49 |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + raster + pathtrace | 7.40 | 0.16 |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + raster + pathtrace | 7.40 | 0.12 |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + raster + pathtrace | 7.40 | 0.25 |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + direct | 7.41 | 0.51 |  |
| gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + direct | 7.41 | 0.60 |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + direct | 7.41 | 0.63 |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 7.41 | 0.61 |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + raster + direct | 7.41 | 0.20 |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + direct | 7.41 | 0.41 |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + direct | 7.41 | 0.58 |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + raster + direct | 7.41 | 0.16 |  |
| gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + direct | 7.41 | 0.46 |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + direct | 7.41 | 0.48 |  |
| gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + direct | 7.41 | 0.47 |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + direct | 7.42 | 0.47 |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + raster + direct | 7.42 | 0.20 |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 7.42 | 0.09 |  |
| gpu: points@cpu + splat + svo_dag + incremental + direct | 7.42 | 0.24 |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + raster + pathtrace | 7.42 | 0.09 |  |
| gpu: naive@cpu + faces + hier + incremental + depth + raster + pathtrace | 7.42 | 0.13 |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 7.42 | 0.19 |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + raster + pathtrace | 7.43 | 0.10 |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + raster + pathtrace | 7.43 | 0.13 |  |
| gpu: naive@cpu + faces + hier + sphere + depth + raster + pathtrace | 7.43 | 0.11 |  |
| gpu: distance + sphere + composite + raycast + pathtrace | 7.43 | 0.63 |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + raster + direct | 7.44 | 0.18 |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + raster + direct | 7.44 | 0.21 |  |
| gpu: naive@cpu + faces + hier + refloor + depth + raster + pathtrace | 7.44 | 0.23 |  |
| gpu: distance + sphere + beam + composite + raycast + pathtrace | 7.44 | 0.60 |  |
| gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 7.44 | 0.49 |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + direct | 7.44 | 0.62 |  |
| gpu: distance + refloor + composite + raycast + pathtrace | 7.44 | 0.57 |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + raster + pathtrace | 7.44 | 0.15 |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 7.44 | 0.61 |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + raster + direct | 7.44 | 0.16 |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + direct | 7.44 | 0.47 |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + raster + direct | 7.44 | 0.18 |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 7.44 | 0.28 |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + direct | 7.44 | 0.49 |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 7.45 | 0.50 |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + direct | 7.45 | 0.41 |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + direct | 7.45 | 0.20 |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + direct | 7.46 | 0.48 |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 7.47 | 0.51 |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + raster + pathtrace | 7.48 | 0.06 |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + direct | 7.48 | 0.52 |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + direct | 7.48 | 0.42 |  |
| gpu: points@cpu + splat + tree64 + refloor + direct | 7.48 | 0.29 |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + raster + pathtrace | 7.49 | 0.07 |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 7.49 | 0.07 |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + raster + pathtrace | 7.50 | 0.07 |  |
| gpu: distance + refloor + beam + composite + raycast + pathtrace | 7.50 | 0.54 |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 7.50 | 0.51 |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + raster + pathtrace | 7.50 | 0.17 |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + raster + pathtrace | 7.50 | 0.08 |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + direct | 7.50 | 0.40 |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 7.50 | 0.40 |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + raster + pathtrace | 7.51 | 0.13 |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + direct | 7.51 | 0.39 |  |
| gpu: naive@cpu + faces + rows + refloor + depth + raster + pathtrace | 7.51 | 0.22 |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + raster + pathtrace | 7.51 | 0.08 |  |
| gpu: points@cpu + splat + tree64 + sphere + direct | 7.52 | 0.24 |  |
| gpu: naive@cpu + faces + rows + sphere + depth + raster + pathtrace | 7.52 | 0.09 |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 7.52 | 0.13 |  |
| gpu: naive@cpu + faces + rows + incremental + depth + raster + pathtrace | 7.52 | 0.14 |  |
| gpu: distance + incremental + beam + composite + raycast + direct | 7.52 | 0.41 |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 7.52 | 0.20 |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + direct | 7.52 | 0.41 |  |
| gpu: distance + rowrun + composite + raycast + pathtrace | 7.52 | 0.48 |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 7.52 | 0.14 |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 7.53 | 0.48 |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 7.53 | 0.47 |  |
| gpu: distance + rowrun + beam + composite + raycast + pathtrace | 7.53 | 0.46 |  |
| gpu: distance + incremental + composite + raycast + pathtrace | 7.53 | 0.34 |  |
| gpu: distance + incremental + composite + raycast + direct | 7.53 | 0.44 |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 7.53 | 0.10 |  |
| gpu: points@cpu + splat + tree64 + rowrun + direct | 7.54 | 0.23 |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + raster + pathtrace | 7.54 | 0.24 |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + raster + pathtrace | 7.54 | 0.09 |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + raster + pathtrace | 7.54 | 0.13 |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 7.54 | 0.27 |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 7.54 | 0.16 |  |
| gpu: distance + incremental + beam + composite + raycast + pathtrace | 7.54 | 0.33 |  |
| gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 7.55 | 0.39 |  |
| gpu: points@cpu + splat + tree64 + incremental + direct | 7.55 | 0.27 |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 7.56 | 0.22 |  |
| gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 7.56 | 0.24 |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + direct | 7.56 | 0.29 |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + direct | 7.56 | 0.41 |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + raster + pathtrace | 7.56 | 0.06 |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + direct | 7.57 | 0.34 |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + direct | 7.57 | 0.34 |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + raster + pathtrace | 7.57 | 0.10 |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + direct | 7.57 | 0.28 |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 7.57 | 0.47 |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 7.58 | 0.40 |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 7.58 | 0.28 |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + direct | 7.58 | 0.31 |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + direct | 7.58 | 0.39 |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 7.58 | 0.25 |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + raster + pathtrace | 7.58 | 0.12 |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 7.58 | 0.25 |  |
| gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 7.58 | 0.26 |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 7.59 | 0.34 |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + raster + pathtrace | 7.59 | 0.09 |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 7.59 | 0.20 |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + direct | 7.59 | 0.28 |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + raster + pathtrace | 7.60 | 0.08 |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + direct | 7.60 | 0.29 |  |
| gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 7.60 | 0.22 |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 7.60 | 0.47 |  |
| gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 7.60 | 0.24 |  |
| gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 7.60 | 0.39 |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + raster + pathtrace | 7.60 | 0.06 |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 7.60 | 0.22 |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 7.60 | 0.48 |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + raster + pathtrace | 7.60 | 0.13 |  |
| gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 7.61 | 0.24 |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 7.61 | 0.10 |  |
| gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 7.62 | 0.25 |  |
| gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 7.62 | 0.25 |  |
| gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 7.62 | 0.35 |  |
| gpu: points@cpu + splat + svo_dag + refloor + pathtrace | 7.63 | 0.21 |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 7.64 | 0.32 |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + direct | 7.64 | 0.28 |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + direct | 7.64 | 0.31 |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 7.64 | 0.23 |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 7.65 | 0.35 |  |
| gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 7.65 | 0.22 |  |
| gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 7.65 | 0.28 |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + direct | 7.65 | 0.29 |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 7.66 | 0.40 |  |
| gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 7.66 | 0.34 |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 7.66 | 0.23 |  |
| gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + direct | 7.66 | 0.34 |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 7.66 | 0.31 |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 7.66 | 0.23 |  |
| gpu: points@cpu + splat + svo_dag + sphere + pathtrace | 7.66 | 0.14 |  |
| gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 7.66 | 0.20 |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + direct | 7.67 | 0.34 |  |
| gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 7.67 | 0.36 |  |
| gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 7.67 | 0.23 |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 7.67 | 0.21 |  |
| gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 7.67 | 0.25 |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 7.68 | 0.34 |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 7.68 | 0.32 |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + direct | 7.68 | 0.28 |  |
| gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 7.68 | 0.31 |  |
| gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 7.68 | 0.22 |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + direct | 7.68 | 0.29 |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + direct | 7.68 | 0.33 |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 7.68 | 0.22 |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + direct | 7.69 | 0.34 |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + direct | 7.69 | 0.33 |  |
| gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 7.69 | 0.22 |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 7.69 | 0.25 |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + direct | 7.69 | 0.28 |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + direct | 7.69 | 0.28 |  |
| gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 7.69 | 0.40 |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 7.69 | 0.28 |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 7.70 | 0.20 |  |
| gpu: points@cpu + splat + svo_dag + rowrun + pathtrace | 7.70 | 0.15 |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 7.70 | 0.32 |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 7.70 | 0.24 |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + direct | 7.70 | 0.28 |  |
| gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 7.71 | 0.34 |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + direct | 7.71 | 0.20 |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + direct | 7.71 | 0.15 |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + direct | 7.71 | 0.28 |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 7.71 | 0.34 |  |
| gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 7.72 | 0.22 |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + direct | 7.72 | 0.17 |  |
| gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 7.72 | 0.35 |  |
| gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 7.72 | 0.24 |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + direct | 7.73 | 0.32 |  |
| gpu: points@cpu + splat + svo_dag + incremental + pathtrace | 7.73 | 0.13 |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + direct | 7.73 | 0.17 |  |
| gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 7.73 | 0.28 |  |
| gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 7.73 | 0.24 |  |
| gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 7.73 | 0.23 |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + direct | 7.73 | 0.33 |  |
| gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 7.73 | 0.24 |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 7.73 | 0.22 |  |
| gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 7.73 | 0.23 |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + direct | 7.74 | 0.28 |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 7.74 | 0.22 |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + direct | 7.74 | 0.34 |  |
| gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 7.74 | 0.20 |  |
| gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 7.74 | 0.31 |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 7.75 | 0.34 |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 7.75 | 0.30 |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + direct | 7.75 | 0.28 |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 7.75 | 0.25 |  |
| gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 7.75 | 0.30 |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 7.76 | 0.16 |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 7.76 | 0.15 |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 7.76 | 0.28 |  |
| gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 7.77 | 0.20 |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + direct | 7.79 | 0.28 |  |
| gpu: points@cpu + splat + tree64 + refloor + pathtrace | 7.79 | 0.22 |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 7.79 | 0.17 |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 7.80 | 0.17 |  |
| gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 7.80 | 0.22 |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 7.81 | 0.17 |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + direct | 7.81 | 0.27 |  |
| gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 7.81 | 0.22 |  |
| gpu: points@cpu + splat + tree64 + sphere + pathtrace | 7.81 | 0.14 |  |
| gpu: points@cpu + splat + tree64 + incremental + pathtrace | 7.82 | 0.18 |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 7.82 | 0.24 |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 7.83 | 0.23 |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + raster + direct | 7.83 | 0.32 |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + raster + direct | 7.83 | 0.47 |  |
| gpu: points@cpu + splat + tree64 + rowrun + pathtrace | 7.83 | 0.12 |  |
| gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 7.83 | 0.28 |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + direct | 7.84 | 0.37 |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 7.85 | 0.16 |  |
| gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 7.85 | 0.16 |  |
| gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 7.85 | 0.17 |  |
| gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 7.85 | 0.24 |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + raster + direct | 7.86 | 0.38 |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 7.86 | 0.25 |  |
| gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 7.86 | 0.17 |  |
| gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 7.87 | 0.15 |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 7.87 | 0.08 |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 7.87 | 0.07 |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 7.87 | 0.23 |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 7.87 | 0.18 |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 7.87 | 0.11 |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 7.87 | 0.15 |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 7.87 | 0.15 |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 7.88 | 0.20 |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 7.89 | 0.05 |  |
| gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 7.89 | 0.24 |  |
| gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 7.89 | 0.16 |  |
| gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 7.90 | 0.22 |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + raster + direct | 7.91 | 0.43 |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + raster + direct | 7.91 | 0.44 |  |
| gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 7.91 | 0.20 |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + raster + direct | 7.92 | 0.38 |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 7.92 | 0.17 |  |
| gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 7.93 | 0.17 |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 7.93 | 0.16 |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + raster + direct | 7.93 | 0.35 |  |
| gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 7.93 | 0.25 |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 7.93 | 0.22 |  |
| gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 7.94 | 0.23 |  |
| gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 7.94 | 0.15 |  |
| gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 7.94 | 0.18 |  |
| gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 7.94 | 0.15 |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 7.95 | 0.33 |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 7.96 | 0.60 |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 7.96 | 0.26 |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 7.98 | 0.57 |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 7.98 | 0.55 |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 7.99 | 0.47 |  |
| gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 7.99 | 0.17 |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 7.99 | 0.31 |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 8.00 | 0.10 |  |
| gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 8.00 | 0.22 |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 8.00 | 0.31 |  |
| gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 8.00 | 0.16 |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 8.01 | 0.12 |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 8.01 | 0.18 |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 8.02 | 0.33 |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 8.02 | 0.08 |  |
| gpu: naive@cpu + faces + distance + sphere + depth + raster + direct | 8.02 | 0.49 |  |
| gpu: naive@cpu + faces + distance + refloor + depth + raster + direct | 8.02 | 0.48 |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 8.03 | 0.59 |  |
| gpu: bricks + refloor + quadtree + raster + direct | 8.03 | 0.40 |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 8.03 | 0.27 |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + raster + direct | 8.03 | 0.40 |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 8.03 | 0.48 |  |
| gpu: naive@cpu + faces + distance + incremental + depth + raster + direct | 8.03 | 0.36 |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + direct | 8.05 | 0.54 |  |
| gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + direct | 8.05 | 0.56 |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 8.06 | 0.55 |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 8.06 | 0.58 |  |
| gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + direct | 8.06 | 0.56 |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 8.07 | 0.31 |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 8.07 | 0.31 |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + raster + pathtrace | 8.09 | 0.24 |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + direct | 8.09 | 0.47 |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + raster + pathtrace | 8.09 | 0.26 |  |
| gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + direct | 8.09 | 0.55 |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + direct | 8.10 | 0.47 |  |
| gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + direct | 8.10 | 0.57 |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + raster + pathtrace | 8.10 | 0.18 |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + direct | 8.10 | 0.54 |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + raster + direct | 8.11 | 0.21 |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + raster + direct | 8.11 | 0.21 |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + direct | 8.11 | 0.46 |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + raster + pathtrace | 8.11 | 0.14 |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + direct | 8.12 | 0.35 |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + raster + direct | 8.12 | 0.19 |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 8.13 | 0.37 |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + raster + direct | 8.13 | 0.18 |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 8.14 | 0.20 |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + direct | 8.15 | 0.47 |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 8.15 | 0.13 |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 8.16 | 0.35 |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + direct | 8.18 | 0.48 |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + direct | 8.19 | 0.46 |  |
| gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + direct | 8.19 | 0.41 |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 8.19 | 0.17 |  |
| gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + direct | 8.19 | 0.41 |  |
| gpu: naive@cpu + faces + distance + sphere + depth + raster + pathtrace | 8.19 | 0.25 |  |
| gpu: naive@cpu + faces + distance + refloor + depth + raster + pathtrace | 8.20 | 0.23 |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + raster + pathtrace | 8.20 | 0.16 |  |
| gpu: naive@cpu + faces + distance + incremental + depth + raster + pathtrace | 8.20 | 0.12 |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 8.21 | 0.16 |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + direct | 8.21 | 0.36 |  |
| gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 8.21 | 0.20 |  |
| gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 8.22 | 0.13 |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 8.23 | 0.25 |  |
| gpu: bricks + refloor + quadtree + visbuffer + direct | 8.24 | 0.54 |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 8.26 | 0.32 |  |
| gpu: bricks + refloor + quadtree + gbuffer + direct | 8.26 | 0.54 |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + direct | 8.26 | 0.48 |  |
| gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 8.27 | 0.17 |  |
| gpu: hier + refloor + quadtree + raster + direct | 8.28 | 0.39 |  |
| gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 8.28 | 0.16 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + refloor + direct | 8.28 | 0.04 |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + raster + pathtrace | 8.28 | 0.14 |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 8.28 | 0.18 |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + raster + pathtrace | 8.28 | 0.10 |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + raster + pathtrace | 8.29 | 0.13 |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + raster + pathtrace | 8.29 | 0.08 |  |
| gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + direct | 8.29 | 0.47 |  |
| gpu: bricks + sphere + quadtree + visbuffer + direct | 8.29 | 0.38 |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + direct | 8.29 | 0.36 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + rowrun + direct | 8.29 | 0.04 |  |
| gpu: hier + refloor + quadtree + visbuffer + direct | 8.29 | 0.52 |  |
| gpu: hier + sphere + quadtree + visbuffer + direct | 8.30 | 0.37 |  |
| gpu: bricks + rowrun + quadtree + visbuffer + direct | 8.30 | 0.43 |  |
| gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 8.30 | 0.40 |  |
| gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 8.31 | 0.38 |  |
| gpu: bricks + sphere + quadtree + gbuffer + direct | 8.32 | 0.38 |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 8.32 | 0.26 |  |
| gpu: hier + sphere + quadtree + gbuffer + direct | 8.33 | 0.38 |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 8.33 | 0.32 |  |
| gpu: hier + refloor + quadtree + gbuffer + direct | 8.33 | 0.51 |  |
| gpu: bricks + rowrun + quadtree + gbuffer + direct | 8.33 | 0.44 |  |
| gpu: points@cpu + splat + bricks + refloor + direct | 8.33 | 0.30 |  |
| gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 8.33 | 0.31 |  |
| gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 8.33 | 0.37 |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 8.33 | 0.25 |  |
| gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 8.34 | 0.21 |  |
| gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 8.34 | 0.40 |  |
| gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 8.34 | 0.36 |  |
| gpu: hier + refloor + quadtree + visbuffer + pathtrace | 8.34 | 0.37 |  |
| gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 8.35 | 0.18 |  |
| gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 8.35 | 0.31 |  |
| gpu: rows + rowrun + quadtree + visbuffer + direct | 8.36 | 0.46 |  |
| gpu: rows + refloor + quadtree + raster + direct | 8.37 | 0.36 |  |
| gpu: rows + refloor + quadtree + visbuffer + direct | 8.37 | 0.49 |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 8.38 | 0.33 |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 8.38 | 0.35 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + incremental + direct | 8.38 | 0.04 |  |
| gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 8.38 | 0.21 |  |
| gpu: hier + rowrun + quadtree + visbuffer + direct | 8.38 | 0.39 |  |
| gpu: hier + refloor + quadtree + gbuffer + pathtrace | 8.39 | 0.36 |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + direct | 8.39 | 0.33 |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + direct | 8.39 | 0.33 |  |
| gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 8.39 | 0.36 |  |
| gpu: rows + rowrun + quadtree + raster + direct | 8.39 | 0.33 |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + direct | 8.39 | 0.32 |  |
| gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 8.40 | 0.26 |  |
| gpu: prefix + refloor + quadtree + raster + direct | 8.40 | 0.40 |  |
| gpu: bricks + rowrun + quadtree + raster + pathtrace | 8.40 | 0.15 |  |
| gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 8.40 | 0.33 |  |
| gpu: rows + refloor + quadtree + gbuffer + direct | 8.40 | 0.47 |  |
| gpu: hier + incremental + quadtree + raster + direct | 8.40 | 0.28 |  |
| gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 8.40 | 0.25 |  |
| gpu: bricks + refloor + quadtree + raster + pathtrace | 8.40 | 0.23 |  |
| gpu: rows + rowrun + quadtree + gbuffer + direct | 8.40 | 0.46 |  |
| gpu: bricks + incremental + quadtree + visbuffer + direct | 8.40 | 0.38 |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + direct | 8.41 | 0.26 |  |
| gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 8.42 | 0.34 |  |
| gpu: clipmap + refloor + quadtree + visbuffer + direct | 8.42 | 0.45 |  |
| gpu: prefix + rowrun + quadtree + visbuffer + direct | 8.42 | 0.47 |  |
| gpu: hier + rowrun + quadtree + gbuffer + direct | 8.42 | 0.39 |  |
| gpu: bricks + incremental + quadtree + raster + direct | 8.42 | 0.28 |  |
| gpu: prefix + refloor + quadtree + visbuffer + direct | 8.42 | 0.52 |  |
| gpu: hier + sphere + quadtree + raster + direct | 8.42 | 0.24 |  |
| gpu: bricks + rowrun + quadtree + raster + direct | 8.42 | 0.23 |  |
| gpu: clipmap + rowrun + quadtree + raster + pathtrace | 8.43 | 0.17 |  |
| gpu: bricks + sphere + quadtree + raster + direct | 8.43 | 0.22 |  |
| gpu: hier + rowrun + quadtree + raster + pathtrace | 8.43 | 0.14 |  |
| gpu: clipmap + refloor + quadtree + raster + direct | 8.43 | 0.32 |  |
| gpu: rows + refloor + quadtree + visbuffer + pathtrace | 8.44 | 0.35 |  |
| gpu: clipmap + rowrun + quadtree + visbuffer + direct | 8.44 | 0.41 |  |
| gpu: clipmap + incremental + quadtree + visbuffer + direct | 8.44 | 0.39 |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + direct | 8.44 | 0.32 |  |
| gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 8.44 | 0.26 |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 8.44 | 0.29 |  |
| gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 8.44 | 0.27 |  |
| gpu: hier + rowrun + quadtree + raster + direct | 8.44 | 0.23 |  |
| gpu: prefix + refloor + quadtree + gbuffer + direct | 8.44 | 0.52 |  |
| gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 8.44 | 0.37 |  |
| gpu: prefix + rowrun + quadtree + gbuffer + direct | 8.45 | 0.47 |  |
| gpu: hier + incremental + quadtree + visbuffer + direct | 8.45 | 0.37 |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 8.45 | 0.19 |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + direct | 8.45 | 0.33 |  |
| gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 8.45 | 0.36 |  |
| gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 8.45 | 0.34 |  |
| gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 8.46 | 0.23 |  |
| gpu: rows + incremental + quadtree + raster + direct | 8.46 | 0.29 |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + direct | 8.46 | 0.33 |  |
| gpu: bricks + incremental + quadtree + gbuffer + direct | 8.46 | 0.38 |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + direct | 8.46 | 0.26 |  |
| gpu: hier + incremental + quadtree + visbuffer + pathtrace | 8.46 | 0.26 |  |
| gpu: clipmap + incremental + quadtree + gbuffer + direct | 8.46 | 0.39 |  |
| gpu: clipmap + refloor + quadtree + gbuffer + direct | 8.46 | 0.44 |  |
| gpu: hier + sphere + quadtree + visbuffer + pathtrace | 8.47 | 0.24 |  |
| gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 8.47 | 0.28 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + sphere + direct | 8.47 | 0.04 |  |
| gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 8.47 | 0.25 |  |
| gpu: hier + incremental + quadtree + gbuffer + direct | 8.47 | 0.36 |  |
| gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 8.47 | 0.27 |  |
| gpu: clipmap + rowrun + quadtree + gbuffer + direct | 8.47 | 0.40 |  |
| gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 8.47 | 0.29 |  |
| gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 8.48 | 0.37 |  |
| gpu: boxes@cpu + rayquery + raycast + direct | 8.48 | 0.55 |  |
| gpu: rows + refloor + quadtree + gbuffer + pathtrace | 8.48 | 0.34 |  |
| gpu: prefix + rowrun + quadtree + raster + direct | 8.48 | 0.32 |  |
| gpu: rows + rowrun + quadtree + raster + pathtrace | 8.49 | 0.20 |  |
| gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 8.49 | 0.28 |  |
| gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 8.49 | 0.30 |  |
| gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 8.49 | 0.23 |  |
| gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 8.49 | 0.31 |  |
| gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 8.50 | 0.23 |  |
| gpu: hier + sphere + quadtree + gbuffer + pathtrace | 8.50 | 0.24 |  |
| gpu: rows + sphere + quadtree + visbuffer + direct | 8.50 | 0.35 |  |
| gpu: hier + incremental + quadtree + gbuffer + pathtrace | 8.51 | 0.26 |  |
| gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 8.51 | 0.27 |  |
| gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 8.51 | 0.24 |  |
| gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 8.51 | 0.29 |  |
| gpu: clipmap + sphere + quadtree + visbuffer + direct | 8.51 | 0.33 |  |
| gpu: prefix + rowrun + quadtree + raster + pathtrace | 8.51 | 0.19 |  |
| gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 8.52 | 0.23 |  |
| gpu: prefix + incremental + quadtree + raster + direct | 8.52 | 0.26 |  |
| gpu: rows + sphere + quadtree + raster + direct | 8.52 | 0.22 |  |
| gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 8.52 | 0.19 |  |
| gpu: prefix + sphere + quadtree + visbuffer + direct | 8.52 | 0.36 |  |
| gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 8.52 | 0.31 |  |
| gpu: prefix + sphere + quadtree + gbuffer + direct | 8.52 | 0.36 |  |
| gpu: rows + incremental + quadtree + visbuffer + direct | 8.53 | 0.36 |  |
| gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 8.53 | 0.24 |  |
| gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 8.53 | 0.31 |  |
| gpu: clipmap + rowrun + quadtree + raster + direct | 8.53 | 0.24 |  |
| gpu: rows + sphere + quadtree + gbuffer + direct | 8.53 | 0.35 |  |
| gpu: bricks + incremental + quadtree + raster + pathtrace | 8.54 | 0.17 |  |
| gpu: clipmap + sphere + quadtree + gbuffer + direct | 8.54 | 0.32 |  |
| gpu: hier + incremental + quadtree + raster + pathtrace | 8.55 | 0.17 |  |
| gpu: hier + refloor + quadtree + raster + pathtrace | 8.55 | 0.24 |  |
| gpu: rows + incremental + quadtree + visbuffer + pathtrace | 8.55 | 0.26 |  |
| gpu: bricks + sphere + quadtree + raster + pathtrace | 8.55 | 0.14 |  |
| gpu: prefix + sphere + quadtree + raster + direct | 8.55 | 0.22 |  |
| gpu: rows + incremental + quadtree + gbuffer + direct | 8.56 | 0.36 |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 8.57 | 0.24 |  |
| gpu: rows + sphere + quadtree + visbuffer + pathtrace | 8.57 | 0.23 |  |
| gpu: hier + sphere + quadtree + raster + pathtrace | 8.57 | 0.15 |  |
| gpu: clipmap + incremental + quadtree + raster + direct | 8.57 | 0.23 |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 8.57 | 0.26 |  |
| gpu: rows + incremental + quadtree + gbuffer + pathtrace | 8.57 | 0.26 |  |
| gpu: clipmap + refloor + quadtree + raster + pathtrace | 8.58 | 0.23 |  |
| gpu: clipmap + sphere + quadtree + raster + pathtrace | 8.58 | 0.14 |  |
| gpu: prefix + incremental + quadtree + visbuffer + direct | 8.58 | 0.36 |  |
| gpu: clipmap + incremental + quadtree + raster + pathtrace | 8.58 | 0.17 |  |
| gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 8.59 | 0.23 |  |
| gpu: rows + sphere + quadtree + gbuffer + pathtrace | 8.59 | 0.23 |  |
| gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 8.59 | 0.25 |  |
| gpu: clipmap + sphere + quadtree + raster + direct | 8.61 | 0.18 |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 8.62 | 0.22 |  |
| gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 8.62 | 0.23 |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 8.63 | 0.15 |  |
| gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 8.63 | 0.24 |  |
| gpu: rows + incremental + quadtree + raster + pathtrace | 8.64 | 0.17 |  |
| gpu: rows + refloor + quadtree + raster + pathtrace | 8.64 | 0.23 |  |
| gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 8.64 | 0.26 |  |
| gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 8.64 | 0.25 |  |
| gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 8.64 | 0.21 |  |
| gpu: prefix + refloor + quadtree + raster + pathtrace | 8.65 | 0.25 |  |
| gpu: rows + sphere + quadtree + raster + pathtrace | 8.65 | 0.14 |  |
| gpu: prefix + incremental + quadtree + raster + pathtrace | 8.67 | 0.16 |  |
| gpu: prefix + sphere + quadtree + raster + pathtrace | 8.68 | 0.14 |  |
| gpu: prefix + incremental + quadtree + gbuffer + direct | 8.69 | 0.36 |  |
| gpu: points@cpu + splat + bricks + refloor + pathtrace | 8.69 | 0.24 |  |
| gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 8.70 | 0.15 |  |
| gpu: boxes@cpu + rayquery + raycast + pathtrace | 8.70 | 0.41 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + rowrun + direct | 8.73 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + refloor + direct | 8.77 | 0.04 |  |
| gpu: points@cpu + splat + hier + sphere + direct | 8.79 | 0.26 |  |
| gpu: points@cpu + splat + bricks + sphere + direct | 8.79 | 0.25 |  |
| gpu: points@cpu + splat + hier + refloor + direct | 8.81 | 0.29 |  |
| gpu: points@cpu + splat + hier + rowrun + direct | 8.81 | 0.27 |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 8.86 | 0.01 |  |
| gpu: points@cpu + splat + bricks + rowrun + direct | 8.86 | 0.28 |  |
| gpu: points@cpu + splat + bricks + incremental + direct | 8.88 | 0.26 |  |
| gpu: points@cpu + splat + hier + incremental + direct | 8.89 | 0.25 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + incremental + direct | 8.89 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + sphere + direct | 8.89 | 0.04 |  |
| gpu: points@cpu + splat + rows + sphere + direct | 8.90 | 0.25 |  |
| gpu: points@cpu + splat + rows + rowrun + direct | 8.91 | 0.28 |  |
| gpu: points@cpu + splat + rows + refloor + direct | 8.91 | 0.29 |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 8.92 | 0.01 |  |
| gpu: points@cpu + splat + prefix + refloor + direct | 8.93 | 0.30 |  |
| gpu: points@cpu + splat + prefix + sphere + direct | 8.93 | 0.25 |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 8.95 | 0.01 |  |
| gpu: points@cpu + splat + prefix + rowrun + direct | 8.97 | 0.29 |  |
| gpu: points@cpu + splat + rows + incremental + direct | 8.99 | 0.25 |  |
| gpu: points@cpu + splat + prefix + incremental + direct | 9.02 | 0.25 |  |
| gpu: points@cpu + splat + hier + refloor + pathtrace | 9.03 | 0.22 |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 9.04 | 0.01 |  |
| gpu: points@cpu + splat + hier + sphere + pathtrace | 9.05 | 0.16 |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 9.06 | 0.01 |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 9.06 | 0.01 |  |
| gpu: points@cpu + splat + bricks + sphere + pathtrace | 9.06 | 0.15 |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 9.08 | 0.01 |  |
| gpu: points@cpu + splat + hier + rowrun + pathtrace | 9.09 | 0.16 |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 9.10 | 0.01 |  |
| gpu: points@cpu + splat + bricks + incremental + pathtrace | 9.11 | 0.16 |  |
| gpu: points@cpu + splat + hier + incremental + pathtrace | 9.12 | 0.17 |  |
| gpu: points@cpu + splat + bricks + rowrun + pathtrace | 9.12 | 0.17 |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 9.14 | 0.01 |  |
| gpu: points@cpu + splat + rows + refloor + pathtrace | 9.14 | 0.21 |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 9.14 | 0.01 |  |
| gpu: distance + sphere + quadtree + raster + direct | 9.15 | 0.38 |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 9.15 | 0.01 |  |
| gpu: distance + sphere + quadtree + visbuffer + pathtrace | 9.15 | 0.36 |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 9.16 | 0.01 |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 9.16 | 0.01 |  |
| gpu: distance + refloor + quadtree + visbuffer + direct | 9.16 | 0.46 |  |
| gpu: points@cpu + splat + rows + rowrun + pathtrace | 9.16 | 0.19 |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 9.16 | 0.01 |  |
| gpu: points@cpu + splat + rows + sphere + pathtrace | 9.17 | 0.14 |  |
| gpu: points@cpu + splat + prefix + refloor + pathtrace | 9.18 | 0.23 |  |
| gpu: distance + sphere + quadtree + visbuffer + direct | 9.18 | 0.48 |  |
| gpu: distance + refloor + quadtree + raster + direct | 9.18 | 0.37 |  |
| gpu: distance + refloor + quadtree + visbuffer + pathtrace | 9.18 | 0.34 |  |
| gpu: points@cpu + splat + prefix + sphere + pathtrace | 9.19 | 0.15 |  |
| gpu: distance + refloor + quadtree + gbuffer + direct | 9.20 | 0.46 |  |
| gpu: distance + sphere + quadtree + gbuffer + pathtrace | 9.20 | 0.36 |  |
| gpu: distance + sphere + quadtree + gbuffer + direct | 9.20 | 0.48 |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 9.20 | 0.01 |  |
| gpu: points@cpu + splat + rows + incremental + pathtrace | 9.20 | 0.16 |  |
| gpu: distance + rowrun + quadtree + visbuffer + direct | 9.21 | 0.43 |  |
| gpu: distance + refloor + quadtree + gbuffer + pathtrace | 9.21 | 0.33 |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 9.21 | 0.01 |  |
| gpu: distance + rowrun + quadtree + raster + pathtrace | 9.22 | 0.18 |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 9.22 | 0.01 |  |
| gpu: distance + rowrun + quadtree + gbuffer + direct | 9.23 | 0.43 |  |
| gpu: distance + rowrun + quadtree + raster + direct | 9.24 | 0.29 |  |
| gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 9.24 | 0.30 |  |
| gpu: points@cpu + splat + prefix + rowrun + pathtrace | 9.26 | 0.20 |  |
| gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 9.27 | 0.29 |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 9.27 | 0.01 |  |
| gpu: distance + incremental + quadtree + raster + direct | 9.27 | 0.25 |  |
| gpu: points@cpu + splat + prefix + incremental + pathtrace | 9.28 | 0.16 |  |
| gpu: distance + incremental + quadtree + visbuffer + direct | 9.30 | 0.31 |  |
| gpu: distance + sphere + quadtree + raster + pathtrace | 9.33 | 0.26 |  |
| gpu: distance + incremental + quadtree + visbuffer + pathtrace | 9.34 | 0.22 |  |
| gpu: distance + incremental + quadtree + gbuffer + direct | 9.34 | 0.31 |  |
| gpu: distance + refloor + quadtree + raster + pathtrace | 9.38 | 0.24 |  |
| gpu: distance + incremental + quadtree + gbuffer + pathtrace | 9.40 | 0.22 |  |
| gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 9.42 | 0.01 |  |
| gpu: distance + incremental + quadtree + raster + pathtrace | 9.42 | 0.15 |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 9.43 | 0.01 |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 9.53 | 0.01 |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 9.54 | 0.01 |  |
| gpu: points@cpu + splat + clipmap + sphere + direct | 9.54 | 0.24 |  |
| gpu: points@cpu + splat + clipmap + refloor + direct | 9.54 | 0.28 |  |
| gpu: points@cpu + splat + clipmap + rowrun + direct | 9.56 | 0.28 |  |
| gpu: points@cpu + splat + clipmap + incremental + direct | 9.56 | 0.27 |  |
| gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 9.59 | 0.01 |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 9.61 | 0.01 |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 9.63 | 0.01 |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 9.64 | 0.01 |  |
| gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 9.65 | 0.01 |  |
| gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 9.65 | 0.01 |  |
| gpu: points@cpu + splat + distance + sphere + direct | 9.68 | 0.29 |  |
| gpu: points@cpu + splat + distance + rowrun + direct | 9.68 | 0.28 |  |
| gpu: points@cpu + splat + distance + refloor + direct | 9.68 | 0.28 |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 9.74 | 0.01 |  |
| gpu: points@cpu + splat + distance + incremental + direct | 9.75 | 0.23 |  |
| gpu: points@cpu + splat + clipmap + refloor + pathtrace | 9.79 | 0.19 |  |
| gpu: points@cpu + splat + clipmap + sphere + pathtrace | 9.82 | 0.13 |  |
| gpu: points@cpu + splat + clipmap + rowrun + pathtrace | 9.83 | 0.17 |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 9.83 | 0.01 |  |
| gpu: points@cpu + splat + clipmap + incremental + pathtrace | 9.84 | 0.16 |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 9.84 | 0.01 |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 9.87 | 0.01 |  |
| gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 9.89 | 0.01 |  |
| gpu: points@cpu + splat + distance + sphere + pathtrace | 9.91 | 0.23 |  |
| gpu: points@cpu + splat + distance + refloor + pathtrace | 9.91 | 0.22 |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 9.92 | 0.01 |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 9.95 | 0.01 |  |
| gpu: points@cpu + splat + distance + rowrun + pathtrace | 9.95 | 0.19 |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 9.98 | 0.01 |  |
| gpu: points@cpu + splat + distance + incremental + pathtrace | 9.99 | 0.15 |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 9.99 | 0.01 |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.07 | 0.01 |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.08 | 0.01 |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.08 | 0.01 |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.09 | 0.01 |  |
| gpu: greedy@cpu + rayquery + raycast + direct | 10.19 | 0.31 |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.20 | 0.01 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 10.24 | 0.04 |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.29 | 0.04 |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 10.31 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.31 | 0.01 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.34 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + pathtrace | 10.35 | 0.28 |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 10.35 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.36 | 0.01 |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.37 | 0.01 |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 10.38 | 0.04 |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.38 | 0.04 |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.40 | 0.01 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.42 | 0.04 |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.45 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.46 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + rowrun + direct | 10.46 | 0.04 |  |
| gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.49 | 0.01 |  |
| gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.49 | 0.01 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + sphere + direct | 10.50 | 0.04 |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.50 | 0.03 |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.51 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + refloor + direct | 10.51 | 0.04 |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.52 | 0.03 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + sphere + direct | 10.55 | 0.04 |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.55 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.55 | 0.04 |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.56 | 0.03 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.57 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + incremental + direct | 10.57 | 0.04 |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.58 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + refloor + direct | 10.59 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + sphere + direct | 10.62 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + rowrun + direct | 10.64 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + rowrun + direct | 10.64 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + refloor + direct | 10.66 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + refloor + direct | 10.67 | 0.04 |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.67 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + incremental + direct | 10.68 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + incremental + pathtrace | 10.71 | 0.19 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + incremental + direct | 10.72 | 0.04 |  |
| gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.73 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + rowrun + direct | 10.73 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + rowrun + pathtrace | 10.74 | 0.12 |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.75 | 0.04 |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 10.76 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + sphere + direct | 10.77 | 0.04 |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.77 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + sphere + pathtrace | 10.77 | 0.15 |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + refloor + pathtrace | 10.77 | 0.22 |  |
| gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.78 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + incremental + pathtrace | 10.78 | 0.15 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + incremental + direct | 10.80 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + rowrun + pathtrace | 10.81 | 0.15 |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 10.81 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + svo_dag + refloor + pathtrace | 10.81 | 0.21 |  |
| gpu: greedy@cpu + rayquery + raycast + tree64 + sphere + pathtrace | 10.84 | 0.14 |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 10.87 | 0.04 |  |
| gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 10.87 | 0.04 |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 10.89 | 0.04 |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 10.90 | 0.04 |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 11.08 | 0.04 |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 11.09 | 0.04 |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 11.18 | 0.04 |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 11.20 | 0.04 |  |
| gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 11.21 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + refloor + direct | 11.24 | 0.04 |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 11.25 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + rowrun + direct | 11.26 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 11.30 | 0.03 |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 11.31 | 0.04 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + incremental + direct | 11.32 | 0.04 |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 11.33 | 0.03 |  |
| gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + sphere + direct | 11.34 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 11.35 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 11.35 | 0.03 |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 11.40 | 0.03 |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 11.41 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 11.47 | 0.03 |  |
| gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 11.48 | 0.04 |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 11.62 | 0.04 |  |
| gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 11.67 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 11.68 | 0.04 |  |
| gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 11.69 | 0.04 |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + refloor + pathtrace | 12.10 | 0.21 |  |
| gpu: greedy@cpu + rayquery + raycast + hier + incremental + pathtrace | 12.16 | 0.19 |  |
| gpu: greedy@cpu + rayquery + raycast + hier + sphere + pathtrace | 12.19 | 0.16 |  |
| gpu: greedy@cpu + rayquery + raycast + hier + refloor + pathtrace | 12.21 | 0.22 |  |
| gpu: greedy@cpu + rayquery + raycast + hier + rowrun + pathtrace | 12.25 | 0.17 |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + incremental + pathtrace | 12.26 | 0.19 |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + sphere + pathtrace | 12.26 | 0.16 |  |
| gpu: greedy@cpu + rayquery + raycast + bricks + rowrun + pathtrace | 12.27 | 0.18 |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + refloor + pathtrace | 12.32 | 0.23 |  |
| gpu: greedy@cpu + rayquery + raycast + rows + sphere + pathtrace | 12.35 | 0.15 |  |
| gpu: greedy@cpu + rayquery + raycast + rows + refloor + pathtrace | 12.35 | 0.21 |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + sphere + pathtrace | 12.36 | 0.15 |  |
| gpu: greedy@cpu + rayquery + raycast + rows + rowrun + pathtrace | 12.37 | 0.20 |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + incremental + pathtrace | 12.39 | 0.18 |  |
| gpu: greedy@cpu + rayquery + raycast + prefix + rowrun + pathtrace | 12.40 | 0.21 |  |
| gpu: greedy@cpu + rayquery + raycast + rows + incremental + pathtrace | 12.41 | 0.19 |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + sphere + pathtrace | 12.66 | 0.14 |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + rowrun + pathtrace | 12.69 | 0.19 |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + refloor + pathtrace | 12.70 | 0.21 |  |
| gpu: greedy@cpu + rayquery + raycast + clipmap + incremental + pathtrace | 12.74 | 0.18 |  |
| gpu: naive@cpu + rayquery + raycast + pathtrace | 12.99 | 0.35 |  |
| gpu: greedy@cpu + rayquery + raycast + distance + incremental + pathtrace | 13.02 | 0.17 |  |
| gpu: greedy@cpu + rayquery + raycast + distance + refloor + pathtrace | 13.04 | 0.21 |  |
| gpu: greedy@cpu + rayquery + raycast + distance + sphere + pathtrace | 13.07 | 0.23 |  |
| gpu: greedy@cpu + rayquery + raycast + distance + rowrun + pathtrace | 13.11 | 0.20 |  |
| gpu: naive@cpu + rayquery + raycast + direct | 13.22 | 0.43 |  |

## Components

The effect of a node's component, among pipelines whose graphs differ in that node's component alone (knobs equal): the geometric mean of fps relative to the best of each such group, wins, and the number of comparisons. A node is named by its executor, its role and the inputs leading to it from the output.

**cpu · geometry (image > hits > faces > faces)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| greedy | 1.00 | 39 | 51 |
| naive | 0.72 | 12 | 51 |

**cpu · storage (image > hits > primary > storage; image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| hier | 1.00 | 10 | 12 |
| prefix | 0.92 | 2 | 12 |
| bricks | 0.87 | 0 | 12 |
| bits | 0.87 | 0 | 12 |
| u8 | 0.85 | 0 | 12 |
| brickmajor | 0.85 | 0 | 12 |
| morton | 0.85 | 0 | 12 |
| palette | 0.83 | 0 | 12 |

**cpu · storage (image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| hier | 1.00 | 11 | 12 |
| prefix | 0.99 | 1 | 12 |
| bricks | 0.97 | 0 | 12 |
| u8 | 0.96 | 0 | 12 |
| brickmajor | 0.96 | 0 | 12 |
| morton | 0.95 | 0 | 12 |
| bits | 0.95 | 0 | 12 |
| palette | 0.94 | 0 | 12 |

**cpu · traversal (image > hits > primary; image > shadow)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 1.00 | 43 | 48 |
| incremental | 0.94 | 5 | 48 |

**cpu · traversal (image > shadow)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 1.00 | 21 | 48 |
| incremental | 0.99 | 27 | 48 |

**gpu · draw (image > hits)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| visbuffer | 0.99 | 509 | 1026 |
| gbuffer | 0.99 | 368 | 1026 |
| raster | 0.65 | 149 | 1026 |

**gpu · geometry (image > hits > faces > faces)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| naive | 0.97 | 487 | 585 |
| greedy | 0.90 | 96 | 585 |
| cubes | 0.57 | 2 | 585 |

**gpu · geometry (image > hits > primary > geometry; image > shadow > geometry)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| boxes | 0.91 | 2 | 3 |
| naive | 0.68 | 1 | 2 |
| greedy | 0.52 | 0 | 3 |

**gpu · geometry (image > hits > primary > geometry; image > shadow > geometry; image > bounce > geometry)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| boxes | 0.94 | 5 | 6 |
| naive | 0.79 | 1 | 4 |
| greedy | 0.64 | 0 | 6 |

**gpu · storage (image > bounce > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.93 | 3 | 12 |
| prefix | 0.89 | 4 | 12 |
| rows | 0.87 | 1 | 12 |
| bricks | 0.85 | 1 | 12 |
| hier | 0.85 | 0 | 12 |
| clipmap | 0.83 | 0 | 12 |
| tree64 | 0.76 | 2 | 12 |
| svo_dag | 0.76 | 1 | 12 |

**gpu · storage (image > hits > faces > rays > storage; image > hits > faces > exposure)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| hier | 1.00 | 49 | 54 |
| bricks | 0.96 | 3 | 54 |
| u8 | 0.95 | 1 | 54 |
| bits | 0.94 | 0 | 54 |
| prefix | 0.94 | 0 | 54 |
| palette | 0.94 | 1 | 54 |
| morton | 0.94 | 0 | 54 |
| brickmajor | 0.90 | 0 | 54 |

**gpu · storage (image > hits > faces > rays > storage; image > hits > faces > exposure; image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.88 | 8 | 36 |
| prefix | 0.86 | 8 | 36 |
| bricks | 0.85 | 11 | 36 |
| rows | 0.85 | 5 | 36 |
| hier | 0.83 | 0 | 36 |
| clipmap | 0.76 | 3 | 36 |
| tree64 | 0.65 | 1 | 36 |
| svo_dag | 0.61 | 0 | 36 |

**gpu · storage (image > hits > faces > rays > storage; image > hits > faces > exposure; image > shadow > storage; image > bounce > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.82 | 17 | 72 |
| prefix | 0.77 | 9 | 72 |
| rows | 0.76 | 9 | 72 |
| clipmap | 0.75 | 12 | 72 |
| bricks | 0.73 | 18 | 72 |
| hier | 0.73 | 2 | 72 |
| tree64 | 0.66 | 5 | 72 |
| svo_dag | 0.62 | 0 | 72 |

**gpu · storage (image > hits > meshes > shadow > storage; image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| prefix | 0.90 | 16 | 108 |
| bricks | 0.89 | 37 | 108 |
| distance | 0.89 | 15 | 108 |
| rows | 0.88 | 8 | 108 |
| hier | 0.88 | 4 | 108 |
| clipmap | 0.87 | 6 | 108 |
| tree64 | 0.81 | 19 | 108 |
| svo_dag | 0.78 | 3 | 108 |

**gpu · storage (image > hits > meshes > shadow > storage; image > shadow > storage; image > bounce > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.91 | 33 | 108 |
| prefix | 0.81 | 30 | 108 |
| rows | 0.78 | 12 | 108 |
| hier | 0.76 | 3 | 108 |
| bricks | 0.74 | 3 | 108 |
| clipmap | 0.68 | 0 | 108 |
| tree64 | 0.64 | 23 | 108 |
| svo_dag | 0.61 | 4 | 108 |

**gpu · storage (image > hits > primary > storage; image > hits > meshes > shadow > storage; image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| bricks | 0.81 | 4 | 12 |
| distance | 0.81 | 4 | 12 |
| prefix | 0.79 | 1 | 12 |
| hier | 0.75 | 1 | 12 |
| rows | 0.73 | 0 | 12 |
| clipmap | 0.68 | 0 | 12 |
| tree64 | 0.63 | 2 | 12 |
| svo_dag | 0.62 | 0 | 12 |

**gpu · storage (image > hits > primary > storage; image > hits > meshes > shadow > storage; image > shadow > storage; image > bounce > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.84 | 7 | 24 |
| prefix | 0.79 | 6 | 24 |
| bricks | 0.77 | 7 | 24 |
| hier | 0.76 | 2 | 24 |
| rows | 0.74 | 0 | 24 |
| clipmap | 0.65 | 0 | 24 |
| tree64 | 0.60 | 2 | 24 |
| svo_dag | 0.59 | 0 | 24 |

**gpu · storage (image > hits > primary > storage; image > hits > start > storage; image > hits > meshes > shadow > storage; image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.82 | 3 | 12 |
| bricks | 0.82 | 5 | 12 |
| prefix | 0.81 | 1 | 12 |
| hier | 0.78 | 2 | 12 |
| rows | 0.75 | 0 | 12 |
| clipmap | 0.65 | 0 | 12 |
| tree64 | 0.61 | 1 | 12 |
| svo_dag | 0.59 | 0 | 12 |

**gpu · storage (image > hits > primary > storage; image > hits > start > storage; image > hits > meshes > shadow > storage; image > shadow > storage; image > bounce > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.85 | 7 | 24 |
| prefix | 0.80 | 6 | 24 |
| bricks | 0.79 | 8 | 24 |
| hier | 0.77 | 0 | 24 |
| rows | 0.75 | 0 | 24 |
| clipmap | 0.60 | 0 | 24 |
| tree64 | 0.58 | 3 | 24 |
| svo_dag | 0.56 | 0 | 24 |

**gpu · storage (image > shadow > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| prefix | 0.96 | 4 | 24 |
| clipmap | 0.96 | 3 | 24 |
| distance | 0.95 | 2 | 24 |
| hier | 0.95 | 1 | 24 |
| rows | 0.95 | 2 | 24 |
| bricks | 0.94 | 5 | 15 |
| tree64 | 0.93 | 4 | 24 |
| svo_dag | 0.93 | 3 | 24 |

**gpu · storage (image > shadow > storage; image > bounce > storage)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| distance | 0.92 | 4 | 12 |
| prefix | 0.86 | 4 | 12 |
| rows | 0.84 | 0 | 12 |
| hier | 0.83 | 0 | 12 |
| bricks | 0.83 | 0 | 12 |
| clipmap | 0.77 | 0 | 12 |
| tree64 | 0.77 | 4 | 12 |
| svo_dag | 0.74 | 0 | 12 |

**gpu · traversal (image > bounce)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 21 | 24 |
| incremental | 0.82 | 0 | 24 |
| rowrun | 0.81 | 0 | 24 |
| sphere | 0.73 | 3 | 24 |

**gpu · traversal (image > hits > faces > rays; image > shadow)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 61 | 72 |
| rowrun | 0.72 | 2 | 72 |
| incremental | 0.71 | 0 | 72 |
| sphere | 0.69 | 9 | 72 |

**gpu · traversal (image > hits > faces > rays; image > shadow; image > bounce)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 126 | 144 |
| incremental | 0.69 | 0 | 144 |
| rowrun | 0.68 | 0 | 144 |
| sphere | 0.66 | 18 | 144 |

**gpu · traversal (image > hits > meshes > shadow; image > shadow)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 1.00 | 185 | 216 |
| rowrun | 0.86 | 8 | 216 |
| incremental | 0.81 | 0 | 216 |
| sphere | 0.80 | 23 | 216 |

**gpu · traversal (image > hits > meshes > shadow; image > shadow; image > bounce)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 189 | 216 |
| rowrun | 0.65 | 0 | 216 |
| incremental | 0.64 | 0 | 216 |
| sphere | 0.59 | 27 | 216 |

**gpu · traversal (image > hits > primary; image > hits > meshes > shadow; image > shadow)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 42 | 48 |
| rowrun | 0.77 | 0 | 48 |
| incremental | 0.63 | 0 | 48 |
| sphere | 0.59 | 6 | 48 |

**gpu · traversal (image > hits > primary; image > hits > meshes > shadow; image > shadow; image > bounce)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 83 | 96 |
| rowrun | 0.70 | 1 | 96 |
| incremental | 0.64 | 0 | 96 |
| sphere | 0.56 | 12 | 96 |

**gpu · traversal (image > shadow)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 23 | 45 |
| rowrun | 0.96 | 11 | 45 |
| incremental | 0.93 | 7 | 45 |
| sphere | 0.92 | 4 | 45 |

**gpu · traversal (image > shadow; image > bounce)**

| component | geomean rel | wins | comparisons |
|---|---:|---:|---:|
| refloor | 0.99 | 21 | 24 |
| rowrun | 0.79 | 0 | 24 |
| incremental | 0.73 | 0 | 24 |
| sphere | 0.71 | 3 | 24 |

## Scenarios

### far [lit]

1920×1080. camera 4.78 blocks/frame (max 6.1), turn 0.52°/frame; 9141 chunks at start, 35.3 loads/frame; chunk fill 8%, solid 5.8%, surface 0.44%; sky 69%, hits at 465 blocks (median 455); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | rel | fps | frame p99 ms | gpu ms | cpu ms | psnr | bad pct | gpu mb | gpu device mb | peak rss mb | mem rel |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu: bricks + refloor + composite + raycast + direct ◆ | 1.00 | 375.0 | 12.83 | 1.14 | 0.08 | 56.7 | 0.01 | 1045 | 1863 | 907 | 5.28 |
| 2 | gpu: bricks + refloor + beam + composite + raycast + direct ◆ | 0.98 | 367.2 | 12.00 | 1.37 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 904 | 5.27 |
| 3 | gpu: tree64 + refloor + composite + raycast + direct ◆ | 0.97 | 362.5 | 13.34 | 1.26 | 1.12 | 56.7 | 0.01 | 191 | 438 | 1078 | 2.89 |
| 4 | gpu: prefix + refloor + composite + raycast + direct | 0.87 | 327.5 | 18.93 | 0.99 | 0.09 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 5 | gpu: bricks + rowrun + composite + raycast + direct | 0.87 | 326.3 | 13.11 | 1.66 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 903 | 5.27 |
| 6 | gpu: tree64 + refloor + beam + composite + raycast + direct ◆ | 0.84 | 313.5 | 12.94 | 1.90 | 1.13 | 56.7 | 0.01 | 191 | 424 | 1075 | 2.86 |
| 7 | gpu: bricks + rowrun + beam + composite + raycast + direct | 0.84 | 313.4 | 11.76 | 1.89 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 903 | 5.27 |
| 8 | gpu: prefix + refloor + beam + composite + raycast + direct | 0.83 | 311.3 | 18.49 | 1.23 | 0.08 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 9 | gpu: hier + refloor + composite + raycast + direct | 0.82 | 309.0 | 19.81 | 1.04 | 0.08 | 56.7 | 0.01 | 1045 | 1863 | 914 | 5.29 |
| 10 | gpu: hier + refloor + beam + composite + raycast + direct | 0.81 | 302.6 | 18.66 | 1.28 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 914 | 5.29 |
| 11 | gpu: clipmap + refloor + composite + raycast + direct | 0.80 | 298.9 | 14.03 | 1.89 | 0.02 | 56.7 | 0.01 | 1239 | 1611 | 1020 | 5.01 |
| 12 | gpu: prefix + rowrun + composite + raycast + direct | 0.80 | 298.9 | 20.06 | 1.31 | 0.10 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 13 | gpu: tree64 + incremental + composite + raycast + direct | 0.80 | 298.2 | 11.83 | 2.16 | 1.13 | 56.7 | 0.01 | 191 | 462 | 1073 | 2.92 |
| 14 | gpu: rows + refloor + composite + raycast + direct | 0.79 | 296.2 | 20.20 | 1.14 | 0.08 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 15 | gpu: prefix + rowrun + beam + composite + raycast + direct | 0.77 | 287.5 | 16.71 | 1.54 | 0.09 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 16 | gpu: svo_dag + refloor + composite + raycast + direct ◆ | 0.75 | 282.8 | 24.33 | 1.17 | 2.06 | 56.7 | 0.01 | 45 | 145 | 1191 | 2.54 |
| 17 | gpu: rows + refloor + beam + composite + raycast + direct | 0.75 | 282.1 | 20.10 | 1.37 | 0.09 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 18 | gpu: rows + rowrun + composite + raycast + direct | 0.75 | 281.0 | 19.89 | 1.36 | 0.08 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 19 | gpu: bricks + sphere + composite + raycast + direct | 0.74 | 276.6 | 12.25 | 2.63 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 903 | 5.27 |
| 20 | gpu: clipmap + rowrun + composite + raycast + direct | 0.73 | 273.0 | 12.59 | 2.30 | 0.03 | 56.7 | 0.01 | 1239 | 1611 | 1020 | 5.01 |
| 21 | gpu: rows + rowrun + beam + composite + raycast + direct | 0.72 | 270.9 | 17.10 | 1.61 | 0.09 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 22 | gpu: bricks + sphere + beam + composite + raycast + direct | 0.71 | 267.8 | 11.67 | 2.89 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 904 | 5.27 |
| 23 | gpu: clipmap + incremental + composite + raycast + direct | 0.70 | 262.0 | 13.06 | 2.57 | 0.03 | 56.7 | 0.01 | 1239 | 1611 | 1020 | 5.01 |
| 24 | gpu: bricks + refloor + quadtree + visbuffer + direct | 0.70 | 261.5 | 9.16 | 2.80 | 0.17 | 48.0 | 0.02 | 1235 | 2086 | 926 | 5.74 |
| 25 | gpu: bricks + incremental + composite + raycast + direct | 0.70 | 260.8 | 12.45 | 2.87 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 903 | 5.27 |
| 26 | gpu: tree64 + incremental + beam + composite + raycast + direct | 0.69 | 260.2 | 10.62 | 2.87 | 1.14 | 56.7 | 0.01 | 191 | 555 | 1073 | 3.10 |
| 27 | gpu: bricks + refloor + quadtree + gbuffer + direct | 0.69 | 259.7 | 12.05 | 2.81 | 0.16 | 48.0 | 0.02 | 1243 | 2095 | 923 | 5.75 |
| 28 | gpu: clipmap + refloor + beam + composite + raycast + direct | 0.69 | 258.0 | 10.35 | 2.74 | 0.03 | 56.7 | 0.01 | 1239 | 1611 | 1020 | 5.01 |
| 29 | gpu: hier + rowrun + composite + raycast + direct | 0.68 | 253.4 | 16.79 | 2.16 | 0.08 | 56.7 | 0.01 | 1045 | 1863 | 914 | 5.29 |
| 30 | gpu: svo_dag + refloor + beam + composite + raycast + direct | 0.67 | 249.8 | 23.70 | 1.80 | 2.10 | 56.7 | 0.01 | 45 | 145 | 1192 | 2.55 |
| 31 | gpu: bricks + incremental + beam + composite + raycast + direct | 0.65 | 243.8 | 10.35 | 3.32 | 0.10 | 56.7 | 0.01 | 1045 | 1863 | 904 | 5.27 |
| 32 | gpu: hier + rowrun + beam + composite + raycast + direct | 0.65 | 242.6 | 15.88 | 2.42 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 913 | 5.29 |
| 33 | gpu: distance + sphere + composite + raycast + direct | 0.65 | 242.4 | 30.82 | 0.81 | 0.08 | 56.7 | 0.01 | 1557 | 2231 | 907 | 5.98 |
| 34 | gpu: hier + sphere + composite + raycast + direct | 0.64 | 241.7 | 16.40 | 2.39 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 913 | 5.29 |
| 35 | gpu: bricks + refloor + composite + raycast + pathtrace | 0.64 | 239.5 | 14.00 | 3.25 | 0.10 | 32.2 | 7.12 | 1076 | 1894 | 904 | 5.33 |
| 36 | gpu: distance + refloor + composite + raycast + direct | 0.64 | 238.7 | 30.54 | 0.95 | 0.09 | 56.7 | 0.01 | 1557 | 2231 | 908 | 5.98 |
| 37 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + direct | 0.63 | 236.8 | 10.13 | 3.26 | 0.18 | 48.4 | 0.02 | 1235 | 2086 | 931 | 5.75 |
| 38 | gpu: tree64 + refloor + quadtree + gbuffer + direct | 0.63 | 236.4 | 11.31 | 3.22 | 1.23 | 48.0 | 0.02 | 389 | 665 | 1118 | 3.39 |
| 39 | gpu: clipmap + rowrun + beam + composite + raycast + direct | 0.63 | 236.3 | 12.62 | 3.15 | 0.04 | 56.7 | 0.01 | 1239 | 1611 | 1016 | 5.00 |
| 40 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + direct | 0.63 | 235.4 | 10.54 | 3.28 | 0.18 | 48.4 | 0.02 | 1243 | 2095 | 934 | 5.77 |
| 41 | gpu: prefix + sphere + composite + raycast + direct | 0.63 | 235.2 | 15.36 | 2.64 | 0.09 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 42 | gpu: distance + sphere + beam + composite + raycast + direct | 0.62 | 232.9 | 30.51 | 1.06 | 0.09 | 56.7 | 0.01 | 1557 | 2231 | 907 | 5.98 |
| 43 | gpu: hier + sphere + beam + composite + raycast + direct | 0.62 | 231.2 | 15.93 | 2.62 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 914 | 5.29 |
| 44 | gpu: tree64 + refloor + quadtree + visbuffer + direct | 0.62 | 231.1 | 11.26 | 3.32 | 1.23 | 48.0 | 0.02 | 381 | 669 | 1135 | 3.43 |
| 45 | gpu: bricks + refloor + beam + composite + raycast + pathtrace | 0.61 | 230.1 | 11.48 | 3.56 | 0.10 | 32.2 | 7.12 | 1076 | 1894 | 904 | 5.33 |
| 46 | gpu: hier + refloor + quadtree + visbuffer + direct | 0.61 | 229.9 | 15.15 | 2.71 | 0.16 | 48.0 | 0.02 | 1235 | 2086 | 932 | 5.75 |
| 47 | gpu: prefix + refloor + quadtree + gbuffer + direct | 0.61 | 229.6 | 13.56 | 2.67 | 0.17 | 48.0 | 0.02 | 1323 | 2139 | 929 | 5.84 |
| 48 | gpu: rows + sphere + composite + raycast + direct | 0.61 | 228.0 | 16.73 | 2.63 | 0.09 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 49 | gpu: prefix + refloor + composite + raycast + pathtrace | 0.61 | 227.4 | 15.49 | 2.90 | 0.09 | 32.2 | 7.12 | 1157 | 1939 | 908 | 5.42 |
| 50 | gpu: tree64 + refloor + composite + raycast + pathtrace | 0.61 | 227.0 | 9.81 | 3.71 | 1.14 | 32.2 | 7.12 | 223 | 482 | 1073 | 2.96 |
| 51 | gpu: clipmap + incremental + beam + composite + raycast + direct | 0.60 | 226.7 | 12.89 | 3.46 | 0.03 | 56.7 | 0.01 | 1239 | 1611 | 1021 | 5.01 |
| 52 | gpu: prefix + refloor + quadtree + visbuffer + direct | 0.60 | 226.4 | 15.19 | 2.79 | 0.16 | 48.0 | 0.02 | 1315 | 2130 | 929 | 5.83 |
| 53 | gpu: hier + refloor + composite + raycast + pathtrace | 0.60 | 225.3 | 15.33 | 2.90 | 0.09 | 32.2 | 7.12 | 1077 | 1894 | 914 | 5.35 |
| 54 | gpu: distance + refloor + beam + composite + raycast + direct | 0.60 | 223.4 | 31.64 | 1.21 | 0.09 | 56.7 | 0.01 | 1557 | 2231 | 908 | 5.98 |
| 55 | gpu: hier + refloor + quadtree + gbuffer + direct | 0.60 | 223.3 | 14.79 | 2.71 | 0.16 | 48.0 | 0.02 | 1243 | 2095 | 931 | 5.76 |
| 56 | gpu: distance + rowrun + composite + raycast + direct | 0.59 | 222.9 | 30.95 | 1.29 | 0.09 | 56.7 | 0.01 | 1557 | 2231 | 908 | 5.98 |
| 57 | gpu: tree64 + sphere + composite + raycast + direct | 0.59 | 222.6 | 9.26 | 3.80 | 1.15 | 56.7 | 0.01 | 191 | 453 | 1072 | 2.90 |
| 58 | gpu: prefix + sphere + beam + composite + raycast + direct | 0.59 | 221.5 | 15.46 | 2.91 | 0.10 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 59 | gpu: clipmap + refloor + quadtree + gbuffer + direct | 0.58 | 219.2 | 12.20 | 3.55 | 0.11 | 48.0 | 0.02 | 1437 | 1846 | 1043 | 5.50 |
| 60 | gpu: hier + incremental + composite + raycast + direct | 0.58 | 217.9 | 16.36 | 3.10 | 0.09 | 56.7 | 0.01 | 1045 | 1863 | 913 | 5.29 |
| 61 | gpu: clipmap + refloor + quadtree + visbuffer + direct | 0.58 | 217.8 | 11.99 | 3.53 | 0.11 | 48.0 | 0.02 | 1429 | 1837 | 1043 | 5.49 |
| 62 | gpu: bricks + rowrun + quadtree + gbuffer + direct | 0.58 | 217.0 | 9.96 | 3.94 | 0.18 | 48.0 | 0.02 | 1243 | 2095 | 935 | 5.77 |
| 63 | gpu: rows + sphere + beam + composite + raycast + direct | 0.58 | 217.0 | 16.35 | 2.85 | 0.09 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 64 | gpu: prefix + refloor + beam + composite + raycast + pathtrace | 0.58 | 216.6 | 16.47 | 3.09 | 0.10 | 32.2 | 7.12 | 1157 | 1939 | 908 | 5.42 |
| 65 | gpu: rows + refloor + quadtree + gbuffer + direct | 0.58 | 216.5 | 14.48 | 2.80 | 0.16 | 48.0 | 0.02 | 1307 | 2127 | 929 | 5.82 |
| 66 | gpu: prefix + incremental + composite + raycast + direct | 0.57 | 214.2 | 15.77 | 3.16 | 0.09 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 67 | gpu: hier + refloor + beam + composite + raycast + pathtrace | 0.57 | 213.8 | 15.29 | 3.16 | 0.10 | 32.2 | 7.12 | 1077 | 1894 | 914 | 5.35 |
| 68 | gpu: bricks + rowrun + quadtree + visbuffer + direct | 0.57 | 212.6 | 10.93 | 3.93 | 0.18 | 48.0 | 0.02 | 1235 | 2086 | 935 | 5.75 |
| 69 | gpu: distance + rowrun + beam + composite + raycast + direct | 0.57 | 212.3 | 30.29 | 1.54 | 0.09 | 56.7 | 0.01 | 1557 | 2231 | 907 | 5.98 |
| 70 | gpu: clipmap + sphere + composite + raycast + direct | 0.56 | 209.7 | 13.26 | 3.76 | 0.03 | 56.7 | 0.01 | 1239 | 1611 | 1020 | 5.01 |
| 71 | gpu: svo_dag + sphere + composite + raycast + direct | 0.56 | 208.9 | 22.50 | 3.00 | 2.09 | 56.7 | 0.01 | 45 | 145 | 1191 | 2.54 |
| 72 | gpu: prefix + rowrun + quadtree + visbuffer + direct | 0.56 | 208.5 | 15.48 | 3.22 | 0.16 | 48.0 | 0.02 | 1315 | 2130 | 930 | 5.83 |
| 73 | gpu: rows + refloor + quadtree + visbuffer + direct | 0.56 | 208.5 | 15.63 | 2.89 | 0.17 | 48.0 | 0.02 | 1299 | 2118 | 929 | 5.80 |
| 74 | gpu: prefix + rowrun + quadtree + gbuffer + direct | 0.55 | 208.1 | 14.29 | 3.30 | 0.17 | 48.0 | 0.02 | 1323 | 2139 | 932 | 5.85 |
| 75 | gpu: rows + rowrun + quadtree + gbuffer + direct | 0.55 | 207.5 | 16.40 | 3.17 | 0.16 | 48.0 | 0.02 | 1307 | 2127 | 930 | 5.82 |
| 76 | gpu: boxes@cpu + rayquery + raycast + direct | 0.55 | 207.2 | 22.53 | 1.45 | 1.79 | 56.6 | 0.01 | 1138 | 1833 | 1608 | 6.55 |
| 77 | gpu: bricks + sphere + quadtree + visbuffer + direct | 0.55 | 205.6 | 9.85 | 4.35 | 0.17 | 48.0 | 0.02 | 1235 | 2086 | 935 | 5.75 |
| 78 | gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 0.55 | 205.2 | 9.25 | 4.29 | 1.16 | 32.2 | 7.12 | 223 | 466 | 1075 | 2.93 |
| 79 | gpu: rows + refloor + composite + raycast + pathtrace | 0.55 | 204.8 | 16.64 | 3.23 | 0.09 | 32.2 | 7.12 | 1140 | 1926 | 908 | 5.40 |
| 80 | gpu: hier + incremental + beam + composite + raycast + direct | 0.55 | 204.4 | 16.26 | 3.34 | 0.10 | 56.7 | 0.01 | 1045 | 1863 | 913 | 5.29 |
| 81 | gpu: bricks + sphere + quadtree + gbuffer + direct | 0.54 | 203.9 | 10.07 | 4.34 | 0.18 | 48.0 | 0.02 | 1243 | 2095 | 934 | 5.77 |
| 82 | gpu: rows + rowrun + quadtree + visbuffer + direct | 0.54 | 201.2 | 16.66 | 3.17 | 0.17 | 48.0 | 0.02 | 1299 | 2118 | 929 | 5.80 |
| 83 | gpu: clipmap + rowrun + quadtree + visbuffer + direct | 0.54 | 200.7 | 11.67 | 4.24 | 0.11 | 48.0 | 0.02 | 1429 | 1837 | 1048 | 5.49 |
| 84 | gpu: svo_dag + rowrun + composite + raycast + direct ◆ | 0.54 | 200.7 | 21.93 | 3.28 | 2.09 | 56.7 | 0.01 | 45 | 145 | 1191 | 2.54 |
| 85 | gpu: rows + refloor + beam + composite + raycast + pathtrace | 0.53 | 200.3 | 16.84 | 3.58 | 0.09 | 32.2 | 7.12 | 1140 | 1926 | 908 | 5.40 |
| 86 | gpu: prefix + incremental + beam + composite + raycast + direct | 0.53 | 200.2 | 15.67 | 3.58 | 0.10 | 56.7 | 0.01 | 1125 | 1907 | 907 | 5.36 |
| 87 | gpu: tree64 + rowrun + composite + raycast + direct | 0.53 | 199.8 | 8.92 | 4.41 | 1.14 | 56.7 | 0.01 | 191 | 568 | 1077 | 3.13 |
| 88 | gpu: tree64 + sphere + beam + composite + raycast + direct | 0.53 | 199.7 | 9.42 | 4.44 | 1.15 | 56.7 | 0.01 | 191 | 456 | 1076 | 2.92 |
| 89 | gpu: rows + incremental + composite + raycast + direct | 0.53 | 198.5 | 17.72 | 3.26 | 0.10 | 56.7 | 0.01 | 1109 | 1895 | 907 | 5.34 |
| 90 | gpu: clipmap + rowrun + quadtree + gbuffer + direct | 0.53 | 198.2 | 11.93 | 4.23 | 0.11 | 48.0 | 0.02 | 1437 | 1846 | 1048 | 5.51 |
| 91 | gpu: distance + sphere + composite + raycast + pathtrace | 0.52 | 195.0 | 23.46 | 2.35 | 0.09 | 32.2 | 7.12 | 1588 | 2262 | 908 | 6.04 |
| 92 | gpu: clipmap + incremental + quadtree + visbuffer + direct | 0.52 | 193.3 | 11.37 | 4.53 | 0.11 | 48.0 | 0.02 | 1429 | 1837 | 1049 | 5.50 |
| 93 | gpu: clipmap + sphere + beam + composite + raycast + direct | 0.51 | 191.7 | 11.68 | 4.64 | 0.04 | 56.7 | 0.01 | 1239 | 1611 | 1025 | 5.02 |
| 94 | gpu: svo_dag + refloor + quadtree + gbuffer + direct | 0.51 | 191.0 | 22.41 | 3.42 | 2.16 | 48.0 | 0.02 | 243 | 378 | 1224 | 3.05 |
| 95 | gpu: svo_dag + refloor + quadtree + visbuffer + direct | 0.51 | 190.6 | 23.11 | 3.42 | 2.17 | 48.0 | 0.02 | 235 | 370 | 1227 | 3.04 |
| 96 | gpu: tree64 + incremental + quadtree + gbuffer + direct | 0.51 | 190.5 | 9.30 | 4.64 | 1.24 | 48.0 | 0.02 | 389 | 680 | 1131 | 3.45 |
| 97 | gpu: svo_dag + sphere + beam + composite + raycast + direct | 0.51 | 190.3 | 22.48 | 3.62 | 2.12 | 56.7 | 0.01 | 45 | 145 | 1191 | 2.54 |
| 98 | gpu: clipmap + incremental + quadtree + gbuffer + direct | 0.51 | 190.0 | 11.54 | 4.53 | 0.12 | 48.0 | 0.02 | 1437 | 1846 | 1048 | 5.51 |
| 99 | gpu: tree64 + incremental + quadtree + visbuffer + direct | 0.51 | 189.7 | 9.45 | 4.64 | 1.24 | 48.0 | 0.02 | 381 | 652 | 1131 | 3.40 |
| 100 | gpu: svo_dag + refloor + composite + raycast + pathtrace | 0.51 | 189.6 | 21.91 | 3.68 | 2.12 | 32.2 | 7.12 | 77 | 176 | 1194 | 2.61 |
| 101 | gpu: rows + incremental + beam + composite + raycast + direct | 0.50 | 189.2 | 16.93 | 3.63 | 0.12 | 56.7 | 0.01 | 1109 | 1895 | 911 | 5.34 |
| 102 | gpu: distance + sphere + quadtree + gbuffer + direct | 0.50 | 189.1 | 20.71 | 2.48 | 0.17 | 48.0 | 0.02 | 1755 | 2463 | 929 | 6.46 |
| 103 | gpu: distance + sphere + beam + composite + raycast + pathtrace | 0.50 | 188.7 | 24.09 | 2.59 | 0.09 | 32.2 | 7.12 | 1588 | 2262 | 912 | 6.05 |
| 104 | gpu: distance + sphere + quadtree + visbuffer + direct | 0.49 | 185.5 | 21.38 | 2.47 | 0.17 | 48.0 | 0.02 | 1747 | 2454 | 929 | 6.44 |
| 105 | gpu: hier + sphere + quadtree + gbuffer + direct | 0.49 | 185.4 | 15.16 | 4.14 | 0.18 | 48.0 | 0.02 | 1243 | 2095 | 939 | 5.78 |
| 106 | gpu: bricks + refloor + quadtree + raster + direct | 0.49 | 185.2 | 10.83 | 5.00 | 0.17 | 48.0 | 0.02 | 1227 | 2078 | 923 | 5.71 |
| 107 | gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 0.49 | 185.0 | 11.72 | 4.91 | 0.18 | 32.1 | 7.13 | 1266 | 2118 | 922 | 5.79 |
| 108 | gpu: hier + rowrun + quadtree + visbuffer + direct | 0.49 | 184.5 | 14.17 | 4.19 | 0.17 | 48.0 | 0.02 | 1235 | 2086 | 935 | 5.75 |
| 109 | gpu: svo_dag + incremental + composite + raycast + direct | 0.49 | 184.2 | 21.65 | 3.95 | 2.09 | 56.7 | 0.01 | 45 | 145 | 1195 | 2.55 |
| 110 | gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 0.49 | 184.0 | 9.86 | 5.07 | 0.17 | 32.1 | 7.13 | 1274 | 2126 | 927 | 5.81 |
| 111 | gpu: distance + refloor + quadtree + visbuffer + direct | 0.49 | 183.4 | 23.84 | 2.71 | 0.17 | 48.0 | 0.02 | 1747 | 2454 | 930 | 6.44 |
| 112 | gpu: hier + sphere + quadtree + visbuffer + direct | 0.49 | 181.9 | 16.11 | 4.15 | 0.17 | 48.0 | 0.02 | 1235 | 2086 | 935 | 5.75 |
| 113 | gpu: bricks + incremental + quadtree + gbuffer + direct | 0.48 | 181.1 | 11.12 | 5.09 | 0.20 | 48.0 | 0.02 | 1243 | 2095 | 939 | 5.78 |
| 114 | gpu: tree64 + rowrun + beam + composite + raycast + direct | 0.48 | 180.9 | 9.08 | 5.06 | 1.16 | 56.7 | 0.01 | 191 | 433 | 1074 | 2.87 |
| 115 | gpu: distance + refloor + quadtree + gbuffer + direct | 0.48 | 180.8 | 23.08 | 2.71 | 0.17 | 48.0 | 0.02 | 1755 | 2463 | 929 | 6.46 |
| 116 | gpu: prefix + rowrun + composite + raycast + pathtrace | 0.48 | 180.7 | 14.43 | 4.31 | 0.11 | 32.2 | 7.12 | 1157 | 1939 | 912 | 5.43 |
| 117 | gpu: hier + rowrun + quadtree + gbuffer + direct | 0.48 | 180.0 | 15.31 | 4.19 | 0.17 | 48.0 | 0.02 | 1243 | 2095 | 935 | 5.77 |
| 118 | gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 0.48 | 179.4 | 13.86 | 4.54 | 0.10 | 32.2 | 7.12 | 1157 | 1939 | 912 | 5.43 |
| 119 | gpu: prefix + sphere + quadtree + gbuffer + direct | 0.48 | 179.3 | 13.83 | 4.48 | 0.18 | 48.0 | 0.02 | 1323 | 2139 | 931 | 5.85 |
| 120 | gpu: bricks + incremental + quadtree + visbuffer + direct | 0.48 | 179.1 | 13.87 | 5.15 | 0.17 | 48.0 | 0.02 | 1235 | 2086 | 935 | 5.75 |
| 121 | gpu: svo_dag + rowrun + beam + composite + raycast + direct | 0.48 | 178.7 | 21.82 | 4.10 | 2.12 | 56.7 | 0.01 | 45 | 145 | 1195 | 2.55 |
| 122 | gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.48 | 178.5 | 35.73 | 1.86 | 3.53 | 50.3 | 0.02 | 456 | 681 | 1127 | 3.44 |
| 123 | gpu: prefix + sphere + quadtree + visbuffer + direct | 0.47 | 178.1 | 13.46 | 4.49 | 0.18 | 48.0 | 0.02 | 1315 | 2130 | 959 | 5.88 |
| 124 | gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.47 | 178.0 | 34.72 | 1.85 | 3.53 | 50.3 | 0.02 | 464 | 679 | 1132 | 3.45 |
| 125 | gpu: distance + refloor + composite + raycast + pathtrace | 0.47 | 177.9 | 24.96 | 2.87 | 0.10 | 32.2 | 7.12 | 1588 | 2262 | 920 | 6.06 |
| 126 | gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 0.47 | 177.9 | 14.05 | 4.55 | 0.18 | 32.1 | 7.13 | 1346 | 2162 | 950 | 5.93 |
| 127 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.47 | 177.8 | 25.27 | 1.80 | 2.54 | 50.3 | 0.02 | 1318 | 1857 | 1133 | 5.69 |
| 128 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.47 | 177.1 | 24.91 | 1.81 | 2.55 | 50.3 | 0.02 | 1310 | 1849 | 1133 | 5.68 |
| 129 | gpu: hier + refloor + quadtree + visbuffer + pathtrace | 0.47 | 176.9 | 13.74 | 4.59 | 0.18 | 32.1 | 7.13 | 1267 | 2118 | 976 | 5.89 |
| 130 | gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.47 | 176.4 | 34.08 | 1.91 | 3.55 | 50.3 | 0.02 | 464 | 674 | 1124 | 3.43 |
| 131 | gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.47 | 175.9 | 24.53 | 1.88 | 2.55 | 50.3 | 0.02 | 1310 | 1849 | 1133 | 5.68 |
| 132 | gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.47 | 175.1 | 34.58 | 1.99 | 3.51 | 50.3 | 0.02 | 456 | 664 | 1136 | 3.43 |
| 133 | gpu: clipmap + refloor + composite + raycast + pathtrace | 0.47 | 174.5 | 12.13 | 5.35 | 0.03 | 32.2 | 7.12 | 1271 | 1643 | 1021 | 5.07 |
| 134 | gpu: rows + sphere + quadtree + visbuffer + direct | 0.47 | 174.4 | 14.79 | 4.41 | 0.18 | 48.0 | 0.02 | 1299 | 2118 | 972 | 5.88 |
| 135 | gpu: distance + refloor + beam + composite + raycast + pathtrace | 0.46 | 174.1 | 23.93 | 3.18 | 0.10 | 32.2 | 7.12 | 1588 | 2262 | 962 | 6.14 |
| 136 | gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 0.46 | 174.0 | 14.04 | 4.61 | 0.18 | 32.1 | 7.13 | 1354 | 2170 | 971 | 5.98 |
| 137 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 0.46 | 172.9 | 11.93 | 5.36 | 0.19 | 32.1 | 7.13 | 1266 | 2118 | 928 | 5.80 |
| 138 | gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.46 | 172.7 | 26.20 | 1.87 | 2.58 | 50.3 | 0.02 | 1318 | 1857 | 1134 | 5.70 |
| 139 | gpu: rows + sphere + quadtree + gbuffer + direct | 0.46 | 172.4 | 15.20 | 4.40 | 0.18 | 48.0 | 0.02 | 1307 | 2127 | 972 | 5.90 |
| 140 | gpu: distance + rowrun + quadtree + visbuffer + direct | 0.46 | 172.4 | 24.13 | 3.17 | 0.17 | 48.0 | 0.02 | 1747 | 2454 | 971 | 6.52 |
| 141 | gpu: prefix + refloor + quadtree + raster + direct | 0.46 | 172.1 | 15.07 | 4.69 | 0.17 | 48.0 | 0.02 | 1307 | 2122 | 975 | 5.90 |
| 142 | gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.46 | 172.0 | 33.87 | 2.14 | 3.54 | 50.3 | 0.02 | 464 | 775 | 1135 | 3.64 |
| 143 | gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.46 | 171.5 | 33.83 | 2.15 | 3.56 | 50.3 | 0.02 | 456 | 705 | 1133 | 3.50 |
| 144 | gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.46 | 171.5 | 24.46 | 1.99 | 2.46 | 50.3 | 0.02 | 1512 | 1759 | 1298 | 5.82 |
| 145 | gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.46 | 171.3 | 25.63 | 2.00 | 2.44 | 50.3 | 0.02 | 1505 | 1750 | 1298 | 5.80 |
| 146 | gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 0.46 | 171.0 | 21.40 | 4.50 | 2.09 | 32.2 | 7.12 | 77 | 176 | 1196 | 2.61 |
| 147 | gpu: hier + refloor + quadtree + gbuffer + pathtrace | 0.46 | 170.9 | 14.77 | 4.64 | 0.18 | 32.1 | 7.13 | 1274 | 2126 | 977 | 5.91 |
| 148 | gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.45 | 170.5 | 22.87 | 2.18 | 2.58 | 50.3 | 0.02 | 1310 | 1849 | 1134 | 5.68 |
| 149 | gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.45 | 170.0 | 24.38 | 2.06 | 2.44 | 50.3 | 0.02 | 1505 | 1750 | 1298 | 5.80 |
| 150 | gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.45 | 169.9 | 24.40 | 2.04 | 2.46 | 50.3 | 0.02 | 1512 | 1759 | 1299 | 5.82 |
| 151 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + direct | 0.45 | 169.7 | 10.67 | 5.49 | 0.17 | 48.5 | 0.02 | 1227 | 2078 | 928 | 5.72 |
| 152 | gpu: clipmap + sphere + quadtree + gbuffer + direct | 0.45 | 169.3 | 13.56 | 5.40 | 0.11 | 48.0 | 0.02 | 1437 | 1846 | 1052 | 5.52 |
| 153 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 0.45 | 169.1 | 10.87 | 5.52 | 0.19 | 32.1 | 7.13 | 1274 | 2126 | 924 | 5.81 |
| 154 | gpu: hier + refloor + quadtree + raster + direct | 0.45 | 169.0 | 14.66 | 4.81 | 0.17 | 48.0 | 0.02 | 1227 | 2078 | 977 | 5.82 |
| 155 | gpu: distance + rowrun + quadtree + gbuffer + direct | 0.45 | 169.0 | 23.72 | 3.17 | 0.17 | 48.0 | 0.02 | 1755 | 2463 | 971 | 6.54 |
| 156 | gpu: clipmap + sphere + quadtree + visbuffer + direct | 0.45 | 168.5 | 13.99 | 5.41 | 0.11 | 48.0 | 0.02 | 1429 | 1837 | 1048 | 5.49 |
| 157 | gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.45 | 167.4 | 24.30 | 2.16 | 2.61 | 50.3 | 0.02 | 1318 | 1857 | 1134 | 5.70 |
| 158 | gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.45 | 167.3 | 23.98 | 2.23 | 2.45 | 50.3 | 0.02 | 1505 | 1750 | 1302 | 5.81 |
| 159 | gpu: svo_dag + incremental + beam + composite + raycast + direct | 0.45 | 167.1 | 21.98 | 4.63 | 2.13 | 56.7 | 0.01 | 45 | 145 | 1195 | 2.55 |
| 160 | gpu: rows + rowrun + composite + raycast + pathtrace | 0.44 | 166.8 | 17.27 | 4.59 | 0.11 | 32.2 | 7.12 | 1140 | 1926 | 975 | 5.52 |
| 161 | gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.44 | 166.8 | 25.08 | 2.22 | 2.47 | 50.3 | 0.02 | 1512 | 1759 | 1298 | 5.82 |
| 162 | gpu: distance + incremental + composite + raycast + direct | 0.44 | 166.6 | 22.71 | 3.62 | 0.09 | 56.7 | 0.01 | 1557 | 2231 | 967 | 6.09 |
| 163 | gpu: rows + rowrun + beam + composite + raycast + pathtrace | 0.44 | 166.2 | 16.23 | 4.84 | 0.10 | 32.2 | 7.12 | 1140 | 1926 | 963 | 5.50 |
| 164 | gpu: prefix + incremental + quadtree + visbuffer + direct | 0.44 | 166.1 | 14.36 | 5.13 | 0.20 | 48.0 | 0.02 | 1315 | 2130 | 984 | 5.93 |
| 165 | gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 0.44 | 166.1 | 9.65 | 5.53 | 1.27 | 32.1 | 7.13 | 413 | 684 | 1127 | 3.45 |
| 166 | gpu: tree64 + refloor + quadtree + raster + direct | 0.44 | 165.9 | 10.27 | 5.55 | 1.24 | 48.0 | 0.02 | 373 | 638 | 1123 | 3.35 |
| 167 | gpu: hier + incremental + quadtree + visbuffer + direct | 0.44 | 165.8 | 13.70 | 5.02 | 0.18 | 48.0 | 0.02 | 1235 | 2086 | 982 | 5.84 |
| 168 | gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.44 | 165.6 | 23.33 | 2.50 | 3.57 | 50.3 | 0.02 | 464 | 899 | 1151 | 3.90 |
| 169 | gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.44 | 165.2 | 26.49 | 2.45 | 3.59 | 50.3 | 0.02 | 456 | 688 | 1142 | 3.49 |
| 170 | gpu: rows + refloor + quadtree + visbuffer + pathtrace | 0.44 | 165.0 | 15.41 | 4.89 | 0.17 | 32.1 | 7.13 | 1330 | 2150 | 970 | 5.94 |
| 171 | gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.44 | 164.2 | 28.42 | 1.77 | 2.51 | 50.3 | 0.02 | 1398 | 1901 | 1129 | 5.77 |
| 172 | gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 0.44 | 163.9 | 9.82 | 5.62 | 1.26 | 32.1 | 7.13 | 421 | 705 | 1127 | 3.49 |
| 173 | gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.44 | 163.8 | 23.11 | 2.44 | 2.48 | 50.3 | 0.02 | 1512 | 1759 | 1297 | 5.82 |
| 174 | gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.44 | 163.6 | 27.99 | 1.78 | 2.50 | 50.3 | 0.02 | 1390 | 1893 | 1134 | 5.76 |
| 175 | gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.44 | 163.5 | 23.68 | 2.45 | 2.46 | 50.3 | 0.02 | 1505 | 1750 | 1302 | 5.81 |
| 176 | gpu: hier + incremental + quadtree + gbuffer + direct | 0.44 | 163.3 | 14.82 | 5.08 | 0.19 | 48.0 | 0.02 | 1243 | 2095 | 982 | 5.86 |
| 177 | gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.43 | 163.0 | 28.17 | 1.83 | 2.50 | 50.3 | 0.02 | 1390 | 1893 | 1129 | 5.75 |
| 178 | gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.43 | 163.0 | 28.86 | 1.82 | 2.54 | 50.3 | 0.02 | 1398 | 1901 | 1129 | 5.77 |
| 179 | gpu: rows + refloor + quadtree + raster + direct | 0.43 | 162.4 | 14.91 | 5.02 | 0.18 | 48.0 | 0.02 | 1291 | 2110 | 976 | 5.88 |
| 180 | gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.43 | 161.7 | 29.83 | 1.80 | 2.48 | 50.3 | 0.02 | 1310 | 1849 | 1134 | 5.68 |
| 181 | gpu: prefix + incremental + quadtree + gbuffer + direct | 0.43 | 161.6 | 15.07 | 5.21 | 0.18 | 48.0 | 0.02 | 1323 | 2139 | 981 | 5.94 |
| 182 | gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.43 | 161.3 | 29.63 | 1.79 | 2.53 | 50.3 | 0.02 | 1318 | 1857 | 1134 | 5.70 |
| 183 | gpu: rows + refloor + quadtree + gbuffer + pathtrace | 0.43 | 161.1 | 15.56 | 5.01 | 0.19 | 32.1 | 7.13 | 1338 | 2158 | 977 | 5.97 |
| 184 | gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.43 | 160.7 | 28.27 | 1.86 | 2.50 | 50.3 | 0.02 | 1318 | 1857 | 1143 | 5.71 |
| 185 | gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.43 | 160.5 | 28.88 | 1.87 | 2.52 | 50.3 | 0.02 | 1310 | 1849 | 1134 | 5.68 |
| 186 | gpu: prefix + rowrun + quadtree + raster + direct | 0.43 | 160.0 | 16.63 | 5.41 | 0.17 | 48.0 | 0.02 | 1307 | 2122 | 981 | 5.91 |
| 187 | gpu: tree64 + sphere + quadtree + visbuffer + direct | 0.43 | 159.9 | 10.47 | 5.76 | 1.27 | 48.0 | 0.02 | 381 | 788 | 1120 | 3.63 |
| 188 | gpu: rows + incremental + quadtree + gbuffer + direct | 0.43 | 159.6 | 15.31 | 5.13 | 0.18 | 48.0 | 0.02 | 1307 | 2127 | 973 | 5.90 |
| 189 | gpu: tree64 + sphere + quadtree + gbuffer + direct | 0.42 | 158.7 | 10.56 | 5.81 | 1.27 | 48.0 | 0.02 | 389 | 674 | 1140 | 3.45 |
| 190 | gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.42 | 158.1 | 29.31 | 1.80 | 2.50 | 50.3 | 0.02 | 1382 | 1889 | 1127 | 5.75 |
| 191 | gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.42 | 157.3 | 30.00 | 1.85 | 2.50 | 50.3 | 0.02 | 1382 | 1889 | 1128 | 5.75 |
| 192 | gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 0.42 | 157.3 | 12.82 | 6.07 | 0.10 | 32.2 | 7.12 | 1076 | 1894 | 960 | 5.44 |
| 193 | gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.42 | 157.3 | 31.82 | 1.81 | 2.51 | 50.3 | 0.02 | 1374 | 1881 | 1127 | 5.73 |
| 194 | gpu: rows + incremental + quadtree + visbuffer + direct | 0.42 | 157.1 | 15.82 | 5.21 | 0.18 | 48.0 | 0.02 | 1299 | 2118 | 977 | 5.89 |
| 195 | gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.42 | 157.0 | 27.98 | 2.09 | 2.53 | 50.3 | 0.02 | 1318 | 1857 | 1136 | 5.70 |
| 196 | gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.42 | 156.7 | 28.28 | 2.20 | 2.55 | 50.3 | 0.02 | 1390 | 1893 | 1132 | 5.76 |
| 197 | gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.42 | 156.6 | 28.44 | 2.11 | 2.54 | 50.3 | 0.02 | 1310 | 1849 | 1134 | 5.68 |
| 198 | gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.42 | 156.4 | 28.00 | 2.21 | 2.54 | 50.3 | 0.02 | 1398 | 1901 | 1129 | 5.77 |
| 199 | gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.42 | 156.0 | 29.89 | 1.86 | 2.54 | 50.3 | 0.02 | 1374 | 1881 | 1127 | 5.73 |
| 200 | gpu: distance + incremental + beam + composite + raycast + direct | 0.41 | 155.1 | 23.41 | 4.03 | 0.10 | 56.7 | 0.01 | 1557 | 2231 | 964 | 6.08 |
| 201 | gpu: distance + sphere + quadtree + visbuffer + pathtrace | 0.41 | 154.1 | 21.83 | 4.00 | 0.18 | 32.1 | 7.13 | 1778 | 2486 | 973 | 6.59 |
| 202 | gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 0.41 | 154.1 | 11.20 | 6.20 | 0.04 | 32.2 | 7.12 | 1271 | 1643 | 1073 | 5.17 |
| 203 | gpu: bricks + rowrun + composite + raycast + pathtrace | 0.41 | 153.5 | 10.33 | 6.29 | 0.10 | 32.2 | 7.12 | 1076 | 1894 | 977 | 5.47 |
| 204 | gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.41 | 153.4 | 45.09 | 1.84 | 4.52 | 50.3 | 0.02 | 318 | 512 | 1294 | 3.44 |
| 205 | gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.41 | 153.3 | 45.51 | 1.85 | 4.53 | 50.3 | 0.02 | 311 | 503 | 1251 | 3.34 |
| 206 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.41 | 153.2 | 43.74 | 1.89 | 4.50 | 50.3 | 0.02 | 311 | 503 | 1274 | 3.38 |
| 207 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.41 | 152.9 | 44.39 | 1.88 | 4.52 | 50.3 | 0.02 | 318 | 512 | 1284 | 3.42 |
| 208 | gpu: distance + sphere + quadtree + gbuffer + pathtrace | 0.41 | 152.9 | 22.94 | 4.01 | 0.18 | 32.1 | 7.13 | 1786 | 2494 | 971 | 6.60 |
| 209 | gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.41 | 152.5 | 28.72 | 2.15 | 2.53 | 50.3 | 0.02 | 1382 | 1889 | 1128 | 5.75 |
| 210 | gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.41 | 152.3 | 28.21 | 3.22 | 2.60 | 50.3 | 0.02 | 1310 | 1849 | 1133 | 5.68 |
| 211 | gpu: distance + sphere + quadtree + raster + direct | 0.41 | 152.2 | 21.53 | 4.42 | 0.17 | 48.0 | 0.02 | 1739 | 2446 | 971 | 6.51 |
| 212 | gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.40 | 151.5 | 28.64 | 2.17 | 2.54 | 50.3 | 0.02 | 1374 | 1881 | 1137 | 5.75 |
| 213 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + direct | 0.40 | 151.4 | 28.90 | 3.32 | 2.60 | 50.4 | 0.02 | 1302 | 1840 | 1135 | 5.67 |
| 214 | gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.40 | 151.3 | 23.91 | 3.21 | 2.64 | 50.3 | 0.02 | 1318 | 1857 | 1134 | 5.70 |
| 215 | gpu: bricks + rowrun + quadtree + raster + direct | 0.40 | 150.8 | 10.47 | 6.36 | 0.18 | 48.0 | 0.02 | 1227 | 2078 | 976 | 5.82 |
| 216 | gpu: rows + rowrun + quadtree + raster + direct | 0.40 | 150.6 | 16.75 | 5.71 | 0.17 | 48.0 | 0.02 | 1291 | 2110 | 983 | 5.89 |
| 217 | gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 0.40 | 149.4 | 14.31 | 6.15 | 0.20 | 32.1 | 7.13 | 1346 | 2162 | 987 | 6.00 |
| 218 | gpu: naive@cpu + faces + tree64 + refloor + depth + raster + direct | 0.40 | 149.4 | 21.51 | 3.52 | 3.57 | 50.4 | 0.02 | 448 | 698 | 1122 | 3.47 |
| 219 | gpu: svo_dag + sphere + quadtree + visbuffer + direct | 0.40 | 148.9 | 21.68 | 5.41 | 2.17 | 48.0 | 0.02 | 235 | 370 | 1230 | 3.05 |
| 220 | gpu: svo_dag + sphere + quadtree + gbuffer + direct | 0.40 | 148.4 | 22.27 | 5.40 | 2.19 | 48.0 | 0.02 | 243 | 378 | 1228 | 3.06 |
| 221 | gpu: tree64 + incremental + composite + raycast + pathtrace | 0.40 | 148.4 | 9.00 | 6.46 | 1.17 | 32.2 | 7.12 | 223 | 606 | 1101 | 3.25 |
| 222 | gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 0.39 | 147.8 | 16.87 | 6.15 | 0.18 | 32.1 | 7.13 | 1354 | 2170 | 986 | 6.01 |
| 223 | gpu: naive@cpu + faces + bricks + rowrun + depth + raster + direct | 0.39 | 147.8 | 28.08 | 3.56 | 2.61 | 50.4 | 0.02 | 1302 | 1840 | 1134 | 5.66 |
| 224 | gpu: distance + rowrun + composite + raycast + pathtrace | 0.39 | 147.7 | 20.07 | 4.67 | 0.10 | 32.2 | 7.12 | 1588 | 2262 | 977 | 6.17 |
| 225 | gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.39 | 146.2 | 21.34 | 3.76 | 3.57 | 50.4 | 0.02 | 448 | 681 | 1129 | 3.45 |
| 226 | gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.39 | 146.1 | 41.47 | 2.36 | 4.51 | 50.3 | 0.02 | 318 | 512 | 1317 | 3.48 |
| 227 | gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 0.39 | 145.4 | 9.32 | 6.59 | 1.19 | 32.2 | 7.12 | 223 | 451 | 1110 | 2.97 |
| 228 | gpu: distance + refloor + quadtree + visbuffer + pathtrace | 0.39 | 145.3 | 20.68 | 4.57 | 0.18 | 32.1 | 7.13 | 1778 | 2486 | 971 | 6.58 |
| 229 | gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.39 | 145.3 | 41.38 | 2.37 | 4.54 | 50.3 | 0.02 | 311 | 503 | 1264 | 3.37 |
| 230 | gpu: clipmap + refloor + quadtree + raster + direct | 0.39 | 145.0 | 12.34 | 6.52 | 0.12 | 48.0 | 0.02 | 1421 | 1829 | 1093 | 5.57 |
| 231 | gpu: distance + refloor + quadtree + gbuffer + pathtrace | 0.39 | 144.6 | 20.91 | 4.70 | 0.20 | 32.1 | 7.13 | 1786 | 2494 | 977 | 6.61 |
| 232 | gpu: distance + rowrun + beam + composite + raycast + pathtrace | 0.38 | 144.1 | 21.49 | 4.84 | 0.11 | 32.2 | 7.12 | 1588 | 2262 | 970 | 6.16 |
| 233 | gpu: distance + refloor + quadtree + raster + direct | 0.38 | 144.0 | 21.96 | 4.78 | 0.17 | 48.0 | 0.02 | 1739 | 2446 | 977 | 6.52 |
| 234 | gpu: naive@cpu + faces + clipmap + refloor + depth + raster + direct | 0.38 | 143.3 | 22.53 | 3.75 | 2.51 | 50.4 | 0.02 | 1497 | 1742 | 1301 | 5.79 |
| 235 | gpu: naive@cpu + faces + prefix + refloor + depth + raster + direct | 0.38 | 143.0 | 26.06 | 3.17 | 2.55 | 50.4 | 0.02 | 1382 | 1884 | 1129 | 5.74 |
| 236 | gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 0.38 | 142.6 | 17.33 | 6.25 | 0.21 | 32.1 | 7.13 | 1330 | 2150 | 984 | 5.97 |
| 237 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.38 | 141.6 | 29.74 | 3.93 | 2.63 | 32.2 | 7.12 | 1342 | 1880 | 1135 | 5.74 |
| 238 | gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 0.38 | 141.5 | 22.39 | 5.83 | 2.20 | 32.1 | 7.13 | 267 | 401 | 1221 | 3.09 |
| 239 | gpu: naive@cpu + faces + hier + refloor + depth + raster + direct | 0.38 | 141.3 | 26.98 | 3.25 | 2.55 | 50.4 | 0.02 | 1302 | 1840 | 1134 | 5.67 |
| 240 | gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.38 | 141.3 | 28.11 | 3.25 | 2.57 | 50.3 | 0.02 | 1390 | 1893 | 1142 | 5.78 |
| 241 | gpu: svo_dag + refloor + quadtree + raster + direct | 0.38 | 141.2 | 22.13 | 5.87 | 2.17 | 48.0 | 0.02 | 227 | 361 | 1229 | 3.03 |
| 242 | gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.38 | 141.1 | 36.18 | 2.73 | 4.56 | 50.3 | 0.02 | 318 | 512 | 1294 | 3.44 |
| 243 | gpu: naive@cpu + faces + prefix + rowrun + depth + raster + direct | 0.38 | 141.0 | 26.04 | 3.30 | 2.57 | 50.4 | 0.02 | 1382 | 1884 | 1128 | 5.74 |
| 244 | gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.38 | 141.0 | 28.54 | 3.94 | 2.51 | 50.4 | 0.02 | 1497 | 1742 | 1302 | 5.80 |
| 245 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.38 | 140.7 | 29.26 | 4.06 | 2.62 | 32.2 | 7.12 | 1350 | 1889 | 1134 | 5.76 |
| 246 | gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 0.38 | 140.7 | 16.70 | 6.33 | 0.19 | 32.1 | 7.13 | 1338 | 2158 | 984 | 5.99 |
| 247 | gpu: hier + rowrun + quadtree + raster + direct | 0.37 | 140.6 | 18.46 | 6.41 | 0.20 | 48.0 | 0.02 | 1227 | 2078 | 986 | 5.83 |
| 248 | gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.37 | 140.5 | 26.61 | 3.28 | 2.60 | 50.3 | 0.02 | 1398 | 1901 | 1128 | 5.77 |
| 249 | gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.37 | 140.4 | 37.15 | 2.74 | 4.58 | 50.3 | 0.02 | 311 | 503 | 1270 | 3.38 |
| 250 | gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.37 | 140.3 | 25.64 | 3.25 | 2.60 | 50.3 | 0.02 | 1318 | 1857 | 1134 | 5.70 |
| 251 | gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.37 | 140.2 | 25.35 | 3.26 | 2.58 | 50.3 | 0.02 | 1310 | 1849 | 1133 | 5.68 |
| 252 | gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.37 | 140.0 | 21.40 | 4.18 | 3.56 | 32.2 | 7.12 | 496 | 711 | 1133 | 3.51 |
| 253 | gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 0.37 | 139.8 | 21.99 | 5.90 | 2.21 | 32.1 | 7.13 | 275 | 410 | 1221 | 3.10 |
| 254 | gpu: naive@cpu + faces + hier + rowrun + depth + raster + direct | 0.37 | 139.1 | 26.65 | 3.43 | 2.57 | 50.4 | 0.02 | 1302 | 1840 | 1135 | 5.67 |
| 255 | gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.37 | 138.4 | 21.85 | 4.19 | 3.62 | 32.2 | 7.12 | 488 | 852 | 1134 | 3.78 |
| 256 | gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.37 | 138.3 | 40.85 | 1.75 | 2.52 | 50.3 | 0.02 | 1830 | 2225 | 1129 | 6.39 |
| 257 | gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.37 | 138.2 | 39.22 | 1.75 | 2.52 | 50.3 | 0.02 | 1822 | 2217 | 1128 | 6.37 |
| 258 | gpu: naive@cpu + faces + rows + refloor + depth + raster + direct | 0.37 | 137.4 | 28.28 | 3.29 | 2.54 | 50.4 | 0.02 | 1366 | 1872 | 1129 | 5.72 |
| 259 | gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.37 | 137.3 | 37.93 | 1.81 | 2.50 | 50.3 | 0.02 | 1830 | 2225 | 1128 | 6.39 |
| 260 | gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.36 | 136.8 | 38.51 | 1.83 | 2.53 | 50.3 | 0.02 | 1822 | 2217 | 1129 | 6.37 |
| 261 | gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.36 | 136.8 | 37.38 | 1.86 | 2.53 | 50.3 | 0.02 | 1822 | 2217 | 1128 | 6.37 |
| 262 | gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.36 | 136.2 | 25.30 | 3.65 | 2.58 | 32.2 | 7.12 | 1422 | 1925 | 1129 | 5.82 |
| 263 | gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.36 | 136.1 | 38.35 | 1.85 | 2.51 | 50.3 | 0.02 | 1830 | 2225 | 1133 | 6.40 |
| 264 | gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 0.36 | 136.0 | 12.51 | 7.02 | 0.14 | 32.1 | 7.13 | 1461 | 1869 | 1094 | 5.64 |
| 265 | gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.36 | 135.7 | 25.34 | 3.70 | 2.60 | 32.2 | 7.12 | 1430 | 1933 | 1129 | 5.83 |
| 266 | gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 0.36 | 135.3 | 13.24 | 7.06 | 0.12 | 32.1 | 7.13 | 1469 | 1877 | 1089 | 5.65 |
| 267 | gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.36 | 135.0 | 26.22 | 3.46 | 2.58 | 50.3 | 0.02 | 1382 | 1889 | 1129 | 5.75 |
| 268 | gpu: naive@cpu + faces + rows + rowrun + depth + raster + direct | 0.36 | 134.9 | 27.97 | 3.50 | 2.56 | 50.4 | 0.02 | 1366 | 1872 | 1128 | 5.71 |
| 269 | gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.36 | 134.7 | 26.67 | 3.71 | 2.59 | 32.2 | 7.12 | 1350 | 1889 | 1134 | 5.76 |
| 270 | gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.36 | 134.6 | 59.68 | 0.97 | 5.96 | 48.8 | 0.02 | 336 | 582 | 1165 | 3.33 |
| 271 | gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.36 | 134.4 | 26.83 | 3.66 | 2.61 | 32.2 | 7.12 | 1342 | 1880 | 1134 | 5.74 |
| 272 | gpu: distance + rowrun + quadtree + raster + direct | 0.36 | 134.3 | 21.47 | 5.53 | 0.19 | 48.0 | 0.02 | 1739 | 2446 | 974 | 6.51 |
| 273 | gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.36 | 134.0 | 27.50 | 3.48 | 2.58 | 50.3 | 0.02 | 1374 | 1881 | 1128 | 5.73 |
| 274 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.36 | 133.8 | 58.57 | 1.02 | 5.95 | 48.8 | 0.02 | 336 | 565 | 1139 | 3.25 |
| 275 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.36 | 133.8 | 59.58 | 1.03 | 5.95 | 48.7 | 0.02 | 328 | 569 | 1173 | 3.32 |
| 276 | gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.36 | 133.2 | 60.69 | 0.99 | 6.01 | 48.7 | 0.02 | 328 | 554 | 1136 | 3.22 |
| 277 | gpu: hier + rowrun + composite + raycast + pathtrace | 0.35 | 132.3 | 15.74 | 7.07 | 0.12 | 32.2 | 7.12 | 1077 | 1894 | 981 | 5.48 |
| 278 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.35 | 131.8 | 40.16 | 0.93 | 5.07 | 48.8 | 0.02 | 1190 | 1857 | 1078 | 5.59 |
| 279 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.35 | 131.6 | 41.32 | 0.93 | 5.07 | 48.7 | 0.02 | 1182 | 1849 | 1083 | 5.58 |
| 280 | gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.35 | 131.3 | 40.96 | 1.00 | 5.08 | 48.8 | 0.02 | 1190 | 1857 | 1084 | 5.60 |
| 281 | gpu: distance + incremental + quadtree + gbuffer + direct | 0.35 | 131.3 | 20.21 | 5.82 | 0.19 | 48.0 | 0.02 | 1755 | 2463 | 978 | 6.55 |
| 282 | gpu: hier + sphere + composite + raycast + pathtrace | 0.35 | 131.3 | 16.10 | 7.21 | 0.10 | 32.2 | 7.12 | 1077 | 1894 | 978 | 5.47 |
| 283 | gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.35 | 131.2 | 38.51 | 1.28 | 4.95 | 48.8 | 0.02 | 1190 | 1857 | 1078 | 5.59 |
| 284 | gpu: naive@cpu + faces + bricks + sphere + depth + raster + direct | 0.35 | 131.1 | 22.63 | 4.86 | 2.62 | 50.4 | 0.02 | 1302 | 1840 | 1132 | 5.66 |
| 285 | gpu: svo_dag + incremental + quadtree + gbuffer + direct | 0.35 | 130.6 | 22.25 | 6.52 | 2.21 | 48.0 | 0.02 | 243 | 378 | 1276 | 3.15 |
| 286 | gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.35 | 130.6 | 58.35 | 1.27 | 5.98 | 48.7 | 0.02 | 328 | 585 | 1139 | 3.28 |
| 287 | gpu: svo_dag + incremental + quadtree + visbuffer + direct | 0.35 | 130.5 | 22.62 | 6.57 | 2.18 | 48.0 | 0.02 | 235 | 370 | 1275 | 3.13 |
| 288 | gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 0.35 | 130.4 | 20.53 | 5.95 | 0.20 | 32.1 | 7.13 | 1778 | 2486 | 975 | 6.59 |
| 289 | gpu: distance + incremental + quadtree + visbuffer + direct | 0.35 | 130.4 | 19.94 | 5.84 | 0.19 | 48.0 | 0.02 | 1747 | 2454 | 977 | 6.54 |
| 290 | gpu: naive@cpu + faces + bricks + incremental + depth + raster + direct | 0.35 | 130.3 | 28.75 | 4.89 | 2.63 | 50.4 | 0.02 | 1302 | 1840 | 1134 | 5.66 |
| 291 | gpu: rows + incremental + quadtree + raster + direct | 0.35 | 130.0 | 15.96 | 7.15 | 0.20 | 48.0 | 0.02 | 1291 | 2110 | 988 | 5.90 |
| 292 | gpu: clipmap + rowrun + composite + raycast + pathtrace | 0.35 | 130.0 | 14.36 | 7.38 | 0.03 | 32.2 | 7.12 | 1271 | 1643 | 1094 | 5.21 |
| 293 | gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.35 | 129.9 | 60.30 | 1.30 | 5.98 | 48.8 | 0.02 | 336 | 554 | 1156 | 3.26 |
| 294 | gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.35 | 129.8 | 41.71 | 1.00 | 5.12 | 48.7 | 0.02 | 1182 | 1849 | 1084 | 5.59 |
| 295 | gpu: svo_dag + rowrun + quadtree + visbuffer + direct | 0.35 | 129.7 | 22.33 | 6.58 | 2.22 | 48.0 | 0.02 | 235 | 370 | 1275 | 3.13 |
| 296 | gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.35 | 129.5 | 36.91 | 3.67 | 4.57 | 50.4 | 0.02 | 303 | 495 | 1276 | 3.37 |
| 297 | gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.34 | 129.2 | 41.18 | 1.16 | 4.87 | 48.8 | 0.02 | 1384 | 1605 | 1204 | 5.35 |
| 298 | gpu: svo_dag + rowrun + quadtree + gbuffer + direct | 0.34 | 129.1 | 22.03 | 6.59 | 2.24 | 48.0 | 0.02 | 243 | 378 | 1275 | 3.15 |
| 299 | gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.34 | 129.1 | 27.09 | 3.90 | 2.60 | 32.2 | 7.12 | 1406 | 1912 | 1129 | 5.79 |
| 300 | gpu: naive@cpu + faces + tree64 + incremental + depth + raster + direct | 0.34 | 128.8 | 21.28 | 4.96 | 3.60 | 50.4 | 0.02 | 448 | 711 | 1124 | 3.49 |
| 301 | gpu: hier + rowrun + beam + composite + raycast + pathtrace | 0.34 | 128.7 | 14.68 | 7.43 | 0.10 | 32.2 | 7.12 | 1077 | 1894 | 988 | 5.49 |
| 302 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.34 | 128.5 | 40.81 | 1.20 | 4.89 | 48.8 | 0.02 | 1384 | 1605 | 1207 | 5.35 |
| 303 | gpu: clipmap + incremental + composite + raycast + pathtrace | 0.34 | 128.3 | 12.99 | 7.53 | 0.05 | 32.2 | 7.12 | 1271 | 1643 | 1095 | 5.21 |
| 304 | gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.34 | 128.3 | 41.14 | 1.18 | 4.90 | 48.7 | 0.02 | 1377 | 1597 | 1206 | 5.34 |
| 305 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.34 | 128.2 | 41.13 | 1.19 | 4.90 | 48.7 | 0.02 | 1377 | 1597 | 1208 | 5.34 |
| 306 | gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.34 | 128.2 | 28.24 | 3.99 | 2.60 | 32.2 | 7.12 | 1414 | 1921 | 1128 | 5.81 |
| 307 | gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.34 | 127.8 | 58.54 | 1.56 | 5.95 | 48.7 | 0.02 | 328 | 542 | 1141 | 3.21 |
| 308 | gpu: hier + sphere + beam + composite + raycast + pathtrace | 0.34 | 127.8 | 17.82 | 7.41 | 0.11 | 32.2 | 7.12 | 1077 | 1894 | 980 | 5.48 |
| 309 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.34 | 127.5 | 35.17 | 3.88 | 4.55 | 50.4 | 0.02 | 303 | 495 | 1303 | 3.42 |
| 310 | gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.34 | 127.2 | 58.68 | 1.58 | 5.98 | 48.8 | 0.02 | 336 | 562 | 1155 | 3.27 |
| 311 | gpu: naive@cpu + faces + rows + incremental + depth + raster + direct | 0.34 | 127.2 | 26.62 | 4.20 | 2.59 | 50.4 | 0.02 | 1366 | 1872 | 1128 | 5.71 |
| 312 | gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.34 | 127.1 | 39.47 | 1.37 | 4.87 | 48.8 | 0.02 | 1384 | 1605 | 1207 | 5.36 |
| 313 | gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.34 | 127.0 | 40.21 | 1.31 | 5.08 | 48.7 | 0.02 | 1182 | 1849 | 1078 | 5.57 |
| 314 | gpu: clipmap + rowrun + quadtree + raster + direct | 0.34 | 126.8 | 12.38 | 7.59 | 0.14 | 48.0 | 0.02 | 1421 | 1829 | 1092 | 5.56 |
| 315 | gpu: hier + incremental + quadtree + raster + direct | 0.34 | 126.7 | 15.81 | 7.46 | 0.18 | 48.0 | 0.02 | 1227 | 2078 | 984 | 5.83 |
| 316 | gpu: naive@cpu + faces + clipmap + incremental + depth + raster + direct | 0.34 | 126.7 | 26.93 | 5.15 | 2.52 | 50.4 | 0.02 | 1497 | 1742 | 1302 | 5.80 |
| 317 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + direct | 0.34 | 126.4 | 38.12 | 1.60 | 5.03 | 48.8 | 0.02 | 1174 | 1840 | 1024 | 5.46 |
| 318 | gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.34 | 126.4 | 39.93 | 1.40 | 4.88 | 48.7 | 0.02 | 1377 | 1597 | 1209 | 5.34 |
| 319 | gpu: boxes@cpu + rayquery + raycast + pathtrace | 0.34 | 126.3 | 27.05 | 4.64 | 2.62 | 32.2 | 7.12 | 1169 | 1865 | 1680 | 6.75 |
| 320 | gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + direct | 0.34 | 126.2 | 58.16 | 1.79 | 5.92 | 48.8 | 0.02 | 320 | 545 | 1101 | 3.13 |
| 321 | gpu: naive@cpu + faces + hier + incremental + depth + raster + direct | 0.34 | 125.7 | 22.64 | 4.58 | 2.56 | 50.4 | 0.02 | 1302 | 1840 | 1133 | 5.66 |
| 322 | gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.33 | 125.1 | 39.73 | 1.57 | 4.89 | 48.7 | 0.02 | 1377 | 1597 | 1205 | 5.34 |
| 323 | gpu: naive@cpu + faces + hier + sphere + depth + raster + direct | 0.33 | 125.0 | 23.02 | 4.59 | 2.59 | 50.4 | 0.02 | 1302 | 1840 | 1134 | 5.66 |
| 324 | gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.33 | 124.9 | 45.15 | 0.89 | 4.97 | 48.8 | 0.02 | 1270 | 1901 | 1077 | 5.67 |
| 325 | gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.33 | 124.7 | 45.03 | 0.91 | 4.97 | 48.7 | 0.02 | 1262 | 1893 | 1078 | 5.66 |
| 326 | gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.33 | 124.4 | 44.95 | 0.95 | 4.98 | 48.8 | 0.02 | 1270 | 1901 | 1084 | 5.69 |
| 327 | gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.33 | 124.3 | 22.96 | 4.75 | 2.62 | 32.2 | 7.12 | 1430 | 1933 | 1129 | 5.83 |
| 328 | gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.33 | 124.2 | 45.17 | 0.92 | 4.97 | 48.7 | 0.02 | 1182 | 1849 | 1079 | 5.58 |
| 329 | gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.33 | 124.1 | 44.54 | 0.95 | 4.96 | 48.7 | 0.02 | 1262 | 1893 | 1083 | 5.67 |
| 330 | gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.33 | 124.1 | 24.13 | 4.77 | 2.62 | 32.2 | 7.12 | 1422 | 1925 | 1129 | 5.82 |
| 331 | gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.33 | 123.9 | 39.67 | 1.63 | 4.91 | 48.8 | 0.02 | 1384 | 1605 | 1205 | 5.35 |
| 332 | gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.33 | 123.8 | 45.74 | 0.92 | 4.97 | 48.8 | 0.02 | 1190 | 1857 | 1078 | 5.59 |
| 333 | gpu: bricks + incremental + quadtree + raster + direct | 0.33 | 123.6 | 11.38 | 7.84 | 0.20 | 48.0 | 0.02 | 1227 | 2078 | 1019 | 5.90 |
| 334 | gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + direct | 0.33 | 123.5 | 38.16 | 1.80 | 5.05 | 48.8 | 0.02 | 1174 | 1840 | 1023 | 5.45 |
| 335 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.33 | 123.4 | 57.13 | 2.02 | 5.95 | 48.8 | 0.02 | 320 | 564 | 1092 | 3.15 |
| 336 | gpu: naive@cpu + faces + distance + sphere + depth + raster + direct | 0.33 | 123.2 | 35.92 | 3.16 | 2.53 | 50.4 | 0.02 | 1814 | 2208 | 1127 | 6.35 |
| 337 | gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 0.33 | 123.2 | 21.42 | 6.35 | 0.21 | 32.1 | 7.13 | 1786 | 2494 | 989 | 6.63 |
| 338 | gpu: hier + incremental + composite + raycast + pathtrace | 0.33 | 123.2 | 15.74 | 7.75 | 0.13 | 32.2 | 7.12 | 1077 | 1894 | 988 | 5.49 |
| 339 | gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.33 | 123.2 | 46.22 | 1.00 | 4.96 | 48.7 | 0.02 | 1182 | 1849 | 1081 | 5.58 |
| 340 | gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.33 | 123.1 | 45.24 | 1.00 | 4.98 | 48.8 | 0.02 | 1190 | 1857 | 1081 | 5.60 |
| 341 | gpu: naive@cpu + faces + prefix + sphere + depth + raster + direct | 0.33 | 122.6 | 29.80 | 4.94 | 2.59 | 50.4 | 0.02 | 1382 | 1884 | 1128 | 5.74 |
| 342 | gpu: naive@cpu + faces + distance + refloor + depth + raster + direct | 0.33 | 122.6 | 36.15 | 3.24 | 2.54 | 50.4 | 0.02 | 1814 | 2208 | 1128 | 6.35 |
| 343 | gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.33 | 122.6 | 21.72 | 5.49 | 2.58 | 32.2 | 7.12 | 1536 | 1782 | 1354 | 5.97 |
| 344 | gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.33 | 122.2 | 21.80 | 5.48 | 2.57 | 32.2 | 7.12 | 1544 | 1790 | 1354 | 5.99 |
| 345 | gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.33 | 122.1 | 34.98 | 4.31 | 4.60 | 32.2 | 7.12 | 342 | 535 | 1331 | 3.55 |
| 346 | gpu: naive@cpu + faces + prefix + incremental + depth + raster + direct | 0.33 | 122.1 | 23.97 | 5.05 | 2.59 | 50.4 | 0.02 | 1382 | 1884 | 1128 | 5.74 |
| 347 | gpu: naive@cpu + faces + clipmap + sphere + depth + raster + direct | 0.32 | 121.7 | 21.28 | 5.68 | 2.53 | 50.4 | 0.02 | 1497 | 1742 | 1302 | 5.80 |
| 348 | gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.32 | 121.7 | 47.18 | 0.93 | 4.96 | 48.8 | 0.02 | 1254 | 1889 | 1077 | 5.65 |
| 349 | gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 0.32 | 121.5 | 11.16 | 7.98 | 0.20 | 32.1 | 7.13 | 1266 | 2118 | 975 | 5.89 |
| 350 | gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + direct | 0.32 | 121.3 | 38.47 | 2.03 | 4.86 | 48.8 | 0.02 | 1369 | 1588 | 1158 | 5.23 |
| 351 | gpu: hier + incremental + beam + composite + raycast + pathtrace | 0.32 | 121.3 | 15.77 | 7.90 | 0.10 | 32.2 | 7.12 | 1077 | 1894 | 992 | 5.50 |
| 352 | gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.32 | 121.3 | 44.36 | 1.23 | 4.96 | 48.8 | 0.02 | 1190 | 1857 | 1079 | 5.59 |
| 353 | gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.32 | 121.3 | 35.67 | 4.37 | 4.62 | 32.2 | 7.12 | 350 | 543 | 1326 | 3.56 |
| 354 | gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 0.32 | 121.3 | 11.37 | 8.00 | 0.19 | 32.1 | 7.13 | 1274 | 2126 | 975 | 5.91 |
| 355 | gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.32 | 121.3 | 47.62 | 0.94 | 4.97 | 48.7 | 0.02 | 1246 | 1881 | 1077 | 5.63 |
| 356 | gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.32 | 121.2 | 44.54 | 1.24 | 4.97 | 48.7 | 0.02 | 1182 | 1849 | 1079 | 5.58 |
| 357 | gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.32 | 121.1 | 44.80 | 1.35 | 4.96 | 48.8 | 0.02 | 1270 | 1901 | 1077 | 5.67 |
| 358 | gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.32 | 121.0 | 43.47 | 1.33 | 4.97 | 48.7 | 0.02 | 1262 | 1893 | 1079 | 5.66 |
| 359 | gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.32 | 120.7 | 47.39 | 0.99 | 4.96 | 48.8 | 0.02 | 1254 | 1889 | 1078 | 5.65 |
| 360 | gpu: naive@cpu + faces + rows + sphere + depth + raster + direct | 0.32 | 120.6 | 23.46 | 4.81 | 2.59 | 50.4 | 0.02 | 1366 | 1872 | 1129 | 5.72 |
| 361 | gpu: naive@cpu + faces + distance + rowrun + depth + raster + direct | 0.32 | 120.5 | 36.45 | 3.43 | 2.55 | 50.4 | 0.02 | 1814 | 2208 | 1128 | 6.35 |
| 362 | gpu: greedy@cpu + faces + prefix + refloor + depth + raster + direct | 0.32 | 120.5 | 42.09 | 1.54 | 4.93 | 48.8 | 0.02 | 1254 | 1884 | 1021 | 5.53 |
| 363 | gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.32 | 120.5 | 36.09 | 3.32 | 2.60 | 32.2 | 7.12 | 1854 | 2248 | 1130 | 6.43 |
| 364 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.32 | 120.5 | 37.65 | 2.16 | 4.84 | 48.8 | 0.02 | 1369 | 1588 | 1158 | 5.23 |
| 365 | gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.32 | 120.5 | 47.35 | 0.98 | 4.97 | 48.7 | 0.02 | 1246 | 1881 | 1078 | 5.63 |
| 366 | gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.32 | 120.3 | 35.69 | 3.31 | 2.59 | 32.2 | 7.12 | 1862 | 2257 | 1129 | 6.45 |
| 367 | gpu: bricks + sphere + composite + raycast + pathtrace | 0.32 | 120.3 | 11.67 | 8.10 | 0.13 | 32.2 | 7.12 | 1076 | 1894 | 968 | 5.45 |
| 368 | gpu: bricks + incremental + composite + raycast + pathtrace | 0.32 | 119.9 | 12.26 | 8.13 | 0.12 | 32.2 | 7.12 | 1076 | 1894 | 977 | 5.47 |
| 369 | gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + direct | 0.32 | 119.3 | 41.70 | 1.65 | 4.93 | 48.8 | 0.02 | 1254 | 1884 | 1022 | 5.53 |
| 370 | gpu: greedy@cpu + faces + hier + refloor + depth + raster + direct | 0.32 | 119.0 | 41.84 | 1.59 | 4.94 | 48.8 | 0.02 | 1174 | 1840 | 1023 | 5.45 |
| 371 | gpu: rows + incremental + beam + composite + raycast + pathtrace | 0.32 | 119.0 | 17.77 | 7.97 | 0.14 | 32.2 | 7.12 | 1140 | 1926 | 988 | 5.55 |
| 372 | gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.32 | 118.8 | 45.41 | 1.31 | 4.95 | 48.7 | 0.02 | 1246 | 1881 | 1081 | 5.64 |
| 373 | gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 0.32 | 118.7 | 15.56 | 8.17 | 0.04 | 32.2 | 7.12 | 1271 | 1643 | 1098 | 5.22 |
| 374 | gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.32 | 118.6 | 47.15 | 1.28 | 4.97 | 48.8 | 0.02 | 1254 | 1889 | 1077 | 5.65 |
| 375 | gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.32 | 118.4 | 24.59 | 4.95 | 2.64 | 32.2 | 7.12 | 1414 | 1921 | 1129 | 5.81 |
| 376 | gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.32 | 118.3 | 24.38 | 4.96 | 2.67 | 32.2 | 7.12 | 1406 | 1912 | 1146 | 5.82 |
| 377 | gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.32 | 118.2 | 21.77 | 5.97 | 2.68 | 32.2 | 7.12 | 1342 | 1880 | 1133 | 5.74 |
| 378 | gpu: greedy@cpu + faces + hier + rowrun + depth + raster + direct | 0.32 | 118.1 | 42.11 | 1.73 | 4.91 | 48.8 | 0.02 | 1174 | 1840 | 1023 | 5.45 |
| 379 | gpu: bricks + incremental + beam + composite + raycast + pathtrace | 0.31 | 117.9 | 12.46 | 8.27 | 0.11 | 32.2 | 7.12 | 1076 | 1894 | 986 | 5.49 |
| 380 | gpu: rows + sphere + composite + raycast + pathtrace | 0.31 | 117.8 | 17.89 | 8.12 | 0.13 | 32.2 | 7.12 | 1140 | 1926 | 975 | 5.53 |
| 381 | gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.31 | 117.7 | 36.40 | 3.65 | 2.60 | 32.2 | 7.12 | 1854 | 2248 | 1128 | 6.43 |
| 382 | gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.31 | 117.4 | 21.40 | 5.85 | 3.66 | 32.2 | 7.12 | 496 | 751 | 1145 | 3.61 |
| 383 | gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.31 | 117.4 | 37.58 | 2.34 | 5.13 | 48.7 | 0.02 | 1182 | 1849 | 1081 | 5.58 |
| 384 | gpu: naive@cpu + faces + tree64 + sphere + depth + raster + direct | 0.31 | 117.2 | 20.96 | 5.93 | 3.63 | 50.4 | 0.02 | 448 | 647 | 1140 | 3.40 |
| 385 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.31 | 117.2 | 68.09 | 0.97 | 7.03 | 48.8 | 0.02 | 190 | 288 | 1272 | 2.97 |
| 386 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.31 | 117.2 | 70.14 | 1.02 | 7.00 | 48.7 | 0.02 | 183 | 279 | 1275 | 2.96 |
| 387 | gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 0.31 | 117.1 | 10.92 | 8.31 | 1.30 | 32.1 | 7.13 | 421 | 709 | 1191 | 3.62 |
| 388 | gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.31 | 117.0 | 22.39 | 5.99 | 2.70 | 32.2 | 7.12 | 1350 | 1889 | 1174 | 5.83 |
| 389 | gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.31 | 116.9 | 21.49 | 5.95 | 3.64 | 32.2 | 7.12 | 488 | 717 | 1155 | 3.57 |
| 390 | gpu: tree64 + rowrun + quadtree + visbuffer + direct | 0.31 | 116.9 | 11.13 | 8.31 | 1.29 | 48.0 | 0.02 | 381 | 665 | 1181 | 3.51 |
| 391 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.31 | 116.8 | 71.11 | 1.01 | 7.02 | 48.8 | 0.02 | 190 | 288 | 1272 | 2.97 |
| 392 | gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.31 | 116.8 | 34.08 | 3.80 | 2.62 | 50.3 | 0.02 | 1830 | 2225 | 1128 | 6.39 |
| 393 | gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.31 | 116.8 | 33.76 | 3.80 | 2.55 | 32.2 | 7.12 | 1862 | 2257 | 1129 | 6.45 |
| 394 | gpu: tree64 + rowrun + quadtree + gbuffer + direct | 0.31 | 116.7 | 11.29 | 8.32 | 1.29 | 48.0 | 0.02 | 389 | 692 | 1179 | 3.56 |
| 395 | gpu: greedy@cpu + faces + rows + refloor + depth + raster + direct | 0.31 | 116.6 | 43.60 | 1.63 | 4.93 | 48.8 | 0.02 | 1238 | 1872 | 1021 | 5.51 |
| 396 | gpu: bricks + sphere + beam + composite + raycast + pathtrace | 0.31 | 116.5 | 11.19 | 8.38 | 0.12 | 32.2 | 7.12 | 1076 | 1894 | 974 | 5.46 |
| 397 | gpu: prefix + sphere + composite + raycast + pathtrace | 0.31 | 116.5 | 16.94 | 8.28 | 0.13 | 32.2 | 7.12 | 1157 | 1939 | 978 | 5.56 |
| 398 | gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.31 | 116.4 | 33.02 | 3.83 | 2.60 | 50.3 | 0.02 | 1822 | 2217 | 1168 | 6.45 |
| 399 | gpu: prefix + incremental + quadtree + raster + direct | 0.31 | 116.3 | 14.21 | 8.27 | 0.18 | 48.0 | 0.02 | 1307 | 2122 | 994 | 5.93 |
| 400 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.31 | 116.1 | 69.55 | 0.97 | 7.08 | 48.7 | 0.02 | 183 | 279 | 1273 | 2.96 |
| 401 | gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + direct | 0.31 | 115.9 | 38.56 | 2.80 | 5.97 | 48.8 | 0.02 | 320 | 649 | 1089 | 3.31 |
| 402 | gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 0.31 | 115.8 | 13.00 | 8.38 | 1.30 | 32.1 | 7.13 | 413 | 851 | 1187 | 3.88 |
| 403 | gpu: prefix + sphere + beam + composite + raycast + pathtrace | 0.31 | 115.7 | 16.70 | 8.33 | 0.11 | 32.2 | 7.12 | 1157 | 1939 | 980 | 5.56 |
| 404 | gpu: greedy@cpu + faces + rows + rowrun + depth + raster + direct | 0.31 | 115.1 | 41.62 | 1.80 | 4.93 | 48.8 | 0.02 | 1238 | 1872 | 1021 | 5.51 |
| 405 | gpu: greedy@cpu + faces + bricks + sphere + depth + raster + direct | 0.31 | 114.9 | 36.70 | 2.73 | 5.09 | 48.8 | 0.02 | 1174 | 1840 | 1022 | 5.45 |
| 406 | gpu: rows + sphere + beam + composite + raycast + pathtrace | 0.31 | 114.4 | 17.17 | 8.36 | 0.11 | 32.2 | 7.12 | 1140 | 1926 | 979 | 5.53 |
| 407 | gpu: rows + incremental + composite + raycast + pathtrace | 0.30 | 114.2 | 16.44 | 8.34 | 0.11 | 32.2 | 7.12 | 1140 | 1926 | 982 | 5.54 |
| 408 | gpu: prefix + incremental + composite + raycast + pathtrace | 0.30 | 114.1 | 15.56 | 8.46 | 0.10 | 32.2 | 7.12 | 1157 | 1939 | 988 | 5.57 |
| 409 | gpu: prefix + incremental + beam + composite + raycast + pathtrace | 0.30 | 113.6 | 16.28 | 8.51 | 0.10 | 32.2 | 7.12 | 1157 | 1939 | 996 | 5.59 |
| 410 | gpu: greedy@cpu + faces + bricks + incremental + depth + raster + direct | 0.30 | 113.0 | 42.00 | 2.74 | 5.10 | 48.8 | 0.02 | 1174 | 1840 | 1022 | 5.45 |
| 411 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.30 | 112.3 | 68.62 | 1.45 | 7.08 | 48.8 | 0.02 | 190 | 288 | 1274 | 2.97 |
| 412 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.30 | 112.3 | 70.16 | 1.45 | 7.06 | 48.7 | 0.02 | 183 | 279 | 1270 | 2.95 |
| 413 | gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 0.30 | 112.1 | 13.11 | 8.67 | 0.05 | 32.2 | 7.12 | 1271 | 1643 | 1102 | 5.23 |
| 414 | gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.30 | 112.1 | 41.51 | 2.38 | 5.00 | 48.8 | 0.02 | 1270 | 1901 | 1082 | 5.68 |
| 415 | gpu: hier + sphere + quadtree + raster + direct | 0.30 | 112.0 | 15.69 | 8.60 | 0.18 | 48.0 | 0.02 | 1227 | 2078 | 1024 | 5.91 |
| 416 | gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.30 | 112.0 | 40.85 | 2.41 | 5.00 | 48.7 | 0.02 | 1262 | 1893 | 1082 | 5.66 |
| 417 | gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + direct | 0.30 | 111.9 | 33.76 | 3.07 | 4.92 | 48.8 | 0.02 | 1369 | 1588 | 1158 | 5.23 |
| 418 | gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.30 | 111.8 | 35.25 | 3.31 | 6.00 | 32.1 | 7.13 | 368 | 625 | 1107 | 3.30 |
| 419 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.30 | 111.4 | 38.65 | 3.07 | 5.15 | 32.1 | 7.13 | 1214 | 1880 | 1088 | 5.65 |
| 420 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.30 | 111.4 | 65.22 | 1.87 | 6.95 | 48.8 | 0.02 | 175 | 271 | 1225 | 2.85 |
| 421 | gpu: greedy@cpu + faces + hier + incremental + depth + raster + direct | 0.30 | 111.4 | 38.43 | 2.52 | 4.95 | 48.8 | 0.02 | 1174 | 1840 | 1023 | 5.45 |
| 422 | gpu: greedy@cpu + faces + rows + incremental + depth + raster + direct | 0.30 | 111.2 | 41.36 | 2.29 | 4.94 | 48.8 | 0.02 | 1238 | 1872 | 1021 | 5.51 |
| 423 | gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.30 | 111.1 | 40.74 | 2.38 | 5.02 | 48.7 | 0.02 | 1182 | 1849 | 1083 | 5.58 |
| 424 | gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.30 | 111.1 | 35.63 | 3.35 | 6.02 | 32.1 | 7.13 | 360 | 583 | 1095 | 3.20 |
| 425 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.30 | 110.9 | 64.82 | 2.01 | 6.93 | 48.8 | 0.02 | 175 | 271 | 1224 | 2.85 |
| 426 | gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.30 | 110.8 | 42.01 | 2.42 | 4.99 | 48.8 | 0.02 | 1190 | 1857 | 1083 | 5.60 |
| 427 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.30 | 110.6 | 67.08 | 1.87 | 7.01 | 48.7 | 0.02 | 183 | 279 | 1278 | 2.97 |
| 428 | gpu: greedy@cpu + faces + hier + sphere + depth + raster + direct | 0.29 | 110.5 | 39.96 | 2.58 | 4.96 | 48.8 | 0.02 | 1174 | 1840 | 1023 | 5.45 |
| 429 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.29 | 110.3 | 70.18 | 1.86 | 7.02 | 48.8 | 0.02 | 190 | 288 | 1277 | 2.98 |
| 430 | gpu: greedy@cpu + faces + prefix + sphere + depth + raster + direct | 0.29 | 110.0 | 39.77 | 2.81 | 4.94 | 48.8 | 0.02 | 1254 | 1884 | 1021 | 5.53 |
| 431 | gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.29 | 109.9 | 32.17 | 4.62 | 2.62 | 32.2 | 7.12 | 1854 | 2248 | 1131 | 6.44 |
| 432 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.29 | 109.8 | 39.68 | 3.21 | 5.17 | 32.1 | 7.13 | 1222 | 1889 | 1083 | 5.66 |
| 433 | gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + direct | 0.29 | 109.8 | 34.47 | 3.38 | 4.90 | 48.8 | 0.02 | 1369 | 1588 | 1158 | 5.23 |
| 434 | gpu: tree64 + incremental + quadtree + raster + direct | 0.29 | 109.8 | 11.61 | 8.90 | 1.28 | 48.0 | 0.02 | 373 | 913 | 1159 | 3.95 |
| 435 | gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.29 | 109.7 | 20.91 | 6.93 | 2.52 | 32.2 | 7.12 | 1536 | 1782 | 1361 | 5.99 |
| 436 | gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.29 | 109.5 | 57.40 | 0.88 | 4.97 | 48.7 | 0.02 | 1694 | 2217 | 1078 | 6.28 |
| 437 | gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.29 | 109.5 | 56.22 | 0.91 | 4.96 | 48.8 | 0.02 | 1702 | 2225 | 1075 | 6.29 |
| 438 | gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + direct | 0.29 | 109.3 | 35.12 | 3.51 | 6.01 | 48.8 | 0.02 | 320 | 558 | 1100 | 3.16 |
| 439 | gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.29 | 109.2 | 34.98 | 5.60 | 4.59 | 50.4 | 0.02 | 303 | 495 | 1276 | 3.37 |
| 440 | gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.29 | 109.1 | 20.91 | 6.94 | 2.56 | 32.2 | 7.12 | 1544 | 1790 | 1361 | 6.00 |
| 441 | gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.29 | 109.0 | 56.43 | 0.96 | 4.97 | 48.8 | 0.02 | 1702 | 2225 | 1080 | 6.29 |
| 442 | gpu: hier + sphere + quadtree + visbuffer + pathtrace | 0.29 | 108.9 | 15.29 | 8.88 | 0.24 | 32.1 | 7.13 | 1267 | 2118 | 1027 | 5.99 |
| 443 | gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.29 | 108.8 | 56.60 | 0.87 | 4.99 | 48.8 | 0.02 | 1702 | 2225 | 1076 | 6.29 |
| 444 | gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.29 | 108.5 | 57.22 | 0.93 | 4.97 | 48.7 | 0.02 | 1694 | 2217 | 1078 | 6.27 |
| 445 | gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.29 | 108.5 | 56.37 | 0.96 | 4.97 | 48.7 | 0.02 | 1694 | 2217 | 1081 | 6.28 |
| 446 | gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.29 | 108.5 | 40.56 | 2.84 | 5.02 | 32.1 | 7.13 | 1294 | 1925 | 1022 | 5.61 |
| 447 | gpu: hier + sphere + quadtree + gbuffer + pathtrace | 0.29 | 108.4 | 14.55 | 8.92 | 0.19 | 32.1 | 7.13 | 1274 | 2126 | 1027 | 6.01 |
| 448 | gpu: greedy@cpu + faces + prefix + incremental + depth + raster + direct | 0.29 | 108.3 | 38.90 | 2.85 | 4.97 | 48.8 | 0.02 | 1254 | 1884 | 1021 | 5.53 |
| 449 | gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.29 | 108.2 | 37.54 | 2.88 | 5.01 | 32.1 | 7.13 | 1302 | 1933 | 1023 | 5.63 |
| 450 | gpu: naive@cpu + faces + distance + incremental + depth + raster + direct | 0.29 | 107.7 | 30.48 | 4.98 | 2.59 | 50.4 | 0.02 | 1814 | 2208 | 1129 | 6.36 |
| 451 | gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.29 | 107.6 | 41.90 | 2.87 | 5.01 | 32.1 | 7.13 | 1222 | 1889 | 1083 | 5.66 |
| 452 | gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.29 | 107.6 | 43.68 | 2.62 | 4.99 | 48.7 | 0.02 | 1246 | 1881 | 1082 | 5.64 |
| 453 | gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 0.29 | 107.1 | 14.41 | 9.05 | 0.15 | 32.1 | 7.13 | 1461 | 1869 | 1134 | 5.72 |
| 454 | gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.28 | 106.7 | 20.53 | 7.29 | 2.56 | 32.2 | 7.12 | 1536 | 1782 | 1362 | 5.99 |
| 455 | gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.28 | 106.6 | 39.20 | 2.85 | 5.04 | 32.1 | 7.13 | 1214 | 1880 | 1084 | 5.65 |
| 456 | gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.28 | 106.6 | 22.43 | 7.28 | 2.57 | 32.2 | 7.12 | 1544 | 1790 | 1362 | 6.00 |
| 457 | gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.28 | 106.4 | 42.08 | 2.62 | 5.04 | 48.8 | 0.02 | 1254 | 1889 | 1080 | 5.66 |
| 458 | gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 0.28 | 106.3 | 14.74 | 9.13 | 0.14 | 32.1 | 7.13 | 1469 | 1877 | 1144 | 5.75 |
| 459 | gpu: greedy@cpu + faces + distance + sphere + depth + raster + direct | 0.28 | 106.1 | 53.10 | 1.52 | 4.91 | 48.8 | 0.02 | 1686 | 2208 | 1021 | 6.15 |
| 460 | gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.28 | 105.8 | 32.37 | 5.04 | 2.64 | 32.2 | 7.12 | 1862 | 2257 | 1128 | 6.45 |
| 461 | gpu: distance + incremental + quadtree + raster + direct | 0.28 | 105.7 | 21.65 | 8.50 | 0.21 | 48.0 | 0.02 | 1739 | 2446 | 1029 | 6.62 |
| 462 | gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 0.28 | 105.6 | 15.60 | 9.17 | 0.20 | 32.1 | 7.13 | 1274 | 2126 | 1024 | 6.00 |
| 463 | gpu: greedy@cpu + faces + distance + refloor + depth + raster + direct | 0.28 | 105.4 | 51.49 | 1.58 | 4.92 | 48.8 | 0.02 | 1686 | 2208 | 1022 | 6.15 |
| 464 | gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.28 | 105.3 | 17.49 | 7.58 | 2.67 | 32.2 | 7.12 | 1342 | 1880 | 1133 | 5.74 |
| 465 | gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.28 | 105.3 | 23.84 | 6.82 | 2.64 | 32.2 | 7.12 | 1350 | 1889 | 1135 | 5.76 |
| 466 | gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.28 | 105.2 | 23.45 | 6.81 | 2.66 | 32.2 | 7.12 | 1342 | 1880 | 1134 | 5.74 |
| 467 | gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 0.28 | 105.1 | 16.19 | 9.19 | 0.21 | 32.1 | 7.13 | 1267 | 2118 | 1024 | 5.98 |
| 468 | gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.28 | 105.0 | 23.51 | 6.86 | 2.65 | 32.2 | 7.12 | 1342 | 1880 | 1137 | 5.75 |
| 469 | gpu: hier + incremental + quadtree + visbuffer + pathtrace | 0.28 | 105.0 | 14.80 | 9.23 | 0.19 | 32.1 | 7.13 | 1267 | 2118 | 1025 | 5.98 |
| 470 | gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.28 | 104.5 | 25.42 | 6.86 | 2.63 | 32.2 | 7.12 | 1350 | 1889 | 1136 | 5.76 |
| 471 | gpu: greedy@cpu + faces + rows + sphere + depth + raster + direct | 0.28 | 104.4 | 46.87 | 2.71 | 4.98 | 48.8 | 0.02 | 1238 | 1872 | 1021 | 5.51 |
| 472 | gpu: hier + incremental + quadtree + gbuffer + pathtrace | 0.28 | 104.3 | 15.47 | 9.29 | 0.22 | 32.1 | 7.13 | 1274 | 2126 | 1024 | 6.00 |
| 473 | gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.28 | 103.8 | 40.21 | 3.05 | 5.04 | 32.1 | 7.13 | 1278 | 1912 | 1084 | 5.71 |
| 474 | gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.28 | 103.8 | 41.19 | 3.14 | 5.03 | 32.1 | 7.13 | 1286 | 1921 | 1081 | 5.72 |
| 475 | gpu: greedy@cpu + faces + distance + rowrun + depth + raster + direct | 0.28 | 103.5 | 51.02 | 1.83 | 4.93 | 48.8 | 0.02 | 1686 | 2208 | 1022 | 6.15 |
| 476 | gpu: rows + sphere + quadtree + raster + direct | 0.27 | 102.9 | 16.55 | 9.38 | 0.22 | 48.0 | 0.02 | 1291 | 2110 | 1025 | 5.97 |
| 477 | gpu: clipmap + incremental + quadtree + raster + direct | 0.27 | 102.8 | 15.83 | 9.43 | 0.13 | 48.0 | 0.02 | 1421 | 1829 | 1142 | 5.66 |
| 478 | gpu: rows + incremental + quadtree + visbuffer + pathtrace | 0.27 | 102.8 | 15.57 | 9.41 | 0.20 | 32.1 | 7.13 | 1330 | 2150 | 1021 | 6.04 |
| 479 | gpu: svo_dag + sphere + composite + raycast + pathtrace | 0.27 | 102.8 | 21.83 | 9.16 | 2.15 | 32.2 | 7.12 | 77 | 176 | 1266 | 2.75 |
| 480 | gpu: rows + incremental + quadtree + gbuffer + pathtrace | 0.27 | 102.6 | 15.99 | 9.43 | 0.21 | 32.1 | 7.13 | 1338 | 2158 | 1026 | 6.06 |
| 481 | gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.27 | 102.5 | 17.73 | 8.01 | 2.72 | 32.2 | 7.12 | 1350 | 1889 | 1135 | 5.76 |
| 482 | gpu: bricks + sphere + quadtree + raster + direct | 0.27 | 102.4 | 13.38 | 9.52 | 0.22 | 48.0 | 0.02 | 1227 | 2078 | 1023 | 5.91 |
| 483 | gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.27 | 102.1 | 18.84 | 7.59 | 2.80 | 32.2 | 7.12 | 1350 | 1889 | 1136 | 5.76 |
| 484 | gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.27 | 101.6 | 17.52 | 8.07 | 2.70 | 32.2 | 7.12 | 1342 | 1880 | 1134 | 5.74 |
| 485 | gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 0.27 | 101.2 | 15.34 | 9.59 | 0.15 | 32.1 | 7.13 | 1469 | 1877 | 1144 | 5.75 |
| 486 | gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 0.27 | 101.1 | 15.98 | 9.60 | 0.13 | 32.1 | 7.13 | 1461 | 1869 | 1141 | 5.73 |
| 487 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.27 | 101.1 | 49.49 | 3.14 | 7.04 | 48.8 | 0.02 | 175 | 271 | 1231 | 2.86 |
| 488 | gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.27 | 100.6 | 38.99 | 3.93 | 5.04 | 32.1 | 7.13 | 1302 | 1933 | 1025 | 5.63 |
| 489 | gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.27 | 100.6 | 37.96 | 3.94 | 5.03 | 32.1 | 7.13 | 1294 | 1925 | 1026 | 5.62 |
| 490 | gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.27 | 100.6 | 23.33 | 7.48 | 2.64 | 32.2 | 7.12 | 1342 | 1880 | 1138 | 5.75 |
| 491 | gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.27 | 100.5 | 19.60 | 7.68 | 2.63 | 32.2 | 7.12 | 1430 | 1933 | 1128 | 5.83 |
| 492 | gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.27 | 100.2 | 23.12 | 7.63 | 2.64 | 32.2 | 7.12 | 1422 | 1925 | 1130 | 5.82 |
| 493 | gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.27 | 100.1 | 23.44 | 7.52 | 2.66 | 32.2 | 7.12 | 1350 | 1889 | 1138 | 5.76 |
| 494 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.27 | 99.6 | 49.85 | 3.47 | 7.06 | 32.1 | 7.13 | 222 | 319 | 1275 | 3.04 |
| 495 | gpu: rows + sphere + quadtree + visbuffer + pathtrace | 0.27 | 99.6 | 16.43 | 9.73 | 0.20 | 32.1 | 7.13 | 1330 | 2150 | 1021 | 6.04 |
| 496 | gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 0.26 | 99.3 | 13.32 | 9.82 | 0.23 | 32.1 | 7.13 | 1266 | 2118 | 1023 | 5.98 |
| 497 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.26 | 99.3 | 50.57 | 3.45 | 7.06 | 32.1 | 7.13 | 214 | 311 | 1278 | 3.03 |
| 498 | gpu: prefix + sphere + quadtree + raster + direct | 0.26 | 99.2 | 15.16 | 9.78 | 0.19 | 48.0 | 0.02 | 1307 | 2122 | 1022 | 5.99 |
| 499 | gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 0.26 | 99.2 | 13.79 | 9.84 | 0.23 | 32.1 | 7.13 | 1274 | 2126 | 1025 | 6.00 |
| 500 | gpu: rows + sphere + quadtree + gbuffer + pathtrace | 0.26 | 99.1 | 16.78 | 9.76 | 0.20 | 32.1 | 7.13 | 1338 | 2158 | 1021 | 6.06 |
| 501 | gpu: svo_dag + rowrun + quadtree + raster + direct | 0.26 | 99.1 | 22.21 | 9.41 | 2.24 | 48.0 | 0.02 | 227 | 361 | 1309 | 3.18 |
| 502 | gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.26 | 99.0 | 35.52 | 4.68 | 4.99 | 32.1 | 7.13 | 1416 | 1637 | 1212 | 5.42 |
| 503 | gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.26 | 98.8 | 38.05 | 4.69 | 5.01 | 32.1 | 7.13 | 1408 | 1628 | 1212 | 5.41 |
| 504 | gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.26 | 98.8 | 24.50 | 7.51 | 2.65 | 32.2 | 7.12 | 1414 | 1921 | 1129 | 5.81 |
| 505 | gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.26 | 98.7 | 24.43 | 7.51 | 2.64 | 32.2 | 7.12 | 1406 | 1912 | 1128 | 5.79 |
| 506 | gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 0.26 | 98.7 | 13.21 | 9.89 | 0.22 | 32.1 | 7.13 | 1274 | 2126 | 1023 | 6.00 |
| 507 | gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.26 | 98.5 | 48.55 | 2.45 | 5.00 | 32.1 | 7.13 | 1726 | 2248 | 1082 | 6.34 |
| 508 | gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 0.26 | 98.4 | 15.37 | 9.92 | 0.23 | 32.1 | 7.13 | 1266 | 2118 | 1023 | 5.98 |
| 509 | gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.26 | 98.1 | 51.05 | 2.46 | 5.05 | 32.1 | 7.13 | 1734 | 2257 | 1081 | 6.36 |
| 510 | gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.26 | 98.0 | 23.23 | 8.05 | 2.61 | 32.2 | 7.12 | 1422 | 1925 | 1129 | 5.82 |
| 511 | gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 0.26 | 97.8 | 16.54 | 9.92 | 0.20 | 32.1 | 7.13 | 1346 | 2162 | 1025 | 6.07 |
| 512 | gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.26 | 97.7 | 34.56 | 5.06 | 6.05 | 32.1 | 7.13 | 368 | 710 | 1124 | 3.49 |
| 513 | gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.26 | 97.4 | 24.11 | 7.74 | 2.66 | 32.2 | 7.12 | 1414 | 1921 | 1158 | 5.86 |
| 514 | gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.26 | 97.3 | 24.36 | 7.74 | 2.65 | 32.2 | 7.12 | 1406 | 1912 | 1134 | 5.80 |
| 515 | gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 0.26 | 97.2 | 17.24 | 9.97 | 0.20 | 32.1 | 7.13 | 1346 | 2162 | 1022 | 6.06 |
| 516 | gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.26 | 97.0 | 23.48 | 8.15 | 2.63 | 32.2 | 7.12 | 1430 | 1933 | 1129 | 5.83 |
| 517 | gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 0.26 | 97.0 | 15.03 | 10.01 | 0.19 | 32.1 | 7.13 | 1354 | 2170 | 1027 | 6.09 |
| 518 | gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.26 | 96.7 | 34.85 | 5.14 | 6.08 | 32.1 | 7.13 | 360 | 568 | 1108 | 3.19 |
| 519 | gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.26 | 96.7 | 41.94 | 4.12 | 5.08 | 32.1 | 7.13 | 1286 | 1921 | 1085 | 5.73 |
| 520 | gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.26 | 96.6 | 48.06 | 2.92 | 4.98 | 32.1 | 7.13 | 1734 | 2257 | 1023 | 6.25 |
| 521 | gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 0.26 | 96.6 | 17.20 | 10.04 | 0.21 | 32.1 | 7.13 | 1354 | 2170 | 1024 | 6.08 |
| 522 | gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.26 | 96.5 | 41.77 | 4.14 | 5.10 | 32.1 | 7.13 | 1278 | 1912 | 1087 | 5.71 |
| 523 | gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.26 | 96.3 | 50.26 | 2.82 | 5.04 | 32.1 | 7.13 | 1726 | 2248 | 1082 | 6.34 |
| 524 | gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 0.26 | 96.2 | 22.16 | 9.82 | 2.20 | 32.2 | 7.12 | 77 | 176 | 1268 | 2.75 |
| 525 | gpu: distance + incremental + composite + raycast + pathtrace | 0.26 | 96.2 | 21.08 | 9.68 | 0.11 | 32.2 | 7.12 | 1588 | 2262 | 984 | 6.18 |
| 526 | gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.26 | 95.8 | 49.76 | 2.92 | 5.03 | 48.7 | 0.02 | 1694 | 2217 | 1080 | 6.28 |
| 527 | gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.25 | 95.6 | 37.16 | 5.19 | 5.19 | 32.1 | 7.13 | 1214 | 1880 | 1097 | 5.67 |
| 528 | gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.25 | 95.5 | 49.48 | 2.99 | 5.02 | 48.8 | 0.02 | 1702 | 2225 | 1080 | 6.29 |
| 529 | gpu: greedy@cpu + faces + distance + incremental + depth + raster + direct | 0.25 | 95.0 | 51.33 | 2.84 | 4.98 | 48.8 | 0.02 | 1686 | 2208 | 1022 | 6.15 |
| 530 | gpu: distance + incremental + beam + composite + raycast + pathtrace | 0.25 | 94.7 | 21.99 | 9.86 | 0.11 | 32.2 | 7.12 | 1588 | 2262 | 988 | 6.19 |
| 531 | gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.25 | 94.0 | 35.28 | 7.40 | 4.63 | 50.4 | 0.02 | 303 | 495 | 1292 | 3.40 |
| 532 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.25 | 93.0 | 48.25 | 4.53 | 7.02 | 48.8 | 0.02 | 175 | 271 | 1230 | 2.86 |
| 533 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.24 | 91.1 | 35.06 | 6.07 | 5.04 | 32.1 | 7.13 | 1408 | 1628 | 1219 | 5.42 |
| 534 | gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.24 | 90.5 | 50.51 | 3.80 | 5.08 | 32.1 | 7.13 | 1726 | 2248 | 1088 | 6.35 |
| 535 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.24 | 89.8 | 38.51 | 6.20 | 5.06 | 32.1 | 7.13 | 1416 | 1637 | 1219 | 5.44 |
| 536 | gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.24 | 89.7 | 18.72 | 9.92 | 2.60 | 32.2 | 7.12 | 1544 | 1790 | 1357 | 5.99 |
| 537 | gpu: tree64 + rowrun + quadtree + raster + direct | 0.24 | 89.2 | 15.59 | 11.05 | 1.29 | 48.0 | 0.02 | 373 | 772 | 1235 | 3.82 |
| 538 | gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.24 | 89.2 | 18.75 | 9.96 | 2.58 | 32.2 | 7.12 | 1536 | 1782 | 1358 | 5.98 |
| 539 | gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.24 | 89.1 | 34.96 | 6.44 | 5.03 | 32.1 | 7.13 | 1416 | 1637 | 1220 | 5.44 |
| 540 | gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.24 | 89.0 | 35.39 | 6.45 | 5.05 | 32.1 | 7.13 | 1408 | 1628 | 1220 | 5.42 |
| 541 | gpu: clipmap + sphere + composite + raycast + pathtrace | 0.24 | 88.6 | 14.94 | 11.04 | 0.06 | 32.2 | 7.12 | 1271 | 1643 | 1093 | 5.21 |
| 542 | gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.24 | 88.5 | 48.93 | 4.20 | 5.09 | 32.1 | 7.13 | 1734 | 2257 | 1093 | 6.38 |
| 543 | gpu: svo_dag + sphere + quadtree + raster + direct | 0.23 | 88.0 | 22.23 | 10.76 | 2.27 | 48.0 | 0.02 | 227 | 361 | 1325 | 3.21 |
| 544 | gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.23 | 87.7 | 30.98 | 6.84 | 5.15 | 32.1 | 7.13 | 1222 | 1889 | 1088 | 5.67 |
| 545 | gpu: svo_dag + rowrun + composite + raycast + pathtrace | 0.23 | 87.6 | 22.41 | 10.91 | 2.20 | 32.2 | 7.12 | 77 | 176 | 1270 | 2.76 |
| 546 | gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.23 | 87.6 | 37.43 | 6.01 | 5.10 | 32.1 | 7.13 | 1222 | 1889 | 1088 | 5.67 |
| 547 | gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.23 | 87.6 | 36.38 | 8.32 | 4.68 | 32.2 | 7.12 | 350 | 543 | 1343 | 3.59 |
| 548 | gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.23 | 87.6 | 38.18 | 6.03 | 5.12 | 32.1 | 7.13 | 1214 | 1880 | 1095 | 5.67 |
| 549 | gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.23 | 87.5 | 39.53 | 6.01 | 5.12 | 32.1 | 7.13 | 1214 | 1880 | 1089 | 5.66 |
| 550 | gpu: tree64 + sphere + composite + raycast + pathtrace | 0.23 | 87.5 | 14.15 | 11.31 | 1.21 | 32.2 | 7.12 | 223 | 501 | 1111 | 3.07 |
| 551 | gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.23 | 87.2 | 35.74 | 8.33 | 4.72 | 32.2 | 7.12 | 342 | 535 | 1348 | 3.59 |
| 552 | gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.23 | 87.1 | 30.04 | 6.76 | 5.25 | 32.1 | 7.13 | 1214 | 1880 | 1087 | 5.65 |
| 553 | gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.23 | 86.9 | 37.24 | 6.10 | 5.11 | 32.1 | 7.13 | 1222 | 1889 | 1094 | 5.68 |
| 554 | gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.23 | 86.7 | 63.75 | 5.14 | 6.07 | 32.1 | 7.13 | 1222 | 1889 | 1099 | 5.69 |
| 555 | gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.23 | 86.6 | 20.99 | 9.59 | 3.71 | 32.2 | 7.12 | 496 | 711 | 1124 | 3.50 |
| 556 | gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.23 | 86.4 | 21.43 | 9.62 | 3.70 | 32.2 | 7.12 | 488 | 814 | 1125 | 3.69 |
| 557 | gpu: svo_dag + incremental + composite + raycast + pathtrace | 0.23 | 86.3 | 21.85 | 11.11 | 2.19 | 32.2 | 7.12 | 77 | 176 | 1273 | 2.76 |
| 558 | gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.23 | 85.4 | 29.85 | 7.15 | 5.22 | 32.1 | 7.13 | 1222 | 1889 | 1098 | 5.69 |
| 559 | gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 0.23 | 85.1 | 14.32 | 11.62 | 1.20 | 32.2 | 7.12 | 223 | 607 | 1139 | 3.33 |
| 560 | gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.23 | 85.0 | 29.73 | 7.24 | 5.22 | 32.1 | 7.13 | 1214 | 1880 | 1098 | 5.67 |
| 561 | gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.23 | 84.7 | 36.07 | 6.84 | 5.04 | 32.1 | 7.13 | 1294 | 1925 | 1023 | 5.61 |
| 562 | gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.23 | 84.5 | 35.75 | 6.68 | 5.09 | 32.1 | 7.13 | 1222 | 1889 | 1095 | 5.68 |
| 563 | gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.23 | 84.4 | 37.19 | 6.64 | 5.14 | 32.1 | 7.13 | 1214 | 1880 | 1097 | 5.67 |
| 564 | gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.22 | 83.9 | 36.74 | 6.94 | 5.05 | 32.1 | 7.13 | 1302 | 1933 | 1023 | 5.63 |
| 565 | gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 0.22 | 83.7 | 23.43 | 11.46 | 2.20 | 32.2 | 7.12 | 77 | 176 | 1273 | 2.76 |
| 566 | gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 0.22 | 83.6 | 22.39 | 11.40 | 2.27 | 32.1 | 7.13 | 267 | 401 | 1324 | 3.29 |
| 567 | gpu: distance + incremental + quadtree + gbuffer + pathtrace | 0.22 | 83.6 | 23.64 | 11.50 | 0.20 | 32.1 | 7.13 | 1786 | 2494 | 1056 | 6.76 |
| 568 | gpu: distance + incremental + quadtree + visbuffer + pathtrace | 0.22 | 83.6 | 23.02 | 11.51 | 0.21 | 32.1 | 7.13 | 1778 | 2486 | 1059 | 6.75 |
| 569 | gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 0.22 | 83.5 | 22.71 | 11.41 | 2.29 | 32.1 | 7.13 | 275 | 410 | 1323 | 3.30 |
| 570 | gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.22 | 83.3 | 39.47 | 6.67 | 5.11 | 32.1 | 7.13 | 1278 | 1912 | 1087 | 5.71 |
| 571 | gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.22 | 83.2 | 38.26 | 6.70 | 5.12 | 32.1 | 7.13 | 1286 | 1921 | 1083 | 5.72 |
| 572 | gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.22 | 82.6 | 36.07 | 7.23 | 5.06 | 32.1 | 7.13 | 1294 | 1925 | 1025 | 5.62 |
| 573 | gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.22 | 82.6 | 36.45 | 7.25 | 5.07 | 32.1 | 7.13 | 1302 | 1933 | 1025 | 5.63 |
| 574 | gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 0.22 | 82.6 | 23.34 | 11.61 | 2.20 | 32.2 | 7.12 | 77 | 176 | 1281 | 2.78 |
| 575 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.22 | 82.6 | 35.75 | 9.12 | 4.71 | 32.2 | 7.12 | 350 | 543 | 1358 | 3.62 |
| 576 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.22 | 82.5 | 35.71 | 9.15 | 4.71 | 32.2 | 7.12 | 342 | 535 | 1304 | 3.50 |
| 577 | gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.22 | 82.2 | 38.29 | 6.94 | 5.11 | 32.1 | 7.13 | 1278 | 1912 | 1094 | 5.73 |
| 578 | gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.22 | 82.2 | 38.36 | 6.94 | 5.12 | 32.1 | 7.13 | 1286 | 1921 | 1093 | 5.74 |
| 579 | gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 0.22 | 81.9 | 15.56 | 11.96 | 0.06 | 32.2 | 7.12 | 1271 | 1643 | 1096 | 5.22 |
| 580 | gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.22 | 81.1 | 29.74 | 9.47 | 2.68 | 32.2 | 7.12 | 1862 | 2257 | 1141 | 6.47 |
| 581 | gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.22 | 81.0 | 29.12 | 9.51 | 2.68 | 32.2 | 7.12 | 1854 | 2248 | 1129 | 6.43 |
| 582 | gpu: greedy@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.22 | 80.8 | 36.76 | 7.68 | 5.01 | 32.1 | 7.13 | 1206 | 1872 | 1029 | 5.52 |
| 583 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.22 | 80.8 | 29.63 | 8.31 | 5.17 | 32.1 | 7.13 | 1206 | 1872 | 1028 | 5.52 |
| 584 | gpu: tree64 + sphere + quadtree + raster + direct | 0.21 | 80.5 | 17.06 | 12.26 | 1.31 | 48.0 | 0.02 | 373 | 674 | 1232 | 3.63 |
| 585 | gpu: greedy@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.21 | 80.3 | 32.01 | 7.87 | 5.03 | 32.1 | 7.13 | 1286 | 1916 | 1027 | 5.60 |
| 586 | gpu: clipmap + sphere + quadtree + raster + direct | 0.21 | 79.9 | 18.37 | 12.24 | 0.15 | 48.0 | 0.02 | 1421 | 1829 | 1152 | 5.68 |
| 587 | gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.21 | 79.4 | 36.36 | 9.74 | 4.70 | 32.2 | 7.12 | 350 | 543 | 1337 | 3.58 |
| 588 | gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.21 | 79.2 | 36.62 | 9.75 | 4.73 | 32.2 | 7.12 | 342 | 535 | 1317 | 3.53 |
| 589 | gpu: greedy@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.21 | 78.3 | 41.55 | 6.72 | 5.04 | 32.1 | 7.13 | 1718 | 2240 | 1027 | 6.22 |
| 590 | gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.21 | 78.3 | 93.95 | 2.43 | 7.88 | 48.8 | 0.02 | 1190 | 1857 | 1090 | 5.61 |
| 591 | gpu: naive@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.21 | 78.2 | 20.48 | 11.77 | 2.63 | 32.2 | 7.12 | 1334 | 1872 | 1136 | 5.73 |
| 592 | gpu: naive@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.21 | 77.7 | 29.40 | 10.51 | 2.60 | 32.2 | 7.12 | 1846 | 2240 | 1129 | 6.42 |
| 593 | gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.21 | 77.5 | 21.11 | 11.28 | 3.70 | 32.2 | 7.12 | 496 | 836 | 1140 | 3.76 |
| 594 | gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.21 | 77.0 | 30.20 | 9.07 | 5.06 | 32.1 | 7.13 | 1416 | 1637 | 1221 | 5.44 |
| 595 | gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.21 | 77.0 | 30.13 | 9.09 | 5.05 | 32.1 | 7.13 | 1408 | 1628 | 1222 | 5.43 |
| 596 | gpu: greedy@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.21 | 76.9 | 33.43 | 8.37 | 5.03 | 32.1 | 7.13 | 1270 | 1904 | 1027 | 5.58 |
| 597 | gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.20 | 76.3 | 21.23 | 11.50 | 3.73 | 32.2 | 7.12 | 488 | 742 | 1138 | 3.58 |
| 598 | gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 0.20 | 76.3 | 16.89 | 12.95 | 1.32 | 32.1 | 7.13 | 413 | 904 | 1252 | 4.11 |
| 599 | gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.20 | 76.2 | 34.44 | 8.62 | 6.08 | 32.1 | 7.13 | 352 | 808 | 1103 | 3.64 |
| 600 | gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 0.20 | 76.1 | 17.32 | 12.96 | 1.36 | 32.1 | 7.13 | 421 | 915 | 1247 | 4.12 |
| 601 | gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.20 | 76.0 | 18.62 | 12.61 | 1.75 | 50.2 | 0.02 | 1398 | 1901 | 1059 | 5.64 |
| 602 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.20 | 76.0 | 16.95 | 12.70 | 1.77 | 50.2 | 0.02 | 1318 | 1857 | 1087 | 5.61 |
| 603 | gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.20 | 76.0 | 18.99 | 12.62 | 1.76 | 50.2 | 0.02 | 1390 | 1893 | 1060 | 5.62 |
| 604 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.20 | 76.0 | 17.01 | 12.71 | 1.78 | 50.2 | 0.02 | 1310 | 1849 | 1078 | 5.57 |
| 605 | gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 0.20 | 75.9 | 17.97 | 12.89 | 0.16 | 32.1 | 7.13 | 1461 | 1869 | 1164 | 5.78 |
| 606 | gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.20 | 75.7 | 18.47 | 12.68 | 1.78 | 50.2 | 0.02 | 1390 | 1893 | 1060 | 5.62 |
| 607 | gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.20 | 75.7 | 18.53 | 12.67 | 1.75 | 50.2 | 0.02 | 1398 | 1901 | 1060 | 5.64 |
| 608 | gpu: naive@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.20 | 75.6 | 21.58 | 12.35 | 2.67 | 32.2 | 7.12 | 1414 | 1916 | 1130 | 5.80 |
| 609 | gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.20 | 75.6 | 17.33 | 12.77 | 1.77 | 50.2 | 0.02 | 1310 | 1849 | 1096 | 5.61 |
| 610 | gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 0.20 | 75.6 | 17.66 | 12.94 | 0.16 | 32.1 | 7.13 | 1469 | 1877 | 1162 | 5.79 |
| 611 | gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.20 | 75.6 | 18.65 | 12.70 | 1.77 | 50.2 | 0.02 | 1310 | 1849 | 1075 | 5.57 |
| 612 | gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.20 | 75.6 | 17.01 | 12.76 | 1.78 | 50.2 | 0.02 | 1318 | 1857 | 1104 | 5.64 |
| 613 | gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.20 | 75.6 | 18.39 | 12.69 | 1.75 | 50.2 | 0.02 | 1318 | 1857 | 1113 | 5.66 |
| 614 | gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.20 | 75.5 | 19.03 | 12.70 | 1.76 | 50.2 | 0.02 | 1382 | 1889 | 1066 | 5.63 |
| 615 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.20 | 75.4 | 49.15 | 7.58 | 7.16 | 32.1 | 7.13 | 222 | 319 | 1284 | 3.05 |
| 616 | gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.20 | 75.4 | 18.48 | 12.71 | 1.75 | 50.2 | 0.02 | 1374 | 1881 | 1105 | 5.69 |
| 617 | gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.20 | 75.3 | 34.62 | 8.79 | 6.12 | 32.1 | 7.13 | 368 | 593 | 1097 | 3.22 |
| 618 | gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.20 | 75.3 | 16.78 | 12.69 | 2.80 | 50.2 | 0.02 | 464 | 695 | 1092 | 3.40 |
| 619 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.20 | 75.3 | 49.71 | 7.56 | 7.16 | 32.1 | 7.13 | 214 | 311 | 1283 | 3.04 |
| 620 | gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.20 | 75.2 | 18.74 | 12.76 | 1.75 | 50.2 | 0.02 | 1318 | 1857 | 1095 | 5.62 |
| 621 | gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.20 | 75.2 | 34.46 | 8.81 | 6.12 | 32.1 | 7.13 | 360 | 688 | 1100 | 3.41 |
| 622 | gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.20 | 75.1 | 19.87 | 12.75 | 1.75 | 50.2 | 0.02 | 1374 | 1881 | 1110 | 5.70 |
| 623 | gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.20 | 75.1 | 19.14 | 12.75 | 1.75 | 50.2 | 0.02 | 1382 | 1889 | 1111 | 5.71 |
| 624 | gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.20 | 75.0 | 18.67 | 12.82 | 1.75 | 50.2 | 0.02 | 1310 | 1849 | 1064 | 5.55 |
| 625 | gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.20 | 75.0 | 16.85 | 12.74 | 2.81 | 50.2 | 0.02 | 456 | 659 | 1173 | 3.49 |
| 626 | gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.20 | 74.7 | 18.18 | 12.88 | 1.69 | 50.2 | 0.02 | 1512 | 1759 | 1279 | 5.78 |
| 627 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.20 | 74.7 | 17.07 | 12.79 | 2.81 | 50.2 | 0.02 | 464 | 681 | 1160 | 3.50 |
| 628 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.20 | 74.7 | 17.02 | 12.80 | 2.81 | 50.2 | 0.02 | 456 | 748 | 1170 | 3.65 |
| 629 | gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.20 | 74.6 | 18.43 | 12.89 | 1.70 | 50.2 | 0.02 | 1505 | 1750 | 1283 | 5.78 |
| 630 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.20 | 74.5 | 18.54 | 12.93 | 1.73 | 50.2 | 0.02 | 1512 | 1759 | 1287 | 5.80 |
| 631 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.20 | 74.4 | 17.96 | 12.94 | 1.72 | 50.2 | 0.02 | 1505 | 1750 | 1288 | 5.79 |
| 632 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.20 | 74.1 | 19.97 | 12.84 | 2.73 | 32.2 | 7.12 | 1334 | 1872 | 1134 | 5.73 |
| 633 | gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.20 | 74.0 | 17.37 | 13.05 | 1.77 | 50.2 | 0.02 | 1318 | 1857 | 1072 | 5.58 |
| 634 | gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.20 | 74.0 | 19.04 | 12.99 | 1.73 | 50.2 | 0.02 | 1318 | 1857 | 1082 | 5.60 |
| 635 | gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.20 | 74.0 | 17.23 | 13.07 | 1.77 | 50.2 | 0.02 | 1310 | 1849 | 1090 | 5.60 |
| 636 | gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.20 | 73.9 | 25.48 | 12.64 | 1.72 | 50.2 | 0.02 | 1822 | 2217 | 1116 | 6.35 |
| 637 | gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.20 | 73.8 | 17.14 | 12.99 | 2.82 | 50.2 | 0.02 | 464 | 669 | 1092 | 3.35 |
| 638 | gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.20 | 73.7 | 18.51 | 13.04 | 1.76 | 50.2 | 0.02 | 1390 | 1893 | 1059 | 5.62 |
| 639 | gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.20 | 73.6 | 25.59 | 12.64 | 1.74 | 50.2 | 0.02 | 1830 | 2225 | 1075 | 6.28 |
| 640 | gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.20 | 73.6 | 18.91 | 13.04 | 1.77 | 50.2 | 0.02 | 1398 | 1901 | 1059 | 5.64 |
| 641 | gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.20 | 73.6 | 16.68 | 13.03 | 2.82 | 50.2 | 0.02 | 456 | 878 | 1168 | 3.90 |
| 642 | gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.20 | 73.6 | 18.95 | 13.08 | 1.74 | 50.2 | 0.02 | 1310 | 1849 | 1065 | 5.55 |
| 643 | gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.20 | 73.5 | 19.21 | 13.05 | 1.75 | 50.2 | 0.02 | 1382 | 1889 | 1084 | 5.66 |
| 644 | gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.20 | 73.5 | 18.33 | 13.11 | 1.70 | 50.2 | 0.02 | 1512 | 1759 | 1288 | 5.80 |
| 645 | gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.20 | 73.5 | 28.80 | 12.68 | 1.72 | 50.2 | 0.02 | 1830 | 2225 | 1091 | 6.32 |
| 646 | gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.20 | 73.5 | 19.56 | 13.06 | 1.76 | 50.2 | 0.02 | 1374 | 1881 | 1116 | 5.71 |
| 647 | gpu: naive@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.20 | 73.5 | 22.94 | 12.70 | 2.66 | 32.2 | 7.12 | 1398 | 1904 | 1129 | 5.78 |
| 648 | gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.20 | 73.4 | 25.69 | 12.71 | 1.71 | 50.2 | 0.02 | 1830 | 2225 | 1109 | 6.35 |
| 649 | gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.20 | 73.4 | 25.76 | 12.72 | 1.71 | 50.2 | 0.02 | 1822 | 2217 | 1103 | 6.32 |
| 650 | gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.20 | 73.4 | 18.26 | 13.12 | 1.69 | 50.2 | 0.02 | 1505 | 1750 | 1289 | 5.79 |
| 651 | gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.20 | 73.3 | 25.78 | 12.73 | 1.73 | 50.2 | 0.02 | 1822 | 2217 | 1115 | 6.35 |
| 652 | gpu: greedy@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.20 | 73.1 | 41.35 | 8.01 | 5.05 | 32.1 | 7.13 | 1718 | 2240 | 1027 | 6.22 |
| 653 | gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.19 | 72.3 | 18.61 | 13.33 | 1.72 | 50.2 | 0.02 | 1512 | 1759 | 1283 | 5.79 |
| 654 | gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.19 | 72.3 | 18.23 | 13.34 | 1.72 | 50.2 | 0.02 | 1505 | 1750 | 1283 | 5.78 |
| 655 | gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.19 | 72.3 | 17.20 | 13.31 | 2.79 | 50.2 | 0.02 | 464 | 798 | 1162 | 3.73 |
| 656 | gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.19 | 72.2 | 16.94 | 13.33 | 2.82 | 50.2 | 0.02 | 456 | 688 | 1169 | 3.54 |
| 657 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.19 | 71.9 | 49.55 | 8.33 | 7.19 | 32.1 | 7.13 | 222 | 319 | 1289 | 3.06 |
| 658 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.19 | 71.9 | 48.88 | 8.30 | 7.21 | 32.1 | 7.13 | 214 | 311 | 1292 | 3.05 |
| 659 | gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 0.19 | 71.3 | 23.89 | 13.53 | 2.30 | 32.1 | 7.13 | 275 | 410 | 1346 | 3.34 |
| 660 | gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 0.19 | 71.0 | 23.58 | 13.58 | 2.31 | 32.1 | 7.13 | 267 | 401 | 1350 | 3.33 |
| 661 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.19 | 70.9 | 28.43 | 12.74 | 3.77 | 50.2 | 0.02 | 311 | 503 | 1313 | 3.46 |
| 662 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.19 | 70.9 | 28.40 | 12.73 | 3.80 | 50.2 | 0.02 | 318 | 512 | 1314 | 3.48 |
| 663 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.19 | 70.7 | 28.89 | 12.77 | 3.78 | 50.2 | 0.02 | 311 | 503 | 1321 | 3.47 |
| 664 | gpu: naive@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.19 | 70.6 | 30.36 | 12.40 | 2.65 | 32.2 | 7.12 | 1846 | 2240 | 1130 | 6.42 |
| 665 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.19 | 70.6 | 28.98 | 12.77 | 3.80 | 50.2 | 0.02 | 318 | 512 | 1317 | 3.48 |
| 666 | gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.19 | 70.5 | 42.55 | 8.67 | 5.13 | 32.1 | 7.13 | 1734 | 2257 | 1091 | 6.38 |
| 667 | gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.19 | 70.3 | 41.96 | 8.67 | 5.15 | 32.1 | 7.13 | 1726 | 2248 | 1093 | 6.36 |
| 668 | gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.19 | 70.1 | 29.88 | 11.00 | 4.98 | 32.1 | 7.13 | 1400 | 1620 | 1168 | 5.31 |
| 669 | gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 0.19 | 69.6 | 23.63 | 13.87 | 2.32 | 32.1 | 7.13 | 275 | 410 | 1352 | 3.36 |
| 670 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.19 | 69.4 | 49.45 | 8.96 | 7.18 | 32.1 | 7.13 | 222 | 319 | 1296 | 3.08 |
| 671 | gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 0.18 | 69.3 | 17.89 | 14.30 | 1.22 | 32.2 | 7.12 | 223 | 546 | 1156 | 3.24 |
| 672 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.18 | 69.3 | 49.69 | 8.95 | 7.21 | 32.1 | 7.13 | 214 | 311 | 1297 | 3.06 |
| 673 | gpu: naive@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.18 | 69.1 | 21.37 | 13.21 | 3.70 | 32.2 | 7.12 | 480 | 695 | 1128 | 3.47 |
| 674 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.18 | 69.0 | 29.65 | 13.17 | 3.80 | 50.2 | 0.02 | 318 | 512 | 1314 | 3.48 |
| 675 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.18 | 69.0 | 28.44 | 13.19 | 3.78 | 50.2 | 0.02 | 311 | 503 | 1315 | 3.46 |
| 676 | gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.18 | 68.8 | 18.63 | 14.10 | 1.78 | 50.2 | 0.02 | 1318 | 1857 | 1099 | 5.63 |
| 677 | gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.18 | 68.7 | 18.63 | 14.11 | 1.79 | 50.2 | 0.02 | 1310 | 1849 | 1091 | 5.60 |
| 678 | gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 0.18 | 68.7 | 23.24 | 14.06 | 2.31 | 32.1 | 7.13 | 267 | 401 | 1348 | 3.33 |
| 679 | gpu: tree64 + rowrun + composite + raycast + pathtrace | 0.18 | 68.6 | 18.31 | 14.47 | 1.23 | 32.2 | 7.12 | 223 | 502 | 1129 | 3.11 |
| 680 | gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.18 | 68.2 | 19.96 | 14.15 | 1.78 | 50.2 | 0.02 | 1318 | 1857 | 1090 | 5.61 |
| 681 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.18 | 68.2 | 34.59 | 10.47 | 6.14 | 32.1 | 7.13 | 368 | 597 | 1113 | 3.25 |
| 682 | gpu: distance + sphere + quadtree + raster + pathtrace | 0.18 | 68.1 | 21.85 | 14.36 | 0.22 | 32.1 | 7.13 | 1770 | 2477 | 1042 | 6.70 |
| 683 | gpu: greedy@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.18 | 68.1 | 32.21 | 11.01 | 5.03 | 32.1 | 7.13 | 1270 | 1904 | 1027 | 5.58 |
| 684 | gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.18 | 67.6 | 20.30 | 14.30 | 1.80 | 50.2 | 0.02 | 1310 | 1849 | 1065 | 5.55 |
| 685 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.18 | 67.5 | 34.51 | 10.68 | 6.14 | 32.1 | 7.13 | 360 | 579 | 1091 | 3.18 |
| 686 | gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.18 | 67.5 | 20.93 | 14.31 | 1.79 | 50.2 | 0.02 | 1398 | 1901 | 1060 | 5.64 |
| 687 | gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.18 | 67.4 | 20.30 | 14.32 | 1.78 | 50.2 | 0.02 | 1390 | 1893 | 1059 | 5.62 |
| 688 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.18 | 67.4 | 30.27 | 13.62 | 3.80 | 50.2 | 0.02 | 318 | 512 | 1323 | 3.49 |
| 689 | gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.18 | 67.3 | 28.13 | 14.16 | 1.75 | 32.2 | 7.12 | 1862 | 2257 | 1098 | 6.39 |
| 690 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.18 | 67.3 | 29.13 | 13.62 | 3.81 | 50.2 | 0.02 | 311 | 503 | 1319 | 3.47 |
| 691 | gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.18 | 67.2 | 27.99 | 14.16 | 1.76 | 32.2 | 7.12 | 1854 | 2248 | 1106 | 6.39 |
| 692 | gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.18 | 67.2 | 20.85 | 14.35 | 1.76 | 50.2 | 0.02 | 1382 | 1889 | 1119 | 5.73 |
| 693 | gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.18 | 67.2 | 20.46 | 14.37 | 1.76 | 50.2 | 0.02 | 1374 | 1881 | 1092 | 5.66 |
| 694 | gpu: svo_dag + incremental + quadtree + raster + direct | 0.18 | 67.1 | 24.27 | 14.41 | 2.32 | 48.0 | 0.02 | 227 | 361 | 1354 | 3.27 |
| 695 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.18 | 66.8 | 47.86 | 9.77 | 7.07 | 32.1 | 7.13 | 206 | 302 | 1235 | 2.93 |
| 696 | gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.18 | 66.6 | 20.25 | 14.49 | 1.81 | 32.2 | 7.12 | 1422 | 1925 | 1060 | 5.68 |
| 697 | gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.18 | 66.4 | 20.15 | 14.53 | 1.80 | 32.2 | 7.12 | 1430 | 1933 | 1061 | 5.70 |
| 698 | gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.18 | 66.4 | 20.56 | 14.55 | 1.78 | 32.2 | 7.12 | 1342 | 1880 | 1078 | 5.63 |
| 699 | gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.18 | 66.3 | 20.88 | 14.59 | 1.79 | 32.2 | 7.12 | 1350 | 1889 | 1104 | 5.70 |
| 700 | gpu: greedy@cpu + rayquery + raycast + direct | 0.18 | 65.7 | 126.32 | 1.59 | 10.48 | 56.7 | 0.01 | 778 | 2526 | 2502 | 9.58 |
| 701 | gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.18 | 65.6 | 28.75 | 14.56 | 1.76 | 32.2 | 7.12 | 1854 | 2248 | 1099 | 6.38 |
| 702 | gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.17 | 65.6 | 31.70 | 12.01 | 5.04 | 32.1 | 7.13 | 1286 | 1916 | 1027 | 5.61 |
| 703 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.17 | 65.6 | 19.08 | 14.81 | 1.81 | 32.2 | 7.12 | 1342 | 1880 | 1106 | 5.69 |
| 704 | gpu: points@cpu + splat + tree64 + refloor + direct | 0.17 | 65.6 | 86.71 | 3.32 | 1.18 | 56.0 | 0.01 | 886 | 1256 | 1727 | 5.68 |
| 705 | gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.17 | 65.3 | 28.26 | 14.67 | 1.76 | 50.2 | 0.02 | 1830 | 2225 | 1076 | 6.29 |
| 706 | gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.17 | 65.3 | 20.49 | 14.79 | 1.80 | 32.2 | 7.12 | 1406 | 1912 | 1105 | 5.75 |
| 707 | gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.17 | 65.3 | 28.74 | 14.65 | 1.77 | 32.2 | 7.12 | 1862 | 2257 | 1082 | 6.36 |
| 708 | gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.17 | 65.2 | 28.56 | 14.69 | 1.76 | 50.2 | 0.02 | 1822 | 2217 | 1094 | 6.31 |
| 709 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.17 | 65.0 | 19.09 | 14.95 | 1.81 | 32.2 | 7.12 | 1350 | 1889 | 1075 | 5.64 |
| 710 | gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.17 | 65.0 | 20.29 | 14.88 | 1.79 | 32.2 | 7.12 | 1414 | 1921 | 1095 | 5.74 |
| 711 | gpu: points@cpu + splat + bricks + rowrun + direct | 0.17 | 64.9 | 87.49 | 3.42 | 0.09 | 56.0 | 0.01 | 1740 | 2194 | 1613 | 7.25 |
| 712 | gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.17 | 64.8 | 19.03 | 15.04 | 2.80 | 32.2 | 7.12 | 496 | 710 | 1109 | 3.47 |
| 713 | gpu: points@cpu + splat + tree64 + rowrun + direct | 0.17 | 64.7 | 99.80 | 3.47 | 1.18 | 56.0 | 0.01 | 886 | 1272 | 1756 | 5.77 |
| 714 | gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.17 | 64.5 | 19.06 | 15.07 | 2.85 | 32.2 | 7.12 | 488 | 932 | 1166 | 4.00 |
| 715 | gpu: points@cpu + splat + tree64 + incremental + direct | 0.17 | 64.4 | 89.08 | 3.72 | 1.19 | 56.0 | 0.01 | 886 | 1270 | 1769 | 5.79 |
| 716 | gpu: points@cpu + splat + bricks + refloor + direct | 0.17 | 64.2 | 88.25 | 3.36 | 0.10 | 56.0 | 0.01 | 1740 | 2194 | 1613 | 7.25 |
| 717 | gpu: points@cpu + splat + tree64 + sphere + direct | 0.17 | 63.8 | 84.57 | 3.96 | 1.19 | 56.0 | 0.01 | 886 | 1262 | 1746 | 5.73 |
| 718 | gpu: points@cpu + splat + clipmap + refloor + direct | 0.17 | 63.6 | 89.92 | 3.47 | 0.03 | 56.0 | 0.01 | 1934 | 2647 | 1729 | 8.33 |
| 719 | gpu: points@cpu + splat + clipmap + incremental + direct | 0.17 | 63.2 | 91.91 | 3.65 | 0.03 | 56.0 | 0.01 | 1934 | 2647 | 1728 | 8.33 |
| 720 | gpu: points@cpu + splat + clipmap + rowrun + direct | 0.17 | 62.9 | 89.53 | 3.60 | 0.03 | 56.0 | 0.01 | 1934 | 2647 | 1729 | 8.33 |
| 721 | gpu: greedy@cpu + rayquery + raycast + pathtrace | 0.17 | 62.9 | 130.37 | 2.07 | 11.18 | 32.2 | 7.12 | 809 | 2558 | 2538 | 9.70 |
| 722 | gpu: points@cpu + splat + bricks + sphere + direct | 0.17 | 62.8 | 90.79 | 3.72 | 0.09 | 56.0 | 0.01 | 1740 | 2194 | 1614 | 7.25 |
| 723 | gpu: points@cpu + splat + hier + refloor + direct | 0.17 | 62.3 | 90.06 | 3.36 | 0.09 | 56.0 | 0.01 | 1740 | 2178 | 1619 | 7.23 |
| 724 | gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.17 | 62.3 | 20.92 | 15.55 | 1.81 | 32.2 | 7.12 | 1422 | 1925 | 1059 | 5.68 |
| 725 | gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.17 | 62.3 | 30.33 | 15.47 | 1.74 | 32.2 | 7.12 | 1854 | 2248 | 1102 | 6.38 |
| 726 | gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.17 | 62.3 | 21.41 | 15.55 | 1.81 | 32.2 | 7.12 | 1430 | 1933 | 1059 | 5.70 |
| 727 | gpu: points@cpu + splat + prefix + rowrun + direct | 0.17 | 62.1 | 97.18 | 3.36 | 0.10 | 55.9 | 0.01 | 1820 | 2263 | 1617 | 7.39 |
| 728 | gpu: points@cpu + splat + prefix + refloor + direct | 0.17 | 62.0 | 92.76 | 3.36 | 0.10 | 55.9 | 0.01 | 1820 | 2263 | 1617 | 7.39 |
| 729 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.17 | 61.9 | 30.24 | 15.19 | 3.81 | 32.2 | 7.12 | 342 | 535 | 1316 | 3.52 |
| 730 | gpu: points@cpu + splat + hier + rowrun + direct | 0.16 | 61.7 | 92.38 | 3.42 | 0.09 | 56.0 | 0.01 | 1740 | 2178 | 1620 | 7.23 |
| 731 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.16 | 61.7 | 29.53 | 15.23 | 3.83 | 32.2 | 7.12 | 350 | 543 | 1317 | 3.54 |
| 732 | gpu: points@cpu + splat + clipmap + sphere + direct | 0.16 | 61.7 | 90.50 | 3.98 | 0.03 | 56.0 | 0.01 | 1934 | 2647 | 1728 | 8.33 |
| 733 | gpu: points@cpu + splat + rows + rowrun + direct | 0.16 | 61.5 | 107.91 | 3.39 | 0.10 | 56.0 | 0.01 | 1804 | 2246 | 1617 | 7.36 |
| 734 | gpu: points@cpu + splat + svo_dag + rowrun + direct | 0.16 | 61.4 | 94.94 | 3.42 | 2.16 | 55.9 | 0.01 | 740 | 1135 | 1802 | 5.59 |
| 735 | gpu: points@cpu + splat + prefix + sphere + direct | 0.16 | 61.3 | 92.66 | 3.77 | 0.10 | 56.0 | 0.01 | 1820 | 2263 | 1619 | 7.39 |
| 736 | gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.16 | 61.2 | 21.84 | 15.82 | 1.79 | 32.2 | 7.12 | 1414 | 1921 | 1093 | 5.74 |
| 737 | gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.16 | 61.2 | 21.49 | 15.83 | 1.79 | 32.2 | 7.12 | 1406 | 1912 | 1101 | 5.74 |
| 738 | gpu: points@cpu + splat + hier + sphere + direct | 0.16 | 61.2 | 93.18 | 3.65 | 0.10 | 56.0 | 0.01 | 1740 | 2178 | 1619 | 7.23 |
| 739 | gpu: points@cpu + splat + svo_dag + refloor + direct | 0.16 | 61.2 | 104.00 | 3.40 | 2.17 | 55.9 | 0.01 | 740 | 1135 | 1802 | 5.59 |
| 740 | gpu: points@cpu + splat + rows + refloor + direct | 0.16 | 61.1 | 95.82 | 3.36 | 0.10 | 56.0 | 0.01 | 1804 | 2246 | 1618 | 7.36 |
| 741 | gpu: points@cpu + splat + rows + sphere + direct | 0.16 | 60.8 | 93.27 | 3.70 | 0.10 | 56.0 | 0.01 | 1804 | 2246 | 1618 | 7.36 |
| 742 | gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.16 | 60.7 | 30.52 | 15.89 | 1.78 | 32.2 | 7.12 | 1862 | 2257 | 1095 | 6.38 |
| 743 | gpu: greedy@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.16 | 60.6 | 32.45 | 13.54 | 5.09 | 32.1 | 7.13 | 1270 | 1904 | 1027 | 5.58 |
| 744 | gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.16 | 60.4 | 35.53 | 14.65 | 4.69 | 32.2 | 7.12 | 334 | 526 | 1274 | 3.43 |
| 745 | gpu: points@cpu + splat + svo_dag + sphere + direct | 0.16 | 60.2 | 101.23 | 3.88 | 2.16 | 56.0 | 0.01 | 740 | 1135 | 1801 | 5.59 |
| 746 | gpu: greedy@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.16 | 60.0 | 36.86 | 12.65 | 5.06 | 32.1 | 7.13 | 1718 | 2240 | 1027 | 6.22 |
| 747 | gpu: points@cpu + splat + svo_dag + incremental + direct | 0.16 | 59.8 | 98.62 | 4.22 | 2.16 | 55.9 | 0.01 | 740 | 1135 | 1841 | 5.67 |
| 748 | gpu: points@cpu + splat + tree64 + refloor + pathtrace | 0.16 | 59.7 | 72.07 | 5.63 | 1.20 | 32.2 | 7.12 | 918 | 1319 | 1761 | 5.87 |
| 749 | gpu: naive@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.16 | 59.7 | 23.40 | 16.24 | 2.65 | 32.2 | 7.12 | 1528 | 1773 | 1307 | 5.87 |
| 750 | gpu: points@cpu + splat + prefix + incremental + direct | 0.16 | 59.6 | 83.52 | 4.77 | 0.10 | 55.9 | 0.01 | 1820 | 2263 | 1618 | 7.39 |
| 751 | gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.16 | 59.4 | 20.91 | 16.37 | 1.74 | 32.2 | 7.12 | 1536 | 1782 | 1284 | 5.84 |
| 752 | gpu: prefix + refloor + quadtree + raster + pathtrace | 0.16 | 59.4 | 22.63 | 16.56 | 0.27 | 32.1 | 7.13 | 1339 | 2154 | 1052 | 6.10 |
| 753 | gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.16 | 59.4 | 21.53 | 16.37 | 1.74 | 32.2 | 7.12 | 1544 | 1790 | 1284 | 5.85 |
| 754 | gpu: hier + refloor + quadtree + raster + pathtrace | 0.16 | 59.3 | 22.86 | 16.61 | 0.26 | 32.1 | 7.13 | 1259 | 2110 | 1052 | 6.02 |
| 755 | gpu: distance + refloor + quadtree + raster + pathtrace | 0.16 | 59.1 | 23.30 | 16.59 | 0.22 | 32.1 | 7.13 | 1770 | 2477 | 1050 | 6.72 |
| 756 | gpu: naive@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.16 | 59.1 | 23.29 | 16.33 | 2.73 | 32.2 | 7.12 | 1398 | 1904 | 1130 | 5.78 |
| 757 | gpu: points@cpu + splat + bricks + incremental + direct | 0.16 | 59.0 | 89.75 | 4.74 | 0.10 | 56.0 | 0.01 | 1740 | 2194 | 1614 | 7.25 |
| 758 | gpu: points@cpu + splat + distance + rowrun + direct | 0.16 | 58.5 | 100.45 | 3.40 | 0.10 | 56.0 | 0.01 | 2252 | 2720 | 1618 | 8.26 |
| 759 | gpu: points@cpu + splat + distance + sphere + direct | 0.16 | 58.4 | 104.42 | 3.31 | 0.10 | 56.0 | 0.01 | 2252 | 2720 | 1617 | 8.26 |
| 760 | gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.16 | 58.3 | 20.87 | 16.81 | 2.85 | 32.2 | 7.12 | 496 | 727 | 1114 | 3.51 |
| 761 | gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.16 | 58.3 | 20.78 | 16.81 | 2.86 | 32.2 | 7.12 | 488 | 698 | 1171 | 3.56 |
| 762 | gpu: points@cpu + splat + distance + refloor + direct | 0.16 | 58.2 | 104.77 | 3.36 | 0.09 | 56.0 | 0.01 | 2252 | 2720 | 1618 | 8.26 |
| 763 | gpu: greedy@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.15 | 58.0 | 31.61 | 14.50 | 5.12 | 32.1 | 7.13 | 1206 | 1872 | 1033 | 5.53 |
| 764 | gpu: points@cpu + splat + rows + incremental + direct | 0.15 | 58.0 | 90.01 | 4.96 | 0.10 | 56.0 | 0.01 | 1804 | 2246 | 1618 | 7.36 |
| 765 | gpu: points@cpu + splat + hier + incremental + direct | 0.15 | 58.0 | 86.61 | 4.83 | 0.10 | 56.0 | 0.01 | 1740 | 2178 | 1621 | 7.24 |
| 766 | gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.15 | 57.9 | 21.19 | 16.83 | 1.81 | 32.2 | 7.12 | 1342 | 1880 | 1098 | 5.67 |
| 767 | gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.15 | 57.8 | 21.33 | 16.86 | 1.83 | 32.2 | 7.12 | 1350 | 1889 | 1116 | 5.72 |
| 768 | gpu: points@cpu + splat + prefix + refloor + pathtrace | 0.15 | 57.7 | 78.45 | 5.14 | 0.10 | 32.2 | 7.12 | 1851 | 2295 | 1618 | 7.45 |
| 769 | gpu: points@cpu + splat + bricks + refloor + pathtrace | 0.15 | 57.7 | 79.72 | 5.95 | 0.10 | 32.2 | 7.12 | 1771 | 2226 | 1619 | 7.32 |
| 770 | gpu: greedy@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.15 | 57.4 | 29.99 | 14.76 | 5.25 | 32.1 | 7.13 | 1206 | 1872 | 1032 | 5.53 |
| 771 | gpu: points@cpu + splat + hier + refloor + pathtrace | 0.15 | 57.4 | 81.16 | 5.23 | 0.10 | 32.2 | 7.12 | 1771 | 2210 | 1620 | 7.29 |
| 772 | gpu: greedy@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.15 | 57.1 | 31.33 | 14.88 | 5.11 | 32.1 | 7.13 | 1286 | 1916 | 1027 | 5.61 |
| 773 | gpu: points@cpu + splat + rows + refloor + pathtrace | 0.15 | 56.4 | 75.78 | 5.38 | 0.10 | 32.2 | 7.12 | 1835 | 2278 | 1619 | 7.42 |
| 774 | gpu: greedy@cpu + rayquery + raycast + bricks + refloor + pathtrace | 0.15 | 56.3 | 133.42 | 4.34 | 11.07 | 32.2 | 7.12 | 1834 | 3582 | 2483 | 11.55 |
| 775 | gpu: greedy@cpu + rayquery + raycast + tree64 + refloor + pathtrace | 0.15 | 56.2 | 85.62 | 4.66 | 12.26 | 32.2 | 7.12 | 981 | 2748 | 2512 | 10.02 |
| 776 | gpu: points@cpu + splat + svo_dag + refloor + pathtrace | 0.15 | 56.1 | 75.70 | 5.80 | 2.18 | 32.2 | 7.12 | 772 | 1166 | 1844 | 5.73 |
| 777 | gpu: points@cpu + splat + tree64 + incremental + pathtrace | 0.15 | 55.9 | 68.36 | 7.35 | 1.21 | 32.2 | 7.12 | 918 | 1304 | 1753 | 5.82 |
| 778 | gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 0.15 | 55.8 | 21.82 | 17.76 | 1.35 | 32.1 | 7.13 | 421 | 683 | 1252 | 3.68 |
| 779 | gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.15 | 55.7 | 34.66 | 14.55 | 6.12 | 32.1 | 7.13 | 352 | 590 | 1092 | 3.20 |
| 780 | gpu: points@cpu + splat + prefix + rowrun + pathtrace | 0.15 | 55.6 | 79.50 | 6.26 | 0.10 | 32.2 | 7.12 | 1851 | 2295 | 1619 | 7.45 |
| 781 | gpu: points@cpu + splat + clipmap + refloor + pathtrace | 0.15 | 55.5 | 79.74 | 6.88 | 0.04 | 32.2 | 7.12 | 1966 | 2679 | 1731 | 8.40 |
| 782 | gpu: greedy@cpu + rayquery + raycast + prefix + refloor + pathtrace | 0.15 | 55.5 | 133.58 | 4.09 | 10.92 | 32.2 | 7.12 | 1914 | 3663 | 2538 | 11.81 |
| 783 | gpu: greedy@cpu + rayquery + raycast + hier + refloor + pathtrace | 0.15 | 55.4 | 136.19 | 4.01 | 11.04 | 32.2 | 7.12 | 1834 | 3583 | 2534 | 11.65 |
| 784 | gpu: points@cpu + splat + distance + sphere + pathtrace | 0.15 | 55.3 | 85.34 | 4.77 | 0.10 | 32.2 | 7.12 | 2283 | 2752 | 1619 | 8.32 |
| 785 | gpu: bricks + refloor + quadtree + raster + pathtrace | 0.15 | 55.1 | 24.07 | 17.90 | 0.27 | 32.1 | 7.13 | 1258 | 2109 | 1032 | 5.98 |
| 786 | gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 0.15 | 55.1 | 22.61 | 17.99 | 1.35 | 32.1 | 7.13 | 413 | 679 | 1253 | 3.68 |
| 787 | gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.15 | 55.1 | 22.59 | 17.69 | 1.83 | 32.2 | 7.12 | 1350 | 1889 | 1086 | 5.67 |
| 788 | gpu: points@cpu + splat + bricks + rowrun + pathtrace | 0.15 | 54.9 | 72.00 | 7.35 | 0.10 | 32.2 | 7.12 | 1771 | 2226 | 1615 | 7.31 |
| 789 | gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.15 | 54.9 | 22.90 | 17.74 | 1.83 | 32.2 | 7.12 | 1350 | 1889 | 1094 | 5.68 |
| 790 | gpu: rows + refloor + quadtree + raster + pathtrace | 0.15 | 54.8 | 24.43 | 17.96 | 0.25 | 32.1 | 7.13 | 1322 | 2141 | 1051 | 6.08 |
| 791 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.15 | 54.8 | 22.72 | 17.78 | 1.75 | 32.2 | 7.12 | 1536 | 1782 | 1294 | 5.86 |
| 792 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.15 | 54.7 | 22.42 | 17.82 | 1.77 | 32.2 | 7.12 | 1544 | 1790 | 1294 | 5.87 |
| 793 | gpu: greedy@cpu + rayquery + raycast + tree64 + incremental + pathtrace | 0.15 | 54.7 | 79.63 | 6.24 | 11.76 | 32.2 | 7.12 | 981 | 2723 | 2510 | 9.97 |
| 794 | gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.15 | 54.7 | 22.59 | 17.81 | 1.81 | 32.2 | 7.12 | 1342 | 1880 | 1065 | 5.61 |
| 795 | gpu: points@cpu + splat + rows + rowrun + pathtrace | 0.15 | 54.6 | 78.26 | 6.38 | 0.10 | 32.2 | 7.12 | 1835 | 2278 | 1618 | 7.42 |
| 796 | gpu: greedy@cpu + rayquery + raycast + clipmap + refloor + pathtrace | 0.15 | 54.5 | 130.32 | 5.21 | 11.03 | 32.2 | 7.12 | 2029 | 3805 | 2601 | 12.20 |
| 797 | gpu: points@cpu + splat + distance + refloor + pathtrace | 0.15 | 54.5 | 89.37 | 5.26 | 0.10 | 32.2 | 7.12 | 2283 | 2752 | 1617 | 8.32 |
| 798 | gpu: greedy@cpu + rayquery + raycast + rows + refloor + pathtrace | 0.14 | 54.3 | 135.97 | 4.18 | 11.12 | 32.2 | 7.12 | 1898 | 3646 | 2514 | 11.73 |
| 799 | gpu: greedy@cpu + rayquery + raycast + bricks + incremental + pathtrace | 0.14 | 54.2 | 92.47 | 5.76 | 11.13 | 32.2 | 7.12 | 1834 | 3582 | 2521 | 11.62 |
| 800 | gpu: greedy@cpu + rayquery + raycast + prefix + rowrun + pathtrace | 0.14 | 54.1 | 134.53 | 4.81 | 11.09 | 32.2 | 7.12 | 1914 | 3662 | 2528 | 11.79 |
| 801 | gpu: points@cpu + splat + distance + incremental + direct | 0.14 | 54.0 | 82.32 | 5.32 | 0.10 | 56.0 | 0.01 | 2252 | 2720 | 1619 | 8.26 |
| 802 | gpu: naive@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.14 | 53.9 | 24.77 | 18.01 | 2.75 | 32.2 | 7.12 | 1414 | 1916 | 1130 | 5.80 |
| 803 | gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.14 | 53.7 | 22.98 | 18.15 | 1.76 | 32.2 | 7.12 | 1544 | 1790 | 1295 | 5.88 |
| 804 | gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.14 | 53.7 | 22.60 | 18.17 | 1.76 | 32.2 | 7.12 | 1536 | 1782 | 1295 | 5.86 |
| 805 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 0.14 | 53.6 | 24.79 | 18.36 | 0.27 | 32.1 | 7.12 | 1258 | 2109 | 1053 | 6.02 |
| 806 | gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.14 | 53.6 | 26.43 | 18.21 | 1.81 | 32.2 | 7.12 | 1342 | 1880 | 1065 | 5.61 |
| 807 | gpu: greedy@cpu + rayquery + raycast + svo_dag + refloor + pathtrace | 0.14 | 53.4 | 91.72 | 4.81 | 13.16 | 32.2 | 7.12 | 835 | 2478 | 2700 | 9.86 |
| 808 | gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.14 | 53.0 | 23.51 | 18.39 | 1.84 | 32.2 | 7.12 | 1350 | 1889 | 1113 | 5.72 |
| 809 | gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.14 | 53.0 | 23.79 | 18.38 | 1.81 | 32.2 | 7.12 | 1406 | 1912 | 1104 | 5.74 |
| 810 | gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.14 | 53.0 | 23.23 | 18.39 | 1.83 | 32.2 | 7.12 | 1414 | 1921 | 1096 | 5.75 |
| 811 | gpu: greedy@cpu + rayquery + raycast + rows + rowrun + pathtrace | 0.14 | 53.0 | 136.13 | 5.13 | 11.05 | 32.2 | 7.12 | 1898 | 3646 | 2561 | 11.82 |
| 812 | gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.14 | 53.0 | 23.23 | 18.45 | 1.82 | 32.2 | 7.12 | 1342 | 1880 | 1080 | 5.64 |
| 813 | gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.14 | 52.9 | 23.12 | 18.47 | 1.85 | 32.2 | 7.12 | 1350 | 1889 | 1118 | 5.73 |
| 814 | gpu: greedy@cpu + rayquery + raycast + bricks + rowrun + pathtrace | 0.14 | 52.9 | 98.37 | 6.41 | 11.24 | 32.2 | 7.12 | 1834 | 3582 | 2504 | 11.59 |
| 815 | gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.14 | 52.7 | 29.60 | 16.81 | 5.07 | 32.1 | 7.13 | 1400 | 1620 | 1168 | 5.31 |
| 816 | gpu: greedy@cpu + rayquery + raycast + distance + sphere + pathtrace | 0.14 | 52.7 | 99.56 | 3.69 | 11.04 | 32.2 | 7.12 | 2346 | 4094 | 2591 | 12.73 |
| 817 | gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.14 | 52.7 | 23.53 | 18.53 | 1.82 | 32.2 | 7.12 | 1342 | 1880 | 1066 | 5.61 |
| 818 | gpu: tree64 + refloor + quadtree + raster + pathtrace | 0.14 | 52.5 | 23.82 | 18.90 | 1.34 | 32.1 | 7.13 | 405 | 711 | 1242 | 3.72 |
| 819 | gpu: greedy@cpu + rayquery + raycast + rows + incremental + pathtrace | 0.14 | 52.5 | 134.71 | 5.40 | 11.12 | 32.2 | 7.12 | 1898 | 3646 | 2554 | 11.81 |
| 820 | gpu: greedy@cpu + rayquery + raycast + clipmap + incremental + pathtrace | 0.14 | 52.5 | 98.26 | 6.50 | 11.11 | 32.2 | 7.12 | 2029 | 3805 | 2602 | 12.20 |
| 821 | gpu: points@cpu + splat + clipmap + rowrun + pathtrace | 0.14 | 52.5 | 78.28 | 8.28 | 0.04 | 32.2 | 7.12 | 1966 | 2679 | 1731 | 8.40 |
| 822 | gpu: greedy@cpu + rayquery + raycast + hier + incremental + pathtrace | 0.14 | 52.5 | 135.77 | 5.68 | 11.13 | 32.2 | 7.12 | 1834 | 3583 | 2521 | 11.62 |
| 823 | gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.14 | 52.4 | 24.15 | 18.61 | 1.84 | 32.2 | 7.12 | 1406 | 1912 | 1111 | 5.76 |
| 824 | gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.14 | 52.3 | 23.97 | 18.63 | 1.83 | 32.2 | 7.12 | 1414 | 1921 | 1103 | 5.76 |
| 825 | gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.14 | 52.3 | 23.59 | 18.65 | 1.82 | 32.2 | 7.12 | 1422 | 1925 | 1059 | 5.68 |
| 826 | gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.14 | 52.2 | 23.92 | 18.68 | 1.83 | 32.2 | 7.12 | 1430 | 1933 | 1060 | 5.70 |
| 827 | gpu: points@cpu + splat + distance + rowrun + pathtrace | 0.14 | 52.2 | 83.85 | 6.47 | 0.11 | 32.2 | 7.12 | 2283 | 2752 | 1628 | 8.34 |
| 828 | gpu: greedy@cpu + rayquery + raycast + clipmap + rowrun + pathtrace | 0.14 | 52.1 | 96.91 | 6.71 | 11.00 | 32.2 | 7.12 | 2029 | 3805 | 2615 | 12.23 |
| 829 | gpu: points@cpu + splat + bricks + sphere + pathtrace | 0.14 | 52.1 | 70.03 | 8.96 | 0.10 | 32.2 | 7.12 | 1771 | 2226 | 1619 | 7.32 |
| 830 | gpu: greedy@cpu + rayquery + raycast + prefix + incremental + pathtrace | 0.14 | 52.0 | 138.31 | 5.90 | 11.24 | 32.2 | 7.12 | 1914 | 3662 | 2498 | 11.73 |
| 831 | gpu: greedy@cpu + rayquery + raycast + distance + refloor + pathtrace | 0.14 | 51.9 | 105.03 | 4.20 | 11.03 | 32.2 | 7.12 | 2346 | 4094 | 2533 | 12.62 |
| 832 | gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.14 | 51.8 | 23.57 | 18.88 | 1.84 | 32.2 | 7.12 | 1350 | 1889 | 1089 | 5.67 |
| 833 | gpu: points@cpu + splat + clipmap + incremental + pathtrace | 0.14 | 51.8 | 71.40 | 8.61 | 0.04 | 32.2 | 7.12 | 1966 | 2679 | 1731 | 8.40 |
| 834 | gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.14 | 51.7 | 23.74 | 18.91 | 1.82 | 32.2 | 7.12 | 1342 | 1880 | 1110 | 5.70 |
| 835 | gpu: greedy@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.14 | 51.5 | 36.66 | 16.85 | 5.08 | 32.1 | 7.13 | 1718 | 2240 | 1027 | 6.22 |
| 836 | gpu: points@cpu + splat + hier + sphere + pathtrace | 0.14 | 51.4 | 83.64 | 8.32 | 0.10 | 32.2 | 7.12 | 1771 | 2210 | 1627 | 7.31 |
| 837 | gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.14 | 51.2 | 24.05 | 19.04 | 1.84 | 32.2 | 7.12 | 1422 | 1925 | 1061 | 5.69 |
| 838 | gpu: points@cpu + splat + hier + rowrun + pathtrace | 0.14 | 51.2 | 73.81 | 8.25 | 0.11 | 32.2 | 7.12 | 1771 | 2210 | 1631 | 7.31 |
| 839 | gpu: greedy@cpu + rayquery + raycast + distance + rowrun + pathtrace | 0.14 | 51.2 | 139.17 | 4.93 | 11.03 | 32.2 | 7.12 | 2346 | 4094 | 2520 | 12.60 |
| 840 | gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.14 | 51.1 | 24.02 | 19.10 | 1.83 | 32.2 | 7.12 | 1430 | 1933 | 1060 | 5.70 |
| 841 | gpu: points@cpu + splat + bricks + incremental + pathtrace | 0.14 | 50.7 | 71.46 | 9.40 | 0.11 | 32.2 | 7.12 | 1771 | 2226 | 1626 | 7.34 |
| 842 | gpu: naive@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.14 | 50.6 | 28.24 | 19.11 | 2.73 | 32.2 | 7.12 | 1846 | 2240 | 1130 | 6.42 |
| 843 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.13 | 50.4 | 31.06 | 19.19 | 3.87 | 32.2 | 7.12 | 342 | 535 | 1320 | 3.53 |
| 844 | gpu: greedy@cpu + rayquery + raycast + bricks + sphere + pathtrace | 0.13 | 50.3 | 74.67 | 8.16 | 11.26 | 32.2 | 7.12 | 1834 | 3582 | 2497 | 11.58 |
| 845 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.13 | 50.3 | 30.37 | 19.21 | 3.87 | 32.2 | 7.12 | 350 | 543 | 1323 | 3.55 |
| 846 | gpu: greedy@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.13 | 50.3 | 31.93 | 17.84 | 5.13 | 32.1 | 7.13 | 1206 | 1872 | 1033 | 5.53 |
| 847 | gpu: points@cpu + splat + rows + sphere + pathtrace | 0.13 | 50.2 | 84.66 | 8.96 | 0.11 | 32.2 | 7.12 | 1835 | 2278 | 1624 | 7.43 |
| 848 | gpu: greedy@cpu + rayquery + raycast + hier + sphere + pathtrace | 0.13 | 49.9 | 101.78 | 7.44 | 11.11 | 32.2 | 7.12 | 1834 | 3583 | 2494 | 11.57 |
| 849 | gpu: points@cpu + splat + prefix + incremental + pathtrace | 0.13 | 49.9 | 74.46 | 9.40 | 0.11 | 32.2 | 7.12 | 1851 | 2295 | 1632 | 7.48 |
| 850 | gpu: points@cpu + splat + hier + incremental + pathtrace | 0.13 | 49.8 | 77.16 | 8.88 | 0.11 | 32.2 | 7.12 | 1771 | 2210 | 1634 | 7.32 |
| 851 | gpu: points@cpu + splat + rows + incremental + pathtrace | 0.13 | 49.8 | 74.20 | 8.99 | 0.10 | 32.2 | 7.12 | 1835 | 2278 | 1634 | 7.45 |
| 852 | gpu: naive@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.13 | 49.7 | 27.24 | 19.56 | 2.73 | 32.2 | 7.12 | 1398 | 1904 | 1130 | 5.78 |
| 853 | gpu: points@cpu + splat + tree64 + sphere + pathtrace | 0.13 | 49.5 | 72.66 | 10.89 | 1.20 | 32.2 | 7.12 | 918 | 1289 | 1774 | 5.83 |
| 854 | gpu: points@cpu + splat + prefix + sphere + pathtrace | 0.13 | 49.2 | 79.44 | 9.05 | 0.11 | 32.2 | 7.12 | 1851 | 2295 | 1626 | 7.47 |
| 855 | gpu: points@cpu + splat + svo_dag + sphere + pathtrace | 0.13 | 49.2 | 103.16 | 9.60 | 2.18 | 32.2 | 7.12 | 772 | 1166 | 1844 | 5.73 |
| 856 | gpu: greedy@cpu + rayquery + raycast + hier + rowrun + pathtrace | 0.13 | 48.9 | 94.52 | 8.00 | 11.23 | 32.2 | 7.12 | 1834 | 3583 | 2554 | 11.69 |
| 857 | gpu: greedy@cpu + rayquery + raycast + prefix + sphere + pathtrace | 0.13 | 48.7 | 95.06 | 8.32 | 11.21 | 32.2 | 7.12 | 1914 | 3662 | 2420 | 11.58 |
| 858 | gpu: greedy@cpu + rayquery + raycast + rows + sphere + pathtrace | 0.13 | 48.6 | 101.24 | 8.09 | 11.22 | 32.2 | 7.12 | 1898 | 3646 | 2520 | 11.74 |
| 859 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.13 | 48.4 | 31.02 | 20.01 | 3.89 | 32.2 | 7.12 | 350 | 543 | 1321 | 3.55 |
| 860 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.13 | 48.4 | 30.42 | 20.03 | 3.87 | 32.2 | 7.12 | 342 | 535 | 1320 | 3.53 |
| 861 | gpu: greedy@cpu + rayquery + raycast + distance + incremental + pathtrace | 0.13 | 48.3 | 142.61 | 6.63 | 11.13 | 32.2 | 7.12 | 2346 | 4094 | 2469 | 12.50 |
| 862 | gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.13 | 48.0 | 24.93 | 20.48 | 2.85 | 32.2 | 7.12 | 496 | 715 | 1169 | 3.59 |
| 863 | gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.13 | 48.0 | 24.84 | 20.50 | 2.90 | 32.2 | 7.12 | 488 | 742 | 1182 | 3.66 |
| 864 | gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.13 | 47.9 | 26.31 | 20.34 | 1.82 | 32.2 | 7.12 | 1862 | 2257 | 1113 | 6.42 |
| 865 | gpu: points@cpu + splat + clipmap + sphere + pathtrace | 0.13 | 47.9 | 85.53 | 11.16 | 0.04 | 32.2 | 7.12 | 1966 | 2679 | 1740 | 8.42 |
| 866 | gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.13 | 47.9 | 26.26 | 20.38 | 1.80 | 32.2 | 7.12 | 1854 | 2248 | 1115 | 6.41 |
| 867 | gpu: points@cpu + splat + svo_dag + rowrun + pathtrace | 0.13 | 47.5 | 105.21 | 10.58 | 2.19 | 32.2 | 7.12 | 772 | 1166 | 1843 | 5.73 |
| 868 | gpu: greedy@cpu + rayquery + raycast + clipmap + sphere + pathtrace | 0.13 | 47.5 | 73.76 | 9.87 | 11.15 | 32.2 | 7.12 | 2029 | 3805 | 2629 | 12.25 |
| 869 | gpu: greedy@cpu + rayquery + raycast + svo_dag + incremental + pathtrace | 0.13 | 47.4 | 85.17 | 8.76 | 12.76 | 32.2 | 7.12 | 835 | 2478 | 2665 | 9.79 |
| 870 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.13 | 47.1 | 31.65 | 20.61 | 3.87 | 32.2 | 7.12 | 350 | 543 | 1330 | 3.57 |
| 871 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.13 | 47.0 | 30.65 | 20.63 | 3.88 | 32.2 | 7.12 | 342 | 535 | 1321 | 3.53 |
| 872 | gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.13 | 47.0 | 26.08 | 20.83 | 1.76 | 32.2 | 7.12 | 1536 | 1782 | 1286 | 5.84 |
| 873 | gpu: points@cpu + splat + svo_dag + incremental + pathtrace | 0.12 | 46.9 | 100.80 | 11.05 | 2.19 | 32.2 | 7.12 | 772 | 1166 | 1845 | 5.74 |
| 874 | gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.12 | 46.8 | 26.67 | 20.89 | 1.77 | 32.2 | 7.12 | 1544 | 1790 | 1285 | 5.86 |
| 875 | gpu: naive@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.12 | 46.7 | 28.08 | 20.89 | 2.76 | 32.2 | 7.12 | 1334 | 1872 | 1135 | 5.73 |
| 876 | gpu: svo_dag + refloor + quadtree + raster + pathtrace | 0.12 | 46.7 | 27.07 | 21.04 | 2.38 | 32.1 | 7.13 | 259 | 393 | 1370 | 3.36 |
| 877 | gpu: points@cpu + splat + tree64 + rowrun + pathtrace | 0.12 | 46.6 | 68.21 | 12.85 | 1.19 | 32.2 | 7.12 | 918 | 1287 | 1754 | 5.79 |
| 878 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.12 | 46.6 | 30.31 | 19.88 | 5.07 | 32.1 | 7.13 | 1400 | 1620 | 1168 | 5.31 |
| 879 | gpu: greedy@cpu + rayquery + raycast + tree64 + sphere + pathtrace | 0.12 | 46.3 | 77.99 | 10.45 | 11.86 | 32.2 | 7.12 | 981 | 2723 | 2546 | 10.03 |
| 880 | gpu: naive@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.12 | 46.3 | 28.02 | 21.21 | 3.76 | 32.2 | 7.12 | 480 | 683 | 1134 | 3.46 |
| 881 | gpu: greedy@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.12 | 46.2 | 32.91 | 20.04 | 5.13 | 32.1 | 7.13 | 1270 | 1904 | 1027 | 5.58 |
| 882 | gpu: naive@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.12 | 46.0 | 28.52 | 21.23 | 2.73 | 32.2 | 7.12 | 1414 | 1916 | 1129 | 5.80 |
| 883 | gpu: greedy@cpu + rayquery + raycast + svo_dag + sphere + pathtrace | 0.12 | 45.9 | 88.53 | 9.00 | 13.26 | 32.2 | 7.12 | 835 | 2478 | 2732 | 9.92 |
| 884 | gpu: naive@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.12 | 45.8 | 28.81 | 21.39 | 2.79 | 32.2 | 7.12 | 1334 | 1872 | 1134 | 5.72 |
| 885 | gpu: greedy@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.12 | 45.3 | 35.28 | 20.53 | 5.14 | 32.1 | 7.13 | 1206 | 1872 | 1033 | 5.53 |
| 886 | gpu: points@cpu + splat + distance + incremental + pathtrace | 0.12 | 45.2 | 81.67 | 10.80 | 0.10 | 32.2 | 7.12 | 2283 | 2752 | 1631 | 8.35 |
| 887 | gpu: greedy@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.12 | 45.1 | 30.24 | 20.69 | 5.28 | 32.1 | 7.13 | 1206 | 1872 | 1032 | 5.53 |
| 888 | gpu: greedy@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.12 | 44.9 | 31.74 | 20.80 | 5.12 | 32.1 | 7.13 | 1286 | 1916 | 1027 | 5.60 |
| 889 | gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.12 | 44.6 | 30.46 | 20.98 | 5.27 | 32.1 | 7.13 | 1206 | 1872 | 1032 | 5.53 |
| 890 | gpu: greedy@cpu + rayquery + raycast + svo_dag + rowrun + pathtrace | 0.12 | 44.5 | 80.89 | 11.18 | 12.22 | 32.2 | 7.12 | 835 | 2478 | 2718 | 9.89 |
| 891 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.12 | 44.3 | 26.69 | 22.24 | 2.88 | 32.2 | 7.12 | 496 | 729 | 1185 | 3.65 |
| 892 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.12 | 43.8 | 27.09 | 22.48 | 2.89 | 32.2 | 7.12 | 488 | 841 | 1200 | 3.89 |
| 893 | gpu: rows + rowrun + quadtree + raster + pathtrace | 0.11 | 42.2 | 30.97 | 23.41 | 0.25 | 32.1 | 7.13 | 1322 | 2141 | 1053 | 6.08 |
| 894 | gpu: clipmap + refloor + quadtree + raster + pathtrace | 0.11 | 41.5 | 32.40 | 23.81 | 0.18 | 32.1 | 7.13 | 1453 | 1860 | 1173 | 5.78 |
| 895 | gpu: greedy@cpu + rayquery + raycast + tree64 + rowrun + pathtrace | 0.11 | 41.5 | 67.10 | 15.41 | 10.46 | 32.2 | 7.12 | 981 | 2711 | 2537 | 9.99 |
| 896 | gpu: naive@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.11 | 40.4 | 32.62 | 24.29 | 2.69 | 32.2 | 7.12 | 1528 | 1773 | 1303 | 5.86 |
| 897 | gpu: naive@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.11 | 40.1 | 32.82 | 24.37 | 2.74 | 32.2 | 7.12 | 1846 | 2240 | 1131 | 6.42 |
| 898 | gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + direct | 0.11 | 39.9 | 29.06 | 24.75 | 2.86 | 50.2 | 0.02 | 448 | 652 | 1123 | 3.38 |
| 899 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct | 0.11 | 39.8 | 29.65 | 24.71 | 1.85 | 50.2 | 0.02 | 1302 | 1840 | 1064 | 5.53 |
| 900 | gpu: prefix + rowrun + quadtree + raster + pathtrace | 0.11 | 39.8 | 32.19 | 24.87 | 0.27 | 32.1 | 7.13 | 1339 | 2154 | 1052 | 6.11 |
| 901 | gpu: cubes@cpu + faces + prefix + refloor + depth + raster + direct | 0.11 | 39.7 | 29.76 | 24.68 | 1.81 | 50.2 | 0.02 | 1382 | 1884 | 1059 | 5.61 |
| 902 | gpu: cubes@cpu + faces + hier + refloor + depth + raster + direct | 0.11 | 39.7 | 30.22 | 24.71 | 1.83 | 50.2 | 0.02 | 1302 | 1840 | 1064 | 5.53 |
| 903 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.11 | 39.7 | 29.28 | 24.86 | 2.89 | 50.2 | 0.02 | 448 | 917 | 1095 | 3.83 |
| 904 | gpu: cubes@cpu + faces + rows + refloor + depth + raster + direct | 0.11 | 39.7 | 30.18 | 24.71 | 1.83 | 50.2 | 0.02 | 1366 | 1872 | 1059 | 5.58 |
| 905 | gpu: cubes@cpu + faces + distance + sphere + depth + raster + direct | 0.11 | 39.7 | 31.01 | 24.68 | 1.80 | 50.2 | 0.02 | 1814 | 2208 | 1058 | 6.22 |
| 906 | gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + direct | 0.11 | 39.7 | 29.77 | 24.77 | 1.81 | 50.2 | 0.02 | 1302 | 1840 | 1065 | 5.53 |
| 907 | gpu: cubes@cpu + faces + distance + refloor + depth + raster + direct | 0.11 | 39.7 | 31.45 | 24.71 | 1.81 | 50.2 | 0.02 | 1814 | 2208 | 1059 | 6.22 |
| 908 | gpu: cubes@cpu + faces + hier + rowrun + depth + raster + direct | 0.11 | 39.7 | 29.92 | 24.75 | 1.80 | 50.2 | 0.02 | 1302 | 1840 | 1065 | 5.53 |
| 909 | gpu: cubes@cpu + faces + rows + rowrun + depth + raster + direct | 0.11 | 39.6 | 29.83 | 24.76 | 1.81 | 50.2 | 0.02 | 1366 | 1872 | 1058 | 5.58 |
| 910 | gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + direct | 0.11 | 39.6 | 30.20 | 24.80 | 1.83 | 50.2 | 0.02 | 1382 | 1884 | 1058 | 5.60 |
| 911 | gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + direct | 0.11 | 39.6 | 30.33 | 24.82 | 1.76 | 50.2 | 0.02 | 1497 | 1742 | 1230 | 5.66 |
| 912 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.11 | 39.5 | 29.93 | 24.87 | 1.76 | 50.2 | 0.02 | 1497 | 1742 | 1230 | 5.66 |
| 913 | gpu: cubes@cpu + faces + distance + rowrun + depth + raster + direct | 0.11 | 39.4 | 31.77 | 24.87 | 1.83 | 50.2 | 0.02 | 1814 | 2208 | 1060 | 6.22 |
| 914 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.10 | 39.4 | 29.96 | 24.87 | 3.88 | 50.2 | 0.02 | 303 | 495 | 1276 | 3.37 |
| 915 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.10 | 39.3 | 30.40 | 24.89 | 3.90 | 50.2 | 0.02 | 303 | 495 | 1271 | 3.36 |
| 916 | gpu: cubes@cpu + faces + rows + incremental + depth + raster + direct | 0.10 | 39.2 | 30.38 | 25.02 | 1.81 | 50.2 | 0.02 | 1366 | 1872 | 1059 | 5.58 |
| 917 | gpu: cubes@cpu + faces + hier + incremental + depth + raster + direct | 0.10 | 39.0 | 30.48 | 25.17 | 1.82 | 50.2 | 0.02 | 1302 | 1840 | 1065 | 5.53 |
| 918 | gpu: cubes@cpu + faces + hier + sphere + depth + raster + direct | 0.10 | 38.9 | 30.60 | 25.22 | 1.82 | 50.2 | 0.02 | 1302 | 1840 | 1065 | 5.53 |
| 919 | gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + direct | 0.10 | 38.9 | 29.45 | 25.40 | 2.88 | 50.2 | 0.02 | 448 | 649 | 1100 | 3.33 |
| 920 | gpu: cubes@cpu + faces + bricks + sphere + depth + raster + direct | 0.10 | 38.8 | 30.34 | 25.38 | 1.82 | 50.2 | 0.02 | 1302 | 1840 | 1065 | 5.53 |
| 921 | gpu: cubes@cpu + faces + rows + sphere + depth + raster + direct | 0.10 | 38.7 | 30.40 | 25.35 | 1.83 | 50.2 | 0.02 | 1366 | 1872 | 1060 | 5.58 |
| 922 | gpu: cubes@cpu + faces + bricks + incremental + depth + raster + direct | 0.10 | 38.7 | 30.56 | 25.44 | 2.48 | 50.2 | 0.02 | 1302 | 1840 | 1064 | 5.53 |
| 923 | gpu: cubes@cpu + faces + distance + incremental + depth + raster + direct | 0.10 | 38.6 | 31.58 | 25.39 | 1.82 | 50.2 | 0.02 | 1814 | 2208 | 1060 | 6.22 |
| 924 | gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + direct | 0.10 | 38.5 | 30.87 | 25.53 | 1.78 | 50.2 | 0.02 | 1497 | 1742 | 1231 | 5.66 |
| 925 | gpu: cubes@cpu + faces + prefix + incremental + depth + raster + direct | 0.10 | 38.5 | 30.98 | 25.52 | 1.83 | 50.2 | 0.02 | 1382 | 1884 | 1058 | 5.60 |
| 926 | gpu: distance + rowrun + quadtree + raster + pathtrace | 0.10 | 38.4 | 33.55 | 25.73 | 0.27 | 32.1 | 7.13 | 1770 | 2477 | 1051 | 6.72 |
| 927 | gpu: cubes@cpu + faces + prefix + sphere + depth + raster + direct | 0.10 | 38.4 | 30.81 | 25.55 | 1.83 | 50.2 | 0.02 | 1382 | 1884 | 1059 | 5.61 |
| 928 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.10 | 38.1 | 30.64 | 25.70 | 3.88 | 50.2 | 0.02 | 303 | 495 | 1270 | 3.36 |
| 929 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.10 | 37.9 | 48.00 | 23.87 | 7.12 | 32.1 | 7.13 | 206 | 302 | 1235 | 2.93 |
| 930 | gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + direct | 0.10 | 37.9 | 30.36 | 26.12 | 2.86 | 50.2 | 0.02 | 448 | 735 | 1110 | 3.51 |
| 931 | gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + direct | 0.10 | 37.8 | 31.50 | 25.98 | 1.77 | 50.2 | 0.02 | 1497 | 1742 | 1230 | 5.66 |
| 932 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.10 | 37.2 | 47.91 | 24.49 | 7.16 | 32.1 | 7.13 | 206 | 302 | 1234 | 2.92 |
| 933 | gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.10 | 36.6 | 36.45 | 26.58 | 5.10 | 32.1 | 7.13 | 1400 | 1620 | 1164 | 5.30 |
| 934 | gpu: naive@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.10 | 36.5 | 35.74 | 26.95 | 2.76 | 32.2 | 7.12 | 1334 | 1872 | 1135 | 5.73 |
| 935 | gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.10 | 36.3 | 35.71 | 26.42 | 6.15 | 32.1 | 7.13 | 352 | 841 | 1091 | 3.68 |
| 936 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.10 | 35.8 | 32.03 | 27.40 | 3.91 | 50.2 | 0.02 | 303 | 495 | 1281 | 3.38 |
| 937 | gpu: rows + incremental + quadtree + raster + pathtrace | 0.09 | 33.1 | 39.72 | 29.92 | 0.26 | 32.1 | 7.13 | 1322 | 2141 | 1052 | 6.08 |
| 938 | gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.09 | 33.0 | 38.88 | 29.85 | 2.72 | 32.2 | 7.12 | 1528 | 1773 | 1303 | 5.86 |
| 939 | gpu: naive@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.09 | 32.4 | 41.25 | 30.34 | 2.76 | 32.2 | 7.12 | 1398 | 1904 | 1129 | 5.78 |
| 940 | gpu: cubes@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.09 | 32.3 | 36.81 | 30.51 | 1.83 | 32.2 | 7.12 | 1846 | 2240 | 1060 | 6.28 |
| 941 | gpu: tree64 + incremental + quadtree + raster + pathtrace | 0.09 | 32.2 | 38.90 | 30.93 | 1.37 | 32.1 | 7.13 | 405 | 688 | 1266 | 3.72 |
| 942 | gpu: hier + incremental + quadtree + raster + pathtrace | 0.09 | 32.1 | 40.45 | 30.84 | 0.28 | 32.1 | 7.13 | 1259 | 2110 | 1055 | 6.03 |
| 943 | cpu: hier + refloor + raycast + direct ◆ | 0.08 | 31.6 | 45.95 | - | 30.31 | 58.2 | 0.00 | - | - | 670 | 1.28 |
| 944 | gpu: naive@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.08 | 31.6 | 40.84 | 31.16 | 2.79 | 32.2 | 7.12 | 1334 | 1872 | 1137 | 5.73 |
| 945 | gpu: naive@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.08 | 31.4 | 41.91 | 31.40 | 2.87 | 32.2 | 7.12 | 1334 | 1872 | 1134 | 5.72 |
| 946 | gpu: bricks + incremental + quadtree + raster + pathtrace | 0.08 | 31.4 | 41.26 | 31.62 | 0.27 | 32.1 | 7.13 | 1258 | 2109 | 1051 | 6.02 |
| 947 | gpu: naive@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.08 | 31.0 | 41.81 | 31.73 | 2.77 | 32.2 | 7.12 | 1414 | 1916 | 1130 | 5.80 |
| 948 | gpu: cubes@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.08 | 30.9 | 37.76 | 31.90 | 1.83 | 32.2 | 7.12 | 1334 | 1872 | 1066 | 5.60 |
| 949 | cpu: prefix + incremental + raycast + direct | 0.08 | 30.7 | 46.03 | - | 31.21 | 58.2 | 0.00 | - | - | 885 | 1.69 |
| 950 | gpu: naive@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.08 | 30.6 | 41.70 | 32.23 | 2.84 | 32.2 | 7.12 | 1334 | 1872 | 1135 | 5.73 |
| 951 | gpu: prefix + incremental + quadtree + raster + pathtrace | 0.08 | 30.5 | 42.24 | 32.54 | 0.27 | 32.1 | 7.13 | 1339 | 2154 | 1053 | 6.11 |
| 952 | gpu: cubes@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.08 | 30.4 | 38.02 | 32.40 | 1.85 | 32.2 | 7.12 | 1414 | 1916 | 1060 | 5.67 |
| 953 | gpu: cubes@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.08 | 30.1 | 38.97 | 32.77 | 1.84 | 32.2 | 7.12 | 1846 | 2240 | 1061 | 6.29 |
| 954 | gpu: cubes@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.08 | 29.9 | 39.17 | 32.94 | 1.86 | 32.2 | 7.12 | 1398 | 1904 | 1059 | 5.64 |
| 955 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.08 | 29.9 | 38.53 | 33.08 | 1.85 | 32.2 | 7.12 | 1334 | 1872 | 1064 | 5.59 |
| 956 | gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.08 | 29.6 | 38.48 | 33.54 | 2.88 | 32.2 | 7.12 | 480 | 705 | 1113 | 3.46 |
| 957 | cpu: hier + incremental + raycast + direct ◆ | 0.08 | 29.5 | 46.71 | - | 32.53 | 58.3 | 0.00 | - | - | 670 | 1.28 |
| 958 | cpu: prefix + refloor + raycast + direct | 0.08 | 29.2 | 48.55 | - | 32.82 | 58.2 | 0.00 | - | - | 885 | 1.69 |
| 959 | gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.08 | 28.7 | 42.41 | 34.37 | 4.85 | 32.2 | 7.12 | 334 | 526 | 1272 | 3.43 |
| 960 | cpu: bricks + refloor + raycast + direct ◆ | 0.08 | 28.5 | 46.50 | - | 34.32 | 58.2 | 0.00 | - | - | 635 | 1.21 |
| 961 | gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.07 | 28.0 | 44.51 | 35.24 | 4.87 | 32.2 | 7.12 | 334 | 526 | 1266 | 3.41 |
| 962 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.07 | 28.0 | 41.43 | 35.34 | 3.92 | 32.2 | 7.12 | 334 | 526 | 1258 | 3.40 |
| 963 | cpu: bricks + incremental + raycast + direct | 0.07 | 27.8 | 47.60 | - | 35.07 | 58.3 | 0.00 | - | - | 635 | 1.21 |
| 964 | gpu: distance + incremental + quadtree + raster + pathtrace | 0.07 | 27.5 | 47.31 | 36.01 | 0.27 | 32.1 | 7.13 | 1770 | 2477 | 1054 | 6.73 |
| 965 | cpu: bits + refloor + raycast + direct | 0.07 | 27.5 | 51.11 | - | 35.06 | 58.2 | 0.00 | - | - | 670 | 1.28 |
| 966 | cpu: brickmajor + refloor + raycast + direct | 0.07 | 27.3 | 51.10 | - | 35.06 | 58.2 | 0.00 | - | - | 959 | 1.83 |
| 967 | cpu: u8 + refloor + raycast + direct ◆ | 0.07 | 27.1 | 49.31 | - | 36.03 | 58.2 | 0.00 | - | - | 534 | 1.02 |
| 968 | cpu: bits + incremental + raycast + direct | 0.07 | 26.9 | 49.55 | - | 35.81 | 58.3 | 0.00 | - | - | 670 | 1.28 |
| 969 | gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.07 | 26.9 | 43.44 | 36.71 | 1.79 | 32.2 | 7.12 | 1528 | 1773 | 1227 | 5.71 |
| 970 | gpu: hier + sphere + quadtree + raster + pathtrace | 0.07 | 26.8 | 48.49 | 37.06 | 0.28 | 32.1 | 7.13 | 1259 | 2110 | 1056 | 6.03 |
| 971 | cpu: brickmajor + incremental + raycast + direct | 0.07 | 26.7 | 51.41 | - | 35.89 | 58.3 | 0.00 | - | - | 982 | 1.87 |
| 972 | gpu: clipmap + incremental + quadtree + raster + pathtrace | 0.07 | 26.6 | 50.40 | 37.19 | 0.20 | 32.1 | 7.13 | 1453 | 1860 | 1174 | 5.78 |
| 973 | gpu: cubes@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.07 | 26.4 | 43.89 | 37.38 | 1.85 | 32.2 | 7.12 | 1398 | 1904 | 1061 | 5.65 |
| 974 | cpu: u8 + incremental + raycast + direct | 0.07 | 26.4 | 67.31 | - | 37.02 | 58.3 | 0.00 | - | - | 552 | 1.05 |
| 975 | cpu: morton + refloor + raycast + direct | 0.07 | 26.3 | 59.53 | - | 36.21 | 58.2 | 0.00 | - | - | 981 | 1.87 |
| 976 | cpu: morton + incremental + raycast + direct | 0.07 | 26.2 | 52.99 | - | 36.36 | 58.3 | 0.00 | - | - | 998 | 1.90 |
| 977 | cpu: palette + incremental + raycast + direct ◆ | 0.07 | 26.0 | 56.49 | - | 36.04 | 58.3 | 0.00 | - | - | 525 | 1.00 |
| 978 | cpu: palette + refloor + raycast + direct | 0.07 | 25.8 | 57.34 | - | 36.39 | 58.2 | 0.00 | - | - | 526 | 1.00 |
| 979 | gpu: clipmap + rowrun + quadtree + raster + pathtrace | 0.07 | 25.7 | 49.86 | 38.60 | 0.19 | 32.1 | 7.13 | 1453 | 1860 | 1174 | 5.78 |
| 980 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.07 | 25.6 | 48.35 | 38.70 | 6.22 | 32.1 | 7.13 | 352 | 559 | 1106 | 3.17 |
| 981 | gpu: hier + rowrun + quadtree + raster + pathtrace | 0.07 | 25.5 | 48.81 | 39.00 | 0.28 | 32.1 | 7.13 | 1259 | 2110 | 1055 | 6.03 |
| 982 | gpu: naive@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.07 | 25.5 | 51.65 | 38.81 | 2.72 | 32.2 | 7.12 | 1528 | 1773 | 1300 | 5.85 |
| 983 | gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.07 | 25.3 | 45.79 | 39.02 | 1.86 | 32.2 | 7.12 | 1414 | 1916 | 1061 | 5.67 |
| 984 | gpu: bricks + rowrun + quadtree + raster + pathtrace | 0.07 | 25.2 | 49.36 | 39.46 | 0.29 | 32.1 | 7.13 | 1258 | 2109 | 1051 | 6.02 |
| 985 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.07 | 25.1 | 61.03 | 39.16 | 7.21 | 32.1 | 7.13 | 206 | 302 | 1236 | 2.93 |
| 986 | gpu: naive@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.07 | 24.7 | 50.91 | 40.21 | 3.79 | 32.2 | 7.12 | 480 | 734 | 1124 | 3.54 |
| 987 | gpu: cubes@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.07 | 24.5 | 47.30 | 40.38 | 1.84 | 32.2 | 7.12 | 1846 | 2240 | 1060 | 6.29 |
| 988 | gpu: cubes@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.06 | 23.8 | 49.61 | 41.63 | 1.84 | 32.2 | 7.12 | 1398 | 1904 | 1060 | 5.64 |
| 989 | gpu: rows + sphere + quadtree + raster + pathtrace | 0.06 | 23.7 | 54.93 | 41.99 | 0.30 | 32.1 | 7.13 | 1322 | 2141 | 1054 | 6.08 |
| 990 | gpu: prefix + sphere + quadtree + raster + pathtrace | 0.06 | 23.5 | 54.75 | 42.29 | 0.29 | 32.1 | 7.13 | 1339 | 2154 | 1053 | 6.11 |
| 991 | gpu: bricks + sphere + quadtree + raster + pathtrace | 0.06 | 23.1 | 56.03 | 43.04 | 0.30 | 32.1 | 7.13 | 1258 | 2109 | 1051 | 6.02 |
| 992 | gpu: cubes@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.06 | 22.9 | 50.62 | 43.17 | 1.85 | 32.2 | 7.12 | 1334 | 1872 | 1066 | 5.60 |
| 993 | gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.06 | 22.8 | 49.85 | 43.64 | 2.90 | 32.2 | 7.12 | 480 | 709 | 1095 | 3.44 |
| 994 | gpu: cubes@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.06 | 22.7 | 50.73 | 43.58 | 1.87 | 32.2 | 7.12 | 1414 | 1916 | 1060 | 5.67 |
| 995 | gpu: cubes@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.06 | 22.6 | 51.20 | 43.86 | 2.37 | 32.2 | 7.12 | 1334 | 1872 | 1065 | 5.59 |
| 996 | gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.06 | 21.1 | 55.77 | 46.98 | 1.79 | 32.2 | 7.12 | 1528 | 1773 | 1228 | 5.72 |
| 997 | gpu: cubes@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.06 | 20.7 | 56.79 | 47.72 | 1.87 | 32.2 | 7.12 | 1846 | 2240 | 1061 | 6.29 |
| 998 | gpu: svo_dag + sphere + quadtree + raster + pathtrace | 0.05 | 20.5 | 61.13 | 48.53 | 2.46 | 32.1 | 7.13 | 259 | 393 | 1366 | 3.35 |
| 999 | gpu: cubes@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.05 | 19.5 | 60.34 | 50.83 | 1.88 | 32.2 | 7.12 | 1334 | 1872 | 1065 | 5.59 |
| 1000 | gpu: clipmap + sphere + quadtree + raster + pathtrace | 0.05 | 18.8 | 70.00 | 52.92 | 0.20 | 32.1 | 7.13 | 1453 | 1860 | 1174 | 5.78 |
| 1001 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.05 | 18.6 | 62.66 | 53.28 | 1.83 | 32.2 | 7.12 | 1528 | 1773 | 1228 | 5.72 |
| 1002 | gpu: svo_dag + incremental + quadtree + raster + pathtrace | 0.05 | 18.3 | 67.54 | 54.56 | 2.49 | 32.1 | 7.13 | 259 | 393 | 1367 | 3.35 |
| 1003 | gpu: cubes@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.05 | 18.0 | 65.81 | 55.15 | 1.88 | 32.2 | 7.12 | 1398 | 1904 | 1060 | 5.65 |
| 1004 | gpu: cubes@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.05 | 18.0 | 64.59 | 55.20 | 1.86 | 32.2 | 7.12 | 1334 | 1872 | 1066 | 5.60 |
| 1005 | gpu: tree64 + sphere + quadtree + raster + pathtrace | 0.05 | 17.8 | 72.53 | 56.11 | 1.43 | 32.1 | 7.13 | 405 | 673 | 1267 | 3.70 |
| 1006 | gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.05 | 17.6 | 65.94 | 56.53 | 1.88 | 32.2 | 7.12 | 1334 | 1872 | 1066 | 5.59 |
| 1007 | gpu: cubes@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.05 | 17.5 | 67.06 | 56.68 | 1.89 | 32.2 | 7.12 | 1414 | 1916 | 1060 | 5.67 |
| 1008 | gpu: cubes@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.05 | 17.5 | 67.99 | 56.84 | 1.94 | 32.2 | 7.12 | 1334 | 1872 | 1066 | 5.59 |
| 1009 | cpu: greedy + faces + raster + hier + incremental + direct | 0.05 | 16.9 | 87.25 | - | 58.00 | 63.0 | 0.00 | - | - | 1111 | 2.12 |
| 1010 | cpu: greedy + faces + raster + prefix + incremental + direct | 0.04 | 16.8 | 88.30 | - | 58.06 | 63.0 | 0.00 | - | - | 1626 | 3.10 |
| 1011 | cpu: greedy + faces + raster + hier + refloor + direct | 0.04 | 16.8 | 87.93 | - | 58.23 | 63.0 | 0.00 | - | - | 1094 | 2.08 |
| 1012 | cpu: greedy + faces + raster + u8 + refloor + direct | 0.04 | 16.8 | 85.39 | - | 58.71 | 63.0 | 0.00 | - | - | 1366 | 2.60 |
| 1013 | cpu: greedy + faces + raster + u8 + incremental + direct | 0.04 | 16.8 | 86.20 | - | 58.71 | 63.0 | 0.00 | - | - | 1400 | 2.67 |
| 1014 | cpu: greedy + faces + raster + bits + incremental + direct | 0.04 | 16.8 | 89.22 | - | 58.43 | 63.0 | 0.00 | - | - | 1122 | 2.14 |
| 1015 | cpu: greedy + faces + raster + prefix + refloor + direct | 0.04 | 16.7 | 88.50 | - | 58.32 | 63.0 | 0.00 | - | - | 1575 | 3.00 |
| 1016 | cpu: greedy + faces + raster + bricks + incremental + direct | 0.04 | 16.7 | 86.59 | - | 58.85 | 63.0 | 0.00 | - | - | 1080 | 2.06 |
| 1017 | cpu: greedy + faces + raster + bits + refloor + direct | 0.04 | 16.7 | 88.40 | - | 58.63 | 63.0 | 0.00 | - | - | 1142 | 2.18 |
| 1018 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.04 | 16.6 | 71.92 | 59.72 | 4.94 | 32.2 | 7.12 | 334 | 526 | 1282 | 3.44 |
| 1019 | cpu: greedy + faces + raster + brickmajor + incremental + direct | 0.04 | 16.6 | 90.51 | - | 58.67 | 63.0 | 0.00 | - | - | 1651 | 3.14 |
| 1020 | cpu: greedy + faces + raster + bricks + refloor + direct | 0.04 | 16.6 | 87.73 | - | 59.42 | 63.0 | 0.00 | - | - | 1067 | 2.03 |
| 1021 | cpu: greedy + faces + raster + morton + incremental + direct | 0.04 | 16.6 | 91.42 | - | 58.73 | 63.0 | 0.00 | - | - | 1710 | 3.26 |
| 1022 | gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.04 | 16.5 | 76.09 | 60.19 | 3.83 | 32.2 | 7.12 | 480 | 733 | 1124 | 3.54 |
| 1023 | cpu: greedy + faces + raster + morton + refloor + direct | 0.04 | 16.5 | 92.03 | - | 58.82 | 63.0 | 0.00 | - | - | 1667 | 3.17 |
| 1024 | cpu: greedy + faces + raster + brickmajor + refloor + direct | 0.04 | 16.5 | 90.56 | - | 59.17 | 63.0 | 0.00 | - | - | 1648 | 3.14 |
| 1025 | cpu: greedy + faces + raster + palette + incremental + direct | 0.04 | 16.4 | 94.08 | - | 58.65 | 63.0 | 0.00 | - | - | 1162 | 2.21 |
| 1026 | cpu: greedy + faces + raster + palette + refloor + direct | 0.04 | 16.4 | 94.62 | - | 58.89 | 63.0 | 0.00 | - | - | 1156 | 2.20 |
| 1027 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.04 | 16.3 | 69.87 | 60.85 | 4.02 | 32.2 | 7.12 | 334 | 526 | 1260 | 3.40 |
| 1028 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.04 | 16.3 | 70.47 | 61.00 | 4.02 | 32.2 | 7.12 | 334 | 526 | 1270 | 3.42 |
| 1029 | gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.04 | 15.3 | 77.91 | 64.78 | 1.84 | 32.2 | 7.12 | 1528 | 1773 | 1232 | 5.72 |
| 1030 | gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.04 | 14.8 | 79.40 | 67.17 | 3.05 | 32.2 | 7.12 | 480 | 907 | 1097 | 3.82 |
| 1031 | gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 0.04 | 14.7 | 81.02 | 67.99 | 2.50 | 32.1 | 7.13 | 259 | 393 | 1360 | 3.34 |
| 1032 | gpu: tree64 + rowrun + quadtree + raster + pathtrace | 0.04 | 14.2 | 84.21 | 70.11 | 1.45 | 32.1 | 7.13 | 405 | 799 | 1264 | 3.93 |
| 1033 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 13.3 | 94.83 | 17.63 | 71.21 | 31.8 | 7.14 | 1084 | 1876 | 1800 | 7.00 |
| 1034 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.04 | 13.1 | 154.44 | 3.35 | 71.35 | 42.7 | 0.04 | 1053 | 1844 | 1812 | 6.96 |
| 1035 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.1 | 155.39 | 4.16 | 71.67 | 31.8 | 7.14 | 1092 | 1884 | 1854 | 7.12 |
| 1036 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.1 | 153.26 | 4.71 | 72.09 | 31.8 | 7.14 | 1100 | 1892 | 1877 | 7.18 |
| 1037 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 13.0 | 156.63 | 1.08 | 71.96 | 42.7 | 0.04 | 1069 | 1861 | 1821 | 7.01 |
| 1038 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 13.0 | 158.23 | 0.89 | 72.10 | 42.7 | 0.04 | 1061 | 1852 | 1815 | 6.98 |
| 1039 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 122.13 | 17.78 | 73.58 | 31.8 | 7.14 | 1084 | 1876 | 2346 | 8.04 |
| 1040 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 119.78 | 17.66 | 73.87 | 31.8 | 7.14 | 1084 | 1876 | 2225 | 7.81 |
| 1041 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 94.32 | 17.86 | 74.91 | 31.8 | 7.14 | 1084 | 1876 | 1822 | 7.04 |
| 1042 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 94.62 | 18.08 | 74.89 | 31.8 | 7.14 | 1084 | 1876 | 2077 | 7.53 |
| 1043 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 95.60 | 18.15 | 74.48 | 31.8 | 7.14 | 1084 | 1876 | 1797 | 6.99 |
| 1044 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 153.82 | 4.46 | 73.05 | 31.8 | 7.14 | 1092 | 1884 | 2443 | 8.24 |
| 1045 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 153.57 | 4.63 | 73.16 | 31.8 | 7.14 | 1100 | 1892 | 2344 | 8.07 |
| 1046 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + refloor + direct | 0.03 | 12.7 | 160.61 | 4.33 | 74.38 | 42.7 | 0.04 | 1247 | 1596 | 1896 | 6.65 |
| 1047 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 156.93 | 0.87 | 73.29 | 42.7 | 0.04 | 1061 | 1852 | 2376 | 8.05 |
| 1048 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 157.41 | 1.09 | 73.13 | 42.7 | 0.04 | 1069 | 1861 | 2372 | 8.06 |
| 1049 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + incremental + direct | 0.03 | 12.7 | 161.32 | 6.61 | 74.47 | 42.7 | 0.04 | 1247 | 1596 | 1897 | 6.65 |
| 1050 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + incremental + direct | 0.03 | 12.7 | 116.79 | 10.54 | 76.77 | 42.7 | 0.04 | 53 | 131 | 2022 | 4.10 |
| 1051 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 124.16 | 17.75 | 74.19 | 31.8 | 7.14 | 1084 | 1876 | 2381 | 8.11 |
| 1052 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + sphere + direct | 0.03 | 12.7 | 158.84 | 6.79 | 73.71 | 42.7 | 0.04 | 1117 | 1876 | 1791 | 6.98 |
| 1053 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + rowrun + direct | 0.03 | 12.7 | 148.85 | 3.95 | 76.75 | 42.7 | 0.04 | 53 | 131 | 2053 | 4.16 |
| 1054 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 150.04 | 4.21 | 74.59 | 31.8 | 7.14 | 1092 | 1884 | 1740 | 6.90 |
| 1055 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + sphere + direct | 0.03 | 12.7 | 114.66 | 7.28 | 76.91 | 42.7 | 0.04 | 53 | 131 | 2060 | 4.17 |
| 1056 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 157.32 | 4.78 | 74.66 | 31.8 | 7.14 | 1100 | 1892 | 1876 | 7.18 |
| 1057 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 153.97 | 4.75 | 73.71 | 31.8 | 7.14 | 1100 | 1892 | 2435 | 8.24 |
| 1058 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 158.36 | 4.17 | 73.66 | 31.8 | 7.14 | 1092 | 1884 | 2369 | 8.10 |
| 1059 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 157.86 | 0.88 | 73.52 | 42.7 | 0.04 | 1061 | 1852 | 2300 | 7.91 |
| 1060 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 159.71 | 3.35 | 73.28 | 42.7 | 0.04 | 1053 | 1844 | 2397 | 8.08 |
| 1061 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + rowrun + direct | 0.03 | 12.7 | 163.05 | 4.78 | 74.67 | 42.7 | 0.04 | 1247 | 1596 | 1941 | 6.74 |
| 1062 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 164.79 | 3.35 | 74.53 | 42.7 | 0.04 | 1053 | 1844 | 2053 | 7.42 |
| 1063 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 158.93 | 0.89 | 74.44 | 42.7 | 0.04 | 1061 | 1852 | 2062 | 7.45 |
| 1064 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.7 | 156.18 | 1.12 | 73.56 | 42.7 | 0.04 | 1069 | 1861 | 2286 | 7.90 |
| 1065 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + sphere + direct | 0.03 | 12.7 | 156.61 | 6.19 | 74.05 | 42.7 | 0.04 | 1053 | 1844 | 1816 | 6.97 |
| 1066 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + rowrun + direct | 0.03 | 12.7 | 160.32 | 3.60 | 73.75 | 42.7 | 0.04 | 1117 | 1876 | 1782 | 6.97 |
| 1067 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 157.43 | 4.71 | 74.37 | 31.8 | 7.14 | 1100 | 1892 | 1784 | 7.00 |
| 1068 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + incremental + direct | 0.03 | 12.7 | 162.11 | 5.74 | 74.05 | 42.7 | 0.04 | 1133 | 1888 | 1793 | 7.01 |
| 1069 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 159.22 | 3.34 | 73.74 | 42.7 | 0.04 | 1053 | 1844 | 2370 | 8.03 |
| 1070 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + refloor + direct | 0.03 | 12.6 | 132.26 | 3.59 | 77.25 | 42.7 | 0.04 | 53 | 131 | 2043 | 4.14 |
| 1071 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + rowrun + direct | 0.03 | 12.6 | 160.90 | 3.49 | 74.03 | 42.7 | 0.04 | 1053 | 1844 | 1789 | 6.92 |
| 1072 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + incremental + direct | 0.03 | 12.6 | 159.15 | 4.78 | 74.00 | 42.7 | 0.04 | 1117 | 1876 | 1821 | 7.04 |
| 1073 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + refloor + direct | 0.03 | 12.6 | 158.62 | 3.24 | 73.85 | 42.7 | 0.04 | 1117 | 1876 | 1789 | 6.98 |
| 1074 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + refloor + direct | 0.03 | 12.6 | 162.63 | 3.23 | 74.07 | 42.7 | 0.04 | 1053 | 1844 | 1789 | 6.92 |
| 1075 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 155.03 | 4.22 | 74.63 | 31.8 | 7.14 | 1092 | 1884 | 1872 | 7.15 |
| 1076 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + refloor + direct | 0.03 | 12.6 | 153.58 | 3.93 | 72.04 | 42.7 | 0.04 | 1565 | 2212 | 1806 | 7.65 |
| 1077 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + sphere + direct | 0.03 | 12.6 | 157.04 | 9.18 | 75.10 | 42.7 | 0.04 | 1247 | 1596 | 1940 | 6.73 |
| 1078 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + refloor + direct | 0.03 | 12.6 | 155.64 | 3.00 | 73.90 | 42.7 | 0.04 | 1133 | 1888 | 1797 | 7.02 |
| 1079 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + incremental + direct | 0.03 | 12.6 | 100.32 | 5.95 | 77.63 | 42.7 | 0.04 | 199 | 533 | 2024 | 4.87 |
| 1080 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + sphere + direct | 0.03 | 12.6 | 149.25 | 3.68 | 72.09 | 42.7 | 0.04 | 1565 | 2212 | 1800 | 7.64 |
| 1081 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + incremental + direct | 0.03 | 12.6 | 151.05 | 5.90 | 72.38 | 42.7 | 0.04 | 1565 | 2212 | 1814 | 7.67 |
| 1082 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + sphere + direct | 0.03 | 12.6 | 161.95 | 7.17 | 74.42 | 42.7 | 0.04 | 1133 | 1888 | 1777 | 6.98 |
| 1083 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 145.46 | 17.89 | 73.68 | 31.8 | 7.14 | 1084 | 1876 | 1865 | 7.12 |
| 1084 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 161.09 | 3.35 | 75.05 | 42.7 | 0.04 | 1053 | 1844 | 1752 | 6.85 |
| 1085 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 158.62 | 0.88 | 73.78 | 42.7 | 0.04 | 1061 | 1852 | 2381 | 8.06 |
| 1086 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 164.82 | 4.21 | 75.25 | 31.8 | 7.14 | 1092 | 1884 | 2063 | 7.52 |
| 1087 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 158.49 | 1.11 | 74.93 | 42.7 | 0.04 | 1069 | 1861 | 2065 | 7.48 |
| 1088 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 165.53 | 4.75 | 75.32 | 31.8 | 7.14 | 1100 | 1892 | 2065 | 7.54 |
| 1089 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + rowrun + direct | 0.03 | 12.6 | 151.79 | 4.21 | 72.27 | 42.7 | 0.04 | 1565 | 2212 | 1765 | 7.57 |
| 1090 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 162.99 | 3.34 | 74.15 | 42.7 | 0.04 | 1053 | 1844 | 2293 | 7.88 |
| 1091 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 160.57 | 1.11 | 74.06 | 42.7 | 0.04 | 1069 | 1861 | 2365 | 8.05 |
| 1092 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 158.32 | 4.27 | 74.41 | 31.8 | 7.14 | 1092 | 1884 | 2244 | 7.86 |
| 1093 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + incremental + direct | 0.03 | 12.6 | 157.78 | 5.14 | 74.72 | 42.7 | 0.04 | 1053 | 1844 | 1794 | 6.93 |
| 1094 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + rowrun + direct | 0.03 | 12.6 | 162.11 | 3.25 | 74.33 | 42.7 | 0.04 | 1133 | 1888 | 1828 | 7.08 |
| 1095 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + refloor + direct | 0.03 | 12.6 | 101.30 | 3.51 | 77.93 | 42.7 | 0.04 | 199 | 451 | 1978 | 4.63 |
| 1096 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 163.47 | 0.90 | 75.15 | 42.7 | 0.04 | 1061 | 1852 | 1764 | 6.89 |
| 1097 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.6 | 157.36 | 1.11 | 75.24 | 42.7 | 0.04 | 1069 | 1861 | 1779 | 6.93 |
| 1098 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.5 | 164.28 | 1.12 | 74.87 | 42.7 | 0.04 | 1069 | 1861 | 1825 | 7.02 |
| 1099 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + rowrun + direct | 0.03 | 12.5 | 106.27 | 4.05 | 78.00 | 42.7 | 0.04 | 199 | 476 | 2000 | 4.72 |
| 1100 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.5 | 162.95 | 4.77 | 74.70 | 31.8 | 7.14 | 1100 | 1892 | 2320 | 8.02 |
| 1101 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.5 | 161.87 | 0.89 | 75.00 | 42.7 | 0.04 | 1061 | 1852 | 1842 | 7.04 |
| 1102 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + sphere + direct | 0.03 | 12.5 | 101.41 | 8.85 | 78.22 | 42.7 | 0.04 | 199 | 547 | 1984 | 4.82 |
| 1103 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.5 | 153.62 | 4.77 | 73.70 | 31.8 | 7.14 | 1100 | 1892 | 1914 | 7.25 |
| 1104 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.5 | 155.15 | 0.89 | 73.57 | 42.7 | 0.04 | 1061 | 1852 | 1840 | 7.03 |
| 1105 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.5 | 160.38 | 3.35 | 75.28 | 42.7 | 0.04 | 1053 | 1844 | 1841 | 7.02 |
| 1106 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.4 | 153.63 | 4.28 | 74.11 | 31.8 | 7.14 | 1092 | 1884 | 1902 | 7.21 |
| 1107 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 12.4 | 155.68 | 1.11 | 74.11 | 42.7 | 0.04 | 1069 | 1861 | 1835 | 7.04 |
| 1108 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 12.4 | 155.62 | 3.36 | 74.44 | 42.7 | 0.04 | 1053 | 1844 | 1829 | 6.99 |
| 1109 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.03 | 11.3 | 100.17 | 88.14 | 4.12 | 32.2 | 7.12 | 334 | 526 | 1253 | 3.39 |
| 1110 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.03 | 11.1 | 103.85 | 89.58 | 2.92 | 32.2 | 7.12 | 480 | 880 | 1095 | 3.76 |
| 1111 | cpu: naive + faces + raster + hier + incremental + direct | 0.03 | 9.5 | 127.08 | - | 103.52 | 64.6 | 0.00 | - | - | 1830 | 3.49 |
| 1112 | cpu: naive + faces + raster + prefix + refloor + direct | 0.03 | 9.5 | 125.42 | - | 103.70 | 64.6 | 0.00 | - | - | 2290 | 4.36 |
| 1113 | cpu: naive + faces + raster + u8 + incremental + direct | 0.03 | 9.5 | 124.97 | - | 104.42 | 64.6 | 0.00 | - | - | 2030 | 3.87 |
| 1114 | cpu: naive + faces + raster + prefix + incremental + direct | 0.03 | 9.5 | 122.52 | - | 103.98 | 64.6 | 0.00 | - | - | 2210 | 4.21 |
| 1115 | cpu: naive + faces + raster + bricks + incremental + direct | 0.03 | 9.5 | 125.99 | - | 104.77 | 64.6 | 0.00 | - | - | 1815 | 3.46 |
| 1116 | cpu: naive + faces + raster + hier + refloor + direct | 0.03 | 9.5 | 126.87 | - | 104.46 | 64.6 | 0.00 | - | - | 1841 | 3.51 |
| 1117 | cpu: naive + faces + raster + u8 + refloor + direct | 0.03 | 9.5 | 125.57 | - | 104.87 | 64.6 | 0.00 | - | - | 2042 | 3.89 |
| 1118 | cpu: naive + faces + raster + bits + incremental + direct | 0.03 | 9.4 | 125.05 | - | 104.57 | 64.6 | 0.00 | - | - | 1775 | 3.38 |
| 1119 | cpu: naive + faces + raster + bricks + refloor + direct | 0.03 | 9.4 | 124.57 | - | 105.42 | 64.6 | 0.00 | - | - | 1721 | 3.28 |
| 1120 | cpu: naive + faces + raster + palette + refloor + direct | 0.03 | 9.4 | 131.24 | - | 104.04 | 64.6 | 0.00 | - | - | 1813 | 3.45 |
| 1121 | cpu: naive + faces + raster + bits + refloor + direct | 0.03 | 9.4 | 127.15 | - | 105.13 | 64.6 | 0.00 | - | - | 1767 | 3.36 |
| 1122 | cpu: naive + faces + raster + palette + incremental + direct | 0.03 | 9.4 | 129.89 | - | 104.33 | 64.6 | 0.00 | - | - | 1817 | 3.46 |
| 1123 | cpu: naive + faces + raster + brickmajor + incremental + direct | 0.03 | 9.4 | 128.27 | - | 105.07 | 64.6 | 0.00 | - | - | 2313 | 4.41 |
| 1124 | cpu: naive + faces + raster + morton + incremental + direct | 0.02 | 9.4 | 130.74 | - | 105.07 | 64.6 | 0.00 | - | - | 2317 | 4.41 |
| 1125 | cpu: naive + faces + raster + brickmajor + refloor + direct | 0.02 | 9.4 | 127.17 | - | 105.19 | 64.6 | 0.00 | - | - | 2325 | 4.43 |
| 1126 | cpu: naive + faces + raster + morton + refloor + direct | 0.02 | 9.3 | 130.80 | - | 105.61 | 64.6 | 0.00 | - | - | 2302 | 4.38 |
| 1127 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.9 | 474.74 | 3.39 | 199.85 | 42.5 | 0.04 | 1053 | 1844 | 1680 | 6.71 |
| 1128 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.9 | 330.60 | 5.22 | 200.29 | 31.8 | 7.14 | 1092 | 1884 | 1673 | 6.77 |
| 1129 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.9 | 328.91 | 6.04 | 200.31 | 31.8 | 7.14 | 1100 | 1892 | 1710 | 6.86 |
| 1130 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.9 | 465.44 | 1.18 | 201.15 | 42.5 | 0.04 | 1069 | 1861 | 1759 | 6.89 |
| 1131 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.8 | 481.78 | 0.92 | 201.82 | 42.5 | 0.04 | 1061 | 1852 | 1699 | 6.76 |
| 1132 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.8 | 256.63 | 20.05 | 202.51 | 31.8 | 7.14 | 1084 | 1876 | 1618 | 6.65 |
| 1133 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.8 | 263.14 | 5.19 | 206.17 | 31.8 | 7.14 | 1092 | 1884 | 1956 | 7.31 |
| 1134 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 441.66 | 3.42 | 206.75 | 42.5 | 0.04 | 1053 | 1844 | 1990 | 7.30 |
| 1135 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 483.02 | 0.93 | 207.14 | 42.5 | 0.04 | 1061 | 1852 | 1980 | 7.30 |
| 1136 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 438.76 | 1.18 | 207.55 | 42.5 | 0.04 | 1069 | 1861 | 1990 | 7.33 |
| 1137 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 261.44 | 20.08 | 207.92 | 31.8 | 7.14 | 1084 | 1876 | 2000 | 7.38 |
| 1138 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 487.25 | 3.41 | 207.27 | 42.5 | 0.04 | 1053 | 1844 | 2283 | 7.86 |
| 1139 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 488.14 | 0.92 | 207.42 | 42.5 | 0.04 | 1061 | 1852 | 2300 | 7.91 |
| 1140 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 354.59 | 6.01 | 207.60 | 31.8 | 7.14 | 1100 | 1892 | 2248 | 7.89 |
| 1141 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 480.40 | 1.17 | 207.68 | 42.5 | 0.04 | 1069 | 1861 | 2277 | 7.88 |
| 1142 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 382.48 | 5.25 | 207.73 | 31.8 | 7.14 | 1092 | 1884 | 2256 | 7.88 |
| 1143 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 268.84 | 5.17 | 209.67 | 31.8 | 7.14 | 1092 | 1884 | 1639 | 6.71 |
| 1144 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 267.66 | 6.05 | 209.63 | 31.8 | 7.14 | 1100 | 1892 | 1602 | 6.66 |
| 1145 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.7 | 421.04 | 3.42 | 209.78 | 42.5 | 0.04 | 1053 | 1844 | 1619 | 6.60 |
| 1146 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 263.26 | 6.02 | 210.80 | 31.8 | 7.14 | 1100 | 1892 | 1970 | 7.36 |
| 1147 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 490.50 | 0.92 | 209.50 | 42.5 | 0.04 | 1061 | 1852 | 1859 | 7.07 |
| 1148 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 484.07 | 5.24 | 209.57 | 31.8 | 7.14 | 1092 | 1884 | 1742 | 6.91 |
| 1149 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 493.41 | 3.41 | 209.68 | 42.5 | 0.04 | 1053 | 1844 | 1722 | 6.79 |
| 1150 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 461.65 | 0.92 | 211.40 | 42.5 | 0.04 | 1061 | 1852 | 1652 | 6.67 |
| 1151 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 493.79 | 1.17 | 210.60 | 42.5 | 0.04 | 1069 | 1861 | 2303 | 7.93 |
| 1152 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 496.44 | 3.38 | 210.65 | 42.5 | 0.04 | 1053 | 1844 | 2267 | 7.83 |
| 1153 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 494.97 | 1.17 | 210.15 | 42.5 | 0.04 | 1069 | 1861 | 1850 | 7.07 |
| 1154 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 498.95 | 0.92 | 211.17 | 42.5 | 0.04 | 1061 | 1852 | 2299 | 7.91 |
| 1155 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 266.00 | 20.10 | 212.27 | 31.8 | 7.14 | 1084 | 1876 | 1657 | 6.73 |
| 1156 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 270.80 | 20.26 | 212.19 | 31.8 | 7.14 | 1084 | 1876 | 1656 | 6.73 |
| 1157 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 443.76 | 1.18 | 212.34 | 42.5 | 0.04 | 1069 | 1861 | 1711 | 6.80 |
| 1158 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 270.81 | 20.17 | 211.92 | 31.8 | 7.14 | 1084 | 1876 | 2296 | 7.94 |
| 1159 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 356.42 | 5.27 | 212.13 | 31.8 | 7.14 | 1092 | 1884 | 1635 | 6.70 |
| 1160 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 488.07 | 3.39 | 212.20 | 42.5 | 0.04 | 1053 | 1844 | 1661 | 6.68 |
| 1161 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 357.26 | 6.08 | 211.99 | 31.8 | 7.14 | 1100 | 1892 | 2147 | 7.69 |
| 1162 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 274.85 | 6.08 | 212.34 | 31.8 | 7.14 | 1100 | 1892 | 1681 | 6.81 |
| 1163 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 343.87 | 5.25 | 213.25 | 31.8 | 7.14 | 1092 | 1884 | 2264 | 7.90 |
| 1164 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 490.55 | 6.08 | 211.40 | 31.8 | 7.14 | 1100 | 1892 | 1728 | 6.89 |
| 1165 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 500.67 | 0.91 | 214.61 | 42.5 | 0.04 | 1061 | 1852 | 2211 | 7.74 |
| 1166 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 281.23 | 20.23 | 212.97 | 31.8 | 7.14 | 1084 | 1876 | 1710 | 6.83 |
| 1167 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 508.10 | 3.40 | 213.74 | 42.5 | 0.04 | 1053 | 1844 | 2142 | 7.59 |
| 1168 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 269.91 | 20.20 | 214.13 | 31.8 | 7.14 | 1084 | 1876 | 2177 | 7.72 |
| 1169 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 485.89 | 1.18 | 214.14 | 42.5 | 0.04 | 1069 | 1861 | 1801 | 6.97 |
| 1170 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 346.83 | 6.09 | 214.20 | 31.8 | 7.14 | 1100 | 1892 | 2154 | 7.71 |
| 1171 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.6 | 512.03 | 0.92 | 214.82 | 42.5 | 0.04 | 1061 | 1852 | 1772 | 6.90 |
| 1172 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 276.47 | 20.27 | 216.17 | 31.8 | 7.14 | 1084 | 1876 | 2233 | 7.82 |
| 1173 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 4.5 | 511.14 | 1.18 | 216.54 | 42.5 | 0.04 | 1069 | 1861 | 2214 | 7.76 |
| 1174 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 351.73 | 5.22 | 218.78 | 31.8 | 7.14 | 1092 | 1884 | 2141 | 7.67 |
| 1175 | gpu: heightmap + columns + heightfield + direct (incorrect) | 0.42 | 158.0 | 14.08 | 5.59 | 0.03 | 15.3 | 23.79 | 28 | 78 | 1160 | 2.36 |
| 1176 | cpu: heightmap + columns + heightfield + direct (incorrect) | 0.36 | 134.8 | 13.64 | - | 6.96 | 15.3 | 23.79 | - | - | 654 | 1.25 |
| 1177 | gpu: heightmap + columns + heightfield + pathtrace (incorrect) | 0.08 | 28.2 | 53.23 | 34.76 | 0.11 | 15.1 | 27.02 | 60 | 110 | 1195 | 2.49 |
| 1178 | gpu: naive@cpu + rayquery + raycast + direct | FAILED: [gpu: naive@cpu + rayquery + raycast + direct]: worker error: the device is out of memory (NVIDIA GeForce RTX 5060 Laptop GPU, this renderer seen holding up to 6923 MiB; the world's size and the resolution set most of it: lower the view radius or the resolution, or the storage's pool or slot knobs): wgpu error: Out of Memory | | | | | | | | | | |
| 1179 | gpu: naive@cpu + rayquery + raycast + pathtrace | FAILED: [gpu: naive@cpu + rayquery + raycast + pathtrace]: worker error: the device is out of memory (NVIDIA GeForce RTX 5060 Laptop GPU, this renderer seen holding up to 6954 MiB; the world's size and the resolution set most of it: lower the view radius or the resolution, or the storage's pool or slot knobs): wgpu error: Out of Memory | | | | | | | | | | |

### far [unlit]

1920×1080. camera 4.78 blocks/frame (max 6.1), turn 0.52°/frame; 9141 chunks at start, 35.3 loads/frame; chunk fill 8%, solid 5.8%, surface 0.44%; sky 69%, hits at 465 blocks (median 455); 0.0 edits/frame; 0 lights; 0 mesh triangles.

| # | renderer | rel | fps | frame p99 ms | gpu ms | cpu ms | psnr | bad pct | gpu mb | gpu device mb | peak rss mb | mem rel |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu: bricks + refloor + composite + raycast + pathtrace ◆ | 1.00 | 400.5 | 13.08 | 0.88 | 0.10 | 57.7 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 2 | gpu: tree64 + refloor + composite + raycast + pathtrace ◆ | 0.96 | 385.1 | 12.81 | 0.97 | 1.14 | 57.7 | 0.02 | 223 | 515 | 1073 | 2.93 |
| 3 | gpu: bricks + refloor + beam + composite + raycast + pathtrace | 0.95 | 381.9 | 13.32 | 1.13 | 0.09 | 57.8 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 4 | gpu: bricks + incremental + composite + raycast + pathtrace | 0.89 | 356.1 | 11.91 | 1.46 | 0.08 | 57.7 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 5 | gpu: bricks + rowrun + composite + raycast + pathtrace | 0.86 | 343.6 | 12.23 | 1.51 | 0.10 | 57.7 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 6 | gpu: tree64 + refloor + beam + composite + raycast + pathtrace ◆ | 0.83 | 332.3 | 13.26 | 1.60 | 1.15 | 57.8 | 0.02 | 223 | 460 | 1073 | 2.83 |
| 7 | gpu: prefix + refloor + composite + raycast + pathtrace | 0.82 | 329.8 | 19.93 | 0.77 | 0.09 | 57.7 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 8 | gpu: clipmap + refloor + composite + raycast + pathtrace | 0.82 | 329.6 | 14.55 | 1.41 | 0.03 | 57.7 | 0.02 | 1271 | 1643 | 1021 | 4.92 |
| 9 | gpu: bricks + incremental + beam + composite + raycast + pathtrace | 0.82 | 326.6 | 12.44 | 1.70 | 0.10 | 57.8 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 10 | gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 0.81 | 322.9 | 12.31 | 1.73 | 0.10 | 57.8 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 11 | gpu: hier + refloor + composite + raycast + pathtrace | 0.80 | 321.7 | 19.84 | 0.81 | 0.09 | 57.7 | 0.02 | 1077 | 1894 | 913 | 5.19 |
| 12 | gpu: prefix + refloor + beam + composite + raycast + pathtrace | 0.80 | 320.6 | 19.25 | 1.00 | 0.09 | 57.8 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 13 | gpu: tree64 + incremental + composite + raycast + pathtrace | 0.79 | 317.7 | 12.00 | 1.88 | 1.13 | 57.7 | 0.02 | 223 | 665 | 1078 | 3.22 |
| 14 | gpu: hier + refloor + beam + composite + raycast + pathtrace | 0.78 | 312.3 | 19.10 | 1.05 | 0.09 | 57.8 | 0.02 | 1077 | 1894 | 914 | 5.19 |
| 15 | gpu: prefix + rowrun + composite + raycast + pathtrace | 0.77 | 307.6 | 18.58 | 1.15 | 0.09 | 57.7 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 16 | gpu: rows + refloor + composite + raycast + pathtrace | 0.76 | 305.8 | 21.68 | 0.89 | 0.09 | 57.7 | 0.02 | 1140 | 1926 | 907 | 5.24 |
| 17 | gpu: bricks + sphere + composite + raycast + pathtrace | 0.76 | 302.5 | 12.25 | 2.06 | 0.09 | 57.7 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 18 | gpu: clipmap + rowrun + composite + raycast + pathtrace | 0.75 | 298.8 | 12.25 | 2.00 | 0.03 | 57.7 | 0.02 | 1271 | 1643 | 1017 | 4.91 |
| 19 | gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 0.74 | 295.2 | 17.89 | 1.36 | 0.10 | 57.8 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 20 | gpu: svo_dag + refloor + composite + raycast + pathtrace ◆ | 0.74 | 295.1 | 24.82 | 0.92 | 2.05 | 57.7 | 0.02 | 77 | 176 | 1191 | 2.53 |
| 21 | gpu: clipmap + incremental + composite + raycast + pathtrace | 0.73 | 293.5 | 12.37 | 1.98 | 0.03 | 57.7 | 0.02 | 1271 | 1643 | 1017 | 4.91 |
| 22 | gpu: rows + rowrun + composite + raycast + pathtrace | 0.73 | 291.3 | 21.90 | 1.16 | 0.09 | 57.7 | 0.02 | 1140 | 1926 | 907 | 5.24 |
| 23 | gpu: bricks + sphere + beam + composite + raycast + pathtrace | 0.73 | 290.8 | 12.52 | 2.29 | 0.10 | 57.8 | 0.02 | 1076 | 1894 | 904 | 5.17 |
| 24 | gpu: rows + refloor + beam + composite + raycast + pathtrace | 0.72 | 289.7 | 22.21 | 1.13 | 0.10 | 57.8 | 0.02 | 1140 | 1926 | 908 | 5.24 |
| 25 | gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 0.72 | 286.5 | 11.41 | 2.33 | 1.15 | 57.8 | 0.02 | 223 | 463 | 1075 | 2.84 |
| 26 | gpu: prefix + incremental + composite + raycast + pathtrace | 0.71 | 282.4 | 16.69 | 1.71 | 0.09 | 57.7 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 27 | gpu: rows + rowrun + beam + composite + raycast + pathtrace | 0.69 | 277.8 | 19.63 | 1.39 | 0.10 | 57.8 | 0.02 | 1140 | 1926 | 908 | 5.24 |
| 28 | gpu: hier + incremental + composite + raycast + pathtrace | 0.69 | 277.7 | 18.42 | 1.62 | 0.09 | 57.7 | 0.02 | 1077 | 1894 | 914 | 5.19 |
| 29 | gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 0.68 | 273.5 | 12.38 | 2.27 | 0.03 | 57.8 | 0.02 | 1271 | 1643 | 1017 | 4.91 |
| 30 | gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 0.68 | 271.6 | 10.84 | 2.51 | 0.17 | 50.2 | 0.02 | 1274 | 2126 | 911 | 5.61 |
| 31 | gpu: bricks + refloor + quadtree + raster + pathtrace | 0.68 | 271.5 | 10.79 | 2.50 | 0.16 | 50.2 | 0.02 | 1258 | 2109 | 912 | 5.58 |
| 32 | gpu: prefix + incremental + beam + composite + raycast + pathtrace | 0.66 | 265.2 | 16.28 | 1.92 | 0.10 | 57.8 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 33 | gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 0.66 | 265.1 | 8.80 | 2.63 | 0.17 | 50.2 | 0.02 | 1266 | 2118 | 910 | 5.59 |
| 34 | gpu: hier + incremental + beam + composite + raycast + pathtrace | 0.66 | 263.6 | 16.64 | 1.83 | 0.10 | 57.8 | 0.02 | 1077 | 1894 | 914 | 5.19 |
| 35 | gpu: rows + incremental + beam + composite + raycast + pathtrace | 0.65 | 260.2 | 16.72 | 1.78 | 0.09 | 57.8 | 0.02 | 1140 | 1926 | 908 | 5.24 |
| 36 | gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 0.65 | 259.2 | 24.62 | 1.59 | 2.08 | 57.8 | 0.02 | 77 | 176 | 1192 | 2.53 |
| 37 | gpu: hier + sphere + composite + raycast + pathtrace | 0.65 | 258.8 | 18.45 | 1.87 | 0.09 | 57.7 | 0.02 | 1077 | 1894 | 913 | 5.19 |
| 38 | gpu: prefix + sphere + composite + raycast + pathtrace | 0.64 | 258.1 | 16.29 | 2.06 | 0.09 | 57.7 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 39 | gpu: hier + rowrun + composite + raycast + pathtrace | 0.64 | 257.9 | 16.28 | 2.00 | 0.10 | 57.7 | 0.02 | 1077 | 1894 | 914 | 5.19 |
| 40 | gpu: hier + sphere + beam + composite + raycast + pathtrace | 0.64 | 256.5 | 15.35 | 2.11 | 0.10 | 57.8 | 0.02 | 1077 | 1894 | 914 | 5.19 |
| 41 | gpu: rows + incremental + composite + raycast + pathtrace | 0.64 | 254.5 | 19.99 | 1.68 | 0.09 | 57.7 | 0.02 | 1140 | 1926 | 907 | 5.24 |
| 42 | gpu: tree64 + sphere + composite + raycast + pathtrace | 0.63 | 253.8 | 10.84 | 2.97 | 1.14 | 57.7 | 0.02 | 223 | 478 | 1077 | 2.87 |
| 43 | gpu: tree64 + refloor + quadtree + raster + pathtrace | 0.63 | 253.2 | 11.29 | 2.84 | 1.21 | 50.2 | 0.02 | 405 | 725 | 1123 | 3.42 |
| 44 | gpu: prefix + sphere + beam + composite + raycast + pathtrace | 0.63 | 251.3 | 14.62 | 2.28 | 0.10 | 57.8 | 0.02 | 1157 | 1939 | 908 | 5.26 |
| 45 | gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 0.63 | 251.0 | 11.09 | 2.87 | 1.23 | 50.2 | 0.02 | 421 | 846 | 1124 | 3.64 |
| 46 | gpu: distance + sphere + composite + raycast + pathtrace | 0.63 | 250.6 | 31.09 | 0.64 | 0.09 | 57.7 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 47 | gpu: rows + sphere + composite + raycast + pathtrace | 0.63 | 250.3 | 15.78 | 2.08 | 0.09 | 57.7 | 0.02 | 1140 | 1926 | 907 | 5.24 |
| 48 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 0.62 | 248.8 | 10.50 | 2.91 | 0.17 | 50.7 | 0.02 | 1258 | 2109 | 915 | 5.59 |
| 49 | gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 0.62 | 247.3 | 11.12 | 2.91 | 1.23 | 50.2 | 0.02 | 413 | 806 | 1124 | 3.57 |
| 50 | gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 0.61 | 246.2 | 13.18 | 2.82 | 0.03 | 57.8 | 0.02 | 1271 | 1643 | 1017 | 4.91 |
| 51 | gpu: clipmap + sphere + composite + raycast + pathtrace | 0.61 | 245.5 | 13.35 | 2.80 | 0.03 | 57.7 | 0.02 | 1271 | 1643 | 1016 | 4.91 |
| 52 | gpu: hier + rowrun + beam + composite + raycast + pathtrace | 0.61 | 244.3 | 16.34 | 2.25 | 0.10 | 57.8 | 0.02 | 1077 | 1894 | 914 | 5.19 |
| 53 | gpu: distance + refloor + composite + raycast + pathtrace | 0.61 | 243.9 | 30.74 | 0.78 | 0.09 | 57.7 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 54 | gpu: prefix + refloor + quadtree + raster + pathtrace | 0.61 | 243.2 | 12.52 | 2.39 | 0.16 | 50.2 | 0.02 | 1339 | 2154 | 918 | 5.68 |
| 55 | gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 0.60 | 240.3 | 11.33 | 3.05 | 0.11 | 50.2 | 0.02 | 1469 | 1877 | 1031 | 5.37 |
| 56 | gpu: distance + sphere + beam + composite + raycast + pathtrace | 0.60 | 240.1 | 30.16 | 0.88 | 0.09 | 57.8 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 57 | gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 0.60 | 239.9 | 14.85 | 2.96 | 0.03 | 57.8 | 0.02 | 1271 | 1643 | 1021 | 4.92 |
| 58 | gpu: clipmap + refloor + quadtree + raster + pathtrace | 0.60 | 239.3 | 11.99 | 3.00 | 0.10 | 50.2 | 0.02 | 1453 | 1860 | 1032 | 5.34 |
| 59 | gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 0.60 | 239.2 | 12.07 | 3.05 | 0.11 | 50.2 | 0.02 | 1461 | 1869 | 1031 | 5.36 |
| 60 | gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 0.59 | 237.8 | 14.25 | 2.41 | 0.18 | 50.2 | 0.02 | 1354 | 2170 | 917 | 5.71 |
| 61 | gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 0.59 | 237.5 | 14.24 | 2.42 | 0.17 | 50.2 | 0.02 | 1346 | 2162 | 917 | 5.69 |
| 62 | gpu: bricks + incremental + quadtree + raster + pathtrace | 0.59 | 237.4 | 10.08 | 3.37 | 0.17 | 50.2 | 0.02 | 1258 | 2109 | 918 | 5.59 |
| 63 | gpu: hier + refloor + quadtree + gbuffer + pathtrace | 0.59 | 236.0 | 14.72 | 2.44 | 0.17 | 50.2 | 0.02 | 1274 | 2126 | 919 | 5.63 |
| 64 | gpu: hier + refloor + quadtree + visbuffer + pathtrace | 0.59 | 235.4 | 15.78 | 2.44 | 0.16 | 50.2 | 0.02 | 1267 | 2118 | 920 | 5.61 |
| 65 | gpu: rows + sphere + beam + composite + raycast + pathtrace | 0.59 | 235.3 | 16.74 | 2.31 | 0.10 | 57.8 | 0.02 | 1140 | 1926 | 908 | 5.24 |
| 66 | gpu: distance + refloor + beam + composite + raycast + pathtrace | 0.58 | 233.8 | 29.87 | 1.01 | 0.09 | 57.8 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 67 | gpu: hier + refloor + quadtree + raster + pathtrace | 0.58 | 233.3 | 14.74 | 2.40 | 0.17 | 50.2 | 0.02 | 1259 | 2110 | 920 | 5.60 |
| 68 | gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 0.58 | 232.0 | 9.83 | 3.40 | 0.18 | 50.1 | 0.02 | 1266 | 2118 | 920 | 5.61 |
| 69 | gpu: bricks + rowrun + quadtree + raster + pathtrace | 0.58 | 231.7 | 9.58 | 3.47 | 0.16 | 50.2 | 0.02 | 1258 | 2109 | 921 | 5.60 |
| 70 | gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 0.57 | 229.8 | 10.08 | 3.52 | 0.17 | 50.2 | 0.02 | 1266 | 2118 | 920 | 5.61 |
| 71 | gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 0.57 | 229.2 | 10.04 | 3.60 | 1.16 | 57.8 | 0.02 | 223 | 470 | 1074 | 2.85 |
| 72 | gpu: distance + rowrun + composite + raycast + pathtrace | 0.57 | 229.0 | 29.81 | 1.22 | 0.09 | 57.7 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 73 | gpu: svo_dag + sphere + composite + raycast + pathtrace | 0.57 | 226.8 | 22.65 | 2.39 | 2.06 | 57.7 | 0.02 | 77 | 176 | 1191 | 2.53 |
| 74 | gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 0.57 | 226.4 | 11.37 | 3.49 | 0.18 | 50.2 | 0.02 | 1274 | 2126 | 920 | 5.63 |
| 75 | gpu: rows + refloor + quadtree + gbuffer + pathtrace | 0.56 | 225.2 | 16.46 | 2.51 | 0.17 | 50.2 | 0.02 | 1338 | 2158 | 917 | 5.68 |
| 76 | gpu: rows + refloor + quadtree + raster + pathtrace | 0.56 | 225.2 | 15.60 | 2.48 | 0.16 | 50.2 | 0.02 | 1322 | 2141 | 918 | 5.65 |
| 77 | gpu: prefix + rowrun + quadtree + raster + pathtrace | 0.56 | 224.5 | 15.01 | 2.89 | 0.17 | 50.2 | 0.02 | 1339 | 2154 | 919 | 5.68 |
| 78 | gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 0.56 | 223.3 | 10.45 | 3.60 | 0.17 | 50.2 | 0.02 | 1274 | 2126 | 921 | 5.63 |
| 79 | gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 0.56 | 222.5 | 10.96 | 3.69 | 0.18 | 50.2 | 0.02 | 1274 | 2126 | 920 | 5.63 |
| 80 | gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 0.55 | 221.8 | 15.25 | 2.92 | 0.18 | 50.2 | 0.02 | 1354 | 2170 | 918 | 5.71 |
| 81 | gpu: rows + refloor + quadtree + visbuffer + pathtrace | 0.55 | 221.2 | 15.80 | 2.51 | 0.17 | 50.2 | 0.02 | 1330 | 2150 | 917 | 5.67 |
| 82 | gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 0.55 | 221.1 | 10.17 | 3.69 | 0.17 | 50.1 | 0.02 | 1266 | 2118 | 917 | 5.61 |
| 83 | gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 0.55 | 221.0 | 13.35 | 2.92 | 0.18 | 50.2 | 0.02 | 1346 | 2162 | 917 | 5.69 |
| 84 | gpu: bricks + sphere + quadtree + raster + pathtrace | 0.55 | 220.6 | 10.58 | 3.67 | 0.17 | 50.2 | 0.02 | 1258 | 2109 | 920 | 5.60 |
| 85 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 0.55 | 219.8 | 17.17 | 2.97 | 0.18 | 50.7 | 0.02 | 1266 | 2118 | 915 | 5.60 |
| 86 | gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 0.55 | 218.3 | 12.76 | 3.75 | 0.03 | 57.8 | 0.02 | 1271 | 1643 | 1021 | 4.92 |
| 87 | gpu: distance + rowrun + beam + composite + raycast + pathtrace | 0.54 | 217.7 | 29.24 | 1.44 | 0.10 | 57.8 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 88 | gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 0.54 | 216.7 | 16.36 | 2.83 | 0.17 | 50.2 | 0.02 | 1338 | 2158 | 917 | 5.68 |
| 89 | gpu: clipmap + rowrun + quadtree + raster + pathtrace | 0.54 | 214.8 | 11.01 | 3.66 | 0.10 | 50.2 | 0.02 | 1453 | 1860 | 1031 | 5.34 |
| 90 | gpu: rows + rowrun + quadtree + raster + pathtrace | 0.53 | 214.2 | 15.11 | 2.80 | 0.17 | 50.2 | 0.02 | 1322 | 2141 | 918 | 5.65 |
| 91 | gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 0.53 | 214.2 | 12.06 | 3.70 | 0.11 | 50.2 | 0.02 | 1469 | 1877 | 1035 | 5.38 |
| 92 | gpu: tree64 + incremental + quadtree + raster + pathtrace | 0.53 | 213.8 | 10.57 | 3.92 | 1.22 | 50.2 | 0.02 | 405 | 704 | 1124 | 3.38 |
| 93 | gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 0.53 | 213.2 | 11.38 | 3.71 | 0.11 | 50.2 | 0.02 | 1461 | 1869 | 1035 | 5.37 |
| 94 | gpu: distance + incremental + composite + raycast + pathtrace | 0.53 | 212.5 | 28.69 | 1.59 | 0.09 | 57.7 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 95 | gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 0.53 | 212.3 | 11.41 | 3.82 | 0.11 | 50.1 | 0.02 | 1461 | 1869 | 1036 | 5.37 |
| 96 | gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 0.53 | 212.1 | 9.82 | 3.96 | 1.24 | 50.1 | 0.02 | 413 | 674 | 1123 | 3.32 |
| 97 | gpu: hier + incremental + quadtree + raster + pathtrace | 0.53 | 211.8 | 15.67 | 3.25 | 0.17 | 50.2 | 0.02 | 1259 | 2110 | 920 | 5.60 |
| 98 | gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 0.53 | 211.2 | 13.97 | 2.84 | 0.17 | 50.2 | 0.02 | 1330 | 2150 | 917 | 5.67 |
| 99 | gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 0.53 | 210.5 | 12.25 | 3.82 | 0.11 | 50.2 | 0.02 | 1469 | 1877 | 1035 | 5.38 |
| 100 | gpu: hier + incremental + quadtree + gbuffer + pathtrace | 0.52 | 209.5 | 15.15 | 3.27 | 0.17 | 50.2 | 0.02 | 1274 | 2126 | 920 | 5.63 |
| 101 | gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 0.52 | 209.4 | 9.92 | 4.01 | 1.23 | 50.2 | 0.02 | 421 | 683 | 1124 | 3.34 |
| 102 | gpu: clipmap + incremental + quadtree + raster + pathtrace | 0.52 | 208.9 | 13.02 | 3.80 | 0.11 | 50.2 | 0.02 | 1453 | 1860 | 1036 | 5.35 |
| 103 | gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 0.52 | 208.3 | 22.32 | 3.00 | 2.08 | 57.8 | 0.02 | 77 | 176 | 1192 | 2.53 |
| 104 | gpu: distance + incremental + beam + composite + raycast + pathtrace | 0.52 | 207.8 | 24.80 | 1.83 | 0.10 | 57.8 | 0.02 | 1588 | 2262 | 908 | 5.86 |
| 105 | gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 0.52 | 207.7 | 14.65 | 3.40 | 0.18 | 50.2 | 0.02 | 1354 | 2170 | 918 | 5.71 |
| 106 | gpu: prefix + incremental + quadtree + raster + pathtrace | 0.52 | 206.9 | 15.83 | 3.38 | 0.17 | 50.2 | 0.02 | 1339 | 2154 | 918 | 5.68 |
| 107 | gpu: rows + incremental + quadtree + raster + pathtrace | 0.52 | 206.3 | 17.18 | 3.16 | 0.16 | 50.2 | 0.02 | 1322 | 2141 | 918 | 5.65 |
| 108 | gpu: hier + incremental + quadtree + visbuffer + pathtrace | 0.51 | 205.3 | 16.06 | 3.28 | 0.18 | 50.1 | 0.02 | 1267 | 2118 | 919 | 5.61 |
| 109 | gpu: svo_dag + incremental + composite + raycast + pathtrace | 0.51 | 205.3 | 22.63 | 3.12 | 2.07 | 57.7 | 0.02 | 77 | 176 | 1191 | 2.53 |
| 110 | gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 0.51 | 205.1 | 15.07 | 3.40 | 0.18 | 50.1 | 0.02 | 1346 | 2162 | 917 | 5.69 |
| 111 | gpu: tree64 + rowrun + composite + raycast + pathtrace | 0.51 | 205.0 | 9.07 | 4.27 | 1.15 | 57.7 | 0.02 | 223 | 464 | 1074 | 2.84 |
| 112 | gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 0.51 | 204.1 | 22.33 | 2.97 | 2.17 | 50.2 | 0.02 | 275 | 410 | 1212 | 3.00 |
| 113 | gpu: svo_dag + refloor + quadtree + raster + pathtrace | 0.51 | 203.7 | 23.16 | 2.96 | 2.15 | 50.2 | 0.02 | 259 | 393 | 1214 | 2.97 |
| 114 | gpu: rows + incremental + quadtree + visbuffer + pathtrace | 0.51 | 202.8 | 14.14 | 3.19 | 0.18 | 50.1 | 0.02 | 1330 | 2150 | 917 | 5.67 |
| 115 | gpu: rows + incremental + quadtree + gbuffer + pathtrace | 0.50 | 201.7 | 17.43 | 3.19 | 0.18 | 50.2 | 0.02 | 1338 | 2158 | 919 | 5.69 |
| 116 | gpu: boxes@cpu + rayquery + raycast + pathtrace | 0.50 | 201.3 | 24.05 | 1.58 | 1.81 | 57.7 | 0.02 | 1169 | 1865 | 1603 | 6.41 |
| 117 | gpu: hier + sphere + quadtree + raster + pathtrace | 0.50 | 201.0 | 14.66 | 3.52 | 0.17 | 50.2 | 0.02 | 1259 | 2110 | 921 | 5.60 |
| 118 | gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 0.50 | 198.4 | 22.66 | 3.09 | 2.17 | 50.2 | 0.02 | 267 | 401 | 1213 | 2.98 |
| 119 | gpu: hier + sphere + quadtree + gbuffer + pathtrace | 0.50 | 198.3 | 15.47 | 3.55 | 0.18 | 50.2 | 0.02 | 1274 | 2126 | 920 | 5.63 |
| 120 | gpu: hier + sphere + quadtree + visbuffer + pathtrace | 0.49 | 196.5 | 15.74 | 3.56 | 0.18 | 50.1 | 0.02 | 1267 | 2118 | 922 | 5.62 |
| 121 | gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 0.49 | 196.1 | 16.23 | 3.80 | 0.18 | 50.2 | 0.02 | 1354 | 2170 | 919 | 5.71 |
| 122 | gpu: prefix + sphere + quadtree + raster + pathtrace | 0.49 | 195.8 | 15.14 | 3.77 | 0.18 | 50.2 | 0.02 | 1339 | 2154 | 918 | 5.68 |
| 123 | gpu: hier + rowrun + quadtree + raster + pathtrace | 0.49 | 195.4 | 15.11 | 3.74 | 0.17 | 50.2 | 0.02 | 1259 | 2110 | 924 | 5.60 |
| 124 | gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 0.49 | 194.6 | 22.36 | 3.47 | 2.10 | 57.8 | 0.02 | 77 | 176 | 1191 | 2.53 |
| 125 | gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 0.48 | 194.0 | 15.26 | 3.78 | 0.18 | 50.2 | 0.02 | 1267 | 2118 | 924 | 5.62 |
| 126 | gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 0.48 | 193.7 | 15.57 | 3.80 | 0.18 | 50.1 | 0.02 | 1346 | 2162 | 917 | 5.69 |
| 127 | gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 0.48 | 193.5 | 11.91 | 4.47 | 0.11 | 50.2 | 0.02 | 1469 | 1877 | 1036 | 5.38 |
| 128 | gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 0.48 | 193.5 | 15.48 | 3.78 | 0.18 | 50.2 | 0.02 | 1274 | 2126 | 924 | 5.64 |
| 129 | gpu: distance + sphere + quadtree + raster + pathtrace | 0.48 | 192.7 | 23.89 | 2.23 | 0.16 | 50.2 | 0.02 | 1770 | 2477 | 918 | 6.27 |
| 130 | gpu: distance + sphere + quadtree + gbuffer + pathtrace | 0.48 | 192.5 | 23.84 | 2.25 | 0.17 | 50.2 | 0.02 | 1786 | 2494 | 917 | 6.30 |
| 131 | gpu: rows + sphere + quadtree + raster + pathtrace | 0.48 | 192.0 | 16.14 | 3.66 | 0.17 | 50.2 | 0.02 | 1322 | 2141 | 922 | 5.66 |
| 132 | gpu: svo_dag + rowrun + composite + raycast + pathtrace | 0.48 | 191.8 | 22.06 | 3.60 | 2.05 | 57.7 | 0.02 | 77 | 176 | 1191 | 2.53 |
| 133 | gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 0.48 | 191.7 | 11.98 | 4.52 | 0.12 | 50.1 | 0.02 | 1461 | 1869 | 1036 | 5.37 |
| 134 | gpu: clipmap + sphere + quadtree + raster + pathtrace | 0.48 | 190.4 | 12.91 | 4.48 | 0.11 | 50.2 | 0.02 | 1453 | 1860 | 1037 | 5.35 |
| 135 | gpu: distance + sphere + quadtree + visbuffer + pathtrace | 0.48 | 190.2 | 23.43 | 2.26 | 0.17 | 50.1 | 0.02 | 1778 | 2486 | 917 | 6.29 |
| 136 | gpu: rows + sphere + quadtree + visbuffer + pathtrace | 0.47 | 189.7 | 16.34 | 3.70 | 0.18 | 50.1 | 0.02 | 1330 | 2150 | 917 | 5.67 |
| 137 | gpu: distance + refloor + quadtree + visbuffer + pathtrace | 0.47 | 189.5 | 23.67 | 2.44 | 0.17 | 50.1 | 0.02 | 1778 | 2486 | 917 | 6.29 |
| 138 | gpu: distance + refloor + quadtree + gbuffer + pathtrace | 0.47 | 188.5 | 21.90 | 2.44 | 0.18 | 50.2 | 0.02 | 1786 | 2494 | 917 | 6.30 |
| 139 | gpu: distance + refloor + quadtree + raster + pathtrace | 0.47 | 188.0 | 20.50 | 2.41 | 0.17 | 50.2 | 0.02 | 1770 | 2477 | 918 | 6.27 |
| 140 | gpu: rows + sphere + quadtree + gbuffer + pathtrace | 0.47 | 187.0 | 16.66 | 3.70 | 0.17 | 50.2 | 0.02 | 1338 | 2158 | 919 | 5.69 |
| 141 | gpu: tree64 + sphere + quadtree + raster + pathtrace | 0.47 | 186.9 | 9.43 | 4.79 | 1.24 | 50.2 | 0.02 | 405 | 710 | 1125 | 3.39 |
| 142 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.47 | 186.7 | 25.51 | 1.45 | 2.50 | 53.2 | 0.02 | 1334 | 1872 | 1134 | 5.55 |
| 143 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.46 | 185.4 | 25.57 | 1.51 | 2.49 | 53.2 | 0.02 | 1350 | 1889 | 1134 | 5.58 |
| 144 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.46 | 184.9 | 25.78 | 1.52 | 2.52 | 53.2 | 0.02 | 1342 | 1880 | 1134 | 5.57 |
| 145 | gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 0.46 | 184.2 | 9.84 | 4.82 | 1.27 | 50.1 | 0.02 | 413 | 916 | 1122 | 3.77 |
| 146 | gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 0.46 | 184.1 | 9.26 | 4.81 | 1.26 | 50.2 | 0.02 | 421 | 966 | 1122 | 3.86 |
| 147 | gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 0.46 | 184.1 | 9.60 | 4.91 | 1.17 | 57.8 | 0.02 | 223 | 491 | 1075 | 2.89 |
| 148 | gpu: distance + rowrun + quadtree + raster + pathtrace | 0.45 | 180.3 | 22.08 | 2.82 | 0.17 | 50.2 | 0.02 | 1770 | 2477 | 918 | 6.27 |
| 149 | gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 0.44 | 177.2 | 21.37 | 4.19 | 2.10 | 57.8 | 0.02 | 77 | 176 | 1196 | 2.54 |
| 150 | gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 0.44 | 177.1 | 23.67 | 2.86 | 0.18 | 50.2 | 0.02 | 1786 | 2494 | 918 | 6.30 |
| 151 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 0.44 | 176.2 | 31.32 | 2.97 | 0.21 | 50.7 | 0.02 | 1274 | 2126 | 926 | 5.64 |
| 152 | gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 0.44 | 175.8 | 22.46 | 2.87 | 0.18 | 50.1 | 0.02 | 1778 | 2486 | 941 | 6.33 |
| 153 | gpu: distance + incremental + quadtree + raster + pathtrace | 0.42 | 167.1 | 23.91 | 3.53 | 0.17 | 50.2 | 0.02 | 1770 | 2477 | 977 | 6.38 |
| 154 | gpu: svo_dag + sphere + quadtree + raster + pathtrace | 0.42 | 166.6 | 22.97 | 4.53 | 2.18 | 50.2 | 0.02 | 259 | 393 | 1213 | 2.97 |
| 155 | gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 0.41 | 165.5 | 22.64 | 4.54 | 2.19 | 50.2 | 0.02 | 275 | 410 | 1213 | 3.00 |
| 156 | gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 0.41 | 164.3 | 22.75 | 4.57 | 2.19 | 50.1 | 0.02 | 267 | 401 | 1213 | 2.98 |
| 157 | gpu: distance + incremental + quadtree + gbuffer + pathtrace | 0.41 | 164.3 | 23.75 | 3.56 | 0.18 | 50.2 | 0.02 | 1786 | 2494 | 974 | 6.41 |
| 158 | gpu: distance + incremental + quadtree + visbuffer + pathtrace | 0.41 | 163.3 | 24.55 | 3.56 | 0.19 | 50.1 | 0.02 | 1778 | 2486 | 977 | 6.40 |
| 159 | gpu: svo_dag + incremental + quadtree + raster + pathtrace | 0.38 | 151.5 | 22.59 | 5.27 | 2.16 | 50.2 | 0.02 | 259 | 393 | 1213 | 2.97 |
| 160 | gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 0.38 | 150.9 | 22.80 | 5.29 | 2.19 | 50.1 | 0.02 | 267 | 401 | 1214 | 2.99 |
| 161 | gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 0.38 | 150.4 | 22.29 | 5.26 | 2.22 | 50.2 | 0.02 | 275 | 410 | 1214 | 3.00 |
| 162 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.34 | 137.4 | 39.51 | 0.63 | 4.96 | 51.8 | 0.03 | 1222 | 1889 | 1024 | 5.38 |
| 163 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.34 | 136.9 | 39.86 | 0.64 | 4.96 | 51.8 | 0.03 | 1214 | 1880 | 1024 | 5.37 |
| 164 | gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 0.34 | 136.3 | 22.24 | 6.17 | 2.18 | 50.2 | 0.02 | 259 | 393 | 1222 | 2.98 |
| 165 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.34 | 136.2 | 44.62 | 0.58 | 4.96 | 51.8 | 0.03 | 1206 | 1872 | 1024 | 5.35 |
| 166 | gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 0.34 | 135.6 | 22.46 | 6.20 | 2.19 | 50.2 | 0.02 | 267 | 401 | 1222 | 3.00 |
| 167 | gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 0.34 | 134.9 | 22.34 | 6.18 | 2.21 | 50.2 | 0.02 | 275 | 410 | 1221 | 3.01 |
| 168 | gpu: tree64 + rowrun + quadtree + raster + pathtrace | 0.31 | 123.3 | 10.72 | 7.84 | 1.28 | 50.2 | 0.02 | 405 | 682 | 1187 | 3.45 |
| 169 | gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 0.31 | 122.6 | 10.87 | 7.88 | 1.30 | 50.2 | 0.02 | 413 | 688 | 1189 | 3.47 |
| 170 | gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 0.31 | 122.5 | 11.83 | 7.87 | 1.29 | 50.2 | 0.02 | 421 | 679 | 1188 | 3.45 |
| 171 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.20 | 78.1 | 16.69 | 12.35 | 1.78 | 53.2 | 0.02 | 1334 | 1872 | 1065 | 5.43 |
| 172 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.19 | 77.7 | 16.73 | 12.41 | 1.81 | 53.2 | 0.02 | 1350 | 1889 | 1065 | 5.46 |
| 173 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.19 | 77.6 | 17.13 | 12.42 | 1.80 | 53.1 | 0.02 | 1342 | 1880 | 1066 | 5.44 |
| 174 | gpu: greedy@cpu + rayquery + raycast + pathtrace | 0.16 | 65.4 | 128.48 | 1.53 | 10.87 | 57.6 | 0.02 | 809 | 2557 | 2490 | 9.33 |
| 175 | gpu: points@cpu + splat + bricks + refloor + pathtrace | 0.16 | 64.7 | 83.87 | 3.53 | 0.10 | 57.7 | 0.02 | 1771 | 2226 | 1615 | 7.10 |
| 176 | cpu: hier + refloor + raycast + direct ◆ | 0.10 | 39.1 | 36.82 | - | 24.23 | 61.7 | 0.01 | - | - | 670 | 1.24 |
| 177 | cpu: prefix + incremental + raycast + direct | 0.09 | 37.4 | 37.29 | - | 25.32 | 61.7 | 0.01 | - | - | 892 | 1.65 |
| 178 | cpu: hier + incremental + raycast + direct | 0.09 | 37.4 | 37.11 | - | 25.40 | 61.7 | 0.01 | - | - | 670 | 1.24 |
| 179 | cpu: prefix + refloor + raycast + direct | 0.09 | 35.6 | 40.63 | - | 26.75 | 61.7 | 0.01 | - | - | 885 | 1.63 |
| 180 | cpu: bricks + refloor + raycast + direct ◆ | 0.09 | 35.4 | 37.93 | - | 27.40 | 61.7 | 0.01 | - | - | 635 | 1.17 |
| 181 | cpu: bits + refloor + raycast + direct | 0.09 | 34.5 | 39.91 | - | 27.68 | 61.7 | 0.01 | - | - | 670 | 1.24 |
| 182 | cpu: bricks + incremental + raycast + direct ◆ | 0.09 | 34.1 | 38.04 | - | 28.44 | 61.7 | 0.01 | - | - | 635 | 1.17 |
| 183 | cpu: u8 + refloor + raycast + direct ◆ | 0.09 | 34.1 | 38.40 | - | 28.44 | 61.7 | 0.01 | - | - | 552 | 1.02 |
| 184 | cpu: brickmajor + refloor + raycast + direct | 0.08 | 33.1 | 42.37 | - | 28.60 | 61.7 | 0.01 | - | - | 982 | 1.82 |
| 185 | cpu: bits + incremental + raycast + direct | 0.08 | 33.0 | 40.13 | - | 28.96 | 61.7 | 0.01 | - | - | 670 | 1.24 |
| 186 | cpu: u8 + incremental + raycast + direct | 0.08 | 32.9 | 39.29 | - | 29.44 | 61.7 | 0.01 | - | - | 568 | 1.05 |
| 187 | cpu: morton + refloor + raycast + direct | 0.08 | 32.8 | 42.42 | - | 28.71 | 61.7 | 0.01 | - | - | 998 | 1.84 |
| 188 | cpu: brickmajor + incremental + raycast + direct | 0.08 | 32.4 | 43.08 | - | 29.24 | 61.7 | 0.01 | - | - | 987 | 1.82 |
| 189 | cpu: morton + incremental + raycast + direct | 0.08 | 32.3 | 43.36 | - | 29.19 | 61.7 | 0.01 | - | - | 998 | 1.84 |
| 190 | cpu: palette + refloor + raycast + direct ◆ | 0.08 | 32.0 | 46.74 | - | 28.92 | 61.7 | 0.01 | - | - | 541 | 1.00 |
| 191 | cpu: palette + incremental + raycast + direct | 0.08 | 31.6 | 45.90 | - | 29.26 | 61.7 | 0.01 | - | - | 549 | 1.02 |
| 192 | cpu: greedy + faces + raster + bricks + refloor + direct | 0.05 | 18.7 | 80.75 | - | 52.50 | 66.7 | 0.00 | - | - | 1062 | 1.96 |
| 193 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.8 | 146.29 | 0.34 | 67.67 | 44.9 | 0.04 | 1084 | 1876 | 1775 | 6.75 |
| 194 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.4 | 151.15 | 0.43 | 69.50 | 44.9 | 0.04 | 1100 | 1892 | 1796 | 6.82 |
| 195 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.4 | 148.43 | 0.34 | 69.87 | 44.9 | 0.04 | 1084 | 1876 | 2327 | 7.77 |
| 196 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.3 | 154.89 | 0.40 | 70.25 | 44.9 | 0.04 | 1092 | 1884 | 1808 | 6.82 |
| 197 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 13.3 | 150.89 | 0.43 | 69.89 | 44.9 | 0.04 | 1100 | 1892 | 2279 | 7.71 |
| 198 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 160.11 | 0.35 | 73.77 | 44.9 | 0.04 | 1084 | 1876 | 1791 | 6.77 |
| 199 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.8 | 156.77 | 0.34 | 72.78 | 44.9 | 0.04 | 1084 | 1876 | 2367 | 7.84 |
| 200 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 162.07 | 0.34 | 74.91 | 44.9 | 0.04 | 1084 | 1876 | 2072 | 7.29 |
| 201 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.7 | 167.45 | 0.40 | 73.18 | 44.9 | 0.04 | 1092 | 1884 | 2282 | 7.70 |
| 202 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 156.48 | 0.34 | 73.49 | 44.9 | 0.04 | 1084 | 1876 | 2368 | 7.84 |
| 203 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 155.98 | 0.44 | 73.73 | 44.9 | 0.04 | 1100 | 1892 | 2365 | 7.87 |
| 204 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 159.46 | 0.43 | 73.59 | 44.9 | 0.04 | 1100 | 1892 | 2343 | 7.83 |
| 205 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.6 | 163.85 | 0.41 | 75.00 | 44.9 | 0.04 | 1092 | 1884 | 2088 | 7.34 |
| 206 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.5 | 161.10 | 0.40 | 74.02 | 44.9 | 0.04 | 1092 | 1884 | 2359 | 7.84 |
| 207 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.5 | 161.86 | 0.44 | 75.34 | 44.9 | 0.04 | 1100 | 1892 | 2051 | 7.29 |
| 208 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.5 | 160.91 | 0.40 | 74.35 | 44.9 | 0.04 | 1092 | 1884 | 2364 | 7.85 |
| 209 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.4 | 163.77 | 0.41 | 75.90 | 44.9 | 0.04 | 1092 | 1884 | 1779 | 6.77 |
| 210 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.4 | 162.46 | 0.34 | 75.58 | 44.9 | 0.04 | 1084 | 1876 | 1802 | 6.80 |
| 211 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.4 | 171.02 | 0.44 | 75.71 | 44.9 | 0.04 | 1100 | 1892 | 1797 | 6.82 |
| 212 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.4 | 165.94 | 0.41 | 75.90 | 44.9 | 0.04 | 1092 | 1884 | 1795 | 6.80 |
| 213 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.3 | 154.47 | 0.40 | 74.49 | 44.9 | 0.04 | 1092 | 1884 | 1858 | 6.91 |
| 214 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.3 | 158.58 | 0.34 | 74.76 | 44.9 | 0.04 | 1084 | 1876 | 1901 | 6.98 |
| 215 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 12.3 | 154.33 | 0.42 | 74.63 | 44.9 | 0.04 | 1100 | 1892 | 1832 | 6.88 |
| 216 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 11.9 | 170.33 | 0.45 | 79.25 | 44.9 | 0.04 | 1100 | 1892 | 1806 | 6.83 |
| 217 | cpu: naive + faces + raster + bricks + refloor + direct | 0.03 | 10.2 | 116.53 | - | 97.59 | 68.1 | 0.00 | - | - | 1702 | 3.15 |
| 218 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.9 | 475.67 | 0.44 | 199.57 | 44.8 | 0.04 | 1100 | 1892 | 1713 | 6.66 |
| 219 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.9 | 481.55 | 0.35 | 200.75 | 44.8 | 0.04 | 1084 | 1876 | 1667 | 6.54 |
| 220 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.8 | 487.23 | 0.42 | 204.19 | 44.8 | 0.04 | 1092 | 1884 | 1713 | 6.65 |
| 221 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 498.60 | 0.35 | 210.32 | 44.8 | 0.04 | 1084 | 1876 | 1651 | 6.52 |
| 222 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.7 | 493.93 | 0.46 | 210.46 | 44.8 | 0.04 | 1100 | 1892 | 1603 | 6.46 |
| 223 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 508.49 | 0.43 | 211.15 | 44.8 | 0.04 | 1092 | 1884 | 1660 | 6.55 |
| 224 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 498.00 | 0.47 | 212.59 | 44.8 | 0.04 | 1100 | 1892 | 1944 | 7.09 |
| 225 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 506.44 | 0.43 | 212.91 | 44.8 | 0.04 | 1092 | 1884 | 1995 | 7.17 |
| 226 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.6 | 514.48 | 0.34 | 216.55 | 44.8 | 0.04 | 1084 | 1876 | 1956 | 7.08 |
| 227 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 505.32 | 0.34 | 215.64 | 44.8 | 0.04 | 1084 | 1876 | 2242 | 7.61 |
| 228 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 516.42 | 0.42 | 215.57 | 44.8 | 0.04 | 1092 | 1884 | 2181 | 7.51 |
| 229 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 514.77 | 0.45 | 216.94 | 44.8 | 0.04 | 1100 | 1892 | 2170 | 7.51 |
| 230 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 508.77 | 0.45 | 216.17 | 44.8 | 0.04 | 1100 | 1892 | 1727 | 6.69 |
| 231 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 501.75 | 0.42 | 216.73 | 44.8 | 0.04 | 1092 | 1884 | 1778 | 6.77 |
| 232 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 517.85 | 0.45 | 217.41 | 44.8 | 0.04 | 1100 | 1892 | 2288 | 7.72 |
| 233 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 515.84 | 0.34 | 217.46 | 44.8 | 0.04 | 1084 | 1876 | 2213 | 7.55 |
| 234 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 519.69 | 0.42 | 217.97 | 44.8 | 0.04 | 1092 | 1884 | 2235 | 7.61 |
| 235 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 519.20 | 0.46 | 218.17 | 44.8 | 0.04 | 1100 | 1892 | 2234 | 7.62 |
| 236 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 513.81 | 0.34 | 217.43 | 44.8 | 0.04 | 1084 | 1876 | 1787 | 6.77 |
| 237 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 531.19 | 0.35 | 219.40 | 44.8 | 0.04 | 1084 | 1876 | 1710 | 6.62 |
| 238 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.5 | 519.21 | 0.46 | 220.06 | 44.8 | 0.04 | 1100 | 1892 | 1573 | 6.40 |
| 239 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.4 | 534.07 | 0.43 | 220.35 | 44.8 | 0.04 | 1092 | 1884 | 2154 | 7.46 |
| 240 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.4 | 531.73 | 0.43 | 221.54 | 44.8 | 0.04 | 1092 | 1884 | 1617 | 6.47 |
| 241 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 4.4 | 528.69 | 0.34 | 221.74 | 44.8 | 0.04 | 1084 | 1876 | 2139 | 7.42 |
| 242 | cpu: heightmap + columns + heightfield + direct (incorrect) | 0.76 | 305.3 | 8.38 | - | 2.67 | 18.2 | 22.12 | - | - | 649 | 1.20 |
| 243 | gpu: heightmap + columns + heightfield + pathtrace (incorrect) | 0.75 | 298.7 | 14.15 | 1.55 | 0.02 | 18.2 | 22.12 | 60 | 110 | 1134 | 2.30 |
| 244 | gpu: naive@cpu + rayquery + raycast + pathtrace | FAILED: [gpu: naive@cpu + rayquery + raycast + pathtrace]: worker error: the device is out of memory (NVIDIA GeForce RTX 5060 Laptop GPU, this renderer seen holding up to 6954 MiB; the world's size and the resolution set most of it: lower the view radius or the resolution, or the storage's pool or slot knobs): wgpu error: Out of Memory | | | | | | | | | | |

### showcase [lit]

960×540. camera 1.36 blocks/frame (max 1.4), turn 2.72°/frame; 84 chunks at start, 0.5 loads/frame; chunk fill 12%, solid 0.3%, surface 0.35%; sky 28%, hits at 38 blocks (median 29); 0.0 edits/frame; 2 lights; 0 mesh triangles.

| # | renderer | rel | fps | frame p99 ms | gpu ms | cpu ms | psnr | bad pct | gpu mb | gpu device mb | peak rss mb | mem rel |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu: distance + sphere + composite + raycast + direct ◆ | 1.00 | 4434.8 | 0.79 | 0.15 | 0.01 | 58.3 | 0.00 | 16 | - | 304 | 12.39 |
| 2 | gpu: distance + sphere + beam + composite + raycast + direct ◆ | 0.94 | 4174.5 | 0.81 | 0.17 | 0.01 | 58.3 | 0.00 | 16 | - | 304 | 12.39 |
| 3 | gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.89 | 3961.6 | 1.29 | 0.15 | 0.07 | 44.0 | 0.03 | 9 | - | 346 | 13.77 |
| 4 | gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.89 | 3936.0 | 1.45 | 0.12 | 0.06 | 44.0 | 0.03 | 23 | - | 308 | 12.83 |
| 5 | gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.88 | 3893.9 | 1.16 | 0.15 | 0.05 | 44.0 | 0.03 | 18 | - | 308 | 12.62 |
| 6 | gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.87 | 3879.9 | 1.16 | 0.16 | 0.05 | 44.0 | 0.03 | 17 | - | 307 | 12.59 |
| 7 | gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.87 | 3855.0 | 1.39 | 0.15 | 0.07 | 44.0 | 0.03 | 11 | - | 354 | 14.15 |
| 8 | gpu: distance + refloor + composite + raycast + direct ◆ | 0.86 | 3820.2 | 0.90 | 0.19 | 0.01 | 58.3 | 0.00 | 16 | - | 304 | 12.39 |
| 9 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.86 | 3808.7 | 1.03 | 0.18 | 0.05 | 44.0 | 0.03 | 17 | - | 307 | 12.58 |
| 10 | gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.86 | 3795.5 | 1.34 | 0.15 | 0.06 | 44.0 | 0.03 | 20 | - | 307 | 12.69 |
| 11 | gpu: prefix + refloor + composite + raycast + direct ◆ | 0.85 | 3766.7 | 0.62 | 0.22 | 0.01 | 58.3 | 0.00 | 13 | - | 303 | 12.25 |
| 12 | gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.84 | 3731.9 | 1.40 | 0.12 | 0.06 | 44.0 | 0.03 | 21 | - | 308 | 12.75 |
| 13 | gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.83 | 3696.0 | 1.13 | 0.14 | 0.05 | 44.0 | 0.03 | 21 | - | 308 | 12.75 |
| 14 | gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.83 | 3674.8 | 1.18 | 0.17 | 0.05 | 44.0 | 0.03 | 18 | - | 308 | 12.64 |
| 15 | gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.82 | 3655.0 | 1.22 | 0.17 | 0.05 | 44.0 | 0.03 | 20 | - | 308 | 12.70 |
| 16 | gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.82 | 3639.5 | 1.64 | 0.16 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.68 |
| 17 | gpu: hier + refloor + composite + raycast + direct ◆ | 0.80 | 3569.6 | 0.73 | 0.22 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.22 |
| 18 | gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.80 | 3552.6 | 1.20 | 0.18 | 0.05 | 44.0 | 0.03 | 20 | - | 308 | 12.70 |
| 19 | gpu: hier + refloor + beam + composite + raycast + direct | 0.80 | 3550.6 | 0.63 | 0.23 | 0.01 | 58.3 | 0.00 | 12 | - | 304 | 12.24 |
| 20 | gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.80 | 3549.4 | 1.81 | 0.15 | 0.08 | 44.0 | 0.03 | 20 | - | 352 | 14.42 |
| 21 | gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.80 | 3547.8 | 1.45 | 0.15 | 0.06 | 44.0 | 0.03 | 23 | - | 308 | 12.84 |
| 22 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.80 | 3529.4 | 1.26 | 0.18 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.67 |
| 23 | gpu: svo_dag + refloor + composite + raycast + direct ◆ | 0.79 | 3523.3 | 0.92 | 0.21 | 0.04 | 58.3 | 0.00 | 4 | - | 305 | 11.98 |
| 24 | gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.79 | 3512.3 | 1.29 | 0.16 | 0.06 | 44.0 | 0.03 | 23 | - | 308 | 12.83 |
| 25 | gpu: distance + refloor + beam + composite + raycast + direct | 0.79 | 3492.5 | 0.90 | 0.21 | 0.01 | 58.3 | 0.00 | 16 | - | 303 | 12.38 |
| 26 | gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.79 | 3489.8 | 1.36 | 0.16 | 0.06 | 44.0 | 0.03 | 21 | - | 308 | 12.75 |
| 27 | gpu: prefix + refloor + beam + composite + raycast + direct | 0.79 | 3489.6 | 0.68 | 0.23 | 0.01 | 58.3 | 0.00 | 13 | - | 304 | 12.25 |
| 28 | gpu: distance + rowrun + composite + raycast + direct | 0.78 | 3478.0 | 0.94 | 0.22 | 0.01 | 58.3 | 0.00 | 16 | - | 304 | 12.39 |
| 29 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.78 | 3449.9 | 1.99 | 0.15 | 0.10 | 44.0 | 0.03 | 11 | - | 354 | 14.15 |
| 30 | gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.77 | 3436.0 | 1.22 | 0.18 | 0.05 | 44.0 | 0.03 | 18 | - | 308 | 12.63 |
| 31 | gpu: prefix + rowrun + composite + raycast + direct | 0.77 | 3424.2 | 0.63 | 0.24 | 0.01 | 58.3 | 0.00 | 13 | - | 304 | 12.26 |
| 32 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.77 | 3421.1 | 2.07 | 0.15 | 0.10 | 44.0 | 0.03 | 9 | - | 346 | 13.77 |
| 33 | gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.77 | 3394.9 | 1.76 | 0.18 | 0.08 | 44.0 | 0.03 | 10 | - | 348 | 13.84 |
| 34 | gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.76 | 3357.5 | 2.01 | 0.12 | 0.09 | 44.0 | 0.03 | 23 | - | 353 | 14.61 |
| 35 | gpu: distance + rowrun + beam + composite + raycast + direct | 0.75 | 3328.5 | 0.86 | 0.23 | 0.01 | 58.3 | 0.00 | 16 | - | 304 | 12.40 |
| 36 | gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.75 | 3320.7 | 1.87 | 0.18 | 0.08 | 44.0 | 0.03 | 12 | - | 353 | 14.15 |
| 37 | gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.75 | 3315.6 | 2.14 | 0.16 | 0.09 | 44.0 | 0.03 | 17 | - | 346 | 14.10 |
| 38 | gpu: prefix + rowrun + beam + composite + raycast + direct | 0.75 | 3314.7 | 0.64 | 0.25 | 0.01 | 58.3 | 0.00 | 13 | - | 304 | 12.28 |
| 39 | gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.74 | 3297.3 | 2.24 | 0.15 | 0.10 | 44.0 | 0.03 | 18 | - | 347 | 14.14 |
| 40 | gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.74 | 3291.9 | 1.18 | 0.21 | 0.05 | 44.0 | 0.03 | 32 | - | 350 | 14.82 |
| 41 | gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.74 | 3289.9 | 1.44 | 0.20 | 0.05 | 44.0 | 0.03 | 20 | - | 308 | 12.71 |
| 42 | gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.74 | 3287.9 | 1.18 | 0.19 | 0.06 | 44.0 | 0.03 | 18 | - | 307 | 12.61 |
| 43 | gpu: naive@cpu + faces + distance + sphere + depth + raster + direct | 0.74 | 3279.9 | 1.19 | 0.18 | 0.05 | 44.0 | 0.03 | 19 | - | 308 | 12.67 |
| 44 | gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.74 | 3264.1 | 1.86 | 0.17 | 0.09 | 44.0 | 0.03 | 20 | - | 351 | 14.38 |
| 45 | gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.74 | 3262.6 | 2.03 | 0.14 | 0.09 | 44.0 | 0.03 | 21 | - | 346 | 14.23 |
| 46 | gpu: svo_dag + refloor + beam + composite + raycast + direct ◆ | 0.74 | 3260.3 | 0.92 | 0.23 | 0.04 | 58.3 | 0.00 | 4 | - | 305 | 11.97 |
| 47 | gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.73 | 3247.6 | 2.13 | 0.12 | 0.11 | 44.0 | 0.03 | 21 | - | 346 | 14.25 |
| 48 | gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.73 | 3231.3 | 2.08 | 0.16 | 0.10 | 44.0 | 0.03 | 19 | - | 354 | 14.47 |
| 49 | gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.72 | 3205.3 | 0.89 | 0.23 | 0.04 | 44.0 | 0.03 | 17 | - | 346 | 14.08 |
| 50 | gpu: bricks + refloor + composite + raycast + direct | 0.72 | 3177.4 | 0.67 | 0.27 | 0.01 | 58.3 | 0.00 | 12 | - | 298 | 12.01 |
| 51 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.72 | 3175.1 | 1.87 | 0.18 | 0.09 | 44.0 | 0.03 | 19 | - | 348 | 14.24 |
| 52 | gpu: naive@cpu + faces + prefix + refloor + depth + raster + direct | 0.71 | 3166.2 | 1.33 | 0.22 | 0.06 | 44.0 | 0.03 | 16 | - | 308 | 12.56 |
| 53 | gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.71 | 3163.9 | 1.87 | 0.16 | 0.08 | 44.0 | 0.03 | 23 | - | 348 | 14.41 |
| 54 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.71 | 3141.9 | 2.04 | 0.18 | 0.09 | 44.0 | 0.03 | 17 | - | 309 | 12.63 |
| 55 | gpu: rows + refloor + composite + raycast + direct | 0.71 | 3137.6 | 0.71 | 0.27 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.25 |
| 56 | gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.71 | 3131.7 | 1.23 | 0.20 | 0.04 | 44.0 | 0.03 | 20 | - | 345 | 14.18 |
| 57 | gpu: naive@cpu + faces + distance + refloor + depth + raster + direct | 0.71 | 3129.3 | 1.18 | 0.21 | 0.05 | 44.0 | 0.03 | 19 | - | 308 | 12.67 |
| 58 | gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.71 | 3128.6 | 0.94 | 0.24 | 0.05 | 44.0 | 0.03 | 17 | - | 308 | 12.61 |
| 59 | gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.70 | 3124.6 | 1.95 | 0.18 | 0.08 | 44.0 | 0.03 | 18 | - | 346 | 14.12 |
| 60 | gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.70 | 3116.5 | 0.87 | 0.25 | 0.03 | 44.0 | 0.03 | 18 | - | 353 | 14.40 |
| 61 | gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.70 | 3108.8 | 0.86 | 0.25 | 0.03 | 44.0 | 0.03 | 17 | - | 351 | 14.25 |
| 62 | gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.70 | 3105.5 | 1.13 | 0.24 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.68 |
| 63 | gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.70 | 3100.7 | 2.18 | 0.17 | 0.10 | 44.0 | 0.03 | 18 | - | 352 | 14.34 |
| 64 | gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.70 | 3098.2 | 0.82 | 0.25 | 0.03 | 44.0 | 0.03 | 19 | - | 350 | 14.30 |
| 65 | gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.70 | 3096.9 | 0.90 | 0.24 | 0.04 | 44.0 | 0.03 | 19 | - | 351 | 14.34 |
| 66 | gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.70 | 3095.5 | 0.88 | 0.24 | 0.04 | 44.0 | 0.03 | 16 | - | 346 | 14.03 |
| 67 | gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.70 | 3085.9 | 2.22 | 0.16 | 0.10 | 44.0 | 0.03 | 21 | - | 348 | 14.33 |
| 68 | gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.70 | 3085.4 | 2.43 | 0.14 | 0.10 | 44.0 | 0.03 | 23 | - | 354 | 14.62 |
| 69 | gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.69 | 3072.9 | 1.83 | 0.20 | 0.08 | 44.0 | 0.03 | 18 | - | 347 | 14.15 |
| 70 | gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.69 | 3062.2 | 1.49 | 0.21 | 0.06 | 44.0 | 0.03 | 34 | - | 356 | 15.13 |
| 71 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.69 | 3059.2 | 1.20 | 0.24 | 0.06 | 44.0 | 0.03 | 10 | - | 353 | 14.08 |
| 72 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.69 | 3051.6 | 1.38 | 0.23 | 0.06 | 44.0 | 0.03 | 8 | - | 346 | 13.72 |
| 73 | gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.69 | 3044.2 | 1.19 | 0.21 | 0.05 | 44.0 | 0.03 | 22 | - | 353 | 14.55 |
| 74 | gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.69 | 3042.6 | 1.76 | 0.21 | 0.08 | 44.0 | 0.03 | 32 | - | 350 | 14.81 |
| 75 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.68 | 3028.4 | 0.83 | 0.26 | 0.04 | 44.0 | 0.03 | 18 | - | 353 | 14.39 |
| 76 | gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.68 | 3027.5 | 1.07 | 0.22 | 0.04 | 44.0 | 0.03 | 20 | - | 345 | 14.18 |
| 77 | gpu: rows + refloor + beam + composite + raycast + direct | 0.68 | 2994.6 | 0.80 | 0.28 | 0.01 | 58.3 | 0.00 | 12 | - | 304 | 12.25 |
| 78 | gpu: naive@cpu + faces + hier + refloor + depth + raster + direct | 0.67 | 2992.3 | 1.34 | 0.23 | 0.05 | 44.0 | 0.03 | 15 | - | 307 | 12.51 |
| 79 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.67 | 2990.1 | 1.28 | 0.24 | 0.06 | 44.0 | 0.03 | 11 | - | 354 | 14.16 |
| 80 | gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.67 | 2976.0 | 1.35 | 0.23 | 0.07 | 44.0 | 0.03 | 7 | - | 333 | 13.18 |
| 81 | gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.67 | 2971.3 | 1.10 | 0.23 | 0.04 | 44.0 | 0.03 | 22 | - | 353 | 14.55 |
| 82 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.67 | 2970.7 | 0.83 | 0.26 | 0.04 | 44.0 | 0.03 | 16 | - | 345 | 14.02 |
| 83 | gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.67 | 2951.5 | 1.15 | 0.25 | 0.05 | 44.0 | 0.03 | 34 | - | 359 | 15.26 |
| 84 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + direct | 0.66 | 2926.4 | 0.89 | 0.26 | 0.05 | 44.0 | 0.03 | 15 | - | 307 | 12.52 |
| 85 | gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.66 | 2923.2 | 0.87 | 0.26 | 0.04 | 44.0 | 0.03 | 19 | - | 353 | 14.42 |
| 86 | gpu: greedy@cpu + faces + distance + sphere + depth + raster + direct | 0.66 | 2922.4 | 2.03 | 0.19 | 0.09 | 44.0 | 0.03 | 19 | - | 332 | 13.62 |
| 87 | gpu: bricks + refloor + beam + composite + raycast + direct | 0.66 | 2918.4 | 0.85 | 0.28 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.23 |
| 88 | gpu: naive@cpu + faces + rows + refloor + depth + raster + direct | 0.66 | 2915.3 | 1.12 | 0.26 | 0.05 | 44.0 | 0.03 | 16 | - | 308 | 12.54 |
| 89 | gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.66 | 2914.9 | 1.07 | 0.26 | 0.04 | 44.0 | 0.03 | 17 | - | 345 | 14.04 |
| 90 | gpu: rows + rowrun + composite + raycast + direct | 0.66 | 2910.4 | 0.72 | 0.30 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.24 |
| 91 | gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.65 | 2885.5 | 1.18 | 0.25 | 0.05 | 44.0 | 0.03 | 32 | - | 353 | 14.96 |
| 92 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.65 | 2884.2 | 1.60 | 0.24 | 0.08 | 44.0 | 0.03 | 9 | - | 348 | 13.86 |
| 93 | gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.65 | 2880.9 | 1.15 | 0.26 | 0.05 | 44.0 | 0.03 | 17 | - | 308 | 12.60 |
| 94 | gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.65 | 2878.3 | 1.60 | 0.23 | 0.08 | 44.0 | 0.03 | 17 | - | 352 | 14.32 |
| 95 | gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.65 | 2868.4 | 2.85 | 0.18 | 0.13 | 44.0 | 0.03 | 12 | - | 353 | 14.14 |
| 96 | gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.65 | 2865.3 | 2.12 | 0.20 | 0.10 | 44.0 | 0.03 | 20 | - | 347 | 14.20 |
| 97 | gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.64 | 2856.6 | 1.23 | 0.24 | 0.04 | 44.0 | 0.03 | 20 | - | 347 | 14.24 |
| 98 | gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.64 | 2856.2 | 0.93 | 0.28 | 0.04 | 44.0 | 0.03 | 17 | - | 346 | 14.07 |
| 99 | gpu: naive@cpu + faces + tree64 + refloor + depth + raster + direct | 0.64 | 2851.8 | 1.75 | 0.24 | 0.08 | 44.0 | 0.03 | 8 | - | 308 | 12.24 |
| 100 | gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.64 | 2850.5 | 1.24 | 0.26 | 0.05 | 44.0 | 0.03 | 17 | - | 308 | 12.61 |
| 101 | gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.64 | 2821.0 | 2.70 | 0.18 | 0.13 | 44.0 | 0.03 | 10 | - | 348 | 13.86 |
| 102 | gpu: rows + rowrun + beam + composite + raycast + direct | 0.64 | 2818.7 | 0.71 | 0.31 | 0.01 | 58.3 | 0.00 | 12 | - | 304 | 12.26 |
| 103 | gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.63 | 2809.6 | 1.03 | 0.27 | 0.05 | 44.0 | 0.03 | 34 | - | 359 | 15.26 |
| 104 | gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.63 | 2806.8 | 0.98 | 0.29 | 0.04 | 44.0 | 0.03 | 33 | - | 354 | 15.03 |
| 105 | gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.63 | 2805.9 | 1.35 | 0.24 | 0.05 | 44.0 | 0.03 | 22 | - | 347 | 14.33 |
| 106 | gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.63 | 2803.7 | 2.42 | 0.19 | 0.11 | 44.0 | 0.03 | 20 | - | 354 | 14.49 |
| 107 | gpu: greedy@cpu + faces + hier + refloor + depth + raster + direct | 0.63 | 2803.4 | 1.87 | 0.24 | 0.08 | 44.0 | 0.03 | 15 | - | 332 | 13.47 |
| 108 | gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.63 | 2795.1 | 1.29 | 0.26 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.67 |
| 109 | gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.63 | 2783.8 | 1.76 | 0.23 | 0.09 | 44.0 | 0.03 | 19 | - | 354 | 14.49 |
| 110 | gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.62 | 2766.6 | 0.94 | 0.29 | 0.04 | 44.0 | 0.03 | 31 | - | 349 | 14.74 |
| 111 | gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.62 | 2759.1 | 1.43 | 0.26 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.68 |
| 112 | gpu: greedy@cpu + faces + distance + refloor + depth + raster + direct | 0.62 | 2753.6 | 2.24 | 0.21 | 0.09 | 44.0 | 0.03 | 19 | - | 332 | 13.61 |
| 113 | gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.62 | 2738.7 | 1.14 | 0.27 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.69 |
| 114 | gpu: tree64 + refloor + composite + raycast + direct ◆ | 0.62 | 2734.7 | 1.61 | 0.28 | 0.06 | 58.3 | 0.00 | 4 | - | 294 | 11.54 |
| 115 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.62 | 2733.7 | 2.48 | 0.23 | 0.11 | 44.0 | 0.03 | 7 | - | 332 | 13.17 |
| 116 | gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.62 | 2733.0 | 1.33 | 0.27 | 0.06 | 44.0 | 0.03 | 32 | - | 353 | 14.95 |
| 117 | gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.62 | 2728.2 | 1.72 | 0.27 | 0.07 | 44.0 | 0.03 | 11 | - | 353 | 14.08 |
| 118 | gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.61 | 2724.6 | 0.96 | 0.28 | 0.04 | 44.0 | 0.03 | 19 | - | 346 | 14.13 |
| 119 | gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.61 | 2712.9 | 1.44 | 0.26 | 0.06 | 44.0 | 0.03 | 17 | - | 308 | 12.61 |
| 120 | gpu: tree64 + refloor + beam + composite + raycast + direct | 0.61 | 2712.0 | 1.54 | 0.29 | 0.05 | 58.3 | 0.00 | 4 | - | 306 | 12.03 |
| 121 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + direct | 0.61 | 2708.0 | 1.72 | 0.26 | 0.08 | 44.0 | 0.03 | 15 | - | 297 | 12.11 |
| 122 | gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.61 | 2701.6 | 1.72 | 0.26 | 0.08 | 44.0 | 0.03 | 9 | - | 347 | 13.78 |
| 123 | gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.60 | 2677.6 | 1.29 | 0.28 | 0.06 | 44.0 | 0.03 | 20 | - | 308 | 12.71 |
| 124 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.60 | 2675.8 | 1.81 | 0.25 | 0.09 | 44.0 | 0.03 | 32 | - | 353 | 14.95 |
| 125 | gpu: naive@cpu + rayquery + raycast + direct | 0.60 | 2653.0 | 2.06 | 0.24 | 0.15 | 58.0 | 0.00 | 16 | - | 403 | 16.26 |
| 126 | gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.60 | 2651.6 | 2.07 | 0.26 | 0.09 | 44.0 | 0.03 | 12 | - | 357 | 14.28 |
| 127 | gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.60 | 2651.3 | 1.33 | 0.28 | 0.06 | 44.0 | 0.03 | 18 | - | 308 | 12.62 |
| 128 | gpu: greedy@cpu + faces + prefix + refloor + depth + raster + direct | 0.60 | 2643.9 | 2.13 | 0.24 | 0.08 | 44.0 | 0.03 | 16 | - | 332 | 13.49 |
| 129 | gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.60 | 2643.5 | 0.93 | 0.31 | 0.04 | 44.0 | 0.03 | 16 | - | 351 | 14.24 |
| 130 | gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.59 | 2638.5 | 1.52 | 0.27 | 0.08 | 44.0 | 0.03 | 17 | - | 348 | 14.17 |
| 131 | gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.59 | 2637.5 | 1.35 | 0.29 | 0.05 | 44.0 | 0.03 | 20 | - | 308 | 12.71 |
| 132 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.59 | 2628.7 | 1.87 | 0.25 | 0.09 | 44.0 | 0.03 | 34 | - | 359 | 15.26 |
| 133 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.59 | 2620.8 | 2.03 | 0.26 | 0.10 | 44.0 | 0.03 | 9 | - | 348 | 13.85 |
| 134 | gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.59 | 2619.8 | 0.87 | 0.32 | 0.04 | 44.0 | 0.03 | 18 | - | 354 | 14.42 |
| 135 | gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.59 | 2611.6 | 1.98 | 0.25 | 0.09 | 44.0 | 0.03 | 17 | - | 348 | 14.17 |
| 136 | gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.58 | 2574.2 | 1.87 | 0.27 | 0.08 | 44.0 | 0.03 | 32 | - | 353 | 14.95 |
| 137 | gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.58 | 2571.1 | 1.29 | 0.28 | 0.07 | 44.0 | 0.03 | 18 | - | 308 | 12.62 |
| 138 | gpu: naive@cpu + faces + distance + rowrun + depth + raster + direct | 0.58 | 2570.2 | 1.54 | 0.28 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.68 |
| 139 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.58 | 2570.0 | 2.39 | 0.26 | 0.11 | 44.0 | 0.03 | 11 | - | 353 | 14.14 |
| 140 | gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.58 | 2569.0 | 1.89 | 0.27 | 0.09 | 44.0 | 0.03 | 19 | - | 358 | 14.62 |
| 141 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.58 | 2562.1 | 0.89 | 0.33 | 0.03 | 44.0 | 0.03 | 31 | - | 352 | 14.86 |
| 142 | gpu: bricks + rowrun + composite + raycast + direct | 0.58 | 2560.4 | 0.76 | 0.35 | 0.01 | 58.3 | 0.00 | 12 | - | 304 | 12.24 |
| 143 | gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.58 | 2555.7 | 2.06 | 0.26 | 0.10 | 44.0 | 0.03 | 10 | - | 351 | 13.97 |
| 144 | gpu: naive@cpu + faces + rows + rowrun + depth + raster + direct | 0.58 | 2553.9 | 1.37 | 0.29 | 0.05 | 44.0 | 0.03 | 16 | - | 308 | 12.54 |
| 145 | gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.58 | 2552.3 | 1.76 | 0.28 | 0.08 | 44.0 | 0.03 | 20 | - | 356 | 14.57 |
| 146 | gpu: hier + rowrun + beam + composite + raycast + direct | 0.58 | 2551.6 | 0.78 | 0.35 | 0.01 | 58.3 | 0.00 | 12 | - | 304 | 12.24 |
| 147 | gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.58 | 2550.4 | 1.69 | 0.27 | 0.08 | 44.0 | 0.03 | 17 | - | 349 | 14.22 |
| 148 | gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.57 | 2546.9 | 1.77 | 0.27 | 0.09 | 44.0 | 0.03 | 34 | - | 359 | 15.24 |
| 149 | gpu: hier + rowrun + composite + raycast + direct | 0.57 | 2537.2 | 0.97 | 0.34 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.23 |
| 150 | gpu: clipmap + refloor + composite + raycast + direct | 0.57 | 2535.3 | 0.76 | 0.35 | 0.01 | 58.3 | 0.00 | 27 | - | 308 | 12.97 |
| 151 | gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.57 | 2530.5 | 2.07 | 0.26 | 0.10 | 44.0 | 0.03 | 19 | - | 353 | 14.45 |
| 152 | gpu: naive@cpu + faces + clipmap + refloor + depth + raster + direct | 0.57 | 2525.9 | 1.26 | 0.30 | 0.06 | 44.0 | 0.03 | 31 | - | 337 | 14.23 |
| 153 | gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.57 | 2524.5 | 1.33 | 0.29 | 0.05 | 44.0 | 0.03 | 21 | - | 308 | 12.76 |
| 154 | gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.57 | 2512.2 | 1.94 | 0.26 | 0.09 | 44.0 | 0.03 | 19 | - | 348 | 14.25 |
| 155 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.57 | 2512.1 | 0.95 | 0.33 | 0.04 | 44.0 | 0.03 | 33 | - | 358 | 15.19 |
| 156 | gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.56 | 2504.2 | 0.93 | 0.33 | 0.04 | 44.0 | 0.03 | 16 | - | 348 | 14.11 |
| 157 | gpu: svo_dag + rowrun + composite + raycast + direct | 0.56 | 2496.2 | 0.92 | 0.34 | 0.04 | 58.3 | 0.00 | 4 | - | 305 | 11.96 |
| 158 | gpu: bricks + rowrun + beam + composite + raycast + direct | 0.56 | 2493.7 | 0.76 | 0.36 | 0.01 | 58.3 | 0.00 | 12 | - | 304 | 12.25 |
| 159 | gpu: cubes@cpu + faces + distance + sphere + depth + raster + direct | 0.56 | 2491.4 | 1.18 | 0.31 | 0.04 | 44.0 | 0.03 | 18 | - | 331 | 13.55 |
| 160 | gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.56 | 2485.6 | 2.32 | 0.26 | 0.12 | 44.0 | 0.03 | 10 | - | 351 | 13.99 |
| 161 | gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.56 | 2483.8 | 1.40 | 0.29 | 0.06 | 44.0 | 0.03 | 23 | - | 308 | 12.84 |
| 162 | gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.56 | 2477.1 | 0.84 | 0.35 | 0.03 | 44.0 | 0.03 | 31 | - | 353 | 14.89 |
| 163 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.56 | 2476.7 | 1.29 | 0.32 | 0.06 | 44.0 | 0.03 | 8 | - | 347 | 13.79 |
| 164 | gpu: cubes@cpu + faces + prefix + refloor + depth + raster + direct | 0.56 | 2468.6 | 0.96 | 0.35 | 0.03 | 44.0 | 0.03 | 15 | - | 331 | 13.42 |
| 165 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.56 | 2465.7 | 1.26 | 0.33 | 0.06 | 44.0 | 0.03 | 10 | - | 353 | 14.09 |
| 166 | gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.55 | 2460.9 | 0.99 | 0.34 | 0.04 | 44.0 | 0.03 | 16 | - | 347 | 14.09 |
| 167 | gpu: points@cpu + splat + distance + sphere + direct | 0.55 | 2454.6 | 1.34 | 0.32 | 0.01 | 58.2 | 0.00 | 20 | - | 346 | 14.19 |
| 168 | gpu: greedy@cpu + faces + rows + refloor + depth + raster + direct | 0.55 | 2450.1 | 1.78 | 0.28 | 0.08 | 44.0 | 0.03 | 16 | - | 332 | 13.49 |
| 169 | gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + direct | 0.55 | 2437.6 | 2.70 | 0.27 | 0.12 | 44.0 | 0.03 | 8 | - | 333 | 13.22 |
| 170 | gpu: naive@cpu + faces + prefix + rowrun + depth + raster + direct | 0.55 | 2431.1 | 1.74 | 0.29 | 0.06 | 44.0 | 0.03 | 16 | - | 308 | 12.56 |
| 171 | gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.55 | 2423.7 | 1.00 | 0.34 | 0.04 | 44.0 | 0.03 | 18 | - | 347 | 14.18 |
| 172 | gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.55 | 2423.3 | 0.91 | 0.34 | 0.05 | 44.0 | 0.03 | 16 | - | 348 | 14.14 |
| 173 | gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.55 | 2421.7 | 0.96 | 0.35 | 0.04 | 44.0 | 0.03 | 18 | - | 353 | 14.40 |
| 174 | gpu: points@cpu + splat + prefix + refloor + direct | 0.54 | 2411.5 | 0.99 | 0.35 | 0.01 | 58.2 | 0.00 | 17 | - | 344 | 14.01 |
| 175 | gpu: clipmap + refloor + beam + composite + raycast + direct | 0.54 | 2409.2 | 0.81 | 0.37 | 0.01 | 58.3 | 0.00 | 27 | - | 308 | 12.99 |
| 176 | gpu: cubes@cpu + faces + distance + refloor + depth + raster + direct | 0.54 | 2406.7 | 1.09 | 0.33 | 0.03 | 44.0 | 0.03 | 18 | - | 331 | 13.56 |
| 177 | gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.54 | 2406.6 | 2.53 | 0.21 | 0.11 | 44.0 | 0.03 | 34 | - | 355 | 15.11 |
| 178 | gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.54 | 2400.4 | 0.98 | 0.35 | 0.04 | 44.0 | 0.03 | 33 | - | 358 | 15.18 |
| 179 | gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.54 | 2399.2 | 2.86 | 0.26 | 0.13 | 44.0 | 0.03 | 12 | - | 357 | 14.28 |
| 180 | gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.54 | 2394.4 | 0.94 | 0.35 | 0.04 | 44.0 | 0.03 | 18 | - | 354 | 14.43 |
| 181 | gpu: svo_dag + rowrun + beam + composite + raycast + direct | 0.54 | 2379.9 | 1.08 | 0.35 | 0.05 | 58.3 | 0.00 | 4 | - | 305 | 11.98 |
| 182 | gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.53 | 2371.4 | 2.01 | 0.28 | 0.10 | 44.0 | 0.03 | 18 | - | 350 | 14.26 |
| 183 | gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.53 | 2369.7 | 1.44 | 0.33 | 0.07 | 44.0 | 0.03 | 9 | - | 350 | 13.91 |
| 184 | gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.53 | 2368.5 | 0.96 | 0.36 | 0.04 | 44.0 | 0.03 | 19 | - | 355 | 14.50 |
| 185 | gpu: points@cpu + splat + svo_dag + refloor + direct | 0.53 | 2367.8 | 1.27 | 0.35 | 0.04 | 58.2 | 0.00 | 8 | - | 345 | 13.69 |
| 186 | gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.53 | 2361.9 | 0.95 | 0.36 | 0.04 | 44.0 | 0.03 | 17 | - | 350 | 14.21 |
| 187 | gpu: greedy@cpu + faces + distance + rowrun + depth + raster + direct | 0.53 | 2354.6 | 2.07 | 0.29 | 0.09 | 44.0 | 0.03 | 19 | - | 334 | 13.69 |
| 188 | gpu: cubes@cpu + faces + hier + refloor + depth + raster + direct | 0.53 | 2353.2 | 0.99 | 0.36 | 0.04 | 44.0 | 0.03 | 14 | - | 331 | 13.39 |
| 189 | gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + direct | 0.53 | 2351.5 | 1.95 | 0.31 | 0.09 | 44.0 | 0.03 | 16 | - | 334 | 13.58 |
| 190 | gpu: points@cpu + splat + hier + refloor + direct | 0.53 | 2347.7 | 1.19 | 0.36 | 0.01 | 58.2 | 0.00 | 16 | - | 346 | 14.06 |
| 191 | gpu: points@cpu + splat + distance + refloor + direct | 0.53 | 2346.5 | 1.36 | 0.34 | 0.01 | 58.2 | 0.00 | 20 | - | 346 | 14.20 |
| 192 | gpu: points@cpu + splat + prefix + rowrun + direct | 0.53 | 2346.0 | 1.04 | 0.36 | 0.01 | 58.2 | 0.00 | 17 | - | 350 | 14.23 |
| 193 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.53 | 2346.0 | 1.35 | 0.35 | 0.06 | 44.0 | 0.03 | 6 | - | 332 | 13.11 |
| 194 | gpu: hier + incremental + composite + raycast + direct | 0.53 | 2335.2 | 0.83 | 0.39 | 0.01 | 58.1 | 0.00 | 12 | - | 304 | 12.23 |
| 195 | gpu: distance + sphere + composite + raycast + pathtrace | 0.53 | 2334.3 | 0.92 | 0.38 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.23 |
| 196 | gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.53 | 2332.4 | 2.14 | 0.29 | 0.10 | 44.0 | 0.03 | 20 | - | 355 | 14.52 |
| 197 | gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.52 | 2327.2 | 2.14 | 0.29 | 0.11 | 44.0 | 0.03 | 18 | - | 349 | 14.24 |
| 198 | gpu: greedy@cpu + rayquery + raycast + direct | 0.52 | 2324.2 | 2.68 | 0.26 | 0.18 | 58.0 | 0.00 | 16 | - | 399 | 16.11 |
| 199 | gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.52 | 2320.5 | 0.90 | 0.37 | 0.04 | 44.0 | 0.03 | 17 | - | 348 | 14.16 |
| 200 | gpu: points@cpu + splat + bricks + refloor + direct | 0.52 | 2320.0 | 1.00 | 0.38 | 0.01 | 58.2 | 0.00 | 16 | - | 293 | 11.97 |
| 201 | gpu: bricks + incremental + composite + raycast + direct | 0.52 | 2297.6 | 0.84 | 0.39 | 0.01 | 58.1 | 0.00 | 12 | - | 292 | 11.77 |
| 202 | gpu: points@cpu + splat + rows + refloor + direct | 0.51 | 2280.9 | 1.02 | 0.38 | 0.01 | 58.2 | 0.00 | 17 | - | 346 | 14.07 |
| 203 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct | 0.51 | 2279.7 | 0.96 | 0.38 | 0.03 | 44.0 | 0.03 | 14 | - | 331 | 13.40 |
| 204 | gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.51 | 2260.6 | 0.99 | 0.35 | 0.06 | 44.0 | 0.03 | 17 | - | 307 | 12.59 |
| 205 | gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.51 | 2258.2 | 1.02 | 0.35 | 0.06 | 44.0 | 0.03 | 19 | - | 307 | 12.67 |
| 206 | gpu: distance + sphere + beam + composite + raycast + pathtrace | 0.51 | 2257.5 | 0.98 | 0.39 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.26 |
| 207 | gpu: points@cpu + splat + distance + rowrun + direct | 0.51 | 2253.6 | 1.47 | 0.35 | 0.01 | 58.2 | 0.00 | 20 | - | 346 | 14.20 |
| 208 | gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.51 | 2250.9 | 2.14 | 0.29 | 0.09 | 44.0 | 0.03 | 23 | - | 354 | 14.63 |
| 209 | gpu: hier + incremental + beam + composite + raycast + direct | 0.51 | 2240.4 | 0.90 | 0.40 | 0.01 | 58.1 | 0.00 | 12 | - | 304 | 12.26 |
| 210 | gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.50 | 2238.6 | 1.69 | 0.34 | 0.08 | 44.0 | 0.03 | 11 | - | 357 | 14.23 |
| 211 | gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.50 | 2236.6 | 1.13 | 0.37 | 0.04 | 44.0 | 0.03 | 19 | - | 354 | 14.47 |
| 212 | gpu: greedy@cpu + faces + rows + rowrun + depth + raster + direct | 0.50 | 2235.6 | 1.86 | 0.32 | 0.08 | 44.0 | 0.03 | 16 | - | 334 | 13.56 |
| 213 | gpu: cubes@cpu + faces + rows + refloor + depth + raster + direct | 0.50 | 2225.5 | 0.99 | 0.39 | 0.04 | 44.0 | 0.03 | 15 | - | 331 | 13.41 |
| 214 | gpu: naive@cpu + faces + rows + incremental + depth + raster + direct | 0.50 | 2224.6 | 1.35 | 0.37 | 0.05 | 44.0 | 0.03 | 16 | - | 308 | 12.55 |
| 215 | gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.50 | 2219.8 | 1.35 | 0.35 | 0.05 | 35.5 | 0.91 | 29 | - | 308 | 13.06 |
| 216 | gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.50 | 2219.4 | 2.42 | 0.29 | 0.09 | 44.0 | 0.03 | 21 | - | 348 | 14.31 |
| 217 | gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.50 | 2200.8 | 1.50 | 0.36 | 0.08 | 44.0 | 0.03 | 11 | - | 352 | 14.09 |
| 218 | gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + direct | 0.50 | 2198.8 | 1.99 | 0.32 | 0.09 | 44.0 | 0.03 | 31 | - | 336 | 14.22 |
| 219 | gpu: naive@cpu + faces + hier + incremental + depth + raster + direct | 0.49 | 2192.1 | 1.37 | 0.38 | 0.05 | 44.0 | 0.03 | 15 | - | 308 | 12.53 |
| 220 | gpu: points@cpu + splat + rows + rowrun + direct | 0.49 | 2187.7 | 1.16 | 0.39 | 0.01 | 58.2 | 0.00 | 17 | - | 345 | 14.04 |
| 221 | gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.49 | 2186.9 | 1.18 | 0.37 | 0.04 | 44.0 | 0.03 | 20 | - | 347 | 14.25 |
| 222 | gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + direct | 0.49 | 2185.5 | 1.13 | 0.41 | 0.04 | 44.0 | 0.03 | 15 | - | 334 | 13.51 |
| 223 | gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.49 | 2178.5 | 1.44 | 0.36 | 0.08 | 44.0 | 0.03 | 9 | - | 346 | 13.79 |
| 224 | gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.49 | 2173.1 | 1.46 | 0.35 | 0.06 | 35.5 | 0.91 | 31 | - | 308 | 13.16 |
| 225 | gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.49 | 2166.3 | 1.75 | 0.35 | 0.08 | 44.0 | 0.03 | 19 | - | 352 | 14.40 |
| 226 | gpu: rows + incremental + composite + raycast + direct | 0.49 | 2163.0 | 0.90 | 0.42 | 0.01 | 58.1 | 0.00 | 12 | - | 303 | 12.24 |
| 227 | gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.49 | 2162.0 | 1.16 | 0.38 | 0.04 | 44.0 | 0.03 | 22 | - | 352 | 14.53 |
| 228 | gpu: clipmap + rowrun + composite + raycast + direct | 0.49 | 2158.9 | 0.88 | 0.41 | 0.01 | 58.3 | 0.00 | 27 | - | 307 | 12.97 |
| 229 | gpu: points@cpu + splat + tree64 + refloor + direct | 0.49 | 2158.3 | 1.31 | 0.38 | 0.05 | 58.2 | 0.00 | 9 | - | 347 | 13.78 |
| 230 | gpu: clipmap + rowrun + beam + composite + raycast + direct | 0.49 | 2156.4 | 0.81 | 0.42 | 0.01 | 58.3 | 0.00 | 27 | - | 307 | 12.97 |
| 231 | gpu: naive@cpu + faces + bricks + incremental + depth + raster + direct | 0.49 | 2150.9 | 1.32 | 0.39 | 0.06 | 44.0 | 0.03 | 15 | - | 308 | 12.52 |
| 232 | gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.48 | 2147.4 | 1.37 | 0.39 | 0.06 | 44.0 | 0.03 | 18 | - | 308 | 12.62 |
| 233 | gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.48 | 2143.5 | 1.13 | 0.38 | 0.06 | 44.0 | 0.03 | 20 | - | 308 | 12.71 |
| 234 | gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.48 | 2136.4 | 1.37 | 0.38 | 0.07 | 44.0 | 0.03 | 9 | - | 350 | 13.93 |
| 235 | gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.48 | 2126.6 | 1.80 | 0.35 | 0.09 | 44.0 | 0.03 | 17 | - | 346 | 14.10 |
| 236 | gpu: prefix + incremental + composite + raycast + direct | 0.48 | 2114.9 | 0.94 | 0.43 | 0.01 | 58.1 | 0.00 | 13 | - | 304 | 12.27 |
| 237 | gpu: points@cpu + splat + clipmap + refloor + direct | 0.48 | 2113.9 | 1.17 | 0.40 | 0.01 | 58.2 | 0.00 | 31 | - | 348 | 14.71 |
| 238 | gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.48 | 2111.8 | 1.14 | 0.40 | 0.05 | 44.0 | 0.03 | 19 | - | 307 | 12.66 |
| 239 | gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + direct | 0.48 | 2111.2 | 1.57 | 0.38 | 0.08 | 44.0 | 0.03 | 7 | - | 333 | 13.16 |
| 240 | gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.47 | 2091.2 | 1.54 | 0.39 | 0.08 | 44.0 | 0.03 | 11 | - | 356 | 14.25 |
| 241 | gpu: cubes@cpu + faces + distance + rowrun + depth + raster + direct | 0.47 | 2090.8 | 1.10 | 0.41 | 0.03 | 44.0 | 0.03 | 18 | - | 333 | 13.61 |
| 242 | gpu: naive@cpu + faces + prefix + incremental + depth + raster + direct | 0.47 | 2087.3 | 1.05 | 0.41 | 0.05 | 44.0 | 0.03 | 16 | - | 308 | 12.55 |
| 243 | gpu: bricks + incremental + beam + composite + raycast + direct | 0.47 | 2084.2 | 0.87 | 0.42 | 0.01 | 58.1 | 0.00 | 12 | - | 304 | 12.24 |
| 244 | gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + direct | 0.47 | 2069.8 | 1.04 | 0.43 | 0.04 | 44.0 | 0.03 | 30 | - | 336 | 14.15 |
| 245 | gpu: cubes@cpu + faces + rows + rowrun + depth + raster + direct | 0.47 | 2069.2 | 0.94 | 0.43 | 0.04 | 44.0 | 0.03 | 15 | - | 332 | 13.46 |
| 246 | gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.47 | 2068.6 | 1.21 | 0.41 | 0.06 | 44.0 | 0.03 | 17 | - | 308 | 12.59 |
| 247 | gpu: points@cpu + splat + bricks + rowrun + direct | 0.47 | 2067.1 | 1.35 | 0.43 | 0.01 | 58.2 | 0.00 | 16 | - | 353 | 14.30 |
| 248 | gpu: distance + incremental + composite + raycast + direct | 0.47 | 2065.1 | 1.03 | 0.44 | 0.01 | 58.1 | 0.00 | 16 | - | 303 | 12.38 |
| 249 | gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.46 | 2045.9 | 2.08 | 0.35 | 0.09 | 35.5 | 0.91 | 31 | - | 356 | 15.02 |
| 250 | gpu: rows + incremental + beam + composite + raycast + direct | 0.46 | 2044.1 | 0.98 | 0.45 | 0.01 | 58.1 | 0.00 | 12 | - | 304 | 12.25 |
| 251 | gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.46 | 2042.0 | 1.45 | 0.41 | 0.05 | 44.0 | 0.03 | 18 | - | 307 | 12.60 |
| 252 | gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.46 | 2041.6 | 0.99 | 0.43 | 0.04 | 44.0 | 0.03 | 18 | - | 351 | 14.34 |
| 253 | gpu: points@cpu + splat + clipmap + rowrun + direct | 0.46 | 2040.7 | 1.00 | 0.44 | 0.01 | 58.2 | 0.00 | 31 | - | 351 | 14.83 |
| 254 | gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.46 | 2034.9 | 1.38 | 0.40 | 0.06 | 44.0 | 0.03 | 20 | - | 308 | 12.69 |
| 255 | gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.46 | 2024.9 | 1.06 | 0.43 | 0.04 | 44.0 | 0.03 | 16 | - | 346 | 14.03 |
| 256 | gpu: boxes@cpu + rayquery + raycast + direct | 0.46 | 2021.6 | 1.05 | 0.44 | 0.09 | 58.0 | 0.00 | 12 | - | 389 | 15.54 |
| 257 | gpu: naive@cpu + faces + distance + incremental + depth + raster + direct | 0.45 | 2007.9 | 1.58 | 0.40 | 0.06 | 44.0 | 0.03 | 19 | - | 308 | 12.68 |
| 258 | gpu: points@cpu + splat + hier + rowrun + direct | 0.45 | 1984.7 | 1.10 | 0.45 | 0.01 | 58.2 | 0.00 | 16 | - | 346 | 14.07 |
| 259 | gpu: tree64 + incremental + composite + raycast + direct | 0.45 | 1977.2 | 1.25 | 0.43 | 0.06 | 58.1 | 0.00 | 4 | - | 294 | 11.55 |
| 260 | gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.45 | 1976.0 | 2.79 | 0.35 | 0.11 | 35.5 | 0.91 | 29 | - | 350 | 14.72 |
| 261 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.45 | 1975.5 | 2.13 | 0.37 | 0.11 | 44.0 | 0.03 | 11 | - | 352 | 14.07 |
| 262 | gpu: prefix + incremental + beam + composite + raycast + direct | 0.44 | 1964.4 | 1.06 | 0.46 | 0.01 | 58.1 | 0.00 | 13 | - | 304 | 12.27 |
| 263 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.44 | 1963.6 | 2.08 | 0.38 | 0.10 | 44.0 | 0.03 | 9 | - | 347 | 13.80 |
| 264 | gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.44 | 1962.6 | 1.21 | 0.43 | 0.04 | 35.5 | 0.91 | 28 | - | 349 | 14.64 |
| 265 | gpu: points@cpu + splat + bricks + incremental + direct | 0.44 | 1962.5 | 1.14 | 0.46 | 0.01 | 58.2 | 0.00 | 16 | - | 346 | 14.06 |
| 266 | gpu: clipmap + incremental + composite + raycast + direct | 0.44 | 1960.2 | 1.07 | 0.47 | 0.01 | 58.1 | 0.00 | 27 | - | 308 | 12.98 |
| 267 | gpu: greedy@cpu + faces + rows + incremental + depth + raster + direct | 0.44 | 1956.7 | 2.17 | 0.38 | 0.11 | 44.0 | 0.03 | 16 | - | 335 | 13.61 |
| 268 | gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.44 | 1956.6 | 2.16 | 0.38 | 0.11 | 44.0 | 0.03 | 20 | - | 353 | 14.45 |
| 269 | gpu: points@cpu + splat + clipmap + incremental + direct | 0.44 | 1956.0 | 1.08 | 0.46 | 0.01 | 58.2 | 0.00 | 31 | - | 351 | 14.83 |
| 270 | gpu: distance + incremental + beam + composite + raycast + direct | 0.44 | 1949.8 | 1.07 | 0.47 | 0.01 | 58.1 | 0.00 | 16 | - | 303 | 12.38 |
| 271 | gpu: naive@cpu + rayquery + raycast + pathtrace | 0.44 | 1946.1 | 2.13 | 0.37 | 0.14 | 36.2 | 0.88 | 24 | - | 403 | 16.57 |
| 272 | gpu: greedy@cpu + faces + hier + incremental + depth + raster + direct | 0.44 | 1945.9 | 1.94 | 0.39 | 0.10 | 44.0 | 0.03 | 15 | - | 333 | 13.51 |
| 273 | gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.44 | 1942.4 | 1.75 | 0.41 | 0.09 | 44.0 | 0.03 | 17 | - | 346 | 14.09 |
| 274 | gpu: tree64 + incremental + beam + composite + raycast + direct | 0.44 | 1941.2 | 1.13 | 0.45 | 0.05 | 58.1 | 0.00 | 4 | - | 306 | 12.04 |
| 275 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.44 | 1940.4 | 2.30 | 0.39 | 0.11 | 44.0 | 0.03 | 9 | - | 350 | 13.94 |
| 276 | gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.44 | 1938.8 | 1.23 | 0.44 | 0.04 | 35.5 | 0.91 | 30 | - | 355 | 14.94 |
| 277 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.44 | 1932.8 | 1.30 | 0.44 | 0.06 | 44.0 | 0.03 | 10 | - | 351 | 14.00 |
| 278 | gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.44 | 1929.9 | 1.11 | 0.44 | 0.06 | 35.5 | 0.91 | 26 | - | 309 | 12.96 |
| 279 | gpu: points@cpu + splat + prefix + incremental + direct | 0.43 | 1926.4 | 1.09 | 0.47 | 0.01 | 58.2 | 0.00 | 17 | - | 348 | 14.14 |
| 280 | gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.43 | 1925.2 | 1.42 | 0.43 | 0.05 | 35.5 | 0.91 | 29 | - | 308 | 13.08 |
| 281 | gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.43 | 1921.7 | 1.41 | 0.44 | 0.07 | 35.5 | 0.91 | 19 | - | 356 | 14.56 |
| 282 | gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.43 | 1916.8 | 1.89 | 0.41 | 0.09 | 44.0 | 0.03 | 18 | - | 346 | 14.12 |
| 283 | gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.43 | 1916.0 | 2.10 | 0.40 | 0.10 | 44.0 | 0.03 | 20 | - | 352 | 14.43 |
| 284 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.43 | 1914.1 | 1.76 | 0.44 | 0.06 | 44.0 | 0.03 | 8 | - | 345 | 13.71 |
| 285 | gpu: naive@cpu + faces + tree64 + incremental + depth + raster + direct | 0.43 | 1912.1 | 1.68 | 0.43 | 0.08 | 44.0 | 0.03 | 8 | - | 309 | 12.27 |
| 286 | gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.43 | 1908.5 | 1.03 | 0.47 | 0.04 | 44.0 | 0.03 | 19 | - | 352 | 14.39 |
| 287 | gpu: greedy@cpu + rayquery + raycast + pathtrace | 0.43 | 1906.4 | 2.73 | 0.37 | 0.18 | 36.2 | 0.88 | 24 | - | 404 | 16.61 |
| 288 | gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.43 | 1901.6 | 1.61 | 0.44 | 0.08 | 35.5 | 0.91 | 17 | - | 350 | 14.25 |
| 289 | gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.43 | 1895.3 | 2.25 | 0.39 | 0.11 | 44.0 | 0.03 | 18 | - | 347 | 14.17 |
| 290 | gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.43 | 1893.8 | 1.32 | 0.45 | 0.06 | 44.0 | 0.03 | 34 | - | 356 | 15.13 |
| 291 | gpu: points@cpu + splat + hier + incremental + direct | 0.43 | 1893.1 | 1.52 | 0.46 | 0.01 | 58.2 | 0.00 | 16 | - | 347 | 14.10 |
| 292 | gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.42 | 1881.1 | 1.26 | 0.47 | 0.04 | 44.0 | 0.03 | 17 | - | 347 | 14.09 |
| 293 | gpu: naive@cpu + faces + clipmap + incremental + depth + raster + direct | 0.42 | 1873.0 | 1.16 | 0.46 | 0.05 | 44.0 | 0.03 | 31 | - | 340 | 14.38 |
| 294 | gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.42 | 1869.1 | 0.95 | 0.49 | 0.03 | 44.0 | 0.03 | 16 | - | 345 | 14.02 |
| 295 | gpu: greedy@cpu + faces + bricks + incremental + depth + raster + direct | 0.42 | 1867.6 | 1.89 | 0.42 | 0.10 | 44.0 | 0.03 | 15 | - | 335 | 13.60 |
| 296 | gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.42 | 1865.3 | 1.23 | 0.46 | 0.05 | 44.0 | 0.03 | 31 | - | 338 | 14.29 |
| 297 | gpu: greedy@cpu + faces + distance + incremental + depth + raster + direct | 0.42 | 1865.1 | 2.26 | 0.41 | 0.08 | 44.0 | 0.03 | 19 | - | 335 | 13.75 |
| 298 | gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.42 | 1861.9 | 1.31 | 0.46 | 0.06 | 44.0 | 0.03 | 32 | - | 354 | 14.97 |
| 299 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.42 | 1860.0 | 1.31 | 0.46 | 0.06 | 44.0 | 0.03 | 8 | - | 349 | 13.86 |
| 300 | gpu: clipmap + incremental + beam + composite + raycast + direct | 0.42 | 1858.2 | 0.90 | 0.49 | 0.01 | 58.1 | 0.00 | 27 | - | 308 | 12.98 |
| 301 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.42 | 1857.4 | 2.67 | 0.39 | 0.14 | 44.0 | 0.03 | 11 | - | 356 | 14.23 |
| 302 | gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.42 | 1852.7 | 2.40 | 0.41 | 0.11 | 44.0 | 0.03 | 19 | - | 352 | 14.39 |
| 303 | gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.42 | 1851.4 | 1.18 | 0.49 | 0.04 | 44.0 | 0.03 | 19 | - | 351 | 14.35 |
| 304 | gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.42 | 1849.8 | 0.92 | 0.49 | 0.04 | 44.0 | 0.03 | 18 | - | 351 | 14.32 |
| 305 | gpu: greedy@cpu + faces + prefix + incremental + depth + raster + direct | 0.42 | 1849.2 | 2.11 | 0.42 | 0.10 | 44.0 | 0.03 | 16 | - | 333 | 13.52 |
| 306 | gpu: points@cpu + splat + rows + incremental + direct | 0.42 | 1846.9 | 1.28 | 0.48 | 0.01 | 58.2 | 0.00 | 17 | - | 347 | 14.10 |
| 307 | gpu: naive@cpu + faces + hier + rowrun + depth + raster + direct | 0.42 | 1846.1 | 1.28 | 0.47 | 0.06 | 44.0 | 0.03 | 15 | - | 308 | 12.53 |
| 308 | gpu: points@cpu + splat + tree64 + incremental + direct | 0.42 | 1845.3 | 1.76 | 0.45 | 0.06 | 58.2 | 0.00 | 9 | - | 351 | 13.92 |
| 309 | gpu: distance + refloor + composite + raycast + pathtrace | 0.42 | 1844.6 | 1.15 | 0.49 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.24 |
| 310 | gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.41 | 1840.1 | 1.17 | 0.46 | 0.05 | 35.5 | 0.91 | 28 | - | 308 | 13.03 |
| 311 | gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.41 | 1839.0 | 1.04 | 0.49 | 0.05 | 44.0 | 0.03 | 17 | - | 345 | 14.04 |
| 312 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.41 | 1825.6 | 2.16 | 0.43 | 0.10 | 35.5 | 0.91 | 17 | - | 350 | 14.25 |
| 313 | gpu: cubes@cpu + faces + hier + incremental + depth + raster + direct | 0.41 | 1812.1 | 1.29 | 0.50 | 0.04 | 44.0 | 0.03 | 14 | - | 332 | 13.44 |
| 314 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.41 | 1811.5 | 1.38 | 0.47 | 0.07 | 44.0 | 0.03 | 10 | - | 355 | 14.18 |
| 315 | gpu: distance + refloor + beam + composite + raycast + pathtrace | 0.41 | 1801.3 | 1.10 | 0.51 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.24 |
| 316 | gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.41 | 1798.9 | 1.84 | 0.45 | 0.09 | 35.5 | 0.91 | 28 | - | 358 | 14.94 |
| 317 | gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.41 | 1798.1 | 1.72 | 0.46 | 0.09 | 44.0 | 0.03 | 10 | - | 351 | 13.96 |
| 318 | gpu: points@cpu + splat + distance + incremental + direct | 0.40 | 1791.0 | 1.46 | 0.49 | 0.01 | 58.2 | 0.00 | 20 | - | 346 | 14.20 |
| 319 | gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.40 | 1790.3 | 1.80 | 0.45 | 0.10 | 35.5 | 0.91 | 19 | - | 358 | 14.65 |
| 320 | gpu: svo_dag + refloor + composite + raycast + pathtrace | 0.40 | 1785.6 | 1.27 | 0.50 | 0.05 | 36.2 | 0.88 | 12 | - | 293 | 11.82 |
| 321 | gpu: naive@cpu + faces + bricks + rowrun + depth + raster + direct | 0.40 | 1784.3 | 1.23 | 0.50 | 0.05 | 44.0 | 0.03 | 15 | - | 307 | 12.51 |
| 322 | gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.40 | 1784.2 | 2.30 | 0.43 | 0.09 | 35.5 | 0.91 | 29 | - | 350 | 14.71 |
| 323 | gpu: prefix + refloor + composite + raycast + pathtrace | 0.40 | 1778.2 | 1.03 | 0.52 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.11 |
| 324 | gpu: cubes@cpu + faces + bricks + incremental + depth + raster + direct | 0.40 | 1778.2 | 1.06 | 0.52 | 0.04 | 44.0 | 0.03 | 14 | - | 335 | 13.53 |
| 325 | gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.40 | 1776.6 | 1.89 | 0.45 | 0.09 | 44.0 | 0.03 | 34 | - | 356 | 15.12 |
| 326 | gpu: points@cpu + splat + svo_dag + rowrun + direct | 0.40 | 1775.8 | 1.16 | 0.49 | 0.04 | 58.2 | 0.00 | 8 | - | 347 | 13.77 |
| 327 | gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.40 | 1774.6 | 2.01 | 0.46 | 0.09 | 44.0 | 0.03 | 12 | - | 354 | 14.18 |
| 328 | gpu: cubes@cpu + faces + rows + incremental + depth + raster + direct | 0.40 | 1770.4 | 1.14 | 0.52 | 0.03 | 44.0 | 0.03 | 15 | - | 335 | 13.55 |
| 329 | gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.40 | 1765.2 | 2.04 | 0.44 | 0.11 | 35.5 | 0.91 | 26 | - | 352 | 14.64 |
| 330 | gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.40 | 1764.7 | 2.11 | 0.45 | 0.10 | 35.5 | 0.91 | 17 | - | 353 | 14.35 |
| 331 | gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.40 | 1762.2 | 1.19 | 0.46 | 0.06 | 35.5 | 0.91 | 31 | - | 308 | 13.16 |
| 332 | gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.40 | 1760.0 | 1.30 | 0.48 | 0.06 | 35.5 | 0.91 | 25 | - | 308 | 12.93 |
| 333 | gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.40 | 1758.9 | 1.24 | 0.48 | 0.07 | 35.5 | 0.91 | 27 | - | 309 | 13.02 |
| 334 | gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.40 | 1756.6 | 1.84 | 0.46 | 0.09 | 44.0 | 0.03 | 10 | - | 349 | 13.89 |
| 335 | gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.40 | 1753.4 | 1.64 | 0.47 | 0.09 | 44.0 | 0.03 | 12 | - | 356 | 14.24 |
| 336 | gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.39 | 1742.6 | 1.01 | 0.52 | 0.04 | 35.5 | 0.91 | 25 | - | 351 | 14.57 |
| 337 | gpu: hier + refloor + composite + raycast + pathtrace | 0.39 | 1739.4 | 0.96 | 0.53 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.08 |
| 338 | gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + direct | 0.39 | 1737.1 | 1.74 | 0.46 | 0.08 | 44.0 | 0.03 | 31 | - | 337 | 14.25 |
| 339 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.39 | 1731.7 | 2.15 | 0.45 | 0.11 | 35.5 | 0.91 | 19 | - | 356 | 14.55 |
| 340 | gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.39 | 1730.4 | 1.32 | 0.51 | 0.04 | 35.5 | 0.91 | 28 | - | 350 | 14.64 |
| 341 | gpu: distance + sphere + quadtree + gbuffer + direct | 0.39 | 1726.9 | 1.23 | 0.48 | 0.04 | 44.0 | 0.03 | 128 | - | 363 | 19.02 |
| 342 | gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.39 | 1714.3 | 2.19 | 0.45 | 0.09 | 35.5 | 0.91 | 31 | - | 356 | 15.00 |
| 343 | gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.39 | 1710.3 | 1.08 | 0.54 | 0.03 | 44.0 | 0.03 | 33 | - | 355 | 15.04 |
| 344 | gpu: prefix + refloor + beam + composite + raycast + pathtrace | 0.39 | 1708.2 | 0.97 | 0.53 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.11 |
| 345 | gpu: distance + sphere + quadtree + visbuffer + direct | 0.38 | 1706.2 | 1.27 | 0.47 | 0.05 | 44.0 | 0.03 | 126 | - | 366 | 19.05 |
| 346 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.38 | 1702.3 | 1.75 | 0.50 | 0.09 | 44.0 | 0.03 | 31 | - | 338 | 14.28 |
| 347 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.38 | 1702.2 | 1.37 | 0.52 | 0.07 | 35.5 | 0.91 | 18 | - | 355 | 14.46 |
| 348 | gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + direct | 0.38 | 1698.7 | 2.48 | 0.46 | 0.12 | 44.0 | 0.03 | 8 | - | 335 | 13.26 |
| 349 | gpu: hier + refloor + beam + composite + raycast + pathtrace | 0.38 | 1693.9 | 0.95 | 0.55 | 0.01 | 36.2 | 0.88 | 20 | - | 291 | 12.07 |
| 350 | gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.38 | 1693.3 | 1.91 | 0.47 | 0.10 | 44.0 | 0.03 | 32 | - | 350 | 14.83 |
| 351 | gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.38 | 1692.4 | 1.13 | 0.54 | 0.04 | 35.5 | 0.91 | 27 | - | 356 | 14.85 |
| 352 | gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.38 | 1691.5 | 1.24 | 0.54 | 0.04 | 35.5 | 0.91 | 24 | - | 350 | 14.51 |
| 353 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.38 | 1687.3 | 2.95 | 0.46 | 0.12 | 44.0 | 0.03 | 10 | - | 350 | 13.95 |
| 354 | gpu: cubes@cpu + faces + prefix + incremental + depth + raster + direct | 0.38 | 1686.8 | 0.96 | 0.55 | 0.03 | 44.0 | 0.03 | 15 | - | 332 | 13.45 |
| 355 | gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.38 | 1680.3 | 1.71 | 0.48 | 0.09 | 35.5 | 0.91 | 25 | - | 351 | 14.58 |
| 356 | gpu: prefix + refloor + quadtree + visbuffer + direct | 0.38 | 1676.5 | 1.02 | 0.52 | 0.05 | 44.0 | 0.03 | 122 | - | 364 | 18.86 |
| 357 | gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.38 | 1673.4 | 2.73 | 0.46 | 0.13 | 44.0 | 0.03 | 12 | - | 354 | 14.17 |
| 358 | gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.38 | 1672.3 | 2.63 | 0.46 | 0.13 | 35.5 | 0.91 | 17 | - | 353 | 14.34 |
| 359 | gpu: prefix + refloor + quadtree + gbuffer + direct | 0.38 | 1663.4 | 1.03 | 0.52 | 0.05 | 44.0 | 0.03 | 124 | - | 361 | 18.82 |
| 360 | gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.37 | 1662.2 | 1.17 | 0.53 | 0.04 | 35.5 | 0.91 | 30 | - | 355 | 14.92 |
| 361 | gpu: points@cpu + splat + hier + sphere + direct | 0.37 | 1662.1 | 1.40 | 0.55 | 0.01 | 58.2 | 0.00 | 16 | - | 344 | 13.98 |
| 362 | gpu: cubes@cpu + faces + distance + incremental + depth + raster + direct | 0.37 | 1661.9 | 1.16 | 0.54 | 0.04 | 44.0 | 0.03 | 18 | - | 334 | 13.66 |
| 363 | gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 0.37 | 1649.2 | 1.30 | 0.54 | 0.05 | 36.2 | 0.88 | 12 | - | 293 | 11.84 |
| 364 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.37 | 1647.9 | 2.38 | 0.47 | 0.12 | 44.0 | 0.03 | 12 | - | 356 | 14.25 |
| 365 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.37 | 1645.7 | 1.30 | 0.52 | 0.06 | 35.5 | 0.91 | 16 | - | 349 | 14.17 |
| 366 | gpu: hier + refloor + quadtree + gbuffer + direct | 0.37 | 1645.0 | 1.14 | 0.52 | 0.05 | 44.0 | 0.03 | 124 | - | 362 | 18.83 |
| 367 | gpu: hier + refloor + quadtree + visbuffer + direct | 0.37 | 1636.4 | 1.34 | 0.53 | 0.04 | 44.0 | 0.03 | 122 | - | 362 | 18.76 |
| 368 | gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.37 | 1636.1 | 1.82 | 0.50 | 0.09 | 35.5 | 0.91 | 27 | - | 357 | 14.88 |
| 369 | gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.37 | 1635.5 | 2.65 | 0.47 | 0.13 | 44.0 | 0.03 | 10 | - | 349 | 13.89 |
| 370 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.37 | 1632.8 | 1.15 | 0.56 | 0.03 | 44.0 | 0.03 | 30 | - | 337 | 14.21 |
| 371 | gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.37 | 1620.6 | 1.47 | 0.55 | 0.05 | 44.0 | 0.03 | 31 | - | 349 | 14.75 |
| 372 | gpu: distance + refloor + quadtree + gbuffer + direct | 0.37 | 1619.5 | 1.31 | 0.51 | 0.05 | 44.0 | 0.03 | 128 | - | 363 | 19.01 |
| 373 | gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.37 | 1619.4 | 1.04 | 0.57 | 0.04 | 35.5 | 0.91 | 26 | - | 356 | 14.80 |
| 374 | gpu: distance + refloor + quadtree + visbuffer + direct | 0.36 | 1617.9 | 1.26 | 0.51 | 0.04 | 44.0 | 0.03 | 126 | - | 363 | 18.95 |
| 375 | gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.36 | 1600.7 | 1.54 | 0.53 | 0.07 | 35.5 | 0.91 | 29 | - | 309 | 13.09 |
| 376 | gpu: points@cpu + splat + prefix + sphere + direct | 0.36 | 1597.5 | 1.45 | 0.58 | 0.01 | 58.2 | 0.00 | 17 | - | 345 | 14.03 |
| 377 | gpu: prefix + rowrun + quadtree + visbuffer + direct | 0.36 | 1596.7 | 1.12 | 0.55 | 0.03 | 44.0 | 0.03 | 122 | - | 363 | 18.80 |
| 378 | gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.36 | 1595.0 | 2.01 | 0.53 | 0.09 | 35.5 | 0.91 | 16 | - | 352 | 14.28 |
| 379 | gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.36 | 1594.3 | 1.70 | 0.54 | 0.08 | 35.5 | 0.91 | 18 | - | 357 | 14.57 |
| 380 | gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.36 | 1593.7 | 1.60 | 0.54 | 0.08 | 44.0 | 0.03 | 11 | - | 354 | 14.12 |
| 381 | gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.36 | 1591.5 | 2.83 | 0.47 | 0.13 | 35.5 | 0.91 | 19 | - | 358 | 14.64 |
| 382 | gpu: bricks + refloor + quadtree + gbuffer + direct | 0.36 | 1588.1 | 1.02 | 0.56 | 0.03 | 44.0 | 0.03 | 124 | - | 353 | 18.47 |
| 383 | gpu: points@cpu + splat + distance + sphere + pathtrace | 0.36 | 1588.0 | 2.08 | 0.54 | 0.01 | 36.2 | 0.88 | 28 | - | 348 | 14.57 |
| 384 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.36 | 1579.8 | 1.77 | 0.54 | 0.07 | 44.0 | 0.03 | 9 | - | 349 | 13.87 |
| 385 | gpu: distance + rowrun + quadtree + gbuffer + direct | 0.36 | 1579.2 | 1.41 | 0.55 | 0.03 | 44.0 | 0.03 | 128 | - | 362 | 19.00 |
| 386 | gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.36 | 1577.5 | 1.54 | 0.54 | 0.08 | 44.0 | 0.03 | 9 | - | 348 | 13.82 |
| 387 | gpu: bricks + refloor + quadtree + visbuffer + direct | 0.36 | 1576.9 | 1.25 | 0.56 | 0.04 | 44.0 | 0.03 | 122 | - | 355 | 18.47 |
| 388 | gpu: points@cpu + splat + svo_dag + incremental + direct | 0.35 | 1570.5 | 1.34 | 0.58 | 0.04 | 58.2 | 0.00 | 8 | - | 349 | 13.83 |
| 389 | gpu: distance + rowrun + quadtree + visbuffer + direct | 0.35 | 1568.4 | 1.09 | 0.54 | 0.05 | 44.0 | 0.03 | 126 | - | 365 | 19.02 |
| 390 | gpu: rows + refloor + quadtree + visbuffer + direct | 0.35 | 1567.9 | 1.14 | 0.56 | 0.04 | 44.0 | 0.03 | 122 | - | 360 | 18.70 |
| 391 | gpu: points@cpu + splat + svo_dag + sphere + direct | 0.35 | 1561.2 | 1.19 | 0.57 | 0.04 | 58.2 | 0.00 | 8 | - | 345 | 13.71 |
| 392 | gpu: greedy@cpu + faces + hier + rowrun + depth + raster + direct | 0.35 | 1559.8 | 1.55 | 0.55 | 0.08 | 44.0 | 0.03 | 15 | - | 333 | 13.52 |
| 393 | gpu: cubes@cpu + faces + hier + rowrun + depth + raster + direct | 0.35 | 1558.7 | 1.21 | 0.60 | 0.05 | 44.0 | 0.03 | 14 | - | 333 | 13.45 |
| 394 | gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + direct | 0.35 | 1557.9 | 1.45 | 0.57 | 0.07 | 44.0 | 0.03 | 7 | - | 334 | 13.19 |
| 395 | gpu: prefix + rowrun + quadtree + gbuffer + direct | 0.35 | 1555.4 | 1.29 | 0.56 | 0.05 | 44.0 | 0.03 | 124 | - | 362 | 18.84 |
| 396 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.35 | 1553.8 | 1.73 | 0.56 | 0.09 | 44.0 | 0.03 | 11 | - | 355 | 14.18 |
| 397 | gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.35 | 1552.3 | 1.43 | 0.57 | 0.07 | 35.5 | 0.91 | 26 | - | 309 | 12.97 |
| 398 | gpu: naive@cpu + faces + hier + sphere + depth + raster + direct | 0.35 | 1547.6 | 1.26 | 0.59 | 0.06 | 44.0 | 0.03 | 15 | - | 308 | 12.53 |
| 399 | gpu: points@cpu + splat + rows + sphere + direct | 0.35 | 1547.4 | 1.26 | 0.60 | 0.01 | 58.2 | 0.00 | 17 | - | 344 | 14.00 |
| 400 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.35 | 1542.3 | 1.45 | 0.58 | 0.06 | 35.5 | 0.91 | 27 | - | 296 | 12.53 |
| 401 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.35 | 1541.6 | 1.37 | 0.57 | 0.06 | 35.5 | 0.91 | 25 | - | 296 | 12.46 |
| 402 | gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.35 | 1535.2 | 1.63 | 0.53 | 0.09 | 35.5 | 0.91 | 29 | - | 355 | 14.90 |
| 403 | gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.35 | 1533.2 | 1.43 | 0.58 | 0.07 | 35.5 | 0.91 | 28 | - | 308 | 13.03 |
| 404 | gpu: naive@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.34 | 1520.3 | 1.40 | 0.56 | 0.06 | 35.5 | 0.91 | 27 | - | 309 | 13.02 |
| 405 | gpu: tree64 + refloor + composite + raycast + pathtrace | 0.34 | 1518.8 | 1.44 | 0.58 | 0.07 | 36.2 | 0.88 | 12 | - | 294 | 11.87 |
| 406 | gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + direct | 0.34 | 1516.5 | 1.54 | 0.60 | 0.04 | 44.0 | 0.03 | 30 | - | 336 | 14.18 |
| 407 | gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.34 | 1512.3 | 1.27 | 0.58 | 0.07 | 35.5 | 0.91 | 31 | - | 308 | 13.17 |
| 408 | gpu: rows + rowrun + quadtree + visbuffer + direct | 0.34 | 1511.1 | 1.09 | 0.59 | 0.03 | 44.0 | 0.03 | 122 | - | 359 | 18.64 |
| 409 | gpu: distance + sphere + quadtree + raster + direct | 0.34 | 1509.8 | 1.23 | 0.57 | 0.05 | 44.0 | 0.03 | 124 | - | 347 | 18.25 |
| 410 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.34 | 1509.5 | 2.11 | 0.55 | 0.10 | 35.5 | 0.91 | 25 | - | 296 | 12.46 |
| 411 | gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 0.34 | 1506.5 | 1.66 | 0.59 | 0.07 | 36.2 | 0.88 | 12 | - | 294 | 11.87 |
| 412 | gpu: rows + rowrun + quadtree + gbuffer + direct | 0.34 | 1505.4 | 1.07 | 0.59 | 0.04 | 44.0 | 0.03 | 124 | - | 361 | 18.79 |
| 413 | gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.34 | 1503.5 | 1.81 | 0.56 | 0.09 | 35.5 | 0.91 | 26 | - | 351 | 14.60 |
| 414 | gpu: points@cpu + splat + bricks + sphere + direct | 0.34 | 1501.6 | 1.48 | 0.62 | 0.01 | 58.2 | 0.00 | 16 | - | 344 | 13.97 |
| 415 | gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.34 | 1500.9 | 1.65 | 0.58 | 0.07 | 44.0 | 0.03 | 7 | - | 334 | 13.22 |
| 416 | gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.34 | 1499.9 | 1.31 | 0.58 | 0.06 | 35.5 | 0.91 | 28 | - | 309 | 13.05 |
| 417 | gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.34 | 1499.1 | 1.21 | 0.61 | 0.04 | 35.5 | 0.91 | 28 | - | 354 | 14.80 |
| 418 | gpu: tree64 + rowrun + beam + composite + raycast + direct | 0.33 | 1480.5 | 1.46 | 0.61 | 0.06 | 58.3 | 0.00 | 4 | - | 306 | 12.02 |
| 419 | gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.33 | 1477.0 | 2.03 | 0.57 | 0.10 | 35.5 | 0.91 | 26 | - | 363 | 15.06 |
| 420 | gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + direct | 0.33 | 1476.3 | 1.78 | 0.57 | 0.09 | 44.0 | 0.03 | 15 | - | 333 | 13.52 |
| 421 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.33 | 1468.4 | 1.20 | 0.63 | 0.05 | 35.5 | 0.91 | 24 | - | 295 | 12.37 |
| 422 | gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.33 | 1463.0 | 1.49 | 0.58 | 0.07 | 35.5 | 0.91 | 26 | - | 308 | 12.95 |
| 423 | gpu: points@cpu + splat + distance + refloor + pathtrace | 0.33 | 1460.3 | 1.46 | 0.63 | 0.01 | 36.2 | 0.88 | 28 | - | 348 | 14.57 |
| 424 | gpu: points@cpu + splat + prefix + refloor + pathtrace | 0.33 | 1458.1 | 1.24 | 0.64 | 0.01 | 36.2 | 0.88 | 25 | - | 349 | 14.48 |
| 425 | gpu: hier + sphere + composite + raycast + direct | 0.33 | 1457.9 | 1.18 | 0.64 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.22 |
| 426 | gpu: greedy@cpu + rayquery + raycast + distance + sphere + pathtrace | 0.33 | 1452.3 | 3.23 | 0.50 | 0.19 | 36.2 | 0.88 | 36 | - | 416 | 17.53 |
| 427 | gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.33 | 1450.1 | 1.59 | 0.61 | 0.08 | 35.5 | 0.91 | 15 | - | 337 | 13.65 |
| 428 | gpu: tree64 + rowrun + composite + raycast + direct | 0.33 | 1447.8 | 1.79 | 0.61 | 0.08 | 58.3 | 0.00 | 4 | - | 306 | 12.02 |
| 429 | gpu: svo_dag + refloor + quadtree + visbuffer + direct | 0.33 | 1442.7 | 1.45 | 0.59 | 0.08 | 44.0 | 0.03 | 114 | - | 364 | 18.50 |
| 430 | gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.33 | 1441.8 | 1.17 | 0.64 | 0.05 | 35.5 | 0.91 | 25 | - | 349 | 14.51 |
| 431 | gpu: prefix + refloor + quadtree + raster + direct | 0.32 | 1440.8 | 1.12 | 0.63 | 0.03 | 44.0 | 0.03 | 120 | - | 343 | 17.97 |
| 432 | gpu: distance + rowrun + composite + raycast + pathtrace | 0.32 | 1437.7 | 1.33 | 0.65 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.23 |
| 433 | gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.32 | 1436.7 | 2.01 | 0.58 | 0.10 | 35.5 | 0.91 | 31 | - | 354 | 14.94 |
| 434 | gpu: distance + refloor + quadtree + raster + direct | 0.32 | 1435.8 | 1.21 | 0.61 | 0.04 | 44.0 | 0.03 | 124 | - | 347 | 18.26 |
| 435 | gpu: svo_dag + refloor + quadtree + gbuffer + direct | 0.32 | 1435.6 | 1.54 | 0.58 | 0.08 | 44.0 | 0.03 | 116 | - | 366 | 18.68 |
| 436 | gpu: hier + sphere + beam + composite + raycast + direct | 0.32 | 1430.7 | 1.10 | 0.66 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.23 |
| 437 | gpu: points@cpu + splat + clipmap + sphere + direct | 0.32 | 1430.5 | 1.45 | 0.66 | 0.01 | 58.2 | 0.00 | 31 | - | 348 | 14.70 |
| 438 | gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.32 | 1430.0 | 1.20 | 0.65 | 0.04 | 35.5 | 0.91 | 25 | - | 357 | 14.81 |
| 439 | gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.32 | 1428.7 | 1.13 | 0.65 | 0.05 | 35.5 | 0.91 | 27 | - | 361 | 15.04 |
| 440 | gpu: distance + rowrun + beam + composite + raycast + pathtrace | 0.32 | 1427.0 | 1.07 | 0.66 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.25 |
| 441 | gpu: prefix + rowrun + composite + raycast + pathtrace | 0.32 | 1425.3 | 1.24 | 0.66 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.11 |
| 442 | gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.32 | 1424.8 | 2.18 | 0.59 | 0.10 | 35.5 | 0.91 | 28 | - | 358 | 14.96 |
| 443 | gpu: greedy@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.32 | 1422.7 | 2.61 | 0.56 | 0.12 | 35.5 | 0.91 | 27 | - | 336 | 14.08 |
| 444 | gpu: points@cpu + splat + tree64 + sphere + direct | 0.32 | 1413.5 | 1.27 | 0.66 | 0.05 | 58.2 | 0.00 | 9 | - | 348 | 13.83 |
| 445 | gpu: tree64 + refloor + quadtree + visbuffer + direct | 0.32 | 1410.2 | 1.61 | 0.59 | 0.09 | 44.0 | 0.03 | 114 | - | 365 | 18.58 |
| 446 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.32 | 1409.6 | 1.74 | 0.59 | 0.10 | 35.5 | 0.91 | 27 | - | 296 | 12.55 |
| 447 | gpu: points@cpu + splat + hier + refloor + pathtrace | 0.32 | 1408.0 | 1.50 | 0.66 | 0.01 | 36.2 | 0.88 | 24 | - | 348 | 14.44 |
| 448 | gpu: tree64 + refloor + quadtree + gbuffer + direct | 0.32 | 1406.2 | 1.60 | 0.60 | 0.08 | 44.0 | 0.03 | 116 | - | 366 | 18.66 |
| 449 | gpu: naive@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.32 | 1405.6 | 1.21 | 0.66 | 0.05 | 35.5 | 0.91 | 24 | - | 310 | 12.92 |
| 450 | gpu: greedy@cpu + faces + hier + sphere + depth + raster + direct | 0.32 | 1404.4 | 2.05 | 0.60 | 0.10 | 44.0 | 0.03 | 15 | - | 332 | 13.49 |
| 451 | gpu: rows + refloor + composite + raycast + pathtrace | 0.32 | 1403.0 | 1.40 | 0.67 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.09 |
| 452 | gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.32 | 1400.7 | 1.08 | 0.67 | 0.04 | 35.5 | 0.91 | 27 | - | 356 | 14.83 |
| 453 | gpu: hier + refloor + quadtree + raster + direct | 0.31 | 1397.0 | 1.18 | 0.64 | 0.04 | 44.0 | 0.03 | 120 | - | 343 | 17.95 |
| 454 | gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 0.31 | 1393.9 | 1.21 | 0.67 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.12 |
| 455 | gpu: rows + refloor + quadtree + gbuffer + direct | 0.31 | 1393.7 | 1.38 | 0.62 | 0.05 | 44.0 | 0.03 | 124 | - | 358 | 18.71 |
| 456 | gpu: bricks + refloor + beam + composite + raycast + pathtrace | 0.31 | 1390.2 | 1.29 | 0.67 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.08 |
| 457 | gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.31 | 1389.1 | 2.21 | 0.59 | 0.11 | 35.5 | 0.91 | 28 | - | 356 | 14.89 |
| 458 | gpu: points@cpu + splat + tree64 + refloor + pathtrace | 0.31 | 1387.8 | 1.69 | 0.65 | 0.06 | 36.2 | 0.88 | 16 | - | 351 | 14.25 |
| 459 | gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.31 | 1385.1 | 1.58 | 0.66 | 0.04 | 35.5 | 0.91 | 30 | - | 354 | 14.89 |
| 460 | gpu: naive@cpu + faces + prefix + sphere + depth + raster + direct | 0.31 | 1383.2 | 1.70 | 0.67 | 0.06 | 44.0 | 0.03 | 16 | - | 307 | 12.53 |
| 461 | gpu: greedy@cpu + rayquery + raycast + svo_dag + refloor + pathtrace | 0.31 | 1378.0 | 3.09 | 0.56 | 0.19 | 36.2 | 0.88 | 24 | - | 416 | 17.08 |
| 462 | gpu: points@cpu + splat + svo_dag + refloor + pathtrace | 0.31 | 1376.7 | 1.18 | 0.66 | 0.04 | 36.2 | 0.88 | 16 | - | 348 | 14.13 |
| 463 | gpu: naive@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.31 | 1374.5 | 1.73 | 0.64 | 0.08 | 35.5 | 0.91 | 16 | - | 310 | 12.62 |
| 464 | gpu: greedy@cpu + rayquery + raycast + tree64 + refloor + pathtrace | 0.31 | 1368.0 | 4.32 | 0.51 | 0.24 | 36.2 | 0.88 | 25 | - | 417 | 17.11 |
| 465 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.31 | 1367.4 | 1.34 | 0.67 | 0.04 | 35.5 | 0.91 | 26 | - | 295 | 12.46 |
| 466 | gpu: svo_dag + sphere + composite + raycast + direct | 0.31 | 1365.1 | 1.04 | 0.69 | 0.04 | 58.3 | 0.00 | 4 | - | 305 | 11.96 |
| 467 | gpu: greedy@cpu + rayquery + raycast + prefix + refloor + pathtrace | 0.31 | 1364.6 | 3.08 | 0.57 | 0.19 | 36.2 | 0.88 | 33 | - | 415 | 17.38 |
| 468 | gpu: svo_dag + incremental + beam + composite + raycast + direct | 0.31 | 1362.0 | 1.51 | 0.68 | 0.06 | 58.1 | 0.00 | 4 | - | 305 | 11.99 |
| 469 | gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + direct | 0.31 | 1357.4 | 1.31 | 0.69 | 0.04 | 44.0 | 0.03 | 14 | - | 333 | 13.46 |
| 470 | gpu: naive@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.31 | 1354.4 | 1.58 | 0.66 | 0.07 | 35.5 | 0.91 | 27 | - | 309 | 13.05 |
| 471 | gpu: bricks + rowrun + quadtree + visbuffer + direct | 0.30 | 1350.6 | 1.55 | 0.66 | 0.04 | 44.0 | 0.03 | 122 | - | 363 | 18.77 |
| 472 | gpu: svo_dag + incremental + composite + raycast + direct | 0.30 | 1348.8 | 1.37 | 0.70 | 0.05 | 58.1 | 0.00 | 4 | - | 305 | 11.97 |
| 473 | gpu: clipmap + refloor + quadtree + visbuffer + direct | 0.30 | 1344.7 | 1.25 | 0.67 | 0.05 | 44.0 | 0.03 | 137 | - | 360 | 19.27 |
| 474 | gpu: boxes@cpu + rayquery + raycast + pathtrace | 0.30 | 1340.8 | 1.28 | 0.70 | 0.08 | 36.2 | 0.88 | 20 | - | 387 | 15.79 |
| 475 | gpu: rows + refloor + beam + composite + raycast + pathtrace | 0.30 | 1338.5 | 1.25 | 0.69 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 476 | gpu: bricks + rowrun + quadtree + gbuffer + direct | 0.30 | 1338.4 | 1.26 | 0.67 | 0.03 | 44.0 | 0.03 | 124 | - | 362 | 18.84 |
| 477 | gpu: cubes@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.30 | 1335.9 | 1.56 | 0.69 | 0.04 | 35.5 | 0.91 | 26 | - | 335 | 14.00 |
| 478 | gpu: naive@cpu + faces + rows + sphere + depth + raster + direct | 0.30 | 1335.3 | 1.30 | 0.68 | 0.06 | 44.0 | 0.03 | 16 | - | 307 | 12.53 |
| 479 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.30 | 1333.9 | 2.45 | 0.62 | 0.13 | 44.0 | 0.03 | 7 | - | 334 | 13.22 |
| 480 | gpu: bricks + refloor + composite + raycast + pathtrace | 0.30 | 1331.2 | 2.32 | 0.64 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 481 | gpu: bricks + incremental + quadtree + visbuffer + direct | 0.30 | 1326.7 | 1.22 | 0.68 | 0.04 | 44.0 | 0.03 | 122 | - | 360 | 18.67 |
| 482 | gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.30 | 1325.4 | 1.42 | 0.69 | 0.06 | 35.5 | 0.91 | 26 | - | 309 | 12.97 |
| 483 | gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.30 | 1324.0 | 1.31 | 0.69 | 0.06 | 35.5 | 0.91 | 40 | - | 354 | 15.30 |
| 484 | gpu: bricks + refloor + quadtree + raster + direct | 0.30 | 1320.4 | 1.27 | 0.68 | 0.04 | 44.0 | 0.03 | 120 | - | 310 | 16.66 |
| 485 | gpu: hier + incremental + quadtree + visbuffer + direct | 0.30 | 1320.1 | 1.30 | 0.68 | 0.04 | 44.0 | 0.03 | 122 | - | 361 | 18.73 |
| 486 | gpu: greedy@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.30 | 1317.2 | 1.95 | 0.67 | 0.10 | 35.5 | 0.91 | 24 | - | 336 | 13.95 |
| 487 | gpu: svo_dag + sphere + beam + composite + raycast + direct | 0.30 | 1314.9 | 1.28 | 0.72 | 0.04 | 58.3 | 0.00 | 4 | - | 305 | 11.97 |
| 488 | gpu: bricks + incremental + quadtree + gbuffer + direct | 0.30 | 1313.2 | 1.39 | 0.68 | 0.04 | 44.0 | 0.03 | 124 | - | 362 | 18.82 |
| 489 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + direct | 0.30 | 1312.6 | 1.30 | 0.67 | 0.05 | 44.0 | 0.03 | 124 | - | 354 | 18.52 |
| 490 | gpu: hier + incremental + quadtree + gbuffer + direct | 0.30 | 1310.6 | 1.61 | 0.69 | 0.05 | 44.0 | 0.03 | 124 | - | 359 | 18.72 |
| 491 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + direct | 0.30 | 1310.3 | 1.27 | 0.67 | 0.05 | 44.0 | 0.03 | 122 | - | 357 | 18.55 |
| 492 | gpu: hier + rowrun + quadtree + gbuffer + direct | 0.30 | 1308.3 | 1.12 | 0.70 | 0.03 | 44.0 | 0.03 | 124 | - | 360 | 18.77 |
| 493 | gpu: hier + rowrun + quadtree + visbuffer + direct | 0.29 | 1308.2 | 1.42 | 0.69 | 0.04 | 44.0 | 0.03 | 122 | - | 359 | 18.62 |
| 494 | gpu: greedy@cpu + rayquery + raycast + hier + refloor + pathtrace | 0.29 | 1306.7 | 2.96 | 0.60 | 0.19 | 36.2 | 0.88 | 32 | - | 416 | 17.38 |
| 495 | gpu: naive@cpu + faces + bricks + sphere + depth + raster + direct | 0.29 | 1300.8 | 1.62 | 0.69 | 0.07 | 44.0 | 0.03 | 15 | - | 308 | 12.52 |
| 496 | gpu: rows + incremental + quadtree + visbuffer + direct | 0.29 | 1300.4 | 1.20 | 0.69 | 0.04 | 44.0 | 0.03 | 122 | - | 360 | 18.68 |
| 497 | gpu: rows + incremental + quadtree + gbuffer + direct | 0.29 | 1289.0 | 1.39 | 0.70 | 0.04 | 44.0 | 0.03 | 124 | - | 362 | 18.86 |
| 498 | gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.29 | 1287.5 | 1.35 | 0.71 | 0.06 | 35.5 | 0.91 | 42 | - | 360 | 15.58 |
| 499 | gpu: prefix + sphere + composite + raycast + direct | 0.29 | 1287.1 | 1.09 | 0.74 | 0.01 | 58.3 | 0.00 | 13 | - | 303 | 12.24 |
| 500 | gpu: greedy@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.29 | 1285.9 | 1.95 | 0.67 | 0.09 | 35.5 | 0.91 | 27 | - | 336 | 14.07 |
| 501 | gpu: greedy@cpu + faces + rows + sphere + depth + raster + direct | 0.29 | 1285.4 | 1.89 | 0.68 | 0.09 | 44.0 | 0.03 | 16 | - | 333 | 13.52 |
| 502 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.29 | 1285.0 | 2.26 | 0.66 | 0.11 | 35.5 | 0.91 | 15 | - | 337 | 13.64 |
| 503 | gpu: prefix + incremental + quadtree + visbuffer + direct | 0.29 | 1284.7 | 1.20 | 0.70 | 0.04 | 44.0 | 0.03 | 122 | - | 363 | 18.80 |
| 504 | gpu: cubes@cpu + faces + hier + sphere + depth + raster + direct | 0.29 | 1283.8 | 1.26 | 0.73 | 0.04 | 44.0 | 0.03 | 14 | - | 332 | 13.42 |
| 505 | gpu: greedy@cpu + faces + prefix + sphere + depth + raster + direct | 0.29 | 1283.8 | 1.89 | 0.68 | 0.09 | 44.0 | 0.03 | 16 | - | 333 | 13.52 |
| 506 | gpu: prefix + incremental + quadtree + gbuffer + direct | 0.29 | 1278.4 | 1.33 | 0.71 | 0.04 | 44.0 | 0.03 | 124 | - | 361 | 18.83 |
| 507 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.29 | 1274.1 | 1.67 | 0.72 | 0.07 | 44.0 | 0.03 | 6 | - | 332 | 13.13 |
| 508 | gpu: greedy@cpu + faces + bricks + sphere + depth + raster + direct | 0.29 | 1273.4 | 1.75 | 0.70 | 0.09 | 44.0 | 0.03 | 15 | - | 332 | 13.48 |
| 509 | gpu: distance + sphere + quadtree + visbuffer + pathtrace | 0.29 | 1272.0 | 1.23 | 0.71 | 0.05 | 35.5 | 0.91 | 134 | - | 308 | 17.12 |
| 510 | gpu: points@cpu + splat + tree64 + rowrun + direct | 0.29 | 1268.2 | 1.86 | 0.71 | 0.06 | 58.2 | 0.00 | 9 | - | 350 | 13.88 |
| 511 | gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.29 | 1266.0 | 1.71 | 0.70 | 0.09 | 35.5 | 0.91 | 42 | - | 359 | 15.58 |
| 512 | gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.29 | 1266.0 | 1.85 | 0.70 | 0.09 | 44.0 | 0.03 | 7 | - | 334 | 13.22 |
| 513 | gpu: points@cpu + splat + bricks + refloor + pathtrace | 0.29 | 1264.7 | 1.16 | 0.75 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.28 |
| 514 | gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.28 | 1258.5 | 2.02 | 0.69 | 0.11 | 35.5 | 0.91 | 40 | - | 354 | 15.28 |
| 515 | gpu: naive@cpu + faces + tree64 + sphere + depth + raster + direct | 0.28 | 1257.9 | 1.78 | 0.71 | 0.09 | 44.0 | 0.03 | 8 | - | 308 | 12.24 |
| 516 | gpu: prefix + sphere + beam + composite + raycast + direct | 0.28 | 1257.8 | 1.33 | 0.75 | 0.01 | 58.3 | 0.00 | 13 | - | 304 | 12.26 |
| 517 | gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.28 | 1257.0 | 1.63 | 0.69 | 0.07 | 35.5 | 0.91 | 28 | - | 308 | 13.03 |
| 518 | gpu: svo_dag + refloor + quadtree + raster + direct | 0.28 | 1255.4 | 1.69 | 0.70 | 0.08 | 44.0 | 0.03 | 112 | - | 348 | 17.83 |
| 519 | gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.28 | 1255.1 | 3.33 | 0.63 | 0.17 | 35.5 | 0.91 | 16 | - | 336 | 13.61 |
| 520 | gpu: naive@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.28 | 1254.3 | 1.21 | 0.73 | 0.06 | 35.5 | 0.91 | 23 | - | 309 | 12.89 |
| 521 | gpu: distance + sphere + quadtree + gbuffer + pathtrace | 0.28 | 1252.4 | 1.29 | 0.72 | 0.05 | 35.5 | 0.91 | 136 | - | 308 | 17.19 |
| 522 | gpu: naive@cpu + faces + clipmap + sphere + depth + raster + direct | 0.28 | 1250.6 | 1.40 | 0.75 | 0.05 | 44.0 | 0.03 | 31 | - | 336 | 14.22 |
| 523 | gpu: points@cpu + splat + rows + refloor + pathtrace | 0.28 | 1250.6 | 1.71 | 0.76 | 0.01 | 36.2 | 0.88 | 25 | - | 348 | 14.45 |
| 524 | gpu: clipmap + rowrun + quadtree + visbuffer + direct | 0.28 | 1247.9 | 1.37 | 0.73 | 0.04 | 44.0 | 0.03 | 137 | - | 362 | 19.35 |
| 525 | gpu: greedy@cpu + rayquery + raycast + prefix + rowrun + pathtrace | 0.28 | 1245.7 | 3.09 | 0.65 | 0.19 | 36.2 | 0.88 | 33 | - | 416 | 17.39 |
| 526 | gpu: distance + incremental + quadtree + visbuffer + direct | 0.28 | 1243.3 | 1.34 | 0.73 | 0.04 | 44.0 | 0.03 | 126 | - | 359 | 18.79 |
| 527 | gpu: points@cpu + splat + prefix + rowrun + pathtrace | 0.28 | 1242.9 | 1.19 | 0.76 | 0.01 | 36.2 | 0.88 | 25 | - | 359 | 14.89 |
| 528 | gpu: distance + incremental + quadtree + gbuffer + direct | 0.28 | 1242.7 | 1.29 | 0.73 | 0.04 | 44.0 | 0.03 | 128 | - | 360 | 18.92 |
| 529 | gpu: clipmap + rowrun + quadtree + gbuffer + direct | 0.28 | 1236.8 | 1.40 | 0.73 | 0.05 | 44.0 | 0.03 | 139 | - | 363 | 19.45 |
| 530 | gpu: rows + sphere + composite + raycast + direct | 0.28 | 1234.0 | 1.38 | 0.77 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.25 |
| 531 | gpu: greedy@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.28 | 1231.6 | 1.86 | 0.73 | 0.09 | 35.5 | 0.91 | 23 | - | 337 | 13.96 |
| 532 | gpu: greedy@cpu + rayquery + raycast + clipmap + refloor + pathtrace | 0.28 | 1229.1 | 2.72 | 0.67 | 0.18 | 36.2 | 0.88 | 48 | - | 416 | 17.96 |
| 533 | gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.28 | 1227.8 | 1.20 | 0.77 | 0.04 | 35.5 | 0.91 | 39 | - | 353 | 15.21 |
| 534 | gpu: greedy@cpu + rayquery + raycast + distance + refloor + pathtrace | 0.28 | 1226.6 | 4.41 | 0.61 | 0.18 | 36.2 | 0.88 | 36 | - | 416 | 17.55 |
| 535 | gpu: clipmap + refloor + quadtree + gbuffer + direct | 0.28 | 1224.4 | 1.41 | 0.72 | 0.04 | 44.0 | 0.03 | 139 | - | 363 | 19.44 |
| 536 | gpu: rows + refloor + quadtree + raster + direct | 0.28 | 1222.5 | 1.31 | 0.73 | 0.03 | 44.0 | 0.03 | 120 | - | 343 | 17.96 |
| 537 | gpu: bricks + sphere + composite + raycast + direct | 0.28 | 1220.1 | 1.32 | 0.78 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.21 |
| 538 | gpu: points@cpu + splat + distance + rowrun + pathtrace | 0.28 | 1220.1 | 2.15 | 0.77 | 0.01 | 36.2 | 0.88 | 28 | - | 352 | 14.73 |
| 539 | gpu: rows + sphere + beam + composite + raycast + direct | 0.27 | 1218.4 | 1.26 | 0.78 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.24 |
| 540 | gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.27 | 1218.4 | 2.45 | 0.70 | 0.11 | 35.5 | 0.91 | 28 | - | 353 | 14.74 |
| 541 | gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.27 | 1215.6 | 1.42 | 0.77 | 0.04 | 35.5 | 0.91 | 25 | - | 351 | 14.57 |
| 542 | gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + direct | 0.27 | 1213.6 | 2.93 | 0.70 | 0.13 | 44.0 | 0.03 | 8 | - | 334 | 13.25 |
| 543 | gpu: tree64 + incremental + quadtree + visbuffer + direct | 0.27 | 1207.9 | 1.71 | 0.73 | 0.08 | 44.0 | 0.03 | 114 | - | 366 | 18.59 |
| 544 | gpu: tree64 + refloor + quadtree + raster + direct | 0.27 | 1207.5 | 2.05 | 0.73 | 0.08 | 44.0 | 0.03 | 112 | - | 350 | 17.91 |
| 545 | gpu: tree64 + incremental + quadtree + gbuffer + direct | 0.27 | 1202.9 | 1.95 | 0.74 | 0.07 | 44.0 | 0.03 | 116 | - | 366 | 18.66 |
| 546 | gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.27 | 1202.1 | 2.48 | 0.70 | 0.13 | 35.5 | 0.91 | 26 | - | 353 | 14.66 |
| 547 | gpu: prefix + rowrun + quadtree + raster + direct | 0.27 | 1197.9 | 1.75 | 0.76 | 0.03 | 44.0 | 0.03 | 120 | - | 344 | 17.99 |
| 548 | gpu: greedy@cpu + rayquery + raycast + distance + rowrun + pathtrace | 0.27 | 1197.9 | 3.26 | 0.66 | 0.20 | 36.2 | 0.88 | 36 | - | 416 | 17.52 |
| 549 | gpu: cubes@cpu + faces + prefix + sphere + depth + raster + direct | 0.27 | 1196.9 | 1.26 | 0.79 | 0.04 | 44.0 | 0.03 | 15 | - | 332 | 13.45 |
| 550 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.27 | 1191.4 | 1.90 | 0.76 | 0.08 | 44.0 | 0.03 | 7 | - | 335 | 13.26 |
| 551 | gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.27 | 1189.7 | 2.13 | 0.74 | 0.11 | 35.5 | 0.91 | 19 | - | 365 | 14.89 |
| 552 | gpu: greedy@cpu + rayquery + raycast + rows + refloor + pathtrace | 0.27 | 1189.3 | 2.92 | 0.69 | 0.19 | 36.2 | 0.88 | 33 | - | 417 | 17.44 |
| 553 | gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.27 | 1188.7 | 1.35 | 0.78 | 0.05 | 35.5 | 0.91 | 27 | - | 351 | 14.66 |
| 554 | gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.27 | 1188.6 | 1.89 | 0.75 | 0.10 | 35.5 | 0.91 | 17 | - | 360 | 14.62 |
| 555 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.27 | 1188.0 | 1.36 | 0.79 | 0.05 | 44.0 | 0.03 | 6 | - | 332 | 13.14 |
| 556 | gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.27 | 1186.9 | 1.48 | 0.79 | 0.04 | 35.5 | 0.91 | 41 | - | 359 | 15.53 |
| 557 | gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.27 | 1183.5 | 1.76 | 0.77 | 0.08 | 35.5 | 0.91 | 15 | - | 335 | 13.56 |
| 558 | gpu: clipmap + incremental + quadtree + visbuffer + direct | 0.27 | 1182.8 | 1.34 | 0.77 | 0.04 | 44.0 | 0.03 | 137 | - | 361 | 19.29 |
| 559 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.27 | 1181.6 | 2.50 | 0.73 | 0.12 | 44.0 | 0.03 | 7 | - | 333 | 13.21 |
| 560 | gpu: clipmap + incremental + quadtree + gbuffer + direct | 0.27 | 1179.3 | 1.33 | 0.77 | 0.04 | 44.0 | 0.03 | 139 | - | 361 | 19.38 |
| 561 | gpu: cubes@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.26 | 1171.6 | 1.74 | 0.81 | 0.04 | 35.5 | 0.91 | 26 | - | 335 | 14.02 |
| 562 | gpu: greedy@cpu + rayquery + raycast + bricks + refloor + pathtrace | 0.26 | 1171.5 | 3.26 | 0.69 | 0.20 | 36.2 | 0.88 | 32 | - | 415 | 17.36 |
| 563 | gpu: bricks + sphere + beam + composite + raycast + direct | 0.26 | 1171.1 | 1.44 | 0.79 | 0.01 | 58.3 | 0.00 | 12 | - | 303 | 12.21 |
| 564 | gpu: cubes@cpu + faces + bricks + sphere + depth + raster + direct | 0.26 | 1170.1 | 1.24 | 0.82 | 0.03 | 44.0 | 0.03 | 14 | - | 332 | 13.41 |
| 565 | gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + direct | 0.26 | 1169.3 | 2.10 | 0.77 | 0.10 | 44.0 | 0.03 | 31 | - | 336 | 14.22 |
| 566 | gpu: rows + rowrun + composite + raycast + pathtrace | 0.26 | 1163.7 | 1.19 | 0.81 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.09 |
| 567 | gpu: cubes@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.26 | 1159.5 | 1.35 | 0.83 | 0.04 | 35.5 | 0.91 | 23 | - | 335 | 13.88 |
| 568 | gpu: cubes@cpu + faces + rows + sphere + depth + raster + direct | 0.26 | 1158.1 | 1.31 | 0.81 | 0.05 | 44.0 | 0.03 | 15 | - | 331 | 13.42 |
| 569 | gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.26 | 1155.7 | 2.33 | 0.75 | 0.12 | 35.5 | 0.91 | 17 | - | 359 | 14.60 |
| 570 | gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.26 | 1151.3 | 1.61 | 0.81 | 0.07 | 35.5 | 0.91 | 25 | - | 309 | 12.95 |
| 571 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.26 | 1150.0 | 1.68 | 0.80 | 0.08 | 35.5 | 0.91 | 17 | - | 354 | 14.38 |
| 572 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.26 | 1148.6 | 1.46 | 0.81 | 0.06 | 35.5 | 0.91 | 14 | - | 336 | 13.57 |
| 573 | gpu: rows + rowrun + beam + composite + raycast + pathtrace | 0.26 | 1141.8 | 1.27 | 0.83 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 574 | gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.26 | 1141.7 | 3.06 | 0.74 | 0.15 | 35.5 | 0.91 | 19 | - | 364 | 14.88 |
| 575 | gpu: distance + refloor + quadtree + visbuffer + pathtrace | 0.26 | 1135.9 | 1.81 | 0.80 | 0.05 | 35.5 | 0.91 | 134 | - | 307 | 17.06 |
| 576 | gpu: clipmap + refloor + quadtree + raster + direct | 0.26 | 1134.3 | 1.35 | 0.81 | 0.04 | 44.0 | 0.03 | 135 | - | 345 | 18.61 |
| 577 | gpu: hier + sphere + quadtree + gbuffer + direct | 0.25 | 1130.5 | 1.90 | 0.81 | 0.05 | 44.0 | 0.03 | 124 | - | 359 | 18.71 |
| 578 | gpu: naive@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.25 | 1124.7 | 1.32 | 0.84 | 0.06 | 35.5 | 0.91 | 24 | - | 309 | 12.90 |
| 579 | gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 0.25 | 1121.3 | 1.47 | 0.82 | 0.04 | 35.5 | 0.91 | 130 | - | 309 | 17.01 |
| 580 | gpu: distance + rowrun + quadtree + raster + direct | 0.25 | 1118.4 | 1.60 | 0.80 | 0.04 | 44.0 | 0.03 | 124 | - | 345 | 18.17 |
| 581 | gpu: rows + rowrun + quadtree + raster + direct | 0.25 | 1115.5 | 1.37 | 0.81 | 0.04 | 44.0 | 0.03 | 120 | - | 345 | 18.02 |
| 582 | gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 0.25 | 1114.6 | 1.33 | 0.83 | 0.03 | 35.5 | 0.91 | 132 | - | 307 | 17.02 |
| 583 | gpu: distance + refloor + quadtree + gbuffer + pathtrace | 0.25 | 1113.9 | 1.37 | 0.82 | 0.05 | 35.5 | 0.91 | 136 | - | 308 | 17.19 |
| 584 | gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.25 | 1112.1 | 1.54 | 0.83 | 0.07 | 35.5 | 0.91 | 27 | - | 309 | 13.03 |
| 585 | gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 0.25 | 1110.0 | 1.26 | 0.86 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.81 |
| 586 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.25 | 1109.5 | 1.89 | 0.79 | 0.09 | 35.5 | 0.91 | 19 | - | 360 | 14.71 |
| 587 | gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.25 | 1107.9 | 2.01 | 0.83 | 0.08 | 35.5 | 0.91 | 18 | - | 364 | 14.82 |
| 588 | gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.25 | 1106.2 | 1.87 | 0.83 | 0.09 | 35.5 | 0.91 | 16 | - | 359 | 14.56 |
| 589 | gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.25 | 1104.6 | 1.46 | 0.86 | 0.06 | 35.5 | 0.91 | 27 | - | 308 | 13.01 |
| 590 | gpu: clipmap + refloor + composite + raycast + pathtrace | 0.25 | 1104.2 | 1.46 | 0.85 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.84 |
| 591 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.25 | 1101.7 | 1.69 | 0.83 | 0.09 | 35.5 | 0.91 | 23 | - | 296 | 12.39 |
| 592 | gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.25 | 1101.0 | 1.46 | 0.85 | 0.07 | 35.5 | 0.91 | 25 | - | 309 | 12.94 |
| 593 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.25 | 1099.2 | 1.51 | 0.85 | 0.07 | 35.5 | 0.91 | 23 | - | 297 | 12.41 |
| 594 | gpu: hier + refloor + quadtree + visbuffer + pathtrace | 0.25 | 1098.2 | 1.33 | 0.84 | 0.04 | 35.5 | 0.91 | 130 | - | 307 | 16.91 |
| 595 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.25 | 1096.9 | 2.14 | 0.81 | 0.11 | 35.5 | 0.91 | 19 | - | 360 | 14.69 |
| 596 | gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.25 | 1087.2 | 2.27 | 0.82 | 0.11 | 35.5 | 0.91 | 27 | - | 363 | 15.11 |
| 597 | gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.24 | 1086.2 | 2.11 | 0.81 | 0.12 | 35.5 | 0.91 | 25 | - | 357 | 14.82 |
| 598 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.24 | 1085.8 | 1.81 | 0.87 | 0.07 | 35.5 | 0.91 | 18 | - | 359 | 14.64 |
| 599 | gpu: points@cpu + splat + rows + rowrun + pathtrace | 0.24 | 1085.3 | 1.41 | 0.88 | 0.01 | 36.2 | 0.88 | 25 | - | 350 | 14.54 |
| 600 | gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + direct | 0.24 | 1081.9 | 1.96 | 0.85 | 0.09 | 44.0 | 0.03 | 7 | - | 333 | 13.18 |
| 601 | gpu: points@cpu + splat + clipmap + refloor + pathtrace | 0.24 | 1079.5 | 1.57 | 0.88 | 0.01 | 36.2 | 0.88 | 39 | - | 351 | 15.13 |
| 602 | gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.24 | 1079.4 | 1.61 | 0.88 | 0.07 | 35.5 | 0.91 | 26 | - | 309 | 12.99 |
| 603 | gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + direct | 0.24 | 1079.1 | 1.33 | 0.88 | 0.04 | 44.0 | 0.03 | 30 | - | 336 | 14.15 |
| 604 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.24 | 1078.8 | 2.04 | 0.82 | 0.11 | 35.5 | 0.91 | 17 | - | 353 | 14.37 |
| 605 | gpu: greedy@cpu + rayquery + raycast + rows + rowrun + pathtrace | 0.24 | 1075.6 | 3.05 | 0.78 | 0.19 | 36.2 | 0.88 | 33 | - | 416 | 17.39 |
| 606 | gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.24 | 1073.8 | 1.41 | 0.88 | 0.06 | 35.5 | 0.91 | 25 | - | 309 | 12.95 |
| 607 | gpu: cubes@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.24 | 1071.3 | 1.40 | 0.88 | 0.04 | 35.5 | 0.91 | 22 | - | 336 | 13.88 |
| 608 | gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.24 | 1071.2 | 1.68 | 0.87 | 0.07 | 35.5 | 0.91 | 40 | - | 359 | 15.50 |
| 609 | gpu: hier + refloor + quadtree + gbuffer + pathtrace | 0.24 | 1070.7 | 1.42 | 0.86 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 16.99 |
| 610 | gpu: greedy@cpu + rayquery + raycast + tree64 + incremental + pathtrace | 0.24 | 1067.0 | 3.95 | 0.75 | 0.24 | 36.2 | 0.88 | 25 | - | 418 | 17.15 |
| 611 | gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.24 | 1066.2 | 1.45 | 0.90 | 0.05 | 35.5 | 0.91 | 24 | - | 356 | 14.75 |
| 612 | gpu: svo_dag + rowrun + composite + raycast + pathtrace | 0.24 | 1065.4 | 1.66 | 0.90 | 0.05 | 36.2 | 0.88 | 12 | - | 293 | 11.82 |
| 613 | gpu: greedy@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.24 | 1065.0 | 1.87 | 0.86 | 0.09 | 35.5 | 0.91 | 24 | - | 337 | 13.97 |
| 614 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.24 | 1064.1 | 1.52 | 0.88 | 0.07 | 35.5 | 0.91 | 16 | - | 352 | 14.29 |
| 615 | gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.24 | 1064.1 | 1.57 | 0.89 | 0.06 | 35.5 | 0.91 | 28 | - | 308 | 13.04 |
| 616 | gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.24 | 1061.0 | 1.97 | 0.89 | 0.07 | 35.5 | 0.91 | 27 | - | 309 | 13.02 |
| 617 | gpu: rows + incremental + quadtree + raster + direct | 0.24 | 1060.3 | 1.40 | 0.88 | 0.03 | 44.0 | 0.03 | 120 | - | 344 | 17.98 |
| 618 | gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.24 | 1059.5 | 1.46 | 0.89 | 0.06 | 35.5 | 0.91 | 42 | - | 365 | 15.79 |
| 619 | gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.24 | 1059.2 | 1.46 | 0.89 | 0.07 | 35.5 | 0.91 | 26 | - | 309 | 12.97 |
| 620 | gpu: clipmap + sphere + composite + raycast + direct | 0.24 | 1058.7 | 1.35 | 0.90 | 0.01 | 58.3 | 0.00 | 27 | - | 307 | 12.96 |
| 621 | gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.24 | 1056.5 | 1.46 | 0.90 | 0.04 | 35.5 | 0.91 | 26 | - | 362 | 15.04 |
| 622 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.24 | 1052.6 | 1.48 | 0.90 | 0.06 | 44.0 | 0.03 | 6 | - | 334 | 13.17 |
| 623 | gpu: naive@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.24 | 1052.5 | 1.49 | 0.91 | 0.05 | 35.5 | 0.91 | 24 | - | 310 | 12.93 |
| 624 | gpu: hier + sphere + quadtree + visbuffer + direct | 0.24 | 1048.1 | 1.35 | 0.87 | 0.03 | 44.0 | 0.03 | 122 | - | 359 | 18.63 |
| 625 | gpu: naive@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.24 | 1047.7 | 1.72 | 0.91 | 0.05 | 35.5 | 0.91 | 27 | - | 310 | 13.06 |
| 626 | gpu: greedy@cpu + rayquery + raycast + clipmap + rowrun + pathtrace | 0.24 | 1046.9 | 2.91 | 0.80 | 0.18 | 36.2 | 0.88 | 48 | - | 416 | 17.99 |
| 627 | gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 0.24 | 1046.0 | 1.61 | 0.91 | 0.04 | 36.2 | 0.88 | 12 | - | 293 | 11.83 |
| 628 | gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 0.24 | 1044.7 | 1.96 | 0.87 | 0.07 | 35.5 | 0.91 | 122 | - | 312 | 16.80 |
| 629 | gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.24 | 1043.1 | 1.59 | 0.91 | 0.06 | 35.5 | 0.91 | 25 | - | 309 | 12.95 |
| 630 | gpu: hier + incremental + quadtree + raster + direct | 0.23 | 1039.7 | 1.83 | 0.88 | 0.04 | 44.0 | 0.03 | 120 | - | 343 | 17.96 |
| 631 | gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.23 | 1038.4 | 1.94 | 0.88 | 0.11 | 35.5 | 0.91 | 26 | - | 359 | 14.91 |
| 632 | gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 0.23 | 1035.9 | 1.99 | 0.87 | 0.09 | 35.5 | 0.91 | 122 | - | 313 | 16.86 |
| 633 | gpu: naive@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.23 | 1033.3 | 1.47 | 0.92 | 0.06 | 35.5 | 0.91 | 38 | - | 340 | 14.68 |
| 634 | gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.23 | 1033.2 | 2.15 | 0.89 | 0.10 | 35.5 | 0.91 | 28 | - | 364 | 15.20 |
| 635 | gpu: bricks + incremental + quadtree + raster + direct | 0.23 | 1032.5 | 1.65 | 0.90 | 0.04 | 44.0 | 0.03 | 120 | - | 343 | 17.95 |
| 636 | gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.23 | 1029.5 | 2.12 | 0.89 | 0.10 | 35.5 | 0.91 | 25 | - | 354 | 14.72 |
| 637 | gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.23 | 1029.1 | 1.54 | 0.91 | 0.07 | 35.5 | 0.91 | 28 | - | 309 | 13.05 |
| 638 | gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.23 | 1028.5 | 1.54 | 0.93 | 0.04 | 35.5 | 0.91 | 24 | - | 361 | 14.92 |
| 639 | gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 0.23 | 1025.9 | 1.74 | 0.88 | 0.09 | 35.5 | 0.91 | 124 | - | 313 | 16.94 |
| 640 | gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.23 | 1024.6 | 2.11 | 0.87 | 0.11 | 35.5 | 0.91 | 27 | - | 361 | 15.06 |
| 641 | gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.23 | 1023.2 | 2.16 | 0.87 | 0.11 | 35.5 | 0.91 | 25 | - | 362 | 15.00 |
| 642 | gpu: clipmap + sphere + beam + composite + raycast + direct | 0.23 | 1023.1 | 1.37 | 0.93 | 0.01 | 58.3 | 0.00 | 27 | - | 307 | 12.96 |
| 643 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + direct | 0.23 | 1021.8 | 1.99 | 0.87 | 0.05 | 44.0 | 0.03 | 120 | - | 323 | 17.17 |
| 644 | gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.23 | 1021.5 | 1.98 | 0.90 | 0.10 | 35.5 | 0.91 | 28 | - | 362 | 15.10 |
| 645 | gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.23 | 1019.8 | 2.05 | 0.91 | 0.09 | 35.5 | 0.91 | 27 | - | 362 | 15.09 |
| 646 | gpu: greedy@cpu + rayquery + raycast + svo_dag + rowrun + pathtrace | 0.23 | 1018.4 | 3.34 | 0.82 | 0.21 | 36.2 | 0.88 | 24 | - | 416 | 17.08 |
| 647 | gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.23 | 1018.3 | 2.36 | 0.89 | 0.11 | 35.5 | 0.91 | 27 | - | 354 | 14.79 |
| 648 | gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.23 | 1017.8 | 2.02 | 0.89 | 0.10 | 35.5 | 0.91 | 26 | - | 356 | 14.80 |
| 649 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.23 | 1015.7 | 2.02 | 0.89 | 0.09 | 35.5 | 0.91 | 42 | - | 365 | 15.79 |
| 650 | gpu: hier + incremental + composite + raycast + pathtrace | 0.23 | 1014.2 | 1.35 | 0.95 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.09 |
| 651 | gpu: points@cpu + splat + tree64 + incremental + pathtrace | 0.23 | 1014.1 | 1.52 | 0.95 | 0.06 | 36.2 | 0.88 | 16 | - | 358 | 14.53 |
| 652 | gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 0.23 | 1013.1 | 1.53 | 0.92 | 0.05 | 35.5 | 0.91 | 134 | - | 306 | 17.06 |
| 653 | gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.23 | 1011.8 | 1.46 | 0.95 | 0.04 | 35.5 | 0.91 | 26 | - | 364 | 15.12 |
| 654 | gpu: distance + incremental + quadtree + raster + direct | 0.23 | 1011.0 | 2.02 | 0.92 | 0.04 | 44.0 | 0.03 | 124 | - | 343 | 18.10 |
| 655 | gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.23 | 1010.6 | 1.76 | 0.92 | 0.07 | 35.5 | 0.91 | 27 | - | 308 | 13.01 |
| 656 | gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.23 | 1009.2 | 2.18 | 0.91 | 0.10 | 35.5 | 0.91 | 25 | - | 356 | 14.79 |
| 657 | gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 0.23 | 1004.3 | 2.18 | 0.93 | 0.08 | 36.2 | 0.88 | 12 | - | 295 | 11.90 |
| 658 | gpu: prefix + incremental + quadtree + raster + direct | 0.23 | 1003.6 | 1.92 | 0.92 | 0.04 | 44.0 | 0.03 | 120 | - | 344 | 17.99 |
| 659 | gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 0.23 | 1002.2 | 2.38 | 0.88 | 0.11 | 35.5 | 0.91 | 124 | - | 313 | 16.94 |
| 660 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.23 | 1001.2 | 1.53 | 0.95 | 0.04 | 35.5 | 0.91 | 39 | - | 358 | 15.42 |
| 661 | gpu: hier + rowrun + composite + raycast + pathtrace | 0.23 | 999.8 | 1.38 | 0.95 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.08 |
| 662 | gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.22 | 996.9 | 1.49 | 0.96 | 0.04 | 35.5 | 0.91 | 25 | - | 357 | 14.82 |
| 663 | gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 0.22 | 996.3 | 1.50 | 0.93 | 0.04 | 35.5 | 0.91 | 130 | - | 307 | 16.91 |
| 664 | gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.22 | 994.0 | 1.58 | 0.96 | 0.04 | 35.5 | 0.91 | 24 | - | 353 | 14.64 |
| 665 | gpu: prefix + sphere + quadtree + visbuffer + direct | 0.22 | 991.7 | 1.48 | 0.92 | 0.04 | 44.0 | 0.03 | 122 | - | 360 | 18.70 |
| 666 | gpu: svo_dag + rowrun + quadtree + visbuffer + direct | 0.22 | 990.6 | 1.70 | 0.94 | 0.06 | 44.0 | 0.03 | 114 | - | 362 | 18.43 |
| 667 | gpu: hier + incremental + beam + composite + raycast + pathtrace | 0.22 | 990.4 | 1.49 | 0.97 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 668 | gpu: prefix + sphere + quadtree + gbuffer + direct | 0.22 | 989.6 | 1.56 | 0.92 | 0.05 | 44.0 | 0.03 | 124 | - | 359 | 18.72 |
| 669 | gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.22 | 988.9 | 1.59 | 0.97 | 0.04 | 35.5 | 0.91 | 26 | - | 354 | 14.75 |
| 670 | gpu: tree64 + incremental + composite + raycast + pathtrace | 0.22 | 988.3 | 1.65 | 0.96 | 0.06 | 36.2 | 0.88 | 12 | - | 294 | 11.88 |
| 671 | gpu: greedy@cpu + rayquery + raycast + hier + incremental + pathtrace | 0.22 | 986.3 | 3.20 | 0.85 | 0.21 | 36.2 | 0.88 | 32 | - | 415 | 17.36 |
| 672 | gpu: svo_dag + rowrun + quadtree + gbuffer + direct | 0.22 | 986.1 | 1.64 | 0.93 | 0.08 | 44.0 | 0.03 | 116 | - | 364 | 18.59 |
| 673 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.22 | 985.1 | 2.77 | 0.88 | 0.13 | 35.5 | 0.91 | 40 | - | 359 | 15.49 |
| 674 | gpu: greedy@cpu + rayquery + raycast + bricks + rowrun + pathtrace | 0.22 | 984.3 | 3.39 | 0.86 | 0.20 | 36.2 | 0.88 | 32 | - | 416 | 17.40 |
| 675 | gpu: rows + refloor + quadtree + visbuffer + pathtrace | 0.22 | 982.1 | 1.39 | 0.95 | 0.03 | 35.5 | 0.91 | 130 | - | 307 | 16.93 |
| 676 | gpu: svo_dag + incremental + quadtree + visbuffer + direct | 0.22 | 980.9 | 1.89 | 0.92 | 0.09 | 44.0 | 0.03 | 114 | - | 364 | 18.52 |
| 677 | gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.22 | 979.9 | 1.67 | 0.97 | 0.05 | 35.5 | 0.91 | 27 | - | 363 | 15.13 |
| 678 | gpu: rows + sphere + quadtree + gbuffer + direct | 0.22 | 979.2 | 1.92 | 0.93 | 0.04 | 44.0 | 0.03 | 124 | - | 359 | 18.71 |
| 679 | gpu: bricks + sphere + quadtree + gbuffer + direct | 0.22 | 978.3 | 1.97 | 0.93 | 0.05 | 44.0 | 0.03 | 124 | - | 359 | 18.70 |
| 680 | gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.22 | 978.3 | 1.57 | 0.98 | 0.05 | 35.5 | 0.91 | 25 | - | 355 | 14.73 |
| 681 | gpu: svo_dag + incremental + quadtree + gbuffer + direct | 0.22 | 977.4 | 1.77 | 0.94 | 0.08 | 44.0 | 0.03 | 116 | - | 364 | 18.58 |
| 682 | gpu: bricks + sphere + quadtree + visbuffer + direct | 0.22 | 977.0 | 1.53 | 0.94 | 0.05 | 44.0 | 0.03 | 122 | - | 360 | 18.67 |
| 683 | gpu: rows + sphere + quadtree + visbuffer + direct | 0.22 | 976.5 | 1.55 | 0.94 | 0.04 | 44.0 | 0.03 | 122 | - | 359 | 18.65 |
| 684 | gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.22 | 974.8 | 1.61 | 0.98 | 0.05 | 35.5 | 0.91 | 27 | - | 361 | 15.01 |
| 685 | gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.22 | 974.2 | 2.35 | 0.93 | 0.11 | 35.5 | 0.91 | 38 | - | 340 | 14.68 |
| 686 | gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.22 | 973.3 | 2.07 | 0.94 | 0.10 | 35.5 | 0.91 | 24 | - | 337 | 14.00 |
| 687 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.22 | 972.0 | 1.76 | 0.97 | 0.05 | 35.5 | 0.91 | 41 | - | 364 | 15.72 |
| 688 | gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.22 | 970.8 | 1.45 | 0.99 | 0.05 | 35.5 | 0.91 | 24 | - | 356 | 14.73 |
| 689 | gpu: greedy@cpu + rayquery + raycast + clipmap + incremental + pathtrace | 0.22 | 970.6 | 2.77 | 0.90 | 0.18 | 36.2 | 0.88 | 48 | - | 416 | 17.96 |
| 690 | gpu: greedy@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.22 | 969.4 | 2.18 | 0.94 | 0.11 | 35.5 | 0.91 | 27 | - | 336 | 14.07 |
| 691 | gpu: tree64 + sphere + composite + raycast + direct | 0.22 | 968.8 | 2.15 | 0.98 | 0.07 | 58.3 | 0.00 | 4 | - | 305 | 12.01 |
| 692 | gpu: hier + rowrun + beam + composite + raycast + pathtrace | 0.22 | 967.1 | 1.55 | 0.99 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 693 | gpu: cubes@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.22 | 965.8 | 1.63 | 1.00 | 0.04 | 35.5 | 0.91 | 23 | - | 335 | 13.88 |
| 694 | gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 0.22 | 962.0 | 1.48 | 0.97 | 0.05 | 35.5 | 0.91 | 136 | - | 307 | 17.14 |
| 695 | gpu: bricks + rowrun + composite + raycast + pathtrace | 0.22 | 961.9 | 1.54 | 1.00 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.09 |
| 696 | gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.22 | 961.0 | 1.60 | 0.99 | 0.04 | 35.5 | 0.91 | 26 | - | 361 | 15.03 |
| 697 | gpu: greedy@cpu + rayquery + raycast + prefix + incremental + pathtrace | 0.22 | 957.6 | 3.12 | 0.89 | 0.20 | 36.2 | 0.88 | 33 | - | 416 | 17.40 |
| 698 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.22 | 956.9 | 1.47 | 1.01 | 0.04 | 35.5 | 0.91 | 22 | - | 296 | 12.33 |
| 699 | gpu: svo_dag + sphere + quadtree + gbuffer + direct | 0.22 | 954.6 | 1.56 | 0.96 | 0.06 | 44.0 | 0.03 | 116 | - | 361 | 18.47 |
| 700 | gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 0.21 | 950.2 | 1.57 | 1.00 | 0.01 | 36.2 | 0.88 | 20 | - | 293 | 12.11 |
| 701 | gpu: svo_dag + sphere + quadtree + visbuffer + direct | 0.21 | 949.3 | 1.72 | 0.96 | 0.08 | 44.0 | 0.03 | 114 | - | 364 | 18.50 |
| 702 | gpu: points@cpu + splat + hier + incremental + pathtrace | 0.21 | 949.1 | 1.78 | 1.02 | 0.01 | 36.2 | 0.88 | 24 | - | 355 | 14.72 |
| 703 | gpu: greedy@cpu + rayquery + raycast + rows + incremental + pathtrace | 0.21 | 942.6 | 3.21 | 0.91 | 0.20 | 36.2 | 0.88 | 33 | - | 415 | 17.36 |
| 704 | gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.21 | 941.6 | 1.77 | 1.00 | 0.07 | 35.5 | 0.91 | 31 | - | 308 | 13.16 |
| 705 | gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.21 | 940.9 | 1.51 | 1.02 | 0.05 | 35.5 | 0.91 | 42 | - | 365 | 15.80 |
| 706 | gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.21 | 939.5 | 1.57 | 1.03 | 0.04 | 35.5 | 0.91 | 23 | - | 337 | 13.93 |
| 707 | gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.21 | 938.7 | 1.57 | 1.02 | 0.06 | 35.5 | 0.91 | 40 | - | 360 | 15.53 |
| 708 | gpu: tree64 + sphere + beam + composite + raycast + direct | 0.21 | 937.9 | 1.92 | 1.00 | 0.07 | 58.3 | 0.00 | 4 | - | 306 | 12.04 |
| 709 | gpu: clipmap + sphere + quadtree + visbuffer + direct | 0.21 | 933.4 | 1.58 | 1.00 | 0.05 | 44.0 | 0.03 | 137 | - | 358 | 19.20 |
| 710 | gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.21 | 933.4 | 1.65 | 1.01 | 0.07 | 35.5 | 0.91 | 29 | - | 308 | 13.09 |
| 711 | gpu: points@cpu + splat + svo_dag + rowrun + pathtrace | 0.21 | 927.6 | 1.71 | 1.03 | 0.05 | 36.2 | 0.88 | 16 | - | 352 | 14.28 |
| 712 | gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.21 | 924.7 | 2.24 | 1.00 | 0.10 | 35.5 | 0.91 | 31 | - | 361 | 15.21 |
| 713 | gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 0.21 | 924.1 | 1.56 | 0.99 | 0.05 | 35.5 | 0.91 | 130 | - | 307 | 16.96 |
| 714 | gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.21 | 923.9 | 2.15 | 1.00 | 0.09 | 35.5 | 0.91 | 29 | - | 355 | 14.91 |
| 715 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.21 | 921.4 | 2.31 | 1.00 | 0.10 | 44.0 | 0.03 | 7 | - | 334 | 13.24 |
| 716 | gpu: greedy@cpu + rayquery + raycast + bricks + incremental + pathtrace | 0.21 | 919.5 | 2.82 | 0.94 | 0.18 | 36.2 | 0.88 | 32 | - | 416 | 17.38 |
| 717 | gpu: cubes@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.21 | 918.4 | 1.47 | 1.05 | 0.04 | 35.5 | 0.91 | 26 | - | 335 | 14.01 |
| 718 | gpu: greedy@cpu + rayquery + raycast + hier + rowrun + pathtrace | 0.21 | 916.4 | 2.61 | 0.97 | 0.18 | 36.2 | 0.88 | 32 | - | 415 | 17.36 |
| 719 | gpu: rows + refloor + quadtree + gbuffer + pathtrace | 0.21 | 911.5 | 1.72 | 1.01 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.01 |
| 720 | gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.21 | 909.8 | 2.35 | 1.02 | 0.10 | 35.5 | 0.91 | 40 | - | 360 | 15.52 |
| 721 | gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.20 | 909.1 | 2.17 | 1.02 | 0.09 | 44.0 | 0.03 | 8 | - | 336 | 13.32 |
| 722 | gpu: clipmap + rowrun + composite + raycast + pathtrace | 0.20 | 907.3 | 1.50 | 1.06 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.83 |
| 723 | gpu: rows + incremental + beam + composite + raycast + pathtrace | 0.20 | 906.9 | 1.59 | 1.06 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.12 |
| 724 | gpu: points@cpu + splat + clipmap + rowrun + pathtrace | 0.20 | 905.9 | 1.70 | 1.06 | 0.01 | 36.2 | 0.88 | 39 | - | 356 | 15.33 |
| 725 | gpu: prefix + incremental + composite + raycast + pathtrace | 0.20 | 904.0 | 2.50 | 1.06 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.12 |
| 726 | gpu: distance + sphere + quadtree + raster + pathtrace | 0.20 | 902.4 | 1.77 | 1.03 | 0.04 | 35.5 | 0.91 | 132 | - | 307 | 16.99 |
| 727 | gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 0.20 | 901.2 | 1.59 | 1.02 | 0.05 | 35.5 | 0.91 | 132 | - | 309 | 17.07 |
| 728 | gpu: prefix + incremental + beam + composite + raycast + pathtrace | 0.20 | 898.6 | 1.40 | 1.07 | 0.01 | 36.2 | 0.88 | 21 | - | 293 | 12.15 |
| 729 | gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 0.20 | 897.7 | 1.66 | 1.02 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.02 |
| 730 | gpu: points@cpu + splat + bricks + rowrun + pathtrace | 0.20 | 895.2 | 1.58 | 1.07 | 0.01 | 36.2 | 0.88 | 24 | - | 358 | 14.81 |
| 731 | gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.20 | 892.2 | 1.62 | 1.07 | 0.04 | 35.5 | 0.91 | 37 | - | 339 | 14.61 |
| 732 | gpu: greedy@cpu + rayquery + raycast + distance + incremental + pathtrace | 0.20 | 890.3 | 2.71 | 1.00 | 0.17 | 36.2 | 0.88 | 36 | - | 416 | 17.52 |
| 733 | gpu: points@cpu + splat + hier + rowrun + pathtrace | 0.20 | 888.8 | 1.80 | 1.09 | 0.01 | 36.2 | 0.88 | 24 | - | 352 | 14.59 |
| 734 | gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.20 | 888.2 | 2.17 | 1.03 | 0.10 | 35.5 | 0.91 | 42 | - | 366 | 15.82 |
| 735 | gpu: bricks + incremental + composite + raycast + pathtrace | 0.20 | 887.4 | 1.67 | 1.07 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.08 |
| 736 | gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.20 | 886.7 | 1.71 | 1.09 | 0.04 | 35.5 | 0.91 | 28 | - | 355 | 14.86 |
| 737 | gpu: points@cpu + splat + rows + incremental + pathtrace | 0.20 | 885.9 | 1.68 | 1.09 | 0.01 | 36.2 | 0.88 | 25 | - | 354 | 14.67 |
| 738 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 0.20 | 883.3 | 1.59 | 1.05 | 0.05 | 35.5 | 0.91 | 130 | - | 306 | 16.88 |
| 739 | gpu: naive@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.20 | 883.0 | 1.72 | 1.08 | 0.06 | 35.5 | 0.91 | 24 | - | 309 | 12.91 |
| 740 | gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.20 | 880.5 | 1.60 | 1.09 | 0.04 | 35.5 | 0.91 | 30 | - | 360 | 15.13 |
| 741 | gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 0.20 | 877.5 | 1.70 | 1.08 | 0.02 | 36.2 | 0.88 | 35 | - | 296 | 12.84 |
| 742 | gpu: greedy@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.20 | 873.7 | 1.98 | 1.09 | 0.08 | 35.5 | 0.91 | 24 | - | 335 | 13.90 |
| 743 | gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.20 | 873.7 | 1.67 | 1.10 | 0.04 | 35.5 | 0.91 | 39 | - | 359 | 15.46 |
| 744 | gpu: bricks + incremental + beam + composite + raycast + pathtrace | 0.20 | 873.6 | 1.71 | 1.09 | 0.01 | 36.2 | 0.88 | 20 | - | 293 | 12.11 |
| 745 | gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.20 | 871.6 | 1.87 | 1.10 | 0.05 | 35.5 | 0.91 | 41 | - | 364 | 15.72 |
| 746 | gpu: clipmap + sphere + quadtree + gbuffer + direct | 0.20 | 870.2 | 1.88 | 1.06 | 0.05 | 44.0 | 0.03 | 139 | - | 358 | 19.26 |
| 747 | gpu: points@cpu + splat + bricks + incremental + pathtrace | 0.20 | 867.5 | 1.66 | 1.11 | 0.01 | 36.2 | 0.88 | 24 | - | 354 | 14.68 |
| 748 | gpu: clipmap + rowrun + quadtree + raster + direct | 0.19 | 863.4 | 1.51 | 1.09 | 0.03 | 44.0 | 0.03 | 135 | - | 344 | 18.56 |
| 749 | gpu: tree64 + sphere + quadtree + visbuffer + direct | 0.19 | 860.2 | 1.80 | 1.07 | 0.08 | 44.0 | 0.03 | 114 | - | 368 | 18.67 |
| 750 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.19 | 858.6 | 2.94 | 1.05 | 0.12 | 44.0 | 0.03 | 8 | - | 336 | 13.32 |
| 751 | gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 0.19 | 858.6 | 1.76 | 1.09 | 0.05 | 35.5 | 0.91 | 132 | - | 308 | 17.05 |
| 752 | gpu: points@cpu + splat + prefix + incremental + pathtrace | 0.19 | 856.5 | 1.79 | 1.11 | 0.01 | 36.2 | 0.88 | 25 | - | 356 | 14.76 |
| 753 | gpu: tree64 + sphere + quadtree + gbuffer + direct | 0.19 | 855.8 | 2.15 | 1.07 | 0.09 | 44.0 | 0.03 | 116 | - | 365 | 18.66 |
| 754 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 0.19 | 853.5 | 1.73 | 1.08 | 0.05 | 35.5 | 0.91 | 132 | - | 308 | 17.05 |
| 755 | gpu: tree64 + incremental + quadtree + raster + direct | 0.19 | 853.5 | 2.46 | 1.06 | 0.10 | 44.0 | 0.03 | 112 | - | 351 | 17.93 |
| 756 | gpu: rows + incremental + composite + raycast + pathtrace | 0.19 | 852.1 | 1.79 | 1.12 | 0.01 | 36.2 | 0.88 | 20 | - | 291 | 12.09 |
| 757 | gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.19 | 833.8 | 1.77 | 1.16 | 0.06 | 35.5 | 0.91 | 25 | - | 308 | 12.93 |
| 758 | gpu: distance + incremental + composite + raycast + pathtrace | 0.19 | 831.4 | 1.50 | 1.16 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.24 |
| 759 | gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 0.19 | 829.7 | 1.73 | 1.12 | 0.04 | 35.5 | 0.91 | 130 | - | 306 | 16.92 |
| 760 | gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.19 | 827.2 | 1.82 | 1.16 | 0.08 | 35.5 | 0.91 | 27 | - | 308 | 12.99 |
| 761 | gpu: clipmap + incremental + quadtree + raster + direct | 0.19 | 825.3 | 1.74 | 1.13 | 0.04 | 44.0 | 0.03 | 135 | - | 344 | 18.55 |
| 762 | gpu: greedy@cpu + rayquery + raycast + svo_dag + incremental + pathtrace | 0.19 | 822.9 | 3.92 | 1.06 | 0.21 | 36.2 | 0.88 | 24 | - | 416 | 17.06 |
| 763 | gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.19 | 822.9 | 2.41 | 1.15 | 0.09 | 35.5 | 0.91 | 25 | - | 352 | 14.62 |
| 764 | gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 0.18 | 815.2 | 1.75 | 1.15 | 0.05 | 35.5 | 0.91 | 145 | - | 309 | 17.57 |
| 765 | gpu: distance + incremental + beam + composite + raycast + pathtrace | 0.18 | 812.7 | 1.72 | 1.19 | 0.01 | 36.2 | 0.88 | 24 | - | 292 | 12.25 |
| 766 | gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 0.18 | 808.8 | 1.75 | 1.16 | 0.05 | 35.5 | 0.91 | 147 | - | 309 | 17.65 |
| 767 | gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.18 | 807.3 | 2.58 | 1.16 | 0.11 | 35.5 | 0.91 | 27 | - | 357 | 14.90 |
| 768 | gpu: points@cpu + splat + distance + incremental + pathtrace | 0.18 | 805.2 | 2.53 | 1.20 | 0.01 | 36.2 | 0.88 | 28 | - | 353 | 14.79 |
| 769 | gpu: clipmap + incremental + composite + raycast + pathtrace | 0.18 | 804.3 | 1.55 | 1.20 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.83 |
| 770 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.18 | 803.1 | 2.27 | 1.17 | 0.10 | 35.5 | 0.91 | 17 | - | 352 | 14.30 |
| 771 | gpu: naive@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.18 | 802.9 | 2.15 | 1.18 | 0.10 | 35.5 | 0.91 | 16 | - | 310 | 12.62 |
| 772 | gpu: points@cpu + splat + clipmap + incremental + pathtrace | 0.18 | 801.4 | 1.76 | 1.21 | 0.01 | 36.2 | 0.88 | 39 | - | 358 | 15.40 |
| 773 | gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.18 | 800.6 | 1.79 | 1.19 | 0.09 | 35.5 | 0.91 | 17 | - | 352 | 14.32 |
| 774 | gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.18 | 798.9 | 1.74 | 1.20 | 0.08 | 35.5 | 0.91 | 19 | - | 358 | 14.62 |
| 775 | gpu: distance + refloor + quadtree + raster + pathtrace | 0.18 | 798.9 | 1.78 | 1.18 | 0.04 | 35.5 | 0.91 | 132 | - | 307 | 17.01 |
| 776 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.18 | 792.9 | 1.76 | 1.22 | 0.07 | 44.0 | 0.03 | 7 | - | 335 | 13.25 |
| 777 | gpu: prefix + refloor + quadtree + raster + pathtrace | 0.18 | 789.6 | 1.95 | 1.20 | 0.04 | 35.5 | 0.91 | 128 | - | 307 | 16.89 |
| 778 | gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.18 | 784.7 | 1.70 | 1.23 | 0.04 | 35.5 | 0.91 | 24 | - | 351 | 14.55 |
| 779 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.18 | 779.8 | 2.88 | 1.20 | 0.12 | 35.5 | 0.91 | 19 | - | 358 | 14.61 |
| 780 | gpu: greedy@cpu + rayquery + raycast + svo_dag + sphere + pathtrace | 0.18 | 777.2 | 3.22 | 1.15 | 0.20 | 36.2 | 0.88 | 24 | - | 416 | 17.08 |
| 781 | gpu: greedy@cpu + rayquery + raycast + tree64 + rowrun + pathtrace | 0.18 | 777.1 | 3.57 | 1.14 | 0.22 | 36.2 | 0.88 | 25 | - | 412 | 16.94 |
| 782 | gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.18 | 776.7 | 1.84 | 1.24 | 0.05 | 35.5 | 0.91 | 26 | - | 357 | 14.84 |
| 783 | gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.17 | 773.6 | 2.30 | 1.22 | 0.11 | 35.5 | 0.91 | 16 | - | 337 | 13.67 |
| 784 | gpu: hier + sphere + quadtree + raster + direct | 0.17 | 772.2 | 1.76 | 1.21 | 0.05 | 44.0 | 0.03 | 120 | - | 343 | 17.94 |
| 785 | gpu: cubes@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.17 | 771.9 | 1.78 | 1.26 | 0.04 | 35.5 | 0.91 | 23 | - | 334 | 13.83 |
| 786 | gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.17 | 771.7 | 1.92 | 1.25 | 0.07 | 35.5 | 0.91 | 28 | - | 309 | 13.04 |
| 787 | gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.17 | 771.6 | 1.98 | 1.25 | 0.07 | 35.5 | 0.91 | 26 | - | 308 | 12.95 |
| 788 | gpu: tree64 + refloor + quadtree + raster + pathtrace | 0.17 | 771.0 | 1.91 | 1.22 | 0.09 | 35.5 | 0.91 | 120 | - | 314 | 16.81 |
| 789 | gpu: tree64 + rowrun + quadtree + gbuffer + direct | 0.17 | 766.3 | 2.17 | 1.22 | 0.11 | 44.0 | 0.03 | 116 | - | 364 | 18.62 |
| 790 | gpu: hier + incremental + quadtree + visbuffer + pathtrace | 0.17 | 765.0 | 1.81 | 1.23 | 0.05 | 35.5 | 0.91 | 130 | - | 309 | 17.00 |
| 791 | gpu: greedy@cpu + rayquery + raycast + hier + sphere + pathtrace | 0.17 | 764.9 | 3.60 | 1.15 | 0.21 | 36.2 | 0.88 | 32 | - | 416 | 17.37 |
| 792 | gpu: tree64 + rowrun + quadtree + visbuffer + direct | 0.17 | 764.8 | 2.39 | 1.22 | 0.11 | 44.0 | 0.03 | 114 | - | 368 | 18.67 |
| 793 | gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 0.17 | 763.0 | 1.86 | 1.27 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.84 |
| 794 | gpu: hier + incremental + quadtree + gbuffer + pathtrace | 0.17 | 762.4 | 1.90 | 1.24 | 0.05 | 35.5 | 0.91 | 132 | - | 308 | 17.05 |
| 795 | gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.17 | 761.1 | 2.61 | 1.25 | 0.10 | 35.5 | 0.91 | 26 | - | 353 | 14.68 |
| 796 | gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 0.17 | 759.3 | 1.94 | 1.23 | 0.09 | 35.5 | 0.91 | 124 | - | 313 | 16.94 |
| 797 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.17 | 758.3 | 1.77 | 1.28 | 0.07 | 35.5 | 0.91 | 16 | - | 351 | 14.25 |
| 798 | gpu: svo_dag + refloor + quadtree + raster + pathtrace | 0.17 | 756.5 | 1.86 | 1.25 | 0.08 | 35.5 | 0.91 | 120 | - | 311 | 16.69 |
| 799 | gpu: hier + refloor + quadtree + raster + pathtrace | 0.17 | 756.5 | 1.75 | 1.25 | 0.04 | 35.5 | 0.91 | 128 | - | 307 | 16.86 |
| 800 | gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.17 | 755.1 | 2.28 | 1.25 | 0.11 | 35.5 | 0.91 | 28 | - | 358 | 14.96 |
| 801 | gpu: bricks + rowrun + quadtree + raster + direct | 0.17 | 753.5 | 1.80 | 1.24 | 0.04 | 44.0 | 0.03 | 120 | - | 345 | 18.02 |
| 802 | gpu: hier + rowrun + quadtree + raster + direct | 0.17 | 753.4 | 1.89 | 1.24 | 0.05 | 44.0 | 0.03 | 120 | - | 344 | 17.96 |
| 803 | gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 0.17 | 751.2 | 2.66 | 1.24 | 0.11 | 35.5 | 0.91 | 122 | - | 315 | 16.94 |
| 804 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.17 | 751.0 | 1.65 | 1.29 | 0.07 | 35.5 | 0.91 | 18 | - | 357 | 14.54 |
| 805 | gpu: greedy@cpu + rayquery + raycast + tree64 + sphere + pathtrace | 0.17 | 741.5 | 3.61 | 1.20 | 0.22 | 36.2 | 0.88 | 25 | - | 417 | 17.10 |
| 806 | gpu: greedy@cpu + rayquery + raycast + prefix + sphere + pathtrace | 0.17 | 737.6 | 3.13 | 1.23 | 0.19 | 36.2 | 0.88 | 33 | - | 416 | 17.42 |
| 807 | gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.17 | 735.3 | 1.92 | 1.32 | 0.08 | 35.5 | 0.91 | 17 | - | 357 | 14.51 |
| 808 | gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 0.16 | 731.4 | 2.00 | 1.30 | 0.04 | 35.5 | 0.91 | 132 | - | 307 | 16.98 |
| 809 | gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.16 | 731.4 | 1.80 | 1.33 | 0.04 | 35.5 | 0.91 | 25 | - | 352 | 14.62 |
| 810 | gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.16 | 730.9 | 1.99 | 1.32 | 0.09 | 35.5 | 0.91 | 19 | - | 363 | 14.80 |
| 811 | gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 0.16 | 730.7 | 2.20 | 1.29 | 0.04 | 35.5 | 0.91 | 130 | - | 307 | 16.92 |
| 812 | gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.16 | 726.5 | 1.84 | 1.34 | 0.04 | 35.5 | 0.91 | 27 | - | 357 | 14.89 |
| 813 | gpu: rows + incremental + quadtree + visbuffer + pathtrace | 0.16 | 726.0 | 1.91 | 1.30 | 0.05 | 35.5 | 0.91 | 130 | - | 307 | 16.94 |
| 814 | gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.16 | 725.6 | 2.29 | 1.32 | 0.10 | 35.5 | 0.91 | 19 | - | 363 | 14.82 |
| 815 | gpu: rows + incremental + quadtree + gbuffer + pathtrace | 0.16 | 724.6 | 1.81 | 1.31 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.02 |
| 816 | gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.16 | 721.2 | 1.98 | 1.35 | 0.07 | 35.5 | 0.91 | 26 | - | 308 | 12.96 |
| 817 | gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 0.16 | 719.8 | 2.07 | 1.31 | 0.04 | 35.5 | 0.91 | 130 | - | 309 | 17.02 |
| 818 | gpu: prefix + sphere + quadtree + raster + direct | 0.16 | 719.5 | 2.10 | 1.31 | 0.05 | 44.0 | 0.03 | 120 | - | 345 | 18.04 |
| 819 | gpu: svo_dag + sphere + quadtree + raster + direct | 0.16 | 717.6 | 1.98 | 1.31 | 0.08 | 44.0 | 0.03 | 112 | - | 349 | 17.84 |
| 820 | gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.16 | 715.9 | 2.30 | 1.34 | 0.10 | 35.5 | 0.91 | 17 | - | 358 | 14.56 |
| 821 | gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.16 | 715.5 | 1.86 | 1.35 | 0.07 | 35.5 | 0.91 | 25 | - | 308 | 12.92 |
| 822 | gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 0.16 | 714.2 | 2.14 | 1.32 | 0.05 | 35.5 | 0.91 | 130 | - | 306 | 16.90 |
| 823 | gpu: rows + sphere + quadtree + raster + direct | 0.16 | 713.8 | 1.95 | 1.32 | 0.04 | 44.0 | 0.03 | 120 | - | 343 | 17.95 |
| 824 | gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.16 | 713.6 | 1.92 | 1.35 | 0.08 | 35.5 | 0.91 | 28 | - | 309 | 13.04 |
| 825 | gpu: naive@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.16 | 713.3 | 2.01 | 1.36 | 0.07 | 35.5 | 0.91 | 23 | - | 309 | 12.89 |
| 826 | gpu: points@cpu + splat + hier + sphere + pathtrace | 0.16 | 712.7 | 1.97 | 1.37 | 0.01 | 36.2 | 0.88 | 24 | - | 349 | 14.47 |
| 827 | gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 0.16 | 712.7 | 2.48 | 1.32 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.02 |
| 828 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.16 | 711.7 | 2.66 | 1.32 | 0.12 | 35.5 | 0.91 | 19 | - | 363 | 14.80 |
| 829 | gpu: points@cpu + splat + svo_dag + sphere + pathtrace | 0.16 | 711.6 | 1.83 | 1.37 | 0.05 | 36.2 | 0.88 | 16 | - | 350 | 14.18 |
| 830 | gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.16 | 711.0 | 2.02 | 1.36 | 0.07 | 35.5 | 0.91 | 27 | - | 308 | 13.00 |
| 831 | gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.16 | 710.6 | 2.03 | 1.37 | 0.08 | 35.5 | 0.91 | 15 | - | 336 | 13.60 |
| 832 | gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 0.16 | 710.4 | 1.97 | 1.33 | 0.04 | 35.5 | 0.91 | 132 | - | 307 | 16.99 |
| 833 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.16 | 708.2 | 3.11 | 1.33 | 0.12 | 35.5 | 0.91 | 19 | - | 363 | 14.84 |
| 834 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.16 | 708.2 | 2.93 | 1.32 | 0.13 | 35.5 | 0.91 | 17 | - | 357 | 14.49 |
| 835 | gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.16 | 706.4 | 2.54 | 1.35 | 0.11 | 35.5 | 0.91 | 26 | - | 351 | 14.62 |
| 836 | gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.16 | 706.3 | 1.95 | 1.37 | 0.08 | 35.5 | 0.91 | 27 | - | 357 | 14.88 |
| 837 | gpu: greedy@cpu + rayquery + raycast + bricks + sphere + pathtrace | 0.16 | 705.9 | 2.87 | 1.30 | 0.19 | 36.2 | 0.88 | 32 | - | 416 | 17.37 |
| 838 | gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 0.16 | 705.9 | 2.07 | 1.34 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.01 |
| 839 | gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.16 | 705.8 | 1.99 | 1.35 | 0.10 | 35.5 | 0.91 | 25 | - | 351 | 14.58 |
| 840 | gpu: greedy@cpu + rayquery + raycast + rows + sphere + pathtrace | 0.16 | 705.8 | 2.93 | 1.30 | 0.19 | 36.2 | 0.88 | 33 | - | 412 | 17.23 |
| 841 | gpu: naive@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.16 | 702.7 | 1.96 | 1.39 | 0.07 | 35.5 | 0.91 | 24 | - | 309 | 12.92 |
| 842 | gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.16 | 702.1 | 2.35 | 1.36 | 0.11 | 35.5 | 0.91 | 28 | - | 357 | 14.91 |
| 843 | gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 0.16 | 698.6 | 2.01 | 1.35 | 0.05 | 35.5 | 0.91 | 145 | - | 307 | 17.51 |
| 844 | gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.16 | 698.1 | 2.39 | 1.37 | 0.11 | 35.5 | 0.91 | 19 | - | 361 | 14.75 |
| 845 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.16 | 697.3 | 2.07 | 1.39 | 0.08 | 35.5 | 0.91 | 16 | - | 356 | 14.42 |
| 846 | gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.16 | 695.9 | 2.45 | 1.37 | 0.11 | 35.5 | 0.91 | 17 | - | 356 | 14.47 |
| 847 | gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 0.16 | 693.8 | 2.01 | 1.37 | 0.05 | 35.5 | 0.91 | 147 | - | 307 | 17.60 |
| 848 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.16 | 693.3 | 2.73 | 1.35 | 0.12 | 35.5 | 0.91 | 17 | - | 357 | 14.51 |
| 849 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.16 | 691.6 | 2.13 | 1.40 | 0.08 | 35.5 | 0.91 | 18 | - | 362 | 14.73 |
| 850 | gpu: svo_dag + incremental + quadtree + raster + direct | 0.16 | 691.6 | 1.99 | 1.38 | 0.09 | 44.0 | 0.03 | 112 | - | 350 | 17.88 |
| 851 | gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.16 | 690.9 | 2.23 | 1.41 | 0.06 | 35.5 | 0.91 | 38 | - | 340 | 14.66 |
| 852 | gpu: bricks + sphere + quadtree + raster + direct | 0.16 | 690.6 | 2.00 | 1.36 | 0.04 | 44.0 | 0.03 | 120 | - | 343 | 17.93 |
| 853 | gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.16 | 690.4 | 2.95 | 1.37 | 0.13 | 35.5 | 0.91 | 17 | - | 355 | 14.44 |
| 854 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.16 | 688.1 | 2.54 | 1.40 | 0.10 | 35.5 | 0.91 | 16 | - | 357 | 14.46 |
| 855 | gpu: greedy@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.15 | 684.0 | 2.18 | 1.40 | 0.11 | 35.5 | 0.91 | 23 | - | 339 | 14.04 |
| 856 | gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 0.15 | 683.3 | 2.01 | 1.37 | 0.05 | 35.5 | 0.91 | 130 | - | 309 | 16.99 |
| 857 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.15 | 683.0 | 2.24 | 1.42 | 0.09 | 35.5 | 0.91 | 18 | - | 362 | 14.75 |
| 858 | gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.15 | 680.5 | 2.05 | 1.43 | 0.04 | 35.5 | 0.91 | 25 | - | 351 | 14.56 |
| 859 | gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.15 | 676.9 | 2.14 | 1.44 | 0.05 | 35.5 | 0.91 | 24 | - | 350 | 14.52 |
| 860 | gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.15 | 674.7 | 2.12 | 1.44 | 0.05 | 35.5 | 0.91 | 27 | - | 356 | 14.85 |
| 861 | gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.15 | 673.1 | 3.11 | 1.37 | 0.16 | 35.5 | 0.91 | 19 | - | 361 | 14.75 |
| 862 | gpu: rows + refloor + quadtree + raster + pathtrace | 0.15 | 672.5 | 1.94 | 1.41 | 0.05 | 35.5 | 0.91 | 128 | - | 307 | 16.86 |
| 863 | gpu: points@cpu + splat + prefix + sphere + pathtrace | 0.15 | 671.7 | 2.05 | 1.45 | 0.02 | 36.2 | 0.88 | 25 | - | 349 | 14.49 |
| 864 | gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.15 | 671.2 | 2.04 | 1.45 | 0.06 | 35.5 | 0.91 | 26 | - | 356 | 14.82 |
| 865 | gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.15 | 669.4 | 2.32 | 1.45 | 0.09 | 35.5 | 0.91 | 16 | - | 355 | 14.38 |
| 866 | gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.15 | 665.8 | 1.95 | 1.46 | 0.09 | 35.5 | 0.91 | 18 | - | 360 | 14.69 |
| 867 | gpu: naive@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.15 | 663.9 | 2.02 | 1.47 | 0.06 | 35.5 | 0.91 | 24 | - | 309 | 12.91 |
| 868 | gpu: greedy@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.15 | 663.4 | 2.39 | 1.44 | 0.11 | 35.5 | 0.91 | 24 | - | 340 | 14.10 |
| 869 | gpu: hier + sphere + composite + raycast + pathtrace | 0.15 | 662.4 | 1.99 | 1.47 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.09 |
| 870 | gpu: greedy@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.15 | 662.3 | 2.12 | 1.45 | 0.10 | 35.5 | 0.91 | 24 | - | 338 | 14.02 |
| 871 | gpu: hier + sphere + beam + composite + raycast + pathtrace | 0.15 | 660.2 | 2.14 | 1.47 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.09 |
| 872 | gpu: distance + incremental + quadtree + gbuffer + pathtrace | 0.15 | 660.2 | 2.18 | 1.44 | 0.05 | 35.5 | 0.91 | 136 | - | 308 | 17.21 |
| 873 | gpu: greedy@cpu + rayquery + raycast + clipmap + sphere + pathtrace | 0.15 | 659.2 | 3.04 | 1.41 | 0.19 | 36.2 | 0.88 | 48 | - | 412 | 17.83 |
| 874 | gpu: distance + incremental + quadtree + visbuffer + pathtrace | 0.15 | 658.9 | 2.00 | 1.44 | 0.05 | 35.5 | 0.91 | 134 | - | 307 | 17.07 |
| 875 | gpu: naive@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.15 | 652.2 | 1.99 | 1.50 | 0.07 | 35.5 | 0.91 | 38 | - | 340 | 14.66 |
| 876 | gpu: tree64 + rowrun + composite + raycast + pathtrace | 0.15 | 651.3 | 2.54 | 1.50 | 0.08 | 36.2 | 0.88 | 12 | - | 294 | 11.88 |
| 877 | gpu: tree64 + sphere + quadtree + raster + direct | 0.15 | 649.2 | 2.21 | 1.45 | 0.10 | 44.0 | 0.03 | 112 | - | 348 | 17.84 |
| 878 | gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 0.15 | 645.7 | 2.27 | 1.50 | 0.08 | 36.2 | 0.88 | 12 | - | 294 | 11.88 |
| 879 | gpu: points@cpu + splat + svo_dag + incremental + pathtrace | 0.15 | 644.0 | 2.21 | 1.52 | 0.06 | 36.2 | 0.88 | 16 | - | 354 | 14.36 |
| 880 | gpu: greedy@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.15 | 643.4 | 2.34 | 1.50 | 0.10 | 35.5 | 0.91 | 23 | - | 338 | 13.99 |
| 881 | gpu: clipmap + sphere + quadtree + raster + direct | 0.14 | 639.7 | 2.03 | 1.50 | 0.04 | 44.0 | 0.03 | 135 | - | 343 | 18.53 |
| 882 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.14 | 638.9 | 2.27 | 1.50 | 0.11 | 35.5 | 0.91 | 38 | - | 340 | 14.66 |
| 883 | gpu: points@cpu + splat + tree64 + rowrun + pathtrace | 0.14 | 636.3 | 2.40 | 1.53 | 0.06 | 36.2 | 0.88 | 16 | - | 357 | 14.46 |
| 884 | gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 0.14 | 635.5 | 2.30 | 1.50 | 0.08 | 35.5 | 0.91 | 124 | - | 308 | 16.72 |
| 885 | gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 0.14 | 629.9 | 2.19 | 1.52 | 0.05 | 35.5 | 0.91 | 145 | - | 309 | 17.58 |
| 886 | gpu: points@cpu + splat + bricks + sphere + pathtrace | 0.14 | 626.3 | 2.13 | 1.56 | 0.02 | 36.2 | 0.88 | 24 | - | 349 | 14.46 |
| 887 | gpu: points@cpu + splat + rows + sphere + pathtrace | 0.14 | 626.1 | 2.66 | 1.56 | 0.01 | 36.2 | 0.88 | 25 | - | 349 | 14.49 |
| 888 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 0.14 | 625.8 | 2.22 | 1.52 | 0.05 | 35.5 | 0.91 | 128 | - | 306 | 16.82 |
| 889 | gpu: points@cpu + splat + tree64 + sphere + pathtrace | 0.14 | 625.6 | 2.29 | 1.56 | 0.08 | 36.2 | 0.88 | 16 | - | 354 | 14.35 |
| 890 | gpu: svo_dag + sphere + composite + raycast + pathtrace | 0.14 | 625.5 | 2.32 | 1.56 | 0.06 | 36.2 | 0.88 | 12 | - | 293 | 11.83 |
| 891 | gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 0.14 | 625.1 | 2.15 | 1.56 | 0.05 | 36.2 | 0.88 | 12 | - | 293 | 11.83 |
| 892 | gpu: cubes@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.14 | 625.0 | 2.30 | 1.56 | 0.04 | 35.5 | 0.91 | 22 | - | 338 | 13.97 |
| 893 | gpu: naive@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.14 | 625.0 | 2.09 | 1.55 | 0.06 | 35.5 | 0.91 | 23 | - | 309 | 12.89 |
| 894 | gpu: naive@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.14 | 624.7 | 2.27 | 1.56 | 0.07 | 35.5 | 0.91 | 23 | - | 309 | 12.89 |
| 895 | gpu: naive@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.14 | 622.2 | 2.02 | 1.57 | 0.07 | 35.5 | 0.91 | 27 | - | 310 | 13.06 |
| 896 | gpu: greedy@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.14 | 621.9 | 2.24 | 1.56 | 0.10 | 35.5 | 0.91 | 27 | - | 338 | 14.14 |
| 897 | gpu: cubes@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.14 | 618.8 | 2.08 | 1.58 | 0.05 | 35.5 | 0.91 | 23 | - | 334 | 13.85 |
| 898 | gpu: bricks + refloor + quadtree + raster + pathtrace | 0.14 | 618.7 | 2.16 | 1.54 | 0.05 | 35.5 | 0.91 | 128 | - | 307 | 16.86 |
| 899 | gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 0.14 | 618.6 | 2.14 | 1.54 | 0.04 | 35.5 | 0.91 | 147 | - | 307 | 17.60 |
| 900 | gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.14 | 617.5 | 2.35 | 1.56 | 0.10 | 35.5 | 0.91 | 38 | - | 340 | 14.66 |
| 901 | gpu: distance + rowrun + quadtree + raster + pathtrace | 0.14 | 615.8 | 2.20 | 1.55 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.00 |
| 902 | gpu: svo_dag + incremental + composite + raycast + pathtrace | 0.14 | 614.0 | 2.28 | 1.59 | 0.06 | 36.2 | 0.88 | 12 | - | 293 | 11.82 |
| 903 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.14 | 612.4 | 2.16 | 1.59 | 0.05 | 35.5 | 0.91 | 37 | - | 339 | 14.58 |
| 904 | gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 0.14 | 607.4 | 2.25 | 1.60 | 0.05 | 36.2 | 0.88 | 12 | - | 294 | 11.84 |
| 905 | gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.14 | 606.8 | 2.17 | 1.61 | 0.07 | 35.5 | 0.91 | 42 | - | 361 | 15.62 |
| 906 | gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 0.14 | 606.8 | 2.45 | 1.56 | 0.08 | 35.5 | 0.91 | 122 | - | 310 | 16.71 |
| 907 | gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.14 | 606.1 | 2.21 | 1.61 | 0.10 | 35.5 | 0.91 | 42 | - | 360 | 15.60 |
| 908 | gpu: clipmap + refloor + quadtree + raster + pathtrace | 0.14 | 605.5 | 2.17 | 1.58 | 0.04 | 35.5 | 0.91 | 143 | - | 308 | 17.46 |
| 909 | gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.14 | 604.8 | 2.31 | 1.61 | 0.08 | 35.5 | 0.91 | 40 | - | 355 | 15.34 |
| 910 | gpu: cubes@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.14 | 603.4 | 2.34 | 1.62 | 0.04 | 35.5 | 0.91 | 23 | - | 337 | 13.94 |
| 911 | gpu: prefix + rowrun + quadtree + raster + pathtrace | 0.14 | 600.9 | 2.28 | 1.58 | 0.05 | 35.5 | 0.91 | 128 | - | 308 | 16.90 |
| 912 | gpu: prefix + sphere + beam + composite + raycast + pathtrace | 0.14 | 598.9 | 2.06 | 1.63 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.11 |
| 913 | gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.13 | 598.2 | 2.38 | 1.61 | 0.11 | 35.5 | 0.91 | 40 | - | 355 | 15.33 |
| 914 | gpu: hier + sphere + quadtree + visbuffer + pathtrace | 0.13 | 595.1 | 2.29 | 1.61 | 0.05 | 35.5 | 0.91 | 130 | - | 306 | 16.89 |
| 915 | gpu: prefix + sphere + composite + raycast + pathtrace | 0.13 | 592.9 | 2.15 | 1.65 | 0.01 | 36.2 | 0.88 | 21 | - | 292 | 12.10 |
| 916 | gpu: hier + sphere + quadtree + gbuffer + pathtrace | 0.13 | 590.6 | 2.23 | 1.62 | 0.06 | 35.5 | 0.91 | 132 | - | 306 | 16.96 |
| 917 | gpu: greedy@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.13 | 584.3 | 2.48 | 1.64 | 0.11 | 35.5 | 0.91 | 23 | - | 336 | 13.93 |
| 918 | gpu: cubes@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.13 | 582.4 | 2.30 | 1.68 | 0.06 | 35.5 | 0.91 | 22 | - | 337 | 13.93 |
| 919 | gpu: naive@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.13 | 580.0 | 2.27 | 1.68 | 0.07 | 35.5 | 0.91 | 23 | - | 309 | 12.89 |
| 920 | gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.13 | 577.5 | 2.34 | 1.67 | 0.10 | 35.5 | 0.91 | 23 | - | 336 | 13.93 |
| 921 | gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.13 | 577.0 | 2.31 | 1.69 | 0.05 | 35.5 | 0.91 | 39 | - | 354 | 15.26 |
| 922 | gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.13 | 576.3 | 2.17 | 1.69 | 0.06 | 35.5 | 0.91 | 41 | - | 360 | 15.54 |
| 923 | gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.13 | 574.9 | 2.33 | 1.70 | 0.05 | 35.5 | 0.91 | 37 | - | 339 | 14.59 |
| 924 | gpu: cubes@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.13 | 570.4 | 2.40 | 1.71 | 0.05 | 35.5 | 0.91 | 22 | - | 335 | 13.85 |
| 925 | gpu: bricks + sphere + composite + raycast + pathtrace | 0.13 | 561.4 | 2.33 | 1.74 | 0.01 | 36.2 | 0.88 | 20 | - | 292 | 12.07 |
| 926 | gpu: rows + sphere + composite + raycast + pathtrace | 0.13 | 558.5 | 2.36 | 1.75 | 0.02 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 927 | gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 0.13 | 556.6 | 2.43 | 1.73 | 0.09 | 35.5 | 0.91 | 122 | - | 310 | 16.72 |
| 928 | gpu: cubes@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.13 | 556.4 | 2.30 | 1.76 | 0.06 | 35.5 | 0.91 | 26 | - | 336 | 14.05 |
| 929 | gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 0.13 | 555.6 | 2.38 | 1.72 | 0.05 | 35.5 | 0.91 | 130 | - | 306 | 16.92 |
| 930 | gpu: rows + rowrun + quadtree + raster + pathtrace | 0.13 | 555.5 | 2.36 | 1.72 | 0.04 | 35.5 | 0.91 | 128 | - | 307 | 16.87 |
| 931 | gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 0.13 | 555.4 | 2.38 | 1.73 | 0.05 | 35.5 | 0.91 | 132 | - | 306 | 16.99 |
| 932 | gpu: rows + sphere + beam + composite + raycast + pathtrace | 0.12 | 553.1 | 2.61 | 1.77 | 0.02 | 36.2 | 0.88 | 20 | - | 292 | 12.10 |
| 933 | gpu: bricks + sphere + beam + composite + raycast + pathtrace | 0.12 | 549.2 | 2.30 | 1.77 | 0.02 | 36.2 | 0.88 | 20 | - | 292 | 12.07 |
| 934 | gpu: naive@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.12 | 548.2 | 2.45 | 1.79 | 0.07 | 35.5 | 0.91 | 23 | - | 309 | 12.88 |
| 935 | gpu: greedy@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.12 | 546.6 | 2.43 | 1.78 | 0.11 | 35.5 | 0.91 | 23 | - | 334 | 13.87 |
| 936 | gpu: points@cpu + splat + clipmap + sphere + pathtrace | 0.12 | 544.0 | 2.26 | 1.80 | 0.01 | 36.2 | 0.88 | 39 | - | 352 | 15.18 |
| 937 | gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.12 | 539.3 | 2.48 | 1.81 | 0.05 | 35.5 | 0.91 | 22 | - | 335 | 13.85 |
| 938 | gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 0.12 | 536.4 | 2.49 | 1.78 | 0.09 | 35.5 | 0.91 | 124 | - | 307 | 16.70 |
| 939 | gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 0.12 | 527.3 | 2.57 | 1.82 | 0.05 | 35.5 | 0.91 | 130 | - | 308 | 16.96 |
| 940 | gpu: rows + sphere + quadtree + visbuffer + pathtrace | 0.12 | 526.6 | 2.51 | 1.82 | 0.05 | 35.5 | 0.91 | 130 | - | 306 | 16.92 |
| 941 | gpu: rows + sphere + quadtree + gbuffer + pathtrace | 0.12 | 526.3 | 2.57 | 1.83 | 0.05 | 35.5 | 0.91 | 132 | - | 306 | 16.98 |
| 942 | gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 0.12 | 523.8 | 2.49 | 1.84 | 0.06 | 35.5 | 0.91 | 132 | - | 306 | 16.96 |
| 943 | gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 0.12 | 518.2 | 2.53 | 1.86 | 0.10 | 35.5 | 0.91 | 124 | - | 310 | 16.82 |
| 944 | gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 0.12 | 515.5 | 2.57 | 1.87 | 0.09 | 35.5 | 0.91 | 122 | - | 308 | 16.64 |
| 945 | gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 0.12 | 510.6 | 2.57 | 1.92 | 0.09 | 36.2 | 0.88 | 12 | - | 294 | 11.86 |
| 946 | gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 0.11 | 505.1 | 2.41 | 1.91 | 0.11 | 35.5 | 0.91 | 122 | - | 313 | 16.87 |
| 947 | gpu: naive@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.11 | 504.2 | 2.49 | 1.95 | 0.08 | 35.5 | 0.91 | 24 | - | 309 | 12.92 |
| 948 | gpu: greedy@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.11 | 503.5 | 2.43 | 1.95 | 0.10 | 35.5 | 0.91 | 24 | - | 334 | 13.89 |
| 949 | gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 0.11 | 501.3 | 2.59 | 1.92 | 0.12 | 35.5 | 0.91 | 124 | - | 313 | 16.93 |
| 950 | gpu: tree64 + sphere + composite + raycast + pathtrace | 0.11 | 499.5 | 2.48 | 1.96 | 0.07 | 36.2 | 0.88 | 12 | - | 294 | 11.88 |
| 951 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.11 | 499.2 | 3.00 | 1.94 | 0.13 | 35.5 | 0.91 | 15 | - | 338 | 13.71 |
| 952 | gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.11 | 495.7 | 2.71 | 1.97 | 0.10 | 35.5 | 0.91 | 15 | - | 335 | 13.57 |
| 953 | gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.11 | 495.5 | 2.45 | 1.98 | 0.09 | 35.5 | 0.91 | 15 | - | 339 | 13.75 |
| 954 | gpu: svo_dag + rowrun + quadtree + raster + direct | 0.11 | 495.0 | 2.69 | 1.95 | 0.09 | 44.0 | 0.03 | 112 | - | 344 | 17.68 |
| 955 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.11 | 494.9 | 2.82 | 1.98 | 0.12 | 35.5 | 0.91 | 15 | - | 339 | 13.75 |
| 956 | gpu: tree64 + incremental + quadtree + raster + pathtrace | 0.11 | 490.9 | 2.64 | 1.97 | 0.11 | 35.5 | 0.91 | 120 | - | 312 | 16.74 |
| 957 | gpu: cubes@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.11 | 488.2 | 2.56 | 2.01 | 0.05 | 35.5 | 0.91 | 22 | - | 333 | 13.79 |
| 958 | gpu: naive@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.11 | 485.9 | 2.69 | 2.02 | 0.12 | 35.5 | 0.91 | 16 | - | 310 | 12.63 |
| 959 | gpu: clipmap + sphere + composite + raycast + pathtrace | 0.11 | 480.6 | 3.00 | 2.04 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.83 |
| 960 | gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 0.11 | 476.9 | 2.71 | 2.06 | 0.01 | 36.2 | 0.88 | 35 | - | 296 | 12.83 |
| 961 | gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.11 | 474.3 | 3.72 | 2.04 | 0.14 | 35.5 | 0.91 | 16 | - | 336 | 13.63 |
| 962 | gpu: naive@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.11 | 472.8 | 2.80 | 2.08 | 0.08 | 35.5 | 0.91 | 24 | - | 309 | 12.91 |
| 963 | gpu: greedy@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.11 | 471.1 | 2.63 | 2.08 | 0.11 | 35.5 | 0.91 | 24 | - | 334 | 13.89 |
| 964 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.11 | 465.8 | 2.64 | 2.11 | 0.08 | 35.5 | 0.91 | 14 | - | 338 | 13.64 |
| 965 | gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 0.10 | 465.3 | 3.14 | 2.07 | 0.12 | 35.5 | 0.91 | 124 | - | 312 | 16.88 |
| 966 | gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 0.10 | 460.5 | 3.40 | 2.09 | 0.13 | 35.5 | 0.91 | 122 | - | 311 | 16.79 |
| 967 | gpu: naive@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.10 | 459.8 | 2.68 | 2.14 | 0.07 | 35.5 | 0.91 | 23 | - | 309 | 12.87 |
| 968 | gpu: greedy@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.10 | 456.7 | 2.74 | 2.14 | 0.11 | 35.5 | 0.91 | 23 | - | 338 | 13.99 |
| 969 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.10 | 455.9 | 2.71 | 2.16 | 0.09 | 35.5 | 0.91 | 14 | - | 339 | 13.68 |
| 970 | gpu: cubes@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.10 | 454.6 | 2.72 | 2.16 | 0.05 | 35.5 | 0.91 | 23 | - | 334 | 13.83 |
| 971 | gpu: hier + incremental + quadtree + raster + pathtrace | 0.10 | 454.4 | 3.09 | 2.13 | 0.05 | 35.5 | 0.91 | 128 | - | 307 | 16.86 |
| 972 | gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 0.10 | 451.1 | 2.95 | 2.14 | 0.06 | 35.5 | 0.91 | 145 | - | 307 | 17.50 |
| 973 | gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 0.10 | 448.9 | 2.85 | 2.15 | 0.07 | 35.5 | 0.91 | 147 | - | 307 | 17.57 |
| 974 | gpu: prefix + incremental + quadtree + raster + pathtrace | 0.10 | 439.5 | 2.77 | 2.20 | 0.06 | 35.5 | 0.91 | 128 | - | 308 | 16.90 |
| 975 | gpu: rows + incremental + quadtree + raster + pathtrace | 0.10 | 439.4 | 2.95 | 2.21 | 0.05 | 35.5 | 0.91 | 128 | - | 307 | 16.88 |
| 976 | gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.10 | 431.2 | 2.87 | 2.28 | 0.10 | 35.5 | 0.91 | 15 | - | 336 | 13.57 |
| 977 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.10 | 430.8 | 3.19 | 2.29 | 0.09 | 35.5 | 0.91 | 14 | - | 336 | 13.57 |
| 978 | gpu: bricks + incremental + quadtree + raster + pathtrace | 0.10 | 427.9 | 2.94 | 2.26 | 0.06 | 35.5 | 0.91 | 128 | - | 309 | 16.93 |
| 979 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.10 | 423.8 | 3.23 | 2.32 | 0.11 | 35.5 | 0.91 | 15 | - | 336 | 13.64 |
| 980 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.09 | 421.1 | 3.05 | 2.34 | 0.09 | 35.5 | 0.91 | 15 | - | 337 | 13.65 |
| 981 | gpu: cubes@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.09 | 418.0 | 3.06 | 2.35 | 0.06 | 35.5 | 0.91 | 23 | - | 334 | 13.81 |
| 982 | gpu: clipmap + rowrun + quadtree + raster + pathtrace | 0.09 | 417.8 | 3.10 | 2.32 | 0.06 | 35.5 | 0.91 | 143 | - | 307 | 17.45 |
| 983 | gpu: tree64 + rowrun + quadtree + raster + direct | 0.09 | 416.1 | 3.38 | 2.33 | 0.12 | 44.0 | 0.03 | 112 | - | 348 | 17.83 |
| 984 | gpu: distance + incremental + quadtree + raster + pathtrace | 0.09 | 413.6 | 2.81 | 2.34 | 0.05 | 35.5 | 0.91 | 132 | - | 307 | 17.00 |
| 985 | gpu: cubes@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.09 | 413.3 | 2.86 | 2.38 | 0.05 | 35.5 | 0.91 | 22 | - | 333 | 13.79 |
| 986 | gpu: naive@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.09 | 411.7 | 3.03 | 2.39 | 0.08 | 35.5 | 0.91 | 38 | - | 342 | 14.75 |
| 987 | gpu: clipmap + incremental + quadtree + raster + pathtrace | 0.09 | 407.6 | 3.60 | 2.38 | 0.06 | 35.5 | 0.91 | 143 | - | 308 | 17.46 |
| 988 | gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.09 | 405.0 | 3.08 | 2.43 | 0.10 | 35.5 | 0.91 | 38 | - | 342 | 14.74 |
| 989 | gpu: hier + rowrun + quadtree + raster + pathtrace | 0.09 | 397.2 | 3.20 | 2.44 | 0.07 | 35.5 | 0.91 | 128 | - | 307 | 16.86 |
| 990 | gpu: bricks + rowrun + quadtree + raster + pathtrace | 0.08 | 376.3 | 3.55 | 2.57 | 0.06 | 35.5 | 0.91 | 128 | - | 307 | 16.83 |
| 991 | gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.08 | 374.3 | 3.23 | 2.63 | 0.06 | 35.5 | 0.91 | 37 | - | 341 | 14.67 |
| 992 | gpu: hier + sphere + quadtree + raster + pathtrace | 0.08 | 369.5 | 3.21 | 2.64 | 0.05 | 35.5 | 0.91 | 128 | - | 307 | 16.84 |
| 993 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.08 | 358.3 | 3.82 | 2.75 | 0.12 | 35.5 | 0.91 | 15 | - | 338 | 13.65 |
| 994 | gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.08 | 356.5 | 3.46 | 2.76 | 0.13 | 35.5 | 0.91 | 16 | - | 311 | 12.64 |
| 995 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.08 | 351.6 | 3.92 | 2.79 | 0.15 | 35.5 | 0.91 | 16 | - | 338 | 13.72 |
| 996 | gpu: prefix + sphere + quadtree + raster + pathtrace | 0.08 | 341.2 | 3.42 | 2.86 | 0.06 | 35.5 | 0.91 | 128 | - | 307 | 16.87 |
| 997 | gpu: svo_dag + sphere + quadtree + raster + pathtrace | 0.08 | 338.4 | 3.65 | 2.89 | 0.12 | 35.5 | 0.91 | 120 | - | 308 | 16.57 |
| 998 | gpu: svo_dag + incremental + quadtree + raster + pathtrace | 0.07 | 325.3 | 4.29 | 3.00 | 0.12 | 35.5 | 0.91 | 120 | - | 309 | 16.60 |
| 999 | gpu: rows + sphere + quadtree + raster + pathtrace | 0.07 | 322.7 | 3.91 | 3.03 | 0.08 | 35.5 | 0.91 | 128 | - | 307 | 16.87 |
| 1000 | gpu: bricks + sphere + quadtree + raster + pathtrace | 0.07 | 315.4 | 3.73 | 3.10 | 0.06 | 35.5 | 0.91 | 128 | - | 306 | 16.83 |
| 1001 | gpu: tree64 + sphere + quadtree + raster + pathtrace | 0.07 | 313.9 | 4.16 | 3.11 | 0.14 | 35.5 | 0.91 | 120 | - | 312 | 16.73 |
| 1002 | gpu: clipmap + sphere + quadtree + raster + pathtrace | 0.06 | 281.1 | 4.58 | 3.49 | 0.08 | 35.5 | 0.91 | 143 | - | 307 | 17.45 |
| 1003 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 275.6 | 7.47 | 0.35 | 3.42 | 44.0 | 0.03 | 16 | - | 396 | 15.98 |
| 1004 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 274.5 | 7.61 | 0.19 | 3.41 | 44.0 | 0.03 | 18 | - | 440 | 17.77 |
| 1005 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 274.1 | 7.91 | 0.19 | 3.43 | 44.0 | 0.03 | 18 | - | 398 | 16.14 |
| 1006 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + rowrun + direct | 0.06 | 274.0 | 6.13 | 1.65 | 3.43 | 44.0 | 0.03 | 8 | - | 429 | 16.93 |
| 1007 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + incremental + direct | 0.06 | 273.9 | 6.31 | 0.59 | 3.45 | 44.0 | 0.03 | 8 | - | 432 | 17.06 |
| 1008 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 273.8 | 7.52 | 0.19 | 3.42 | 44.0 | 0.03 | 20 | - | 440 | 17.82 |
| 1009 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + refloor + direct | 0.06 | 273.6 | 7.04 | 0.41 | 3.45 | 44.0 | 0.03 | 31 | - | 437 | 18.13 |
| 1010 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 273.0 | 7.74 | 0.19 | 3.43 | 44.0 | 0.03 | 18 | - | 439 | 17.71 |
| 1011 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 273.0 | 6.54 | 0.60 | 3.43 | 35.5 | 0.91 | 28 | - | 387 | 16.08 |
| 1012 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + incremental + direct | 0.06 | 273.0 | 6.78 | 0.55 | 3.44 | 44.0 | 0.03 | 17 | - | 429 | 17.27 |
| 1013 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 272.7 | 7.26 | 0.19 | 3.43 | 44.0 | 0.03 | 18 | - | 442 | 17.84 |
| 1014 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + rowrun + direct | 0.06 | 272.4 | 7.67 | 0.40 | 3.46 | 44.0 | 0.03 | 16 | - | 429 | 17.28 |
| 1015 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + rowrun + direct | 0.06 | 272.3 | 7.19 | 0.41 | 3.43 | 44.0 | 0.03 | 20 | - | 429 | 17.40 |
| 1016 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 272.2 | 6.04 | 1.13 | 3.46 | 35.5 | 0.91 | 24 | - | 392 | 16.11 |
| 1017 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 271.3 | 7.82 | 0.35 | 3.48 | 44.0 | 0.03 | 16 | - | 401 | 16.17 |
| 1018 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 271.2 | 7.17 | 0.19 | 3.47 | 44.0 | 0.03 | 20 | - | 400 | 16.27 |
| 1019 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 271.1 | 7.51 | 0.19 | 3.46 | 44.0 | 0.03 | 20 | - | 443 | 17.94 |
| 1020 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 271.0 | 7.20 | 0.35 | 3.48 | 44.0 | 0.03 | 16 | - | 414 | 16.66 |
| 1021 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + rowrun + direct | 0.06 | 271.0 | 6.65 | 0.43 | 3.47 | 44.0 | 0.03 | 17 | - | 431 | 17.34 |
| 1022 | gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 0.06 | 271.0 | 4.51 | 3.62 | 0.13 | 35.5 | 0.91 | 120 | - | 308 | 16.59 |
| 1023 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + sphere + direct | 0.06 | 270.9 | 7.26 | 0.25 | 3.45 | 44.0 | 0.03 | 20 | - | 427 | 17.32 |
| 1024 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 270.8 | 5.86 | 0.60 | 3.46 | 35.5 | 0.91 | 28 | - | 394 | 16.35 |
| 1025 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 270.6 | 7.39 | 0.35 | 3.50 | 44.0 | 0.03 | 16 | - | 411 | 16.56 |
| 1026 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + incremental + direct | 0.06 | 270.6 | 7.25 | 0.54 | 3.45 | 44.0 | 0.03 | 20 | - | 431 | 17.48 |
| 1027 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + incremental + direct | 0.06 | 270.5 | 6.65 | 0.94 | 3.50 | 44.0 | 0.03 | 8 | - | 427 | 16.87 |
| 1028 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + refloor + direct | 0.06 | 270.1 | 6.95 | 0.31 | 3.49 | 44.0 | 0.03 | 8 | - | 431 | 17.02 |
| 1029 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + rowrun + direct | 0.06 | 270.1 | 6.90 | 0.84 | 3.48 | 44.0 | 0.03 | 16 | - | 433 | 17.40 |
| 1030 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + incremental + direct | 0.06 | 270.0 | 5.69 | 0.64 | 3.48 | 44.0 | 0.03 | 31 | - | 438 | 18.17 |
| 1031 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + incremental + direct | 0.06 | 269.9 | 6.45 | 0.53 | 3.49 | 44.0 | 0.03 | 16 | - | 432 | 17.40 |
| 1032 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 269.9 | 8.03 | 0.36 | 3.51 | 44.0 | 0.03 | 16 | - | 405 | 16.33 |
| 1033 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + sphere + direct | 0.06 | 269.8 | 5.36 | 0.80 | 3.50 | 44.0 | 0.03 | 8 | - | 430 | 16.98 |
| 1034 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 269.6 | 6.32 | 1.13 | 3.49 | 35.5 | 0.91 | 24 | - | 390 | 16.04 |
| 1035 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 269.4 | 8.36 | 0.19 | 3.48 | 44.0 | 0.03 | 20 | - | 448 | 18.13 |
| 1036 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + rowrun + direct | 0.06 | 269.4 | 5.44 | 1.74 | 3.51 | 44.0 | 0.03 | 8 | - | 432 | 17.05 |
| 1037 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 269.1 | 6.21 | 0.60 | 3.48 | 35.5 | 0.91 | 28 | - | 395 | 16.40 |
| 1038 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 269.0 | 7.69 | 0.19 | 3.49 | 44.0 | 0.03 | 20 | - | 439 | 17.78 |
| 1039 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + incremental + direct | 0.06 | 269.0 | 7.24 | 0.53 | 3.50 | 44.0 | 0.03 | 16 | - | 429 | 17.24 |
| 1040 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 269.0 | 8.21 | 0.19 | 3.50 | 44.0 | 0.03 | 20 | - | 445 | 18.02 |
| 1041 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 268.7 | 6.10 | 0.57 | 3.50 | 35.5 | 0.91 | 26 | - | 392 | 16.20 |
| 1042 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + sphere + direct | 0.06 | 268.6 | 5.50 | 1.06 | 3.50 | 44.0 | 0.03 | 31 | - | 433 | 17.99 |
| 1043 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 268.6 | 7.88 | 0.19 | 3.49 | 44.0 | 0.03 | 18 | - | 443 | 17.87 |
| 1044 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + sphere + direct | 0.06 | 268.5 | 7.55 | 0.89 | 3.48 | 44.0 | 0.03 | 16 | - | 427 | 17.17 |
| 1045 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 268.4 | 5.82 | 1.13 | 3.48 | 35.5 | 0.91 | 24 | - | 401 | 16.46 |
| 1046 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + rowrun + direct | 0.06 | 268.4 | 6.20 | 0.69 | 3.51 | 44.0 | 0.03 | 31 | - | 432 | 17.95 |
| 1047 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 268.3 | 7.81 | 0.36 | 3.50 | 44.0 | 0.03 | 16 | - | 406 | 16.36 |
| 1048 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 268.2 | 5.98 | 0.60 | 3.49 | 35.5 | 0.91 | 28 | - | 402 | 16.65 |
| 1049 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 268.2 | 8.18 | 0.19 | 3.48 | 44.0 | 0.03 | 20 | - | 443 | 17.95 |
| 1050 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + refloor + direct | 0.06 | 268.1 | 6.89 | 0.34 | 3.55 | 44.0 | 0.03 | 8 | - | 432 | 17.07 |
| 1051 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 268.1 | 4.79 | 1.12 | 3.52 | 35.5 | 0.91 | 24 | - | 389 | 16.01 |
| 1052 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + sphere + direct | 0.06 | 268.0 | 5.73 | 0.77 | 3.50 | 44.0 | 0.03 | 16 | - | 426 | 17.15 |
| 1053 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + refloor + direct | 0.06 | 267.9 | 7.55 | 0.36 | 3.52 | 44.0 | 0.03 | 16 | - | 428 | 17.24 |
| 1054 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + sphere + direct | 0.06 | 267.8 | 6.18 | 0.95 | 3.51 | 44.0 | 0.03 | 8 | - | 434 | 17.13 |
| 1055 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.8 | 6.58 | 0.57 | 3.50 | 35.5 | 0.91 | 26 | - | 396 | 16.35 |
| 1056 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.8 | 6.47 | 0.57 | 3.49 | 35.5 | 0.91 | 26 | - | 397 | 16.38 |
| 1057 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 267.5 | 6.73 | 0.35 | 3.54 | 44.0 | 0.03 | 16 | - | 400 | 16.14 |
| 1058 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.5 | 6.75 | 0.60 | 3.48 | 35.5 | 0.91 | 28 | - | 396 | 16.44 |
| 1059 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + refloor + direct | 0.06 | 267.3 | 8.78 | 0.28 | 3.50 | 44.0 | 0.03 | 20 | - | 427 | 17.31 |
| 1060 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.3 | 5.48 | 0.60 | 3.51 | 35.5 | 0.91 | 28 | - | 389 | 16.16 |
| 1061 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.2 | 7.54 | 0.60 | 3.51 | 35.5 | 0.91 | 28 | - | 392 | 16.26 |
| 1062 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.2 | 6.37 | 1.13 | 3.53 | 35.5 | 0.91 | 24 | - | 396 | 16.29 |
| 1063 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + refloor + direct | 0.06 | 267.1 | 7.50 | 0.32 | 3.54 | 44.0 | 0.03 | 16 | - | 426 | 17.15 |
| 1064 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 267.1 | 7.39 | 0.60 | 3.48 | 35.5 | 0.91 | 28 | - | 391 | 16.26 |
| 1065 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 267.0 | 7.74 | 0.19 | 3.52 | 44.0 | 0.03 | 18 | - | 444 | 17.92 |
| 1066 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + refloor + direct | 0.06 | 266.9 | 7.70 | 0.30 | 3.53 | 44.0 | 0.03 | 17 | - | 434 | 17.47 |
| 1067 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 266.8 | 6.09 | 1.13 | 3.49 | 35.5 | 0.91 | 24 | - | 392 | 16.12 |
| 1068 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 266.1 | 5.08 | 1.13 | 3.53 | 35.5 | 0.91 | 24 | - | 390 | 16.03 |
| 1069 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 266.1 | 6.43 | 0.57 | 3.54 | 35.5 | 0.91 | 26 | - | 389 | 16.07 |
| 1070 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 266.0 | 7.41 | 0.57 | 3.51 | 35.5 | 0.91 | 26 | - | 394 | 16.29 |
| 1071 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 265.6 | 8.33 | 0.19 | 3.54 | 44.0 | 0.03 | 20 | - | 442 | 17.90 |
| 1072 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.06 | 265.4 | 7.36 | 0.35 | 3.54 | 44.0 | 0.03 | 16 | - | 404 | 16.29 |
| 1073 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 265.3 | 7.39 | 0.57 | 3.53 | 35.5 | 0.91 | 26 | - | 396 | 16.34 |
| 1074 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 265.1 | 6.48 | 0.57 | 3.54 | 35.5 | 0.91 | 26 | - | 392 | 16.22 |
| 1075 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.06 | 264.9 | 6.95 | 1.13 | 3.54 | 35.5 | 0.91 | 24 | - | 393 | 16.17 |
| 1076 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.06 | 264.8 | 7.14 | 0.57 | 3.54 | 35.5 | 0.91 | 26 | - | 393 | 16.25 |
| 1077 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 263.3 | 7.47 | 0.20 | 3.55 | 44.0 | 0.03 | 18 | - | 443 | 17.88 |
| 1078 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.06 | 263.3 | 8.33 | 0.19 | 3.55 | 44.0 | 0.03 | 18 | - | 443 | 17.85 |
| 1079 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + sphere + direct | 0.05 | 242.7 | 6.83 | 0.87 | 3.86 | 44.0 | 0.03 | 17 | - | 430 | 17.33 |
| 1080 | gpu: tree64 + rowrun + quadtree + raster + pathtrace | 0.05 | 235.9 | 6.42 | 4.16 | 0.16 | 35.5 | 0.91 | 120 | - | 312 | 16.74 |
| 1081 | cpu: greedy + faces + raster + hier + refloor + direct ◆ | 0.03 | 122.6 | 9.55 | - | 8.13 | 71.0 | 0.00 | - | - | 56 | 2.17 |
| 1082 | cpu: naive + faces + raster + hier + refloor + direct ◆ | 0.03 | 122.5 | 9.82 | - | 8.13 | 71.0 | 0.00 | - | - | 54 | 2.11 |
| 1083 | cpu: greedy + faces + raster + prefix + refloor + direct | 0.03 | 119.9 | 9.86 | - | 8.30 | 71.0 | 0.00 | - | - | 56 | 2.19 |
| 1084 | cpu: naive + faces + raster + prefix + refloor + direct | 0.03 | 119.4 | 9.49 | - | 8.34 | 71.0 | 0.00 | - | - | 57 | 2.20 |
| 1085 | cpu: naive + faces + raster + hier + incremental + direct | 0.03 | 118.1 | 10.16 | - | 8.44 | 71.0 | 0.00 | - | - | 57 | 2.20 |
| 1086 | cpu: greedy + faces + raster + hier + incremental + direct | 0.03 | 116.9 | 10.05 | - | 8.52 | 71.0 | 0.00 | - | - | 55 | 2.12 |
| 1087 | cpu: naive + faces + raster + prefix + incremental + direct | 0.03 | 114.8 | 10.16 | - | 8.67 | 71.0 | 0.00 | - | - | 57 | 2.20 |
| 1088 | cpu: naive + faces + raster + bricks + refloor + direct | 0.03 | 112.3 | 10.34 | - | 8.88 | 71.0 | 0.00 | - | - | 55 | 2.13 |
| 1089 | cpu: hier + refloor + raycast + direct ◆ | 0.03 | 112.1 | 12.78 | - | 8.89 | 59.6 | 0.00 | - | - | 31 | 1.20 |
| 1090 | cpu: greedy + faces + raster + bricks + refloor + direct | 0.03 | 111.2 | 10.88 | - | 8.96 | 71.0 | 0.00 | - | - | 56 | 2.17 |
| 1091 | cpu: greedy + faces + raster + bricks + incremental + direct | 0.02 | 110.8 | 10.69 | - | 9.00 | 71.0 | 0.00 | - | - | 56 | 2.19 |
| 1092 | cpu: naive + faces + raster + bricks + incremental + direct | 0.02 | 110.6 | 11.18 | - | 9.02 | 71.0 | 0.00 | - | - | 57 | 2.22 |
| 1093 | cpu: greedy + faces + raster + prefix + incremental + direct | 0.02 | 110.4 | 10.53 | - | 9.02 | 71.0 | 0.00 | - | - | 55 | 2.15 |
| 1094 | cpu: naive + faces + raster + brickmajor + incremental + direct | 0.02 | 108.9 | 11.21 | - | 9.14 | 71.0 | 0.00 | - | - | 61 | 2.36 |
| 1095 | cpu: greedy + faces + raster + brickmajor + incremental + direct | 0.02 | 108.5 | 11.27 | - | 9.18 | 71.0 | 0.00 | - | - | 61 | 2.38 |
| 1096 | cpu: naive + faces + raster + morton + refloor + direct | 0.02 | 108.3 | 10.76 | - | 9.19 | 71.0 | 0.00 | - | - | 61 | 2.36 |
| 1097 | cpu: greedy + faces + raster + morton + refloor + direct | 0.02 | 108.2 | 11.18 | - | 9.20 | 71.0 | 0.00 | - | - | 60 | 2.32 |
| 1098 | cpu: greedy + faces + raster + morton + incremental + direct | 0.02 | 108.1 | 11.32 | - | 9.20 | 71.0 | 0.00 | - | - | 60 | 2.34 |
| 1099 | cpu: naive + faces + raster + u8 + incremental + direct | 0.02 | 108.1 | 10.82 | - | 9.22 | 71.0 | 0.00 | - | - | 59 | 2.28 |
| 1100 | cpu: naive + faces + raster + morton + incremental + direct | 0.02 | 108.1 | 10.93 | - | 9.21 | 71.0 | 0.00 | - | - | 61 | 2.38 |
| 1101 | cpu: naive + faces + raster + brickmajor + refloor + direct | 0.02 | 108.1 | 11.02 | - | 9.21 | 71.0 | 0.00 | - | - | 62 | 2.41 |
| 1102 | cpu: naive + faces + raster + bits + incremental + direct | 0.02 | 108.0 | 11.27 | - | 9.23 | 71.0 | 0.00 | - | - | 55 | 2.15 |
| 1103 | cpu: greedy + faces + raster + brickmajor + refloor + direct | 0.02 | 107.9 | 10.97 | - | 9.23 | 71.0 | 0.00 | - | - | 60 | 2.33 |
| 1104 | cpu: naive + faces + raster + u8 + refloor + direct | 0.02 | 107.9 | 11.11 | - | 9.24 | 71.0 | 0.00 | - | - | 58 | 2.24 |
| 1105 | cpu: greedy + faces + raster + u8 + incremental + direct | 0.02 | 107.4 | 10.89 | - | 9.28 | 71.0 | 0.00 | - | - | 59 | 2.30 |
| 1106 | cpu: greedy + faces + raster + u8 + refloor + direct | 0.02 | 107.3 | 12.42 | - | 9.29 | 71.0 | 0.00 | - | - | 57 | 2.22 |
| 1107 | cpu: naive + faces + raster + palette + incremental + direct | 0.02 | 107.1 | 10.68 | - | 9.28 | 71.0 | 0.00 | - | - | 57 | 2.21 |
| 1108 | cpu: greedy + faces + raster + bits + incremental + direct | 0.02 | 106.7 | 11.11 | - | 9.34 | 71.0 | 0.00 | - | - | 55 | 2.13 |
| 1109 | cpu: greedy + faces + raster + palette + incremental + direct | 0.02 | 106.5 | 11.09 | - | 9.33 | 71.0 | 0.00 | - | - | 56 | 2.17 |
| 1110 | cpu: naive + faces + raster + bits + refloor + direct | 0.02 | 106.1 | 10.98 | - | 9.39 | 71.0 | 0.00 | - | - | 55 | 2.13 |
| 1111 | cpu: greedy + faces + raster + palette + refloor + direct | 0.02 | 105.9 | 11.54 | - | 9.39 | 71.0 | 0.00 | - | - | 56 | 2.18 |
| 1112 | cpu: naive + faces + raster + palette + refloor + direct | 0.02 | 105.5 | 11.32 | - | 9.43 | 71.0 | 0.00 | - | - | 56 | 2.17 |
| 1113 | cpu: greedy + faces + raster + bits + refloor + direct | 0.02 | 105.0 | 11.36 | - | 9.49 | 71.0 | 0.00 | - | - | 54 | 2.08 |
| 1114 | cpu: prefix + refloor + raycast + direct ◆ | 0.02 | 103.5 | 13.17 | - | 9.63 | 59.6 | 0.00 | - | - | 26 | 1.00 |
| 1115 | cpu: hier + incremental + raycast + direct | 0.02 | 95.7 | 13.67 | - | 10.42 | 59.3 | 0.00 | - | - | 31 | 1.20 |
| 1116 | cpu: bricks + refloor + raycast + direct | 0.02 | 92.5 | 15.10 | - | 10.79 | 59.6 | 0.00 | - | - | 30 | 1.18 |
| 1117 | cpu: morton + refloor + raycast + direct | 0.02 | 89.7 | 14.73 | - | 11.11 | 59.6 | 0.00 | - | - | 31 | 1.19 |
| 1118 | cpu: brickmajor + refloor + raycast + direct | 0.02 | 89.6 | 14.92 | - | 11.12 | 59.6 | 0.00 | - | - | 31 | 1.20 |
| 1119 | cpu: u8 + refloor + raycast + direct | 0.02 | 89.5 | 15.13 | - | 11.14 | 59.6 | 0.00 | - | - | 28 | 1.09 |
| 1120 | cpu: bits + refloor + raycast + direct | 0.02 | 88.3 | 16.01 | - | 11.30 | 59.6 | 0.00 | - | - | 31 | 1.20 |
| 1121 | cpu: palette + refloor + raycast + direct | 0.02 | 87.0 | 15.56 | - | 11.45 | 59.6 | 0.00 | - | - | 26 | 1.01 |
| 1122 | cpu: prefix + incremental + raycast + direct | 0.02 | 86.1 | 15.10 | - | 11.58 | 59.3 | 0.00 | - | - | 26 | 1.01 |
| 1123 | cpu: bricks + incremental + raycast + direct | 0.02 | 85.4 | 14.83 | - | 11.68 | 59.3 | 0.00 | - | - | 31 | 1.19 |
| 1124 | cpu: brickmajor + incremental + raycast + direct | 0.02 | 84.5 | 15.38 | - | 11.79 | 59.3 | 0.00 | - | - | 31 | 1.20 |
| 1125 | cpu: bits + incremental + raycast + direct | 0.02 | 83.8 | 16.07 | - | 11.90 | 59.3 | 0.00 | - | - | 31 | 1.21 |
| 1126 | cpu: morton + incremental + raycast + direct | 0.02 | 83.6 | 14.91 | - | 11.92 | 59.3 | 0.00 | - | - | 31 | 1.20 |
| 1127 | cpu: u8 + incremental + raycast + direct | 0.02 | 83.3 | 15.23 | - | 11.97 | 59.3 | 0.00 | - | - | 28 | 1.09 |
| 1128 | cpu: palette + incremental + raycast + direct | 0.02 | 80.1 | 16.55 | - | 12.43 | 59.3 | 0.00 | - | - | 26 | 1.02 |
| 1129 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 55.2 | 34.09 | 0.36 | 17.85 | 44.0 | 0.03 | 16 | - | 394 | 15.89 |
| 1130 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 55.2 | 21.66 | 0.60 | 17.86 | 35.5 | 0.91 | 28 | - | 375 | 15.62 |
| 1131 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 54.5 | 23.73 | 0.57 | 18.07 | 35.5 | 0.91 | 26 | - | 366 | 15.20 |
| 1132 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 54.5 | 21.78 | 1.15 | 18.07 | 35.5 | 0.91 | 24 | - | 372 | 15.34 |
| 1133 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 53.8 | 38.13 | 0.19 | 18.29 | 44.0 | 0.03 | 20 | - | 439 | 17.80 |
| 1134 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 53.4 | 39.18 | 0.19 | 18.45 | 44.0 | 0.03 | 18 | - | 410 | 16.59 |
| 1135 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.3 | 22.87 | 0.60 | 18.85 | 35.5 | 0.91 | 28 | - | 371 | 15.45 |
| 1136 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.2 | 22.94 | 1.15 | 18.87 | 35.5 | 0.91 | 24 | - | 360 | 14.89 |
| 1137 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 52.1 | 40.30 | 0.20 | 18.92 | 44.0 | 0.03 | 18 | - | 410 | 16.60 |
| 1138 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.1 | 23.92 | 0.57 | 18.91 | 35.5 | 0.91 | 26 | - | 377 | 15.63 |
| 1139 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.9 | 26.90 | 1.14 | 18.97 | 35.5 | 0.91 | 24 | - | 374 | 15.42 |
| 1140 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.9 | 24.50 | 1.14 | 18.97 | 35.5 | 0.91 | 24 | - | 383 | 15.79 |
| 1141 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.9 | 22.56 | 1.14 | 18.99 | 35.5 | 0.91 | 24 | - | 358 | 14.81 |
| 1142 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.8 | 40.92 | 0.19 | 18.97 | 44.0 | 0.03 | 20 | - | 428 | 17.36 |
| 1143 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.7 | 23.80 | 0.60 | 19.01 | 35.5 | 0.91 | 28 | - | 372 | 15.51 |
| 1144 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.7 | 23.14 | 0.57 | 19.03 | 35.5 | 0.91 | 26 | - | 383 | 15.84 |
| 1145 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 51.6 | 42.52 | 0.36 | 19.11 | 44.0 | 0.03 | 16 | - | 388 | 15.66 |
| 1146 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.6 | 22.79 | 0.60 | 19.08 | 35.5 | 0.91 | 28 | - | 370 | 15.41 |
| 1147 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.6 | 43.47 | 0.19 | 19.10 | 44.0 | 0.03 | 20 | - | 412 | 16.76 |
| 1148 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.5 | 38.89 | 0.19 | 19.12 | 44.0 | 0.03 | 18 | - | 436 | 17.62 |
| 1149 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.5 | 23.82 | 0.56 | 19.10 | 35.5 | 0.91 | 26 | - | 374 | 15.49 |
| 1150 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.5 | 43.03 | 0.19 | 19.12 | 44.0 | 0.03 | 18 | - | 437 | 17.65 |
| 1151 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 51.4 | 30.06 | 0.36 | 19.15 | 44.0 | 0.03 | 16 | - | 360 | 14.59 |
| 1152 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 51.4 | 40.72 | 0.36 | 19.19 | 44.0 | 0.03 | 16 | - | 389 | 15.68 |
| 1153 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.3 | 41.86 | 0.19 | 19.19 | 44.0 | 0.03 | 18 | - | 423 | 17.11 |
| 1154 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.3 | 40.53 | 0.19 | 19.21 | 44.0 | 0.03 | 20 | - | 418 | 16.98 |
| 1155 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 51.3 | 41.17 | 0.19 | 19.18 | 44.0 | 0.03 | 18 | - | 422 | 17.04 |
| 1156 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.3 | 23.51 | 0.57 | 19.16 | 35.5 | 0.91 | 26 | - | 366 | 15.18 |
| 1157 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.2 | 24.21 | 0.60 | 19.22 | 35.5 | 0.91 | 28 | - | 377 | 15.71 |
| 1158 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 51.1 | 34.06 | 0.36 | 19.28 | 44.0 | 0.03 | 16 | - | 385 | 15.53 |
| 1159 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.1 | 24.07 | 1.14 | 19.29 | 35.5 | 0.91 | 24 | - | 372 | 15.36 |
| 1160 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.9 | 25.38 | 0.60 | 19.34 | 35.5 | 0.91 | 28 | - | 366 | 15.26 |
| 1161 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 50.9 | 40.49 | 0.19 | 19.38 | 44.0 | 0.03 | 20 | - | 436 | 17.66 |
| 1162 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.9 | 23.54 | 0.60 | 19.33 | 35.5 | 0.91 | 28 | - | 376 | 15.64 |
| 1163 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 50.9 | 37.83 | 0.19 | 19.37 | 44.0 | 0.03 | 18 | - | 418 | 16.91 |
| 1164 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 50.8 | 42.79 | 0.36 | 19.39 | 44.0 | 0.03 | 16 | - | 394 | 15.88 |
| 1165 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 50.8 | 40.30 | 0.36 | 19.42 | 44.0 | 0.03 | 16 | - | 381 | 15.40 |
| 1166 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.8 | 22.91 | 0.57 | 19.37 | 35.5 | 0.91 | 26 | - | 370 | 15.35 |
| 1167 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.7 | 37.76 | 0.60 | 19.43 | 35.5 | 0.91 | 28 | - | 374 | 15.56 |
| 1168 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.7 | 23.45 | 1.15 | 19.45 | 35.5 | 0.91 | 24 | - | 380 | 15.67 |
| 1169 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.7 | 22.44 | 0.57 | 19.50 | 35.5 | 0.91 | 26 | - | 374 | 15.49 |
| 1170 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 50.7 | 40.34 | 0.19 | 19.41 | 44.0 | 0.03 | 20 | - | 437 | 17.70 |
| 1171 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 50.6 | 40.93 | 0.19 | 19.43 | 44.0 | 0.03 | 20 | - | 420 | 17.07 |
| 1172 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.4 | 24.67 | 0.57 | 19.56 | 35.5 | 0.91 | 26 | - | 375 | 15.53 |
| 1173 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.3 | 24.15 | 1.15 | 19.57 | 35.5 | 0.91 | 24 | - | 379 | 15.63 |
| 1174 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 50.2 | 42.99 | 0.19 | 19.62 | 44.0 | 0.03 | 20 | - | 422 | 17.11 |
| 1175 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 49.9 | 42.32 | 0.19 | 19.73 | 44.0 | 0.03 | 18 | - | 442 | 17.84 |
| 1176 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 49.7 | 43.78 | 0.36 | 19.81 | 44.0 | 0.03 | 16 | - | 389 | 15.71 |
| 1177 | gpu: heightmap + columns + heightfield + direct (incorrect) | 0.06 | 267.0 | 6.67 | 3.66 | 0.02 | 10.9 | 53.57 | 3 | - | 298 | 11.66 |
| 1178 | cpu: heightmap + columns + heightfield + direct (incorrect) | 0.05 | 238.0 | 7.10 | - | 4.16 | 10.9 | 53.57 | - | - | 32 | 1.23 |
| 1179 | gpu: heightmap + columns + heightfield + pathtrace (incorrect) | 0.02 | 108.0 | 12.31 | 9.18 | 0.05 | 10.7 | 53.91 | 11 | - | 309 | 12.42 |

### showcase [unlit]

960×540. camera 1.36 blocks/frame (max 1.4), turn 2.72°/frame; 84 chunks at start, 0.5 loads/frame; chunk fill 12%, solid 0.3%, surface 0.35%; sky 28%, hits at 38 blocks (median 29); 0.0 edits/frame; 2 lights; 0 mesh triangles.

| # | renderer | rel | fps | frame p99 ms | gpu ms | cpu ms | psnr | bad pct | gpu mb | gpu device mb | peak rss mb | mem rel |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace ◆ | 1.00 | 6842.2 | 1.17 | 0.03 | 0.04 | 44.1 | 0.03 | 23 | - | 309 | 12.75 |
| 2 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.88 | 6035.5 | 1.07 | 0.04 | 0.05 | 44.1 | 0.03 | 25 | - | 309 | 12.82 |
| 3 | gpu: prefix + refloor + composite + raycast + pathtrace ◆ | 0.86 | 5894.5 | 0.49 | 0.11 | 0.01 | 60.9 | 0.00 | 21 | - | 304 | 12.46 |
| 4 | gpu: distance + sphere + composite + raycast + pathtrace | 0.85 | 5849.2 | 0.91 | 0.08 | 0.01 | 60.9 | 0.00 | 24 | - | 304 | 12.60 |
| 5 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.84 | 5770.9 | 1.80 | 0.03 | 0.08 | 44.1 | 0.03 | 23 | - | 336 | 13.79 |
| 6 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.84 | 5732.1 | 1.27 | 0.04 | 0.05 | 44.1 | 0.03 | 27 | - | 308 | 12.88 |
| 7 | gpu: distance + sphere + beam + composite + raycast + pathtrace | 0.83 | 5712.9 | 0.92 | 0.09 | 0.01 | 60.9 | 0.00 | 24 | - | 304 | 12.59 |
| 8 | gpu: hier + refloor + composite + raycast + pathtrace ◆ | 0.82 | 5605.3 | 0.82 | 0.10 | 0.01 | 60.9 | 0.00 | 20 | - | 304 | 12.44 |
| 9 | gpu: distance + refloor + composite + raycast + pathtrace | 0.80 | 5485.6 | 0.97 | 0.10 | 0.01 | 60.9 | 0.00 | 24 | - | 304 | 12.59 |
| 10 | gpu: svo_dag + refloor + composite + raycast + pathtrace ◆ | 0.79 | 5400.6 | 0.90 | 0.11 | 0.04 | 60.9 | 0.00 | 12 | - | 306 | 12.19 |
| 11 | gpu: prefix + refloor + beam + composite + raycast + pathtrace | 0.79 | 5395.6 | 0.49 | 0.12 | 0.01 | 60.9 | 0.00 | 21 | - | 305 | 12.49 |
| 12 | gpu: hier + refloor + beam + composite + raycast + pathtrace | 0.78 | 5334.2 | 0.70 | 0.11 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 13 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.77 | 5239.1 | 0.82 | 0.11 | 0.03 | 44.1 | 0.03 | 22 | - | 335 | 13.72 |
| 14 | gpu: prefix + rowrun + composite + raycast + pathtrace | 0.76 | 5233.3 | 0.51 | 0.13 | 0.01 | 60.9 | 0.00 | 21 | - | 305 | 12.50 |
| 15 | gpu: distance + refloor + beam + composite + raycast + pathtrace | 0.74 | 5072.4 | 1.10 | 0.11 | 0.01 | 60.9 | 0.00 | 24 | - | 305 | 12.60 |
| 16 | gpu: distance + rowrun + composite + raycast + pathtrace | 0.74 | 5038.4 | 0.77 | 0.12 | 0.01 | 60.9 | 0.00 | 24 | - | 304 | 12.58 |
| 17 | gpu: hier + rowrun + composite + raycast + pathtrace | 0.74 | 5036.2 | 0.56 | 0.14 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.46 |
| 18 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.73 | 4970.1 | 1.80 | 0.04 | 0.08 | 44.1 | 0.03 | 27 | - | 356 | 14.72 |
| 19 | gpu: rows + refloor + composite + raycast + pathtrace | 0.73 | 4968.6 | 0.67 | 0.14 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 20 | gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 0.72 | 4946.2 | 0.55 | 0.14 | 0.01 | 60.9 | 0.00 | 21 | - | 305 | 12.50 |
| 21 | gpu: bricks + refloor + beam + composite + raycast + pathtrace | 0.72 | 4906.7 | 0.56 | 0.15 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.45 |
| 22 | gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 0.71 | 4841.2 | 0.93 | 0.13 | 0.04 | 60.9 | 0.00 | 12 | - | 306 | 12.21 |
| 23 | gpu: hier + rowrun + beam + composite + raycast + pathtrace | 0.70 | 4779.3 | 0.51 | 0.15 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 24 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.69 | 4745.1 | 0.74 | 0.12 | 0.03 | 44.1 | 0.03 | 24 | - | 349 | 14.34 |
| 25 | gpu: distance + rowrun + beam + composite + raycast + pathtrace | 0.69 | 4702.2 | 1.05 | 0.13 | 0.01 | 60.9 | 0.00 | 24 | - | 305 | 12.62 |
| 26 | gpu: rows + rowrun + composite + raycast + pathtrace | 0.68 | 4672.2 | 0.52 | 0.16 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 27 | gpu: bricks + rowrun + composite + raycast + pathtrace | 0.67 | 4596.8 | 0.57 | 0.17 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.46 |
| 28 | gpu: rows + refloor + beam + composite + raycast + pathtrace | 0.67 | 4592.1 | 0.52 | 0.15 | 0.01 | 60.9 | 0.00 | 20 | - | 304 | 12.46 |
| 29 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.67 | 4590.6 | 0.83 | 0.13 | 0.03 | 44.1 | 0.03 | 26 | - | 355 | 14.63 |
| 30 | gpu: bricks + refloor + composite + raycast + pathtrace | 0.67 | 4577.2 | 0.83 | 0.15 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 31 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.67 | 4568.2 | 2.07 | 0.04 | 0.10 | 44.1 | 0.03 | 25 | - | 351 | 14.45 |
| 32 | gpu: svo_dag + rowrun + composite + raycast + pathtrace | 0.65 | 4479.5 | 0.83 | 0.16 | 0.03 | 60.9 | 0.00 | 12 | - | 306 | 12.19 |
| 33 | gpu: rows + rowrun + beam + composite + raycast + pathtrace | 0.65 | 4426.4 | 0.55 | 0.17 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 34 | gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 0.63 | 4312.2 | 0.56 | 0.18 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.48 |
| 35 | gpu: tree64 + refloor + composite + raycast + pathtrace | 0.62 | 4266.1 | 1.22 | 0.14 | 0.05 | 60.9 | 0.00 | 12 | - | 307 | 12.23 |
| 36 | gpu: hier + incremental + composite + raycast + pathtrace | 0.62 | 4210.2 | 0.65 | 0.18 | 0.01 | 60.6 | 0.00 | 20 | - | 305 | 12.46 |
| 37 | gpu: clipmap + refloor + composite + raycast + pathtrace | 0.61 | 4153.5 | 0.58 | 0.19 | 0.01 | 60.9 | 0.00 | 35 | - | 309 | 13.18 |
| 38 | gpu: bricks + incremental + beam + composite + raycast + pathtrace | 0.59 | 4045.1 | 0.58 | 0.20 | 0.01 | 60.6 | 0.00 | 20 | - | 305 | 12.47 |
| 39 | gpu: hier + incremental + beam + composite + raycast + pathtrace | 0.59 | 4016.2 | 0.59 | 0.19 | 0.01 | 60.6 | 0.00 | 20 | - | 305 | 12.47 |
| 40 | gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 0.59 | 4014.4 | 0.81 | 0.17 | 0.04 | 60.9 | 0.00 | 12 | - | 306 | 12.20 |
| 41 | gpu: bricks + incremental + composite + raycast + pathtrace | 0.57 | 3924.6 | 0.62 | 0.21 | 0.01 | 60.6 | 0.00 | 20 | - | 304 | 12.44 |
| 42 | gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 0.57 | 3924.5 | 0.52 | 0.20 | 0.01 | 60.9 | 0.00 | 35 | - | 309 | 13.19 |
| 43 | gpu: rows + incremental + composite + raycast + pathtrace | 0.57 | 3883.7 | 0.66 | 0.21 | 0.01 | 60.6 | 0.00 | 20 | - | 305 | 12.48 |
| 44 | gpu: rows + incremental + beam + composite + raycast + pathtrace | 0.56 | 3846.0 | 0.55 | 0.21 | 0.01 | 60.6 | 0.00 | 20 | - | 305 | 12.47 |
| 45 | gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 0.55 | 3784.4 | 1.21 | 0.16 | 0.06 | 60.9 | 0.00 | 12 | - | 307 | 12.23 |
| 46 | gpu: prefix + incremental + composite + raycast + pathtrace | 0.55 | 3751.7 | 0.62 | 0.21 | 0.01 | 60.6 | 0.00 | 21 | - | 305 | 12.47 |
| 47 | gpu: clipmap + rowrun + composite + raycast + pathtrace | 0.53 | 3657.6 | 0.61 | 0.23 | 0.01 | 60.9 | 0.00 | 35 | - | 309 | 13.19 |
| 48 | gpu: distance + incremental + composite + raycast + pathtrace | 0.53 | 3650.6 | 0.78 | 0.20 | 0.01 | 60.6 | 0.00 | 24 | - | 304 | 12.60 |
| 49 | gpu: prefix + incremental + beam + composite + raycast + pathtrace | 0.53 | 3620.5 | 0.61 | 0.23 | 0.01 | 60.6 | 0.00 | 21 | - | 305 | 12.48 |
| 50 | gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 0.51 | 3522.8 | 0.63 | 0.24 | 0.01 | 60.9 | 0.00 | 35 | - | 309 | 13.21 |
| 51 | gpu: distance + incremental + beam + composite + raycast + pathtrace | 0.50 | 3425.1 | 0.85 | 0.22 | 0.01 | 60.6 | 0.00 | 24 | - | 305 | 12.62 |
| 52 | gpu: clipmap + incremental + composite + raycast + pathtrace | 0.49 | 3380.0 | 0.74 | 0.25 | 0.01 | 60.6 | 0.00 | 35 | - | 309 | 13.19 |
| 53 | gpu: tree64 + rowrun + composite + raycast + pathtrace | 0.49 | 3373.6 | 1.21 | 0.21 | 0.05 | 60.9 | 0.00 | 12 | - | 307 | 12.24 |
| 54 | gpu: points@cpu + splat + bricks + refloor + pathtrace | 0.47 | 3232.8 | 0.84 | 0.24 | 0.01 | 61.1 | 0.00 | 24 | - | 347 | 14.24 |
| 55 | gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 0.47 | 3197.3 | 1.32 | 0.22 | 0.06 | 60.9 | 0.00 | 12 | - | 308 | 12.27 |
| 56 | gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 0.46 | 3116.4 | 0.64 | 0.27 | 0.01 | 60.6 | 0.00 | 35 | - | 309 | 13.21 |
| 57 | gpu: tree64 + incremental + composite + raycast + pathtrace | 0.45 | 3076.9 | 1.07 | 0.25 | 0.05 | 60.6 | 0.00 | 12 | - | 307 | 12.24 |
| 58 | gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 0.44 | 3021.7 | 1.25 | 0.24 | 0.05 | 60.6 | 0.00 | 12 | - | 307 | 12.25 |
| 59 | gpu: boxes@cpu + rayquery + raycast + pathtrace | 0.43 | 2971.0 | 0.80 | 0.28 | 0.08 | 60.9 | 0.00 | 20 | - | 401 | 16.17 |
| 60 | gpu: naive@cpu + rayquery + raycast + pathtrace | 0.43 | 2935.6 | 2.32 | 0.19 | 0.15 | 60.3 | 0.00 | 24 | - | 405 | 16.49 |
| 61 | gpu: greedy@cpu + rayquery + raycast + pathtrace | 0.39 | 2684.8 | 2.84 | 0.19 | 0.18 | 60.3 | 0.00 | 24 | - | 413 | 16.79 |
| 62 | gpu: hier + sphere + composite + raycast + pathtrace | 0.39 | 2635.5 | 0.76 | 0.34 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.45 |
| 63 | gpu: hier + sphere + beam + composite + raycast + pathtrace | 0.37 | 2526.1 | 0.72 | 0.35 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.46 |
| 64 | gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 0.36 | 2480.3 | 0.87 | 0.35 | 0.03 | 60.6 | 0.00 | 12 | - | 307 | 12.22 |
| 65 | gpu: svo_dag + incremental + composite + raycast + pathtrace | 0.35 | 2408.4 | 0.89 | 0.36 | 0.04 | 60.6 | 0.00 | 12 | - | 306 | 12.20 |
| 66 | gpu: prefix + sphere + composite + raycast + pathtrace | 0.33 | 2270.5 | 0.90 | 0.40 | 0.01 | 60.9 | 0.00 | 21 | - | 304 | 12.46 |
| 67 | gpu: svo_dag + sphere + composite + raycast + pathtrace | 0.33 | 2262.8 | 0.90 | 0.39 | 0.04 | 60.9 | 0.00 | 12 | - | 306 | 12.20 |
| 68 | gpu: prefix + sphere + beam + composite + raycast + pathtrace | 0.32 | 2185.2 | 0.87 | 0.41 | 0.01 | 60.9 | 0.00 | 21 | - | 305 | 12.48 |
| 69 | gpu: svo_dag + sphere + beam + composite + raycast + pathtrace ◆ | 0.32 | 2176.4 | 0.90 | 0.40 | 0.04 | 60.9 | 0.00 | 12 | - | 306 | 12.19 |
| 70 | gpu: rows + sphere + composite + raycast + pathtrace | 0.32 | 2167.6 | 0.78 | 0.42 | 0.01 | 60.9 | 0.00 | 20 | - | 304 | 12.45 |
| 71 | gpu: bricks + sphere + beam + composite + raycast + pathtrace | 0.31 | 2122.2 | 0.90 | 0.43 | 0.02 | 60.9 | 0.00 | 20 | - | 305 | 12.45 |
| 72 | gpu: rows + sphere + beam + composite + raycast + pathtrace | 0.31 | 2111.8 | 0.79 | 0.43 | 0.01 | 60.9 | 0.00 | 20 | - | 305 | 12.47 |
| 73 | gpu: bricks + refloor + quadtree + raster + pathtrace | 0.30 | 2085.2 | 0.92 | 0.40 | 0.04 | 44.1 | 0.03 | 128 | - | 349 | 18.28 |
| 74 | gpu: hier + refloor + quadtree + raster + pathtrace | 0.30 | 2085.2 | 1.03 | 0.39 | 0.03 | 44.1 | 0.03 | 128 | - | 397 | 20.12 |
| 75 | gpu: rows + refloor + quadtree + raster + pathtrace | 0.30 | 2068.6 | 0.97 | 0.40 | 0.03 | 44.1 | 0.03 | 128 | - | 397 | 20.14 |
| 76 | gpu: prefix + refloor + quadtree + raster + pathtrace | 0.30 | 2066.5 | 1.03 | 0.39 | 0.04 | 44.1 | 0.03 | 128 | - | 398 | 20.21 |
| 77 | gpu: distance + sphere + quadtree + raster + pathtrace | 0.30 | 2038.6 | 1.10 | 0.38 | 0.04 | 44.1 | 0.03 | 132 | - | 398 | 20.33 |
| 78 | gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 0.30 | 2036.2 | 0.81 | 0.42 | 0.03 | 44.1 | 0.03 | 130 | - | 354 | 18.57 |
| 79 | gpu: rows + refloor + quadtree + visbuffer + pathtrace | 0.30 | 2027.4 | 0.83 | 0.42 | 0.03 | 44.1 | 0.03 | 130 | - | 354 | 18.57 |
| 80 | gpu: hier + refloor + quadtree + visbuffer + pathtrace | 0.30 | 2026.2 | 1.04 | 0.41 | 0.04 | 44.1 | 0.03 | 130 | - | 358 | 18.69 |
| 81 | gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 0.30 | 2019.3 | 0.95 | 0.42 | 0.04 | 44.1 | 0.03 | 132 | - | 357 | 18.73 |
| 82 | gpu: rows + rowrun + quadtree + raster + pathtrace | 0.30 | 2019.1 | 1.10 | 0.41 | 0.03 | 44.1 | 0.03 | 128 | - | 349 | 18.30 |
| 83 | gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 0.29 | 2011.8 | 0.88 | 0.41 | 0.04 | 44.1 | 0.03 | 132 | - | 359 | 18.86 |
| 84 | gpu: distance + sphere + quadtree + gbuffer + pathtrace | 0.29 | 2006.8 | 1.04 | 0.40 | 0.03 | 44.1 | 0.03 | 136 | - | 357 | 18.89 |
| 85 | gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 0.29 | 1994.3 | 1.02 | 0.41 | 0.04 | 44.1 | 0.03 | 130 | - | 359 | 18.76 |
| 86 | gpu: distance + refloor + quadtree + raster + pathtrace | 0.29 | 1992.5 | 1.42 | 0.39 | 0.04 | 44.1 | 0.03 | 132 | - | 398 | 20.34 |
| 87 | gpu: distance + sphere + quadtree + visbuffer + pathtrace | 0.29 | 1986.9 | 1.15 | 0.40 | 0.04 | 44.1 | 0.03 | 134 | - | 355 | 18.73 |
| 88 | gpu: distance + refloor + quadtree + gbuffer + pathtrace | 0.29 | 1986.4 | 0.95 | 0.41 | 0.03 | 44.1 | 0.03 | 136 | - | 355 | 18.84 |
| 89 | gpu: bricks + sphere + composite + raycast + pathtrace | 0.29 | 1986.4 | 0.91 | 0.46 | 0.01 | 60.9 | 0.00 | 20 | - | 304 | 12.43 |
| 90 | gpu: prefix + rowrun + quadtree + raster + pathtrace | 0.29 | 1984.8 | 1.13 | 0.42 | 0.04 | 44.1 | 0.03 | 128 | - | 350 | 18.34 |
| 91 | gpu: rows + refloor + quadtree + gbuffer + pathtrace | 0.29 | 1977.4 | 1.03 | 0.42 | 0.04 | 44.1 | 0.03 | 132 | - | 357 | 18.78 |
| 92 | gpu: hier + refloor + quadtree + gbuffer + pathtrace | 0.29 | 1966.9 | 1.11 | 0.41 | 0.05 | 44.1 | 0.03 | 132 | - | 360 | 18.85 |
| 93 | gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 0.29 | 1965.7 | 0.80 | 0.44 | 0.03 | 44.1 | 0.03 | 132 | - | 355 | 18.70 |
| 94 | gpu: distance + refloor + quadtree + visbuffer + pathtrace | 0.29 | 1959.6 | 1.05 | 0.41 | 0.04 | 44.1 | 0.03 | 134 | - | 359 | 18.89 |
| 95 | gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 0.28 | 1949.9 | 0.97 | 0.44 | 0.03 | 44.1 | 0.03 | 132 | - | 360 | 18.88 |
| 96 | gpu: rows + incremental + quadtree + raster + pathtrace | 0.28 | 1942.3 | 1.05 | 0.43 | 0.03 | 44.1 | 0.03 | 128 | - | 398 | 20.20 |
| 97 | gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 0.28 | 1936.9 | 0.96 | 0.43 | 0.04 | 44.1 | 0.03 | 130 | - | 358 | 18.71 |
| 98 | gpu: distance + rowrun + quadtree + raster + pathtrace | 0.28 | 1931.5 | 1.16 | 0.41 | 0.04 | 44.1 | 0.03 | 132 | - | 348 | 18.40 |
| 99 | gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 0.28 | 1921.2 | 1.03 | 0.43 | 0.03 | 44.1 | 0.03 | 136 | - | 358 | 18.93 |
| 100 | gpu: rows + incremental + quadtree + visbuffer + pathtrace | 0.28 | 1910.9 | 0.96 | 0.45 | 0.03 | 44.1 | 0.03 | 130 | - | 360 | 18.81 |
| 101 | gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 0.28 | 1904.1 | 1.08 | 0.44 | 0.04 | 44.1 | 0.03 | 130 | - | 361 | 18.85 |
| 102 | gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 0.28 | 1894.7 | 1.06 | 0.43 | 0.03 | 44.1 | 0.03 | 134 | - | 359 | 18.92 |
| 103 | gpu: clipmap + sphere + composite + raycast + pathtrace | 0.28 | 1894.6 | 0.90 | 0.49 | 0.01 | 60.9 | 0.00 | 35 | - | 309 | 13.18 |
| 104 | gpu: bricks + rowrun + quadtree + raster + pathtrace | 0.28 | 1894.4 | 0.86 | 0.45 | 0.03 | 44.1 | 0.03 | 128 | - | 348 | 18.25 |
| 105 | gpu: bricks + incremental + quadtree + raster + pathtrace | 0.28 | 1892.3 | 1.10 | 0.44 | 0.04 | 44.1 | 0.03 | 128 | - | 397 | 20.13 |
| 106 | gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 0.27 | 1875.6 | 0.83 | 0.46 | 0.03 | 44.1 | 0.03 | 130 | - | 361 | 18.82 |
| 107 | gpu: hier + incremental + quadtree + raster + pathtrace | 0.27 | 1859.0 | 1.06 | 0.44 | 0.04 | 44.1 | 0.03 | 128 | - | 398 | 20.19 |
| 108 | gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 0.27 | 1851.8 | 1.09 | 0.46 | 0.05 | 44.1 | 0.03 | 132 | - | 360 | 18.84 |
| 109 | gpu: hier + rowrun + quadtree + raster + pathtrace | 0.27 | 1851.5 | 0.88 | 0.46 | 0.03 | 44.1 | 0.03 | 128 | - | 347 | 18.21 |
| 110 | gpu: rows + incremental + quadtree + gbuffer + pathtrace | 0.27 | 1840.2 | 1.11 | 0.45 | 0.04 | 44.1 | 0.03 | 132 | - | 359 | 18.86 |
| 111 | gpu: prefix + incremental + quadtree + raster + pathtrace | 0.27 | 1836.5 | 1.16 | 0.45 | 0.04 | 44.1 | 0.03 | 128 | - | 399 | 20.21 |
| 112 | gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 0.27 | 1829.5 | 0.85 | 0.47 | 0.03 | 44.1 | 0.03 | 130 | - | 364 | 18.93 |
| 113 | gpu: hier + incremental + quadtree + gbuffer + pathtrace | 0.27 | 1828.5 | 1.11 | 0.46 | 0.04 | 44.1 | 0.03 | 132 | - | 359 | 18.81 |
| 114 | gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 0.27 | 1822.3 | 0.94 | 0.51 | 0.01 | 60.9 | 0.00 | 35 | - | 309 | 13.18 |
| 115 | gpu: hier + incremental + quadtree + visbuffer + pathtrace | 0.27 | 1819.9 | 1.03 | 0.46 | 0.04 | 44.1 | 0.03 | 130 | - | 359 | 18.76 |
| 116 | gpu: distance + incremental + quadtree + raster + pathtrace | 0.27 | 1816.9 | 1.15 | 0.46 | 0.03 | 44.1 | 0.03 | 132 | - | 397 | 20.29 |
| 117 | gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 0.26 | 1812.5 | 1.11 | 0.48 | 0.03 | 44.1 | 0.03 | 132 | - | 363 | 18.96 |
| 118 | gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 0.26 | 1811.0 | 1.10 | 0.47 | 0.03 | 44.1 | 0.03 | 130 | - | 361 | 18.85 |
| 119 | gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 0.26 | 1801.9 | 0.95 | 0.47 | 0.03 | 44.1 | 0.03 | 132 | - | 362 | 18.96 |
| 120 | gpu: svo_dag + refloor + quadtree + raster + pathtrace | 0.26 | 1788.7 | 1.21 | 0.45 | 0.06 | 44.1 | 0.03 | 120 | - | 400 | 19.95 |
| 121 | gpu: distance + incremental + quadtree + visbuffer + pathtrace | 0.26 | 1785.3 | 1.09 | 0.48 | 0.03 | 44.1 | 0.03 | 134 | - | 359 | 18.91 |
| 122 | gpu: hier + sphere + quadtree + raster + pathtrace | 0.26 | 1784.9 | 1.16 | 0.48 | 0.03 | 44.1 | 0.03 | 128 | - | 398 | 20.17 |
| 123 | gpu: bricks + sphere + quadtree + raster + pathtrace | 0.26 | 1781.2 | 0.89 | 0.49 | 0.03 | 44.1 | 0.03 | 128 | - | 397 | 20.12 |
| 124 | gpu: clipmap + refloor + quadtree + raster + pathtrace | 0.26 | 1769.9 | 1.17 | 0.48 | 0.03 | 44.1 | 0.03 | 143 | - | 400 | 20.81 |
| 125 | gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 0.26 | 1766.2 | 1.11 | 0.49 | 0.03 | 44.1 | 0.03 | 130 | - | 359 | 18.74 |
| 126 | gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 0.26 | 1759.1 | 0.87 | 0.50 | 0.03 | 44.1 | 0.03 | 145 | - | 354 | 19.14 |
| 127 | gpu: distance + incremental + quadtree + gbuffer + pathtrace | 0.26 | 1753.1 | 1.18 | 0.48 | 0.04 | 44.1 | 0.03 | 136 | - | 359 | 18.97 |
| 128 | gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 0.26 | 1752.4 | 1.15 | 0.48 | 0.06 | 44.1 | 0.03 | 122 | - | 358 | 18.42 |
| 129 | gpu: tree64 + refloor + quadtree + raster + pathtrace | 0.26 | 1750.6 | 1.98 | 0.44 | 0.09 | 44.1 | 0.03 | 120 | - | 406 | 20.18 |
| 130 | gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 0.25 | 1735.9 | 1.26 | 0.48 | 0.06 | 44.1 | 0.03 | 124 | - | 358 | 18.48 |
| 131 | gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 0.25 | 1732.8 | 1.12 | 0.49 | 0.04 | 44.1 | 0.03 | 132 | - | 358 | 18.77 |
| 132 | gpu: rows + sphere + quadtree + raster + pathtrace | 0.25 | 1732.5 | 1.10 | 0.49 | 0.04 | 44.1 | 0.03 | 128 | - | 397 | 20.14 |
| 133 | gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 0.25 | 1726.7 | 1.77 | 0.46 | 0.09 | 44.1 | 0.03 | 122 | - | 360 | 18.48 |
| 134 | gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 0.25 | 1725.1 | 1.65 | 0.46 | 0.08 | 44.1 | 0.03 | 124 | - | 360 | 18.56 |
| 135 | gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 0.25 | 1724.1 | 1.03 | 0.51 | 0.04 | 44.1 | 0.03 | 132 | - | 357 | 18.74 |
| 136 | gpu: hier + sphere + quadtree + gbuffer + pathtrace | 0.25 | 1722.8 | 1.13 | 0.50 | 0.04 | 44.1 | 0.03 | 132 | - | 355 | 18.66 |
| 137 | gpu: clipmap + rowrun + quadtree + raster + pathtrace | 0.25 | 1719.1 | 1.00 | 0.51 | 0.03 | 44.1 | 0.03 | 143 | - | 346 | 18.74 |
| 138 | gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 0.25 | 1718.2 | 0.91 | 0.51 | 0.03 | 44.1 | 0.03 | 130 | - | 355 | 18.59 |
| 139 | gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 0.25 | 1706.5 | 1.17 | 0.50 | 0.04 | 44.1 | 0.03 | 147 | - | 356 | 19.31 |
| 140 | gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 0.25 | 1705.5 | 1.02 | 0.52 | 0.03 | 44.1 | 0.03 | 132 | - | 355 | 18.69 |
| 141 | gpu: hier + sphere + quadtree + visbuffer + pathtrace | 0.25 | 1704.7 | 1.30 | 0.50 | 0.04 | 44.1 | 0.03 | 130 | - | 355 | 18.58 |
| 142 | gpu: prefix + sphere + quadtree + raster + pathtrace | 0.25 | 1703.4 | 1.24 | 0.50 | 0.04 | 44.1 | 0.03 | 128 | - | 397 | 20.14 |
| 143 | gpu: clipmap + incremental + quadtree + raster + pathtrace | 0.25 | 1678.7 | 0.89 | 0.53 | 0.03 | 44.1 | 0.03 | 143 | - | 400 | 20.82 |
| 144 | gpu: rows + sphere + quadtree + gbuffer + pathtrace | 0.24 | 1675.3 | 1.24 | 0.52 | 0.04 | 44.1 | 0.03 | 132 | - | 355 | 18.69 |
| 145 | gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 0.24 | 1673.0 | 0.95 | 0.51 | 0.03 | 44.1 | 0.03 | 130 | - | 356 | 18.66 |
| 146 | gpu: rows + sphere + quadtree + visbuffer + pathtrace | 0.24 | 1650.3 | 1.25 | 0.51 | 0.04 | 44.1 | 0.03 | 130 | - | 359 | 18.77 |
| 147 | gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 0.24 | 1649.8 | 1.16 | 0.53 | 0.03 | 44.1 | 0.03 | 147 | - | 358 | 19.38 |
| 148 | gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 0.24 | 1641.3 | 1.13 | 0.53 | 0.03 | 44.1 | 0.03 | 145 | - | 358 | 19.30 |
| 149 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 0.24 | 1640.4 | 1.20 | 0.51 | 0.05 | 44.1 | 0.03 | 128 | - | 324 | 17.32 |
| 150 | gpu: tree64 + incremental + quadtree + raster + pathtrace | 0.24 | 1611.7 | 1.69 | 0.50 | 0.09 | 44.1 | 0.03 | 120 | - | 409 | 20.30 |
| 151 | gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 0.23 | 1600.0 | 1.12 | 0.54 | 0.04 | 44.1 | 0.03 | 145 | - | 359 | 19.32 |
| 152 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 0.23 | 1593.3 | 1.13 | 0.53 | 0.05 | 44.1 | 0.03 | 132 | - | 320 | 17.34 |
| 153 | gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 0.23 | 1580.4 | 1.27 | 0.54 | 0.04 | 44.1 | 0.03 | 147 | - | 357 | 19.34 |
| 154 | gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 0.23 | 1579.6 | 1.45 | 0.52 | 0.08 | 44.1 | 0.03 | 122 | - | 366 | 18.70 |
| 155 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 0.23 | 1575.9 | 1.27 | 0.53 | 0.05 | 44.1 | 0.03 | 130 | - | 326 | 17.48 |
| 156 | gpu: tree64 + sphere + composite + raycast + pathtrace | 0.23 | 1568.9 | 1.28 | 0.57 | 0.06 | 60.9 | 0.00 | 12 | - | 307 | 12.25 |
| 157 | gpu: clipmap + sphere + quadtree + raster + pathtrace | 0.23 | 1561.4 | 0.95 | 0.57 | 0.03 | 44.1 | 0.03 | 143 | - | 400 | 20.82 |
| 158 | gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 0.23 | 1548.5 | 1.80 | 0.52 | 0.09 | 44.1 | 0.03 | 124 | - | 364 | 18.72 |
| 159 | gpu: svo_dag + sphere + quadtree + raster + pathtrace | 0.23 | 1540.2 | 1.31 | 0.55 | 0.07 | 44.1 | 0.03 | 120 | - | 398 | 19.88 |
| 160 | gpu: svo_dag + incremental + quadtree + raster + pathtrace | 0.22 | 1532.8 | 1.39 | 0.56 | 0.06 | 44.1 | 0.03 | 120 | - | 399 | 19.91 |
| 161 | gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 0.22 | 1512.4 | 1.17 | 0.59 | 0.03 | 44.1 | 0.03 | 147 | - | 353 | 19.17 |
| 162 | gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 0.22 | 1503.1 | 1.38 | 0.58 | 0.06 | 44.1 | 0.03 | 122 | - | 358 | 18.42 |
| 163 | gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 0.22 | 1500.5 | 1.15 | 0.59 | 0.04 | 44.1 | 0.03 | 145 | - | 355 | 19.17 |
| 164 | gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 0.22 | 1486.8 | 1.36 | 0.59 | 0.06 | 44.1 | 0.03 | 124 | - | 361 | 18.59 |
| 165 | gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 0.22 | 1486.7 | 1.43 | 0.60 | 0.06 | 60.9 | 0.00 | 12 | - | 307 | 12.24 |
| 166 | gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 0.22 | 1474.4 | 1.42 | 0.58 | 0.07 | 44.1 | 0.03 | 122 | - | 361 | 18.51 |
| 167 | gpu: tree64 + sphere + quadtree + raster + pathtrace | 0.21 | 1466.0 | 1.54 | 0.58 | 0.07 | 44.1 | 0.03 | 120 | - | 408 | 20.26 |
| 168 | gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 0.21 | 1449.0 | 1.49 | 0.59 | 0.07 | 44.1 | 0.03 | 124 | - | 360 | 18.55 |
| 169 | gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 0.21 | 1438.1 | 1.61 | 0.58 | 0.08 | 44.1 | 0.03 | 124 | - | 361 | 18.58 |
| 170 | gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 0.21 | 1418.9 | 1.50 | 0.60 | 0.08 | 44.1 | 0.03 | 122 | - | 361 | 18.53 |
| 171 | gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 0.18 | 1253.5 | 1.35 | 0.71 | 0.07 | 44.1 | 0.03 | 120 | - | 352 | 18.09 |
| 172 | gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 0.18 | 1212.6 | 1.46 | 0.74 | 0.07 | 44.1 | 0.03 | 124 | - | 360 | 18.57 |
| 173 | gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 0.18 | 1202.1 | 1.63 | 0.74 | 0.08 | 44.1 | 0.03 | 122 | - | 360 | 18.49 |
| 174 | gpu: tree64 + rowrun + quadtree + raster + pathtrace | 0.17 | 1141.9 | 1.81 | 0.77 | 0.09 | 44.1 | 0.03 | 120 | - | 352 | 18.10 |
| 175 | gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 0.16 | 1120.8 | 1.55 | 0.79 | 0.09 | 44.1 | 0.03 | 122 | - | 363 | 18.60 |
| 176 | gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 0.16 | 1098.9 | 1.93 | 0.80 | 0.09 | 44.1 | 0.03 | 124 | - | 362 | 18.62 |
| 177 | cpu: naive + faces + raster + bricks + refloor + direct ◆ | 0.05 | 327.6 | 4.52 | - | 3.03 | 71.0 | 0.00 | - | - | 56 | 2.15 |
| 178 | cpu: greedy + faces + raster + bricks + refloor + direct ◆ | 0.05 | 326.1 | 4.09 | - | 3.04 | 71.0 | 0.00 | - | - | 56 | 2.14 |
| 179 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 268.8 | 8.90 | 0.03 | 3.53 | 44.1 | 0.03 | 24 | - | 412 | 16.71 |
| 180 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 266.8 | 7.82 | 0.05 | 3.51 | 44.1 | 0.03 | 26 | - | 405 | 16.51 |
| 181 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 266.8 | 9.05 | 0.05 | 3.53 | 44.1 | 0.03 | 26 | - | 405 | 16.51 |
| 182 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 265.9 | 8.19 | 0.05 | 3.54 | 44.1 | 0.03 | 28 | - | 446 | 18.17 |
| 183 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 265.2 | 8.52 | 0.03 | 3.57 | 44.1 | 0.03 | 24 | - | 406 | 16.49 |
| 184 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 265.2 | 8.58 | 0.05 | 3.55 | 44.1 | 0.03 | 28 | - | 401 | 16.47 |
| 185 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 265.2 | 9.59 | 0.05 | 3.55 | 44.1 | 0.03 | 26 | - | 404 | 16.48 |
| 186 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 265.0 | 7.70 | 0.05 | 3.52 | 44.1 | 0.03 | 28 | - | 404 | 16.58 |
| 187 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 264.9 | 8.47 | 0.05 | 3.54 | 44.1 | 0.03 | 28 | - | 404 | 16.59 |
| 188 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 263.2 | 8.04 | 0.05 | 3.55 | 44.1 | 0.03 | 28 | - | 402 | 16.51 |
| 189 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 262.0 | 9.25 | 0.03 | 3.63 | 44.1 | 0.03 | 24 | - | 403 | 16.38 |
| 190 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 262.0 | 8.30 | 0.05 | 3.59 | 44.1 | 0.03 | 26 | - | 443 | 17.99 |
| 191 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 259.4 | 8.00 | 0.05 | 3.63 | 44.1 | 0.03 | 26 | - | 407 | 16.59 |
| 192 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 259.0 | 8.05 | 0.05 | 3.64 | 44.1 | 0.03 | 26 | - | 404 | 16.51 |
| 193 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 259.0 | 9.75 | 0.03 | 3.62 | 44.1 | 0.03 | 24 | - | 405 | 16.44 |
| 194 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 258.3 | 8.56 | 0.03 | 3.67 | 44.1 | 0.03 | 24 | - | 404 | 16.44 |
| 195 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 257.5 | 7.99 | 0.05 | 3.64 | 44.1 | 0.03 | 28 | - | 402 | 16.50 |
| 196 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.04 | 256.9 | 9.35 | 0.03 | 3.66 | 44.1 | 0.03 | 24 | - | 432 | 17.50 |
| 197 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.04 | 254.8 | 9.07 | 0.05 | 3.67 | 44.1 | 0.03 | 26 | - | 407 | 16.61 |
| 198 | cpu: hier + refloor + raycast + direct ◆ | 0.04 | 248.9 | 7.53 | - | 3.99 | 64.8 | 0.00 | - | - | 31 | 1.19 |
| 199 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 218.3 | 12.18 | 0.03 | 4.37 | 44.1 | 0.03 | 24 | - | 413 | 16.76 |
| 200 | cpu: prefix + refloor + raycast + direct ◆ | 0.03 | 212.8 | 8.09 | - | 4.67 | 64.8 | 0.00 | - | - | 26 | 1.00 |
| 201 | cpu: u8 + refloor + raycast + direct | 0.03 | 191.8 | 8.87 | - | 5.19 | 64.8 | 0.00 | - | - | 28 | 1.07 |
| 202 | cpu: bits + refloor + raycast + direct | 0.03 | 191.6 | 8.40 | - | 5.19 | 64.8 | 0.00 | - | - | 31 | 1.18 |
| 203 | cpu: morton + refloor + raycast + direct | 0.03 | 189.2 | 9.19 | - | 5.24 | 64.8 | 0.00 | - | - | 31 | 1.18 |
| 204 | cpu: hier + incremental + raycast + direct | 0.03 | 188.9 | 8.69 | - | 5.27 | 63.9 | 0.00 | - | - | 31 | 1.20 |
| 205 | cpu: palette + refloor + raycast + direct | 0.03 | 187.0 | 8.90 | - | 5.30 | 64.8 | 0.00 | - | - | 26 | 1.00 |
| 206 | cpu: bricks + refloor + raycast + direct | 0.03 | 186.8 | 8.69 | - | 5.33 | 64.8 | 0.00 | - | - | 30 | 1.17 |
| 207 | cpu: brickmajor + refloor + raycast + direct | 0.03 | 184.9 | 8.22 | - | 5.35 | 64.8 | 0.00 | - | - | 31 | 1.19 |
| 208 | cpu: bits + incremental + raycast + direct | 0.02 | 166.3 | 9.32 | - | 5.98 | 63.9 | 0.00 | - | - | 31 | 1.19 |
| 209 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.02 | 166.1 | 22.88 | 0.05 | 5.67 | 44.1 | 0.03 | 28 | - | 414 | 16.96 |
| 210 | cpu: morton + incremental + raycast + direct | 0.02 | 164.2 | 9.40 | - | 6.04 | 63.9 | 0.00 | - | - | 31 | 1.18 |
| 211 | cpu: brickmajor + incremental + raycast + direct | 0.02 | 162.4 | 9.40 | - | 6.12 | 63.9 | 0.00 | - | - | 31 | 1.19 |
| 212 | cpu: palette + incremental + raycast + direct | 0.02 | 161.0 | 10.16 | - | 6.16 | 63.9 | 0.00 | - | - | 26 | 1.00 |
| 213 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.02 | 160.4 | 23.08 | 0.05 | 5.94 | 44.1 | 0.03 | 28 | - | 415 | 16.98 |
| 214 | cpu: bricks + incremental + raycast + direct | 0.02 | 159.6 | 9.29 | - | 6.24 | 63.9 | 0.00 | - | - | 31 | 1.18 |
| 215 | cpu: u8 + incremental + raycast + direct | 0.02 | 159.1 | 9.14 | - | 6.25 | 63.9 | 0.00 | - | - | 28 | 1.08 |
| 216 | cpu: prefix + incremental + raycast + direct | 0.02 | 158.8 | 8.90 | - | 6.26 | 63.9 | 0.00 | - | - | 26 | 1.00 |
| 217 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.02 | 126.1 | 22.26 | 0.05 | 7.62 | 44.1 | 0.03 | 26 | - | 417 | 17.01 |
| 218 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.02 | 106.2 | 26.24 | 0.03 | 9.04 | 44.1 | 0.03 | 24 | - | 424 | 17.18 |
| 219 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.3 | 40.09 | 0.05 | 18.81 | 44.1 | 0.03 | 26 | - | 377 | 15.47 |
| 220 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.3 | 39.62 | 0.05 | 18.86 | 44.1 | 0.03 | 26 | - | 392 | 16.02 |
| 221 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.0 | 39.62 | 0.05 | 18.91 | 44.1 | 0.03 | 28 | - | 384 | 15.82 |
| 222 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.0 | 41.82 | 0.05 | 18.94 | 44.1 | 0.03 | 26 | - | 378 | 15.51 |
| 223 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 52.0 | 41.43 | 0.05 | 18.95 | 44.1 | 0.03 | 26 | - | 388 | 15.89 |
| 224 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.6 | 43.57 | 0.05 | 19.07 | 44.1 | 0.03 | 26 | - | 381 | 15.60 |
| 225 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.4 | 42.89 | 0.05 | 19.17 | 44.1 | 0.03 | 28 | - | 396 | 16.28 |
| 226 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.1 | 45.29 | 0.04 | 19.31 | 44.1 | 0.03 | 24 | - | 375 | 15.32 |
| 227 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 51.0 | 40.97 | 0.05 | 19.30 | 44.1 | 0.03 | 28 | - | 373 | 15.39 |
| 228 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.9 | 43.04 | 0.04 | 19.35 | 44.1 | 0.03 | 24 | - | 386 | 15.73 |
| 229 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.9 | 42.48 | 0.04 | 19.39 | 44.1 | 0.03 | 24 | - | 375 | 15.29 |
| 230 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.6 | 43.26 | 0.03 | 19.49 | 44.1 | 0.03 | 24 | - | 403 | 16.39 |
| 231 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.3 | 43.30 | 0.05 | 19.56 | 44.1 | 0.03 | 28 | - | 378 | 15.56 |
| 232 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.2 | 42.63 | 0.05 | 19.61 | 44.1 | 0.03 | 28 | - | 387 | 15.91 |
| 233 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 50.0 | 42.94 | 0.05 | 19.68 | 44.1 | 0.03 | 28 | - | 385 | 15.83 |
| 234 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 49.6 | 42.40 | 0.05 | 19.88 | 44.1 | 0.03 | 28 | - | 388 | 15.95 |
| 235 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 49.4 | 44.12 | 0.04 | 19.98 | 44.1 | 0.03 | 24 | - | 380 | 15.49 |
| 236 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 49.4 | 42.85 | 0.05 | 19.94 | 44.1 | 0.03 | 26 | - | 376 | 15.43 |
| 237 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 48.9 | 44.80 | 0.04 | 20.17 | 44.1 | 0.03 | 24 | - | 383 | 15.60 |
| 238 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 48.8 | 44.61 | 0.05 | 20.18 | 44.1 | 0.03 | 26 | - | 377 | 15.47 |
| 239 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 47.4 | 43.79 | 0.03 | 20.81 | 44.1 | 0.03 | 24 | - | 387 | 15.76 |
| 240 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 42.3 | 50.11 | 0.05 | 23.30 | 44.1 | 0.03 | 26 | - | 404 | 16.48 |
| 241 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.00 | 31.4 | 122.29 | 0.04 | 31.40 | 44.1 | 0.03 | 24 | - | 437 | 17.67 |
| 242 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.00 | 27.7 | 118.99 | 0.05 | 35.50 | 44.1 | 0.03 | 28 | - | 440 | 17.95 |
| 243 | gpu: heightmap + columns + heightfield + pathtrace (incorrect) | 0.38 | 2580.7 | 0.96 | 0.30 | 0.01 | 12.2 | 47.60 | 11 | - | 310 | 12.30 |
| 244 | cpu: heightmap + columns + heightfield + direct (incorrect) | 0.19 | 1325.0 | 1.47 | - | 0.72 | 12.2 | 47.60 | - | - | 32 | 1.23 |

### terrain-flight [lit]

1920×1080. camera 2.25 blocks/frame (max 3.0), turn 0.35°/frame; 3985 chunks at start, 11.1 loads/frame; chunk fill 11%, solid 8.5%, surface 0.65%; sky 64%, hits at 268 blocks (median 235); 0.0 edits/frame; 400 lights (animated); 0 mesh triangles.

| # | renderer | rel | fps | frame p99 ms | gpu ms | cpu ms | psnr | bad pct | gpu mb | gpu device mb | peak rss mb | mem rel |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu: bricks + refloor + composite + raycast + direct ◆ | 1.00 | 568.7 | 8.77 | 1.16 | 0.77 | 57.8 | 0.01 | 531 | - | 587 | 4.43 |
| 2 | gpu: prefix + refloor + composite + raycast + direct | 0.96 | 545.8 | 11.12 | 1.03 | 0.78 | 57.8 | 0.01 | 571 | - | 584 | 4.58 |
| 3 | gpu: hier + refloor + composite + raycast + direct ◆ | 0.92 | 522.5 | 11.82 | 1.08 | 0.79 | 57.8 | 0.01 | 531 | - | 585 | 4.43 |
| 4 | gpu: bricks + refloor + beam + composite + raycast + direct ◆ | 0.89 | 504.9 | 7.80 | 1.41 | 0.80 | 57.7 | 0.01 | 531 | - | 584 | 4.42 |
| 5 | gpu: prefix + refloor + beam + composite + raycast + direct | 0.87 | 496.2 | 11.54 | 1.25 | 0.78 | 57.8 | 0.01 | 571 | - | 584 | 4.58 |
| 6 | gpu: tree64 + refloor + composite + raycast + direct ◆ | 0.86 | 486.3 | 8.66 | 1.43 | 1.15 | 57.8 | 0.01 | 94 | - | 681 | 3.07 |
| 7 | gpu: prefix + rowrun + composite + raycast + direct | 0.84 | 478.8 | 10.13 | 1.37 | 0.76 | 57.8 | 0.01 | 571 | - | 584 | 4.58 |
| 8 | gpu: hier + refloor + beam + composite + raycast + direct | 0.84 | 478.2 | 10.49 | 1.30 | 0.80 | 57.7 | 0.01 | 531 | - | 586 | 4.43 |
| 9 | gpu: distance + sphere + composite + raycast + direct | 0.83 | 473.0 | 18.42 | 0.85 | 0.77 | 57.7 | 0.01 | 787 | - | 584 | 5.44 |
| 10 | gpu: rows + refloor + composite + raycast + direct | 0.83 | 470.7 | 13.76 | 1.18 | 0.81 | 57.8 | 0.01 | 563 | - | 584 | 4.55 |
| 11 | gpu: distance + refloor + composite + raycast + direct | 0.80 | 453.0 | 18.54 | 0.99 | 0.79 | 57.7 | 0.01 | 787 | - | 584 | 5.44 |
| 12 | gpu: svo_dag + refloor + composite + raycast + direct ◆ | 0.79 | 450.0 | 12.70 | 1.35 | 1.41 | 57.8 | 0.01 | 33 | - | 716 | 2.97 |
| 13 | gpu: rows + refloor + beam + composite + raycast + direct | 0.78 | 445.3 | 11.93 | 1.40 | 0.78 | 57.7 | 0.01 | 563 | - | 584 | 4.55 |
| 14 | gpu: distance + sphere + beam + composite + raycast + direct | 0.78 | 444.2 | 15.89 | 1.08 | 0.75 | 57.7 | 0.01 | 787 | - | 584 | 5.44 |
| 15 | gpu: rows + rowrun + composite + raycast + direct | 0.78 | 443.8 | 10.67 | 1.42 | 0.80 | 57.8 | 0.01 | 563 | - | 584 | 4.55 |
| 16 | gpu: prefix + rowrun + beam + composite + raycast + direct | 0.77 | 438.5 | 9.05 | 1.58 | 0.80 | 57.8 | 0.01 | 571 | - | 584 | 4.58 |
| 17 | gpu: bricks + rowrun + composite + raycast + direct | 0.75 | 428.7 | 7.53 | 1.83 | 0.88 | 57.8 | 0.01 | 531 | - | 583 | 4.42 |
| 18 | gpu: distance + refloor + beam + composite + raycast + direct | 0.74 | 420.2 | 14.94 | 1.23 | 0.79 | 57.7 | 0.01 | 787 | - | 585 | 5.44 |
| 19 | gpu: clipmap + refloor + composite + raycast + direct | 0.72 | 411.8 | 7.89 | 1.87 | 0.85 | 57.8 | 0.01 | 571 | - | 633 | 4.78 |
| 20 | gpu: rows + rowrun + beam + composite + raycast + direct | 0.71 | 405.1 | 10.55 | 1.64 | 0.81 | 57.7 | 0.01 | 563 | - | 585 | 4.55 |
| 21 | gpu: bricks + rowrun + beam + composite + raycast + direct | 0.71 | 402.0 | 7.28 | 2.00 | 0.89 | 57.7 | 0.01 | 531 | - | 584 | 4.42 |
| 22 | gpu: distance + rowrun + composite + raycast + direct | 0.71 | 401.2 | 15.45 | 1.35 | 0.80 | 57.7 | 0.01 | 787 | - | 584 | 5.44 |
| 23 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.70 | 397.0 | 17.92 | 1.05 | 1.53 | 52.0 | 0.01 | 676 | - | 698 | 5.45 |
| 24 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.70 | 395.6 | 17.94 | 1.04 | 1.54 | 51.9 | 0.01 | 668 | - | 702 | 5.43 |
| 25 | gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.67 | 380.6 | 17.88 | 1.17 | 1.92 | 52.0 | 0.01 | 240 | - | 726 | 3.83 |
| 26 | gpu: boxes@cpu + rayquery + raycast + direct | 0.66 | 376.3 | 14.01 | 1.41 | 1.42 | 57.6 | 0.01 | 569 | - | 942 | 5.99 |
| 27 | gpu: distance + rowrun + beam + composite + raycast + direct | 0.65 | 372.4 | 15.00 | 1.57 | 0.82 | 57.7 | 0.01 | 787 | - | 585 | 5.44 |
| 28 | gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.65 | 372.4 | 17.86 | 1.25 | 1.55 | 51.9 | 0.01 | 668 | - | 701 | 5.43 |
| 29 | gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.65 | 372.2 | 21.76 | 1.01 | 1.55 | 51.9 | 0.01 | 708 | - | 701 | 5.59 |
| 30 | gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.65 | 371.4 | 17.25 | 1.24 | 1.57 | 52.0 | 0.01 | 676 | - | 703 | 5.47 |
| 31 | gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.65 | 367.9 | 18.96 | 1.24 | 1.53 | 52.0 | 0.01 | 717 | - | 767 | 5.88 |
| 32 | gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.65 | 367.5 | 17.28 | 1.27 | 1.93 | 51.9 | 0.01 | 232 | - | 726 | 3.80 |
| 33 | gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.64 | 364.0 | 21.08 | 1.04 | 1.58 | 52.0 | 0.01 | 716 | - | 702 | 5.62 |
| 34 | gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.64 | 362.8 | 21.72 | 1.07 | 1.55 | 51.9 | 0.01 | 668 | - | 703 | 5.44 |
| 35 | gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.64 | 362.4 | 20.84 | 1.06 | 1.55 | 52.0 | 0.01 | 676 | - | 703 | 5.47 |
| 36 | gpu: bricks + refloor + quadtree + gbuffer + direct | 0.64 | 361.6 | 8.05 | 2.25 | 0.93 | 50.8 | 0.02 | 729 | - | 612 | 5.32 |
| 37 | gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.64 | 361.4 | 17.38 | 1.28 | 1.54 | 51.9 | 0.01 | 709 | - | 768 | 5.85 |
| 38 | gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.63 | 360.1 | 20.34 | 1.13 | 1.56 | 51.9 | 0.01 | 708 | - | 702 | 5.59 |
| 39 | gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.63 | 359.6 | 21.97 | 1.13 | 1.55 | 52.0 | 0.01 | 716 | - | 702 | 5.62 |
| 40 | gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.63 | 358.6 | 17.61 | 1.34 | 1.53 | 52.0 | 0.01 | 717 | - | 768 | 5.89 |
| 41 | gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.63 | 358.4 | 22.35 | 1.04 | 1.56 | 51.9 | 0.01 | 700 | - | 701 | 5.56 |
| 42 | gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.63 | 358.2 | 16.08 | 1.39 | 1.56 | 52.0 | 0.01 | 676 | - | 701 | 5.46 |
| 43 | gpu: tree64 + incremental + composite + raycast + direct | 0.63 | 358.1 | 8.01 | 2.30 | 1.32 | 57.7 | 0.01 | 94 | - | 676 | 3.05 |
| 44 | gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.63 | 357.6 | 22.58 | 1.03 | 1.57 | 52.0 | 0.01 | 708 | - | 698 | 5.57 |
| 45 | gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.63 | 356.0 | 17.64 | 1.36 | 1.53 | 51.9 | 0.01 | 709 | - | 768 | 5.86 |
| 46 | gpu: clipmap + rowrun + composite + raycast + direct | 0.62 | 354.5 | 7.87 | 2.34 | 0.95 | 57.8 | 0.01 | 571 | - | 634 | 4.78 |
| 47 | gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.62 | 353.4 | 21.86 | 1.12 | 1.54 | 52.0 | 0.01 | 708 | - | 698 | 5.57 |
| 48 | gpu: bricks + refloor + quadtree + visbuffer + direct | 0.62 | 352.5 | 7.81 | 2.31 | 0.96 | 50.8 | 0.02 | 721 | - | 613 | 5.29 |
| 49 | gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.62 | 352.5 | 17.44 | 1.44 | 1.57 | 51.9 | 0.01 | 668 | - | 702 | 5.43 |
| 50 | gpu: tree64 + refloor + beam + composite + raycast + direct | 0.62 | 351.7 | 8.83 | 2.35 | 1.25 | 57.7 | 0.01 | 94 | - | 677 | 3.06 |
| 51 | gpu: prefix + refloor + quadtree + visbuffer + direct | 0.62 | 351.6 | 9.80 | 2.13 | 0.93 | 50.8 | 0.02 | 761 | - | 610 | 5.44 |
| 52 | gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.62 | 350.8 | 22.83 | 1.12 | 1.56 | 51.9 | 0.01 | 700 | - | 702 | 5.56 |
| 53 | gpu: prefix + refloor + quadtree + gbuffer + direct | 0.62 | 350.4 | 9.44 | 2.14 | 0.92 | 50.8 | 0.02 | 769 | - | 612 | 5.48 |
| 54 | gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.61 | 349.1 | 17.48 | 1.49 | 1.93 | 52.0 | 0.01 | 240 | - | 726 | 3.83 |
| 55 | gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.61 | 347.2 | 17.57 | 1.50 | 1.92 | 51.9 | 0.01 | 232 | - | 726 | 3.80 |
| 56 | gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.61 | 345.5 | 16.89 | 1.45 | 1.56 | 51.9 | 0.01 | 709 | - | 768 | 5.86 |
| 57 | gpu: hier + refloor + quadtree + gbuffer + direct | 0.61 | 345.4 | 9.07 | 2.18 | 0.92 | 50.8 | 0.02 | 729 | - | 614 | 5.32 |
| 58 | gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.61 | 344.5 | 26.62 | 1.15 | 2.22 | 52.0 | 0.01 | 178 | - | 803 | 3.89 |
| 59 | gpu: hier + refloor + quadtree + visbuffer + direct | 0.61 | 344.4 | 9.45 | 2.18 | 0.93 | 50.8 | 0.02 | 721 | - | 614 | 5.29 |
| 60 | gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.61 | 344.2 | 17.46 | 1.44 | 1.56 | 52.0 | 0.01 | 717 | - | 768 | 5.89 |
| 61 | gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.60 | 341.8 | 26.45 | 1.19 | 2.21 | 51.9 | 0.01 | 170 | - | 799 | 3.85 |
| 62 | gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.60 | 338.9 | 19.14 | 1.34 | 1.56 | 52.0 | 0.01 | 676 | - | 707 | 5.48 |
| 63 | gpu: hier + rowrun + composite + raycast + direct | 0.59 | 337.8 | 9.70 | 2.36 | 0.93 | 57.8 | 0.01 | 531 | - | 586 | 4.43 |
| 64 | gpu: clipmap + incremental + composite + raycast + direct | 0.59 | 336.2 | 8.18 | 2.52 | 0.95 | 57.7 | 0.01 | 571 | - | 634 | 4.78 |
| 65 | gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.59 | 336.1 | 20.71 | 1.35 | 1.57 | 51.9 | 0.01 | 668 | - | 707 | 5.45 |
| 66 | gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.59 | 333.9 | 18.29 | 1.43 | 1.56 | 52.0 | 0.01 | 716 | - | 702 | 5.62 |
| 67 | gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.59 | 333.8 | 18.31 | 1.44 | 1.55 | 51.9 | 0.01 | 708 | - | 697 | 5.57 |
| 68 | gpu: hier + sphere + composite + raycast + direct | 0.59 | 333.4 | 9.40 | 2.42 | 0.95 | 57.7 | 0.01 | 531 | - | 585 | 4.43 |
| 69 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + direct | 0.59 | 332.8 | 7.50 | 2.48 | 1.00 | 50.9 | 0.02 | 721 | - | 615 | 5.30 |
| 70 | gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.58 | 332.5 | 20.92 | 1.38 | 1.58 | 52.0 | 0.01 | 676 | - | 707 | 5.49 |
| 71 | gpu: bricks + sphere + composite + raycast + direct | 0.58 | 331.7 | 7.21 | 2.68 | 0.98 | 57.7 | 0.01 | 531 | - | 583 | 4.42 |
| 72 | gpu: rows + refloor + quadtree + visbuffer + direct | 0.58 | 330.1 | 10.17 | 2.25 | 0.96 | 50.8 | 0.02 | 753 | - | 612 | 5.41 |
| 73 | gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.58 | 329.4 | 17.64 | 1.65 | 1.57 | 52.0 | 0.01 | 717 | - | 768 | 5.89 |
| 74 | gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.58 | 329.0 | 20.88 | 1.42 | 1.58 | 51.9 | 0.01 | 668 | - | 707 | 5.45 |
| 75 | gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.58 | 328.5 | 27.71 | 0.98 | 1.55 | 51.9 | 0.01 | 924 | - | 698 | 6.43 |
| 76 | gpu: rows + refloor + quadtree + gbuffer + direct | 0.58 | 327.3 | 9.62 | 2.30 | 0.94 | 50.8 | 0.02 | 761 | - | 612 | 5.44 |
| 77 | gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.58 | 327.0 | 27.65 | 1.04 | 1.53 | 51.9 | 0.01 | 924 | - | 702 | 6.45 |
| 78 | gpu: distance + sphere + quadtree + visbuffer + direct | 0.57 | 326.4 | 14.03 | 1.96 | 0.88 | 50.8 | 0.02 | 977 | - | 612 | 6.30 |
| 79 | gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.57 | 326.1 | 21.87 | 1.41 | 1.56 | 51.9 | 0.01 | 700 | - | 701 | 5.56 |
| 80 | gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.57 | 325.2 | 28.99 | 1.03 | 1.54 | 52.0 | 0.01 | 932 | - | 699 | 6.47 |
| 81 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.57 | 324.8 | 27.31 | 0.66 | 2.32 | 50.0 | 0.02 | 612 | - | 637 | 4.95 |
| 82 | gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.57 | 324.7 | 28.36 | 1.01 | 1.56 | 52.0 | 0.01 | 932 | - | 702 | 6.48 |
| 83 | gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.57 | 324.3 | 16.04 | 1.70 | 1.58 | 51.9 | 0.01 | 709 | - | 768 | 5.86 |
| 84 | gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.57 | 324.2 | 22.16 | 1.41 | 1.58 | 52.0 | 0.01 | 708 | - | 701 | 5.59 |
| 85 | gpu: clipmap + refloor + beam + composite + raycast + direct | 0.57 | 324.2 | 7.63 | 2.67 | 1.00 | 57.7 | 0.01 | 571 | - | 638 | 4.79 |
| 86 | gpu: bricks + incremental + composite + raycast + direct | 0.57 | 321.9 | 7.37 | 2.78 | 0.98 | 57.7 | 0.01 | 531 | - | 583 | 4.42 |
| 87 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + direct | 0.57 | 321.4 | 7.65 | 2.56 | 1.00 | 50.9 | 0.02 | 729 | - | 615 | 5.33 |
| 88 | gpu: svo_dag + refloor + beam + composite + raycast + direct | 0.56 | 320.6 | 12.75 | 2.37 | 1.60 | 57.8 | 0.01 | 33 | - | 718 | 2.98 |
| 89 | gpu: distance + sphere + quadtree + gbuffer + direct | 0.56 | 320.6 | 14.13 | 2.02 | 0.90 | 50.8 | 0.02 | 985 | - | 612 | 6.33 |
| 90 | gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.56 | 320.4 | 26.36 | 1.12 | 1.54 | 51.9 | 0.01 | 924 | - | 694 | 6.42 |
| 91 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.56 | 319.8 | 27.63 | 0.68 | 2.35 | 50.0 | 0.02 | 604 | - | 637 | 4.92 |
| 92 | gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.56 | 319.4 | 16.63 | 1.81 | 1.98 | 51.9 | 0.01 | 232 | - | 725 | 3.79 |
| 93 | gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.56 | 318.0 | 17.37 | 1.82 | 1.98 | 52.0 | 0.01 | 240 | - | 727 | 3.83 |
| 94 | gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.56 | 317.7 | 27.18 | 0.81 | 2.67 | 50.0 | 0.02 | 176 | - | 699 | 3.47 |
| 95 | gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.56 | 317.4 | 27.09 | 0.87 | 2.31 | 50.0 | 0.02 | 612 | - | 636 | 4.95 |
| 96 | gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.56 | 317.2 | 28.31 | 0.80 | 2.69 | 50.0 | 0.02 | 168 | - | 698 | 3.43 |
| 97 | gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.56 | 316.8 | 25.66 | 1.12 | 1.56 | 52.0 | 0.01 | 932 | - | 702 | 6.48 |
| 98 | gpu: hier + rowrun + beam + composite + raycast + direct | 0.56 | 315.8 | 9.31 | 2.57 | 0.98 | 57.7 | 0.01 | 531 | - | 586 | 4.43 |
| 99 | gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.55 | 315.4 | 27.76 | 0.82 | 2.29 | 50.0 | 0.02 | 645 | - | 703 | 5.34 |
| 100 | gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.55 | 314.8 | 27.40 | 0.87 | 2.32 | 50.0 | 0.02 | 604 | - | 636 | 4.92 |
| 101 | gpu: hier + sphere + beam + composite + raycast + direct | 0.55 | 314.6 | 9.34 | 2.62 | 0.95 | 57.7 | 0.01 | 531 | - | 586 | 4.43 |
| 102 | gpu: prefix + sphere + composite + raycast + direct | 0.55 | 310.8 | 9.26 | 2.68 | 1.02 | 57.7 | 0.01 | 571 | - | 584 | 4.58 |
| 103 | gpu: distance + refloor + quadtree + gbuffer + direct | 0.54 | 309.4 | 13.61 | 2.15 | 0.93 | 50.8 | 0.02 | 985 | - | 611 | 6.33 |
| 104 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + direct | 0.54 | 309.4 | 15.56 | 1.98 | 1.64 | 52.0 | 0.01 | 660 | - | 701 | 5.40 |
| 105 | gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.54 | 309.1 | 28.05 | 0.83 | 2.33 | 50.0 | 0.02 | 653 | - | 703 | 5.37 |
| 106 | gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.54 | 307.9 | 29.50 | 0.75 | 2.29 | 50.0 | 0.02 | 652 | - | 636 | 5.11 |
| 107 | gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.54 | 307.4 | 29.83 | 0.63 | 2.33 | 50.0 | 0.02 | 644 | - | 636 | 5.08 |
| 108 | gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.54 | 306.9 | 26.69 | 1.00 | 2.32 | 50.0 | 0.02 | 612 | - | 636 | 4.95 |
| 109 | gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.54 | 306.7 | 30.37 | 0.66 | 2.30 | 50.0 | 0.02 | 612 | - | 642 | 4.97 |
| 110 | gpu: bricks + sphere + beam + composite + raycast + direct | 0.54 | 306.6 | 7.46 | 2.92 | 1.07 | 57.7 | 0.01 | 531 | - | 583 | 4.42 |
| 111 | gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.54 | 306.4 | 26.73 | 1.03 | 2.29 | 50.0 | 0.02 | 604 | - | 636 | 4.92 |
| 112 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.54 | 306.3 | 28.11 | 0.98 | 2.29 | 50.0 | 0.02 | 645 | - | 702 | 5.34 |
| 113 | gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.54 | 305.0 | 29.76 | 0.65 | 2.33 | 50.0 | 0.02 | 652 | - | 637 | 5.11 |
| 114 | gpu: tree64 + refloor + quadtree + visbuffer + direct | 0.54 | 304.3 | 8.07 | 2.82 | 1.37 | 50.8 | 0.02 | 284 | - | 711 | 3.95 |
| 115 | gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.53 | 303.8 | 29.89 | 0.75 | 2.32 | 50.0 | 0.02 | 644 | - | 636 | 5.08 |
| 116 | gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.53 | 303.8 | 30.33 | 0.68 | 2.32 | 50.0 | 0.02 | 604 | - | 642 | 4.94 |
| 117 | gpu: distance + refloor + quadtree + visbuffer + direct | 0.53 | 303.6 | 14.27 | 2.19 | 0.94 | 50.8 | 0.02 | 977 | - | 612 | 6.30 |
| 118 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.53 | 303.4 | 27.56 | 0.97 | 2.32 | 50.0 | 0.02 | 653 | - | 702 | 5.37 |
| 119 | gpu: prefix + rowrun + quadtree + gbuffer + direct | 0.53 | 302.9 | 9.09 | 2.67 | 1.01 | 50.8 | 0.02 | 769 | - | 611 | 5.47 |
| 120 | gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.53 | 302.2 | 24.27 | 1.69 | 2.24 | 51.9 | 0.01 | 170 | - | 803 | 3.86 |
| 121 | gpu: clipmap + refloor + quadtree + gbuffer + direct | 0.53 | 302.0 | 8.48 | 2.85 | 1.02 | 50.8 | 0.02 | 769 | - | 662 | 5.67 |
| 122 | gpu: rows + rowrun + quadtree + gbuffer + direct | 0.53 | 302.0 | 9.88 | 2.61 | 1.00 | 50.8 | 0.02 | 761 | - | 607 | 5.42 |
| 123 | gpu: rows + rowrun + quadtree + visbuffer + direct | 0.53 | 301.8 | 9.69 | 2.61 | 1.00 | 50.8 | 0.02 | 753 | - | 610 | 5.41 |
| 124 | gpu: clipmap + refloor + quadtree + visbuffer + direct | 0.53 | 301.7 | 8.12 | 2.84 | 1.05 | 50.8 | 0.02 | 761 | - | 662 | 5.65 |
| 125 | gpu: rows + sphere + composite + raycast + direct | 0.53 | 300.5 | 10.10 | 2.66 | 0.98 | 57.7 | 0.01 | 563 | - | 584 | 4.55 |
| 126 | gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.53 | 300.2 | 31.08 | 0.71 | 2.29 | 50.0 | 0.02 | 644 | - | 636 | 5.08 |
| 127 | gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.53 | 300.0 | 27.26 | 1.06 | 2.30 | 50.0 | 0.02 | 645 | - | 703 | 5.34 |
| 128 | gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.53 | 300.0 | 24.14 | 1.70 | 2.25 | 52.0 | 0.01 | 178 | - | 802 | 3.89 |
| 129 | gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.53 | 299.8 | 27.25 | 1.11 | 2.68 | 50.0 | 0.02 | 168 | - | 699 | 3.44 |
| 130 | gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.53 | 299.7 | 27.06 | 1.07 | 2.28 | 50.0 | 0.02 | 653 | - | 703 | 5.38 |
| 131 | gpu: naive@cpu + faces + prefix + refloor + depth + raster + direct | 0.53 | 299.2 | 17.41 | 1.88 | 1.62 | 52.0 | 0.01 | 700 | - | 694 | 5.53 |
| 132 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + direct | 0.53 | 299.2 | 25.58 | 1.12 | 2.32 | 50.0 | 0.02 | 596 | - | 637 | 4.89 |
| 133 | gpu: tree64 + refloor + quadtree + gbuffer + direct | 0.53 | 299.2 | 8.02 | 2.88 | 1.40 | 50.8 | 0.02 | 292 | - | 710 | 3.97 |
| 134 | gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.53 | 298.9 | 31.24 | 0.74 | 2.31 | 50.0 | 0.02 | 636 | - | 636 | 5.05 |
| 135 | gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.52 | 298.4 | 31.75 | 0.68 | 2.32 | 50.0 | 0.02 | 636 | - | 636 | 5.05 |
| 136 | gpu: prefix + rowrun + quadtree + visbuffer + direct | 0.52 | 297.2 | 9.55 | 2.72 | 1.04 | 50.8 | 0.02 | 761 | - | 613 | 5.45 |
| 137 | gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.52 | 296.4 | 30.83 | 0.74 | 2.33 | 50.0 | 0.02 | 644 | - | 636 | 5.08 |
| 138 | gpu: prefix + sphere + beam + composite + raycast + direct | 0.52 | 296.3 | 8.86 | 2.92 | 1.00 | 57.7 | 0.01 | 571 | - | 584 | 4.58 |
| 139 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.52 | 296.3 | 35.54 | 0.77 | 2.95 | 50.0 | 0.02 | 106 | - | 741 | 3.36 |
| 140 | gpu: distance + sphere + composite + raycast + pathtrace | 0.52 | 296.0 | 13.21 | 2.41 | 0.95 | 31.7 | 8.00 | 819 | - | 584 | 5.56 |
| 141 | gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.52 | 295.9 | 27.71 | 1.16 | 2.67 | 50.0 | 0.02 | 176 | - | 699 | 3.47 |
| 142 | gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.52 | 292.9 | 16.33 | 2.16 | 2.03 | 52.0 | 0.01 | 240 | - | 726 | 3.83 |
| 143 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.51 | 291.0 | 24.18 | 1.87 | 2.25 | 51.9 | 0.01 | 170 | - | 804 | 3.86 |
| 144 | gpu: bricks + incremental + beam + composite + raycast + direct | 0.51 | 290.4 | 7.55 | 3.18 | 1.05 | 57.7 | 0.01 | 531 | - | 584 | 4.42 |
| 145 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.51 | 290.2 | 34.74 | 0.77 | 3.00 | 50.0 | 0.02 | 114 | - | 742 | 3.39 |
| 146 | gpu: prefix + refloor + composite + raycast + pathtrace | 0.51 | 290.0 | 9.55 | 2.97 | 1.02 | 31.7 | 8.00 | 603 | - | 585 | 4.71 |
| 147 | gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.51 | 289.8 | 30.87 | 0.97 | 2.32 | 50.0 | 0.02 | 604 | - | 641 | 4.94 |
| 148 | gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.51 | 289.6 | 30.10 | 0.96 | 2.33 | 50.0 | 0.02 | 612 | - | 641 | 4.97 |
| 149 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.51 | 288.7 | 23.78 | 1.88 | 2.26 | 52.0 | 0.01 | 178 | - | 803 | 3.89 |
| 150 | gpu: hier + refloor + composite + raycast + pathtrace | 0.51 | 287.7 | 10.09 | 2.98 | 1.03 | 31.7 | 8.00 | 563 | - | 586 | 4.56 |
| 151 | gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.51 | 287.3 | 29.59 | 1.05 | 2.31 | 50.0 | 0.02 | 652 | - | 636 | 5.11 |
| 152 | gpu: greedy@cpu + faces + prefix + refloor + depth + raster + direct | 0.51 | 287.2 | 29.49 | 1.04 | 2.33 | 50.0 | 0.02 | 636 | - | 636 | 5.05 |
| 153 | gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.50 | 286.9 | 29.85 | 1.02 | 2.33 | 50.0 | 0.02 | 604 | - | 641 | 4.94 |
| 154 | gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.50 | 286.6 | 29.57 | 1.06 | 2.33 | 50.0 | 0.02 | 644 | - | 636 | 5.08 |
| 155 | gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.50 | 286.3 | 30.79 | 1.00 | 2.30 | 50.0 | 0.02 | 644 | - | 636 | 5.08 |
| 156 | gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.50 | 286.2 | 30.04 | 1.04 | 2.31 | 50.0 | 0.02 | 612 | - | 642 | 4.97 |
| 157 | gpu: clipmap + rowrun + beam + composite + raycast + direct | 0.50 | 286.1 | 8.29 | 3.13 | 1.04 | 57.7 | 0.01 | 571 | - | 639 | 4.80 |
| 158 | gpu: naive@cpu + faces + rows + refloor + depth + raster + direct | 0.50 | 286.1 | 17.64 | 1.98 | 1.62 | 52.0 | 0.01 | 692 | - | 702 | 5.53 |
| 159 | gpu: rows + sphere + beam + composite + raycast + direct | 0.50 | 285.8 | 10.46 | 2.87 | 1.04 | 57.7 | 0.01 | 563 | - | 584 | 4.55 |
| 160 | gpu: naive@cpu + faces + hier + refloor + depth + raster + direct | 0.50 | 285.2 | 17.32 | 2.03 | 1.68 | 52.0 | 0.01 | 660 | - | 699 | 5.39 |
| 161 | gpu: prefix + incremental + composite + raycast + direct | 0.50 | 284.5 | 9.64 | 3.09 | 0.99 | 57.7 | 0.01 | 571 | - | 584 | 4.58 |
| 162 | gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.50 | 284.0 | 16.70 | 2.27 | 2.05 | 51.9 | 0.01 | 232 | - | 726 | 3.80 |
| 163 | gpu: hier + incremental + composite + raycast + direct | 0.50 | 283.8 | 9.73 | 3.07 | 1.03 | 57.7 | 0.01 | 531 | - | 586 | 4.43 |
| 164 | gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.50 | 283.7 | 26.57 | 1.28 | 2.33 | 50.0 | 0.02 | 645 | - | 702 | 5.34 |
| 165 | gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.50 | 283.3 | 30.44 | 1.03 | 2.31 | 50.0 | 0.02 | 636 | - | 636 | 5.04 |
| 166 | gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.50 | 281.5 | 25.95 | 1.41 | 2.67 | 50.0 | 0.02 | 176 | - | 699 | 3.47 |
| 167 | gpu: greedy@cpu + faces + hier + refloor + depth + raster + direct | 0.49 | 281.4 | 29.26 | 1.14 | 2.31 | 50.0 | 0.02 | 596 | - | 642 | 4.91 |
| 168 | gpu: prefix + refloor + beam + composite + raycast + pathtrace | 0.49 | 280.7 | 9.30 | 3.13 | 1.04 | 31.7 | 8.00 | 603 | - | 585 | 4.71 |
| 169 | gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.49 | 280.6 | 15.80 | 2.35 | 1.73 | 52.0 | 0.01 | 676 | - | 702 | 5.46 |
| 170 | gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.49 | 280.3 | 26.77 | 1.29 | 2.32 | 50.0 | 0.02 | 653 | - | 702 | 5.37 |
| 171 | gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.49 | 280.3 | 35.81 | 0.61 | 2.29 | 50.0 | 0.02 | 868 | - | 636 | 5.97 |
| 172 | gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.49 | 280.1 | 36.10 | 0.61 | 2.29 | 50.0 | 0.02 | 860 | - | 636 | 5.93 |
| 173 | gpu: greedy@cpu + faces + rows + refloor + depth + raster + direct | 0.49 | 280.0 | 30.37 | 1.12 | 2.30 | 50.0 | 0.02 | 628 | - | 636 | 5.01 |
| 174 | gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.49 | 279.9 | 26.09 | 1.42 | 2.69 | 50.0 | 0.02 | 168 | - | 699 | 3.44 |
| 175 | gpu: tree64 + incremental + beam + composite + raycast + direct | 0.49 | 279.7 | 7.85 | 3.21 | 1.39 | 57.7 | 0.01 | 94 | - | 681 | 3.07 |
| 176 | gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.49 | 279.2 | 15.63 | 2.36 | 1.72 | 51.9 | 0.01 | 668 | - | 702 | 5.43 |
| 177 | gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + direct | 0.49 | 278.9 | 26.92 | 1.41 | 2.31 | 50.0 | 0.02 | 637 | - | 702 | 5.31 |
| 178 | gpu: distance + rowrun + quadtree + visbuffer + direct | 0.49 | 278.7 | 12.92 | 2.61 | 0.99 | 50.8 | 0.02 | 977 | - | 613 | 6.30 |
| 179 | gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.49 | 278.3 | 36.53 | 0.66 | 2.30 | 50.0 | 0.02 | 860 | - | 636 | 5.93 |
| 180 | gpu: svo_dag + refloor + quadtree + visbuffer + direct | 0.49 | 278.1 | 11.97 | 2.86 | 1.64 | 50.8 | 0.02 | 223 | - | 750 | 3.86 |
| 181 | gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.49 | 277.1 | 36.39 | 0.63 | 2.31 | 50.0 | 0.02 | 868 | - | 637 | 5.97 |
| 182 | gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.49 | 276.7 | 36.87 | 0.74 | 2.30 | 50.0 | 0.02 | 860 | - | 636 | 5.93 |
| 183 | gpu: svo_dag + refloor + quadtree + gbuffer + direct | 0.49 | 276.3 | 14.22 | 2.87 | 1.67 | 50.8 | 0.02 | 231 | - | 751 | 3.89 |
| 184 | gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.49 | 276.2 | 36.33 | 0.76 | 2.29 | 50.0 | 0.02 | 868 | - | 636 | 5.96 |
| 185 | gpu: distance + rowrun + quadtree + gbuffer + direct | 0.48 | 275.6 | 13.77 | 2.62 | 1.00 | 50.8 | 0.02 | 985 | - | 613 | 6.33 |
| 186 | gpu: bricks + refloor + composite + raycast + pathtrace | 0.48 | 275.6 | 7.50 | 3.35 | 1.09 | 31.7 | 8.00 | 563 | - | 584 | 4.55 |
| 187 | gpu: distance + sphere + beam + composite + raycast + pathtrace | 0.48 | 273.8 | 14.39 | 2.66 | 1.03 | 31.7 | 8.00 | 819 | - | 585 | 5.56 |
| 188 | gpu: clipmap + incremental + beam + composite + raycast + direct | 0.48 | 273.5 | 8.55 | 3.33 | 1.11 | 57.7 | 0.01 | 571 | - | 638 | 4.80 |
| 189 | gpu: rows + incremental + composite + raycast + direct | 0.48 | 272.5 | 10.31 | 3.12 | 1.02 | 57.7 | 0.01 | 563 | - | 584 | 4.55 |
| 190 | gpu: naive@cpu + faces + clipmap + refloor + depth + raster + direct | 0.48 | 271.8 | 15.55 | 2.45 | 1.70 | 52.0 | 0.01 | 701 | - | 768 | 5.82 |
| 191 | gpu: hier + incremental + beam + composite + raycast + direct | 0.48 | 271.7 | 9.48 | 3.25 | 1.06 | 57.7 | 0.01 | 531 | - | 586 | 4.43 |
| 192 | gpu: hier + refloor + beam + composite + raycast + pathtrace | 0.48 | 270.8 | 10.22 | 3.25 | 1.02 | 31.7 | 8.00 | 563 | - | 586 | 4.56 |
| 193 | gpu: bricks + rowrun + quadtree + gbuffer + direct | 0.48 | 270.6 | 8.52 | 3.32 | 1.06 | 50.8 | 0.02 | 729 | - | 612 | 5.32 |
| 194 | gpu: naive@cpu + faces + distance + sphere + depth + raster + direct | 0.47 | 270.1 | 21.74 | 1.88 | 1.64 | 52.0 | 0.01 | 916 | - | 702 | 6.42 |
| 195 | gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + direct | 0.47 | 268.6 | 26.15 | 1.61 | 2.72 | 50.0 | 0.02 | 160 | - | 687 | 3.36 |
| 196 | gpu: naive@cpu + faces + distance + refloor + depth + raster + direct | 0.47 | 268.5 | 21.25 | 1.89 | 1.63 | 52.0 | 0.01 | 916 | - | 702 | 6.42 |
| 197 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.47 | 268.0 | 32.87 | 1.29 | 2.96 | 50.0 | 0.02 | 106 | - | 742 | 3.36 |
| 198 | gpu: bricks + rowrun + quadtree + visbuffer + direct | 0.47 | 267.5 | 8.42 | 3.38 | 1.10 | 50.8 | 0.02 | 721 | - | 613 | 5.29 |
| 199 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.47 | 266.2 | 36.44 | 1.29 | 3.01 | 50.0 | 0.02 | 114 | - | 740 | 3.39 |
| 200 | gpu: distance + refloor + composite + raycast + pathtrace | 0.47 | 265.5 | 14.16 | 2.88 | 1.00 | 31.7 | 8.00 | 819 | - | 584 | 5.56 |
| 201 | gpu: greedy@cpu + faces + rows + rowrun + depth + raster + direct | 0.47 | 265.3 | 30.50 | 1.37 | 2.33 | 50.0 | 0.02 | 628 | - | 636 | 5.01 |
| 202 | gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.46 | 263.3 | 17.25 | 2.45 | 1.71 | 52.0 | 0.01 | 716 | - | 701 | 5.62 |
| 203 | gpu: greedy@cpu + faces + distance + refloor + depth + raster + direct | 0.46 | 262.3 | 35.32 | 1.04 | 2.31 | 50.0 | 0.02 | 852 | - | 636 | 5.90 |
| 204 | gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.46 | 261.9 | 24.87 | 2.31 | 2.36 | 52.0 | 0.01 | 178 | - | 804 | 3.89 |
| 205 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.46 | 261.7 | 26.00 | 1.77 | 2.73 | 50.0 | 0.02 | 176 | - | 699 | 3.47 |
| 206 | gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.46 | 261.6 | 25.29 | 2.32 | 2.34 | 51.9 | 0.01 | 170 | - | 804 | 3.86 |
| 207 | gpu: naive@cpu + faces + rows + rowrun + depth + raster + direct | 0.46 | 261.5 | 18.04 | 2.37 | 1.69 | 52.0 | 0.01 | 692 | - | 702 | 5.53 |
| 208 | gpu: greedy@cpu + faces + distance + sphere + depth + raster + direct | 0.46 | 261.4 | 36.42 | 1.03 | 2.33 | 50.0 | 0.02 | 852 | - | 636 | 5.90 |
| 209 | gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.46 | 261.0 | 17.32 | 2.48 | 1.71 | 51.9 | 0.01 | 708 | - | 702 | 5.59 |
| 210 | gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.46 | 260.9 | 17.07 | 2.44 | 1.71 | 51.9 | 0.01 | 668 | - | 708 | 5.46 |
| 211 | gpu: prefix + incremental + beam + composite + raycast + direct | 0.46 | 260.8 | 9.79 | 3.46 | 1.06 | 57.7 | 0.01 | 571 | - | 584 | 4.58 |
| 212 | gpu: bricks + refloor + beam + composite + raycast + pathtrace | 0.46 | 260.3 | 7.95 | 3.59 | 1.11 | 31.7 | 8.00 | 563 | - | 584 | 4.55 |
| 213 | gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.46 | 260.1 | 17.35 | 2.45 | 1.72 | 52.0 | 0.01 | 676 | - | 707 | 5.49 |
| 214 | gpu: svo_dag + sphere + composite + raycast + direct | 0.46 | 259.9 | 11.69 | 3.22 | 1.69 | 57.7 | 0.01 | 33 | - | 720 | 2.99 |
| 215 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.46 | 259.4 | 31.89 | 1.47 | 2.98 | 50.0 | 0.02 | 98 | - | 741 | 3.33 |
| 216 | gpu: rows + refloor + composite + raycast + pathtrace | 0.46 | 259.1 | 10.71 | 3.31 | 1.10 | 31.7 | 8.00 | 595 | - | 585 | 4.68 |
| 217 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.46 | 258.8 | 31.76 | 1.50 | 2.97 | 50.0 | 0.02 | 106 | - | 743 | 3.37 |
| 218 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.45 | 258.3 | 26.40 | 1.78 | 2.73 | 50.0 | 0.02 | 168 | - | 700 | 3.44 |
| 219 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.45 | 256.2 | 32.15 | 1.52 | 2.99 | 50.0 | 0.02 | 114 | - | 741 | 3.39 |
| 220 | gpu: clipmap + rowrun + quadtree + gbuffer + direct | 0.45 | 256.2 | 8.85 | 3.55 | 1.07 | 50.8 | 0.02 | 769 | - | 663 | 5.68 |
| 221 | gpu: clipmap + rowrun + quadtree + visbuffer + direct | 0.45 | 256.0 | 8.94 | 3.54 | 1.09 | 50.8 | 0.02 | 761 | - | 663 | 5.65 |
| 222 | gpu: bricks + sphere + quadtree + visbuffer + direct | 0.45 | 254.8 | 8.43 | 3.59 | 1.10 | 50.8 | 0.02 | 721 | - | 617 | 5.30 |
| 223 | gpu: bricks + sphere + quadtree + gbuffer + direct | 0.45 | 254.7 | 8.35 | 3.57 | 1.10 | 50.8 | 0.02 | 729 | - | 617 | 5.33 |
| 224 | gpu: greedy@cpu + faces + bricks + sphere + depth + raster + direct | 0.45 | 254.4 | 24.70 | 1.88 | 2.39 | 50.0 | 0.02 | 596 | - | 635 | 4.88 |
| 225 | gpu: rows + incremental + beam + composite + raycast + direct | 0.45 | 253.5 | 10.44 | 3.44 | 1.09 | 57.7 | 0.01 | 563 | - | 584 | 4.55 |
| 226 | gpu: greedy@cpu + faces + bricks + incremental + depth + raster + direct | 0.45 | 253.3 | 24.61 | 1.94 | 2.39 | 50.0 | 0.02 | 596 | - | 635 | 4.88 |
| 227 | gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.44 | 252.8 | 17.23 | 2.55 | 1.72 | 52.0 | 0.01 | 708 | - | 698 | 5.58 |
| 228 | gpu: hier + sphere + quadtree + gbuffer + direct | 0.44 | 252.7 | 10.48 | 3.41 | 1.13 | 50.8 | 0.02 | 729 | - | 618 | 5.34 |
| 229 | gpu: hier + sphere + quadtree + visbuffer + direct | 0.44 | 252.5 | 9.89 | 3.42 | 1.13 | 50.8 | 0.02 | 721 | - | 622 | 5.33 |
| 230 | gpu: greedy@cpu + faces + rows + incremental + depth + raster + direct | 0.44 | 251.6 | 26.70 | 1.66 | 2.34 | 50.0 | 0.02 | 628 | - | 636 | 5.01 |
| 231 | gpu: clipmap + sphere + composite + raycast + direct | 0.44 | 251.3 | 8.74 | 3.71 | 1.04 | 57.7 | 0.01 | 571 | - | 638 | 4.79 |
| 232 | gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.44 | 251.2 | 24.37 | 1.98 | 2.38 | 50.0 | 0.02 | 604 | - | 637 | 4.92 |
| 233 | gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.44 | 251.0 | 17.34 | 2.56 | 1.75 | 51.9 | 0.01 | 700 | - | 701 | 5.56 |
| 234 | gpu: naive@cpu + faces + tree64 + refloor + depth + raster + direct | 0.44 | 250.7 | 16.23 | 2.81 | 2.09 | 52.0 | 0.01 | 224 | - | 712 | 3.71 |
| 235 | gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.44 | 250.3 | 24.73 | 1.97 | 2.42 | 50.0 | 0.02 | 612 | - | 636 | 4.95 |
| 236 | gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.44 | 248.6 | 18.94 | 2.56 | 2.38 | 52.0 | 0.01 | 162 | - | 803 | 3.83 |
| 237 | gpu: clipmap + incremental + quadtree + visbuffer + direct | 0.44 | 247.6 | 8.96 | 3.70 | 1.08 | 50.8 | 0.02 | 761 | - | 667 | 5.66 |
| 238 | gpu: clipmap + incremental + quadtree + gbuffer + direct | 0.43 | 246.9 | 8.87 | 3.71 | 1.11 | 50.8 | 0.02 | 769 | - | 662 | 5.68 |
| 239 | gpu: greedy@cpu + faces + hier + sphere + depth + raster + direct | 0.43 | 246.5 | 26.39 | 1.80 | 2.38 | 50.0 | 0.02 | 596 | - | 641 | 4.91 |
| 240 | gpu: distance + refloor + beam + composite + raycast + pathtrace | 0.43 | 246.4 | 14.09 | 3.19 | 1.05 | 31.7 | 8.00 | 819 | - | 636 | 5.77 |
| 241 | gpu: prefix + refloor + quadtree + raster + direct | 0.43 | 245.9 | 9.58 | 3.59 | 1.10 | 50.8 | 0.01 | 753 | - | 657 | 5.59 |
| 242 | gpu: greedy@cpu + faces + hier + incremental + depth + raster + direct | 0.43 | 244.6 | 25.73 | 1.86 | 2.38 | 50.0 | 0.02 | 596 | - | 642 | 4.91 |
| 243 | gpu: rows + refloor + beam + composite + raycast + pathtrace | 0.43 | 243.1 | 10.68 | 3.61 | 1.08 | 31.7 | 8.00 | 595 | - | 636 | 4.88 |
| 244 | gpu: prefix + sphere + quadtree + visbuffer + direct | 0.43 | 242.4 | 9.83 | 3.67 | 1.10 | 50.8 | 0.02 | 761 | - | 659 | 5.63 |
| 245 | gpu: greedy@cpu + faces + rows + sphere + depth + raster + direct | 0.42 | 241.3 | 27.26 | 1.86 | 2.38 | 50.0 | 0.02 | 628 | - | 636 | 5.01 |
| 246 | gpu: bricks + refloor + quadtree + raster + direct | 0.42 | 241.2 | 8.91 | 3.85 | 1.13 | 50.8 | 0.01 | 713 | - | 660 | 5.44 |
| 247 | gpu: prefix + sphere + quadtree + gbuffer + direct | 0.42 | 241.1 | 10.06 | 3.67 | 1.13 | 50.8 | 0.02 | 769 | - | 658 | 5.66 |
| 248 | gpu: naive@cpu + faces + rows + incremental + depth + raster + direct | 0.42 | 240.7 | 17.37 | 2.79 | 1.77 | 52.0 | 0.01 | 692 | - | 702 | 5.53 |
| 249 | gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + direct | 0.42 | 240.2 | 25.68 | 1.99 | 2.39 | 50.0 | 0.02 | 636 | - | 636 | 5.05 |
| 250 | gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + direct | 0.42 | 239.4 | 25.25 | 2.20 | 2.40 | 50.0 | 0.02 | 637 | - | 702 | 5.31 |
| 251 | gpu: greedy@cpu + faces + prefix + sphere + depth + raster + direct | 0.42 | 239.3 | 26.18 | 1.98 | 2.40 | 50.0 | 0.02 | 636 | - | 636 | 5.04 |
| 252 | gpu: naive@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.42 | 239.0 | 17.34 | 2.88 | 1.81 | 31.7 | 8.01 | 740 | - | 762 | 5.95 |
| 253 | gpu: rows + sphere + quadtree + visbuffer + direct | 0.42 | 238.9 | 10.93 | 3.63 | 1.12 | 50.8 | 0.02 | 753 | - | 659 | 5.60 |
| 254 | gpu: svo_dag + rowrun + composite + raycast + direct | 0.42 | 238.4 | 11.95 | 3.63 | 1.72 | 57.8 | 0.01 | 33 | - | 720 | 2.99 |
| 255 | gpu: hier + rowrun + quadtree + gbuffer + direct | 0.42 | 238.1 | 10.09 | 3.73 | 1.10 | 50.8 | 0.02 | 729 | - | 661 | 5.51 |
| 256 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.42 | 237.5 | 15.82 | 3.12 | 1.81 | 31.7 | 8.01 | 700 | - | 734 | 5.69 |
| 257 | gpu: hier + refloor + quadtree + raster + direct | 0.42 | 237.2 | 10.12 | 3.79 | 1.10 | 50.8 | 0.01 | 713 | - | 655 | 5.43 |
| 258 | gpu: rows + sphere + quadtree + gbuffer + direct | 0.42 | 236.9 | 10.77 | 3.64 | 1.14 | 50.8 | 0.02 | 761 | - | 658 | 5.63 |
| 259 | gpu: hier + rowrun + quadtree + visbuffer + direct | 0.42 | 236.8 | 10.32 | 3.73 | 1.10 | 50.8 | 0.02 | 721 | - | 665 | 5.50 |
| 260 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.41 | 235.7 | 33.12 | 1.94 | 3.04 | 50.0 | 0.02 | 114 | - | 742 | 3.40 |
| 261 | gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.41 | 235.6 | 25.52 | 2.08 | 2.42 | 50.0 | 0.02 | 644 | - | 674 | 5.23 |
| 262 | gpu: naive@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.41 | 235.2 | 17.54 | 2.95 | 1.79 | 31.7 | 8.01 | 748 | - | 762 | 5.98 |
| 263 | gpu: naive@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.41 | 235.1 | 17.67 | 2.92 | 1.79 | 31.7 | 8.01 | 700 | - | 763 | 5.80 |
| 264 | gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.41 | 235.1 | 25.63 | 2.08 | 2.42 | 50.0 | 0.02 | 652 | - | 666 | 5.23 |
| 265 | gpu: naive@cpu + faces + bricks + sphere + depth + raster + direct | 0.41 | 235.0 | 16.60 | 3.11 | 1.81 | 52.0 | 0.01 | 660 | - | 701 | 5.40 |
| 266 | gpu: naive@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.41 | 235.0 | 20.21 | 2.54 | 1.75 | 31.7 | 8.01 | 964 | - | 760 | 6.84 |
| 267 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.41 | 235.0 | 33.42 | 1.94 | 3.05 | 50.0 | 0.02 | 106 | - | 741 | 3.36 |
| 268 | gpu: naive@cpu + faces + bricks + incremental + depth + raster + direct | 0.41 | 234.6 | 15.09 | 3.21 | 1.81 | 52.0 | 0.01 | 660 | - | 701 | 5.40 |
| 269 | gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.41 | 234.3 | 25.88 | 2.05 | 2.44 | 50.0 | 0.02 | 604 | - | 668 | 5.04 |
| 270 | gpu: naive@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.41 | 234.3 | 20.74 | 2.53 | 1.77 | 31.7 | 8.01 | 956 | - | 761 | 6.81 |
| 271 | gpu: distance + sphere + quadtree + raster + direct | 0.41 | 233.6 | 14.12 | 3.42 | 1.10 | 50.8 | 0.01 | 969 | - | 657 | 6.45 |
| 272 | gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.41 | 233.1 | 25.72 | 2.08 | 2.44 | 50.0 | 0.02 | 612 | - | 699 | 5.20 |
| 273 | gpu: naive@cpu + faces + hier + sphere + depth + raster + direct | 0.41 | 233.0 | 16.72 | 3.00 | 1.78 | 52.0 | 0.01 | 660 | - | 708 | 5.42 |
| 274 | gpu: greedy@cpu + faces + prefix + incremental + depth + raster + direct | 0.41 | 232.7 | 26.15 | 2.15 | 2.43 | 50.0 | 0.02 | 636 | - | 636 | 5.05 |
| 275 | gpu: naive@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.41 | 231.5 | 17.42 | 3.03 | 1.78 | 31.7 | 8.01 | 708 | - | 768 | 5.85 |
| 276 | gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + direct | 0.40 | 230.2 | 25.45 | 2.34 | 2.45 | 50.0 | 0.02 | 637 | - | 701 | 5.31 |
| 277 | gpu: distance + incremental + composite + raycast + direct | 0.40 | 230.0 | 14.57 | 3.55 | 1.06 | 57.7 | 0.01 | 787 | - | 644 | 5.67 |
| 278 | gpu: tree64 + incremental + quadtree + gbuffer + direct | 0.40 | 229.8 | 8.74 | 4.01 | 1.50 | 50.8 | 0.02 | 292 | - | 761 | 4.18 |
| 279 | gpu: distance + sphere + quadtree + visbuffer + pathtrace | 0.40 | 229.8 | 13.98 | 3.52 | 1.09 | 31.7 | 8.01 | 1009 | - | 658 | 6.61 |
| 280 | gpu: rows + refloor + quadtree + raster + direct | 0.40 | 229.8 | 10.91 | 3.85 | 1.11 | 50.8 | 0.01 | 745 | - | 658 | 5.56 |
| 281 | gpu: naive@cpu + faces + hier + incremental + depth + raster + direct | 0.40 | 228.8 | 16.90 | 3.11 | 1.77 | 52.0 | 0.01 | 660 | - | 708 | 5.42 |
| 282 | gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + direct | 0.40 | 228.2 | 25.36 | 2.43 | 2.84 | 50.0 | 0.02 | 160 | - | 686 | 3.35 |
| 283 | gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.40 | 227.7 | 25.96 | 2.18 | 2.44 | 50.0 | 0.02 | 636 | - | 694 | 5.27 |
| 284 | gpu: tree64 + incremental + quadtree + visbuffer + direct | 0.40 | 227.5 | 8.78 | 4.05 | 1.50 | 50.8 | 0.02 | 284 | - | 762 | 4.15 |
| 285 | gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.40 | 227.4 | 26.31 | 2.17 | 2.43 | 50.0 | 0.02 | 644 | - | 693 | 5.30 |
| 286 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.40 | 227.3 | 16.02 | 3.32 | 1.83 | 31.7 | 8.01 | 708 | - | 764 | 5.84 |
| 287 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + direct | 0.40 | 227.2 | 9.05 | 4.07 | 1.16 | 50.9 | 0.02 | 713 | - | 661 | 5.45 |
| 288 | gpu: distance + sphere + quadtree + gbuffer + pathtrace | 0.40 | 226.1 | 13.87 | 3.58 | 1.06 | 31.7 | 8.01 | 1016 | - | 659 | 6.64 |
| 289 | gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 0.40 | 225.4 | 10.00 | 4.01 | 1.16 | 31.7 | 8.01 | 793 | - | 664 | 5.78 |
| 290 | gpu: distance + refloor + quadtree + raster + direct | 0.40 | 225.2 | 14.07 | 3.65 | 1.08 | 50.8 | 0.01 | 969 | - | 671 | 6.50 |
| 291 | gpu: naive@cpu + faces + prefix + sphere + depth + raster + direct | 0.39 | 224.6 | 16.72 | 3.22 | 1.82 | 52.0 | 0.01 | 700 | - | 705 | 5.57 |
| 292 | gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 0.39 | 223.5 | 9.88 | 4.06 | 1.12 | 31.7 | 8.01 | 800 | - | 660 | 5.79 |
| 293 | gpu: naive@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.39 | 223.1 | 18.20 | 3.13 | 1.81 | 31.7 | 8.01 | 732 | - | 758 | 5.91 |
| 294 | gpu: naive@cpu + faces + rows + sphere + depth + raster + direct | 0.39 | 222.6 | 17.13 | 3.17 | 1.80 | 52.0 | 0.01 | 692 | - | 702 | 5.53 |
| 295 | gpu: hier + refloor + quadtree + visbuffer + pathtrace | 0.39 | 221.6 | 10.55 | 4.07 | 1.17 | 31.7 | 8.01 | 753 | - | 664 | 5.62 |
| 296 | gpu: bricks + incremental + quadtree + visbuffer + direct | 0.39 | 221.4 | 9.23 | 4.24 | 1.14 | 50.8 | 0.02 | 721 | - | 673 | 5.53 |
| 297 | gpu: naive@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.39 | 221.4 | 20.38 | 2.86 | 1.77 | 31.7 | 8.01 | 956 | - | 762 | 6.81 |
| 298 | gpu: naive@cpu + faces + clipmap + incremental + depth + raster + direct | 0.39 | 221.1 | 16.24 | 3.48 | 1.82 | 52.0 | 0.01 | 701 | - | 767 | 5.82 |
| 299 | gpu: greedy@cpu + faces + distance + incremental + depth + raster + direct | 0.39 | 221.1 | 29.02 | 2.00 | 2.38 | 50.0 | 0.02 | 852 | - | 636 | 5.90 |
| 300 | gpu: bricks + incremental + quadtree + gbuffer + direct | 0.39 | 220.6 | 9.31 | 4.26 | 1.15 | 50.8 | 0.02 | 729 | - | 676 | 5.57 |
| 301 | gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.39 | 220.3 | 19.75 | 2.93 | 1.76 | 52.0 | 0.01 | 932 | - | 759 | 6.70 |
| 302 | gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.39 | 219.8 | 19.93 | 2.92 | 1.77 | 51.9 | 0.01 | 924 | - | 760 | 6.68 |
| 303 | gpu: naive@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.39 | 219.6 | 17.53 | 3.22 | 1.81 | 31.7 | 8.01 | 740 | - | 766 | 5.97 |
| 304 | gpu: greedy@cpu + faces + distance + rowrun + depth + raster + direct | 0.39 | 219.5 | 29.74 | 2.01 | 2.43 | 50.0 | 0.02 | 852 | - | 637 | 5.90 |
| 305 | gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 0.38 | 217.9 | 9.62 | 4.32 | 1.12 | 31.7 | 8.01 | 753 | - | 680 | 5.68 |
| 306 | gpu: hier + refloor + quadtree + gbuffer + pathtrace | 0.38 | 217.9 | 10.54 | 4.16 | 1.13 | 31.7 | 8.01 | 760 | - | 665 | 5.65 |
| 307 | gpu: tree64 + sphere + composite + raycast + direct | 0.38 | 216.9 | 9.22 | 4.35 | 1.44 | 57.7 | 0.01 | 94 | - | 739 | 3.30 |
| 308 | gpu: distance + incremental + beam + composite + raycast + direct | 0.38 | 216.7 | 14.04 | 3.91 | 1.09 | 57.7 | 0.01 | 787 | - | 636 | 5.64 |
| 309 | gpu: greedy@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.38 | 216.5 | 26.15 | 2.49 | 2.51 | 31.7 | 8.01 | 676 | - | 697 | 5.44 |
| 310 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.38 | 216.1 | 24.72 | 2.73 | 2.49 | 50.0 | 0.02 | 637 | - | 702 | 5.31 |
| 311 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.38 | 215.6 | 24.88 | 2.73 | 2.54 | 31.7 | 8.01 | 636 | - | 699 | 5.29 |
| 312 | gpu: naive@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.38 | 215.5 | 20.37 | 2.99 | 1.77 | 31.7 | 8.01 | 964 | - | 761 | 6.84 |
| 313 | gpu: naive@cpu + faces + prefix + rowrun + depth + raster + direct | 0.38 | 215.1 | 16.76 | 3.42 | 1.84 | 52.0 | 0.01 | 700 | - | 706 | 5.57 |
| 314 | gpu: greedy@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.38 | 214.1 | 26.36 | 2.55 | 2.53 | 31.7 | 8.01 | 684 | - | 697 | 5.47 |
| 315 | gpu: greedy@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.38 | 214.0 | 29.40 | 2.14 | 2.43 | 31.7 | 8.01 | 892 | - | 695 | 6.29 |
| 316 | gpu: hier + incremental + quadtree + gbuffer + direct | 0.38 | 213.9 | 10.51 | 4.27 | 1.13 | 50.8 | 0.02 | 729 | - | 668 | 5.54 |
| 317 | gpu: hier + incremental + quadtree + visbuffer + direct | 0.38 | 213.4 | 10.12 | 4.28 | 1.17 | 50.8 | 0.02 | 721 | - | 668 | 5.51 |
| 318 | gpu: greedy@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.38 | 213.3 | 26.55 | 2.54 | 2.54 | 31.7 | 8.01 | 636 | - | 701 | 5.30 |
| 319 | gpu: naive@cpu + faces + prefix + incremental + depth + raster + direct | 0.37 | 213.2 | 17.29 | 3.49 | 1.83 | 52.0 | 0.01 | 700 | - | 706 | 5.58 |
| 320 | gpu: tree64 + refloor + composite + raycast + pathtrace | 0.37 | 213.2 | 8.82 | 4.42 | 1.45 | 31.7 | 8.00 | 126 | - | 742 | 3.44 |
| 321 | gpu: naive@cpu + faces + clipmap + sphere + depth + raster + direct | 0.37 | 212.6 | 16.00 | 3.69 | 1.85 | 52.0 | 0.01 | 701 | - | 768 | 5.82 |
| 322 | gpu: greedy@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.37 | 212.6 | 29.66 | 2.15 | 2.44 | 31.7 | 8.01 | 900 | - | 695 | 6.32 |
| 323 | gpu: svo_dag + sphere + beam + composite + raycast + direct | 0.37 | 211.9 | 11.75 | 4.24 | 1.75 | 57.7 | 0.01 | 33 | - | 771 | 3.19 |
| 324 | gpu: clipmap + sphere + beam + composite + raycast + direct | 0.37 | 211.7 | 9.54 | 4.53 | 1.16 | 57.7 | 0.01 | 571 | - | 691 | 5.01 |
| 325 | gpu: prefix + rowrun + composite + raycast + pathtrace | 0.37 | 211.3 | 10.19 | 4.45 | 1.10 | 31.7 | 8.00 | 603 | - | 658 | 5.00 |
| 326 | gpu: prefix + incremental + quadtree + visbuffer + direct | 0.37 | 210.9 | 10.26 | 4.38 | 1.12 | 50.8 | 0.02 | 761 | - | 667 | 5.66 |
| 327 | gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + direct | 0.37 | 210.5 | 25.30 | 2.87 | 2.87 | 50.0 | 0.02 | 160 | - | 686 | 3.35 |
| 328 | gpu: clipmap + sphere + quadtree + visbuffer + direct | 0.37 | 210.2 | 9.33 | 4.49 | 1.15 | 50.8 | 0.02 | 761 | - | 713 | 5.85 |
| 329 | gpu: clipmap + sphere + quadtree + gbuffer + direct | 0.37 | 210.0 | 9.55 | 4.48 | 1.14 | 50.8 | 0.02 | 769 | - | 709 | 5.86 |
| 330 | gpu: rows + incremental + quadtree + gbuffer + direct | 0.37 | 209.8 | 11.22 | 4.33 | 1.15 | 50.8 | 0.02 | 761 | - | 662 | 5.64 |
| 331 | gpu: prefix + incremental + quadtree + gbuffer + direct | 0.37 | 209.8 | 10.35 | 4.38 | 1.18 | 50.8 | 0.02 | 769 | - | 709 | 5.86 |
| 332 | gpu: svo_dag + incremental + composite + raycast + direct | 0.37 | 209.7 | 11.29 | 4.29 | 1.70 | 57.7 | 0.01 | 33 | - | 781 | 3.23 |
| 333 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.37 | 209.7 | 33.72 | 2.55 | 3.17 | 50.0 | 0.02 | 98 | - | 741 | 3.33 |
| 334 | gpu: rows + incremental + quadtree + visbuffer + direct | 0.37 | 209.5 | 10.79 | 4.35 | 1.11 | 50.8 | 0.02 | 753 | - | 666 | 5.63 |
| 335 | gpu: distance + refloor + quadtree + visbuffer + pathtrace | 0.37 | 209.5 | 14.59 | 4.00 | 1.10 | 31.7 | 8.01 | 1009 | - | 658 | 6.61 |
| 336 | gpu: rows + refloor + quadtree + visbuffer + pathtrace | 0.37 | 209.5 | 10.96 | 4.34 | 1.09 | 31.7 | 8.01 | 785 | - | 667 | 5.76 |
| 337 | gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 0.37 | 209.4 | 9.24 | 4.50 | 1.20 | 31.7 | 8.01 | 760 | - | 693 | 5.76 |
| 338 | gpu: greedy@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.37 | 209.0 | 27.46 | 2.65 | 2.54 | 31.7 | 8.01 | 644 | - | 701 | 5.33 |
| 339 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.37 | 208.8 | 24.95 | 2.90 | 2.55 | 31.7 | 8.01 | 644 | - | 704 | 5.35 |
| 340 | gpu: svo_dag + refloor + composite + raycast + pathtrace | 0.36 | 206.7 | 11.70 | 4.32 | 1.78 | 31.7 | 8.00 | 65 | - | 782 | 3.36 |
| 341 | gpu: rows + refloor + quadtree + gbuffer + pathtrace | 0.36 | 204.7 | 11.23 | 4.45 | 1.16 | 31.7 | 8.01 | 792 | - | 664 | 5.78 |
| 342 | gpu: naive@cpu + faces + distance + incremental + depth + raster + direct | 0.36 | 204.6 | 19.48 | 3.29 | 1.81 | 52.0 | 0.01 | 916 | - | 706 | 6.43 |
| 343 | gpu: distance + refloor + quadtree + gbuffer + pathtrace | 0.36 | 204.5 | 14.77 | 4.13 | 1.12 | 31.7 | 8.01 | 1016 | - | 664 | 6.66 |
| 344 | gpu: greedy@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.36 | 203.9 | 26.67 | 2.75 | 2.53 | 31.7 | 8.01 | 668 | - | 700 | 5.42 |
| 345 | gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 0.36 | 202.2 | 10.00 | 4.68 | 1.09 | 31.7 | 8.00 | 603 | - | 659 | 5.00 |
| 346 | gpu: greedy@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.36 | 202.2 | 29.22 | 2.47 | 2.51 | 31.7 | 8.01 | 892 | - | 696 | 6.30 |
| 347 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 0.36 | 202.0 | 9.66 | 4.61 | 1.14 | 31.7 | 8.01 | 753 | - | 657 | 5.59 |
| 348 | gpu: rows + rowrun + quadtree + raster + direct | 0.35 | 201.5 | 11.33 | 4.55 | 1.13 | 50.8 | 0.01 | 745 | - | 660 | 5.57 |
| 349 | gpu: naive@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.35 | 201.2 | 15.66 | 3.95 | 2.19 | 31.7 | 8.01 | 271 | - | 780 | 4.17 |
| 350 | gpu: naive@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.35 | 201.1 | 15.90 | 3.95 | 2.17 | 31.7 | 8.01 | 263 | - | 775 | 4.12 |
| 351 | gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.35 | 200.6 | 28.65 | 2.54 | 2.49 | 50.0 | 0.02 | 860 | - | 694 | 6.16 |
| 352 | gpu: naive@cpu + faces + distance + rowrun + depth + raster + direct | 0.35 | 200.2 | 18.08 | 3.46 | 1.81 | 52.0 | 0.01 | 916 | - | 706 | 6.43 |
| 353 | gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.35 | 200.2 | 27.91 | 2.55 | 2.48 | 50.0 | 0.02 | 868 | - | 694 | 6.19 |
| 354 | gpu: greedy@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.35 | 200.1 | 26.78 | 2.84 | 2.54 | 31.7 | 8.01 | 676 | - | 700 | 5.45 |
| 355 | gpu: rows + rowrun + composite + raycast + pathtrace | 0.35 | 199.4 | 11.00 | 4.66 | 1.10 | 31.7 | 8.00 | 595 | - | 652 | 4.94 |
| 356 | gpu: naive@cpu + faces + tree64 + incremental + depth + raster + direct | 0.35 | 198.3 | 15.30 | 4.05 | 2.19 | 52.0 | 0.01 | 224 | - | 713 | 3.71 |
| 357 | gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + direct | 0.35 | 198.1 | 24.74 | 3.21 | 2.57 | 50.0 | 0.02 | 596 | - | 639 | 4.90 |
| 358 | gpu: greedy@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.35 | 197.9 | 29.33 | 2.61 | 2.49 | 31.7 | 8.01 | 900 | - | 696 | 6.33 |
| 359 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 0.35 | 196.9 | 9.98 | 4.75 | 1.19 | 31.7 | 8.01 | 760 | - | 661 | 5.64 |
| 360 | gpu: clipmap + refloor + quadtree + raster + direct | 0.34 | 194.6 | 9.54 | 4.89 | 1.21 | 50.8 | 0.01 | 754 | - | 707 | 5.79 |
| 361 | gpu: tree64 + refloor + quadtree + raster + direct | 0.34 | 193.6 | 9.45 | 4.88 | 1.53 | 50.8 | 0.01 | 276 | - | 761 | 4.11 |
| 362 | gpu: naive@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.34 | 193.0 | 17.80 | 4.07 | 1.85 | 31.7 | 8.01 | 748 | - | 770 | 6.02 |
| 363 | gpu: naive@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.34 | 192.8 | 16.69 | 4.09 | 1.83 | 31.7 | 8.01 | 740 | - | 771 | 5.99 |
| 364 | gpu: rows + rowrun + beam + composite + raycast + pathtrace | 0.34 | 190.9 | 11.14 | 4.91 | 1.13 | 31.7 | 8.00 | 595 | - | 655 | 4.95 |
| 365 | gpu: greedy@cpu + faces + hier + rowrun + depth + raster + direct | 0.34 | 190.6 | 26.28 | 3.24 | 2.58 | 50.0 | 0.02 | 596 | - | 641 | 4.91 |
| 366 | gpu: tree64 + rowrun + composite + raycast + direct | 0.33 | 190.1 | 9.89 | 5.05 | 1.49 | 57.8 | 0.01 | 94 | - | 742 | 3.31 |
| 367 | gpu: distance + rowrun + composite + raycast + pathtrace | 0.33 | 189.2 | 14.32 | 4.71 | 1.12 | 31.7 | 8.00 | 819 | - | 656 | 5.85 |
| 368 | gpu: svo_dag + sphere + quadtree + gbuffer + direct | 0.33 | 189.1 | 12.20 | 4.75 | 1.80 | 50.8 | 0.02 | 231 | - | 805 | 4.11 |
| 369 | gpu: svo_dag + rowrun + beam + composite + raycast + direct | 0.33 | 188.9 | 11.59 | 4.84 | 1.80 | 57.8 | 0.01 | 33 | - | 786 | 3.25 |
| 370 | gpu: svo_dag + sphere + quadtree + visbuffer + direct | 0.33 | 188.6 | 12.44 | 4.75 | 1.82 | 50.8 | 0.02 | 223 | - | 806 | 4.08 |
| 371 | gpu: greedy@cpu + rayquery + raycast + direct | 0.33 | 187.5 | 48.13 | 0.86 | 3.86 | 57.7 | 0.01 | 358 | - | 1371 | 6.85 |
| 372 | gpu: naive@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.33 | 187.0 | 16.75 | 4.22 | 1.86 | 31.7 | 8.01 | 732 | - | 769 | 5.95 |
| 373 | gpu: naive@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.33 | 186.8 | 20.82 | 4.02 | 2.52 | 31.7 | 8.01 | 202 | - | 851 | 4.18 |
| 374 | gpu: naive@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.33 | 186.6 | 17.38 | 4.20 | 1.87 | 31.7 | 8.01 | 740 | - | 767 | 5.98 |
| 375 | gpu: clipmap + refloor + composite + raycast + pathtrace | 0.33 | 186.2 | 9.86 | 5.16 | 1.16 | 31.7 | 8.00 | 603 | - | 701 | 5.17 |
| 376 | gpu: naive@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.33 | 185.5 | 14.34 | 4.49 | 1.85 | 31.7 | 8.01 | 740 | - | 812 | 6.16 |
| 377 | gpu: naive@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.33 | 185.1 | 14.42 | 4.51 | 1.86 | 31.7 | 8.01 | 748 | - | 816 | 6.20 |
| 378 | gpu: boxes@cpu + rayquery + raycast + pathtrace | 0.33 | 185.1 | 12.03 | 4.94 | 1.71 | 31.7 | 8.00 | 601 | - | 1014 | 6.40 |
| 379 | gpu: naive@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.33 | 184.9 | 19.64 | 4.09 | 2.56 | 31.7 | 8.01 | 210 | - | 855 | 4.22 |
| 380 | gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.32 | 184.7 | 14.09 | 4.54 | 1.86 | 52.0 | 0.01 | 701 | - | 768 | 5.82 |
| 381 | gpu: greedy@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.32 | 184.7 | 25.43 | 3.60 | 2.95 | 31.7 | 8.01 | 207 | - | 747 | 3.78 |
| 382 | gpu: distance + rowrun + beam + composite + raycast + pathtrace | 0.32 | 184.6 | 14.61 | 4.87 | 1.09 | 31.7 | 8.00 | 819 | - | 659 | 5.86 |
| 383 | gpu: greedy@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.32 | 184.4 | 25.40 | 3.60 | 2.95 | 31.7 | 8.01 | 199 | - | 747 | 3.75 |
| 384 | gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.32 | 184.1 | 19.56 | 4.16 | 2.52 | 52.0 | 0.01 | 162 | - | 804 | 3.83 |
| 385 | gpu: tree64 + sphere + beam + composite + raycast + direct | 0.32 | 184.1 | 9.84 | 5.21 | 1.51 | 57.7 | 0.01 | 94 | - | 748 | 3.34 |
| 386 | gpu: naive@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.32 | 183.8 | 20.51 | 3.95 | 1.88 | 31.7 | 8.01 | 956 | - | 772 | 6.85 |
| 387 | gpu: tree64 + sphere + quadtree + visbuffer + direct | 0.32 | 183.3 | 10.11 | 5.18 | 1.54 | 50.8 | 0.02 | 284 | - | 768 | 4.17 |
| 388 | gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 0.32 | 183.1 | 10.10 | 5.22 | 1.48 | 31.7 | 8.00 | 126 | - | 746 | 3.46 |
| 389 | gpu: tree64 + sphere + quadtree + gbuffer + direct | 0.32 | 182.5 | 10.36 | 5.21 | 1.51 | 50.8 | 0.02 | 292 | - | 767 | 4.20 |
| 390 | gpu: greedy@cpu + rayquery + raycast + pathtrace | 0.32 | 182.0 | 48.97 | 1.44 | 3.87 | 31.7 | 8.00 | 389 | - | 1379 | 7.01 |
| 391 | gpu: svo_dag + refloor + quadtree + raster + direct | 0.32 | 180.1 | 11.97 | 5.04 | 1.81 | 50.8 | 0.01 | 215 | - | 819 | 4.10 |
| 392 | gpu: naive@cpu + faces + tree64 + sphere + depth + raster + direct | 0.32 | 180.1 | 15.82 | 4.61 | 2.22 | 52.0 | 0.01 | 224 | - | 712 | 3.71 |
| 393 | gpu: distance + incremental + quadtree + gbuffer + direct | 0.32 | 179.9 | 14.33 | 4.98 | 1.13 | 50.8 | 0.02 | 985 | - | 674 | 6.58 |
| 394 | gpu: distance + incremental + quadtree + visbuffer + direct | 0.32 | 179.4 | 14.38 | 4.99 | 1.13 | 50.8 | 0.02 | 977 | - | 675 | 6.55 |
| 395 | gpu: greedy@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.31 | 178.5 | 26.05 | 3.71 | 2.61 | 31.7 | 8.01 | 676 | - | 710 | 5.49 |
| 396 | gpu: greedy@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.31 | 177.9 | 25.84 | 3.70 | 2.63 | 31.7 | 8.01 | 684 | - | 710 | 5.53 |
| 397 | gpu: svo_dag + incremental + beam + composite + raycast + direct | 0.31 | 177.6 | 11.93 | 5.23 | 1.81 | 57.7 | 0.01 | 33 | - | 792 | 3.27 |
| 398 | gpu: naive@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.31 | 175.1 | 19.90 | 4.32 | 1.87 | 31.7 | 8.01 | 964 | - | 768 | 6.87 |
| 399 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.31 | 174.7 | 27.35 | 3.63 | 3.27 | 31.7 | 8.01 | 138 | - | 792 | 3.69 |
| 400 | gpu: greedy@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.30 | 173.4 | 25.53 | 3.83 | 2.59 | 31.7 | 8.01 | 668 | - | 703 | 5.44 |
| 401 | gpu: naive@cpu + rayquery + raycast + direct | 0.30 | 173.0 | 53.94 | 1.36 | 3.65 | 57.7 | 0.01 | 919 | - | 1790 | 10.74 |
| 402 | gpu: greedy@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.30 | 172.8 | 26.34 | 3.83 | 2.62 | 31.7 | 8.01 | 676 | - | 703 | 5.47 |
| 403 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.30 | 172.3 | 27.69 | 3.71 | 3.26 | 31.7 | 8.01 | 146 | - | 795 | 3.73 |
| 404 | gpu: greedy@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.30 | 171.9 | 23.67 | 4.11 | 2.62 | 31.7 | 8.01 | 676 | - | 751 | 5.66 |
| 405 | gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 0.30 | 171.5 | 10.85 | 5.56 | 1.55 | 31.7 | 8.01 | 316 | - | 801 | 4.43 |
| 406 | gpu: rows + incremental + quadtree + raster + direct | 0.30 | 171.1 | 11.14 | 5.55 | 1.16 | 50.8 | 0.01 | 745 | - | 694 | 5.70 |
| 407 | gpu: greedy@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.30 | 170.5 | 28.04 | 3.58 | 2.60 | 31.7 | 8.01 | 892 | - | 710 | 6.35 |
| 408 | gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 0.30 | 170.2 | 11.82 | 5.46 | 1.81 | 31.7 | 8.00 | 65 | - | 787 | 3.38 |
| 409 | gpu: greedy@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.30 | 170.0 | 23.16 | 4.13 | 2.63 | 31.7 | 8.01 | 684 | - | 750 | 5.69 |
| 410 | gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 0.30 | 169.1 | 10.40 | 5.65 | 1.54 | 31.7 | 8.01 | 324 | - | 805 | 4.47 |
| 411 | gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 0.30 | 168.8 | 10.82 | 5.64 | 1.18 | 31.7 | 8.01 | 793 | - | 667 | 5.79 |
| 412 | gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 0.30 | 168.7 | 11.04 | 5.64 | 1.19 | 31.7 | 8.01 | 800 | - | 668 | 5.82 |
| 413 | gpu: naive@cpu + rayquery + raycast + pathtrace | 0.30 | 168.4 | 53.37 | 1.99 | 3.65 | 31.7 | 8.00 | 951 | - | 1595 | 10.10 |
| 414 | gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 0.29 | 167.3 | 14.46 | 5.46 | 1.15 | 31.7 | 8.01 | 1009 | - | 704 | 6.79 |
| 415 | gpu: points@cpu + splat + bricks + refloor + direct | 0.29 | 166.6 | 48.00 | 2.16 | 0.94 | 57.1 | 0.01 | 801 | - | 879 | 6.66 |
| 416 | gpu: naive@cpu + faces + bricks + rowrun + depth + raster + direct | 0.29 | 166.4 | 14.99 | 5.25 | 1.92 | 52.0 | 0.01 | 660 | - | 705 | 5.41 |
| 417 | gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 0.29 | 166.0 | 11.32 | 5.72 | 1.18 | 31.7 | 8.01 | 785 | - | 706 | 5.91 |
| 418 | gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 0.29 | 165.4 | 11.93 | 5.73 | 1.18 | 31.7 | 8.01 | 792 | - | 711 | 5.96 |
| 419 | gpu: tree64 + rowrun + beam + composite + raycast + direct | 0.29 | 163.9 | 10.51 | 5.92 | 1.52 | 57.7 | 0.01 | 94 | - | 747 | 3.33 |
| 420 | gpu: points@cpu + splat + tree64 + refloor + direct | 0.29 | 163.7 | 43.63 | 2.29 | 1.30 | 57.1 | 0.01 | 365 | - | 986 | 5.36 |
| 421 | gpu: points@cpu + splat + prefix + refloor + direct | 0.29 | 163.3 | 50.31 | 2.14 | 0.90 | 57.1 | 0.01 | 841 | - | 892 | 6.87 |
| 422 | gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 0.29 | 163.1 | 11.10 | 5.97 | 1.18 | 31.7 | 8.00 | 603 | - | 706 | 5.19 |
| 423 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + direct | 0.29 | 162.4 | 11.71 | 5.77 | 1.62 | 51.9 | 0.01 | 676 | - | 729 | 5.57 |
| 424 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + direct | 0.29 | 162.3 | 11.75 | 5.78 | 1.62 | 51.9 | 0.02 | 668 | - | 730 | 5.54 |
| 425 | gpu: greedy@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.29 | 162.1 | 27.91 | 3.94 | 2.60 | 31.7 | 8.01 | 900 | - | 706 | 6.37 |
| 426 | gpu: hier + incremental + quadtree + raster + direct | 0.28 | 161.9 | 11.29 | 5.91 | 1.17 | 50.8 | 0.01 | 713 | - | 715 | 5.66 |
| 427 | gpu: points@cpu + splat + prefix + rowrun + direct | 0.28 | 161.2 | 51.15 | 2.23 | 0.95 | 57.1 | 0.01 | 841 | - | 892 | 6.87 |
| 428 | gpu: points@cpu + splat + clipmap + rowrun + direct | 0.28 | 161.1 | 48.67 | 2.45 | 0.96 | 57.1 | 0.01 | 842 | - | 943 | 7.08 |
| 429 | gpu: prefix + rowrun + quadtree + raster + direct | 0.28 | 161.0 | 11.28 | 5.98 | 1.17 | 50.8 | 0.01 | 753 | - | 694 | 5.74 |
| 430 | gpu: points@cpu + splat + bricks + rowrun + direct | 0.28 | 161.0 | 46.62 | 2.38 | 0.97 | 57.1 | 0.01 | 801 | - | 892 | 6.71 |
| 431 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.28 | 160.9 | 28.07 | 4.23 | 3.26 | 50.0 | 0.02 | 98 | - | 742 | 3.33 |
| 432 | gpu: points@cpu + splat + rows + refloor + direct | 0.28 | 160.9 | 50.02 | 2.16 | 0.93 | 57.1 | 0.01 | 833 | - | 892 | 6.84 |
| 433 | gpu: points@cpu + splat + rows + rowrun + direct | 0.28 | 160.8 | 48.95 | 2.22 | 0.94 | 57.1 | 0.01 | 833 | - | 893 | 6.84 |
| 434 | gpu: points@cpu + splat + clipmap + refloor + direct | 0.28 | 160.4 | 48.61 | 2.31 | 0.94 | 57.1 | 0.01 | 842 | - | 943 | 7.08 |
| 435 | gpu: points@cpu + splat + hier + refloor + direct | 0.28 | 159.9 | 51.14 | 2.17 | 0.93 | 57.1 | 0.01 | 801 | - | 894 | 6.72 |
| 436 | gpu: naive@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.28 | 159.4 | 13.47 | 5.57 | 1.92 | 31.7 | 8.01 | 700 | - | 779 | 5.86 |
| 437 | gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + direct | 0.28 | 159.4 | 11.64 | 5.75 | 1.59 | 51.9 | 0.01 | 716 | - | 730 | 5.73 |
| 438 | gpu: bricks + incremental + quadtree + raster + direct | 0.28 | 159.3 | 11.14 | 6.09 | 1.15 | 50.8 | 0.01 | 713 | - | 711 | 5.64 |
| 439 | gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + direct | 0.28 | 159.1 | 12.29 | 5.75 | 1.61 | 51.9 | 0.02 | 708 | - | 731 | 5.71 |
| 440 | gpu: naive@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.28 | 158.9 | 13.78 | 5.60 | 1.92 | 31.7 | 8.01 | 708 | - | 778 | 5.89 |
| 441 | gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + direct | 0.28 | 158.6 | 11.44 | 5.92 | 1.60 | 51.9 | 0.02 | 709 | - | 780 | 5.90 |
| 442 | gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 0.28 | 158.4 | 14.93 | 5.82 | 1.20 | 31.7 | 8.01 | 1016 | - | 704 | 6.82 |
| 443 | gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + direct | 0.28 | 157.9 | 11.68 | 5.93 | 1.61 | 51.9 | 0.01 | 717 | - | 778 | 5.93 |
| 444 | gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + direct | 0.28 | 157.9 | 11.94 | 5.78 | 1.63 | 51.9 | 0.01 | 676 | - | 730 | 5.58 |
| 445 | gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + direct | 0.28 | 157.8 | 12.31 | 5.79 | 1.62 | 51.9 | 0.02 | 668 | - | 731 | 5.55 |
| 446 | gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 0.28 | 157.8 | 12.90 | 5.86 | 1.88 | 31.7 | 8.01 | 254 | - | 842 | 4.35 |
| 447 | gpu: points@cpu + splat + clipmap + incremental + direct | 0.28 | 157.5 | 48.40 | 2.55 | 0.97 | 57.1 | 0.01 | 842 | - | 943 | 7.08 |
| 448 | gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + direct | 0.28 | 157.4 | 11.51 | 6.00 | 1.60 | 51.9 | 0.02 | 668 | - | 739 | 5.58 |
| 449 | gpu: naive@cpu + faces + hier + rowrun + depth + raster + direct | 0.28 | 157.4 | 16.21 | 5.42 | 1.90 | 52.0 | 0.01 | 660 | - | 708 | 5.42 |
| 450 | gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + direct | 0.28 | 157.1 | 14.39 | 5.78 | 1.62 | 51.9 | 0.02 | 700 | - | 730 | 5.67 |
| 451 | gpu: points@cpu + splat + tree64 + incremental + direct | 0.28 | 157.0 | 44.81 | 2.60 | 1.33 | 57.1 | 0.01 | 365 | - | 985 | 5.35 |
| 452 | gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + direct | 0.28 | 156.9 | 14.94 | 5.78 | 1.61 | 51.9 | 0.01 | 708 | - | 728 | 5.70 |
| 453 | gpu: svo_dag + incremental + quadtree + visbuffer + direct | 0.28 | 156.7 | 12.65 | 5.92 | 1.84 | 50.8 | 0.02 | 223 | - | 835 | 4.19 |
| 454 | gpu: points@cpu + splat + svo_dag + refloor + direct | 0.28 | 156.7 | 45.48 | 2.27 | 1.61 | 57.1 | 0.01 | 303 | - | 1005 | 5.19 |
| 455 | gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 0.28 | 156.6 | 13.03 | 5.91 | 1.84 | 31.7 | 8.01 | 262 | - | 836 | 4.36 |
| 456 | gpu: svo_dag + incremental + quadtree + gbuffer + direct | 0.28 | 156.6 | 12.69 | 5.94 | 1.86 | 50.8 | 0.02 | 231 | - | 835 | 4.23 |
| 457 | gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + direct | 0.28 | 156.5 | 12.23 | 6.00 | 1.64 | 51.9 | 0.01 | 676 | - | 740 | 5.61 |
| 458 | gpu: distance + rowrun + quadtree + raster + direct | 0.28 | 156.4 | 15.05 | 5.95 | 1.15 | 50.8 | 0.01 | 969 | - | 712 | 6.66 |
| 459 | gpu: points@cpu + splat + bricks + sphere + direct | 0.27 | 156.3 | 48.93 | 2.53 | 1.00 | 57.1 | 0.01 | 801 | - | 891 | 6.71 |
| 460 | gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 0.27 | 156.2 | 11.00 | 6.18 | 1.18 | 31.7 | 8.01 | 801 | - | 750 | 6.15 |
| 461 | gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 0.27 | 156.1 | 10.58 | 6.18 | 1.19 | 31.7 | 8.01 | 793 | - | 750 | 6.12 |
| 462 | gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + direct | 0.27 | 156.0 | 12.01 | 5.89 | 1.61 | 51.9 | 0.02 | 708 | - | 736 | 5.73 |
| 463 | gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + direct | 0.27 | 156.0 | 12.30 | 5.88 | 1.64 | 51.9 | 0.01 | 716 | - | 737 | 5.76 |
| 464 | gpu: points@cpu + splat + distance + refloor + direct | 0.27 | 155.1 | 55.13 | 2.13 | 0.89 | 57.1 | 0.01 | 1057 | - | 893 | 7.73 |
| 465 | gpu: points@cpu + splat + hier + sphere + direct | 0.27 | 154.9 | 48.81 | 2.46 | 0.97 | 57.1 | 0.01 | 801 | - | 894 | 6.72 |
| 466 | gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + direct | 0.27 | 154.8 | 12.22 | 5.92 | 1.99 | 51.9 | 0.02 | 232 | - | 749 | 3.89 |
| 467 | gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + direct | 0.27 | 154.8 | 13.59 | 5.88 | 1.64 | 51.9 | 0.02 | 700 | - | 731 | 5.67 |
| 468 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + direct | 0.27 | 154.7 | 11.56 | 6.08 | 1.61 | 51.9 | 0.02 | 709 | - | 788 | 5.94 |
| 469 | gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + direct | 0.27 | 154.7 | 14.47 | 5.87 | 1.62 | 51.9 | 0.01 | 708 | - | 732 | 5.71 |
| 470 | gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + direct | 0.27 | 154.6 | 12.07 | 5.92 | 1.98 | 51.9 | 0.01 | 240 | - | 748 | 3.92 |
| 471 | gpu: points@cpu + splat + clipmap + sphere + direct | 0.27 | 154.5 | 46.51 | 2.75 | 1.01 | 57.1 | 0.01 | 842 | - | 944 | 7.08 |
| 472 | gpu: points@cpu + splat + hier + rowrun + direct | 0.27 | 154.4 | 49.87 | 2.50 | 0.95 | 57.1 | 0.01 | 801 | - | 895 | 6.73 |
| 473 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + direct | 0.27 | 154.3 | 11.98 | 6.08 | 1.61 | 51.9 | 0.01 | 717 | - | 788 | 5.97 |
| 474 | gpu: points@cpu + splat + distance + sphere + direct | 0.27 | 154.1 | 54.76 | 2.12 | 0.91 | 57.1 | 0.01 | 1057 | - | 892 | 7.73 |
| 475 | gpu: points@cpu + splat + prefix + sphere + direct | 0.27 | 153.9 | 49.51 | 2.57 | 0.98 | 57.1 | 0.01 | 841 | - | 892 | 6.87 |
| 476 | gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + direct | 0.27 | 153.7 | 11.87 | 6.15 | 1.62 | 51.9 | 0.02 | 668 | - | 726 | 5.53 |
| 477 | gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + direct | 0.27 | 153.6 | 11.94 | 6.14 | 1.64 | 51.9 | 0.01 | 676 | - | 730 | 5.58 |
| 478 | gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + direct | 0.27 | 152.7 | 11.57 | 6.18 | 1.59 | 51.9 | 0.02 | 709 | - | 788 | 5.94 |
| 479 | gpu: points@cpu + splat + rows + sphere + direct | 0.27 | 152.6 | 50.69 | 2.50 | 0.94 | 57.1 | 0.01 | 833 | - | 892 | 6.84 |
| 480 | gpu: points@cpu + splat + distance + rowrun + direct | 0.27 | 152.6 | 54.70 | 2.26 | 0.93 | 57.1 | 0.01 | 1057 | - | 892 | 7.73 |
| 481 | gpu: greedy@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.27 | 152.5 | 27.81 | 4.47 | 2.61 | 31.7 | 8.01 | 884 | - | 642 | 6.05 |
| 482 | gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + direct | 0.27 | 152.3 | 11.84 | 6.18 | 1.61 | 51.9 | 0.01 | 717 | - | 787 | 5.96 |
| 483 | gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + direct | 0.27 | 151.4 | 16.89 | 5.73 | 1.62 | 51.9 | 0.02 | 924 | - | 731 | 6.56 |
| 484 | gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 0.27 | 151.3 | 10.11 | 6.45 | 1.20 | 31.7 | 8.00 | 563 | - | 674 | 4.90 |
| 485 | gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + direct | 0.27 | 151.3 | 16.98 | 5.72 | 1.61 | 51.9 | 0.01 | 932 | - | 733 | 6.60 |
| 486 | gpu: points@cpu + splat + tree64 + sphere + direct | 0.27 | 151.1 | 44.54 | 2.88 | 1.40 | 57.1 | 0.01 | 365 | - | 986 | 5.36 |
| 487 | gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + direct | 0.27 | 150.8 | 12.34 | 6.11 | 1.64 | 51.9 | 0.02 | 668 | - | 732 | 5.55 |
| 488 | gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + direct | 0.27 | 150.7 | 16.85 | 5.76 | 1.62 | 51.9 | 0.02 | 924 | - | 731 | 6.56 |
| 489 | gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + direct | 0.26 | 150.7 | 12.21 | 6.14 | 1.60 | 51.9 | 0.02 | 668 | - | 735 | 5.56 |
| 490 | gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + direct | 0.26 | 150.6 | 16.89 | 5.76 | 1.65 | 51.9 | 0.01 | 932 | - | 729 | 6.59 |
| 491 | gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + direct | 0.26 | 150.2 | 13.21 | 6.11 | 1.64 | 51.9 | 0.01 | 676 | - | 731 | 5.58 |
| 492 | gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + direct | 0.26 | 150.0 | 12.85 | 6.14 | 1.61 | 51.9 | 0.01 | 676 | - | 734 | 5.59 |
| 493 | gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + direct | 0.26 | 150.0 | 12.47 | 6.18 | 1.59 | 51.9 | 0.02 | 708 | - | 731 | 5.71 |
| 494 | gpu: greedy@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.26 | 149.6 | 22.16 | 5.18 | 2.67 | 31.7 | 8.01 | 636 | - | 713 | 5.35 |
| 495 | gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + direct | 0.26 | 149.6 | 13.07 | 6.18 | 1.64 | 51.9 | 0.01 | 716 | - | 731 | 5.74 |
| 496 | gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + direct | 0.26 | 149.4 | 12.40 | 6.15 | 1.62 | 51.9 | 0.02 | 700 | - | 731 | 5.68 |
| 497 | gpu: naive@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.26 | 149.1 | 15.84 | 5.87 | 2.25 | 31.7 | 8.01 | 271 | - | 776 | 4.15 |
| 498 | gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + direct | 0.26 | 149.0 | 12.74 | 6.13 | 1.65 | 51.9 | 0.01 | 708 | - | 730 | 5.70 |
| 499 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + direct | 0.26 | 148.8 | 15.89 | 5.89 | 2.27 | 51.9 | 0.02 | 170 | - | 822 | 3.94 |
| 500 | gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + direct | 0.26 | 148.7 | 11.39 | 6.38 | 1.62 | 51.9 | 0.02 | 709 | - | 784 | 5.92 |
| 501 | gpu: greedy@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.26 | 148.7 | 22.49 | 5.22 | 2.67 | 31.7 | 8.01 | 644 | - | 713 | 5.38 |
| 502 | gpu: naive@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.26 | 148.6 | 15.36 | 5.90 | 2.27 | 31.7 | 8.01 | 263 | - | 782 | 4.14 |
| 503 | gpu: hier + sphere + quadtree + raster + direct | 0.26 | 148.5 | 12.12 | 6.50 | 1.19 | 50.8 | 0.01 | 713 | - | 709 | 5.64 |
| 504 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + direct | 0.26 | 148.4 | 15.85 | 5.90 | 2.30 | 51.9 | 0.01 | 178 | - | 820 | 3.96 |
| 505 | gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + direct | 0.26 | 148.4 | 16.98 | 5.87 | 1.64 | 51.9 | 0.02 | 924 | - | 733 | 6.57 |
| 506 | gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + direct | 0.26 | 148.3 | 12.07 | 6.37 | 1.62 | 51.9 | 0.01 | 717 | - | 784 | 5.95 |
| 507 | gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + direct | 0.26 | 148.3 | 17.30 | 5.87 | 1.64 | 51.9 | 0.01 | 932 | - | 733 | 6.60 |
| 508 | gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + direct | 0.26 | 148.1 | 11.93 | 6.24 | 1.96 | 51.9 | 0.02 | 232 | - | 751 | 3.90 |
| 509 | gpu: prefix + incremental + quadtree + raster + direct | 0.26 | 147.7 | 12.52 | 6.54 | 1.22 | 50.8 | 0.01 | 753 | - | 706 | 5.79 |
| 510 | gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + direct | 0.26 | 147.6 | 12.24 | 6.25 | 1.99 | 51.9 | 0.01 | 240 | - | 753 | 3.94 |
| 511 | gpu: points@cpu + splat + svo_dag + sphere + direct | 0.26 | 147.1 | 45.70 | 2.77 | 1.70 | 57.1 | 0.01 | 303 | - | 1010 | 5.21 |
| 512 | gpu: naive@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.26 | 147.0 | 14.40 | 6.00 | 1.92 | 31.7 | 8.01 | 700 | - | 769 | 5.82 |
| 513 | gpu: naive@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.26 | 147.0 | 14.57 | 6.01 | 1.91 | 31.7 | 8.01 | 708 | - | 771 | 5.86 |
| 514 | gpu: svo_dag + rowrun + quadtree + visbuffer + direct | 0.26 | 146.9 | 12.66 | 6.39 | 1.87 | 50.8 | 0.02 | 223 | - | 844 | 4.23 |
| 515 | gpu: svo_dag + rowrun + quadtree + gbuffer + direct | 0.26 | 146.7 | 13.00 | 6.40 | 1.86 | 50.8 | 0.02 | 231 | - | 843 | 4.26 |
| 516 | gpu: bricks + rowrun + composite + raycast + pathtrace | 0.26 | 146.1 | 10.66 | 6.68 | 1.20 | 31.7 | 8.00 | 563 | - | 667 | 4.88 |
| 517 | gpu: greedy@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.26 | 146.0 | 23.03 | 5.24 | 2.67 | 31.7 | 8.01 | 668 | - | 642 | 5.19 |
| 518 | gpu: naive@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.26 | 145.8 | 14.03 | 6.23 | 1.89 | 31.7 | 8.01 | 748 | - | 827 | 6.25 |
| 519 | gpu: naive@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.26 | 145.6 | 13.93 | 6.24 | 1.87 | 31.7 | 8.01 | 740 | - | 828 | 6.22 |
| 520 | gpu: points@cpu + splat + tree64 + rowrun + direct | 0.26 | 145.3 | 43.66 | 3.26 | 1.42 | 57.1 | 0.01 | 365 | - | 986 | 5.35 |
| 521 | gpu: points@cpu + splat + svo_dag + rowrun + direct | 0.25 | 144.1 | 43.88 | 3.00 | 1.68 | 57.1 | 0.01 | 303 | - | 1010 | 5.21 |
| 522 | gpu: distance + incremental + quadtree + raster + direct | 0.25 | 143.6 | 14.49 | 6.58 | 1.14 | 50.8 | 0.01 | 969 | - | 709 | 6.65 |
| 523 | gpu: naive@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.25 | 142.9 | 13.73 | 6.39 | 1.88 | 31.7 | 8.01 | 740 | - | 829 | 6.22 |
| 524 | gpu: greedy@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.25 | 142.9 | 23.30 | 5.37 | 2.67 | 31.7 | 8.01 | 628 | - | 643 | 5.04 |
| 525 | gpu: naive@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.25 | 142.7 | 13.42 | 6.40 | 1.90 | 31.7 | 8.01 | 748 | - | 829 | 6.25 |
| 526 | gpu: greedy@cpu + rayquery + raycast + prefix + refloor + pathtrace | 0.25 | 142.4 | 52.33 | 3.12 | 4.06 | 31.7 | 8.00 | 942 | - | 1354 | 9.10 |
| 527 | gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + direct | 0.25 | 142.3 | 12.27 | 6.54 | 2.01 | 51.9 | 0.02 | 232 | - | 753 | 3.90 |
| 528 | gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + direct | 0.25 | 142.2 | 12.23 | 6.53 | 2.00 | 51.9 | 0.01 | 240 | - | 758 | 3.95 |
| 529 | gpu: greedy@cpu + rayquery + raycast + hier + refloor + pathtrace | 0.25 | 142.0 | 51.95 | 3.12 | 4.04 | 31.7 | 8.00 | 902 | - | 1365 | 8.99 |
| 530 | gpu: greedy@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.25 | 141.4 | 24.84 | 5.44 | 3.03 | 31.7 | 8.01 | 207 | - | 756 | 3.82 |
| 531 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.25 | 141.0 | 21.52 | 5.70 | 2.68 | 31.7 | 8.01 | 628 | - | 641 | 5.03 |
| 532 | gpu: greedy@cpu + rayquery + raycast + distance + sphere + pathtrace | 0.25 | 140.9 | 52.84 | 2.74 | 3.98 | 31.7 | 8.00 | 1158 | - | 1364 | 10.00 |
| 533 | gpu: greedy@cpu + rayquery + raycast + bricks + refloor + pathtrace | 0.25 | 140.7 | 53.43 | 3.40 | 4.00 | 31.7 | 8.00 | 902 | - | 1325 | 8.83 |
| 534 | gpu: greedy@cpu + rayquery + raycast + rows + refloor + pathtrace | 0.24 | 139.2 | 51.94 | 3.26 | 4.05 | 31.7 | 8.00 | 934 | - | 1388 | 9.21 |
| 535 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + direct | 0.24 | 139.1 | 16.05 | 6.40 | 2.32 | 51.9 | 0.02 | 170 | - | 822 | 3.94 |
| 536 | gpu: points@cpu + splat + bricks + incremental + direct | 0.24 | 139.0 | 48.96 | 3.46 | 1.07 | 57.1 | 0.01 | 801 | - | 932 | 6.87 |
| 537 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + direct | 0.24 | 139.0 | 16.30 | 6.40 | 2.30 | 51.9 | 0.01 | 178 | - | 821 | 3.96 |
| 538 | gpu: rows + sphere + quadtree + raster + direct | 0.24 | 138.9 | 12.48 | 6.96 | 1.21 | 50.8 | 0.01 | 745 | - | 710 | 5.77 |
| 539 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.24 | 138.9 | 21.59 | 5.77 | 2.66 | 31.7 | 8.01 | 676 | - | 759 | 5.69 |
| 540 | gpu: greedy@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.24 | 138.8 | 27.73 | 5.24 | 2.64 | 31.7 | 8.01 | 884 | - | 642 | 6.05 |
| 541 | gpu: bricks + sphere + quadtree + raster + direct | 0.24 | 138.7 | 12.47 | 7.03 | 1.21 | 50.8 | 0.01 | 713 | - | 712 | 5.65 |
| 542 | gpu: naive@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.24 | 138.6 | 14.35 | 6.66 | 1.95 | 31.7 | 8.01 | 700 | - | 768 | 5.82 |
| 543 | gpu: greedy@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.24 | 138.5 | 25.20 | 5.56 | 3.04 | 31.7 | 8.01 | 199 | - | 758 | 3.79 |
| 544 | gpu: naive@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.24 | 138.3 | 13.85 | 6.68 | 1.93 | 31.7 | 8.01 | 708 | - | 769 | 5.86 |
| 545 | gpu: greedy@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.24 | 138.2 | 23.94 | 5.61 | 2.69 | 31.7 | 8.01 | 636 | - | 703 | 5.31 |
| 546 | gpu: greedy@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.24 | 138.0 | 23.92 | 5.63 | 2.68 | 31.7 | 8.01 | 644 | - | 706 | 5.35 |
| 547 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.24 | 137.5 | 21.79 | 5.84 | 2.67 | 31.7 | 8.01 | 684 | - | 758 | 5.72 |
| 548 | gpu: naive@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.24 | 137.2 | 14.96 | 6.56 | 1.94 | 31.7 | 8.01 | 700 | - | 778 | 5.86 |
| 549 | gpu: naive@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.24 | 137.0 | 14.38 | 6.59 | 1.94 | 31.7 | 8.01 | 708 | - | 777 | 5.89 |
| 550 | gpu: greedy@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.24 | 136.9 | 24.68 | 5.70 | 2.66 | 31.7 | 8.01 | 660 | - | 641 | 5.16 |
| 551 | gpu: points@cpu + splat + prefix + incremental + direct | 0.24 | 136.2 | 50.77 | 3.57 | 1.07 | 57.1 | 0.01 | 841 | - | 932 | 7.03 |
| 552 | gpu: naive@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.24 | 136.2 | 15.26 | 6.58 | 1.91 | 31.7 | 8.01 | 732 | - | 771 | 5.96 |
| 553 | gpu: naive@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.24 | 136.2 | 15.45 | 6.59 | 1.91 | 31.7 | 8.01 | 740 | - | 775 | 6.01 |
| 554 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + direct | 0.24 | 136.0 | 12.19 | 6.90 | 2.03 | 51.9 | 0.02 | 232 | - | 751 | 3.89 |
| 555 | gpu: points@cpu + splat + svo_dag + incremental + direct | 0.24 | 135.9 | 45.64 | 3.43 | 1.76 | 57.1 | 0.01 | 303 | - | 1011 | 5.21 |
| 556 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + direct | 0.24 | 135.6 | 12.71 | 6.91 | 2.01 | 51.9 | 0.01 | 240 | - | 749 | 3.92 |
| 557 | gpu: hier + sphere + composite + raycast + pathtrace | 0.24 | 135.5 | 11.40 | 7.19 | 1.20 | 31.7 | 8.00 | 563 | - | 654 | 4.83 |
| 558 | gpu: points@cpu + splat + hier + incremental + direct | 0.24 | 135.4 | 49.54 | 3.60 | 1.05 | 57.1 | 0.01 | 801 | - | 934 | 6.88 |
| 559 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + direct | 0.24 | 135.2 | 16.80 | 6.62 | 2.32 | 51.9 | 0.02 | 170 | - | 824 | 3.94 |
| 560 | gpu: naive@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.24 | 135.1 | 14.58 | 6.67 | 1.91 | 31.7 | 8.01 | 732 | - | 783 | 6.00 |
| 561 | gpu: naive@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.24 | 134.9 | 15.20 | 6.68 | 1.93 | 31.7 | 8.01 | 740 | - | 781 | 6.03 |
| 562 | gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + direct | 0.24 | 134.9 | 13.27 | 7.11 | 1.62 | 51.9 | 0.02 | 668 | - | 733 | 5.56 |
| 563 | gpu: naive@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.24 | 134.8 | 14.90 | 6.75 | 1.93 | 31.7 | 8.01 | 740 | - | 777 | 6.02 |
| 564 | gpu: greedy@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.24 | 134.8 | 22.05 | 6.00 | 2.66 | 31.7 | 8.01 | 676 | - | 760 | 5.69 |
| 565 | gpu: tree64 + incremental + composite + raycast + pathtrace | 0.24 | 134.7 | 11.88 | 7.27 | 1.55 | 31.7 | 8.00 | 126 | - | 758 | 3.51 |
| 566 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + direct | 0.24 | 134.7 | 16.80 | 6.65 | 2.28 | 51.9 | 0.01 | 178 | - | 822 | 3.97 |
| 567 | gpu: clipmap + incremental + quadtree + raster + direct | 0.24 | 134.6 | 11.99 | 7.25 | 1.19 | 50.8 | 0.01 | 754 | - | 759 | 6.00 |
| 568 | gpu: naive@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.24 | 134.5 | 14.72 | 6.78 | 1.96 | 31.7 | 8.01 | 748 | - | 776 | 6.04 |
| 569 | gpu: greedy@cpu + rayquery + raycast + distance + refloor + pathtrace | 0.24 | 134.5 | 53.91 | 3.18 | 4.12 | 31.7 | 8.00 | 1158 | - | 1369 | 10.02 |
| 570 | gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + direct | 0.24 | 134.2 | 12.76 | 7.12 | 1.68 | 51.9 | 0.01 | 676 | - | 733 | 5.59 |
| 571 | gpu: greedy@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.24 | 134.0 | 22.22 | 6.01 | 2.68 | 31.7 | 8.01 | 684 | - | 758 | 5.72 |
| 572 | gpu: naive@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.23 | 133.6 | 14.66 | 6.95 | 1.96 | 31.7 | 8.01 | 708 | - | 774 | 5.87 |
| 573 | gpu: naive@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.23 | 133.3 | 14.21 | 6.97 | 1.93 | 31.7 | 8.01 | 700 | - | 778 | 5.86 |
| 574 | gpu: points@cpu + splat + rows + incremental + direct | 0.23 | 133.1 | 49.74 | 3.68 | 1.06 | 57.1 | 0.01 | 833 | - | 932 | 7.00 |
| 575 | gpu: prefix + sphere + quadtree + raster + direct | 0.23 | 132.9 | 12.92 | 7.31 | 1.18 | 50.8 | 0.01 | 753 | - | 709 | 5.80 |
| 576 | gpu: clipmap + rowrun + composite + raycast + pathtrace | 0.23 | 132.6 | 12.14 | 7.38 | 1.20 | 31.7 | 8.00 | 603 | - | 711 | 5.21 |
| 577 | gpu: hier + sphere + beam + composite + raycast + pathtrace | 0.23 | 132.3 | 11.25 | 7.37 | 1.18 | 31.7 | 8.00 | 563 | - | 658 | 4.84 |
| 578 | gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + direct | 0.23 | 132.1 | 13.36 | 7.19 | 1.65 | 51.9 | 0.02 | 668 | - | 737 | 5.57 |
| 579 | gpu: clipmap + incremental + composite + raycast + pathtrace | 0.23 | 132.0 | 12.52 | 7.41 | 1.14 | 31.7 | 8.00 | 603 | - | 714 | 5.22 |
| 580 | gpu: naive@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.23 | 132.0 | 19.88 | 6.58 | 1.93 | 31.7 | 8.01 | 948 | - | 711 | 6.58 |
| 581 | gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + direct | 0.23 | 132.0 | 14.48 | 7.19 | 1.65 | 51.9 | 0.02 | 708 | - | 741 | 5.75 |
| 582 | gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + direct | 0.23 | 131.6 | 13.40 | 7.20 | 1.63 | 51.9 | 0.01 | 716 | - | 740 | 5.77 |
| 583 | gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.23 | 131.4 | 19.24 | 6.58 | 2.60 | 52.0 | 0.01 | 162 | - | 803 | 3.83 |
| 584 | gpu: greedy@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.23 | 131.3 | 22.40 | 6.26 | 2.72 | 31.7 | 8.01 | 636 | - | 702 | 5.30 |
| 585 | gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + direct | 0.23 | 131.2 | 13.93 | 7.22 | 1.65 | 51.9 | 0.01 | 676 | - | 739 | 5.61 |
| 586 | gpu: hier + incremental + composite + raycast + pathtrace | 0.23 | 131.0 | 12.13 | 7.44 | 1.24 | 31.7 | 8.00 | 563 | - | 667 | 4.88 |
| 587 | gpu: naive@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.23 | 131.0 | 14.84 | 6.97 | 1.91 | 31.7 | 8.01 | 700 | - | 778 | 5.86 |
| 588 | gpu: greedy@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.23 | 130.9 | 22.02 | 6.29 | 2.69 | 31.7 | 8.01 | 644 | - | 702 | 5.34 |
| 589 | gpu: greedy@cpu + rayquery + raycast + clipmap + refloor + pathtrace | 0.23 | 130.6 | 48.67 | 4.15 | 4.05 | 31.7 | 8.00 | 942 | - | 1413 | 9.34 |
| 590 | gpu: greedy@cpu + rayquery + raycast + prefix + rowrun + pathtrace | 0.23 | 130.5 | 53.58 | 3.98 | 4.14 | 31.7 | 8.00 | 942 | - | 1402 | 9.29 |
| 591 | gpu: naive@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.23 | 130.5 | 14.83 | 6.97 | 1.94 | 31.7 | 8.01 | 708 | - | 778 | 5.89 |
| 592 | gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 0.23 | 130.2 | 12.53 | 7.53 | 1.55 | 31.7 | 8.00 | 126 | - | 767 | 3.54 |
| 593 | gpu: greedy@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.23 | 129.8 | 23.89 | 6.18 | 2.70 | 31.7 | 8.01 | 636 | - | 714 | 5.35 |
| 594 | gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + direct | 0.23 | 129.8 | 13.37 | 7.29 | 1.66 | 51.9 | 0.02 | 700 | - | 739 | 5.71 |
| 595 | gpu: points@cpu + splat + prefix + refloor + pathtrace | 0.23 | 129.8 | 50.44 | 4.03 | 1.10 | 31.7 | 8.00 | 873 | - | 932 | 7.16 |
| 596 | gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + direct | 0.23 | 129.5 | 14.17 | 7.30 | 1.64 | 51.9 | 0.01 | 708 | - | 741 | 5.75 |
| 597 | gpu: greedy@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.23 | 129.2 | 22.70 | 6.21 | 2.68 | 31.7 | 8.01 | 644 | - | 715 | 5.39 |
| 598 | gpu: points@cpu + splat + distance + sphere + pathtrace | 0.23 | 129.2 | 54.20 | 3.64 | 1.05 | 31.7 | 8.00 | 1089 | - | 933 | 8.02 |
| 599 | gpu: greedy@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.23 | 128.8 | 22.50 | 6.22 | 2.70 | 31.7 | 8.01 | 676 | - | 709 | 5.49 |
| 600 | gpu: greedy@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.23 | 128.5 | 22.49 | 6.20 | 2.68 | 31.7 | 8.01 | 668 | - | 710 | 5.46 |
| 601 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + direct | 0.23 | 128.5 | 16.77 | 7.05 | 2.29 | 51.9 | 0.02 | 170 | - | 829 | 3.96 |
| 602 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + direct | 0.23 | 128.2 | 16.49 | 7.07 | 2.30 | 51.9 | 0.01 | 178 | - | 829 | 3.99 |
| 603 | gpu: naive@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.23 | 128.2 | 14.78 | 7.17 | 1.92 | 31.7 | 8.01 | 740 | - | 786 | 6.05 |
| 604 | gpu: hier + incremental + beam + composite + raycast + pathtrace | 0.22 | 127.9 | 12.32 | 7.63 | 1.18 | 31.7 | 8.00 | 563 | - | 670 | 4.89 |
| 605 | gpu: rows + incremental + beam + composite + raycast + pathtrace | 0.22 | 127.6 | 12.54 | 7.62 | 1.17 | 31.7 | 8.00 | 595 | - | 669 | 5.01 |
| 606 | gpu: points@cpu + splat + bricks + refloor + pathtrace | 0.22 | 127.6 | 51.56 | 4.27 | 1.13 | 31.7 | 8.00 | 833 | - | 931 | 7.00 |
| 607 | gpu: greedy@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.22 | 127.6 | 22.70 | 6.30 | 2.69 | 31.7 | 8.01 | 676 | - | 712 | 5.50 |
| 608 | gpu: tree64 + incremental + quadtree + raster + direct | 0.22 | 127.5 | 12.66 | 7.68 | 1.56 | 50.8 | 0.01 | 276 | - | 817 | 4.34 |
| 609 | gpu: greedy@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.22 | 127.4 | 22.83 | 6.29 | 2.70 | 31.7 | 8.01 | 668 | - | 712 | 5.47 |
| 610 | gpu: greedy@cpu + rayquery + raycast + tree64 + refloor + pathtrace | 0.22 | 127.3 | 44.06 | 4.30 | 4.58 | 31.7 | 8.00 | 465 | - | 1376 | 7.30 |
| 611 | gpu: greedy@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.22 | 127.2 | 22.96 | 6.39 | 2.70 | 31.7 | 8.01 | 676 | - | 707 | 5.48 |
| 612 | gpu: naive@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.22 | 127.0 | 14.94 | 7.27 | 1.93 | 31.7 | 8.01 | 748 | - | 785 | 6.08 |
| 613 | gpu: bricks + incremental + composite + raycast + pathtrace | 0.22 | 126.9 | 12.35 | 7.74 | 1.20 | 31.7 | 8.00 | 563 | - | 662 | 4.85 |
| 614 | gpu: greedy@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.22 | 126.7 | 22.11 | 6.56 | 2.72 | 31.7 | 8.01 | 644 | - | 713 | 5.38 |
| 615 | gpu: greedy@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.22 | 126.7 | 22.58 | 6.58 | 2.69 | 31.7 | 8.01 | 636 | - | 714 | 5.35 |
| 616 | gpu: points@cpu + splat + hier + refloor + pathtrace | 0.22 | 126.6 | 51.87 | 4.04 | 1.11 | 31.7 | 8.00 | 833 | - | 933 | 7.00 |
| 617 | gpu: cubes@cpu + faces + distance + sphere + depth + visbuffer + pathtrace | 0.22 | 126.6 | 16.86 | 7.26 | 1.63 | 31.7 | 8.01 | 956 | - | 742 | 6.73 |
| 618 | gpu: greedy@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.22 | 126.6 | 22.99 | 6.42 | 2.70 | 31.7 | 8.01 | 684 | - | 706 | 5.51 |
| 619 | gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 0.22 | 126.5 | 13.31 | 7.72 | 1.20 | 31.7 | 8.01 | 753 | - | 721 | 5.84 |
| 620 | gpu: greedy@cpu + rayquery + raycast + rows + rowrun + pathtrace | 0.22 | 126.3 | 52.36 | 4.23 | 4.14 | 31.7 | 8.00 | 934 | - | 1389 | 9.21 |
| 621 | gpu: cubes@cpu + faces + distance + sphere + depth + gbuffer + pathtrace | 0.22 | 126.2 | 17.30 | 7.28 | 1.63 | 31.7 | 8.01 | 964 | - | 742 | 6.76 |
| 622 | gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 0.22 | 126.0 | 13.17 | 7.75 | 1.19 | 31.7 | 8.01 | 760 | - | 720 | 5.87 |
| 623 | gpu: hier + rowrun + composite + raycast + pathtrace | 0.22 | 125.9 | 12.51 | 7.75 | 1.14 | 31.7 | 8.00 | 563 | - | 662 | 4.86 |
| 624 | gpu: cubes@cpu + faces + prefix + refloor + depth + visbuffer + pathtrace | 0.22 | 125.5 | 13.38 | 7.60 | 1.64 | 31.7 | 8.01 | 740 | - | 739 | 5.86 |
| 625 | gpu: points@cpu + splat + rows + refloor + pathtrace | 0.22 | 125.0 | 51.49 | 4.26 | 1.08 | 31.7 | 8.00 | 865 | - | 934 | 7.13 |
| 626 | gpu: cubes@cpu + faces + hier + refloor + depth + visbuffer + pathtrace | 0.22 | 124.6 | 13.71 | 7.66 | 1.65 | 31.7 | 8.01 | 700 | - | 737 | 5.70 |
| 627 | gpu: points@cpu + splat + distance + incremental + direct | 0.22 | 124.3 | 54.55 | 4.06 | 1.09 | 57.1 | 0.01 | 1057 | - | 933 | 7.89 |
| 628 | gpu: bricks + incremental + beam + composite + raycast + pathtrace | 0.22 | 124.2 | 12.31 | 7.91 | 1.23 | 31.7 | 8.00 | 563 | - | 670 | 4.89 |
| 629 | gpu: cubes@cpu + faces + prefix + refloor + depth + gbuffer + pathtrace | 0.22 | 124.1 | 13.63 | 7.69 | 1.65 | 31.7 | 8.01 | 748 | - | 739 | 5.90 |
| 630 | gpu: greedy@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.22 | 123.8 | 23.33 | 6.59 | 2.71 | 31.7 | 8.01 | 636 | - | 713 | 5.35 |
| 631 | gpu: points@cpu + splat + distance + refloor + pathtrace | 0.22 | 123.6 | 55.25 | 3.99 | 1.10 | 31.7 | 8.00 | 1089 | - | 934 | 8.02 |
| 632 | gpu: greedy@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.22 | 123.5 | 23.58 | 6.59 | 2.70 | 31.7 | 8.01 | 644 | - | 712 | 5.38 |
| 633 | gpu: rows + incremental + composite + raycast + pathtrace | 0.22 | 123.1 | 13.71 | 7.91 | 1.20 | 31.7 | 8.00 | 595 | - | 661 | 4.98 |
| 634 | gpu: greedy@cpu + rayquery + raycast + distance + rowrun + pathtrace | 0.22 | 123.1 | 52.86 | 4.04 | 4.24 | 31.7 | 8.00 | 1158 | - | 1414 | 10.20 |
| 635 | gpu: bricks + sphere + composite + raycast + pathtrace | 0.22 | 123.1 | 12.00 | 7.99 | 1.23 | 31.7 | 8.00 | 563 | - | 651 | 4.81 |
| 636 | gpu: greedy@cpu + rayquery + raycast + rows + incremental + pathtrace | 0.22 | 122.9 | 50.11 | 4.52 | 4.14 | 31.7 | 8.00 | 934 | - | 1416 | 9.32 |
| 637 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.22 | 122.7 | 13.78 | 7.86 | 1.67 | 31.7 | 8.01 | 700 | - | 735 | 5.69 |
| 638 | gpu: cubes@cpu + faces + hier + refloor + depth + gbuffer + pathtrace | 0.22 | 122.6 | 14.32 | 7.78 | 1.67 | 31.7 | 8.01 | 708 | - | 740 | 5.74 |
| 639 | gpu: naive@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.22 | 122.6 | 15.17 | 7.62 | 1.95 | 31.7 | 8.01 | 732 | - | 711 | 5.72 |
| 640 | gpu: greedy@cpu + rayquery + raycast + svo_dag + refloor + pathtrace | 0.22 | 122.6 | 50.83 | 4.35 | 4.86 | 31.7 | 8.00 | 404 | - | 1490 | 7.51 |
| 641 | gpu: cubes@cpu + faces + distance + refloor + depth + visbuffer + pathtrace | 0.21 | 122.1 | 16.04 | 7.61 | 1.65 | 31.7 | 8.01 | 956 | - | 742 | 6.73 |
| 642 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.21 | 121.8 | 24.88 | 6.72 | 3.06 | 50.0 | 0.02 | 160 | - | 707 | 3.44 |
| 643 | gpu: greedy@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.21 | 121.4 | 22.94 | 6.80 | 2.74 | 31.7 | 8.01 | 676 | - | 716 | 5.52 |
| 644 | gpu: rows + sphere + composite + raycast + pathtrace | 0.21 | 121.4 | 13.29 | 8.01 | 1.25 | 31.7 | 8.00 | 595 | - | 653 | 4.95 |
| 645 | gpu: tree64 + rowrun + quadtree + visbuffer + direct | 0.21 | 121.4 | 12.65 | 8.08 | 1.62 | 50.8 | 0.02 | 284 | - | 860 | 4.54 |
| 646 | gpu: tree64 + rowrun + quadtree + gbuffer + direct | 0.21 | 121.4 | 13.04 | 8.08 | 1.64 | 50.8 | 0.02 | 292 | - | 854 | 4.55 |
| 647 | gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + direct | 0.21 | 121.1 | 16.18 | 7.66 | 1.65 | 51.9 | 0.02 | 924 | - | 745 | 6.62 |
| 648 | gpu: greedy@cpu + rayquery + raycast + bricks + incremental + pathtrace | 0.21 | 120.6 | 49.54 | 4.87 | 4.23 | 31.7 | 8.00 | 902 | - | 1395 | 9.11 |
| 649 | gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + direct | 0.21 | 120.6 | 15.75 | 7.72 | 1.65 | 51.9 | 0.01 | 932 | - | 740 | 6.63 |
| 650 | gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 0.21 | 120.4 | 12.36 | 8.16 | 1.19 | 31.7 | 8.00 | 603 | - | 719 | 5.24 |
| 651 | gpu: greedy@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.21 | 120.4 | 22.64 | 6.91 | 2.69 | 31.7 | 8.01 | 684 | - | 714 | 5.54 |
| 652 | gpu: naive@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.21 | 120.2 | 15.00 | 7.77 | 1.94 | 31.7 | 8.01 | 692 | - | 713 | 5.57 |
| 653 | gpu: cubes@cpu + faces + rows + refloor + depth + visbuffer + pathtrace | 0.21 | 120.1 | 13.64 | 7.94 | 1.65 | 31.7 | 8.01 | 732 | - | 743 | 5.85 |
| 654 | gpu: hier + sphere + quadtree + visbuffer + pathtrace | 0.21 | 120.0 | 13.36 | 8.11 | 1.19 | 31.7 | 8.01 | 753 | - | 737 | 5.91 |
| 655 | gpu: cubes@cpu + faces + distance + refloor + depth + gbuffer + pathtrace | 0.21 | 119.5 | 15.86 | 7.80 | 1.63 | 31.7 | 8.01 | 964 | - | 742 | 6.76 |
| 656 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.21 | 119.5 | 14.10 | 8.08 | 1.66 | 31.7 | 8.01 | 708 | - | 738 | 5.73 |
| 657 | gpu: hier + sphere + quadtree + gbuffer + pathtrace | 0.21 | 119.4 | 13.81 | 8.14 | 1.23 | 31.7 | 8.01 | 760 | - | 732 | 5.92 |
| 658 | gpu: prefix + incremental + composite + raycast + pathtrace | 0.21 | 118.9 | 13.32 | 8.23 | 1.20 | 31.7 | 8.00 | 603 | - | 669 | 5.04 |
| 659 | gpu: bricks + sphere + beam + composite + raycast + pathtrace | 0.21 | 118.9 | 12.40 | 8.27 | 1.20 | 31.7 | 8.00 | 563 | - | 657 | 4.84 |
| 660 | gpu: cubes@cpu + faces + rows + refloor + depth + gbuffer + pathtrace | 0.21 | 118.9 | 14.81 | 8.03 | 1.66 | 31.7 | 8.01 | 740 | - | 742 | 5.88 |
| 661 | gpu: prefix + sphere + composite + raycast + pathtrace | 0.21 | 118.8 | 12.65 | 8.23 | 1.24 | 31.7 | 8.00 | 603 | - | 654 | 4.98 |
| 662 | gpu: clipmap + rowrun + quadtree + raster + direct | 0.21 | 118.8 | 13.00 | 8.26 | 1.18 | 50.8 | 0.01 | 754 | - | 764 | 6.02 |
| 663 | gpu: hier + rowrun + beam + composite + raycast + pathtrace | 0.21 | 118.5 | 12.85 | 8.24 | 1.22 | 31.7 | 8.00 | 563 | - | 667 | 4.88 |
| 664 | gpu: points@cpu + splat + tree64 + refloor + pathtrace | 0.21 | 118.4 | 45.04 | 5.07 | 1.49 | 31.7 | 8.00 | 396 | - | 1028 | 5.65 |
| 665 | gpu: rows + sphere + beam + composite + raycast + pathtrace | 0.21 | 118.4 | 13.82 | 8.24 | 1.17 | 31.7 | 8.00 | 595 | - | 657 | 4.96 |
| 666 | gpu: prefix + sphere + beam + composite + raycast + pathtrace | 0.21 | 118.2 | 12.52 | 8.27 | 1.14 | 31.7 | 8.00 | 603 | - | 660 | 5.00 |
| 667 | gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 0.21 | 117.9 | 13.09 | 8.32 | 1.61 | 31.7 | 8.01 | 324 | - | 842 | 4.62 |
| 668 | gpu: prefix + incremental + beam + composite + raycast + pathtrace | 0.21 | 117.8 | 13.49 | 8.30 | 1.19 | 31.7 | 8.00 | 603 | - | 676 | 5.07 |
| 669 | gpu: greedy@cpu + rayquery + raycast + hier + incremental + pathtrace | 0.21 | 117.7 | 52.78 | 4.89 | 4.22 | 31.7 | 8.00 | 902 | - | 1344 | 8.90 |
| 670 | gpu: naive@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.21 | 117.6 | 17.17 | 7.68 | 1.95 | 31.7 | 8.01 | 948 | - | 711 | 6.58 |
| 671 | gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 0.21 | 117.1 | 13.66 | 8.36 | 1.19 | 31.7 | 8.01 | 793 | - | 779 | 6.23 |
| 672 | gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 0.21 | 117.0 | 13.29 | 8.37 | 1.58 | 31.7 | 8.01 | 316 | - | 853 | 4.63 |
| 673 | gpu: greedy@cpu + rayquery + raycast + prefix + incremental + pathtrace | 0.21 | 116.9 | 47.94 | 5.08 | 4.26 | 31.7 | 8.00 | 942 | - | 1411 | 9.33 |
| 674 | gpu: hier + incremental + quadtree + visbuffer + pathtrace | 0.20 | 116.4 | 14.01 | 8.37 | 1.24 | 31.7 | 8.01 | 753 | - | 727 | 5.87 |
| 675 | gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 0.20 | 115.7 | 13.58 | 8.46 | 1.21 | 31.7 | 8.01 | 801 | - | 774 | 6.25 |
| 676 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.20 | 115.7 | 14.99 | 8.24 | 1.96 | 31.7 | 8.01 | 692 | - | 711 | 5.56 |
| 677 | gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 0.20 | 115.6 | 13.80 | 8.49 | 1.19 | 31.7 | 8.00 | 603 | - | 724 | 5.26 |
| 678 | gpu: hier + incremental + quadtree + gbuffer + pathtrace | 0.20 | 115.6 | 14.15 | 8.42 | 1.20 | 31.7 | 8.01 | 760 | - | 752 | 6.00 |
| 679 | gpu: points@cpu + splat + prefix + rowrun + pathtrace | 0.20 | 115.5 | 51.58 | 5.19 | 1.14 | 31.7 | 8.00 | 873 | - | 933 | 7.16 |
| 680 | gpu: rows + incremental + quadtree + visbuffer + pathtrace | 0.20 | 115.4 | 13.98 | 8.43 | 1.25 | 31.7 | 8.01 | 785 | - | 734 | 6.02 |
| 681 | gpu: rows + incremental + quadtree + gbuffer + pathtrace | 0.20 | 115.1 | 14.15 | 8.45 | 1.23 | 31.7 | 8.01 | 792 | - | 723 | 6.01 |
| 682 | gpu: greedy@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.20 | 115.1 | 20.92 | 7.52 | 2.69 | 31.7 | 8.01 | 669 | - | 708 | 5.46 |
| 683 | gpu: naive@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.20 | 114.6 | 15.30 | 8.21 | 1.94 | 31.7 | 8.01 | 724 | - | 711 | 5.69 |
| 684 | gpu: greedy@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.20 | 114.6 | 24.37 | 7.25 | 3.04 | 31.7 | 8.01 | 191 | - | 691 | 3.50 |
| 685 | gpu: greedy@cpu + rayquery + raycast + clipmap + incremental + pathtrace | 0.20 | 114.5 | 47.63 | 5.44 | 4.18 | 31.7 | 8.00 | 942 | - | 1436 | 9.43 |
| 686 | gpu: points@cpu + splat + clipmap + refloor + pathtrace | 0.20 | 113.6 | 49.84 | 5.56 | 1.11 | 31.7 | 8.00 | 873 | - | 988 | 7.38 |
| 687 | gpu: points@cpu + splat + svo_dag + refloor + pathtrace | 0.20 | 113.2 | 46.10 | 5.16 | 1.78 | 31.7 | 8.00 | 335 | - | 1050 | 5.49 |
| 688 | gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 0.20 | 112.8 | 13.72 | 8.68 | 1.20 | 31.7 | 8.01 | 793 | - | 784 | 6.25 |
| 689 | gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 0.20 | 112.8 | 13.71 | 8.68 | 1.27 | 31.7 | 8.01 | 801 | - | 776 | 6.25 |
| 690 | gpu: points@cpu + splat + rows + rowrun + pathtrace | 0.20 | 112.3 | 52.41 | 5.32 | 1.13 | 31.7 | 8.00 | 865 | - | 933 | 7.13 |
| 691 | gpu: greedy@cpu + rayquery + raycast + bricks + rowrun + pathtrace | 0.20 | 111.2 | 51.21 | 5.59 | 4.32 | 31.7 | 8.00 | 902 | - | 1408 | 9.16 |
| 692 | gpu: greedy@cpu + rayquery + raycast + clipmap + rowrun + pathtrace | 0.19 | 110.5 | 47.48 | 5.76 | 4.23 | 31.7 | 8.00 | 942 | - | 1399 | 9.28 |
| 693 | gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 0.19 | 110.5 | 14.88 | 8.88 | 1.25 | 31.7 | 8.01 | 753 | - | 733 | 5.89 |
| 694 | gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 0.19 | 110.5 | 14.72 | 8.87 | 1.22 | 31.7 | 8.01 | 760 | - | 715 | 5.85 |
| 695 | gpu: rows + sphere + quadtree + visbuffer + pathtrace | 0.19 | 110.5 | 14.19 | 8.83 | 1.23 | 31.7 | 8.01 | 785 | - | 736 | 6.03 |
| 696 | gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 0.19 | 110.3 | 14.95 | 8.88 | 1.21 | 31.7 | 8.01 | 753 | - | 720 | 5.84 |
| 697 | gpu: naive@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.19 | 110.2 | 15.01 | 8.67 | 1.94 | 31.7 | 8.01 | 740 | - | 821 | 6.19 |
| 698 | gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 0.19 | 110.0 | 14.18 | 8.92 | 1.24 | 31.7 | 8.01 | 760 | - | 727 | 5.90 |
| 699 | gpu: rows + sphere + quadtree + gbuffer + pathtrace | 0.19 | 109.9 | 14.49 | 8.87 | 1.23 | 31.7 | 8.01 | 792 | - | 744 | 6.09 |
| 700 | gpu: naive@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.19 | 109.9 | 15.28 | 8.68 | 1.94 | 31.7 | 8.01 | 748 | - | 820 | 6.22 |
| 701 | gpu: cubes@cpu + faces + tree64 + refloor + depth + gbuffer + pathtrace | 0.19 | 109.5 | 14.35 | 8.79 | 2.01 | 31.7 | 8.01 | 271 | - | 760 | 4.09 |
| 702 | gpu: clipmap + sphere + quadtree + raster + direct | 0.19 | 109.4 | 14.42 | 8.96 | 1.23 | 50.8 | 0.01 | 754 | - | 775 | 6.06 |
| 703 | gpu: naive@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.19 | 109.4 | 19.26 | 8.23 | 2.61 | 31.7 | 8.01 | 210 | - | 862 | 4.25 |
| 704 | gpu: naive@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.19 | 109.4 | 19.23 | 8.24 | 2.63 | 31.7 | 8.01 | 202 | - | 863 | 4.22 |
| 705 | gpu: naive@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.19 | 109.4 | 17.66 | 8.39 | 1.96 | 31.7 | 8.01 | 964 | - | 780 | 6.92 |
| 706 | gpu: greedy@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.19 | 109.0 | 22.69 | 7.92 | 2.69 | 31.7 | 8.01 | 660 | - | 646 | 5.18 |
| 707 | gpu: naive@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.19 | 109.0 | 17.07 | 8.43 | 1.94 | 31.7 | 8.01 | 956 | - | 780 | 6.88 |
| 708 | gpu: cubes@cpu + faces + tree64 + refloor + depth + visbuffer + pathtrace | 0.19 | 108.9 | 15.06 | 8.84 | 2.03 | 31.7 | 8.01 | 263 | - | 759 | 4.05 |
| 709 | gpu: svo_dag + sphere + quadtree + raster + direct | 0.19 | 108.7 | 15.23 | 8.90 | 1.90 | 50.8 | 0.01 | 215 | - | 873 | 4.31 |
| 710 | gpu: cubes@cpu + faces + distance + rowrun + depth + visbuffer + pathtrace | 0.19 | 108.5 | 15.52 | 8.75 | 1.61 | 31.7 | 8.01 | 956 | - | 752 | 6.77 |
| 711 | gpu: points@cpu + splat + distance + rowrun + pathtrace | 0.19 | 108.5 | 56.40 | 5.41 | 1.09 | 31.7 | 8.00 | 1089 | - | 933 | 8.02 |
| 712 | gpu: cubes@cpu + faces + prefix + rowrun + depth + visbuffer + pathtrace | 0.19 | 108.4 | 15.60 | 8.89 | 1.64 | 31.7 | 8.01 | 740 | - | 757 | 5.94 |
| 713 | gpu: cubes@cpu + faces + prefix + rowrun + depth + gbuffer + pathtrace | 0.19 | 108.1 | 15.41 | 8.91 | 1.67 | 31.7 | 8.01 | 748 | - | 757 | 5.97 |
| 714 | gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 0.19 | 107.9 | 14.48 | 9.05 | 1.24 | 31.7 | 8.01 | 793 | - | 741 | 6.08 |
| 715 | gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 0.19 | 107.6 | 15.01 | 9.08 | 1.23 | 31.7 | 8.01 | 800 | - | 743 | 6.12 |
| 716 | gpu: greedy@cpu + rayquery + raycast + tree64 + incremental + pathtrace | 0.19 | 107.6 | 45.67 | 5.89 | 4.68 | 31.7 | 8.00 | 465 | - | 1348 | 7.19 |
| 717 | gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 0.19 | 107.2 | 14.49 | 9.10 | 1.22 | 31.7 | 8.01 | 793 | - | 742 | 6.09 |
| 718 | gpu: cubes@cpu + faces + rows + rowrun + depth + gbuffer + pathtrace | 0.19 | 106.9 | 15.39 | 9.02 | 1.68 | 31.7 | 8.01 | 740 | - | 745 | 5.89 |
| 719 | gpu: cubes@cpu + faces + rows + rowrun + depth + visbuffer + pathtrace | 0.19 | 106.7 | 15.38 | 9.04 | 1.68 | 31.7 | 8.01 | 732 | - | 746 | 5.86 |
| 720 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + visbuffer + pathtrace | 0.19 | 106.5 | 18.19 | 8.81 | 2.33 | 31.7 | 8.01 | 202 | - | 830 | 4.09 |
| 721 | gpu: greedy@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.19 | 106.4 | 21.35 | 8.26 | 2.72 | 31.7 | 8.01 | 684 | - | 759 | 5.72 |
| 722 | gpu: bricks + rowrun + quadtree + raster + direct | 0.19 | 106.4 | 14.46 | 9.24 | 1.19 | 50.8 | 0.01 | 713 | - | 726 | 5.70 |
| 723 | gpu: greedy@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.19 | 106.3 | 21.24 | 8.28 | 2.70 | 31.7 | 8.01 | 676 | - | 759 | 5.69 |
| 724 | gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 0.19 | 106.1 | 15.01 | 9.21 | 1.20 | 31.7 | 8.01 | 800 | - | 760 | 6.19 |
| 725 | gpu: greedy@cpu + rayquery + raycast + distance + incremental + pathtrace | 0.19 | 105.4 | 52.49 | 5.73 | 4.32 | 31.7 | 8.00 | 1158 | - | 1384 | 10.08 |
| 726 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + gbuffer + pathtrace | 0.18 | 105.2 | 18.09 | 8.93 | 2.34 | 31.7 | 8.01 | 210 | - | 831 | 4.12 |
| 727 | gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 0.18 | 104.9 | 14.96 | 9.32 | 1.20 | 31.7 | 8.01 | 760 | - | 751 | 5.99 |
| 728 | gpu: greedy@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.18 | 104.7 | 27.26 | 8.02 | 2.70 | 31.7 | 8.01 | 900 | - | 711 | 6.39 |
| 729 | gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 0.18 | 104.7 | 14.95 | 9.34 | 1.22 | 31.7 | 8.01 | 753 | - | 761 | 6.00 |
| 730 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.18 | 104.6 | 27.58 | 7.87 | 3.39 | 31.7 | 8.01 | 138 | - | 802 | 3.73 |
| 731 | gpu: distance + incremental + composite + raycast + pathtrace | 0.18 | 104.6 | 15.49 | 9.31 | 1.21 | 31.7 | 8.00 | 819 | - | 660 | 5.86 |
| 732 | gpu: cubes@cpu + faces + distance + rowrun + depth + gbuffer + pathtrace | 0.18 | 104.6 | 15.77 | 9.12 | 1.66 | 31.7 | 8.01 | 964 | - | 748 | 6.79 |
| 733 | gpu: greedy@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.18 | 104.5 | 26.31 | 8.04 | 2.71 | 31.7 | 8.01 | 892 | - | 711 | 6.36 |
| 734 | gpu: cubes@cpu + faces + clipmap + refloor + depth + gbuffer + pathtrace | 0.18 | 104.2 | 14.71 | 9.32 | 1.70 | 31.7 | 8.01 | 748 | - | 788 | 6.09 |
| 735 | gpu: cubes@cpu + faces + clipmap + refloor + depth + visbuffer + pathtrace | 0.18 | 104.2 | 15.49 | 9.32 | 1.73 | 31.7 | 8.01 | 740 | - | 788 | 6.06 |
| 736 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.18 | 103.9 | 28.15 | 7.88 | 3.42 | 31.7 | 8.01 | 146 | - | 801 | 3.75 |
| 737 | gpu: greedy@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.18 | 103.5 | 27.78 | 8.00 | 3.37 | 31.7 | 8.01 | 130 | - | 746 | 3.47 |
| 738 | gpu: hier + rowrun + quadtree + raster + direct | 0.18 | 102.9 | 15.17 | 9.53 | 1.21 | 50.8 | 0.01 | 713 | - | 736 | 5.74 |
| 739 | gpu: greedy@cpu + rayquery + raycast + hier + sphere + pathtrace | 0.18 | 102.8 | 49.95 | 6.36 | 4.27 | 31.7 | 8.00 | 902 | - | 1371 | 9.01 |
| 740 | gpu: naive@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.18 | 102.8 | 15.90 | 9.18 | 2.31 | 31.7 | 8.01 | 263 | - | 786 | 4.16 |
| 741 | gpu: naive@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.18 | 102.4 | 15.91 | 9.22 | 2.31 | 31.7 | 8.01 | 271 | - | 785 | 4.19 |
| 742 | gpu: distance + incremental + beam + composite + raycast + pathtrace | 0.18 | 102.1 | 15.71 | 9.52 | 1.19 | 31.7 | 8.00 | 819 | - | 670 | 5.90 |
| 743 | gpu: points@cpu + splat + bricks + rowrun + pathtrace | 0.18 | 102.0 | 50.94 | 6.71 | 1.14 | 31.7 | 8.00 | 833 | - | 933 | 7.00 |
| 744 | gpu: points@cpu + splat + tree64 + incremental + pathtrace | 0.18 | 99.7 | 45.60 | 6.99 | 1.49 | 31.7 | 8.00 | 396 | - | 1028 | 5.65 |
| 745 | gpu: greedy@cpu + rayquery + raycast + bricks + sphere + pathtrace | 0.17 | 99.4 | 45.34 | 6.98 | 4.32 | 31.7 | 8.00 | 902 | - | 1411 | 9.17 |
| 746 | gpu: tree64 + sphere + quadtree + raster + direct | 0.17 | 99.4 | 15.40 | 9.92 | 1.60 | 50.8 | 0.01 | 276 | - | 834 | 4.40 |
| 747 | gpu: greedy@cpu + rayquery + raycast + rows + sphere + pathtrace | 0.17 | 99.0 | 47.33 | 6.89 | 4.33 | 31.7 | 8.00 | 934 | - | 1412 | 9.30 |
| 748 | gpu: svo_dag + sphere + composite + raycast + pathtrace | 0.17 | 98.7 | 16.17 | 9.90 | 1.87 | 31.7 | 8.00 | 65 | - | 791 | 3.39 |
| 749 | gpu: greedy@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.17 | 98.1 | 23.96 | 8.87 | 3.08 | 31.7 | 8.01 | 207 | - | 757 | 3.82 |
| 750 | gpu: greedy@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.17 | 98.0 | 24.10 | 8.83 | 3.10 | 31.7 | 8.01 | 199 | - | 759 | 3.80 |
| 751 | gpu: greedy@cpu + rayquery + raycast + prefix + sphere + pathtrace | 0.17 | 97.4 | 46.97 | 7.17 | 4.28 | 31.7 | 8.00 | 942 | - | 1418 | 9.35 |
| 752 | gpu: points@cpu + splat + hier + sphere + pathtrace | 0.17 | 97.3 | 51.39 | 7.18 | 1.10 | 31.7 | 8.00 | 833 | - | 938 | 7.02 |
| 753 | gpu: points@cpu + splat + clipmap + rowrun + pathtrace | 0.17 | 97.2 | 49.04 | 7.30 | 1.13 | 31.7 | 8.00 | 873 | - | 985 | 7.37 |
| 754 | gpu: points@cpu + splat + clipmap + incremental + pathtrace | 0.17 | 95.9 | 50.23 | 7.50 | 1.12 | 31.7 | 8.00 | 873 | - | 984 | 7.37 |
| 755 | gpu: greedy@cpu + rayquery + raycast + hier + rowrun + pathtrace | 0.17 | 95.8 | 50.67 | 7.24 | 4.34 | 31.7 | 8.00 | 902 | - | 1383 | 9.06 |
| 756 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.17 | 94.9 | 27.73 | 8.96 | 3.39 | 50.0 | 0.02 | 98 | - | 746 | 3.35 |
| 757 | gpu: distance + sphere + quadtree + raster + pathtrace | 0.17 | 94.9 | 16.62 | 10.29 | 1.21 | 31.7 | 8.01 | 1001 | - | 727 | 6.85 |
| 758 | gpu: greedy@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.17 | 94.7 | 23.82 | 9.51 | 2.75 | 31.7 | 8.01 | 668 | - | 647 | 5.22 |
| 759 | gpu: clipmap + sphere + composite + raycast + pathtrace | 0.17 | 94.0 | 15.83 | 10.52 | 1.18 | 31.7 | 8.00 | 603 | - | 704 | 5.18 |
| 760 | gpu: cubes@cpu + faces + bricks + rowrun + depth + visbuffer + pathtrace | 0.16 | 93.7 | 16.84 | 10.41 | 1.70 | 31.7 | 8.01 | 700 | - | 753 | 5.76 |
| 761 | gpu: points@cpu + splat + bricks + sphere + pathtrace | 0.16 | 93.6 | 49.97 | 7.86 | 1.14 | 31.7 | 8.00 | 833 | - | 940 | 7.03 |
| 762 | gpu: cubes@cpu + faces + bricks + rowrun + depth + gbuffer + pathtrace | 0.16 | 93.5 | 17.35 | 10.44 | 1.69 | 31.7 | 8.01 | 708 | - | 753 | 5.79 |
| 763 | gpu: points@cpu + splat + hier + incremental + pathtrace | 0.16 | 93.1 | 51.44 | 7.69 | 1.12 | 31.7 | 8.00 | 833 | - | 941 | 7.03 |
| 764 | gpu: distance + incremental + quadtree + gbuffer + pathtrace | 0.16 | 93.0 | 17.27 | 10.46 | 1.22 | 31.7 | 8.01 | 1016 | - | 752 | 7.01 |
| 765 | gpu: distance + incremental + quadtree + visbuffer + pathtrace | 0.16 | 92.9 | 16.82 | 10.48 | 1.25 | 31.7 | 8.01 | 1009 | - | 735 | 6.91 |
| 766 | gpu: naive@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.16 | 92.5 | 16.84 | 10.32 | 2.30 | 31.7 | 8.01 | 255 | - | 725 | 3.89 |
| 767 | gpu: points@cpu + splat + rows + incremental + pathtrace | 0.16 | 92.4 | 52.64 | 7.76 | 1.17 | 31.7 | 8.00 | 865 | - | 934 | 7.13 |
| 768 | gpu: points@cpu + splat + prefix + sphere + pathtrace | 0.16 | 91.6 | 52.18 | 7.92 | 1.14 | 31.7 | 8.00 | 873 | - | 938 | 7.18 |
| 769 | gpu: points@cpu + splat + bricks + incremental + pathtrace | 0.16 | 91.6 | 49.59 | 8.13 | 1.13 | 31.7 | 8.00 | 833 | - | 939 | 7.02 |
| 770 | gpu: points@cpu + splat + rows + sphere + pathtrace | 0.16 | 91.5 | 52.51 | 7.81 | 1.13 | 31.7 | 8.00 | 865 | - | 939 | 7.15 |
| 771 | gpu: greedy@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.16 | 91.3 | 22.71 | 9.95 | 2.69 | 31.7 | 8.01 | 660 | - | 646 | 5.18 |
| 772 | gpu: naive@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.16 | 91.2 | 17.11 | 10.65 | 1.96 | 31.7 | 8.01 | 733 | - | 774 | 5.97 |
| 773 | gpu: cubes@cpu + faces + tree64 + incremental + depth + gbuffer + pathtrace | 0.16 | 91.1 | 16.49 | 10.70 | 2.02 | 31.7 | 8.01 | 271 | - | 776 | 4.15 |
| 774 | gpu: greedy@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.16 | 91.0 | 24.52 | 9.76 | 2.71 | 31.7 | 8.01 | 884 | - | 646 | 6.07 |
| 775 | gpu: cubes@cpu + faces + tree64 + incremental + depth + visbuffer + pathtrace | 0.16 | 90.8 | 16.57 | 10.74 | 2.04 | 31.7 | 8.01 | 263 | - | 770 | 4.10 |
| 776 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.16 | 90.1 | 21.67 | 10.38 | 2.61 | 31.7 | 8.01 | 202 | - | 870 | 4.25 |
| 777 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.16 | 90.0 | 21.43 | 10.36 | 2.64 | 31.7 | 8.01 | 210 | - | 865 | 4.26 |
| 778 | gpu: cubes@cpu + faces + hier + sphere + depth + visbuffer + pathtrace | 0.16 | 89.8 | 16.97 | 10.84 | 1.73 | 31.7 | 8.01 | 700 | - | 742 | 5.72 |
| 779 | gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.16 | 89.7 | 17.42 | 10.71 | 2.30 | 52.0 | 0.01 | 224 | - | 731 | 3.79 |
| 780 | gpu: cubes@cpu + faces + hier + sphere + depth + gbuffer + pathtrace | 0.16 | 89.7 | 16.87 | 10.85 | 1.71 | 31.7 | 8.01 | 708 | - | 742 | 5.75 |
| 781 | gpu: greedy@cpu + rayquery + raycast + clipmap + sphere + pathtrace | 0.16 | 89.2 | 42.24 | 8.47 | 4.20 | 31.7 | 8.00 | 942 | - | 1398 | 9.28 |
| 782 | gpu: cubes@cpu + faces + prefix + refloor + depth + raster + direct | 0.16 | 89.1 | 16.91 | 10.96 | 1.70 | 51.8 | 0.01 | 700 | - | 678 | 5.47 |
| 783 | gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 0.16 | 89.1 | 17.03 | 11.02 | 1.90 | 31.7 | 8.00 | 65 | - | 799 | 3.43 |
| 784 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + direct | 0.16 | 88.9 | 17.44 | 11.02 | 1.70 | 51.8 | 0.01 | 660 | - | 678 | 5.31 |
| 785 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + visbuffer + pathtrace | 0.16 | 88.9 | 17.34 | 10.99 | 1.72 | 31.7 | 8.01 | 740 | - | 794 | 6.08 |
| 786 | gpu: points@cpu + splat + prefix + incremental + pathtrace | 0.16 | 88.7 | 53.68 | 8.30 | 1.14 | 31.7 | 8.00 | 873 | - | 950 | 7.23 |
| 787 | gpu: cubes@cpu + faces + rows + refloor + depth + raster + direct | 0.16 | 88.5 | 17.49 | 11.03 | 1.70 | 51.8 | 0.01 | 692 | - | 678 | 5.43 |
| 788 | gpu: cubes@cpu + faces + hier + refloor + depth + raster + direct | 0.16 | 88.5 | 17.55 | 11.04 | 1.68 | 51.8 | 0.01 | 660 | - | 680 | 5.32 |
| 789 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + gbuffer + pathtrace | 0.16 | 88.4 | 17.14 | 11.06 | 1.67 | 31.7 | 8.01 | 748 | - | 797 | 6.13 |
| 790 | gpu: cubes@cpu + faces + distance + sphere + depth + raster + direct | 0.16 | 88.3 | 17.06 | 10.98 | 1.67 | 51.8 | 0.01 | 916 | - | 678 | 6.32 |
| 791 | gpu: cubes@cpu + faces + distance + refloor + depth + raster + direct | 0.16 | 88.3 | 17.26 | 10.99 | 1.68 | 51.8 | 0.01 | 916 | - | 678 | 6.32 |
| 792 | gpu: points@cpu + splat + hier + rowrun + pathtrace | 0.16 | 88.2 | 54.34 | 8.18 | 1.15 | 31.7 | 8.00 | 833 | - | 941 | 7.04 |
| 793 | gpu: naive@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.15 | 87.8 | 21.34 | 10.69 | 2.61 | 31.7 | 8.01 | 210 | - | 879 | 4.32 |
| 794 | gpu: naive@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.15 | 87.7 | 20.82 | 10.70 | 2.63 | 31.7 | 8.01 | 202 | - | 879 | 4.29 |
| 795 | gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + direct | 0.15 | 87.2 | 17.29 | 11.24 | 1.67 | 51.8 | 0.01 | 701 | - | 744 | 5.73 |
| 796 | gpu: cubes@cpu + faces + clipmap + incremental + depth + visbuffer + pathtrace | 0.15 | 87.1 | 17.47 | 11.22 | 1.69 | 31.7 | 8.01 | 740 | - | 800 | 6.11 |
| 797 | gpu: naive@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.15 | 87.1 | 18.23 | 11.12 | 1.97 | 31.7 | 8.01 | 724 | - | 703 | 5.66 |
| 798 | gpu: cubes@cpu + faces + rows + rowrun + depth + raster + direct | 0.15 | 87.0 | 17.35 | 11.22 | 1.68 | 51.8 | 0.01 | 692 | - | 679 | 5.44 |
| 799 | gpu: cubes@cpu + faces + clipmap + incremental + depth + gbuffer + pathtrace | 0.15 | 87.0 | 17.53 | 11.24 | 1.68 | 31.7 | 8.01 | 748 | - | 800 | 6.14 |
| 800 | gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 0.15 | 86.9 | 16.32 | 11.38 | 1.25 | 31.7 | 8.00 | 603 | - | 709 | 5.20 |
| 801 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.15 | 86.8 | 28.36 | 10.00 | 3.40 | 31.7 | 8.01 | 138 | - | 806 | 3.74 |
| 802 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.15 | 86.6 | 28.15 | 10.01 | 3.43 | 31.7 | 8.01 | 146 | - | 807 | 3.78 |
| 803 | gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 0.15 | 86.1 | 16.96 | 11.45 | 1.26 | 31.7 | 8.01 | 793 | - | 799 | 6.31 |
| 804 | gpu: cubes@cpu + faces + rows + sphere + depth + visbuffer + pathtrace | 0.15 | 85.9 | 17.74 | 11.33 | 1.72 | 31.7 | 8.01 | 732 | - | 740 | 5.84 |
| 805 | gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 0.15 | 85.9 | 17.60 | 11.38 | 1.93 | 31.7 | 8.01 | 254 | - | 897 | 4.57 |
| 806 | gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + direct | 0.15 | 85.7 | 16.84 | 11.45 | 2.05 | 51.8 | 0.01 | 224 | - | 708 | 3.70 |
| 807 | gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 0.15 | 85.7 | 17.95 | 11.41 | 1.97 | 31.7 | 8.01 | 262 | - | 908 | 4.64 |
| 808 | gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 0.15 | 85.5 | 17.31 | 11.53 | 1.23 | 31.7 | 8.01 | 801 | - | 784 | 6.28 |
| 809 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + direct | 0.15 | 85.5 | 18.96 | 11.31 | 2.37 | 51.8 | 0.01 | 162 | - | 777 | 3.73 |
| 810 | gpu: cubes@cpu + faces + hier + incremental + depth + visbuffer + pathtrace | 0.15 | 85.4 | 18.19 | 11.41 | 1.71 | 31.7 | 8.01 | 700 | - | 752 | 5.76 |
| 811 | gpu: greedy@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.15 | 85.4 | 20.60 | 10.85 | 2.74 | 31.7 | 8.01 | 628 | - | 647 | 5.06 |
| 812 | gpu: cubes@cpu + faces + rows + sphere + depth + gbuffer + pathtrace | 0.15 | 85.3 | 18.11 | 11.44 | 1.71 | 31.7 | 8.01 | 740 | - | 740 | 5.87 |
| 813 | gpu: cubes@cpu + faces + hier + incremental + depth + gbuffer + pathtrace | 0.15 | 85.1 | 18.68 | 11.45 | 1.74 | 31.7 | 8.01 | 708 | - | 753 | 5.79 |
| 814 | gpu: cubes@cpu + faces + bricks + sphere + depth + visbuffer + pathtrace | 0.15 | 85.1 | 18.20 | 11.51 | 1.71 | 31.7 | 8.01 | 700 | - | 740 | 5.71 |
| 815 | gpu: cubes@cpu + faces + bricks + sphere + depth + gbuffer + pathtrace | 0.15 | 84.9 | 17.97 | 11.53 | 1.69 | 31.7 | 8.01 | 708 | - | 740 | 5.74 |
| 816 | gpu: naive@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.15 | 84.9 | 20.81 | 11.12 | 2.64 | 31.7 | 8.01 | 194 | - | 809 | 3.98 |
| 817 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.15 | 84.7 | 28.15 | 10.32 | 3.40 | 31.7 | 8.01 | 138 | - | 818 | 3.79 |
| 818 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.15 | 84.6 | 28.60 | 10.34 | 3.43 | 31.7 | 8.01 | 146 | - | 816 | 3.81 |
| 819 | gpu: cubes@cpu + faces + rows + incremental + depth + visbuffer + pathtrace | 0.15 | 84.5 | 18.43 | 11.52 | 1.70 | 31.7 | 8.01 | 732 | - | 748 | 5.87 |
| 820 | gpu: cubes@cpu + faces + rows + incremental + depth + raster + direct | 0.15 | 84.5 | 17.67 | 11.55 | 1.72 | 51.8 | 0.01 | 692 | - | 678 | 5.44 |
| 821 | gpu: greedy@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.15 | 84.5 | 23.61 | 10.93 | 2.73 | 31.7 | 8.01 | 628 | - | 648 | 5.06 |
| 822 | gpu: cubes@cpu + faces + rows + incremental + depth + gbuffer + pathtrace | 0.15 | 84.4 | 18.14 | 11.53 | 1.70 | 31.7 | 8.01 | 740 | - | 747 | 5.89 |
| 823 | gpu: cubes@cpu + faces + prefix + sphere + depth + visbuffer + pathtrace | 0.15 | 84.2 | 18.00 | 11.59 | 1.67 | 31.7 | 8.01 | 740 | - | 742 | 5.87 |
| 824 | gpu: greedy@cpu + rayquery + raycast + svo_dag + incremental + pathtrace | 0.15 | 84.1 | 47.98 | 8.54 | 4.88 | 31.7 | 8.00 | 404 | - | 1488 | 7.50 |
| 825 | gpu: cubes@cpu + faces + prefix + sphere + depth + gbuffer + pathtrace | 0.15 | 84.0 | 18.06 | 11.61 | 1.72 | 31.7 | 8.01 | 748 | - | 741 | 5.90 |
| 826 | gpu: greedy@cpu + rayquery + raycast + svo_dag + sphere + pathtrace | 0.15 | 83.5 | 48.67 | 8.64 | 4.92 | 31.7 | 8.00 | 404 | - | 1457 | 7.38 |
| 827 | gpu: cubes@cpu + faces + hier + sphere + depth + raster + direct | 0.15 | 83.3 | 18.40 | 11.75 | 1.70 | 51.8 | 0.01 | 660 | - | 680 | 5.32 |
| 828 | gpu: cubes@cpu + faces + hier + rowrun + depth + visbuffer + pathtrace | 0.15 | 83.3 | 18.63 | 11.71 | 1.73 | 31.7 | 8.01 | 700 | - | 753 | 5.76 |
| 829 | gpu: cubes@cpu + faces + hier + incremental + depth + raster + direct | 0.15 | 83.3 | 18.00 | 11.75 | 1.71 | 51.8 | 0.01 | 660 | - | 679 | 5.31 |
| 830 | gpu: cubes@cpu + faces + hier + rowrun + depth + gbuffer + pathtrace | 0.15 | 83.3 | 18.82 | 11.72 | 1.70 | 31.7 | 8.01 | 708 | - | 750 | 5.78 |
| 831 | gpu: prefix + refloor + quadtree + raster + pathtrace | 0.15 | 82.9 | 17.95 | 11.87 | 1.26 | 31.7 | 8.01 | 785 | - | 762 | 6.13 |
| 832 | gpu: cubes@cpu + faces + bricks + incremental + depth + gbuffer + pathtrace | 0.15 | 82.9 | 18.46 | 11.81 | 1.73 | 31.7 | 8.01 | 708 | - | 748 | 5.77 |
| 833 | gpu: cubes@cpu + faces + bricks + incremental + depth + raster + direct | 0.15 | 82.8 | 18.67 | 11.86 | 1.70 | 51.8 | 0.01 | 660 | - | 678 | 5.31 |
| 834 | gpu: distance + refloor + quadtree + raster + pathtrace | 0.15 | 82.8 | 17.81 | 11.85 | 1.22 | 31.7 | 8.01 | 1001 | - | 745 | 6.92 |
| 835 | gpu: cubes@cpu + faces + bricks + incremental + depth + visbuffer + pathtrace | 0.15 | 82.7 | 18.47 | 11.83 | 1.72 | 31.7 | 8.01 | 700 | - | 751 | 5.75 |
| 836 | gpu: cubes@cpu + faces + bricks + sphere + depth + raster + direct | 0.15 | 82.6 | 17.96 | 11.88 | 1.69 | 51.8 | 0.01 | 660 | - | 678 | 5.31 |
| 837 | gpu: cubes@cpu + faces + rows + sphere + depth + raster + direct | 0.15 | 82.5 | 18.25 | 11.85 | 1.70 | 51.8 | 0.01 | 692 | - | 678 | 5.43 |
| 838 | gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + direct | 0.14 | 82.3 | 17.90 | 11.90 | 1.71 | 51.8 | 0.01 | 700 | - | 679 | 5.47 |
| 839 | gpu: cubes@cpu + faces + prefix + incremental + depth + visbuffer + pathtrace | 0.14 | 82.1 | 18.79 | 11.89 | 1.69 | 31.7 | 8.01 | 740 | - | 754 | 5.92 |
| 840 | gpu: cubes@cpu + faces + prefix + sphere + depth + raster + direct | 0.14 | 81.7 | 18.30 | 11.97 | 1.73 | 51.8 | 0.01 | 700 | - | 678 | 5.47 |
| 841 | gpu: greedy@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.14 | 81.7 | 23.55 | 11.37 | 2.76 | 31.7 | 8.01 | 668 | - | 647 | 5.21 |
| 842 | gpu: cubes@cpu + faces + distance + rowrun + depth + raster + direct | 0.14 | 81.5 | 17.78 | 11.95 | 1.66 | 51.8 | 0.01 | 916 | - | 678 | 6.32 |
| 843 | gpu: cubes@cpu + faces + distance + incremental + depth + raster + direct | 0.14 | 81.5 | 18.68 | 11.95 | 1.68 | 51.8 | 0.01 | 916 | - | 678 | 6.32 |
| 844 | gpu: tree64 + sphere + composite + raycast + pathtrace | 0.14 | 81.4 | 18.37 | 12.17 | 1.59 | 31.7 | 8.00 | 126 | - | 751 | 3.48 |
| 845 | gpu: cubes@cpu + faces + prefix + incremental + depth + gbuffer + pathtrace | 0.14 | 81.4 | 19.10 | 12.00 | 1.69 | 31.7 | 8.01 | 748 | - | 752 | 5.95 |
| 846 | gpu: points@cpu + splat + svo_dag + sphere + pathtrace | 0.14 | 81.2 | 48.38 | 9.41 | 1.82 | 31.7 | 8.00 | 335 | - | 1060 | 5.53 |
| 847 | gpu: cubes@cpu + faces + prefix + incremental + depth + raster + direct | 0.14 | 81.1 | 18.11 | 12.08 | 1.71 | 51.8 | 0.01 | 700 | - | 679 | 5.47 |
| 848 | gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + direct | 0.14 | 81.1 | 17.64 | 12.11 | 1.69 | 51.8 | 0.01 | 701 | - | 745 | 5.73 |
| 849 | gpu: points@cpu + splat + clipmap + sphere + pathtrace | 0.14 | 81.0 | 49.69 | 9.80 | 1.14 | 31.7 | 8.00 | 873 | - | 994 | 7.41 |
| 850 | gpu: hier + refloor + quadtree + raster + pathtrace | 0.14 | 80.7 | 18.46 | 12.22 | 1.24 | 31.7 | 8.01 | 745 | - | 762 | 5.97 |
| 851 | gpu: points@cpu + splat + distance + incremental + pathtrace | 0.14 | 79.4 | 56.50 | 9.57 | 1.13 | 31.7 | 8.00 | 1089 | - | 949 | 8.08 |
| 852 | gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + direct | 0.14 | 79.0 | 18.25 | 12.44 | 1.70 | 51.8 | 0.01 | 701 | - | 745 | 5.73 |
| 853 | gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 0.14 | 78.7 | 18.55 | 12.56 | 1.62 | 31.7 | 8.01 | 316 | - | 867 | 4.69 |
| 854 | gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + direct | 0.14 | 78.7 | 18.20 | 12.49 | 2.09 | 51.8 | 0.01 | 224 | - | 709 | 3.70 |
| 855 | gpu: greedy@cpu + rayquery + raycast + tree64 + sphere + pathtrace | 0.14 | 78.5 | 43.13 | 9.73 | 4.78 | 31.7 | 8.00 | 465 | - | 1405 | 7.42 |
| 856 | gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 0.14 | 78.3 | 18.89 | 12.63 | 1.64 | 31.7 | 8.01 | 324 | - | 880 | 4.77 |
| 857 | gpu: svo_dag + incremental + composite + raycast + pathtrace | 0.14 | 78.3 | 19.92 | 12.58 | 1.91 | 31.7 | 8.00 | 65 | - | 805 | 3.45 |
| 858 | gpu: svo_dag + rowrun + composite + raycast + pathtrace | 0.14 | 78.1 | 19.81 | 12.62 | 1.90 | 31.7 | 8.00 | 65 | - | 796 | 3.41 |
| 859 | gpu: points@cpu + splat + tree64 + sphere + pathtrace | 0.14 | 77.9 | 45.58 | 10.35 | 1.53 | 31.7 | 8.00 | 396 | - | 1040 | 5.69 |
| 860 | gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 0.14 | 77.5 | 19.13 | 12.79 | 1.58 | 31.7 | 8.00 | 126 | - | 757 | 3.50 |
| 861 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + direct | 0.13 | 76.7 | 19.76 | 12.68 | 2.36 | 51.8 | 0.01 | 162 | - | 777 | 3.72 |
| 862 | gpu: bricks + refloor + quadtree + raster + pathtrace | 0.13 | 76.6 | 19.02 | 12.90 | 1.23 | 31.7 | 8.01 | 745 | - | 745 | 5.91 |
| 863 | gpu: rows + refloor + quadtree + raster + pathtrace | 0.13 | 76.6 | 19.17 | 12.87 | 1.28 | 31.7 | 8.01 | 777 | - | 745 | 6.03 |
| 864 | gpu: greedy@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.13 | 76.5 | 20.26 | 12.35 | 2.74 | 31.7 | 8.01 | 669 | - | 712 | 5.47 |
| 865 | gpu: greedy@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.13 | 76.5 | 24.66 | 12.05 | 3.10 | 31.7 | 8.01 | 191 | - | 696 | 3.52 |
| 866 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + direct | 0.13 | 76.5 | 18.40 | 12.87 | 1.69 | 51.8 | 0.01 | 701 | - | 745 | 5.73 |
| 867 | gpu: greedy@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.13 | 76.1 | 23.78 | 12.36 | 2.73 | 31.7 | 8.01 | 628 | - | 647 | 5.06 |
| 868 | gpu: svo_dag + incremental + quadtree + raster + direct | 0.13 | 75.8 | 19.54 | 12.97 | 1.98 | 50.8 | 0.01 | 215 | - | 894 | 4.40 |
| 869 | gpu: greedy@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.13 | 75.4 | 25.01 | 12.39 | 2.73 | 31.7 | 8.01 | 884 | - | 647 | 6.07 |
| 870 | gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + direct | 0.13 | 75.0 | 18.69 | 13.11 | 2.09 | 51.8 | 0.01 | 224 | - | 710 | 3.70 |
| 871 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 0.13 | 75.0 | 19.53 | 13.11 | 1.32 | 31.7 | 8.01 | 745 | - | 750 | 5.93 |
| 872 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + visbuffer + pathtrace | 0.13 | 74.9 | 21.67 | 12.97 | 2.40 | 31.7 | 8.01 | 202 | - | 837 | 4.12 |
| 873 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + gbuffer + pathtrace | 0.13 | 74.8 | 22.09 | 12.99 | 2.39 | 31.7 | 8.01 | 210 | - | 834 | 4.14 |
| 874 | gpu: cubes@cpu + faces + distance + incremental + depth + gbuffer + pathtrace | 0.13 | 74.3 | 21.23 | 13.11 | 1.74 | 31.7 | 8.01 | 964 | - | 744 | 6.77 |
| 875 | gpu: cubes@cpu + faces + distance + incremental + depth + visbuffer + pathtrace | 0.13 | 74.1 | 21.07 | 13.15 | 1.67 | 31.7 | 8.01 | 956 | - | 745 | 6.74 |
| 876 | gpu: naive@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.13 | 73.5 | 20.95 | 13.29 | 2.01 | 31.7 | 8.01 | 732 | - | 712 | 5.72 |
| 877 | gpu: cubes@cpu + faces + clipmap + sphere + depth + gbuffer + pathtrace | 0.13 | 73.4 | 20.56 | 13.38 | 1.71 | 31.7 | 8.01 | 748 | - | 792 | 6.11 |
| 878 | gpu: cubes@cpu + faces + clipmap + sphere + depth + visbuffer + pathtrace | 0.13 | 73.2 | 20.80 | 13.41 | 1.72 | 31.7 | 8.01 | 740 | - | 794 | 6.08 |
| 879 | gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 0.13 | 72.6 | 20.03 | 13.61 | 1.88 | 31.7 | 8.00 | 65 | - | 804 | 3.44 |
| 880 | gpu: naive@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.13 | 72.2 | 21.79 | 13.51 | 2.34 | 31.7 | 8.01 | 271 | - | 791 | 4.21 |
| 881 | gpu: greedy@cpu + rayquery + raycast + svo_dag + rowrun + pathtrace | 0.13 | 71.9 | 44.93 | 10.94 | 4.84 | 31.7 | 8.00 | 404 | - | 1483 | 7.48 |
| 882 | gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 0.13 | 71.9 | 21.03 | 13.73 | 1.93 | 31.7 | 8.00 | 65 | - | 811 | 3.47 |
| 883 | gpu: naive@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.13 | 71.8 | 21.77 | 13.60 | 2.00 | 31.7 | 8.01 | 724 | - | 712 | 5.69 |
| 884 | gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + direct | 0.13 | 71.6 | 20.67 | 13.78 | 1.75 | 51.8 | 0.01 | 660 | - | 678 | 5.31 |
| 885 | gpu: naive@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.13 | 71.2 | 21.88 | 13.72 | 2.37 | 31.7 | 8.01 | 263 | - | 791 | 4.18 |
| 886 | gpu: cubes@cpu + faces + hier + rowrun + depth + raster + direct | 0.13 | 71.2 | 19.85 | 13.83 | 1.73 | 51.8 | 0.01 | 660 | - | 680 | 5.31 |
| 887 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.12 | 71.0 | 24.82 | 13.14 | 3.12 | 31.7 | 8.01 | 207 | - | 762 | 3.84 |
| 888 | gpu: naive@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.12 | 70.6 | 21.96 | 13.77 | 1.98 | 31.7 | 8.01 | 948 | - | 712 | 6.58 |
| 889 | gpu: cubes@cpu + faces + tree64 + sphere + depth + gbuffer + pathtrace | 0.12 | 70.6 | 21.18 | 13.94 | 2.06 | 31.7 | 8.01 | 271 | - | 761 | 4.09 |
| 890 | gpu: cubes@cpu + faces + tree64 + sphere + depth + visbuffer + pathtrace | 0.12 | 70.3 | 21.30 | 13.99 | 2.07 | 31.7 | 8.01 | 263 | - | 771 | 4.10 |
| 891 | gpu: points@cpu + splat + svo_dag + rowrun + pathtrace | 0.12 | 70.3 | 46.07 | 11.74 | 1.82 | 31.7 | 8.00 | 335 | - | 1070 | 5.57 |
| 892 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.12 | 70.3 | 22.38 | 13.68 | 2.62 | 52.0 | 0.01 | 162 | - | 807 | 3.85 |
| 893 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.12 | 70.1 | 24.59 | 13.34 | 3.12 | 31.7 | 8.01 | 199 | - | 766 | 3.83 |
| 894 | gpu: points@cpu + splat + svo_dag + incremental + pathtrace | 0.12 | 69.6 | 47.34 | 11.91 | 1.84 | 31.7 | 8.00 | 335 | - | 1078 | 5.60 |
| 895 | gpu: greedy@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.12 | 69.5 | 23.74 | 13.70 | 2.76 | 31.7 | 8.01 | 660 | - | 646 | 5.18 |
| 896 | gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 0.12 | 68.6 | 21.98 | 14.35 | 1.97 | 31.7 | 8.01 | 254 | - | 909 | 4.61 |
| 897 | gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 0.12 | 68.5 | 22.03 | 14.37 | 1.93 | 31.7 | 8.01 | 262 | - | 914 | 4.67 |
| 898 | gpu: greedy@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.12 | 68.3 | 21.79 | 14.06 | 2.77 | 31.7 | 8.01 | 628 | - | 647 | 5.05 |
| 899 | gpu: greedy@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.12 | 67.3 | 22.51 | 14.24 | 2.73 | 31.7 | 8.01 | 668 | - | 647 | 5.21 |
| 900 | gpu: naive@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.12 | 66.2 | 23.21 | 14.85 | 2.01 | 31.7 | 8.01 | 692 | - | 712 | 5.57 |
| 901 | gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 0.12 | 66.0 | 23.01 | 14.95 | 1.98 | 31.7 | 8.01 | 262 | - | 907 | 4.64 |
| 902 | gpu: naive@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.12 | 65.8 | 23.84 | 14.89 | 1.99 | 31.7 | 8.01 | 692 | - | 706 | 5.54 |
| 903 | gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 0.12 | 65.5 | 22.16 | 15.06 | 1.99 | 31.7 | 8.01 | 254 | - | 912 | 4.62 |
| 904 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + gbuffer + pathtrace | 0.11 | 64.9 | 23.84 | 15.08 | 2.38 | 31.7 | 8.01 | 210 | - | 838 | 4.16 |
| 905 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + visbuffer + pathtrace | 0.11 | 64.8 | 23.74 | 15.11 | 2.38 | 31.7 | 8.01 | 202 | - | 840 | 4.13 |
| 906 | gpu: greedy@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.11 | 64.7 | 22.94 | 14.95 | 2.74 | 31.7 | 8.01 | 669 | - | 708 | 5.46 |
| 907 | gpu: cubes@cpu + faces + distance + sphere + depth + raster + pathtrace | 0.11 | 64.1 | 21.97 | 15.29 | 1.74 | 31.7 | 8.01 | 948 | - | 680 | 6.45 |
| 908 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + direct | 0.11 | 63.8 | 24.12 | 15.36 | 2.40 | 51.8 | 0.01 | 162 | - | 778 | 3.73 |
| 909 | gpu: naive@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.11 | 63.8 | 24.60 | 15.38 | 2.00 | 31.7 | 8.01 | 732 | - | 712 | 5.73 |
| 910 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + visbuffer + pathtrace | 0.11 | 63.5 | 25.21 | 15.43 | 2.38 | 31.7 | 8.01 | 202 | - | 847 | 4.16 |
| 911 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + gbuffer + pathtrace | 0.11 | 63.4 | 24.64 | 15.43 | 2.38 | 31.7 | 8.01 | 210 | - | 846 | 4.19 |
| 912 | gpu: tree64 + refloor + quadtree + raster + pathtrace | 0.11 | 62.4 | 23.09 | 15.90 | 1.68 | 31.7 | 8.01 | 308 | - | 879 | 4.71 |
| 913 | gpu: cubes@cpu + faces + prefix + refloor + depth + raster + pathtrace | 0.11 | 60.1 | 23.32 | 16.41 | 1.74 | 31.7 | 8.01 | 732 | - | 675 | 5.58 |
| 914 | gpu: points@cpu + splat + tree64 + rowrun + pathtrace | 0.11 | 59.9 | 45.80 | 15.06 | 1.53 | 31.7 | 8.00 | 396 | - | 1051 | 5.74 |
| 915 | gpu: clipmap + refloor + quadtree + raster + pathtrace | 0.10 | 59.6 | 23.86 | 16.62 | 1.23 | 31.7 | 8.01 | 785 | - | 785 | 6.23 |
| 916 | gpu: cubes@cpu + faces + hier + refloor + depth + raster + pathtrace | 0.10 | 59.3 | 24.29 | 16.65 | 1.74 | 31.7 | 8.01 | 692 | - | 681 | 5.44 |
| 917 | gpu: cubes@cpu + faces + distance + refloor + depth + raster + pathtrace | 0.10 | 59.1 | 23.91 | 16.64 | 1.73 | 31.7 | 8.01 | 948 | - | 680 | 6.45 |
| 918 | gpu: naive@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.10 | 58.8 | 25.36 | 16.75 | 2.37 | 31.7 | 8.01 | 255 | - | 718 | 3.86 |
| 919 | gpu: naive@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.10 | 58.7 | 26.08 | 16.80 | 1.99 | 31.7 | 8.01 | 733 | - | 778 | 5.99 |
| 920 | gpu: tree64 + rowrun + composite + raycast + pathtrace | 0.10 | 58.5 | 25.60 | 17.01 | 1.64 | 31.7 | 8.00 | 126 | - | 754 | 3.49 |
| 921 | gpu: greedy@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.10 | 58.2 | 25.99 | 16.67 | 2.78 | 31.7 | 8.01 | 628 | - | 648 | 5.06 |
| 922 | gpu: greedy@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.10 | 58.2 | 25.66 | 16.75 | 2.77 | 31.7 | 8.01 | 628 | - | 647 | 5.05 |
| 923 | gpu: rows + rowrun + quadtree + raster + pathtrace | 0.10 | 57.8 | 24.76 | 17.14 | 1.31 | 31.7 | 8.01 | 777 | - | 735 | 5.99 |
| 924 | gpu: naive@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.10 | 57.7 | 26.81 | 16.98 | 2.00 | 31.7 | 8.01 | 948 | - | 712 | 6.58 |
| 925 | gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 0.10 | 57.7 | 24.73 | 17.24 | 1.64 | 31.7 | 8.00 | 126 | - | 762 | 3.52 |
| 926 | gpu: cubes@cpu + faces + rows + refloor + depth + raster + pathtrace | 0.10 | 57.5 | 24.67 | 17.17 | 1.76 | 31.7 | 8.01 | 724 | - | 679 | 5.56 |
| 927 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.10 | 57.4 | 24.33 | 17.22 | 1.76 | 31.7 | 8.01 | 692 | - | 680 | 5.44 |
| 928 | gpu: svo_dag + refloor + quadtree + raster + pathtrace | 0.10 | 56.8 | 26.77 | 17.43 | 1.99 | 31.7 | 8.01 | 247 | - | 904 | 4.56 |
| 929 | gpu: naive@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.10 | 56.1 | 26.29 | 17.55 | 2.01 | 31.7 | 8.01 | 692 | - | 713 | 5.57 |
| 930 | gpu: greedy@cpu + rayquery + raycast + tree64 + rowrun + pathtrace | 0.10 | 56.1 | 43.09 | 15.36 | 4.72 | 31.7 | 8.00 | 465 | - | 1381 | 7.32 |
| 931 | gpu: greedy@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.10 | 55.4 | 26.77 | 17.64 | 2.75 | 31.7 | 8.01 | 669 | - | 712 | 5.47 |
| 932 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + gbuffer + pathtrace | 0.10 | 54.1 | 27.50 | 18.28 | 2.11 | 31.7 | 8.01 | 271 | - | 768 | 4.12 |
| 933 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + visbuffer + pathtrace | 0.09 | 53.6 | 27.75 | 18.46 | 2.12 | 31.7 | 8.01 | 263 | - | 765 | 4.08 |
| 934 | gpu: greedy@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.09 | 51.9 | 30.12 | 18.53 | 3.41 | 31.7 | 8.01 | 130 | - | 753 | 3.50 |
| 935 | gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 0.09 | 51.4 | 27.66 | 19.35 | 1.66 | 31.7 | 8.01 | 324 | - | 845 | 4.63 |
| 936 | gpu: naive@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.09 | 50.9 | 28.88 | 19.38 | 2.03 | 31.7 | 8.01 | 724 | - | 712 | 5.69 |
| 937 | gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 0.09 | 50.8 | 28.05 | 19.56 | 1.65 | 31.7 | 8.01 | 316 | - | 842 | 4.59 |
| 938 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + direct | 0.09 | 50.5 | 29.34 | 19.63 | 2.10 | 51.8 | 0.01 | 224 | - | 706 | 3.69 |
| 939 | gpu: greedy@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.09 | 50.5 | 28.92 | 19.32 | 3.12 | 31.7 | 8.01 | 191 | - | 697 | 3.52 |
| 940 | gpu: cubes@cpu + faces + tree64 + refloor + depth + raster + pathtrace | 0.09 | 50.3 | 28.89 | 19.68 | 2.12 | 31.7 | 8.01 | 255 | - | 707 | 3.82 |
| 941 | gpu: cubes@cpu + faces + clipmap + refloor + depth + raster + pathtrace | 0.09 | 50.2 | 27.96 | 19.72 | 1.76 | 31.7 | 8.01 | 733 | - | 741 | 5.84 |
| 942 | gpu: prefix + rowrun + quadtree + raster + pathtrace | 0.09 | 50.0 | 29.00 | 19.84 | 1.28 | 31.7 | 8.01 | 785 | - | 736 | 6.03 |
| 943 | gpu: naive@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.09 | 49.6 | 29.25 | 19.95 | 2.00 | 31.7 | 8.01 | 692 | - | 707 | 5.55 |
| 944 | gpu: naive@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.09 | 49.1 | 29.29 | 20.14 | 2.03 | 31.7 | 8.01 | 732 | - | 712 | 5.72 |
| 945 | gpu: distance + rowrun + quadtree + raster + pathtrace | 0.09 | 48.5 | 29.83 | 20.43 | 1.30 | 31.7 | 8.01 | 1001 | - | 735 | 6.88 |
| 946 | gpu: tree64 + rowrun + quadtree + raster + direct | 0.09 | 48.4 | 28.50 | 20.57 | 1.67 | 50.8 | 0.01 | 276 | - | 838 | 4.42 |
| 947 | gpu: cubes@cpu + faces + svo_dag + refloor + depth + raster + pathtrace | 0.08 | 47.6 | 29.81 | 20.76 | 2.43 | 31.7 | 8.01 | 194 | - | 777 | 3.85 |
| 948 | gpu: cubes@cpu + faces + rows + rowrun + depth + raster + pathtrace | 0.08 | 47.3 | 29.93 | 20.89 | 1.75 | 31.7 | 8.01 | 724 | - | 676 | 5.55 |
| 949 | gpu: naive@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.08 | 46.6 | 30.98 | 21.25 | 2.00 | 31.7 | 8.01 | 733 | - | 770 | 5.96 |
| 950 | gpu: greedy@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.08 | 45.9 | 34.49 | 21.19 | 3.42 | 31.7 | 8.01 | 130 | - | 748 | 3.48 |
| 951 | gpu: rows + incremental + quadtree + raster + pathtrace | 0.08 | 45.5 | 31.31 | 21.80 | 1.25 | 31.7 | 8.01 | 777 | - | 737 | 6.00 |
| 952 | gpu: hier + incremental + quadtree + raster + pathtrace | 0.08 | 42.9 | 33.33 | 23.15 | 1.24 | 31.7 | 8.01 | 745 | - | 737 | 5.88 |
| 953 | gpu: cubes@cpu + faces + prefix + rowrun + depth + raster + pathtrace | 0.08 | 42.8 | 33.22 | 23.11 | 1.77 | 31.7 | 8.01 | 732 | - | 676 | 5.58 |
| 954 | gpu: bricks + incremental + quadtree + raster + pathtrace | 0.07 | 42.6 | 33.06 | 23.31 | 1.28 | 31.7 | 8.01 | 745 | - | 734 | 5.86 |
| 955 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + direct | 0.07 | 42.5 | 34.93 | 23.30 | 2.44 | 51.8 | 0.01 | 162 | - | 776 | 3.72 |
| 956 | gpu: cubes@cpu + faces + distance + rowrun + depth + raster + pathtrace | 0.07 | 41.9 | 33.65 | 23.66 | 1.75 | 31.7 | 8.01 | 948 | - | 680 | 6.45 |
| 957 | gpu: naive@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.07 | 41.7 | 34.90 | 23.75 | 2.05 | 31.7 | 8.01 | 692 | - | 709 | 5.56 |
| 958 | gpu: naive@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.07 | 41.2 | 34.89 | 24.05 | 2.03 | 31.7 | 8.01 | 692 | - | 703 | 5.53 |
| 959 | gpu: cubes@cpu + faces + rows + incremental + depth + raster + pathtrace | 0.07 | 41.1 | 33.86 | 24.11 | 1.76 | 31.7 | 8.01 | 724 | - | 676 | 5.55 |
| 960 | gpu: prefix + incremental + quadtree + raster + pathtrace | 0.07 | 40.5 | 35.39 | 24.53 | 1.30 | 31.7 | 8.01 | 785 | - | 737 | 6.03 |
| 961 | gpu: naive@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.07 | 40.2 | 35.55 | 24.66 | 2.04 | 31.7 | 8.01 | 733 | - | 774 | 5.97 |
| 962 | gpu: svo_dag + rowrun + quadtree + raster + direct | 0.07 | 40.0 | 38.22 | 24.86 | 1.99 | 50.8 | 0.01 | 215 | - | 876 | 4.32 |
| 963 | gpu: tree64 + incremental + quadtree + raster + pathtrace | 0.07 | 39.5 | 36.10 | 25.23 | 1.73 | 31.7 | 8.01 | 308 | - | 841 | 4.56 |
| 964 | gpu: cubes@cpu + faces + hier + incremental + depth + raster + pathtrace | 0.07 | 39.2 | 36.19 | 25.29 | 1.79 | 31.7 | 8.01 | 692 | - | 677 | 5.43 |
| 965 | gpu: naive@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.07 | 39.0 | 38.05 | 25.41 | 2.73 | 31.7 | 8.01 | 194 | - | 806 | 3.97 |
| 966 | gpu: cubes@cpu + faces + bricks + incremental + depth + raster + pathtrace | 0.07 | 38.8 | 35.89 | 25.59 | 1.79 | 31.7 | 8.01 | 692 | - | 676 | 5.42 |
| 967 | gpu: cubes@cpu + faces + prefix + incremental + depth + raster + pathtrace | 0.07 | 38.5 | 36.59 | 25.77 | 1.81 | 31.7 | 8.01 | 732 | - | 676 | 5.58 |
| 968 | gpu: hier + sphere + quadtree + raster + pathtrace | 0.07 | 37.7 | 36.82 | 26.38 | 1.38 | 31.7 | 8.01 | 745 | - | 736 | 5.87 |
| 969 | gpu: clipmap + incremental + quadtree + raster + pathtrace | 0.07 | 37.5 | 37.33 | 26.49 | 1.31 | 31.7 | 8.01 | 785 | - | 785 | 6.22 |
| 970 | gpu: distance + incremental + quadtree + raster + pathtrace | 0.07 | 37.3 | 38.16 | 26.66 | 1.30 | 31.7 | 8.01 | 1001 | - | 736 | 6.89 |
| 971 | gpu: naive@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.06 | 36.4 | 40.31 | 27.32 | 2.42 | 31.7 | 8.01 | 255 | - | 718 | 3.86 |
| 972 | gpu: cubes@cpu + faces + tree64 + incremental + depth + raster + pathtrace | 0.06 | 36.1 | 38.84 | 27.57 | 2.17 | 31.7 | 8.01 | 255 | - | 709 | 3.82 |
| 973 | gpu: cubes@cpu + faces + clipmap + incremental + depth + raster + pathtrace | 0.06 | 35.8 | 38.83 | 27.78 | 1.78 | 31.7 | 8.01 | 733 | - | 742 | 5.85 |
| 974 | gpu: cubes@cpu + faces + distance + incremental + depth + raster + pathtrace | 0.06 | 35.5 | 39.14 | 27.97 | 1.79 | 31.7 | 8.01 | 948 | - | 676 | 6.44 |
| 975 | gpu: naive@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.06 | 34.7 | 44.13 | 28.61 | 2.72 | 31.7 | 8.01 | 194 | - | 809 | 3.98 |
| 976 | gpu: cubes@cpu + faces + hier + sphere + depth + raster + pathtrace | 0.06 | 34.4 | 39.48 | 28.88 | 1.83 | 31.7 | 8.01 | 692 | - | 681 | 5.44 |
| 977 | gpu: rows + sphere + quadtree + raster + pathtrace | 0.06 | 33.8 | 40.59 | 29.45 | 1.31 | 31.7 | 8.01 | 777 | - | 735 | 5.99 |
| 978 | gpu: prefix + sphere + quadtree + raster + pathtrace | 0.06 | 33.1 | 41.50 | 30.03 | 1.32 | 31.7 | 8.01 | 785 | - | 735 | 6.03 |
| 979 | gpu: clipmap + rowrun + quadtree + raster + pathtrace | 0.06 | 33.0 | 42.29 | 30.15 | 1.28 | 31.7 | 8.01 | 785 | - | 787 | 6.23 |
| 980 | gpu: bricks + sphere + quadtree + raster + pathtrace | 0.06 | 32.9 | 41.33 | 30.21 | 1.33 | 31.7 | 8.01 | 745 | - | 735 | 5.87 |
| 981 | gpu: cubes@cpu + faces + rows + sphere + depth + raster + pathtrace | 0.06 | 31.9 | 42.36 | 31.16 | 1.78 | 31.7 | 8.01 | 724 | - | 676 | 5.55 |
| 982 | gpu: cubes@cpu + faces + bricks + sphere + depth + raster + pathtrace | 0.06 | 31.3 | 43.12 | 31.77 | 1.81 | 31.7 | 8.01 | 692 | - | 675 | 5.42 |
| 983 | gpu: cubes@cpu + faces + prefix + sphere + depth + raster + pathtrace | 0.05 | 31.2 | 43.39 | 31.82 | 1.79 | 31.7 | 8.01 | 732 | - | 680 | 5.60 |
| 984 | gpu: bricks + rowrun + quadtree + raster + pathtrace | 0.05 | 30.7 | 45.91 | 32.41 | 1.30 | 31.7 | 8.01 | 745 | - | 735 | 5.87 |
| 985 | cpu: hier + refloor + raycast + direct ◆ | 0.05 | 30.5 | 50.44 | - | 31.38 | 59.1 | 0.00 | - | - | 339 | 1.35 |
| 986 | gpu: cubes@cpu + faces + clipmap + rowrun + depth + raster + pathtrace | 0.05 | 30.5 | 44.71 | 32.62 | 1.81 | 31.7 | 8.01 | 733 | - | 742 | 5.85 |
| 987 | gpu: hier + rowrun + quadtree + raster + pathtrace | 0.05 | 30.4 | 46.11 | 32.75 | 1.34 | 31.7 | 8.01 | 745 | - | 744 | 5.90 |
| 988 | gpu: greedy@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.05 | 29.7 | 48.66 | 33.45 | 3.18 | 31.7 | 8.01 | 191 | - | 692 | 3.50 |
| 989 | cpu: hier + incremental + raycast + direct | 0.05 | 28.8 | 52.26 | - | 33.29 | 59.1 | 0.00 | - | - | 340 | 1.35 |
| 990 | gpu: greedy@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.05 | 28.1 | 53.49 | 35.28 | 3.49 | 31.7 | 8.01 | 130 | - | 750 | 3.49 |
| 991 | cpu: prefix + incremental + raycast + direct | 0.05 | 27.9 | 54.97 | - | 34.34 | 59.0 | 0.00 | - | - | 406 | 1.61 |
| 992 | gpu: cubes@cpu + faces + hier + rowrun + depth + raster + pathtrace | 0.05 | 27.8 | 49.42 | 35.81 | 1.83 | 31.7 | 8.01 | 692 | - | 681 | 5.44 |
| 993 | cpu: prefix + refloor + raycast + direct | 0.05 | 27.7 | 56.00 | - | 34.62 | 59.1 | 0.00 | - | - | 408 | 1.62 |
| 994 | gpu: cubes@cpu + faces + bricks + rowrun + depth + raster + pathtrace | 0.05 | 27.6 | 49.83 | 36.06 | 1.84 | 31.7 | 8.01 | 692 | - | 677 | 5.43 |
| 995 | cpu: greedy + faces + raster + hier + incremental + direct | 0.05 | 27.5 | 63.87 | - | 34.72 | 63.9 | 0.00 | - | - | 551 | 2.18 |
| 996 | cpu: greedy + faces + raster + hier + refloor + direct | 0.05 | 27.5 | 63.76 | - | 34.74 | 63.9 | 0.00 | - | - | 550 | 2.18 |
| 997 | cpu: greedy + faces + raster + bricks + refloor + direct | 0.05 | 27.4 | 63.85 | - | 35.04 | 63.9 | 0.00 | - | - | 540 | 2.14 |
| 998 | cpu: greedy + faces + raster + prefix + refloor + direct | 0.05 | 27.3 | 65.16 | - | 34.81 | 63.9 | 0.00 | - | - | 768 | 3.04 |
| 999 | cpu: greedy + faces + raster + bricks + incremental + direct | 0.05 | 27.2 | 64.82 | - | 35.31 | 63.9 | 0.00 | - | - | 536 | 2.12 |
| 1000 | cpu: greedy + faces + raster + u8 + refloor + direct | 0.05 | 27.2 | 64.83 | - | 35.37 | 63.9 | 0.00 | - | - | 666 | 2.64 |
| 1001 | cpu: greedy + faces + raster + prefix + incremental + direct | 0.05 | 27.2 | 65.99 | - | 34.98 | 63.9 | 0.00 | - | - | 768 | 3.05 |
| 1002 | cpu: bits + refloor + raycast + direct ◆ | 0.05 | 27.1 | 53.21 | - | 35.47 | 59.1 | 0.00 | - | - | 339 | 1.35 |
| 1003 | cpu: greedy + faces + raster + u8 + incremental + direct | 0.05 | 27.1 | 63.84 | - | 35.44 | 63.9 | 0.00 | - | - | 653 | 2.59 |
| 1004 | gpu: clipmap + sphere + quadtree + raster + pathtrace | 0.05 | 27.1 | 49.93 | 36.78 | 1.29 | 31.7 | 8.01 | 785 | - | 784 | 6.22 |
| 1005 | cpu: greedy + faces + raster + brickmajor + incremental + direct | 0.05 | 27.0 | 66.52 | - | 35.34 | 63.9 | 0.00 | - | - | 800 | 3.17 |
| 1006 | cpu: bricks + refloor + raycast + direct ◆ | 0.05 | 27.0 | 52.62 | - | 35.83 | 59.1 | 0.00 | - | - | 323 | 1.28 |
| 1007 | cpu: greedy + faces + raster + bits + refloor + direct | 0.05 | 27.0 | 65.66 | - | 35.43 | 63.9 | 0.00 | - | - | 556 | 2.21 |
| 1008 | cpu: greedy + faces + raster + bits + incremental + direct | 0.05 | 26.9 | 66.54 | - | 35.50 | 63.9 | 0.00 | - | - | 551 | 2.18 |
| 1009 | cpu: greedy + faces + raster + morton + refloor + direct | 0.05 | 26.9 | 67.52 | - | 35.51 | 63.9 | 0.00 | - | - | 805 | 3.19 |
| 1010 | cpu: greedy + faces + raster + brickmajor + refloor + direct | 0.05 | 26.9 | 66.73 | - | 35.54 | 63.9 | 0.00 | - | - | 796 | 3.16 |
| 1011 | cpu: greedy + faces + raster + morton + incremental + direct | 0.05 | 26.8 | 67.46 | - | 35.55 | 63.9 | 0.00 | - | - | 786 | 3.12 |
| 1012 | cpu: greedy + faces + raster + palette + refloor + direct | 0.05 | 26.7 | 69.86 | - | 35.44 | 63.9 | 0.00 | - | - | 575 | 2.28 |
| 1013 | gpu: cubes@cpu + faces + clipmap + sphere + depth + raster + pathtrace | 0.05 | 26.7 | 50.37 | 37.24 | 1.86 | 31.7 | 8.01 | 733 | - | 742 | 5.85 |
| 1014 | cpu: greedy + faces + raster + palette + incremental + direct | 0.05 | 26.6 | 69.54 | - | 35.58 | 63.9 | 0.00 | - | - | 572 | 2.27 |
| 1015 | cpu: brickmajor + refloor + raycast + direct | 0.05 | 26.6 | 55.17 | - | 36.15 | 59.1 | 0.00 | - | - | 448 | 1.78 |
| 1016 | cpu: u8 + refloor + raycast + direct ◆ | 0.05 | 26.5 | 54.96 | - | 36.46 | 59.1 | 0.00 | - | - | 259 | 1.03 |
| 1017 | cpu: morton + refloor + raycast + direct | 0.05 | 26.4 | 56.58 | - | 36.33 | 59.1 | 0.00 | - | - | 445 | 1.76 |
| 1018 | gpu: svo_dag + sphere + quadtree + raster + pathtrace | 0.05 | 26.3 | 54.65 | 37.94 | 2.02 | 31.7 | 8.01 | 247 | - | 871 | 4.43 |
| 1019 | cpu: bits + incremental + raycast + direct | 0.05 | 26.1 | 55.73 | - | 36.82 | 59.1 | 0.00 | - | - | 340 | 1.35 |
| 1020 | cpu: bricks + incremental + raycast + direct | 0.05 | 26.1 | 54.64 | - | 37.14 | 59.1 | 0.00 | - | - | 324 | 1.28 |
| 1021 | gpu: cubes@cpu + faces + svo_dag + sphere + depth + raster + pathtrace | 0.05 | 26.1 | 53.09 | 38.21 | 2.56 | 31.7 | 8.01 | 194 | - | 777 | 3.85 |
| 1022 | cpu: brickmajor + incremental + raycast + direct | 0.05 | 26.1 | 56.36 | - | 36.89 | 59.1 | 0.00 | - | - | 448 | 1.78 |
| 1023 | cpu: palette + refloor + raycast + direct ◆ | 0.05 | 26.0 | 57.53 | - | 36.67 | 59.1 | 0.00 | - | - | 253 | 1.00 |
| 1024 | cpu: u8 + incremental + raycast + direct | 0.05 | 25.9 | 55.57 | - | 37.25 | 59.1 | 0.00 | - | - | 258 | 1.02 |
| 1025 | cpu: morton + incremental + raycast + direct | 0.05 | 25.8 | 57.27 | - | 37.19 | 59.1 | 0.00 | - | - | 448 | 1.78 |
| 1026 | cpu: palette + incremental + raycast + direct ◆ | 0.04 | 25.2 | 59.06 | - | 38.02 | 59.1 | 0.00 | - | - | 252 | 1.00 |
| 1027 | gpu: cubes@cpu + faces + tree64 + sphere + depth + raster + pathtrace | 0.04 | 24.5 | 55.88 | 40.62 | 2.20 | 31.7 | 8.01 | 255 | - | 708 | 3.82 |
| 1028 | gpu: tree64 + sphere + quadtree + raster + pathtrace | 0.04 | 23.9 | 58.73 | 41.64 | 1.78 | 31.7 | 8.01 | 308 | - | 845 | 4.57 |
| 1029 | gpu: cubes@cpu + faces + svo_dag + incremental + depth + raster + pathtrace | 0.04 | 23.9 | 59.90 | 41.63 | 2.56 | 31.7 | 8.01 | 194 | - | 780 | 3.86 |
| 1030 | gpu: svo_dag + incremental + quadtree + raster + pathtrace | 0.04 | 21.9 | 67.00 | 45.56 | 2.06 | 31.7 | 8.01 | 247 | - | 870 | 4.43 |
| 1031 | gpu: naive@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.04 | 20.7 | 68.61 | 48.20 | 2.41 | 31.7 | 8.01 | 255 | - | 719 | 3.86 |
| 1032 | gpu: naive@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.03 | 19.5 | 75.02 | 51.27 | 2.77 | 31.7 | 8.01 | 194 | - | 805 | 3.96 |
| 1033 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 18.0 | 77.33 | 12.08 | 53.69 | 29.8 | 8.11 | 567 | - | 1944 | 9.95 |
| 1034 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 18.0 | 134.56 | 0.62 | 53.73 | 42.3 | 0.15 | 543 | - | 1926 | 9.79 |
| 1035 | cpu: naive + faces + raster + hier + incremental + direct | 0.03 | 17.9 | 81.77 | - | 54.10 | 64.5 | 0.00 | - | - | 816 | 3.23 |
| 1036 | cpu: naive + faces + raster + hier + refloor + direct | 0.03 | 17.9 | 80.12 | - | 54.14 | 64.5 | 0.00 | - | - | 848 | 3.36 |
| 1037 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.9 | 132.32 | 0.80 | 53.74 | 42.4 | 0.15 | 551 | - | 1942 | 9.88 |
| 1038 | cpu: naive + faces + raster + u8 + incremental + direct | 0.03 | 17.9 | 80.05 | - | 54.45 | 64.5 | 0.00 | - | - | 944 | 3.74 |
| 1039 | cpu: naive + faces + raster + morton + refloor + direct | 0.03 | 17.8 | 81.80 | - | 54.42 | 64.5 | 0.00 | - | - | 1104 | 4.38 |
| 1040 | cpu: naive + faces + raster + prefix + refloor + direct | 0.03 | 17.8 | 81.71 | - | 54.34 | 64.5 | 0.00 | - | - | 1068 | 4.23 |
| 1041 | cpu: naive + faces + raster + u8 + refloor + direct | 0.03 | 17.8 | 80.25 | - | 54.71 | 64.5 | 0.00 | - | - | 969 | 3.84 |
| 1042 | cpu: naive + faces + raster + bricks + refloor + direct | 0.03 | 17.8 | 79.38 | - | 54.77 | 64.5 | 0.00 | - | - | 839 | 3.33 |
| 1043 | cpu: naive + faces + raster + prefix + incremental + direct | 0.03 | 17.8 | 81.22 | - | 54.38 | 64.5 | 0.00 | - | - | 1069 | 4.24 |
| 1044 | cpu: naive + faces + raster + brickmajor + incremental + direct | 0.03 | 17.8 | 82.10 | - | 54.59 | 64.5 | 0.00 | - | - | 1105 | 4.38 |
| 1045 | cpu: naive + faces + raster + bricks + incremental + direct | 0.03 | 17.8 | 81.61 | - | 54.91 | 64.5 | 0.00 | - | - | 827 | 3.28 |
| 1046 | cpu: naive + faces + raster + bits + incremental + direct | 0.03 | 17.7 | 80.67 | - | 54.70 | 64.5 | 0.00 | - | - | 842 | 3.34 |
| 1047 | cpu: naive + faces + raster + bits + refloor + direct | 0.03 | 17.7 | 81.87 | - | 54.80 | 64.5 | 0.00 | - | - | 859 | 3.41 |
| 1048 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 17.7 | 131.66 | 2.27 | 54.53 | 42.4 | 0.15 | 535 | - | 1862 | 9.51 |
| 1049 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.7 | 128.64 | 4.27 | 54.59 | 29.8 | 8.11 | 582 | - | 1967 | 10.11 |
| 1050 | cpu: naive + faces + raster + brickmajor + refloor + direct | 0.03 | 17.7 | 81.22 | - | 54.96 | 64.5 | 0.00 | - | - | 1086 | 4.31 |
| 1051 | cpu: naive + faces + raster + morton + incremental + direct | 0.03 | 17.7 | 83.55 | - | 54.97 | 64.5 | 0.00 | - | - | 1102 | 4.37 |
| 1052 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.6 | 130.95 | 3.60 | 54.75 | 29.8 | 8.11 | 574 | - | 2042 | 10.37 |
| 1053 | cpu: naive + faces + raster + palette + refloor + direct | 0.03 | 17.6 | 83.95 | - | 54.88 | 64.5 | 0.00 | - | - | 874 | 3.47 |
| 1054 | cpu: naive + faces + raster + palette + incremental + direct | 0.03 | 17.3 | 87.88 | - | 55.79 | 64.5 | 0.00 | - | - | 866 | 3.44 |
| 1055 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.3 | 141.39 | 0.61 | 56.10 | 42.3 | 0.15 | 543 | - | 2028 | 10.19 |
| 1056 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.2 | 80.21 | 11.96 | 56.15 | 29.8 | 8.11 | 567 | - | 2007 | 10.21 |
| 1057 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + refloor + direct | 0.03 | 17.2 | 137.28 | 2.09 | 56.08 | 42.4 | 0.15 | 575 | - | 1869 | 9.69 |
| 1058 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.2 | 136.41 | 0.61 | 56.12 | 42.3 | 0.15 | 543 | - | 2099 | 10.47 |
| 1059 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.2 | 96.30 | 11.85 | 56.05 | 29.8 | 8.11 | 567 | - | 2175 | 10.87 |
| 1060 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + incremental + direct | 0.03 | 17.2 | 82.83 | 9.23 | 57.15 | 42.4 | 0.15 | 37 | - | 2106 | 8.50 |
| 1061 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.2 | 135.97 | 0.78 | 55.86 | 42.4 | 0.15 | 551 | - | 2190 | 10.87 |
| 1062 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.2 | 137.41 | 0.79 | 56.17 | 42.4 | 0.15 | 551 | - | 2151 | 10.71 |
| 1063 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + incremental + direct | 0.03 | 17.2 | 135.90 | 3.48 | 56.12 | 42.4 | 0.15 | 567 | - | 1938 | 9.93 |
| 1064 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 17.2 | 135.53 | 2.25 | 56.14 | 42.4 | 0.15 | 535 | - | 2129 | 10.56 |
| 1065 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.2 | 136.53 | 3.61 | 55.95 | 29.8 | 8.11 | 574 | - | 2280 | 11.32 |
| 1066 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + rowrun + direct | 0.03 | 17.1 | 134.23 | 6.89 | 56.39 | 42.4 | 0.15 | 535 | - | 1862 | 9.50 |
| 1067 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.1 | 135.65 | 4.37 | 56.17 | 29.8 | 8.11 | 582 | - | 2259 | 11.27 |
| 1068 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.1 | 137.88 | 3.66 | 56.52 | 29.8 | 8.11 | 574 | - | 2010 | 10.25 |
| 1069 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + sphere + direct | 0.03 | 17.1 | 136.16 | 4.48 | 56.18 | 42.4 | 0.15 | 567 | - | 1954 | 9.99 |
| 1070 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + sphere + direct | 0.03 | 17.1 | 135.43 | 4.80 | 56.16 | 42.4 | 0.15 | 575 | - | 1838 | 9.57 |
| 1071 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + refloor + direct | 0.03 | 17.1 | 111.88 | 2.96 | 57.39 | 42.4 | 0.15 | 37 | - | 1995 | 8.06 |
| 1072 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 17.1 | 137.71 | 2.26 | 56.42 | 42.4 | 0.15 | 535 | - | 2171 | 10.73 |
| 1073 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + sphere + direct | 0.03 | 17.1 | 136.91 | 2.10 | 55.83 | 42.4 | 0.15 | 791 | - | 1989 | 11.02 |
| 1074 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + rowrun + direct | 0.03 | 17.1 | 135.92 | 4.09 | 56.28 | 42.4 | 0.15 | 575 | - | 1965 | 10.07 |
| 1075 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + prefix + incremental + direct | 0.03 | 17.1 | 136.74 | 4.29 | 56.31 | 42.4 | 0.15 | 575 | - | 1987 | 10.16 |
| 1076 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + refloor + direct | 0.03 | 17.1 | 138.15 | 2.89 | 56.90 | 42.4 | 0.15 | 575 | - | 1963 | 10.07 |
| 1077 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + sphere + direct | 0.03 | 17.1 | 137.03 | 4.16 | 56.41 | 42.4 | 0.15 | 535 | - | 1940 | 9.82 |
| 1078 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.1 | 138.92 | 0.61 | 56.48 | 42.3 | 0.15 | 543 | - | 2256 | 11.10 |
| 1079 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + incremental + direct | 0.03 | 17.1 | 137.86 | 3.82 | 56.54 | 42.4 | 0.15 | 535 | - | 1961 | 9.90 |
| 1080 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.1 | 79.61 | 11.93 | 56.97 | 29.8 | 8.11 | 567 | - | 1929 | 9.90 |
| 1081 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.1 | 136.28 | 3.47 | 56.53 | 29.8 | 8.11 | 574 | - | 2200 | 11.00 |
| 1082 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + refloor + direct | 0.03 | 17.0 | 136.55 | 2.26 | 56.73 | 42.4 | 0.15 | 567 | - | 1976 | 10.08 |
| 1083 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + rowrun + direct | 0.03 | 17.0 | 132.47 | 4.16 | 55.91 | 42.4 | 0.15 | 791 | - | 1944 | 10.85 |
| 1084 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + hier + refloor + direct | 0.03 | 17.0 | 137.24 | 2.27 | 56.74 | 42.4 | 0.15 | 535 | - | 1936 | 9.80 |
| 1085 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.0 | 137.93 | 4.40 | 56.57 | 29.8 | 8.11 | 582 | - | 2271 | 11.31 |
| 1086 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.0 | 134.80 | 4.16 | 56.85 | 29.8 | 8.11 | 582 | - | 1925 | 9.94 |
| 1087 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.0 | 138.85 | 0.81 | 56.49 | 42.4 | 0.15 | 551 | - | 2239 | 11.06 |
| 1088 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.0 | 136.75 | 0.61 | 56.59 | 42.3 | 0.15 | 543 | - | 2205 | 10.90 |
| 1089 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.0 | 136.10 | 3.95 | 56.55 | 29.8 | 8.11 | 582 | - | 2292 | 11.40 |
| 1090 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.0 | 86.47 | 11.88 | 56.96 | 29.8 | 8.11 | 567 | - | 2176 | 10.88 |
| 1091 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + sphere + direct | 0.03 | 17.0 | 88.63 | 5.55 | 57.85 | 42.4 | 0.15 | 37 | - | 2124 | 8.57 |
| 1092 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 17.0 | 139.24 | 0.82 | 56.97 | 42.4 | 0.15 | 551 | - | 1925 | 9.82 |
| 1093 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 17.0 | 138.01 | 2.26 | 57.06 | 42.4 | 0.15 | 535 | - | 1937 | 9.80 |
| 1094 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + refloor + direct | 0.03 | 16.9 | 114.87 | 2.90 | 58.07 | 42.4 | 0.15 | 98 | - | 2059 | 8.55 |
| 1095 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + sphere + direct | 0.03 | 16.9 | 134.21 | 5.90 | 57.31 | 42.4 | 0.15 | 575 | - | 2022 | 10.30 |
| 1096 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + svo_dag + rowrun + direct | 0.03 | 16.9 | 83.50 | 19.90 | 58.05 | 42.4 | 0.15 | 37 | - | 2007 | 8.10 |
| 1097 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + rowrun + direct | 0.03 | 16.9 | 82.46 | 14.82 | 58.16 | 42.4 | 0.15 | 98 | - | 1990 | 8.28 |
| 1098 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + sphere + direct | 0.03 | 16.9 | 83.03 | 6.25 | 58.19 | 42.4 | 0.15 | 98 | - | 2051 | 8.52 |
| 1099 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + rowrun + direct | 0.03 | 16.9 | 137.58 | 5.75 | 57.46 | 42.4 | 0.15 | 575 | - | 1935 | 9.95 |
| 1100 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + refloor + direct | 0.03 | 16.9 | 134.43 | 2.18 | 56.70 | 42.4 | 0.15 | 791 | - | 1912 | 10.72 |
| 1101 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + tree64 + incremental + direct | 0.03 | 16.9 | 83.19 | 4.84 | 58.29 | 42.4 | 0.15 | 98 | - | 2035 | 8.46 |
| 1102 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + rows + rowrun + direct | 0.03 | 16.9 | 137.29 | 2.71 | 57.11 | 42.4 | 0.15 | 567 | - | 1955 | 10.00 |
| 1103 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 16.9 | 140.28 | 2.26 | 57.36 | 42.4 | 0.15 | 535 | - | 2219 | 10.92 |
| 1104 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.8 | 137.94 | 3.52 | 57.19 | 29.8 | 8.11 | 574 | - | 2205 | 11.02 |
| 1105 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + clipmap + incremental + direct | 0.03 | 16.8 | 138.51 | 4.66 | 57.63 | 42.4 | 0.15 | 575 | - | 2055 | 10.43 |
| 1106 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + distance + incremental + direct | 0.03 | 16.8 | 134.74 | 4.26 | 56.69 | 42.4 | 0.15 | 791 | - | 1936 | 10.81 |
| 1107 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 16.8 | 140.62 | 0.61 | 57.62 | 42.3 | 0.15 | 543 | - | 1963 | 9.94 |
| 1108 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 16.5 | 139.24 | 0.61 | 58.29 | 42.3 | 0.15 | 543 | - | 2035 | 10.22 |
| 1109 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.4 | 130.35 | 11.93 | 58.68 | 29.8 | 8.11 | 567 | - | 1966 | 10.04 |
| 1110 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 16.3 | 139.22 | 0.83 | 58.85 | 42.4 | 0.15 | 551 | - | 1972 | 10.00 |
| 1111 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.3 | 139.68 | 3.62 | 58.84 | 29.8 | 8.11 | 574 | - | 2003 | 10.22 |
| 1112 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 16.2 | 142.39 | 2.26 | 59.22 | 42.4 | 0.15 | 535 | - | 1919 | 9.73 |
| 1113 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.1 | 140.76 | 4.28 | 59.49 | 29.8 | 8.11 | 582 | - | 2032 | 10.37 |
| 1114 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.1 | 84.43 | 11.97 | 60.34 | 29.8 | 8.11 | 567 | - | 2048 | 10.37 |
| 1115 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 16.1 | 146.69 | 2.26 | 60.22 | 42.4 | 0.15 | 535 | - | 1929 | 9.77 |
| 1116 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.0 | 147.45 | 3.72 | 60.52 | 29.8 | 8.11 | 574 | - | 2152 | 10.81 |
| 1117 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.0 | 87.90 | 11.94 | 60.69 | 29.8 | 8.11 | 567 | - | 1910 | 9.82 |
| 1118 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 16.0 | 149.14 | 0.61 | 60.73 | 42.3 | 0.15 | 543 | - | 2054 | 10.30 |
| 1119 | gpu: tree64 + rowrun + quadtree + raster + pathtrace | 0.03 | 15.9 | 88.30 | 62.65 | 1.72 | 31.7 | 8.01 | 308 | - | 839 | 4.55 |
| 1120 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + direct | 0.03 | 15.9 | 149.15 | 2.26 | 60.90 | 42.4 | 0.15 | 535 | - | 2086 | 10.39 |
| 1121 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 15.9 | 149.59 | 0.83 | 61.10 | 42.4 | 0.15 | 551 | - | 2086 | 10.46 |
| 1122 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 15.9 | 145.21 | 4.13 | 60.80 | 29.8 | 8.11 | 582 | - | 1962 | 10.09 |
| 1123 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.03 | 15.8 | 146.11 | 0.83 | 61.04 | 42.4 | 0.15 | 551 | - | 1978 | 10.03 |
| 1124 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 15.8 | 144.79 | 4.19 | 61.28 | 29.8 | 8.11 | 582 | - | 2187 | 10.98 |
| 1125 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 15.8 | 145.73 | 3.68 | 61.28 | 29.8 | 8.11 | 574 | - | 2029 | 10.32 |
| 1126 | gpu: cubes@cpu + faces + tree64 + rowrun + depth + raster + pathtrace | 0.03 | 15.4 | 87.60 | 64.61 | 2.25 | 31.7 | 8.01 | 255 | - | 708 | 3.82 |
| 1127 | gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 0.03 | 15.3 | 96.91 | 65.26 | 2.10 | 31.7 | 8.01 | 247 | - | 874 | 4.44 |
| 1128 | gpu: cubes@cpu + faces + svo_dag + rowrun + depth + raster + pathtrace | 0.03 | 14.7 | 95.05 | 67.77 | 2.62 | 31.7 | 8.01 | 194 | - | 778 | 3.85 |
| 1129 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.02 | 9.1 | 268.53 | 0.61 | 108.72 | 42.4 | 0.15 | 543 | - | 1218 | 6.98 |
| 1130 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.02 | 9.1 | 205.28 | 3.92 | 108.87 | 29.8 | 8.11 | 574 | - | 1293 | 7.41 |
| 1131 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.02 | 9.0 | 272.07 | 2.27 | 110.26 | 42.5 | 0.15 | 535 | - | 1235 | 7.02 |
| 1132 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.02 | 9.0 | 265.95 | 0.82 | 110.19 | 42.5 | 0.15 | 551 | - | 1294 | 7.32 |
| 1133 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.02 | 8.9 | 237.96 | 4.69 | 110.63 | 29.8 | 8.11 | 582 | - | 1252 | 7.27 |
| 1134 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.02 | 8.8 | 150.08 | 12.43 | 112.25 | 29.8 | 8.11 | 567 | - | 1275 | 7.30 |
| 1135 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.02 | 8.6 | 273.06 | 2.26 | 114.41 | 42.5 | 0.15 | 535 | - | 1504 | 8.08 |
| 1136 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.02 | 8.6 | 202.51 | 4.64 | 115.01 | 29.8 | 8.11 | 582 | - | 1278 | 7.38 |
| 1137 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.02 | 8.6 | 276.56 | 0.62 | 114.67 | 42.4 | 0.15 | 543 | - | 1238 | 7.06 |
| 1138 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.02 | 8.6 | 273.37 | 2.26 | 115.03 | 42.5 | 0.15 | 535 | - | 1230 | 7.00 |
| 1139 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.02 | 8.6 | 272.96 | 0.84 | 115.15 | 42.5 | 0.15 | 551 | - | 1204 | 6.96 |
| 1140 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.02 | 8.5 | 150.10 | 12.48 | 115.54 | 29.8 | 8.11 | 567 | - | 1240 | 7.17 |
| 1141 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.5 | 275.99 | 0.61 | 115.79 | 42.4 | 0.15 | 543 | - | 1537 | 8.25 |
| 1142 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.5 | 187.69 | 12.49 | 115.64 | 29.8 | 8.11 | 567 | - | 1533 | 8.32 |
| 1143 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.5 | 235.88 | 4.79 | 116.39 | 29.8 | 8.11 | 582 | - | 1573 | 8.54 |
| 1144 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.5 | 272.34 | 0.82 | 116.35 | 42.5 | 0.15 | 551 | - | 1486 | 8.08 |
| 1145 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 234.36 | 3.91 | 117.06 | 29.8 | 8.11 | 574 | - | 1281 | 7.36 |
| 1146 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 218.53 | 3.99 | 117.06 | 29.8 | 8.11 | 574 | - | 1356 | 7.66 |
| 1147 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 258.58 | 3.88 | 116.72 | 29.8 | 8.11 | 574 | - | 1511 | 8.27 |
| 1148 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 238.15 | 4.78 | 116.86 | 29.8 | 8.11 | 582 | - | 1543 | 8.43 |
| 1149 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 187.85 | 12.48 | 117.23 | 29.8 | 8.11 | 567 | - | 1439 | 7.95 |
| 1150 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.4 | 279.22 | 0.84 | 117.65 | 42.5 | 0.15 | 551 | - | 1320 | 7.42 |
| 1151 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 228.85 | 4.66 | 117.92 | 29.8 | 8.11 | 582 | - | 1329 | 7.58 |
| 1152 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.3 | 157.45 | 12.54 | 118.38 | 29.8 | 8.11 | 567 | - | 1253 | 7.21 |
| 1153 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.3 | 283.85 | 0.61 | 118.39 | 42.4 | 0.15 | 543 | - | 1331 | 7.43 |
| 1154 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.3 | 250.06 | 4.74 | 118.40 | 29.8 | 8.11 | 582 | - | 1301 | 7.47 |
| 1155 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 8.3 | 277.48 | 2.27 | 118.68 | 42.5 | 0.15 | 535 | - | 1263 | 7.13 |
| 1156 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 8.3 | 280.63 | 2.26 | 118.58 | 42.5 | 0.15 | 535 | - | 1487 | 8.02 |
| 1157 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.3 | 278.39 | 0.83 | 118.66 | 42.5 | 0.15 | 551 | - | 1259 | 7.17 |
| 1158 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 8.3 | 285.70 | 2.27 | 118.64 | 42.5 | 0.15 | 535 | - | 1281 | 7.20 |
| 1159 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.2 | 284.92 | 0.61 | 119.55 | 42.4 | 0.15 | 543 | - | 1284 | 7.24 |
| 1160 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.2 | 260.06 | 4.01 | 119.56 | 29.8 | 8.11 | 574 | - | 1334 | 7.57 |
| 1161 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.2 | 207.46 | 12.56 | 119.33 | 29.8 | 8.11 | 567 | - | 1234 | 7.14 |
| 1162 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 8.1 | 299.05 | 0.61 | 122.04 | 42.4 | 0.15 | 543 | - | 1496 | 8.09 |
| 1163 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 231.14 | 3.91 | 122.32 | 29.8 | 8.11 | 574 | - | 1471 | 8.11 |
| 1164 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 167.84 | 12.54 | 122.52 | 29.8 | 8.11 | 567 | - | 1443 | 7.97 |
| 1165 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 262.42 | 4.68 | 122.44 | 29.8 | 8.11 | 582 | - | 1576 | 8.56 |
| 1166 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 8.0 | 302.88 | 2.26 | 123.10 | 42.5 | 0.15 | 535 | - | 1495 | 8.05 |
| 1167 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 7.9 | 300.66 | 0.81 | 124.09 | 42.5 | 0.15 | 551 | - | 1571 | 8.41 |
| 1168 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 7.8 | 292.72 | 0.62 | 125.93 | 42.4 | 0.15 | 543 | - | 1346 | 7.49 |
| 1169 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.8 | 252.80 | 3.91 | 126.11 | 29.8 | 8.11 | 574 | - | 1418 | 7.90 |
| 1170 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 7.8 | 292.29 | 0.83 | 125.64 | 42.5 | 0.15 | 551 | - | 1523 | 8.22 |
| 1171 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 7.8 | 298.97 | 0.86 | 126.07 | 42.5 | 0.15 | 551 | - | 1406 | 7.76 |
| 1172 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.8 | 170.84 | 12.58 | 127.03 | 29.8 | 8.11 | 567 | - | 1338 | 7.55 |
| 1173 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.8 | 228.35 | 4.65 | 127.37 | 29.8 | 8.11 | 582 | - | 1408 | 7.89 |
| 1174 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.8 | 293.68 | 3.90 | 126.86 | 29.8 | 8.11 | 574 | - | 1622 | 8.71 |
| 1175 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + direct | 0.01 | 7.7 | 306.83 | 0.61 | 127.82 | 42.4 | 0.15 | 543 | - | 1546 | 8.28 |
| 1176 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + direct | 0.01 | 7.7 | 301.57 | 2.27 | 128.31 | 42.5 | 0.15 | 535 | - | 1409 | 7.71 |
| 1177 | gpu: heightmap + columns + heightfield + direct (incorrect) | 0.23 | 130.0 | 13.18 | 7.46 | 1.19 | 18.0 | 22.90 | 27 | - | 725 | 2.98 |
| 1178 | cpu: heightmap + columns + heightfield + direct (incorrect) | 0.21 | 119.0 | 13.95 | - | 7.32 | 18.0 | 22.90 | - | - | 341 | 1.35 |
| 1179 | gpu: heightmap + columns + heightfield + pathtrace (incorrect) | 0.06 | 32.9 | 55.02 | 30.13 | 1.25 | 17.6 | 26.16 | 58 | - | 730 | 3.12 |

### terrain-flight [unlit]

1920×1080. camera 2.25 blocks/frame (max 3.0), turn 0.35°/frame; 3985 chunks at start, 11.1 loads/frame; chunk fill 11%, solid 8.5%, surface 0.65%; sky 64%, hits at 268 blocks (median 235); 0.0 edits/frame; 400 lights (animated); 0 mesh triangles.

| # | renderer | rel | fps | frame p99 ms | gpu ms | cpu ms | psnr | bad pct | gpu mb | gpu device mb | peak rss mb | mem rel |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | gpu: bricks + refloor + composite + raycast + pathtrace ◆ | 1.00 | 611.8 | 10.11 | 0.86 | 0.80 | 59.5 | 0.01 | 563 | - | 584 | 4.55 |
| 2 | gpu: tree64 + refloor + composite + raycast + pathtrace ◆ | 0.93 | 567.5 | 9.77 | 1.03 | 1.15 | 59.5 | 0.01 | 126 | - | 677 | 3.19 |
| 3 | gpu: prefix + refloor + composite + raycast + pathtrace | 0.91 | 559.8 | 13.92 | 0.76 | 0.79 | 59.5 | 0.01 | 603 | - | 585 | 4.72 |
| 4 | gpu: hier + refloor + composite + raycast + pathtrace | 0.90 | 550.7 | 13.75 | 0.78 | 0.82 | 59.5 | 0.01 | 563 | - | 586 | 4.56 |
| 5 | gpu: bricks + refloor + beam + composite + raycast + pathtrace | 0.89 | 545.1 | 10.46 | 1.08 | 0.79 | 59.5 | 0.01 | 563 | - | 584 | 4.55 |
| 6 | gpu: prefix + refloor + beam + composite + raycast + pathtrace | 0.87 | 534.3 | 13.67 | 0.96 | 0.81 | 59.5 | 0.01 | 603 | - | 585 | 4.72 |
| 7 | gpu: rows + refloor + composite + raycast + pathtrace | 0.86 | 526.9 | 15.00 | 0.87 | 0.80 | 59.5 | 0.01 | 595 | - | 584 | 4.68 |
| 8 | gpu: hier + refloor + beam + composite + raycast + pathtrace | 0.83 | 507.4 | 13.79 | 1.01 | 0.81 | 59.5 | 0.01 | 563 | - | 587 | 4.56 |
| 9 | gpu: bricks + incremental + composite + raycast + pathtrace | 0.82 | 503.7 | 9.18 | 1.35 | 0.78 | 59.5 | 0.01 | 563 | - | 585 | 4.55 |
| 10 | gpu: prefix + rowrun + composite + raycast + pathtrace | 0.81 | 497.5 | 13.72 | 1.08 | 0.82 | 59.5 | 0.01 | 603 | - | 585 | 4.71 |
| 11 | gpu: svo_dag + refloor + composite + raycast + pathtrace ◆ | 0.80 | 486.8 | 18.08 | 0.98 | 1.47 | 59.5 | 0.01 | 65 | - | 717 | 3.10 |
| 12 | gpu: rows + refloor + beam + composite + raycast + pathtrace | 0.79 | 481.2 | 14.94 | 1.09 | 0.82 | 59.5 | 0.01 | 595 | - | 585 | 4.68 |
| 13 | gpu: clipmap + refloor + composite + raycast + pathtrace | 0.78 | 480.0 | 10.20 | 1.38 | 0.80 | 59.5 | 0.01 | 603 | - | 635 | 4.91 |
| 14 | gpu: distance + sphere + composite + raycast + pathtrace | 0.78 | 479.9 | 20.90 | 0.61 | 0.79 | 59.5 | 0.01 | 819 | - | 585 | 5.57 |
| 15 | gpu: rows + rowrun + composite + raycast + pathtrace | 0.78 | 474.8 | 14.52 | 1.11 | 0.81 | 59.5 | 0.01 | 595 | - | 585 | 4.68 |
| 16 | gpu: bricks + rowrun + composite + raycast + pathtrace | 0.77 | 471.9 | 10.50 | 1.42 | 0.85 | 59.5 | 0.01 | 563 | - | 584 | 4.55 |
| 17 | gpu: distance + refloor + composite + raycast + pathtrace | 0.76 | 466.7 | 21.56 | 0.73 | 0.81 | 59.5 | 0.01 | 819 | - | 584 | 5.57 |
| 18 | gpu: prefix + rowrun + beam + composite + raycast + pathtrace | 0.76 | 465.2 | 12.96 | 1.29 | 0.81 | 59.5 | 0.01 | 603 | - | 586 | 4.72 |
| 19 | gpu: distance + sphere + beam + composite + raycast + pathtrace | 0.76 | 463.0 | 21.28 | 0.83 | 0.79 | 59.5 | 0.01 | 819 | - | 585 | 5.57 |
| 20 | gpu: bricks + incremental + beam + composite + raycast + pathtrace | 0.73 | 449.3 | 10.68 | 1.56 | 0.84 | 59.5 | 0.01 | 563 | - | 585 | 4.56 |
| 21 | gpu: distance + refloor + beam + composite + raycast + pathtrace | 0.73 | 445.7 | 20.88 | 0.95 | 0.79 | 59.5 | 0.01 | 819 | - | 585 | 5.57 |
| 22 | gpu: bricks + rowrun + beam + composite + raycast + pathtrace | 0.73 | 444.4 | 9.18 | 1.63 | 0.85 | 59.5 | 0.01 | 563 | - | 584 | 4.55 |
| 23 | gpu: rows + rowrun + beam + composite + raycast + pathtrace | 0.72 | 440.5 | 14.64 | 1.33 | 0.82 | 59.5 | 0.01 | 595 | - | 585 | 4.68 |
| 24 | gpu: distance + rowrun + composite + raycast + pathtrace | 0.70 | 426.8 | 20.10 | 1.12 | 0.79 | 59.5 | 0.01 | 819 | - | 585 | 5.57 |
| 25 | gpu: hier + incremental + composite + raycast + pathtrace | 0.69 | 424.5 | 13.75 | 1.49 | 0.85 | 59.5 | 0.01 | 563 | - | 586 | 4.56 |
| 26 | gpu: naive@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.69 | 423.3 | 19.04 | 0.63 | 1.57 | 55.2 | 0.01 | 692 | - | 703 | 5.54 |
| 27 | gpu: prefix + incremental + composite + raycast + pathtrace | 0.69 | 420.2 | 11.22 | 1.58 | 0.82 | 59.5 | 0.01 | 603 | - | 585 | 4.71 |
| 28 | gpu: naive@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.68 | 419.0 | 18.63 | 0.70 | 1.57 | 55.0 | 0.02 | 700 | - | 702 | 5.57 |
| 29 | gpu: naive@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.68 | 414.8 | 19.69 | 0.69 | 1.59 | 55.1 | 0.02 | 708 | - | 703 | 5.60 |
| 30 | gpu: rows + incremental + composite + raycast + pathtrace | 0.68 | 413.5 | 13.58 | 1.56 | 0.82 | 59.5 | 0.01 | 595 | - | 585 | 4.68 |
| 31 | gpu: hier + incremental + beam + composite + raycast + pathtrace | 0.66 | 405.8 | 10.36 | 1.66 | 0.85 | 59.5 | 0.01 | 563 | - | 587 | 4.56 |
| 32 | gpu: tree64 + incremental + composite + raycast + pathtrace | 0.66 | 405.4 | 9.27 | 1.86 | 1.27 | 59.5 | 0.01 | 126 | - | 676 | 3.18 |
| 33 | gpu: rows + incremental + beam + composite + raycast + pathtrace | 0.66 | 404.1 | 10.76 | 1.64 | 0.81 | 59.5 | 0.01 | 595 | - | 585 | 4.68 |
| 34 | gpu: tree64 + refloor + beam + composite + raycast + pathtrace | 0.66 | 402.7 | 9.50 | 1.90 | 1.23 | 59.5 | 0.01 | 126 | - | 677 | 3.19 |
| 35 | gpu: clipmap + incremental + composite + raycast + pathtrace | 0.65 | 400.4 | 9.09 | 1.88 | 0.85 | 59.5 | 0.01 | 603 | - | 635 | 4.92 |
| 36 | gpu: distance + rowrun + beam + composite + raycast + pathtrace | 0.65 | 399.5 | 17.05 | 1.31 | 0.80 | 59.5 | 0.01 | 819 | - | 585 | 5.58 |
| 37 | gpu: clipmap + rowrun + composite + raycast + pathtrace | 0.65 | 398.5 | 8.07 | 1.92 | 0.88 | 59.5 | 0.01 | 603 | - | 634 | 4.91 |
| 38 | gpu: prefix + incremental + beam + composite + raycast + pathtrace | 0.65 | 397.5 | 9.40 | 1.75 | 0.85 | 59.5 | 0.01 | 603 | - | 585 | 4.71 |
| 39 | gpu: bricks + sphere + composite + raycast + pathtrace | 0.63 | 388.2 | 7.88 | 2.03 | 0.94 | 59.5 | 0.01 | 563 | - | 584 | 4.55 |
| 40 | gpu: hier + rowrun + composite + raycast + pathtrace | 0.62 | 381.3 | 10.13 | 1.87 | 0.86 | 59.5 | 0.01 | 563 | - | 586 | 4.56 |
| 41 | gpu: hier + sphere + composite + raycast + pathtrace | 0.62 | 381.2 | 10.34 | 1.83 | 0.89 | 59.5 | 0.01 | 563 | - | 586 | 4.56 |
| 42 | gpu: boxes@cpu + rayquery + raycast + pathtrace | 0.62 | 377.0 | 14.60 | 1.33 | 1.43 | 59.5 | 0.01 | 601 | - | 942 | 6.12 |
| 43 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + raster(faces~iterate2) + pathtrace | 0.61 | 374.6 | 7.72 | 2.07 | 0.93 | 53.8 | 0.02 | 745 | - | 601 | 5.34 |
| 44 | gpu: distance + incremental + composite + raycast + pathtrace | 0.61 | 372.9 | 16.01 | 1.50 | 0.83 | 59.5 | 0.01 | 819 | - | 584 | 5.57 |
| 45 | gpu: clipmap + refloor + beam + composite + raycast + pathtrace | 0.60 | 367.8 | 8.06 | 2.18 | 0.92 | 59.5 | 0.01 | 603 | - | 635 | 4.91 |
| 46 | gpu: prefix + sphere + composite + raycast + pathtrace | 0.59 | 363.1 | 9.77 | 2.02 | 0.93 | 59.5 | 0.01 | 603 | - | 584 | 4.71 |
| 47 | gpu: bricks + sphere + beam + composite + raycast + pathtrace | 0.59 | 362.1 | 7.94 | 2.26 | 0.97 | 59.5 | 0.01 | 563 | - | 584 | 4.55 |
| 48 | gpu: hier + sphere + beam + composite + raycast + pathtrace | 0.59 | 359.8 | 10.00 | 2.06 | 0.91 | 59.5 | 0.01 | 563 | - | 586 | 4.56 |
| 49 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + visbuffer(faces~iterate2) + pathtrace | 0.58 | 355.8 | 7.79 | 2.22 | 0.95 | 53.7 | 0.02 | 753 | - | 603 | 5.38 |
| 50 | gpu: hier + rowrun + beam + composite + raycast + pathtrace | 0.58 | 353.2 | 9.99 | 2.12 | 0.90 | 59.5 | 0.01 | 563 | - | 587 | 4.56 |
| 51 | gpu: bricks + refloor + coverage(sky~prev) + quadtree + gbuffer(faces~iterate2) + pathtrace | 0.58 | 352.0 | 8.51 | 2.22 | 0.97 | 53.7 | 0.02 | 760 | - | 602 | 5.41 |
| 52 | gpu: rows + sphere + composite + raycast + pathtrace | 0.57 | 350.7 | 11.25 | 2.05 | 0.94 | 59.5 | 0.01 | 595 | - | 584 | 4.68 |
| 53 | gpu: distance + incremental + beam + composite + raycast + pathtrace | 0.57 | 350.1 | 14.15 | 1.70 | 0.88 | 59.5 | 0.01 | 819 | - | 585 | 5.57 |
| 54 | gpu: clipmap + refloor + quadtree + raster + pathtrace | 0.56 | 345.5 | 9.05 | 2.30 | 0.95 | 53.6 | 0.02 | 785 | - | 651 | 5.70 |
| 55 | gpu: clipmap + refloor + quadtree + visbuffer + pathtrace | 0.56 | 344.8 | 8.36 | 2.34 | 0.94 | 53.5 | 0.02 | 793 | - | 646 | 5.71 |
| 56 | gpu: tree64 + refloor + quadtree + visbuffer + pathtrace | 0.56 | 344.7 | 9.83 | 2.31 | 1.32 | 53.5 | 0.02 | 316 | - | 699 | 4.03 |
| 57 | gpu: prefix + sphere + beam + composite + raycast + pathtrace | 0.56 | 344.0 | 10.09 | 2.23 | 0.97 | 59.5 | 0.01 | 603 | - | 585 | 4.72 |
| 58 | gpu: svo_dag + refloor + beam + composite + raycast + pathtrace | 0.56 | 343.7 | 11.94 | 2.05 | 1.55 | 59.5 | 0.01 | 65 | - | 717 | 3.11 |
| 59 | gpu: clipmap + refloor + quadtree + gbuffer + pathtrace | 0.56 | 342.4 | 8.39 | 2.35 | 0.95 | 53.5 | 0.02 | 801 | - | 645 | 5.74 |
| 60 | gpu: tree64 + refloor + quadtree + gbuffer + pathtrace | 0.55 | 337.5 | 9.29 | 2.39 | 1.35 | 53.5 | 0.02 | 324 | - | 698 | 4.06 |
| 61 | gpu: rows + sphere + beam + composite + raycast + pathtrace | 0.54 | 330.9 | 10.22 | 2.27 | 0.95 | 59.5 | 0.01 | 595 | - | 585 | 4.68 |
| 62 | gpu: tree64 + incremental + beam + composite + raycast + pathtrace | 0.54 | 329.7 | 9.01 | 2.53 | 1.31 | 59.5 | 0.01 | 126 | - | 682 | 3.21 |
| 63 | gpu: greedy@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.53 | 325.9 | 28.27 | 0.27 | 2.33 | 53.2 | 0.02 | 628 | - | 638 | 5.03 |
| 64 | gpu: greedy@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.53 | 324.8 | 27.73 | 0.32 | 2.33 | 53.1 | 0.02 | 636 | - | 638 | 5.06 |
| 65 | gpu: greedy@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.53 | 324.7 | 28.04 | 0.32 | 2.33 | 53.1 | 0.02 | 644 | - | 637 | 5.09 |
| 66 | gpu: clipmap + rowrun + beam + composite + raycast + pathtrace | 0.51 | 314.3 | 8.37 | 2.69 | 1.04 | 59.5 | 0.01 | 603 | - | 635 | 4.92 |
| 67 | gpu: svo_dag + sphere + composite + raycast + pathtrace | 0.51 | 310.3 | 13.35 | 2.43 | 1.65 | 59.5 | 0.01 | 65 | - | 717 | 3.10 |
| 68 | gpu: clipmap + sphere + composite + raycast + pathtrace | 0.51 | 309.3 | 8.57 | 2.76 | 0.99 | 59.5 | 0.01 | 603 | - | 638 | 4.93 |
| 69 | gpu: svo_dag + refloor + quadtree + raster + pathtrace | 0.51 | 309.0 | 12.56 | 2.38 | 1.63 | 53.6 | 0.02 | 247 | - | 738 | 3.91 |
| 70 | gpu: clipmap + incremental + beam + composite + raycast + pathtrace | 0.50 | 307.0 | 8.75 | 2.77 | 1.10 | 59.5 | 0.01 | 603 | - | 640 | 4.93 |
| 71 | gpu: svo_dag + refloor + quadtree + visbuffer + pathtrace | 0.50 | 304.4 | 12.20 | 2.41 | 1.67 | 53.5 | 0.02 | 254 | - | 737 | 3.94 |
| 72 | gpu: svo_dag + refloor + quadtree + gbuffer + pathtrace | 0.50 | 303.7 | 13.07 | 2.44 | 1.66 | 53.5 | 0.02 | 262 | - | 738 | 3.97 |
| 73 | gpu: tree64 + refloor + quadtree + raster + pathtrace | 0.49 | 300.7 | 9.66 | 2.78 | 1.32 | 53.6 | 0.02 | 308 | - | 699 | 4.00 |
| 74 | gpu: clipmap + rowrun + quadtree + raster + pathtrace | 0.49 | 297.2 | 8.36 | 2.86 | 1.01 | 53.6 | 0.02 | 785 | - | 646 | 5.68 |
| 75 | gpu: clipmap + incremental + quadtree + raster + pathtrace | 0.48 | 293.6 | 8.26 | 2.92 | 1.06 | 53.6 | 0.02 | 785 | - | 651 | 5.70 |
| 76 | gpu: clipmap + rowrun + quadtree + gbuffer + pathtrace | 0.48 | 292.9 | 8.67 | 2.90 | 1.05 | 53.5 | 0.02 | 801 | - | 651 | 5.76 |
| 77 | gpu: clipmap + rowrun + quadtree + visbuffer + pathtrace | 0.48 | 291.8 | 8.79 | 2.93 | 1.07 | 53.5 | 0.02 | 793 | - | 650 | 5.73 |
| 78 | gpu: clipmap + incremental + quadtree + visbuffer + pathtrace | 0.47 | 289.8 | 8.51 | 2.98 | 1.06 | 53.5 | 0.02 | 793 | - | 648 | 5.72 |
| 79 | gpu: clipmap + incremental + quadtree + gbuffer + pathtrace | 0.47 | 285.8 | 8.62 | 3.00 | 1.06 | 53.5 | 0.02 | 801 | - | 651 | 5.76 |
| 80 | gpu: tree64 + incremental + quadtree + gbuffer + pathtrace | 0.45 | 274.1 | 8.58 | 3.20 | 1.43 | 53.5 | 0.02 | 324 | - | 699 | 4.06 |
| 81 | gpu: tree64 + incremental + quadtree + visbuffer + pathtrace | 0.45 | 273.0 | 8.35 | 3.21 | 1.43 | 53.5 | 0.02 | 316 | - | 699 | 4.03 |
| 82 | gpu: tree64 + incremental + quadtree + raster + pathtrace | 0.44 | 271.9 | 8.99 | 3.24 | 1.42 | 53.6 | 0.02 | 308 | - | 704 | 4.02 |
| 83 | gpu: svo_dag + incremental + composite + raycast + pathtrace | 0.44 | 269.9 | 12.50 | 3.01 | 1.70 | 59.5 | 0.01 | 65 | - | 717 | 3.10 |
| 84 | gpu: tree64 + sphere + composite + raycast + pathtrace | 0.44 | 266.7 | 8.54 | 3.39 | 1.40 | 59.5 | 0.01 | 126 | - | 681 | 3.20 |
| 85 | gpu: clipmap + sphere + quadtree + raster + pathtrace | 0.42 | 258.3 | 8.91 | 3.46 | 1.11 | 53.6 | 0.02 | 785 | - | 651 | 5.70 |
| 86 | gpu: clipmap + sphere + quadtree + gbuffer + pathtrace | 0.42 | 255.8 | 8.83 | 3.49 | 1.11 | 53.5 | 0.02 | 801 | - | 655 | 5.78 |
| 87 | gpu: clipmap + sphere + quadtree + visbuffer + pathtrace | 0.42 | 255.3 | 9.16 | 3.49 | 1.13 | 53.5 | 0.02 | 793 | - | 650 | 5.73 |
| 88 | gpu: svo_dag + rowrun + composite + raycast + pathtrace | 0.42 | 254.4 | 12.03 | 3.26 | 1.74 | 59.5 | 0.01 | 65 | - | 721 | 3.12 |
| 89 | gpu: clipmap + sphere + beam + composite + raycast + pathtrace | 0.41 | 251.9 | 9.28 | 3.63 | 1.15 | 59.5 | 0.01 | 603 | - | 638 | 4.93 |
| 90 | gpu: svo_dag + sphere + beam + composite + raycast + pathtrace | 0.40 | 244.3 | 12.77 | 3.43 | 1.75 | 59.5 | 0.01 | 65 | - | 721 | 3.12 |
| 91 | gpu: svo_dag + incremental + beam + composite + raycast + pathtrace | 0.38 | 231.3 | 12.13 | 3.72 | 1.77 | 59.5 | 0.01 | 65 | - | 721 | 3.12 |
| 92 | gpu: tree64 + rowrun + composite + raycast + pathtrace | 0.38 | 230.3 | 8.91 | 4.04 | 1.46 | 59.5 | 0.01 | 126 | - | 703 | 3.29 |
| 93 | gpu: svo_dag + sphere + quadtree + raster + pathtrace | 0.37 | 229.0 | 12.40 | 3.68 | 1.78 | 53.6 | 0.02 | 247 | - | 792 | 4.12 |
| 94 | gpu: svo_dag + sphere + quadtree + gbuffer + pathtrace | 0.37 | 226.2 | 12.67 | 3.73 | 1.76 | 53.5 | 0.02 | 262 | - | 797 | 4.21 |
| 95 | gpu: svo_dag + sphere + quadtree + visbuffer + pathtrace | 0.37 | 225.1 | 12.50 | 3.76 | 1.78 | 53.5 | 0.02 | 254 | - | 797 | 4.18 |
| 96 | gpu: tree64 + sphere + quadtree + visbuffer + pathtrace | 0.37 | 223.6 | 9.38 | 4.10 | 1.52 | 53.5 | 0.02 | 316 | - | 761 | 4.28 |
| 97 | gpu: tree64 + sphere + quadtree + gbuffer + pathtrace | 0.36 | 221.5 | 9.12 | 4.15 | 1.51 | 53.5 | 0.02 | 324 | - | 762 | 4.31 |
| 98 | gpu: tree64 + sphere + quadtree + raster + pathtrace | 0.36 | 220.8 | 8.94 | 4.18 | 1.49 | 53.6 | 0.02 | 308 | - | 761 | 4.24 |
| 99 | gpu: tree64 + sphere + beam + composite + raycast + pathtrace | 0.36 | 218.9 | 9.27 | 4.27 | 1.46 | 59.5 | 0.01 | 126 | - | 734 | 3.42 |
| 100 | gpu: svo_dag + rowrun + beam + composite + raycast + pathtrace | 0.34 | 209.4 | 12.13 | 4.22 | 1.80 | 59.5 | 0.01 | 65 | - | 771 | 3.32 |
| 101 | gpu: svo_dag + incremental + quadtree + visbuffer + pathtrace | 0.33 | 204.8 | 12.42 | 4.27 | 1.81 | 53.5 | 0.02 | 254 | - | 801 | 4.19 |
| 102 | gpu: svo_dag + incremental + quadtree + raster + pathtrace | 0.33 | 204.4 | 13.50 | 4.26 | 1.80 | 53.6 | 0.02 | 247 | - | 801 | 4.16 |
| 103 | gpu: svo_dag + incremental + quadtree + gbuffer + pathtrace | 0.33 | 203.9 | 12.11 | 4.27 | 1.80 | 53.5 | 0.02 | 262 | - | 800 | 4.22 |
| 104 | gpu: tree64 + rowrun + beam + composite + raycast + pathtrace | 0.32 | 194.1 | 9.51 | 4.92 | 1.49 | 59.5 | 0.01 | 126 | - | 747 | 3.47 |
| 105 | gpu: distance + sphere + quadtree + raster + pathtrace | 0.32 | 193.5 | 15.42 | 4.30 | 1.13 | 53.6 | 0.02 | 1001 | - | 668 | 6.63 |
| 106 | gpu: prefix + refloor + quadtree + raster + pathtrace | 0.31 | 192.0 | 11.31 | 4.76 | 1.17 | 53.6 | 0.02 | 785 | - | 668 | 5.77 |
| 107 | gpu: distance + sphere + quadtree + visbuffer + pathtrace | 0.31 | 191.5 | 15.13 | 4.38 | 1.14 | 53.5 | 0.02 | 1009 | - | 677 | 6.69 |
| 108 | gpu: distance + sphere + quadtree + gbuffer + pathtrace | 0.31 | 191.4 | 14.70 | 4.39 | 1.15 | 53.5 | 0.02 | 1016 | - | 681 | 6.74 |
| 109 | gpu: prefix + refloor + quadtree + gbuffer + pathtrace | 0.31 | 190.2 | 10.73 | 4.85 | 1.09 | 53.5 | 0.02 | 800 | - | 675 | 5.86 |
| 110 | gpu: hier + refloor + quadtree + raster + pathtrace | 0.31 | 188.9 | 11.78 | 4.85 | 1.22 | 53.6 | 0.02 | 745 | - | 693 | 5.71 |
| 111 | gpu: prefix + refloor + quadtree + visbuffer + pathtrace | 0.31 | 188.7 | 11.61 | 4.85 | 1.17 | 53.5 | 0.02 | 793 | - | 677 | 5.84 |
| 112 | gpu: hier + refloor + quadtree + visbuffer + pathtrace | 0.31 | 187.0 | 10.96 | 4.94 | 1.15 | 53.5 | 0.02 | 753 | - | 692 | 5.74 |
| 113 | gpu: hier + refloor + quadtree + gbuffer + pathtrace | 0.30 | 186.2 | 11.26 | 4.94 | 1.18 | 53.5 | 0.02 | 760 | - | 695 | 5.78 |
| 114 | gpu: greedy@cpu + rayquery + raycast + pathtrace | 0.30 | 184.3 | 49.20 | 0.82 | 3.90 | 59.5 | 0.01 | 389 | - | 1361 | 6.95 |
| 115 | gpu: bricks + refloor + quadtree + gbuffer + pathtrace | 0.30 | 183.9 | 10.41 | 5.13 | 1.19 | 53.5 | 0.02 | 760 | - | 703 | 5.81 |
| 116 | gpu: bricks + refloor + quadtree + visbuffer + pathtrace | 0.30 | 183.6 | 10.44 | 5.14 | 1.22 | 53.5 | 0.02 | 753 | - | 708 | 5.80 |
| 117 | gpu: distance + refloor + quadtree + raster + pathtrace | 0.30 | 183.4 | 15.12 | 4.68 | 1.13 | 53.6 | 0.02 | 1001 | - | 696 | 6.74 |
| 118 | gpu: rows + refloor + quadtree + raster + pathtrace | 0.30 | 181.5 | 12.18 | 5.07 | 1.11 | 53.6 | 0.02 | 777 | - | 707 | 5.89 |
| 119 | gpu: distance + refloor + quadtree + visbuffer + pathtrace | 0.30 | 180.7 | 15.61 | 4.77 | 1.23 | 53.5 | 0.02 | 1009 | - | 705 | 6.80 |
| 120 | gpu: distance + refloor + quadtree + gbuffer + pathtrace | 0.29 | 180.1 | 15.56 | 4.77 | 1.19 | 53.5 | 0.02 | 1016 | - | 697 | 6.80 |
| 121 | gpu: rows + refloor + quadtree + visbuffer + pathtrace | 0.29 | 178.4 | 11.83 | 5.17 | 1.18 | 53.5 | 0.02 | 785 | - | 705 | 5.91 |
| 122 | gpu: rows + refloor + quadtree + gbuffer + pathtrace | 0.29 | 178.3 | 11.67 | 5.17 | 1.19 | 53.5 | 0.02 | 792 | - | 704 | 5.94 |
| 123 | gpu: svo_dag + rowrun + quadtree + raster + pathtrace | 0.29 | 175.4 | 12.63 | 5.15 | 1.85 | 53.6 | 0.02 | 247 | - | 805 | 4.18 |
| 124 | gpu: points@cpu + splat + bricks + refloor + pathtrace | 0.29 | 175.3 | 46.88 | 1.81 | 0.85 | 59.5 | 0.01 | 833 | - | 880 | 6.80 |
| 125 | gpu: svo_dag + rowrun + quadtree + gbuffer + pathtrace | 0.29 | 174.5 | 12.56 | 5.19 | 1.86 | 53.5 | 0.02 | 262 | - | 808 | 4.25 |
| 126 | gpu: svo_dag + rowrun + quadtree + visbuffer + pathtrace | 0.28 | 173.0 | 13.73 | 5.24 | 1.85 | 53.5 | 0.02 | 254 | - | 808 | 4.22 |
| 127 | gpu: cubes@cpu + faces + bricks + refloor + depth + raster + pathtrace | 0.28 | 170.8 | 11.64 | 5.38 | 1.62 | 55.1 | 0.01 | 692 | - | 675 | 5.43 |
| 128 | gpu: cubes@cpu + faces + bricks + refloor + depth + visbuffer + pathtrace | 0.28 | 170.3 | 11.80 | 5.42 | 1.60 | 55.0 | 0.02 | 700 | - | 671 | 5.44 |
| 129 | gpu: cubes@cpu + faces + bricks + refloor + depth + gbuffer + pathtrace | 0.28 | 169.2 | 12.10 | 5.40 | 1.64 | 55.1 | 0.02 | 708 | - | 734 | 5.72 |
| 130 | gpu: naive@cpu + rayquery + raycast + pathtrace | 0.27 | 165.2 | 57.04 | 1.31 | 3.81 | 59.5 | 0.01 | 951 | - | 1651 | 10.33 |
| 131 | gpu: rows + rowrun + quadtree + raster + pathtrace | 0.26 | 159.2 | 11.61 | 5.91 | 1.14 | 53.6 | 0.02 | 777 | - | 703 | 5.87 |
| 132 | gpu: prefix + rowrun + quadtree + raster + pathtrace | 0.26 | 156.7 | 12.03 | 6.04 | 1.23 | 53.6 | 0.02 | 785 | - | 696 | 5.88 |
| 133 | gpu: distance + rowrun + quadtree + raster + pathtrace | 0.26 | 156.6 | 15.02 | 5.80 | 1.16 | 53.6 | 0.02 | 1001 | - | 702 | 6.76 |
| 134 | gpu: rows + rowrun + quadtree + gbuffer + pathtrace | 0.26 | 156.5 | 12.41 | 6.02 | 1.19 | 53.5 | 0.02 | 792 | - | 707 | 5.95 |
| 135 | gpu: rows + rowrun + quadtree + visbuffer + pathtrace | 0.26 | 156.3 | 11.88 | 6.04 | 1.23 | 53.5 | 0.02 | 785 | - | 705 | 5.91 |
| 136 | gpu: prefix + rowrun + quadtree + gbuffer + pathtrace | 0.25 | 154.2 | 11.69 | 6.15 | 1.25 | 53.5 | 0.02 | 800 | - | 705 | 5.98 |
| 137 | gpu: distance + rowrun + quadtree + gbuffer + pathtrace | 0.25 | 153.9 | 14.93 | 5.91 | 1.18 | 53.5 | 0.02 | 1016 | - | 705 | 6.83 |
| 138 | gpu: distance + rowrun + quadtree + visbuffer + pathtrace | 0.25 | 153.7 | 15.14 | 5.91 | 1.23 | 53.5 | 0.02 | 1009 | - | 705 | 6.80 |
| 139 | gpu: prefix + rowrun + quadtree + visbuffer + pathtrace | 0.25 | 153.4 | 11.50 | 6.17 | 1.21 | 53.5 | 0.02 | 793 | - | 702 | 5.93 |
| 140 | gpu: bricks + refloor + quadtree + raster + pathtrace | 0.24 | 148.9 | 24.09 | 6.38 | 1.20 | 53.6 | 0.02 | 745 | - | 713 | 5.79 |
| 141 | gpu: tree64 + rowrun + quadtree + raster + pathtrace | 0.24 | 148.0 | 11.58 | 6.56 | 1.59 | 53.6 | 0.02 | 308 | - | 808 | 4.43 |
| 142 | gpu: tree64 + rowrun + quadtree + visbuffer + pathtrace | 0.24 | 147.0 | 11.37 | 6.60 | 1.58 | 53.5 | 0.02 | 316 | - | 816 | 4.49 |
| 143 | gpu: tree64 + rowrun + quadtree + gbuffer + pathtrace | 0.24 | 146.8 | 11.57 | 6.62 | 1.58 | 53.5 | 0.02 | 324 | - | 807 | 4.49 |
| 144 | gpu: rows + incremental + quadtree + raster + pathtrace | 0.23 | 141.3 | 12.23 | 6.75 | 1.23 | 53.6 | 0.02 | 777 | - | 708 | 5.89 |
| 145 | gpu: rows + incremental + quadtree + visbuffer + pathtrace | 0.23 | 139.0 | 12.60 | 6.86 | 1.25 | 53.5 | 0.02 | 785 | - | 710 | 5.93 |
| 146 | gpu: rows + incremental + quadtree + gbuffer + pathtrace | 0.23 | 138.7 | 12.82 | 6.87 | 1.26 | 53.5 | 0.02 | 792 | - | 708 | 5.96 |
| 147 | gpu: hier + incremental + quadtree + raster + pathtrace | 0.22 | 137.4 | 12.36 | 6.96 | 1.25 | 53.6 | 0.02 | 745 | - | 709 | 5.77 |
| 148 | gpu: hier + incremental + quadtree + gbuffer + pathtrace | 0.22 | 136.9 | 12.18 | 6.99 | 1.27 | 53.5 | 0.02 | 760 | - | 708 | 5.83 |
| 149 | gpu: hier + incremental + quadtree + visbuffer + pathtrace | 0.22 | 136.8 | 12.42 | 7.01 | 1.18 | 53.5 | 0.02 | 753 | - | 718 | 5.84 |
| 150 | gpu: bricks + incremental + quadtree + raster + pathtrace | 0.22 | 133.7 | 12.90 | 7.22 | 1.27 | 53.6 | 0.02 | 745 | - | 709 | 5.77 |
| 151 | gpu: prefix + incremental + quadtree + raster + pathtrace | 0.22 | 132.7 | 12.73 | 7.22 | 1.25 | 53.6 | 0.02 | 785 | - | 708 | 5.93 |
| 152 | gpu: bricks + incremental + quadtree + gbuffer + pathtrace | 0.22 | 132.2 | 12.52 | 7.30 | 1.30 | 53.5 | 0.02 | 760 | - | 711 | 5.84 |
| 153 | gpu: bricks + incremental + quadtree + visbuffer + pathtrace | 0.22 | 132.0 | 12.81 | 7.32 | 1.27 | 53.5 | 0.02 | 753 | - | 706 | 5.79 |
| 154 | gpu: prefix + incremental + quadtree + visbuffer + pathtrace | 0.22 | 131.8 | 12.57 | 7.29 | 1.23 | 53.5 | 0.02 | 793 | - | 712 | 5.97 |
| 155 | gpu: prefix + incremental + quadtree + gbuffer + pathtrace | 0.22 | 131.7 | 12.37 | 7.29 | 1.26 | 53.5 | 0.02 | 800 | - | 713 | 6.01 |
| 156 | gpu: distance + incremental + quadtree + raster + pathtrace | 0.21 | 125.9 | 15.58 | 7.52 | 1.22 | 53.6 | 0.02 | 1001 | - | 725 | 6.85 |
| 157 | gpu: bricks + rowrun + quadtree + raster + pathtrace | 0.21 | 125.6 | 13.20 | 7.71 | 1.26 | 53.6 | 0.02 | 745 | - | 712 | 5.78 |
| 158 | gpu: distance + incremental + quadtree + visbuffer + pathtrace | 0.20 | 125.2 | 15.01 | 7.60 | 1.20 | 53.5 | 0.02 | 1009 | - | 727 | 6.89 |
| 159 | gpu: hier + sphere + quadtree + raster + pathtrace | 0.20 | 125.2 | 13.34 | 7.69 | 1.27 | 53.6 | 0.02 | 745 | - | 736 | 5.88 |
| 160 | gpu: distance + incremental + quadtree + gbuffer + pathtrace | 0.20 | 125.1 | 15.40 | 7.58 | 1.22 | 53.5 | 0.02 | 1016 | - | 731 | 6.94 |
| 161 | gpu: hier + sphere + quadtree + gbuffer + pathtrace | 0.20 | 124.2 | 12.90 | 7.76 | 1.27 | 53.5 | 0.02 | 760 | - | 735 | 5.94 |
| 162 | gpu: hier + sphere + quadtree + visbuffer + pathtrace | 0.20 | 124.0 | 13.43 | 7.77 | 1.24 | 53.5 | 0.02 | 753 | - | 735 | 5.91 |
| 163 | gpu: bricks + rowrun + quadtree + gbuffer + pathtrace | 0.20 | 123.4 | 13.77 | 7.85 | 1.28 | 53.5 | 0.02 | 760 | - | 722 | 5.89 |
| 164 | gpu: bricks + rowrun + quadtree + visbuffer + pathtrace | 0.20 | 123.2 | 13.57 | 7.86 | 1.29 | 53.5 | 0.02 | 753 | - | 722 | 5.86 |
| 165 | gpu: bricks + sphere + quadtree + raster + pathtrace | 0.20 | 120.3 | 13.68 | 8.06 | 1.29 | 53.6 | 0.02 | 745 | - | 727 | 5.84 |
| 166 | gpu: bricks + sphere + quadtree + visbuffer + pathtrace | 0.19 | 119.1 | 13.79 | 8.15 | 1.25 | 53.5 | 0.02 | 753 | - | 724 | 5.86 |
| 167 | gpu: rows + sphere + quadtree + raster + pathtrace | 0.19 | 119.1 | 13.49 | 8.08 | 1.21 | 53.6 | 0.02 | 777 | - | 731 | 5.99 |
| 168 | gpu: prefix + sphere + quadtree + raster + pathtrace | 0.19 | 118.8 | 13.87 | 8.13 | 1.24 | 53.6 | 0.02 | 785 | - | 731 | 6.02 |
| 169 | gpu: bricks + sphere + quadtree + gbuffer + pathtrace | 0.19 | 118.7 | 13.57 | 8.17 | 1.25 | 53.5 | 0.02 | 760 | - | 729 | 5.91 |
| 170 | gpu: prefix + sphere + quadtree + gbuffer + pathtrace | 0.19 | 117.7 | 13.73 | 8.20 | 1.18 | 53.5 | 0.02 | 800 | - | 734 | 6.09 |
| 171 | gpu: prefix + sphere + quadtree + visbuffer + pathtrace | 0.19 | 117.4 | 13.87 | 8.22 | 1.23 | 53.5 | 0.02 | 793 | - | 735 | 6.06 |
| 172 | gpu: rows + sphere + quadtree + gbuffer + pathtrace | 0.19 | 117.3 | 14.07 | 8.23 | 1.28 | 53.5 | 0.02 | 792 | - | 728 | 6.04 |
| 173 | gpu: rows + sphere + quadtree + visbuffer + pathtrace | 0.19 | 117.3 | 13.60 | 8.22 | 1.26 | 53.5 | 0.02 | 785 | - | 735 | 6.04 |
| 174 | gpu: hier + rowrun + quadtree + raster + pathtrace | 0.19 | 114.7 | 13.93 | 8.44 | 1.26 | 53.6 | 0.02 | 745 | - | 731 | 5.86 |
| 175 | gpu: hier + rowrun + quadtree + gbuffer + pathtrace | 0.19 | 113.3 | 14.54 | 8.55 | 1.24 | 53.5 | 0.02 | 760 | - | 736 | 5.94 |
| 176 | gpu: hier + rowrun + quadtree + visbuffer + pathtrace | 0.18 | 113.1 | 14.43 | 8.55 | 1.25 | 53.5 | 0.02 | 753 | - | 737 | 5.92 |
| 177 | cpu: hier + refloor + raycast + direct ◆ | 0.06 | 39.5 | 39.16 | - | 23.81 | 63.2 | 0.00 | - | - | 340 | 1.35 |
| 178 | cpu: hier + incremental + raycast + direct | 0.06 | 37.2 | 38.53 | - | 25.32 | 63.2 | 0.00 | - | - | 340 | 1.35 |
| 179 | cpu: prefix + incremental + raycast + direct | 0.06 | 36.7 | 40.43 | - | 25.58 | 63.1 | 0.00 | - | - | 415 | 1.65 |
| 180 | cpu: bits + refloor + raycast + direct ◆ | 0.06 | 35.1 | 41.34 | - | 26.89 | 63.2 | 0.00 | - | - | 339 | 1.35 |
| 181 | cpu: greedy + faces + raster + bricks + refloor + direct | 0.06 | 35.0 | 51.00 | - | 27.27 | 67.2 | 0.00 | - | - | 521 | 2.07 |
| 182 | cpu: bricks + refloor + raycast + direct ◆ | 0.06 | 34.9 | 41.13 | - | 27.46 | 63.2 | 0.00 | - | - | 324 | 1.29 |
| 183 | cpu: prefix + refloor + raycast + direct | 0.06 | 34.8 | 45.50 | - | 27.08 | 63.2 | 0.00 | - | - | 404 | 1.60 |
| 184 | cpu: u8 + refloor + raycast + direct ◆ | 0.06 | 33.9 | 42.62 | - | 28.11 | 63.2 | 0.00 | - | - | 257 | 1.02 |
| 185 | cpu: brickmajor + refloor + raycast + direct | 0.06 | 33.9 | 42.82 | - | 28.01 | 63.2 | 0.00 | - | - | 448 | 1.78 |
| 186 | cpu: morton + refloor + raycast + direct | 0.06 | 33.8 | 43.24 | - | 28.05 | 63.2 | 0.00 | - | - | 448 | 1.78 |
| 187 | cpu: bricks + incremental + raycast + direct | 0.05 | 33.2 | 42.52 | - | 28.87 | 63.2 | 0.00 | - | - | 324 | 1.29 |
| 188 | cpu: palette + refloor + raycast + direct ◆ | 0.05 | 33.1 | 45.23 | - | 28.35 | 63.2 | 0.00 | - | - | 252 | 1.00 |
| 189 | cpu: bits + incremental + raycast + direct | 0.05 | 33.1 | 42.01 | - | 28.62 | 63.2 | 0.00 | - | - | 340 | 1.35 |
| 190 | cpu: u8 + incremental + raycast + direct | 0.05 | 33.0 | 42.77 | - | 28.95 | 63.2 | 0.00 | - | - | 257 | 1.02 |
| 191 | cpu: brickmajor + incremental + raycast + direct | 0.05 | 32.8 | 43.73 | - | 28.98 | 63.2 | 0.00 | - | - | 449 | 1.78 |
| 192 | cpu: morton + incremental + raycast + direct | 0.05 | 32.6 | 43.85 | - | 29.01 | 63.2 | 0.00 | - | - | 448 | 1.78 |
| 193 | cpu: palette + incremental + raycast + direct ◆ | 0.05 | 32.1 | 47.02 | - | 29.29 | 63.2 | 0.00 | - | - | 252 | 1.00 |
| 194 | cpu: naive + faces + raster + bricks + refloor + direct | 0.03 | 20.7 | 67.19 | - | 46.97 | 67.9 | 0.00 | - | - | 836 | 3.32 |
| 195 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.5 | 137.25 | 0.26 | 55.27 | 44.8 | 0.15 | 582 | - | 1996 | 10.24 |
| 196 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.3 | 136.53 | 0.23 | 55.66 | 44.7 | 0.15 | 574 | - | 2007 | 10.25 |
| 197 | gpu: hier@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 17.2 | 137.70 | 0.20 | 56.14 | 44.8 | 0.15 | 567 | - | 1979 | 10.11 |
| 198 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.8 | 142.12 | 0.26 | 57.38 | 44.8 | 0.15 | 582 | - | 1950 | 10.06 |
| 199 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.8 | 142.68 | 0.25 | 57.42 | 44.8 | 0.15 | 582 | - | 2135 | 10.79 |
| 200 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 143.85 | 0.26 | 58.08 | 44.8 | 0.15 | 582 | - | 1877 | 9.77 |
| 201 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 144.31 | 0.23 | 58.05 | 44.7 | 0.15 | 574 | - | 2050 | 10.42 |
| 202 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 141.37 | 0.23 | 57.94 | 44.7 | 0.15 | 574 | - | 1953 | 10.04 |
| 203 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 143.32 | 0.20 | 58.01 | 44.8 | 0.15 | 567 | - | 1920 | 9.87 |
| 204 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 138.60 | 0.24 | 57.43 | 44.7 | 0.15 | 574 | - | 1997 | 10.21 |
| 205 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 140.53 | 0.20 | 58.02 | 44.8 | 0.15 | 567 | - | 2028 | 10.30 |
| 206 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.7 | 139.61 | 0.20 | 58.05 | 44.8 | 0.15 | 567 | - | 2231 | 11.11 |
| 207 | gpu: morton@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.6 | 139.79 | 0.24 | 57.79 | 44.7 | 0.15 | 574 | - | 2078 | 10.53 |
| 208 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.6 | 142.43 | 0.24 | 58.20 | 44.7 | 0.15 | 574 | - | 2189 | 10.97 |
| 209 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.6 | 142.89 | 0.20 | 58.45 | 44.8 | 0.15 | 567 | - | 1857 | 9.62 |
| 210 | gpu: bits@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.6 | 143.47 | 0.25 | 58.39 | 44.8 | 0.15 | 582 | - | 1961 | 10.10 |
| 211 | gpu: prefix@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.6 | 141.94 | 0.25 | 58.28 | 44.8 | 0.15 | 582 | - | 2135 | 10.79 |
| 212 | gpu: bricks@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.6 | 145.23 | 0.24 | 58.60 | 44.7 | 0.15 | 574 | - | 1895 | 9.81 |
| 213 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.5 | 140.97 | 0.20 | 58.52 | 44.8 | 0.15 | 567 | - | 2268 | 11.26 |
| 214 | gpu: u8@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.5 | 144.41 | 0.20 | 58.73 | 44.8 | 0.15 | 567 | - | 1943 | 9.97 |
| 215 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.5 | 141.80 | 0.23 | 58.58 | 44.7 | 0.15 | 574 | - | 2128 | 10.73 |
| 216 | gpu: brickmajor@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 16.4 | 140.38 | 0.25 | 58.85 | 44.8 | 0.15 | 582 | - | 2217 | 11.11 |
| 217 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.03 | 15.7 | 145.64 | 0.25 | 61.26 | 44.8 | 0.15 | 582 | - | 1918 | 9.93 |
| 218 | gpu: palette@cpu + refloor@cpu + quadtree@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.02 | 14.3 | 167.92 | 0.20 | 67.47 | 44.8 | 0.15 | 567 | - | 2015 | 10.25 |
| 219 | gpu: hier@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.5 | 280.62 | 0.20 | 116.42 | 44.9 | 0.15 | 567 | - | 1161 | 6.86 |
| 220 | gpu: hier@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 281.85 | 0.24 | 117.00 | 44.9 | 0.15 | 574 | - | 1255 | 7.26 |
| 221 | gpu: hier@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.4 | 290.30 | 0.25 | 117.75 | 44.9 | 0.15 | 582 | - | 1234 | 7.21 |
| 222 | gpu: palette@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.3 | 273.54 | 0.26 | 118.58 | 44.9 | 0.15 | 582 | - | 1353 | 7.68 |
| 223 | gpu: palette@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.3 | 277.44 | 0.24 | 118.93 | 44.9 | 0.15 | 574 | - | 1249 | 7.24 |
| 224 | gpu: palette@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.3 | 284.97 | 0.20 | 119.19 | 44.9 | 0.15 | 567 | - | 1238 | 7.17 |
| 225 | gpu: u8@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 291.32 | 0.24 | 121.64 | 44.9 | 0.15 | 574 | - | 1413 | 7.89 |
| 226 | gpu: u8@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 294.94 | 0.20 | 122.19 | 44.9 | 0.15 | 567 | - | 1392 | 7.78 |
| 227 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 293.82 | 0.20 | 122.22 | 44.9 | 0.15 | 567 | - | 1259 | 7.25 |
| 228 | gpu: u8@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 295.07 | 0.26 | 122.49 | 44.9 | 0.15 | 582 | - | 1313 | 7.52 |
| 229 | gpu: morton@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 290.59 | 0.20 | 122.26 | 44.9 | 0.15 | 567 | - | 1486 | 8.15 |
| 230 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.1 | 296.31 | 0.24 | 122.53 | 44.9 | 0.15 | 574 | - | 1227 | 7.15 |
| 231 | gpu: morton@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.0 | 293.37 | 0.23 | 122.63 | 44.9 | 0.15 | 574 | - | 1507 | 8.27 |
| 232 | gpu: morton@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.0 | 294.27 | 0.25 | 122.62 | 44.9 | 0.15 | 582 | - | 1478 | 8.18 |
| 233 | gpu: bricks@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 8.0 | 290.51 | 0.26 | 123.53 | 44.9 | 0.15 | 582 | - | 1243 | 7.25 |
| 234 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.9 | 295.87 | 0.25 | 124.60 | 44.9 | 0.15 | 582 | - | 1453 | 8.08 |
| 235 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.9 | 297.43 | 0.20 | 124.98 | 44.9 | 0.15 | 567 | - | 1503 | 8.22 |
| 236 | gpu: brickmajor@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.9 | 296.54 | 0.24 | 125.19 | 44.9 | 0.15 | 574 | - | 1529 | 8.35 |
| 237 | gpu: bits@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.8 | 301.66 | 0.20 | 126.84 | 44.9 | 0.15 | 567 | - | 1252 | 7.22 |
| 238 | gpu: bits@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.6 | 311.24 | 0.24 | 130.02 | 44.9 | 0.15 | 574 | - | 1197 | 7.04 |
| 239 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + visbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.6 | 319.98 | 0.23 | 130.81 | 44.9 | 0.15 | 574 | - | 1514 | 8.29 |
| 240 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + raster(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.6 | 323.40 | 0.20 | 130.91 | 44.9 | 0.15 | 567 | - | 1492 | 8.17 |
| 241 | gpu: bits@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.5 | 312.66 | 0.26 | 130.97 | 44.9 | 0.15 | 582 | - | 1242 | 7.24 |
| 242 | gpu: prefix@cpu + refloor@cpu + cluster@cpu + gbuffer(faces~prev) + bricks + refloor + pathtrace | 0.01 | 7.5 | 319.93 | 0.25 | 131.19 | 44.9 | 0.15 | 582 | - | 1502 | 8.28 |
| 243 | gpu: heightmap + columns + heightfield + pathtrace (incorrect) | 0.95 | 580.3 | 9.95 | 0.92 | 0.78 | 20.6 | 20.50 | 58 | - | 665 | 2.87 |
| 244 | cpu: heightmap + columns + heightfield + direct (incorrect) | 0.56 | 344.3 | 5.13 | - | 2.05 | 20.6 | 20.50 | - | - | 334 | 1.32 |

