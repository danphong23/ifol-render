use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParentId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MaskId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Layer(pub i32);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Materials(pub Vec<crate::scene::MaterialV2>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FloatUniforms(pub std::collections::HashMap<String, crate::scene::FloatTrack>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StringUniforms(pub std::collections::HashMap<String, crate::scene::StringTrack>);
