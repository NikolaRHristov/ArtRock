use noise::{Fbm, MultiFractal, NoiseFn, Simplex};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct FbmParameters {
	pub octaves:u32,

	pub frequency:f64,

	pub persistence:f64,

	pub lacunarity:f64,

	pub amplitude:f64,

	pub seed_offset:u32,
}

impl Default for FbmParameters {
	fn default() -> Self {
		FbmParameters {
			octaves:5,

			frequency:1.0,

			persistence:0.5,

			lacunarity:2.0,

			amplitude:1.0,

			seed_offset:0,
		}
	}
}

#[derive(Clone, Copy)]
pub struct NoiseController {
	global_seed:u32,
}

impl NoiseController {
	pub fn new(global_seed:u32) -> Self { NoiseController { global_seed } }

	pub fn sample_fbm(&self, params:&FbmParameters, coords:[f64; 3]) -> f64 {
		let final_seed = self.global_seed.wrapping_add(params.seed_offset);

		let fbm_instance:Fbm<Simplex> = Fbm::new(final_seed)
			.set_octaves(params.octaves as usize)
			.set_frequency(1.0)
			.set_persistence(params.persistence)
			.set_lacunarity(params.lacunarity);

		let scaled_coords = [
			coords[0] * params.frequency,
			coords[1] * params.frequency,
			coords[2] * params.frequency,
		];

		let noise_val = fbm_instance.get(scaled_coords);

		let normalized_noise = (noise_val + 1.0) * 0.5;

		normalized_noise * params.amplitude
	}
}
