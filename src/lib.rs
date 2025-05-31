mod marching_cubes;

mod marching_cubes_tables;

mod noise_module;

mod scalar_field;

mod texture_baker;

// For mesh_scale, mesh_offset
use nalgebra::Point3;
// Assuming this derives Serialize, Deserialize
use noise_module::FbmParameters;
// Assuming these derive Serialize, Deserialize
use scalar_field::{GridDimensions, ScalarFieldShapeParams};
// For parameter structs
// No longer need: use serde::{Deserialize, Serialize};

// Texture Baker specific parameter structs
use texture_baker::{
	AlbedoBakeParams,

	AoBakeParams,

	HeightBakeParams,

	NormalBakeParams,

	RoughnessBakeParams,

	// Alias to avoid conflict if JsTextureType is same name
	TextureType as RustTextureType,
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
	// Expect GridDimensions JSON
	grid_dims_js:JsValue,

	global_seed:u32,

	// Expect ScalarFieldShapeParams JSON
	shape_params_js:JsValue,
) -> Result<Vec<f32>, JsValue> {
	let grid_dims:GridDimensions = serde_wasm_bindgen::from_value(grid_dims_js)?;

	let shape_params:ScalarFieldShapeParams = serde_wasm_bindgen::from_value(shape_params_js)?;

	Ok(scalar_field::generate_scalar_field(grid_dims, global_seed, &shape_params))
}

// --- Marching Cubes ---
#[wasm_bindgen]
pub struct MeshData {
	// These fields are not directly accessible from JS if they are not Copy.
	// We will provide getters.
	vertices:Vec<f32>,
	indices:Vec<u32>,
}

#[wasm_bindgen]
impl MeshData {
	// Constructor accessible from Rust
	pub fn new(vertices:Vec<f32>, indices:Vec<u32>) -> Self { MeshData { vertices, indices } }

	#[wasm_bindgen(getter)]
	pub fn vertices(&self) -> js_sys::Float32Array {
		// Clone data into a JS-compatible array
		js_sys::Float32Array::from(self.vertices.as_slice())
	}

	#[wasm_bindgen(getter)]
	pub fn indices(&self) -> js_sys::Uint32Array {
		// Clone data into a JS-compatible array
		js_sys::Uint32Array::from(self.indices.as_slice())
	}
}

#[wasm_bindgen]
pub fn extract_mesh_wasm(
	scalar_field_js_array:js_sys::Float32Array,

	// Expect GridDimensions JSON
	grid_dims_js:JsValue,

	iso_level:f32,

	mesh_scale_x:f32,

	mesh_scale_y:f32,

	mesh_scale_z:f32,

	mesh_offset_x:f32,

	mesh_offset_y:f32,

	mesh_offset_z:f32,
) -> Result<MeshData, JsValue> {
	let scalar_field_vec:Vec<f32> = scalar_field_js_array.to_vec();

	let grid_dims:GridDimensions = serde_wasm_bindgen::from_value(grid_dims_js)?;

	let mesh_scale = Point3::new(mesh_scale_x, mesh_scale_y, mesh_scale_z);

	let mesh_offset = Point3::new(mesh_offset_x, mesh_offset_y, mesh_offset_z);

	let mc_output =
		marching_cubes::run_marching_cubes(&scalar_field_vec, grid_dims, iso_level, mesh_scale, mesh_offset);

	// Use the Rust constructor for MeshData
	Ok(MeshData::new(mc_output.vertices, mc_output.indices))
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

	// JSON object for the specific bake params
	params_js_value:JsValue,

	// For normal baking
	height_map_js_array_opt:Option<js_sys::Float32Array>,
) -> Result<Vec<u8>, JsValue> {
	let rust_tex_type = match js_texture_type {
		JsTextureType::Albedo => RustTextureType::Albedo,

		JsTextureType::Height => RustTextureType::Height,

		JsTextureType::Normal => RustTextureType::Normal,

		JsTextureType::Roughness => RustTextureType::Roughness,

		JsTextureType::AmbientOcclusion => RustTextureType::AmbientOcclusion,
	};

	let height_map_vec_opt:Option<Vec<f32>> = height_map_js_array_opt.map(|arr| arr.to_vec());

	texture_baker::bake_texture_from_js_params_refined(
		rust_tex_type,
		width,
		height,
		global_seed,
		&params_js_value,
		height_map_vec_opt.as_ref(),
	)
}

// --- Default Parameter Getters ---

#[wasm_bindgen]
pub fn get_default_fbm_params() -> JsValue {
	let params = FbmParameters::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_scalar_field_shape_params() -> JsValue {
	let params = scalar_field::ScalarFieldShapeParams::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_grid_dimensions() -> JsValue {
	let params = scalar_field::GridDimensions::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_albedo_bake_params() -> JsValue {
	let params = AlbedoBakeParams::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_height_bake_params() -> JsValue {
	let params = HeightBakeParams::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_normal_bake_params() -> JsValue {
	let params = NormalBakeParams::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_roughness_bake_params() -> JsValue {
	let params = RoughnessBakeParams::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}

#[wasm_bindgen]
pub fn get_default_ao_bake_params() -> JsValue {
	let params = AoBakeParams::default();

	serde_wasm_bindgen::to_value(&params).unwrap()
}
