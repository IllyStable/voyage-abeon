use bevy::render::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::Mesh;
use bevy::prelude::Vec3;
use itertools::Itertools;
use avian3d::collision::collider::Collider;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq)]
struct Index {
    x: u32,
    y: u32,
    z: u32,
}

impl From<[u32;3]> for Index {
    fn from(raw_index: [u32; 3]) -> Self {
        Self {
            x: raw_index[0],
            y: raw_index[1],
            z: raw_index[2],
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vertex {
    x: f32,
    y: f32,
    z: f32,
}

impl From<[f32;3]> for Vertex {
    fn from(raw_vertex: [f32; 3]) -> Self {
        Self {
            x: raw_vertex[0],
            y: raw_vertex[1],
            z: raw_vertex[2],
        }
    }
}

impl From<[u32;3]> for Vertex {
    fn from(raw_vertex: [u32; 3]) -> Self {
        Self {
            x: raw_vertex[0] as f32,
            y: raw_vertex[1] as f32,
            z: raw_vertex[2] as f32,
        }
    }
}

impl From<Vec3> for Vertex {
    fn from(vec3: Vec3) -> Self {
        Self {
            x: vec3.x,
            y: vec3.y,
            z: vec3.z
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,   
    pub z: i32,
}

impl ChunkPos {
    pub fn from_vertex(vertex: &Vertex, chunk_size: &f32) -> ChunkPos {
        ChunkPos {
            x: (vertex.x / (*chunk_size / 2.0)).floor() as i32,
            z: (vertex.z / (*chunk_size / 2.0)).floor() as i32,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ChunkData {
    vertices: Vec<Vertex>,
    indices: Vec<Index>,
    global_local_index_map: HashMap<u32, u32>,
}

fn split_mesh(vertices: Vec<Vertex>, indices: Vec<Index>, chunk_size: f32) -> HashMap<ChunkPos, (Vec<Vertex>, Vec<Index>)> {
    let mut chunks: HashMap<ChunkPos, ChunkData> = HashMap::new();

    // assign vertices to chunks
    for (global_idx, vertex) in vertices.iter().enumerate() {
        let chunk_pos = ChunkPos::from_vertex(&vertex, &chunk_size);
        let entry = chunks.entry(chunk_pos).or_insert(ChunkData {
            vertices: Vec::new(),
            indices: Vec::new(),
            global_local_index_map: HashMap::new()
        });

        entry.vertices.push(*vertex);
        entry.global_local_index_map.insert(global_idx.try_into().unwrap(), entry.vertices.len() as u32 - 1);
    }

    // assign indices to chunks 
    for index in indices {
        let vert_chunks = [
            ChunkPos::from_vertex(&vertices[index.x as usize], &chunk_size),
            ChunkPos::from_vertex(&vertices[index.y as usize], &chunk_size),
            ChunkPos::from_vertex(&vertices[index.z as usize], &chunk_size),
        ];

        // if all are in the same chunk, add triangle to it 
        if vert_chunks[0] == vert_chunks[1] && vert_chunks[1] == vert_chunks[2] {
            if let Some(chunk) = chunks.get_mut(&vert_chunks[0]) {
                chunk.indices.push(Index {
                    x: *chunk.global_local_index_map.get(&index.x).unwrap(),
                    y: *chunk.global_local_index_map.get(&index.y).unwrap(),
                    z: *chunk.global_local_index_map.get(&index.z).unwrap(),
                });
            }
        }
        // the triangle is in multiple chunks - add it to all relative chunks 
        else {
            // for every unique chunk
            for pos in vert_chunks.iter().unique() {
                if let Some(chunk) = chunks.get_mut(pos) {
                    // convert global indices to local indices 
                    let mut local_index = [0, 0, 0];
                    let mut added_new_verts = false;

                    for (i, global_idx) in [index.x, index.y, index.z].iter().enumerate() {
                        // if vertex is already in this chunk, we don't need to add to the chunk
                        if let Some(local_idx) = chunk.global_local_index_map.get(&global_idx) {
                            local_index[i] = *local_idx;
                        // vertex isn't already in this chunk - we need to add it 
                        } else {
                            let new_vertex = vertices[*global_idx as usize];

                            chunk.vertices.push(new_vertex);

                            let new_vert_idx = chunk.vertices.len() as u32 - 1;

                            chunk.global_local_index_map.insert(*global_idx, new_vert_idx);

                            local_index[i] = new_vert_idx;
                            added_new_verts = true;
                        }
                    }

                    if added_new_verts || ChunkPos::from_vertex(&vertices[index.x as usize], &chunk_size) == *pos {
                        chunk.indices.push(Index {x: local_index[0], y: local_index[1], z: local_index[2]});
                    }
                }
            }
        }
    }

    chunks.into_iter()
        .map(|(chunk_pos, chunk_data)| (chunk_pos, (chunk_data.vertices, chunk_data.indices)))
        .collect()
}

/// Adapted from https://github.com/dimforge/bevy_rapier/blob/master/src/geometry/collider_impl.rs#L738
/// Returns vertex and index buffers (in that order)
fn to_vertices(mesh: &Mesh) -> Option<(Vec<Vertex>, Vec<Index>)> {
    let vertices = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?;
    let indices = mesh.indices()?;

    let vtx: Vec<_> = match vertices {
        VertexAttributeValues::Float32(vtx) => Some(
            vtx.chunks(3)
                .map(|v| Vertex::from([v[0] as f32, v[1] as f32, v[2] as f32]))
                .collect(),
        ),
        VertexAttributeValues::Float32x3(vtx) => Some(
            vtx.iter()
                .map(|v| Vertex::from([v[0] as f32, v[1] as f32, v[2] as f32]))
                .collect(),
        ),
        _=> None,
    }?;

    let idx = match indices {
        Indices::U16(idx) => idx.chunks_exact(3)
            .map(|i| Index::from([i[0] as u32, i[1] as u32, i[2] as u32]))
            .collect(),
        Indices::U32(idx) => idx.chunks_exact(3).map(|i| Index::from([i[0], i[1], i[2]])).collect(),
    };

    Some((vtx, idx))
}

fn split_bevy_mesh(mesh: &Mesh, chunk_size: f32) -> HashMap<ChunkPos, (Vec<Vertex>, Vec<Index>)> {
    let mesh = to_vertices(mesh).unwrap_or((vec![], vec![]));
    split_mesh(mesh.0, mesh.1, chunk_size)
}

pub fn split_subcolliders(mesh: &Mesh, chunk_size: f32) -> Vec<(ChunkPos, Collider)> {
    let meshlets: Vec<(ChunkPos, (Vec<Vertex>, Vec<Index>))>  = split_bevy_mesh(mesh, chunk_size).into_iter().filter(|(pos, (verts, indices))| indices.len() > 0).collect();

    let mut subcolliders = Vec::new();
    for meshlet in meshlets {
        let vertices: Vec<Vec3> = meshlet.1.0.into_iter().map(|vert|{
            vert
        }).map(|vert| Vec3::new(vert.x, vert.y, vert.z)).collect();
        let indices = meshlet.1.1.into_iter().map(|idx| {
            idx
        }).map(|idx| [idx.x, idx.y, idx.z]).collect();

        subcolliders.push((meshlet.0, Collider::trimesh(vertices, indices)));
    }

    subcolliders
}
