//! Global hotkeys — Using the `global-hotkey` crate. 
//! Hotkeys fire even when the app is unfocused.

use anyhow::Context as _;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::collections::HashMap;
use std::sync::Mutex;

/// Parse "Ctrl+Shift+1" / "F5" / "Alt+K" into a `HotKey`.
pub fn parse_hotkey(s: &str) -> Option<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code = None;

    for part in s.split('+') {
        let part = part.trim();
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            "alt" => mods |= Modifiers::ALT,
            "meta" | "super" | "win" | "cmd" => mods |= Modifiers::META,
            _ => code = Some(parse_code(part)?),
        }
    }

    Some(HotKey::new(Some(mods), code?))
}

fn parse_code(part: &str) -> Option<Code> {
    let p = part.to_ascii_uppercase();
    // F-keys
    if let Some(rest) = p.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(match n {
                    1 => Code::F1,
                    2 => Code::F2,
                    3 => Code::F3,
                    4 => Code::F4,
                    5 => Code::F5,
                    6 => Code::F6,
                    7 => Code::F7,
                    8 => Code::F8,
                    9 => Code::F9,
                    10 => Code::F10,
                    11 => Code::F11,
                    12 => Code::F12,
                    13 => Code::F13,
                    14 => Code::F14,
                    15 => Code::F15,
                    16 => Code::F16,
                    17 => Code::F17,
                    18 => Code::F18,
                    19 => Code::F19,
                    20 => Code::F20,
                    _ => return None,
                });
            }
        }
        return None;
    }

    let code = match p.as_str() {
        "SPACE" => Code::Space,
        "TAB" => Code::Tab,
        "ENTER" | "RETURN" => Code::Enter,
        "BACKSPACE" => Code::Backspace,
        "ESC" | "ESCAPE" => Code::Escape,
        "DELETE" | "DEL" => Code::Delete,
        "INSERT" | "INS" => Code::Insert,
        "HOME" => Code::Home,
        "END" => Code::End,
        "PAGEUP" => Code::PageUp,
        "PAGEDOWN" => Code::PageDown,
        "UP" | "ARROWUP" => Code::ArrowUp,
        "DOWN" | "ARROWDOWN" => Code::ArrowDown,
        "LEFT" | "ARROWLEFT" => Code::ArrowLeft,
        "RIGHT" | "ARROWRIGHT" => Code::ArrowRight,
        "CAPSLOCK" => Code::CapsLock,
        "NUMLOCK" => Code::NumLock,
        "PRINTSCREEN" => Code::PrintScreen,
        "SCROLLLOCK" => Code::ScrollLock,
        "PAUSE" => Code::Pause,
        _ => {
            // Single letters A-Z
            if p.len() == 1 && p.as_bytes()[0].is_ascii_uppercase() {
                return Some(match p.as_bytes()[0] {
                    b'A' => Code::KeyA,
                    b'B' => Code::KeyB,
                    b'C' => Code::KeyC,
                    b'D' => Code::KeyD,
                    b'E' => Code::KeyE,
                    b'F' => Code::KeyF,
                    b'G' => Code::KeyG,
                    b'H' => Code::KeyH,
                    b'I' => Code::KeyI,
                    b'J' => Code::KeyJ,
                    b'K' => Code::KeyK,
                    b'L' => Code::KeyL,
                    b'M' => Code::KeyM,
                    b'N' => Code::KeyN,
                    b'O' => Code::KeyO,
                    b'P' => Code::KeyP,
                    b'Q' => Code::KeyQ,
                    b'R' => Code::KeyR,
                    b'S' => Code::KeyS,
                    b'T' => Code::KeyT,
                    b'U' => Code::KeyU,
                    b'V' => Code::KeyV,
                    b'W' => Code::KeyW,
                    b'X' => Code::KeyX,
                    b'Y' => Code::KeyY,
                    b'Z' => Code::KeyZ,
                    _ => unreachable!(),
                });
            }
            // Digits 0-9 (top row)
            if p.len() == 1 && p.as_bytes()[0].is_ascii_digit() {
                return Some(match p.as_bytes()[0] {
                    b'0' => Code::Digit0,
                    b'1' => Code::Digit1,
                    b'2' => Code::Digit2,
                    b'3' => Code::Digit3,
                    b'4' => Code::Digit4,
                    b'5' => Code::Digit5,
                    b'6' => Code::Digit6,
                    b'7' => Code::Digit7,
                    b'8' => Code::Digit8,
                    b'9' => Code::Digit9,
                    _ => unreachable!(),
                });
            }
            // Numpad 0-9
            if let Some(rest) = p.strip_prefix("NUM") {
                if let Ok(n) = rest.parse::<u8>() {
                    if n <= 9 {
                        return Some(match n {
                            0 => Code::Numpad0,
                            1 => Code::Numpad1,
                            2 => Code::Numpad2,
                            3 => Code::Numpad3,
                            4 => Code::Numpad4,
                            5 => Code::Numpad5,
                            6 => Code::Numpad6,
                            7 => Code::Numpad7,
                            8 => Code::Numpad8,
                            _ => Code::Numpad9,
                        });
                    }
                }
            }
            return None;
        }
    };
    Some(code)
}

