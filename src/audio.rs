//! Audio engine — built on rodio/symphonia.

use crate::models::{PlaybackState, SoundFile};
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::stream::MixerDeviceSink;
use rodio::{Decoder, Player, Source};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Decoded duration of an audio file in seconds (best effort).
pub fn probe_duration(path: &Path) -> f32 {
    if let Ok(file) = File::open(path) {
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let reader = BufReader::new(file);
        if let Ok(dec) = Decoder::builder()
            .with_data(reader)
            .with_byte_len(len)
            .build()
        {
            return dec.total_duration().map(|d| d.as_secs_f32()).unwrap_or(0.0);
        }
    }
    0.0
}

struct ActiveSound {
    player: Player,
    /// Kept alive so the device stream stays open while this sound plays.
    _stream: MixerDeviceSink,
    path: PathBuf,
    /// Per-sound volume (0..=1).
    volume: f32,
    looped: bool,
    started: Instant,
    duration_secs: f32,
}

#[derive(Default)]
struct Inner {
    active: HashMap<String, ActiveSound>,
    master_volume: f32,
    output_device: Option<String>,
}

/// Multi-sound playback engine shared between the UI thread and hotkey handler.
pub struct AudioPlayer {
    inner: Arc<Mutex<Inner>>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    /// List available output device names ("default" first).
    pub fn output_devices() -> Vec<String> {
        let mut names = vec!["default".to_string()];
        let host = cpal::default_host();
        if let Ok(devs) = host.output_devices() {
            for dev in devs {
                if let Ok(desc) = dev.description() {
                    let name = desc.name().to_string();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        names
    }

    /// Select the output device ("default" → OS default). Applies to future plays.
    pub fn set_output_device(&self, device: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.output_device = if device.eq_ignore_ascii_case("default") {
            None
        } else {
            Some(device.to_string())
        };
    }

    pub fn set_master_volume(&self, v: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.master_volume = v.clamp(0.0, 1.0);
        let mv = inner.master_volume;
        for a in inner.active.values() {
            a.player.set_volume((mv * a.volume).clamp(0.0, 1.0));
        }
    }

    /// Open a player on the given device (None = OS default).
    fn open_player(device: Option<&str>) -> Option<(Player, MixerDeviceSink)> {
        let stream = if let Some(name) = device {
            let host = cpal::default_host();
            let dev = host
                .output_devices()
                .ok()?
                .find(|d| {
                    d.description()
                        .map(|desc| desc.name() == name)
                        .unwrap_or(false)
                })?;
            rodio::DeviceSinkBuilder::from_device(dev)
                .ok()?
                .open_stream()
                .ok()?
        } else {
            rodio::DeviceSinkBuilder::open_default_sink().ok()?
        };
        let player = Player::connect_new(stream.mixer());
        Some((player, stream))
    }

    /// Play a sound (stops an existing instance of the same sound first).
    pub fn play(&self, sound: &SoundFile, per_sound_volume: f32) {
        self.stop(&sound.id);
        self.play_raw(&sound.id, &sound.file_path, per_sound_volume, sound.looped);
    }

    fn play_raw(&self, id: &str, path: &Path, per_sound_volume: f32, looped: bool) {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("cannot open sound {}: {e}", path.display());
                return;
            }
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let source = match Decoder::builder()
            .with_data(BufReader::new(file))
            .with_byte_len(len)
            .build()
        {
            Ok(s) => s,
            Err(e) => {
                log::warn!("cannot decode {}: {e}", path.display());
                return;
            }
        };

        let device = self.inner.lock().unwrap().output_device.clone();
        let (player, stream) = match Self::open_player(device.as_deref()) {
            Some(pair) => pair,
            None => {
                log::error!("no output stream available");
                return;
            }
        };

        let (mv, prev_duration) = {
            let inner = self.inner.lock().unwrap();
            let prev = inner.active.get(id).map(|a| a.duration_secs).unwrap_or(0.0);
            (inner.master_volume, prev)
        };
        player.set_volume((mv * per_sound_volume).clamp(0.0, 1.0));
        player.append(source);

        let duration = if prev_duration > 0.0 {
            prev_duration
        } else {
            probe_duration(path)
        };

        self.inner.lock().unwrap().active.insert(
            id.to_string(),
            ActiveSound {
                player,
                _stream: stream,
                path: path.to_path_buf(),
                volume: per_sound_volume,
                looped,
                started: Instant::now(),
                duration_secs: duration,
            },
        );
    }

    pub fn stop(&self, sound_id: &str) {
        if let Some(a) = self.inner.lock().unwrap().active.remove(sound_id) {
            a.player.stop();
        }
    }

    pub fn stop_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        for (_, a) in inner.active.drain() {
            a.player.stop();
        }
    }

    pub fn state_of(&self, sound_id: &str) -> PlaybackState {
        let inner = self.inner.lock().unwrap();
        match inner.active.get(sound_id) {
            Some(a) if !a.player.empty() => {
                if a.player.is_paused() {
                    PlaybackState::Paused
                } else {
                    PlaybackState::Playing
                }
            }
            _ => PlaybackState::Stopped,
        }
    }

    pub fn is_playing(&self, sound_id: &str) -> bool {
        self.state_of(sound_id) == PlaybackState::Playing
    }

    /// Progress 0..=1 for an active sound.
    pub fn progress(&self, sound_id: &str) -> f32 {
        let inner = self.inner.lock().unwrap();
        match inner.active.get(sound_id) {
            Some(a) if a.duration_secs > 0.0 => {
                (a.started.elapsed().as_secs_f32() / a.duration_secs).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    pub fn elapsed_secs(&self, sound_id: &str) -> f32 {
        let inner = self.inner.lock().unwrap();
        inner
            .active
            .get(sound_id)
            .map(|a| a.started.elapsed().as_secs_f32())
            .unwrap_or(0.0)
    }

    pub fn duration_secs(&self, sound_id: &str) -> f32 {
        let inner = self.inner.lock().unwrap();
        inner
            .active
            .get(sound_id)
            .map(|a| a.duration_secs)
            .unwrap_or(0.0)
    }

    /// Called periodically from the UI loop: restarts looped sounds that finished.
    pub fn tick(&self) {
        let restarts: Vec<(String, PathBuf, f32)> = {
            let mut inner = self.inner.lock().unwrap();
            let done: Vec<String> = inner
                .active
                .iter()
                .filter(|(_, a)| a.looped && a.player.empty())
                .map(|(id, _)| id.clone())
                .collect();
            done.iter()
                .filter_map(|id| {
                    inner
                        .active
                        .remove(id)
                        .map(|a| (id.clone(), a.path.clone(), a.volume))
                })
                .collect()
        };
        for (id, path, vol) in restarts {
            self.play_raw(&id, &path, vol, true);
        }
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
