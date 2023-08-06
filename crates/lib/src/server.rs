use crate::level::Level;
use crate::sonolus::{EffectData, EffectInfo, ItemResponse, LevelData, LevelInfo, Srl};
use crate::sound::Effect;
use crate::utils::debug;

use anyhow::Result;
use dirs::cache_dir;
use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use std::io::Read;
use std::path::Path;
use tokio::try_join;

#[derive(Debug, Clone)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub color: i32,
    pub url: String,
}

static CACHE_DIR: Lazy<Box<Path>> = Lazy::new(|| {
    let mut path = cache_dir().or_else(|| "./cache".parse().ok()).unwrap();
    path.push("pjsekai-soundgen-rust");
    path.into_boxed_path()
});

impl Server {
    pub fn guess(level_name: &str) -> Result<Server> {
        if level_name.starts_with("ptlv-") {
            Ok(Server {
                id: "potato_leaves".to_string(),
                name: "Potato Leaves".to_string(),
                color: 0x88cb7f,
                url: "https://ptlv.sevenc7c.com".to_string(),
            })
        } else if level_name.starts_with("chcy-") {
            Ok(Server {
                id: "chart_cyanvas".to_string(),
                name: "Chart Cyanvas".to_string(),
                color: 0x83ccd2,
                url: "https://cc.sevenc7c.com".to_string(),
            })
        } else {
            Err(anyhow::anyhow!("サーバーを特定できませんでした。"))
        }
    }

    async fn fetch_srl_with_cache(&self, srl: &Srl) -> Result<Vec<u8>> {
        let key = format!("{}-{}", self.id, srl.hash);

        debug!(&key);

        let cache_path = CACHE_DIR.join(&key);
        if let Ok(cache) = tokio::fs::read(&cache_path).await {
            debug!("cache hit");
            return Ok(cache);
        }
        debug!("cache miss");

        let client = reqwest::Client::new();
        let url = self.merge_url(&srl.url);
        debug!(&url);
        let bgm_response = client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("{}の取得に失敗しました。: {}", srl.r#type, e))?;

        if !bgm_response.status().is_success() {
            return Err(anyhow::anyhow!("{}の取得に失敗しました。", srl.r#type));
        }

        let bytes = bgm_response
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("{}の取得に失敗しました。: {}", srl.r#type, e))?
            .to_vec();

        tokio::fs::create_dir_all(CACHE_DIR.as_ref()).await?;
        tokio::fs::write(&cache_path, &bytes).await?;

        Ok(bytes)
    }

    pub async fn fetch_level(&self, level_name: &str) -> Result<Level> {
        let client = reqwest::Client::new();
        let level_info = client
            .get(format!("{}/sonolus/levels/{}", self.url, level_name).as_str())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("譜面情報の取得に失敗しました。: {}", e))?
            .json::<ItemResponse<LevelInfo>>()
            .await
            .map_err(|e| anyhow::anyhow!("譜面情報の取得に失敗しました。: {}", e))?
            .item;
        let data_bytes = &self
            .fetch_srl_with_cache(&level_info.data)
            .await
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        let mut data_raw = GzDecoder::new(&data_bytes[..]);
        let mut buf = Vec::new();
        data_raw
            .read_to_end(&mut buf)
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        let level_data = serde_json::from_slice::<LevelData>(&buf[..])
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        Ok(Level::new(self.clone(), level_info, level_data))
    }

    pub fn merge_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}/{}", self.url, path).replace("//", "/")
        }
    }

    pub async fn fetch_effect(&self, effect: EffectInfo) -> Result<Effect> {
        let (data_compressed, audio) =
            try_join!(self.fetch_srl_with_cache(&effect.data), self.fetch_srl_with_cache(&effect.audio))
                .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        let zip = zip::ZipArchive::new(std::io::Cursor::new(audio))
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        let mut data_raw = GzDecoder::new(&data_compressed[..]);
        let mut buf = Vec::new();
        data_raw.read_to_end(&mut buf).map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;
        let data = serde_json::from_slice::<EffectData>(&buf[..])
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        Effect::new(data, zip)
    }
}
