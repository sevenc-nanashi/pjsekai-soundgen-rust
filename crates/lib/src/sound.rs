use std::collections::HashMap;
use std::io::Read;

use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use zip::ZipArchive;

use crate::sonolus::EffectData;

pub static SOUND_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("NormalTapNote", "#PERFECT"),
        ("CriticalTapNote", "Sekai Critical Tap"),
        ("NormalFlickNote", "#PERFECT_ALTERNATIVE"),
        ("CriticalFlickNote", "Sekai Critical Flick"),
        ("NormalSlideStartNote", "#PERFECT"),
        ("CriticalSlideStartNote", "#PERFECT"),
        ("NormalSlideEndNote", "#PERFECT"),
        ("CriticalSlideEndNote", "#PERFECT"),
        ("NormalSlideEndFlickNote", "#PERFECT_ALTERNATIVE"),
        ("CriticalSlideEndFlickNote", "Sekai Critical Flick"),
        ("NormalSlideTickNote", "Sekai Tick"),
        ("CriticalSlideTickNote", "Sekai Critical Tick"),
        ("NormalAttachedSlideTickNote", "Sekai Tick"),
        ("CriticalAttachedSlideTickNote", "Sekai Critical Tick"),
        ("NormalTraceNote", "Sekai+ Normal Trace"),
        ("CriticalTraceNote", "Sekai+ Critical Trace"),
        ("NormalTraceFlickNote", "Sekai+ Normal Trace Flick"),
        ("CriticalTraceFlickNote", "Sekai+ Critical Trace Flick"),
        ("NonDirectionalTraceFlickNote", "Sekai+ Normal Trace Flick"),
        ("TraceSlideStartNote", "Sekai+ Normal Trace"),
        ("TraceSlideEndNote", "Sekai+ Normal Trace"),
    ])
});
pub static LOOP_SOUND_MAP: Lazy<HashMap<&'static str, &'static str>> =
    Lazy::new(|| HashMap::from([("NormalSlideConnector", "#HOLD"), ("CriticalSlideConnector", "Sekai Critical Hold")]));

#[derive(Debug, Clone)]
pub struct Sound {
    pub data: Vec<i16>,
    pub bitrate: u32,
}

impl Sound {
    pub fn load(buf: &[u8]) -> Sound {
        Sound::load_with_args(buf, &[])
    }
    pub fn load_with_args(buf: &[u8], args: &[String]) -> Sound {
        let mut child = Command::new("ffmpeg")
            .arg("-i")
            .arg("-")
            .args(args)
            .arg("-ac")
            .arg("2")
            .arg("-f")
            .arg("s16le")
            .arg("-ar")
            .arg("48k")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let local_buf = buf.to_vec();
        let mut stdin = child.stdin.take().unwrap();
        let thread = std::thread::spawn(move || {
            stdin.write_all(&local_buf).unwrap();
        });
        let output = child.wait_with_output().unwrap();
        thread.join().unwrap();
        if !output.status.success() {
            panic!("ffmpeg failed");
        }
        let output_buf = output.stdout;
        Sound {
            data: output_buf.chunks_exact(2).map(|a| i16::from_le_bytes([a[0], a[1]])).collect(),
            bitrate: 48000,
        }
    }

    pub fn empty(bitrate: Option<u32>) -> Sound {
        Sound {
            data: vec![],
            bitrate: bitrate.unwrap_or(48000),
        }
    }

    pub fn overlay_at(self, other: &Sound, seconds: f32) -> Sound {
        let mut new_data = self.data.clone();
        let start_index = (seconds * self.bitrate as f32) as usize * 2;
        let end_index = start_index + other.data.len();
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        new_data.splice(
            start_index..end_index,
            other
                .data
                .iter()
                .cloned()
                .zip(new_data.clone()[start_index..end_index].iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect::<Vec<i16>>(),
        );

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }

    pub fn overlay_loop(self, other: &Sound, start: f32, end: f32) -> Sound {
        let mut new_data = self.data.clone();
        let start_index = (start * self.bitrate as f32) as usize * 2;
        let end_index = (end * self.bitrate as f32) as usize * 2;
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        new_data.splice(
            start_index..end_index,
            other
                .data
                .iter()
                .cycle()
                .cloned()
                .zip(new_data.clone()[start_index..end_index].iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect::<Vec<i16>>(),
        );

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }

    pub fn export(self, path: &str) {
        let mut child = Command::new("ffmpeg")
            .arg("-y")
            .args(["-f", "s16le"])
            .args(["-c:a", "pcm_s16le"])
            .args(["-ar", self.bitrate.to_string().as_str()])
            .args(["-ac", "2"])
            .args(["-i", "-"])
            .args(["-b:a", "480k"])
            .args(["-maxrate", "480k"])
            .args(["-bufsize", "480k"])
            .args(["-minrate", "480k"])
            .arg(path)
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(&self.data.iter().flat_map(|a| a.to_le_bytes().to_vec()).collect::<Vec<u8>>())
            .unwrap();
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        if !output.status.success() {
            panic!("ffmpeg failed");
        }
    }

    pub fn overlay_until(self, sound: &Sound, start: f32, end: f32) -> Sound {
        let mut new_data = self.data.clone();
        let start_index = (start * self.bitrate as f32) as usize * 2;
        let mut end_index = (end * self.bitrate as f32) as usize * 2;
        if (end_index - start_index) > sound.data.len() {
            end_index = start_index + sound.data.len();
        }
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        new_data.splice(
            start_index..end_index,
            sound
                .data
                .iter()
                .cloned()
                .zip(new_data.clone()[start_index..end_index - 1].iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect::<Vec<i16>>(),
        );

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }
}

impl std::ops::Mul<f32> for Sound {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        let mut result = vec![];
        for a in self.data.iter() {
            result.push(((*a as f32) * rhs) as i16);
        }
        Sound {
            data: result,
            bitrate: self.bitrate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub audio: HashMap<String, Sound>,
}

impl Effect {
    pub fn new(data: EffectData, mut zip: ZipArchive<Cursor<Vec<u8>>>) -> Result<Self> {
        let mut audio = HashMap::new();
        for clip in data.clips {
            let mut file =
                zip.by_name(&clip.filename).map_err(|_| anyhow!("効果音のファイルが見つかりませんでした"))?;
            let mut buf = vec![];
            file.read_to_end(&mut buf).map_err(|_| anyhow!("効果音のファイルが読み込めませんでした"))?;
            audio.insert(clip.name, Sound::load(&buf));
        }
        Ok(Self { audio })
    }
}
