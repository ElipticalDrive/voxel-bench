// The common packed record's decoding for the deferred pass (`deferred.wgsl`): records
// written by `pack_hit` (ray walks, `gbuffer`, columns). The surface is the one
// `record_surface` rebuilds, passed through the back end's `view_surface(s)` (a strategy
// with a better normal than the face's sets it there: columns' heightmap slope; others
// return `s`).

fn record_hit(r: vec2<u32>) -> bool { return r.y != 0u; }
fn record_decode(r: vec2<u32>, d: vec3<f32>) -> Surface { return view_surface(record_surface(r, view_origin(), d)); }
fn record_view(r: vec2<u32>, color: vec4<f32>) -> vec4<f32> { return color; }

#ifdef MESHES
fn record_is_mesh(r: vec2<u32>) -> bool { return record_is_mesh_hit(r); }
fn record_mesh(r: vec2<u32>, d: vec3<f32>) -> MeshSurface { return mesh_surface(record_mesh_tri(r), view_origin(), d, bitcast<f32>(r.x)); }
#endif
