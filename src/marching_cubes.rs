use std::collections::HashMap;

use nalgebra::{Point3, Vector3};

use super::{
	marching_cubes_tables::{EDGE_TABLE, TRI_TABLE},
	scalar_field::GridDimensions,
};

pub struct MarchingCubesOutput {
	// Flattened: [x,y,z, nx,ny,nz, u,v, ...]
	pub vertices:Vec<f32>,

	pub indices:Vec<u32>,
}

#[derive(Debug, Clone, Copy, Default)]
struct GridPointData {
	position:Point3<f32>,

	value:f32,
}

fn interpolate_vertex(iso_level:f32, p1_data:&GridPointData, p2_data:&GridPointData) -> Point3<f32> {
	if (p1_data.value - p2_data.value).abs() < 1e-6 {
		return p1_data.position;
	}

	let t = (iso_level - p1_data.value) / (p2_data.value - p1_data.value);

	// Clamp t for robustness
	p1_data.position + (p2_data.position - p1_data.position) * t.clamp(0.0, 1.0)
}

// Global index for a grid corner
fn corner_global_index(x:usize, y:usize, z:usize, grid_width:usize, grid_height:usize) -> usize {
	x + y * grid_width + z * grid_width * grid_height
}

// Canonical edge key from two global corner indices
fn edge_key(idx1_global:usize, idx2_global:usize) -> (usize, usize) {
	if idx1_global < idx2_global {
		(idx1_global, idx2_global)
	} else {
		(idx2_global, idx1_global)
	}
}

