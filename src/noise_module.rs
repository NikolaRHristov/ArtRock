use noise::{Fbm, MultiFractal, NoiseFn, Simplex};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct FbmParameters {
	pub octaves:u32,

	// Base frequency, effectively the initial "scale" for noise sampling
	pub frequency:f64,

	pub persistence:f64,

	pub lacunarity:f64,

	// Final multiplier for the noise output (after normalization)
	pub amplitude:f64,

	// Added to global seed for this specific noise layer
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

pub struct NoiseController {
	global_seed:u32,
	// We don't store FBM instances directly anymore if params are passed for each call

	// This controller can just hold the global seed and provide utility methods.
}

impl NoiseController {
	pub fn new(global_seed:u32) -> Self { NoiseController { global_seed } }

	// Generic FBM sampling function

	pub fn sample_fbm(
		&self,

		params:&FbmParameters,

		// 3D coordinates
		coords:[f64; 3],
	) -> f64 {
		let mut final_seed = self.global_seed;
		// Make seed_offset more impactful by mixing it differently

		// A simple addition might lead to similar patterns if global_seed changes
		// slightly

		// and seed_offset is large. Hashing or XORing might be better.

		// For now, wrapping_add is fine.

		final_seed = final_seed.wrapping_add(params.seed_offset);
		final_seed = final_seed.wrapping_add(
			((coords[0] * 73856093.0) as u32) ^ ((coords[1] * 19349663.0) as u32) ^ ((coords[2] * 83492791.0) as u32),
		);

		let simplex_source = Simplex::new(final_seed);
		let fbm_instance = Fbm::new(simplex_source)
            .set_octaves(params.octaves as usize)
             // Base frequency for noise-rs FBM; scale is applied to coords
			.set_frequency(1.0)
            .set_persistence(params.persistence)
            .set_lacunarity(params.lacunarity);

		let scaled_coords = [
			coords[0] * params.frequency,
			coords[1] * params.frequency,
			coords[2] * params.frequency,
		];

		// noise-rs FBM output is typically in range approx [-1, 1] depending on
		// octaves/persistence

		let noise_val = fbm_instance.get(scaled_coords);

		// Normalize to [0, 1] then apply amplitude.

		// A common way to estimate max possible FBM value for normalization:

		// let mut max_val_est = 0.0;

		// let mut amp_iter = 1.0;

		// for _ in 0..params.octaves {

		//     max_val_est += amp_iter;

		//     amp_iter *= params.persistence;

		// }

		// let normalized_noise = if max_val_est > 0.0 { noise_val / max_val_est } else
		// { noise_val };

		// For Simplex base, it's roughly [-1, 1], so a simple normalization is:

		let normalized_noise = (noise_val + 1.0) * 0.5;

		// Apply overall amplitude to the 0-1 ranged noise
		normalized_noise * params.amplitude
	}
}
