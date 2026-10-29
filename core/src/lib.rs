//! Unsealed draw engine: MTGJSON `booster` key parsing,
//! weighted draw with an injectable RNG, validation simulation,
//! compact catalog files and update planning.

pub mod achievements;
pub mod artifacts;
pub mod betting;
pub mod booster;
pub mod builder;
pub mod catalog;
pub mod crafted;
pub mod cycles;
pub mod draw;
pub mod economy;
pub mod import;
pub mod limited;
pub mod pack;
pub mod scenes;
pub mod scryfall;
pub mod sim;
pub mod update;

pub use booster::{BoosterConfig, BoosterPack, BoosterSheet, ConfigError};
pub use draw::{open_booster, open_booster_excluding, ColorIndex, DrawError, DrawnCard};
pub use sim::{simulate, SimReport};
