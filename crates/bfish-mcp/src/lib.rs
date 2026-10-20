pub mod bridge;
pub mod engine;
pub mod install;
pub mod mcp;
pub mod tools;

pub use bridge::{Bridge, BridgeStatus, DEFAULT_STUDIO_PORT};
pub use engine::{existing_bridge, Engine};
pub use install::{install_plugin, plugin_path, plugin_xml, plugins_dir};

#[cfg(test)]
mod tests;