/// Display form of a `HotKey` (canonical "Ctrl+Shift+X" style).
#[cfg(test)]
pub fn hotkey_to_string(hk: &HotKey) -> String {
    let mut parts: Vec<String> = Vec::new();
    if hk.mods.contains(Modifiers::CONTROL) {
        parts.push("Ctrl".into());
    }
    if hk.mods.contains(Modifiers::SHIFT) {
        parts.push("Shift".into());
    }
    if hk.mods.contains(Modifiers::ALT) {
        parts.push("Alt".into());
    }
    if hk.mods.contains(Modifiers::META) {
        parts.push("Meta".into());
    }
    parts.push(code_to_string(hk.key));
    parts.join("+")
}

#[cfg(test)]
fn code_to_string(code: Code) -> String {
    format!("{code:?}")
        .trim_start_matches("Key")
        .to_string()
}

/// Manages global hotkey bindings: hotkey string -> sound id.
pub struct HotkeyManager {
    manager: Option<GlobalHotKeyManager>,
    /// Registered HotKey id -> sound id.
    bindings: Mutex<HashMap<u32, String>>,
    /// Sound id -> hotkey string (for display).
    display: Mutex<HashMap<String, String>>,
    /// None while iterating inside `poll_triggered` (see re-entrancy note there).
    events: Option<global_hotkey::GlobalHotKeyEventReceiver>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        // The manager must live on a thread with a win32 event loop; on Windows
        // eframe runs its event loop on the main thread, which is where we are.
        let (manager, events) = match GlobalHotKeyManager::new() {
            Ok(m) => (Some(m), Some(GlobalHotKeyEvent::receiver().clone())),
            Err(e) => {
                log::warn!("global hotkeys unavailable: {e}");
                (None, None)
            }
        };
        Self {
            manager,
            bindings: Mutex::new(HashMap::new()),
            display: Mutex::new(HashMap::new()),
            events,
        }
    }

    pub fn is_available(&self) -> bool {
        self.manager.is_some()
    }

    /// Bind a hotkey string to a sound id. Rebinds safely if the key changed.
    pub fn bind(&self, hotkey_str: &str, sound_id: &str) -> anyhow::Result<()> {
        let Some(mgr) = &self.manager else {
            anyhow::bail!("global hotkeys unavailable on this system");
        };
        let hk = parse_hotkey(hotkey_str).context("unrecognized key")?;

        let mut bindings = self.bindings.lock().unwrap();

        // If this sound had a previous hotkey, unregister it.
        let prev = self.display.lock().unwrap().remove(sound_id);
        if let Some(prev_str) = prev {
            if let Some(prev_hk) = parse_hotkey(&prev_str) {
                let _ = mgr.unregister(prev_hk);
                bindings.remove(&prev_hk.id());
            }
        }

        // Reject duplicates.
        if bindings.contains_key(&hk.id()) {
            anyhow::bail!("hotkey already assigned");
        }

        mgr.register(hk)?;
        bindings.insert(hk.id(), sound_id.to_string());
        self.display
            .lock()
            .unwrap()
            .insert(sound_id.to_string(), hotkey_str.to_string());
        Ok(())
    }

    pub fn unbind(&self, sound_id: &str) {
        let Some(mgr) = &self.manager else { return };
        if let Some(prev_str) = self.display.lock().unwrap().remove(sound_id) {
            if let Some(hk) = parse_hotkey(&prev_str) {
                let _ = mgr.unregister(hk);
                self.bindings.lock().unwrap().remove(&hk.id());
            }
        }
    }

    pub fn unbind_all(&self) {
        let strs: Vec<String> = self.display.lock().unwrap().values().cloned().collect();
        if let Some(mgr) = &self.manager {
            for s in &strs {
                if let Some(hk) = parse_hotkey(s) {
                    let _ = mgr.unregister(hk);
                }
            }
        }
        self.bindings.lock().unwrap().clear();
        self.display.lock().unwrap().clear();
    }

    /// Drain triggered hotkey events; returns sound ids to play.
    pub fn poll_triggered(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(rx) = &self.events {
            loop {
                match rx.try_recv() {
                    Ok(ev) => {
                        if ev.state == global_hotkey::HotKeyState::Pressed {
                            if let Some(id) = self.bindings.lock().unwrap().get(&ev.id) {
                                out.push(id.clone());
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fkeys() {
        let hk = parse_hotkey("F5").unwrap();
        assert_eq!(hk.key, Code::F5);
    }

    #[test]
    fn parses_modifiers() {
        let hk = parse_hotkey("Ctrl+Shift+1").unwrap();
        assert!(hk.mods.contains(Modifiers::CONTROL));
        assert!(hk.mods.contains(Modifiers::SHIFT));
        assert_eq!(hk.key, Code::Digit1);
    }

    #[test]
    fn parses_letters() {
        let hk = parse_hotkey("Alt+K").unwrap();
        assert_eq!(hk.key, Code::KeyK);
    }

    #[test]
    fn rejects_unknown() {
        assert!(parse_hotkey("NotAKey").is_none());
    }

    #[test]
    fn roundtrip_display() {
        assert_eq!(hotkey_to_string(&parse_hotkey("Ctrl+F2").unwrap()), "Ctrl+F2");
    }
}
