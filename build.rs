use std::path::Path;
use std::process::Command;
use std::fs;

fn main(){
    for f in  fs::read_dir("./sounds").unwrap() {
        let f = f.unwrap();
        let path = String::from(f.path().to_str().unwrap()).replace(".mp3", ".pcm");
        if Path::new(&path).exists() {
            continue;
        }
        println!("Converting {}", f.path().to_str().unwrap());
        Command::new("ffmpeg")
            .arg("-i")
            .arg(f.path().to_str().unwrap())
            .arg("-ac")
            .arg("2")
            .arg("-f")
            .arg("s16le")
            .arg("-ar")
            .arg("48k")
            .arg(path)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }
}