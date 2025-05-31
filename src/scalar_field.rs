use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::noise_module::{FbmParameters, NoiseController};

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct GridDimensions {
	pub width:usize,

	pub height:usize,

	pub depth:usize,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ScalarFieldShapeParams {
	pub base_sphere_radius:f64,

	pub base_sphere_influence:f64,

	pub sphere_distort_fbm:FbmParameters,

	pub large_form_fbm:FbmParameters,

	pub medium_detail_fbm:FbmParameters,

	// Added for more granularity
	pub fine_detail_fbm:FbmParameters,
}

pub fn generate_scalar_field(
	grid_dims:GridDimensions,

	global_seed:u32,

	shape_params:&ScalarFieldShapeParams,
) -> Vec<f32> {
	let num_elements = grid_dims.width * grid_dims.height * grid_dims.depth;
	let mut scalar_field = vec![0.0f32; num_elements];
	let noise_controller = NoiseController::new(global_seed);

	let half_w = grid_dims.width as f64 / 2.0;
	let half_h = grid_dims.height as f64 / 2.0;
	let half_d = grid_dims.depth as f64 / 2.0;

	scalar_field.par_iter_mut().enumerate().for_each(|(index, cell_value)| {
		let z = index / (grid_dims.width * grid_dims.height);
		let rem = index % (grid_dims.width * grid_dims.height);
		let y = rem / grid_dims.width;
		let x = rem % grid_dims.width;

		// Avoid div by zero if grid dim is 1
		let norm_x = (x as f64 - half_w) / half_w.max(1.0);
		let norm_y = (y as f64 - half_h) / half_h.max(1.0);
		let norm_z = (z as f64 - half_d) / half_d.max(1.0);

		let mut density = 0.0f64;
		let base_sample_p = [norm_x, norm_y, norm_z];

		// 1. Base Sphere SDF with distortion
		let sphere_distort_val = noise_controller.sample_fbm(&shape_params.sphere_distort_fbm, base_sample_p);
		// Apply distortion more uniformly or selectively. Example: radial distortion.
		// Modulate distance from center
		let dist_factor = 1.0 + sphere_distort_val * 0.3;
		let dist_from_center = (norm_x.powi(2) + norm_y.powi(2) + norm_z.powi(2)).sqrt() / dist_factor.max(0.1);
		let base_sphere_sdf = shape_params.base_sphere_radius - dist_from_center;

		// 2. Layered FBM for density
		density += noise_controller.sample_fbm(&shape_params.large_form_fbm, base_sample_p);
		density += noise_controller.sample_fbm(&shape_params.medium_detail_fbm, base_sample_p);
		density += noise_controller.sample_fbm(&shape_params.fine_detail_fbm, base_sample_p);

		// Combine with base shape SDF
		if base_sphere_sdf < 0.0 {
			// Outside the base distorted sphere
			// Rapidly decrease density or set to a very negative value
			density = base_sphere_sdf * shape_params.base_sphere_influence.max(1.0);
		} else {
			// Inside the base distorted sphere
			// Modulate noise by how "solidly" inside the sphere we are
			density *= (base_sphere_sdf / shape_params.base_sphere_radius.max(0.01)).clamp(0.2, 1.0)
				* shape_params.base_sphere_influence;
		}

		*cell_value = density as f32;
	});
	scalar_field
}
