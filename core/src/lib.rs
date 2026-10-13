//! Unsealed draw engine: MTGJSON `booster` key parsing,
//! weighted draw with an injectable RNG, validation simulation.

pub mod booster;
pub mod draw;
pub mod sim;

pub use booster::{BoosterConfig, BoosterPack, BoosterSheet, ConfigError};
pub use draw::{open_booster, DrawError, DrawnCard};
pub use sim::{simulate, SimReport};
