use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Srl {
    pub hash: String,
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct LevelListResponse {
    pub items: Vec<LevelInfo>,
    #[serde(rename = "pageCount")]
    pub page_count: i32,
}

#[derive(Serialize, Deserialize)]
pub struct ItemResponse<T> {
    pub item: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelEntity {
    pub archetype: String,
    pub data: Vec<LevelEntityData>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelEntityData {
    pub name: String,
    pub value: Option<f32>,
    pub r#ref: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct EffectData {
    pub clips: Vec<EffectClip>,
}

#[derive(Serialize, Deserialize)]
pub struct EffectClip {
    pub name: String,
    pub filename: String,
}

impl LevelEntity {
    pub fn get_value(&self, key: &str) -> Option<f32> {
        for data in self.data.iter() {
            if data.name == key {
                data.value?;

                return Some(data.value.unwrap());
            }
        }
        None
    }

    pub fn get_ref_raw(&self, key: &str) -> Option<String> {
        for data in self.data.iter() {
            if data.name == key {
                let r#ref = data.r#ref.as_ref()?;
                return Some(r#ref.to_string());
            }
        }
        None
    }

    pub fn get_ref(&self, entities: &[LevelEntity], key: &str) -> Option<LevelEntity> {
        let ref_raw = self.get_ref_raw(key)?;
        for entity in entities.iter() {
            if entity.name.as_ref().is_some_and(|name| name == &ref_raw) {
                return Some(entity.clone());
            }
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LevelData {
    #[serde(rename = "bgmOffset")]
    pub bgm_offset: f32,
    pub entities: Vec<LevelEntity>,
}

#[derive(Serialize, Deserialize)]
pub struct LevelInfo {
    pub title: String,
    pub artists: String,
    pub author: String,
    pub name: String,
    pub rating: i32,
    pub bgm: Srl,
    pub data: Srl,
    pub engine: EngineInfo,
}

#[derive(Serialize, Deserialize)]
pub struct EngineInfo {
    pub version: i32,
    pub effect: EffectInfo,
}

#[derive(Serialize, Deserialize)]
pub struct EffectInfo {
    pub audio: Srl,
    pub data: Srl,
}
