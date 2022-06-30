use std::collections::HashMap;

use std::io::Write;
use std::process::{Command, Stdio};

use once_cell::sync::Lazy;

pub static SOUND_MAP: Lazy<HashMap<i32, (&[u8], &'static str)>> = Lazy::new(|| {
    HashMap::from([
        (3, (&include_bytes!("../sounds/tap.mp3")[..], "tap")),
        (4, (&include_bytes!("../sounds/flick.mp3")[..], "flick")),
        (5, (&include_bytes!("../sounds/tap.mp3")[..], "slide_tap")),
        (6, (&include_bytes!("../sounds/tick.mp3")[..], "slide_tick")),
        (7, (&include_bytes!("../sounds/tap.mp3")[..], "slide_tap")),
        (8, (&include_bytes!("../sounds/flick.mp3")[..], "flick")),
        (9, (&include_bytes!("../sounds/connect.mp3")[..], "connect")),
        (10, (&include_bytes!("../sounds/critical_tap.mp3")[..], "critical_tap")),
        (11, (&include_bytes!("../sounds/critical_flick.mp3")[..], "critical_flick")),
        (12, (&include_bytes!("../sounds/tap.mp3")[..], "slide_tap")),
        (13, (&include_bytes!("../sounds/critical_tick.mp3")[..], "critical_slide_tick")),
        (14, (&include_bytes!("../sounds/tap.mp3")[..], "slide_tap")),
        (15, (&include_bytes!("../sounds/critical_flick.mp3")[..], "critical_flick")),
        (16, (&include_bytes!("../sounds/critical_connect.mp3")[..], "critical_connect")),
    ])
});

pub static NOTE_NAME_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("tap", "タップ"),
        ("flick", "フリック"),
        ("slide_tap", "スライド始点/終点"),
        ("slide_tick", "スライド中継点"),
        ("connect", "ロング"),
    ])
});

pub struct Sound {
    pub data: Vec<i16>,
    pub bitrate: u32,
}

impl Sound {
    pub fn load(buf: &Vec<u8>) -> Sound {
        let mut child = Command::new("ffmpeg")
            .arg("-i")
            .arg("-")
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
        let local_buf = buf.clone();
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
            data: output_buf.chunks_exact(2).into_iter().map(|a| i16::from_le_bytes([a[0], a[1]])).collect(),
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
            .arg("-f")
            .arg("s16le")
            .arg("-c:a")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg(format!("{}", self.bitrate))
            .arg("-ac")
            .arg("2")
            .arg("-i")
            .arg("-")
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

impl std::clone::Clone for Sound {
    fn clone(&self) -> Self {
        Sound {
            data: self.data.clone(),
            bitrate: self.bitrate,
        }
    }
}
