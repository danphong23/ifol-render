use serde::{Deserialize, Serialize};
use crate::schema::tracks::{FloatTrack, StringTrack};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneV2 {
    #[serde(default)]
    pub assets: std::collections::HashMap<String, AssetDef>,
    #[serde(default)]
    pub entities: Vec<EntityV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AssetDef {
    Video { url: String },
    Image { url: String },
    Font { url: String },
    Audio { url: String },
    Shader { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShaderScope {
    Clipped,
    #[default]
    Padded,
    Layer,
    Camera,
    Masked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialV2 {
    pub shader_id: String,
    
    #[serde(default)]
    pub scope: ShaderScope,
    
    #[serde(default)]
    pub float_uniforms: std::collections::HashMap<String, FloatTrack>,
    
    #[serde(default)]
    pub string_uniforms: std::collections::HashMap<String, StringTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityV2 {
    pub id: String,
    #[serde(flatten)]
    pub components: std::collections::HashMap<String, serde_json::Value>,
}