pub fn run_marching_cubes(
	scalar_field:&[f32],

	grid_dims:GridDimensions,

	iso_level:f32,

	// Add scale and offset if grid coordinates are not already world coordinates
	// e.g. Point3::new(1.0, 1.0, 1.0)
	mesh_scale:Point3<f32>,

	// e.g. Point3::new(-grid_width/2.0, ...)
	mesh_offset:Point3<f32>,
) -> MarchingCubesOutput {
	let mut output_vertex_positions:Vec<Point3<f32>> = Vec::new();

	let mut output_indices:Vec<u32> = Vec::new();

	let mut vertex_normals_accumulator:Vec<Vector3<f32>> = Vec::new();

	let mut vertex_shared_face_count:Vec<u32> = Vec::new();

	let mut edge_to_vertex_index_map:HashMap<(usize, usize), u32> = HashMap::new();

	let GridDimensions { width, height, depth } = grid_dims;

	let corner_offsets:[Point3<usize>; 8] = [
		Point3::new(0, 0, 0),
		Point3::new(1, 0, 0),
		Point3::new(1, 1, 0),
		Point3::new(0, 1, 0),
		Point3::new(0, 0, 1),
		Point3::new(1, 0, 1),
		Point3::new(1, 1, 1),
		Point3::new(0, 1, 1),
	];

	let cube_edge_corner_indices:[(usize, usize); 12] = [
		(0, 1),
		(1, 2),
		(2, 3),
		(3, 0),
		(4, 5),
		(5, 6),
		(6, 7),
		(7, 4),
		(0, 4),
		(1, 5),
		(2, 6),
		(3, 7),
	];

	for z_grid in 0..depth - 1 {
		for y_grid in 0..height - 1 {
			for x_grid in 0..width - 1 {
				let mut cell_corner_data:[GridPointData; 8] = [Default::default(); 8];

				let mut cube_case_index:usize = 0;

				for i in 0..8 {
					let cx = x_grid + corner_offsets[i].x;

					let cy = y_grid + corner_offsets[i].y;

					let cz = z_grid + corner_offsets[i].z;

					let scalar_idx = corner_global_index(cx, cy, cz, width, height);

					cell_corner_data[i] = GridPointData {
						// Grid coordinates
						position:Point3::new(cx as f32, cy as f32, cz as f32),

						value:scalar_field[scalar_idx],
					};

					if cell_corner_data[i].value < iso_level {
						cube_case_index |= 1 << i;
					}
				}

				if EDGE_TABLE[cube_case_index] == 0 {
					continue;
				}

				let mut current_cell_edge_vertex_ids:[Option<u32>; 12] = [None; 12];

				for edge_idx in 0..12 {
					if (EDGE_TABLE[cube_case_index] & (1 << edge_idx)) != 0 {
						let (c1_local_idx, c2_local_idx) = cube_edge_corner_indices[edge_idx];

						let p1_data = &cell_corner_data[c1_local_idx];

						let p2_data = &cell_corner_data[c2_local_idx];

						// Global indices for edge key
						let g_c1_x = x_grid + corner_offsets[c1_local_idx].x;

						let g_c1_y = y_grid + corner_offsets[c1_local_idx].y;

						let g_c1_z = z_grid + corner_offsets[c1_local_idx].z;

						let g_idx1 = corner_global_index(g_c1_x, g_c1_y, g_c1_z, width, height);

						let g_c2_x = x_grid + corner_offsets[c2_local_idx].x;

						let g_c2_y = y_grid + corner_offsets[c2_local_idx].y;

						let g_c2_z = z_grid + corner_offsets[c2_local_idx].z;

						let g_idx2 = corner_global_index(g_c2_x, g_c2_y, g_c2_z, width, height);

						let map_key = edge_key(g_idx1, g_idx2);

						let vertex_id = *edge_to_vertex_index_map.entry(map_key).or_insert_with(|| {
							let new_pos = interpolate_vertex(iso_level, p1_data, p2_data);

							output_vertex_positions.push(new_pos);

							vertex_normals_accumulator.push(Vector3::zeros());

							vertex_shared_face_count.push(0);

							(output_vertex_positions.len() - 1) as u32
						});

						current_cell_edge_vertex_ids[edge_idx] = Some(vertex_id);
					}
				}

				let tri_table_start_idx = cube_case_index * 16;

				for i in (0..15).step_by(3) {
					if TRI_TABLE[tri_table_start_idx + i] == -1 {
						break;
					}

					let tri_v_indices_in_cell_edges = [
						TRI_TABLE[tri_table_start_idx + i] as usize,
						TRI_TABLE[tri_table_start_idx + i + 1] as usize,
						TRI_TABLE[tri_table_start_idx + i + 2] as usize,
					];

					let idx0 = current_cell_edge_vertex_ids[tri_v_indices_in_cell_edges[0]].unwrap();

					let idx1 = current_cell_edge_vertex_ids[tri_v_indices_in_cell_edges[1]].unwrap();

					let idx2 = current_cell_edge_vertex_ids[tri_v_indices_in_cell_edges[2]].unwrap();

					// Check for degenerate triangles (can happen with some MC ambiguities or
					// iso-levels)
					if idx0 == idx1 || idx0 == idx2 || idx1 == idx2 {
						continue;
					}

					output_indices.push(idx0);

					output_indices.push(idx1);

					output_indices.push(idx2);

					let v0_pos = output_vertex_positions[idx0 as usize];

					let v1_pos = output_vertex_positions[idx1 as usize];

					let v2_pos = output_vertex_positions[idx2 as usize];

					let edge_a = v1_pos - v0_pos;

					let edge_b = v2_pos - v0_pos;

					// Not normalized yet
					let face_normal = edge_a.cross(&edge_b);

					// Add to accumulator (ensure consistent winding order)
					// MC tables usually provide consistent winding. If not, may need to check
					// dot(face_normal, average_cell_gradient)
					vertex_normals_accumulator[idx0 as usize] += face_normal;

					vertex_normals_accumulator[idx1 as usize] += face_normal;

					vertex_normals_accumulator[idx2 as usize] += face_normal;

					vertex_shared_face_count[idx0 as usize] += 1;

					vertex_shared_face_count[idx1 as usize] += 1;

					vertex_shared_face_count[idx2 as usize] += 1;
				}
			}
		}
	}

	// pos(3) + normal(3) + uv(2)
	let mut final_vertices_flat:Vec<f32> = Vec::with_capacity(output_vertex_positions.len() * 8);

	// For UV normalization
	let uv_scale_x = 1.0 / ((width - 1) as f32 * mesh_scale.x).max(1.0);

	let uv_scale_y = 1.0 / ((height - 1) as f32 * mesh_scale.y).max(1.0);

	for i in 0..output_vertex_positions.len() {
		let grid_pos = output_vertex_positions[i];

		// Apply scale and offset to grid positions to get world positions
		let world_pos = Point3::new(
			grid_pos.x * mesh_scale.x + mesh_offset.x,
			grid_pos.y * mesh_scale.y + mesh_offset.y,
			grid_pos.z * mesh_scale.z + mesh_offset.z,
		);

		final_vertices_flat.push(world_pos.x);

		final_vertices_flat.push(world_pos.y);

		final_vertices_flat.push(world_pos.z);

		let normal = if vertex_shared_face_count[i] > 0 {
			vertex_normals_accumulator[i].normalize()
		} else {
			// Default for isolated vertices
			Vector3::new(0.0, 1.0, 0.0)
		};

		final_vertices_flat.push(normal.x);

		final_vertices_flat.push(normal.y);

		final_vertices_flat.push(normal.z);

		// Simple UVs based on scaled XZ world position (top-down projection)
		// This is often okay for rocks if tri-planar is used for main texturing.
		// Normalize based on original grid span
		let u = (world_pos.x - mesh_offset.x) * uv_scale_x;

		// Using Z for V for top-down
		let v = (world_pos.z - mesh_offset.z) * uv_scale_y;

		final_vertices_flat.push(u.clamp(0.0, 1.0));

		final_vertices_flat.push(v.clamp(0.0, 1.0));
	}

	MarchingCubesOutput { vertices:final_vertices_flat, indices:output_indices }
}
