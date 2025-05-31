use nalgebra::{Point3, Vector3};
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

// Main baking function now takes JsValue for params and deserializes
pub fn bake_texture_from_js_params(
	texture_type:TextureType,

	width:u32,

	height:u32,

	global_seed:u32,

	// Changed to reference
	params_js:&JsValue,

	height_map_data_opt:Option<&Vec<f32>>,
) -> Result<Vec<u8>, JsValue> {
	// Return Result for error handling
	let num_pixels = (width * height) as usize;
	let mut image_data = vec![0u8; num_pixels * 4];
	let noise_controller = NoiseController::new(global_seed);

	// Deserialize params based on texture_type
	// Note: This is simplified. In reality, NoiseController's sample_fbm would be
	// used, and it would construct FBM instances on the fly from FbmParameters.
	// The FbmParameters structs themselves would be part of the larger param
	// structs.

	image_data.par_chunks_mut(4).enumerate().for_each(|(pixel_idx, rgba_chunk)| {
		let x_coord = pixel_idx % (width as usize);
		let y_coord = pixel_idx / (width as usize);
		let u = x_coord as f64 / (width - 1).max(1) as f64;
		let v = y_coord as f64 / (height - 1).max(1) as f64;
		// Generic 2D slice
		let sample_p = [u, v, 0.5];

		let mut r = 0.0f32;
		let mut g = 0.0f32;
		let mut b = 0.0f32;
		let mut a_val = 1.0f32;

		match texture_type {
			TextureType::Albedo => {
				// Use clone if params_js is a ref
				let p:AlbedoBakeParams = serde_wasm_bindgen::from_value(params_js.clone()).unwrap_or_default();
				let base_noise = noise_controller.sample_fbm(&p.base_fbm, sample_p);
				let mut albedo_rgb = lerp_color_f64_arr(
					[p.slate_color_dark_r, p.slate_color_dark_g, p.slate_color_dark_b],
					[p.slate_color_light_r, p.slate_color_light_g, p.slate_color_light_b],
					base_noise.clamp(0.0, 1.0),
				);
				// ... (strata, veins, flecks logic using p.strata_fbm, p.vein_fbm etc. and
				// their specific color params) ... Example for strata:
				let strata_sample_p = [sample_p[0], sample_p[1] * p.strata_frequency_y_stretch, sample_p[2]];
				let strata_noise = noise_controller.sample_fbm(&p.strata_fbm, strata_sample_p);
				albedo_rgb = lerp_color_f64_arr(
					albedo_rgb,
					[p.strata_color_r, p.strata_color_g, p.strata_color_b],
					strata_noise.clamp(0.0, 1.0) * p.strata_influence,
				);

				r = albedo_rgb[0];
				g = albedo_rgb[1];
				b = albedo_rgb[2];
			},

			TextureType::Height => {
				let p:HeightBakeParams = serde_wasm_bindgen::from_value(params_js.clone()).unwrap_or_default();
				let base_h = noise_controller.sample_fbm(&p.base_fbm, sample_p);
				let detail_h = noise_controller.sample_fbm(&p.detail_fbm, sample_p);
				let combined_h = base_h * (1.0 - p.detail_blend_factor) + detail_h * p.detail_blend_factor;
				let final_h = (combined_h * p.overall_amplitude).clamp(0.0, 1.0);
				r = final_h as f32;
				g = r;
				b = r;
			},

			TextureType::Normal => {
				let p:NormalBakeParams = serde_wasm_bindgen::from_value(params_js.clone()).unwrap_or_default();
				if let Some(height_map) = height_map_data_opt {
					let h_c = height_map[pixel_idx];
					// ... (Sobel/central differences for dx, dy from height_map as before) ...
					let dx = 0.0;
					// Placeholder for brevity
					let dy = 0.0;
					let normal_vec = Vector3::new(-dx * p.strength as f32, -dy * p.strength as f32, 1.0).normalize();
					r = (normal_vec.x * 0.5 + 0.5) as f32;
					g = (normal_vec.y * 0.5 + 0.5) as f32;
					// Ensure Z is mostly positive for tangent space
					b = (normal_vec.z * 0.5 + 0.5) as f32;
				}
			},

			TextureType::Roughness => {
				let p:RoughnessBakeParams = serde_wasm_bindgen::from_value(params_js.clone()).unwrap_or_default();
				let noise = noise_controller.sample_fbm(&p.fbm_params, sample_p);
				let final_rough = p.min_roughness + noise.clamp(0.0, 1.0) * (p.max_roughness - p.min_roughness);
				r = final_rough.clamp(0.0, 1.0) as f32;
				g = r;
				b = r;
			},

			TextureType::AmbientOcclusion => {
				let p:AoBakeParams = serde_wasm_bindgen::from_value(params_js.clone()).unwrap_or_default();
				let noise = noise_controller.sample_fbm(&p.fbm_params, sample_p);
				let final_ao = 1.0 - (1.0 - noise.clamp(0.0, 1.0)) * p.strength;
				r = final_ao.clamp(0.0, 1.0) as f32;
				g = r;
				b = r;
			},
		}

		rgba_chunk[0] = (r.clamp(0.0, 1.0) * 255.0) as u8;
		rgba_chunk[1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
		rgba_chunk[2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
		rgba_chunk[3] = (a_val.clamp(0.0, 1.0) * 255.0) as u8;
	});
	Ok(image_data)
}
