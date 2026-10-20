use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

/// The connector is embedded so `bfish-mcp --install-plugin` works without a
/// checkout, an npm install, or a running Studio.
pub const PLUGIN_SOURCE: &str = include_str!("../../../plugin/src/init.server.lua");

pub fn plugin_xml() -> String {
    let source = xml_escape(PLUGIN_SOURCE);
    format!(
        r#"<roblox xmlns:xmime="http://www.w3.org/2005/05/xmlmime" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.roblox.com/roblox.xsd" version="4">
  <Item class="Script" referent="RBX0">
    <Properties>
      <string name="Name">BFishConnector</string>
      <bool name="Disabled">false</bool>
      <ProtectedString name="Source">{source}</ProtectedString>
    </Properties>
  </Item>
</roblox>
"#
    )
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn plugins_dir() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("BFISH_PLUGINS_DIR") {
        if !custom.trim().is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return Ok(PathBuf::from(local).join("Roblox").join("Plugins"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(docs) = dirs::document_dir() {
            return Ok(docs.join("Roblox").join("Plugins"));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(data) = dirs::data_dir() {
            return Ok(data.join("roblox").join("Plugins"));
        }
    }

    bail!("Could not find the Roblox Plugins folder. Set BFISH_PLUGINS_DIR to its path and retry.")
}

/// Install (or update) the connector and return the written path.
pub fn install_plugin() -> Result<PathBuf> {
    let dir = plugins_dir()?;
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating Roblox Plugins folder {}", dir.display()))?;
    let path = dir.join("BFishConnector.rbxmx");
    fs::write(&path, plugin_xml())
        .with_context(|| format!("writing connector to {}", path.display()))?;
    Ok(path)
}

pub fn plugin_path() -> Result<PathBuf> {
    Ok(plugins_dir()?.join("BFishConnector.rbxmx"))
}
