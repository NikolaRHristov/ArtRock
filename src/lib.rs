mod marching_cubes;
mod marching_cubes_tables;
mod noise_module;
mod scalar_field;
mod texture_baker;

use nalgebra::Point3; // For mesh_scale, mesh_offset
use noise_module::FbmParameters; // Assuming this derives Serialize, Deserialize
use scalar_field::{GridDimensions, ScalarFieldShapeParams}; // Assuming these derive Serialize, Deserialize
use serde::{Deserialize, Serialize}; // For parameter structs
// Texture Baker specific parameter structs
use texture_baker::{
	AlbedoBakeParams,
	AoBakeParams,
	HeightBakeParams,
	NormalBakeParams,
	RoughnessBakeParams,
	TextureType as RustTextureType, // Alias to avoid conflict if JsTextureType is same name
};
use wasm_bindgen::prelude::*;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC:wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
	#[cfg(feature = "console_error_panic_hook")]
	console_error_panic_hook::set_once();
	Ok(())
}

// --- Scalar Field Generation ---
#[wasm_bindgen]
pub fn get_scalar_field_structured_params_wasm(
	grid_dims_js:JsValue, // Expect GridDimensions JSON
	global_seed:u32,
	shape_params_js:JsValue, // Expect ScalarFieldShapeParams JSON
) -> Result<Vec<f32>, JsValue> {
	let grid_dims:GridDimensions = serde_wasm_bindgen::from_value(grid_dims_js)?;
	let shape_params:ScalarFieldShapeParams = serde_wasm_bindgen::from_value(shape_params_js)?;
	Ok(scalar_field::generate_scalar_field(grid_dims, global_seed, &shape_params))
}

// --- Marching Cubes ---
#[wasm_bindgen]
pub struct MeshData {
	pub vertices:Vec<f32>,
	pub indices:Vec<u32>,
}

#[wasm_bindgen]
pub fn extract_mesh_wasm(
	scalar_field_js_array:js_sys::Float32Array,
	grid_dims_js:JsValue, // Expect GridDimensions JSON
	iso_level:f32,
	mesh_scale_x:f32,
	mesh_scale_y:f32,
	mesh_scale_z:f32,
	mesh_offset_x:f32,
	mesh_offset_y:f32,
	mesh_offset_z:f32,
) -> Result<MeshData, JsValue> {
	let scalar_field_vec:Vec<f32> = scalar_field_js_array.to_vec(); // Consider zero-copy alternatives for performance
	let grid_dims:GridDimensions = serde_wasm_bindgen::from_value(grid_dims_js)?;

	let mesh_scale = Point3::new(mesh_scale_x, mesh_scale_y, mesh_scale_z);
	let mesh_offset = Point3::new(mesh_offset_x, mesh_offset_y, mesh_offset_z);

	let mc_output =
		marching_cubes::run_marching_cubes(&scalar_field_vec, grid_dims, iso_level, mesh_scale, mesh_offset);
	Ok(MeshData { vertices:mc_output.vertices, indices:mc_output.indices })
}

// --- Texture Baking ---
#[wasm_bindgen]
pub enum JsTextureType {
	// Keep this JS-facing enum simple
	Albedo = 0,
	Height = 1,
	Normal = 2,
	Roughness = 3,
	AmbientOcclusion = 4,
}

#[wasm_bindgen]
pub fn bake_texture_wasm(
	js_texture_type:JsTextureType,
	width:u32,
	height:u32,
	global_seed:u32,
	params_js_value:JsValue,                              // JSON object for the specific bake params
	height_map_js_array_opt:Option<js_sys::Float32Array>, // For normal baking
) -> Result<Vec<u8>, JsValue> {
	let rust_tex_type = match js_texture_type {
		JsTextureType::Albedo => RustTextureType::Albedo,
		JsTextureType::Height => RustTextureType::Height,
		JsTextureType::Normal => RustTextureType::Normal,
		JsTextureType::Roughness => RustTextureType::Roughness,
		JsTextureType::AmbientOcclusion => RustTextureType::AmbientOcclusion,
	};
	let height_map_vec_opt:Option<Vec<f32>> = height_map_js_array_opt.map(|arr| arr.to_vec());

	// Call the internal baker which uses the params_js_value
	texture_baker::bake_texture_from_js_params(
		rust_tex_type,
		width,
		height,
		global_seed,
		params_js_value,             // Pass as reference
		height_map_vec_opt.as_ref(), // Pass as reference
	)
}

// Utility to demonstrate FbmParameters can be passed (for UI default population
// etc.)
#[wasm_bindgen]
pub fn get_default_fbm_params() -> JsValue {
	let params = FbmParameters::default();
	serde_wasm_bindgen::to_value(params).unwrap()
}
// Similar default getters for AlbedoBakeParams, etc., can be useful for JS to
// know the structure.
