use std::{collections::HashMap, io::Read};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use crate::sound::SOUND_MAP;

#[derive(Serialize, Deserialize)]
pub struct SRL {
    pub hash: String,
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct LevelListResponse {
    pub items: Vec<Level>,
    #[serde(rename = "pageCount")]
    pub page_count: i32,
}

#[derive(Serialize, Deserialize)]
pub struct SingleLevelResponse {
    pub item: Level,
}

#[derive(Serialize, Deserialize)]
pub struct LevelEntity {
    pub archetype: i32,
    pub data: Option<LevelEntityData>,
}

#[derive(Serialize, Deserialize)]
pub struct LevelEntityData {
    pub values: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct LevelData {
    pub entities: Vec<LevelEntity>,
}

#[derive(Serialize, Deserialize)]
pub struct Level {
    pub title: String,
    pub artists: String,
    pub author: String,
    pub name: String,
    pub bgm: SRL,
    pub data: SRL,
}

#[derive(Debug)]
pub enum LevelError {
    NotFound,
    InvalidFormat,
}

impl Level {
    pub fn fetch(id: &str) -> Result<Level, LevelError> {
        let url = format!("https://servers-legacy.purplepalette.net/levels/{}", id);
        let resp = reqwest::blocking::get(&url).unwrap();
        if resp.status() != reqwest::StatusCode::OK {
            return Err(LevelError::NotFound);
        }
        let buf: &[u8] = &resp.bytes().unwrap()[..];
        let level: SingleLevelResponse = serde_json::from_slice(&buf).unwrap();
        Ok(level.item)
    }

    pub fn get_sound_timings(self, offset: f32) -> (HashMap<String, Vec<f32>>, HashMap<String, Vec<(f32, f32)>>) {
        let client = reqwest::blocking::Client::new();
        let data = client
            .get(format!("https://servers-legacy.purplepalette.net{}", self.data.url).as_str())
            .send()
            .unwrap()
            .bytes()
            .unwrap();
        let mut level_data_raw = GzDecoder::new(&data[..]);
        let mut buf = Vec::new();
        level_data_raw.read_to_end(&mut buf).unwrap();
        let level_data = serde_json::from_str::<LevelData>(&String::from_utf8_lossy(&buf)).unwrap();
        let mut timings: HashMap<String, Vec<f32>> = HashMap::new();
        let mut connect_timings: HashMap<String, Vec<(f32, f32)>> = HashMap::new();
        for note in level_data.entities.iter() {
            let sound_map_data = SOUND_MAP.get(&note.archetype);
            if sound_map_data.is_none() {
                continue;
            }
            let sound_data = sound_map_data.unwrap().1.to_string();
            if sound_data.contains("connect") {
                if !connect_timings.contains_key(&sound_data) {
                    connect_timings.insert(sound_data.clone(), vec![]);
                }
                connect_timings.get_mut(&sound_data).unwrap().push((
                    note.data.as_ref().unwrap().values[0] + offset,
                    note.data.as_ref().unwrap().values[3] + offset,
                ));
                continue;
            }
            if !timings.contains_key(&sound_data) {
                timings.insert(sound_data.clone(), vec![]);
            }
            let values = note.data.as_ref().unwrap().values.clone();
            timings.get_mut(&sound_data).unwrap().push(values[0] + offset);
        }
        timings.values_mut().for_each(|v| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.dedup()
        });
        (timings, connect_timings)
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} : {} / {} #{}", self.title, self.artists, self.author, self.name)
    }
}
