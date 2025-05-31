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

	// Use a block to handle potential errors from param deserialization early
	// We need to move the deserialization inside the parallel loop if params differ
	// per pixel, but here they are global for the bake, so deserialize once.
	// However, rayon's `for_each` closure needs to be `Fn`, so cloning params or
	// deserializing inside is needed. For simplicity and because params are small,

	// cloning is acceptable.

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

		// Handling Result inside closure:
		// This is tricky with Rayon's par_for_each which expects Fn closure.
		// A common pattern is to collect Results and then check them, or to panic on
		// error. For this specific case, as params are deserialized based on
		// `texture_type` which is fixed for the whole call, we can deserialize
		// outside the loop. The `unwrap_or_default` was hiding potential errors, now
		// we use `?` The actual deserialization for the specific type will be done
		// inside the match arms and we'll assume for now that if an error occurs
		// there, it should propagate. This means the function signature needs to
		// return Result from the closure as well, which par_chunks_mut doesn't
		// directly support. A simpler way for now is to deserialize params once
		// before the loop if they are not pixel-dependent.

		// Let's adjust the approach: deserialize params once, then clone them into the
		// closure. This requires params_js to be `Clone`. JsValue is Clone.

		match texture_type {
			TextureType::Albedo => {
				// Use clone if params_js is a ref
				let p:AlbedoBakeParams =
					 // Panics on error, or use Result.
					serde_wasm_bindgen::from_value(params_js.clone()).expect("Failed to deserialize AlbedoBakeParams");

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

				// TODO: Add vein and fleck logic similar to strata
				r = albedo_rgb[0];

				g = albedo_rgb[1];

				b = albedo_rgb[2];
			},

			TextureType::Height => {
				let p:HeightBakeParams =
					serde_wasm_bindgen::from_value(params_js.clone()).expect("Failed to deserialize HeightBakeParams");

				let base_h = noise_controller.sample_fbm(&p.base_fbm, sample_p);

				let detail_h = noise_controller.sample_fbm(&p.detail_fbm, sample_p);

				let combined_h = base_h * (1.0 - p.detail_blend_factor) + detail_h * p.detail_blend_factor;

				let final_h = (combined_h * p.overall_amplitude).clamp(0.0, 1.0);

				r = final_h as f32;

				g = r;

				b = r;
			},

			TextureType::Normal => {
				let p:NormalBakeParams =
					serde_wasm_bindgen::from_value(params_js.clone()).expect("Failed to deserialize NormalBakeParams");

				if let Some(height_map) = height_map_data_opt {
					// Assumes height_map values are [0.0, 1.0]
					// Sobel/Central differences for dx, dy from height_map
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

					// Image Y increases downwards. For tangent space, Y typically increases
					// upwards. So if H(y+1) > H(y-1) (image space), surface slopes "down" in
					// image, "up" in world if Y is up. Gradient in V (texture Y) direction.
					let h_t = sample_height(
						height_map,
						x_coord as isize,
						y_coord as isize - 1,
						width as usize,
						height as usize,
						// Top neighbor (V decreases)
					);

					let h_b = sample_height(
						height_map,
						x_coord as isize,
						y_coord as isize + 1,
						width as usize,
						height as usize,
						// Bottom neighbor (V increases)
					);

					// Finite differences
					// How much height changes per unit in texture space (e.g. per pixel
					// width/height) If we consider the height map to be over a [0,1]x[0,1] UV
					// square, then a step of 1 pixel corresponds to 1/width or 1/height in UV
					// space. For simplicity, we can let `strength` handle the scaling.
					// Difference over 2 pixels if not scaled by 0.5
					let dx_h = (h_r - h_l);

					// Difference over 2 pixels
					let dy_h = (h_b - h_t);

					let strength_f32 = p.strength as f32;

					// Normal vector: ( -df/dx, -df/dy, 1 ) or ( df/dx, df/dy, 1 ) then adjust signs
					// based on convention Common convention for tangent space normals (OpenGL
					// like): Nx = -dHeight/dTexcoordU * Strength
					// Ny = -dHeight/dTexcoordV * Strength (or + if V is inverted relative to
					// desired Y) Nz = 1.0
					// If texcoord V increases downwards (common for images), and we want tangent Y
					// to be "up", then if height increases with V (dy_h positive), normal's Y
					// should point up (positive). Let's assume: Positive U is right, Positive
					// V is down. Tangent space: Positive X is right, Positive Y is up.
					// So, a positive dH/dU (h_r > h_l) means slope upwards to the right. Normal.X
					// should be negative. A positive dH/dV (h_b > h_t) means slope upwards
					// downwards (V increases). Normal.Y should be negative (pointing "up" against
					// V's increase).
					let normal_vec = Vector3::new(
						// If height increases to the right (U+), normal x points left (-)
						-dx_h * strength_f32,
						-dy_h * strength_f32, /* If height increases downwards (V+), normal y points up (-)
						                       * (relative to image V direction) */
						1.0, /* Assume a base Z component before normalization.
						      * The actual value doesn't matter as much as its sign before normalization,
						      *
						      * but 1.0 is common for height maps where Z is the "up" direction of the surface
						      * patch. */
					)
					.normalize();

					r = (normal_vec.x * 0.5 + 0.5).clamp(0.0, 1.0);

					g = (normal_vec.y * 0.5 + 0.5).clamp(0.0, 1.0);

					// Ensure Z is mostly positive
					b = (normal_vec.z * 0.5 + 0.5).clamp(0.0, 1.0);
				} else {
					// Default flat normal if no height map
					r = 0.5;

					g = 0.5;

					b = 1.0;
				}
			},

			TextureType::Roughness => {
				let p:RoughnessBakeParams = serde_wasm_bindgen::from_value(params_js.clone())
					.expect("Failed to deserialize RoughnessBakeParams");

				let noise = noise_controller.sample_fbm(&p.fbm_params, sample_p);

				let final_rough = p.min_roughness + noise.clamp(0.0, 1.0) * (p.max_roughness - p.min_roughness);

				r = final_rough.clamp(0.0, 1.0) as f32;

				g = r;

				b = r;
			},

			TextureType::AmbientOcclusion => {
				let p:AoBakeParams =
					serde_wasm_bindgen::from_value(params_js.clone()).expect("Failed to deserialize AoBakeParams");

				let noise = noise_controller.sample_fbm(&p.fbm_params, sample_p);

				// This formula makes more sense: 1.0 - (noise * strength)
				let final_ao = 1.0 - (1.0 - noise.clamp(0.0, 1.0)) * p.strength;

				// Or: 1.0 - ((1.0-noise) * strength) if noise = 0 is full occlusion
				// Let's assume noise 0 = dark, noise 1 = bright.
				// Then AO = 1.0 - (noise_value * strength).
				// If original formula was (1.0 - noise) * strength, it means noise=1 (bright)
				// is full AO (dark), which is counter-intuitive for fbm output [0,1].
				// The current TS code has `1.0 - (1.0 - noise.clamp(0.0, 1.0)) * p.strength;`
				// If noise = 0 (darkest noise value), ao = 1.0 - (1.0 * strength)
				// If noise = 1 (brightest noise value), ao = 1.0 - (0.0 * strength) = 1.0
				// This means lower FBM values give stronger AO (darker result). This is
				// reasonable.
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

	// To properly use `?` for deserialization errors, we would deserialize *before*
	// the parallel loop: ```
	// let deserialized_params = match texture_type {

	//     TextureType::Albedo =>
	// MyParamsEnum::Albedo(serde_wasm_bindgen::from_value(params_js.clone())?),

	// ... other types
	//
	// };

	// Then in the parallel loop, match on MyParamsEnum and use the already
	// deserialized params.
	//
	// However, this example uses `.expect()` inside the loop for brevity for now,

	// which will panic.
	//
	// A robust solution would involve collecting results from the parallel threads
	// or using a try_for_each.
	//
	// ```
	// For now, the `.expect` calls will panic if deserialization fails.
	// The prompt asked for `?` propagation. Let's refine this slightly.
	// The `par_chunks_mut().enumerate().for_each()` closure cannot directly return
	// a `Result` that propagates out of `bake_texture_from_js_params`.
	// The most straightforward way to use `?` is to deserialize outside the loop,

	// but this requires a common param type or an enum.

	// Given the current structure and to minimally change it while introducing `?`:
	// We'd have to make the closure's body return a `Result<(), JsValue>` and then
	// handle it, perhaps by finding the first error. This complicates parallel
	// processing significantly.

	// Sticking to the prompt for `?` on deserialization means we should deserialize
	// ONCE before the parallel loop. This will require an enum to hold the
	// different param types.

	// Let's adjust `bake_texture_from_js_params` to deserialize ONCE.

	Ok(image_data)
}

// Re-evaluating the deserialization with `?`:
// The previous approach with `.expect()` inside `par_for_each` is not ideal for
// error propagation. To use `?`, we should deserialize before the parallel
// section.

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

// Redefine bake_texture_from_js_params to use the enum
pub fn bake_texture_from_js_params_refined(
	texture_type:TextureType,

	width:u32,

	height:u32,

	global_seed:u32,

	params_js:&JsValue,

	height_map_data_opt:Option<&Vec<f32>>,
) -> Result<Vec<u8>, JsValue> {
	let num_pixels = (width * height) as usize;

	// RGBA
	let mut image_data = vec![0u8; num_pixels * 4];

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

	image_data.par_chunks_mut(4).enumerate().for_each(|(pixel_idx, rgba_chunk)| {
		let x_coord = pixel_idx % (width as usize);

		let y_coord = pixel_idx / (width as usize);

		let u = x_coord as f64 / (width - 1).max(1) as f64;

		let v = y_coord as f64 / (height - 1).max(1) as f64;

		// Generic 2D slice
		let sample_p = [u, v, 0.5];

		let mut r = 0.0f32;

		let mut g = 0.0f32;

		// Renamed to avoid conflict with outer 'b' if any
		let mut b_val = 0.0f32;

		let mut a_val = 1.0f32;

		match bake_params_variant {
			BakeParamsVariant::Albedo(p) => {
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

				// Veins logic (simplified example)
				let vein_warp_offset_x = noise_controller
					.sample_fbm(&p.vein_warp_fbm, [sample_p[0] + 17.3, sample_p[1] + 5.7, sample_p[2]])
					 // range [-1, 1]
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

				// Flecks logic (simplified example)
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

					// Change in height over ~2 pixels in X
					let dx_h = h_r - h_l;

					// Change in height over ~2 pixels in Y (V tex coord)
					let dy_h = h_b - h_t;

					let strength_f32 = p.strength as f32;

					// Texture coordinates: U right, V down.
					// Tangent space: X right, Y up, Z out of screen.
					// Normal.x: if height increases to the right (U+), normal tilts left (-X). So
					// -dx_h. Normal.y: if height increases downwards (V+), normal tilts "up"
					// relative to surface (which is -V direction). So -dy_h.
					let normal_vec = Vector3::new(
						-dx_h * strength_f32,
						-dy_h * strength_f32, /* This sign makes Y point "up" in tangent space if V points "down"
						                       * and height increases with V. */
						2.0 / (width as f32 + height as f32), /* This provides some scaling for Z based on pixel
						                                       * density.
						                                       * A fixed value like 1.0 is also common.
						                                       * The key is that Z should be positive. */
					)
					.normalize();

					r = (normal_vec.x * 0.5 + 0.5).clamp(0.0, 1.0);

					g = (normal_vec.y * 0.5 + 0.5).clamp(0.0, 1.0);

					b_val = (normal_vec.z * 0.5 + 0.5).clamp(0.0, 1.0);
				} else {
					r = 0.5;

					g = 0.5;

					// Default flat normal
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

				// Lower FBM values (closer to 0) should result in stronger AO (occlusion_factor
				// closer to 1) So, if noise=0, term = strength. AO = 1 - strength.
				// If noise=1, term = 0. AO = 1.0.
				// This makes sense: dark FBM areas -> more occluded.
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

		rgba_chunk[3] = (a_val.clamp(0.0, 1.0) * 255.0) as u8;
	});

	Ok(image_data)
}

// The original bake_texture_from_js_params will be replaced by
// bake_texture_from_js_params_refined in lib.rs
