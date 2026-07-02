//! starfox.ini configuration.
//!
//! Port (C oracle): `src/config.c` — same sections/keys, same defaults
//! (`Config_SetDefaults`, src/config.c:7), same atoi/atof semantics
//! (non-numeric values parse to 0, trailing junk ignored).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    /// C `rom_file` ([General] RomFile).
    pub rom_file: String,
    /// C `asset_dir` ([General] AssetDir) — the directory holding
    /// `bg/ font/ map/ snd/ sprites/ title/`.
    pub asset_dir: String,
    pub window_width: i32,
    pub window_height: i32,
    /// 0 = windowed, 1 = fullscreen, 2 = fullscreen desktop.
    pub fullscreen: i32,
    /// 0 = vsync.
    pub target_fps: i32,
    pub widescreen: i32,
    pub msaa: i32,
    pub crt_filter: i32,
    pub bloom_enabled: i32,
    pub sample_rate: i32,
    pub buffer_size: i32,
    pub volume: i32,
    pub deadzone: f32,
}

impl Default for Config {
    /// C `Config_SetDefaults` (src/config.c:7).
    fn default() -> Self {
        Config {
            rom_file: "Star Fox (USA) (Rev 2).sfc".into(),
            asset_dir: "data".into(),
            window_width: 1280,
            window_height: 720,
            fullscreen: 0,
            target_fps: 0,
            widescreen: 0,
            msaa: 4,
            crt_filter: 0,
            bloom_enabled: 0,
            sample_rate: 48000,
            buffer_size: 1024,
            volume: 100,
            deadzone: 0.15,
        }
    }
}

/// C `atoi`: optional sign + leading digits, 0 on garbage.
fn atoi(s: &str) -> i32 {
    let s = s.trim_start();
    let mut chars = s.char_indices();
    let mut end = 0;
    let mut seen_digit = false;
    if let Some((_, c)) = chars.next() {
        if c == '+' || c == '-' || c.is_ascii_digit() {
            end = c.len_utf8();
            seen_digit = c.is_ascii_digit();
            for (i, c) in chars {
                if c.is_ascii_digit() {
                    end = i + 1;
                    seen_digit = true;
                } else {
                    break;
                }
            }
        }
    }
    if !seen_digit {
        return 0;
    }
    s[..end].parse::<i64>().unwrap_or(0).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// C `atof` subset (leading float, 0.0 on garbage).
fn atof(s: &str) -> f32 {
    let s = s.trim_start();
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || c == '.' || ((c == '+' || c == '-') && i == 0) {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    s[..end].parse::<f32>().unwrap_or(0.0)
}

impl Config {
    /// C `Config_Load` (src/config.c:32): defaults, then INI overlay.
    /// Missing file keeps defaults (message matches the C).
    pub fn load(path: &Path) -> Config {
        let mut cfg = Config::default();
        let Ok(text) = std::fs::read_to_string(path) else {
            println!("Config: {} not found, using defaults", path.display());
            return cfg;
        };

        let mut section = String::new();
        for line in text.lines() {
            let s = line.trim();
            if s.is_empty() || s.starts_with('#') || s.starts_with(';') {
                continue;
            }
            if let Some(rest) = s.strip_prefix('[') {
                if let Some(end) = rest.find(']') {
                    section = rest[..end].to_string();
                }
                continue;
            }
            let Some((key, val)) = s.split_once('=') else { continue };
            let (key, val) = (key.trim(), val.trim());

            match (section.as_str(), key) {
                ("General", "RomFile") => cfg.rom_file = val.to_string(),
                ("General", "AssetDir") => cfg.asset_dir = val.to_string(),
                ("Video", "WindowWidth") => cfg.window_width = atoi(val),
                ("Video", "WindowHeight") => cfg.window_height = atoi(val),
                ("Video", "Fullscreen") => cfg.fullscreen = atoi(val),
                ("Video", "TargetFPS") => cfg.target_fps = atoi(val),
                ("Video", "Widescreen") => cfg.widescreen = atoi(val),
                ("Video", "MSAA") => cfg.msaa = atoi(val),
                ("Video", "CRTFilter") => cfg.crt_filter = atoi(val),
                ("Video", "BloomEnabled") => cfg.bloom_enabled = atoi(val),
                ("Audio", "SampleRate") => cfg.sample_rate = atoi(val),
                ("Audio", "BufferSize") => cfg.buffer_size = atoi(val),
                ("Audio", "Volume") => cfg.volume = atoi(val),
                ("Input", "DeadZone") => cfg.deadzone = atof(val),
                _ => {}
            }
        }
        println!("Config: loaded {}", path.display());
        cfg
    }

    /// The sf-render asset root: the directory CONTAINING `data/`
    /// (sf_render passes join into `data/...`). For the default
    /// `AssetDir = data` this is the working directory, matching the C
    /// build's cwd-relative asset loads.
    pub fn asset_root(&self) -> PathBuf {
        let dir = Path::new(&self.asset_dir);
        match dir.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        }
    }

    /// The audio data dir (`<asset_dir>` itself; sf_audio joins `snd/`).
    pub fn audio_asset_dir(&self) -> PathBuf {
        PathBuf::from(&self.asset_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_config_c() {
        let c = Config::default();
        assert_eq!(c.window_width, 1280);
        assert_eq!(c.window_height, 720);
        assert_eq!(c.msaa, 4);
        assert_eq!(c.sample_rate, 48000);
        assert_eq!(c.volume, 100);
        assert!((c.deadzone - 0.15).abs() < 1e-6);
        assert_eq!(c.asset_dir, "data");
        assert_eq!(c.rom_file, "Star Fox (USA) (Rev 2).sfc");
    }

    #[test]
    fn ini_overlay_and_atoi_semantics() {
        let dir = std::env::temp_dir().join("sf_app_cfg_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("starfox.ini");
        std::fs::write(
            &p,
            "# comment\n[Video]\nWindowWidth = 1920\nMSAA = junk\n; c\n[Input]\nDeadZone = 0.25\n[Audio]\nVolume=80\n",
        )
        .unwrap();
        let c = Config::load(&p);
        assert_eq!(c.window_width, 1920);
        assert_eq!(c.msaa, 0); // atoi("junk") == 0, like the C
        assert_eq!(c.volume, 80);
        assert!((c.deadzone - 0.25).abs() < 1e-6);
        assert_eq!(c.window_height, 720); // untouched default
    }
}
