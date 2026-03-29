//! Asset Management based on Lifespan and Preloading.
//!
//! Evaluates the `SceneV2` against the current time to automatically
//! decide which assets need to be loaded into VRAM and which should be evicted.

use crate::scene::{AssetDef, SceneV2};
use std::collections::HashSet;

/// Abstract commands emitted by the AssetManager.
/// Consumed by the renderer (GPU or WASM) to execute the actual loads/evicts.
#[derive(Debug, Clone)]
pub enum AssetCommand {
    Load { id: String, asset: AssetDef },
    Evict { id: String },
}

/// Manages the lifecycle of assets based on entity lifespans.
#[derive(Debug, Default)]
pub struct AssetManager {
    /// Asset IDs currently marked as loaded in memory.
    pub loaded: HashSet<String>,
    /// Time (in seconds) to preload an asset before it appears.
    pub preload_margin: f64,
}

impl AssetManager {
    /// Creates a new AssetManager with a specified preload margin.
    pub fn new(preload_margin: f64) -> Self {
        Self {
            loaded: HashSet::new(),
            preload_margin,
        }
    }

    /// Evaluates the scene at the given time and returns a list of commands
    /// to load needed assets or evict unused ones, utilizing Lifespan bounds.
    pub fn update(&mut self, scene: &SceneV2, time: f64) -> Vec<AssetCommand> {
        let mut needed: HashSet<String> = HashSet::new();

        // 1. Identify which assets are needed right now or in the immediate future
        for entity in &scene.entities {
            let asset_id = entity.components.get("videoSource").and_then(|v| v.get("assetId")).and_then(|v| v.as_str())
                .or_else(|| entity.components.get("imageSource").and_then(|v| v.get("assetId")).and_then(|v| v.as_str()))
                .or_else(|| entity.components.get("audioSource").and_then(|v| v.get("assetId")).and_then(|v| v.as_str()));

            if let Some(aid) = asset_id {
                let mut is_needed = false;
                if let Some(ls) = entity.components.get("lifespan") {
                    if let Ok(lifespan) = serde_json::from_value::<crate::scene::Lifespan>(ls.clone()) {
                        let active_start = lifespan.start - self.preload_margin;
                        if time >= active_start && time < lifespan.end {
                            is_needed = true;
                        }
                    } else { is_needed = true; } // play it safe
                } else {
                    is_needed = true; // no lifespan = always active
                }

                if is_needed {
                    needed.insert(aid.to_string());
                }
            }
        }

        let mut commands = Vec::new();

        // 2. Evict assets no longer needed
        let to_evict: Vec<String> = self.loaded.difference(&needed).cloned().collect();
        for id in to_evict {
            commands.push(AssetCommand::Evict { id: id.clone() });
            self.loaded.remove(&id);
        }

        // 3. Load newly needed assets
        let to_load: Vec<String> = needed.difference(&self.loaded).cloned().collect();
        for id in to_load {
            if let Some(asset) = scene.assets.get(&id) {
                commands.push(AssetCommand::Load {
                    id: id.clone(),
                    asset: asset.clone(),
                });
                self.loaded.insert(id);
            } else {
                log::warn!("AssetManager: Entity referenced missing asset '{}'", id);
            }
        }

        commands
    }
}
