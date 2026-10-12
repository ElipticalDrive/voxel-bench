//! Visibility strategies: how primary visibility is organised. `raycast` casts one ray per
//! pixel through the traversal kernel (`crate::traversal`); `faces` hands every face of the
//! geometry component to the draw stage (`crate::draw`); discovery (`quadtree`, `cluster`)
//! finds the visible faces with few rays and hands that set to the draw stage; `splat` and
//! `columns` fill their own buffers. The strategies' code lives in their back ends for now.
