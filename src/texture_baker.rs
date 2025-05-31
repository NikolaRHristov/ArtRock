use nalgebra::Vector3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::noise_module::{FbmParameters, NoiseController};

// So JS can see this enum
#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum TextureType {
	Albedo = 0,

	Height = 1,

	Normal = 2,

	Roughness = 3,

	AmbientOcclusion = 4,
}

// Parameter structs (must be Serialize, Deserialize, Clone, Copy for
// wasm_bindgen + serde)
#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct AlbedoBakeParams {
	pub base_fbm:FbmParameters,

	pub strata_fbm:FbmParameters,

	pub vein_fbm:FbmParameters,

	pub vein_warp_fbm:FbmParameters,

	pub fleck_fbm:FbmParameters,

	pub slate_color_dark_r:f32,

	pub slate_color_dark_g:f32,

	pub slate_color_dark_b:f32,

	pub slate_color_light_r:f32,

	pub slate_color_light_g:f32,

	pub slate_color_light_b:f32,

	pub strata_color_r:f32,

	pub strata_color_g:f32,

	pub strata_color_b:f32,

	pub strata_influence:f64,

	pub strata_frequency_y_stretch:f64,

	pub vein_color_primary_r:f32,

	pub vein_color_primary_g:f32,

	pub vein_color_primary_b:f32,

	pub vein_threshold:f64,

	pub fleck_color_r:f32,

	pub fleck_color_g:f32,

	pub fleck_color_b:f32,

	pub fleck_threshold:f64,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct HeightBakeParams {
	pub base_fbm:FbmParameters,

	pub detail_fbm:FbmParameters,

	pub detail_blend_factor:f64,

	pub overall_amplitude:f64,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct NormalBakeParams {
	pub strength:f64,
	// Expects a height map to be passed in for derivation
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct RoughnessBakeParams {
	pub fbm_params:FbmParameters,

	pub min_roughness:f64,

	pub max_roughness:f64,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct AoBakeParams {
	// FBM for simple AO
	pub fbm_params:FbmParameters,

	pub strength:f64,
	// For more advanced AO, would need height map or geometry info
}

// Color lerp helper
fn lerp_color_f64_arr(c1:[f32; 3], c2:[f32; 3], t:f64) -> [f32; 3] {
	let t_f32 = t as f32;

	[
		c1[0] + (c2[0] - c1[0]) * t_f32,
		c1[1] + (c2[1] - c1[1]) * t_f32,
		c1[2] + (c2[2] - c1[2]) * t_f32,
	]
}

// Helper to sample height map with boundary clamping
fn sample_height(height_map:&[f32], x:isize, y:isize, width:usize, height:usize) -> f32 {
	let clamped_x = x.clamp(0, width as isize - 1) as usize;

	let clamped_y = y.clamp(0, height as isize - 1) as usize;

	height_map[clamped_y * width + clamped_x]
}

// Define an internal enum to hold different bake parameters
// To be usable in the parallel closure
#[derive(Clone, Copy)]
enum BakeParamsVariant {
	Albedo(AlbedoBakeParams),

	Height(HeightBakeParams),

	Normal(NormalBakeParams),

	Roughness(RoughnessBakeParams),

	AmbientOcclusion(AoBakeParams),
}

// bake_texture_from_js_params_refined (main public-facing logic now in this
// function)
pub fn bake_texture_from_js_params_refined(
	texture_type:TextureType,

	width:u32,

	height:u32,

	global_seed:u32,

	// Still take JsValue by reference
	params_js:&JsValue,

	// This is Option<&Vec<f32>>
	height_map_data_opt:Option<&Vec<f32>>,
) -> Result<Vec<u8>, JsValue> {
	let num_pixels = (width * height) as usize;

	// RGBA
	let mut image_data = vec![0u8; num_pixels * 4];

	// NoiseController is Copy
	let noise_controller = NoiseController::new(global_seed);

	// Deserialize parameters ONCE before the parallel loop
	let bake_params_variant = match texture_type {
		TextureType::Albedo => {
			let p:AlbedoBakeParams = serde_wasm_bindgen::from_value(params_js.clone())?;

			BakeParamsVariant::Albedo(p)
		},

		TextureType::Height => {
			let p:HeightBakeParams = serde_wasm_bindgen::from_value(params_js.clone())?;

			BakeParamsVariant::Height(p)
		},

		TextureType::Normal => {
			let p:NormalBakeParams = serde_wasm_bindgen::from_value(params_js.clone())?;

			BakeParamsVariant::Normal(p)
		},

		TextureType::Roughness => {
			let p:RoughnessBakeParams = serde_wasm_bindgen::from_value(params_js.clone())?;

			BakeParamsVariant::Roughness(p)
		},

		TextureType::AmbientOcclusion => {
			let p:AoBakeParams = serde_wasm_bindgen::from_value(params_js.clone())?;

			BakeParamsVariant::AmbientOcclusion(p)
		},
	};

	// `height_map_data_opt` is `Option<&Vec<f32>>`. To pass it to a parallel
	// closure, if it's `Some(vec_ref)`, `vec_ref` itself is `&Vec<f32>` which is
	// `Sync`. So `height_map_data_opt` can be captured directly.

	image_data.par_chunks_mut(4).enumerate().for_each(|(pixel_idx, rgba_chunk)| {
		let x_coord = pixel_idx % (width as usize);

		let y_coord = pixel_idx / (width as usize);

		let u = x_coord as f64 / (width - 1).max(1) as f64;

		let v = y_coord as f64 / (height - 1).max(1) as f64;

		let sample_p = [u, v, 0.5];

		// Declare r, g, b_val here, no initial assignment needed as all paths in match
		// will assign them.
		let r:f32;

		let g:f32;

		// Renamed to avoid conflict
		let b_val:f32;

		// a_val is not changed, so can be const or non-mut let.
		let a_val:f32 = 1.0;

		match bake_params_variant {
			// bake_params_variant is Copy
			BakeParamsVariant::Albedo(p) => {
				// p is AlbedoBakeParams which is Copy
				let base_noise = noise_controller.sample_fbm(&p.base_fbm, sample_p);

				let mut albedo_rgb = lerp_color_f64_arr(
					[p.slate_color_dark_r, p.slate_color_dark_g, p.slate_color_dark_b],
					[p.slate_color_light_r, p.slate_color_light_g, p.slate_color_light_b],
					base_noise.clamp(0.0, 1.0),
				);

				let strata_sample_p = [sample_p[0], sample_p[1] * p.strata_frequency_y_stretch, sample_p[2]];

				let strata_noise = noise_controller.sample_fbm(&p.strata_fbm, strata_sample_p);

				albedo_rgb = lerp_color_f64_arr(
					albedo_rgb,
					[p.strata_color_r, p.strata_color_g, p.strata_color_b],
					strata_noise.clamp(0.0, 1.0) * p.strata_influence,
				);

				let vein_warp_offset_x = noise_controller
					.sample_fbm(&p.vein_warp_fbm, [sample_p[0] + 17.3, sample_p[1] + 5.7, sample_p[2]])
					* 2.0 - 1.0;

				let vein_warp_offset_y = noise_controller
					.sample_fbm(&p.vein_warp_fbm, [sample_p[0] - 9.1, sample_p[1] - 13.9, sample_p[2]])
					* 2.0 - 1.0;

				let vein_sample_p = [
					sample_p[0] + vein_warp_offset_x * p.vein_warp_fbm.amplitude,
					sample_p[1] + vein_warp_offset_y * p.vein_warp_fbm.amplitude,
					sample_p[2],
				];

				let vein_noise = noise_controller.sample_fbm(&p.vein_fbm, vein_sample_p);

				if vein_noise > p.vein_threshold {
					let vein_mix =
						((vein_noise - p.vein_threshold) / (1.0 - p.vein_threshold).max(0.01)).clamp(0.0, 1.0);

					albedo_rgb = lerp_color_f64_arr(
						albedo_rgb,
						[p.vein_color_primary_r, p.vein_color_primary_g, p.vein_color_primary_b],
						vein_mix,
					);
				}

				let fleck_noise = noise_controller.sample_fbm(&p.fleck_fbm, sample_p);

				if fleck_noise > p.fleck_threshold {
					let fleck_mix =
						((fleck_noise - p.fleck_threshold) / (1.0 - p.fleck_threshold).max(0.01)).clamp(0.0, 1.0);

					albedo_rgb =
						lerp_color_f64_arr(albedo_rgb, [p.fleck_color_r, p.fleck_color_g, p.fleck_color_b], fleck_mix);
				}

				r = albedo_rgb[0];

				g = albedo_rgb[1];

				b_val = albedo_rgb[2];
			},

			BakeParamsVariant::Height(p) => {
				let base_h = noise_controller.sample_fbm(&p.base_fbm, sample_p);

				let detail_h = noise_controller.sample_fbm(&p.detail_fbm, sample_p);

				let combined_h = base_h * (1.0 - p.detail_blend_factor) + detail_h * p.detail_blend_factor;

				let final_h = (combined_h * p.overall_amplitude).clamp(0.0, 1.0);

				r = final_h as f32;

				g = r;

				b_val = r;
			},

			BakeParamsVariant::Normal(p) => {
				if let Some(height_map) = height_map_data_opt {
					// height_map_data_opt is captured (Option<&Vec<f32>>)
					let h_l = sample_height(
						height_map,
						x_coord as isize - 1,
						y_coord as isize,
						width as usize,
						height as usize,
					);

					let h_r = sample_height(
						height_map,
						x_coord as isize + 1,
						y_coord as isize,
						width as usize,
						height as usize,
					);

					let h_t = sample_height(
						height_map,
						x_coord as isize,
						y_coord as isize - 1,
						width as usize,
						height as usize,
					);

					let h_b = sample_height(
						height_map,
						x_coord as isize,
						y_coord as isize + 1,
						width as usize,
						height as usize,
					);

					// No parens needed
					let dx_h = h_r - h_l;

					// No parens needed
					let dy_h = h_b - h_t;

					let strength_f32 = p.strength as f32;

					let normal_vec = Vector3::new(
						-dx_h * strength_f32,
						-dy_h * strength_f32,
						2.0 / (width as f32 + height as f32).max(1.0), /* Added max(1.0) to prevent div by zero if
						                                                * width/height are 0 */
					)
					.normalize();

					r = (normal_vec.x * 0.5 + 0.5).clamp(0.0, 1.0);

					g = (normal_vec.y * 0.5 + 0.5).clamp(0.0, 1.0);

					b_val = (normal_vec.z * 0.5 + 0.5).clamp(0.0, 1.0);
				} else {
					r = 0.5;

					g = 0.5;

					b_val = 1.0;
				}
			},

			BakeParamsVariant::Roughness(p) => {
				let noise = noise_controller.sample_fbm(&p.fbm_params, sample_p);

				let final_rough = p.min_roughness + noise.clamp(0.0, 1.0) * (p.max_roughness - p.min_roughness);

				r = final_rough.clamp(0.0, 1.0) as f32;

				g = r;

				b_val = r;
			},

			BakeParamsVariant::AmbientOcclusion(p) => {
				let noise = noise_controller.sample_fbm(&p.fbm_params, sample_p);

				let occlusion_factor = (1.0 - noise.clamp(0.0, 1.0)) * p.strength;

				let final_ao = (1.0 - occlusion_factor).clamp(0.0, 1.0);

				r = final_ao as f32;

				g = r;

				b_val = r;
			},
		}

		rgba_chunk[0] = (r.clamp(0.0, 1.0) * 255.0) as u8;

		rgba_chunk[1] = (g.clamp(0.0, 1.0) * 255.0) as u8;

		rgba_chunk[2] = (b_val.clamp(0.0, 1.0) * 255.0) as u8;

		// a_val is used here
		rgba_chunk[3] = (a_val.clamp(0.0, 1.0) * 255.0) as u8;
	});

	Ok(image_data)
}
