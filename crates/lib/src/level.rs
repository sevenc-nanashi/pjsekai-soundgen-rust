use crate::{
    server::Server,
    sonolus::{LevelData, LevelInfo},
};
use anyhow::Result;

pub struct Level {
    pub server: Server,
    pub info: LevelInfo,
    pub data: LevelData,
}

impl Level {
    pub fn new(server: Server, info: LevelInfo, data: LevelData) -> Self {
        Self { server, info, data }
    }

    pub async fn fetch_bgm(&self, buf: &mut Vec<u8>) -> Result<()> {
        let client = reqwest::Client::new();
        let bgm_response = client
            .get(self.server.merge_url(&self.info.bgm.url))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("BGMの取得に失敗しました。: {}", e))?;

        if !bgm_response.status().is_success() {
            return Err(anyhow::anyhow!("BGMの取得に失敗しました。"));
        }

        buf.append(
            &mut bgm_response
                .bytes()
                .await
                .map_err(|e| anyhow::anyhow!("BGMの取得に失敗しました。: {}", e))?
                .to_vec(),
        );
        Ok(())
    }
}
