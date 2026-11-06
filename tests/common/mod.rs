#![allow(dead_code)]
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use zip::{ZipWriter, write::SimpleFileOptions};

pub fn archive(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
    for (name, content) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

pub fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wowza"));
    command
        .arg("--path")
        .arg(root)
        .env_remove("WOWZA_FOREVER_PATH")
        .env_remove("WOWZA_TEST_STOP_AFTER")
        .env("WOWZA_CONFIG_FILE", root.join("test-config.json"));
    command
}

pub fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[cfg(windows)]
pub fn junction(link: &Path, target: &Path) {
    let output = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    success(output);
}

pub fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                result.insert(relative, Vec::new());
                walk(root, &path, result);
            } else {
                result.insert(relative, fs::read(path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

pub struct Installation {
    dir: tempfile::TempDir,
}

impl Installation {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("wowza test '")
            .tempdir()
            .unwrap();
        fs::create_dir_all(dir.path().join("Interface/AddOns")).unwrap();
        fs::create_dir_all(dir.path().join("WTF")).unwrap();
        fs::write(dir.path().join("WTF/settings"), "keep settings").unwrap();
        Self { dir }
    }
    pub fn root(&self) -> &Path {
        self.dir.path()
    }
    pub fn addons(&self) -> PathBuf {
        self.root().join("Interface/AddOns")
    }
    pub fn command(&self) -> Command {
        command(self.root())
    }
    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    pub fn input(&self, args: &[&str], answer: &[u8]) -> Output {
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(answer).unwrap();
        child.wait_with_output().unwrap()
    }
    pub fn zip(&self, name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let path = self.root().join(name);
        archive(&path, entries);
        path
    }
    pub fn install(&self, archive: &Path) {
        success(
            self.command()
                .arg("install-archive")
                .arg(archive)
                .arg("--yes")
                .output()
                .unwrap(),
        );
    }
    pub fn preserved(&self) {
        assert_eq!(
            fs::read_to_string(self.root().join("WTF/settings")).unwrap(),
            "keep settings"
        );
    }
}
