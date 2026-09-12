//! Global hotkeys — passive keyboard polling via `device_query` (`GetAsyncKeyState`).
//!
//! Unlike `RegisterHotKey` (used by the `global-hotkey` crate), which *swallows* the
//! registered key system-wide, polling observes keys without consuming them: the bound
//! key still reaches other apps (e.g. pressing W still moves you forward in a game).
//!
//! Key press detection is edge-triggered: a binding fires exactly once per physical
//! press, not repeatedly while the key is held.

use anyhow::Context as _;
use device_query::{DeviceQuery, DeviceState, Keycode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A parsed binding: optional modifiers + one non-modifier key.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParsedHotkey {
    ctrl: bool,
    shift: bool,
    alt: bool,
    key: Keycode,
}

/// Parse "Ctrl+Shift+1" / "F5" / "Alt+K" into a `ParsedHotkey`.
pub fn parse_hotkey(s: &str) -> Option<ParsedHotkey> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut key = None;

    for part in s.split('+') {
        let part = part.trim();
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "meta" | "super" | "win" | "cmd" => ctrl = true, // treat Meta as Ctrl (no win-key polling)
            _ => key = Some(parse_code(part)?),
        }
    }

    Some(ParsedHotkey { ctrl, shift, alt, key: key? })
}

fn parse_code(part: &str) -> Option<Keycode> {
    let p = part.to_ascii_uppercase();
    // F-keys
    if let Some(rest) = p.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=20).contains(&n) {
                return Some(match n {
                    1 => Keycode::F1,
                    2 => Keycode::F2,
                    3 => Keycode::F3,
                    4 => Keycode::F4,
                    5 => Keycode::F5,
                    6 => Keycode::F6,
                    7 => Keycode::F7,
                    8 => Keycode::F8,
                    9 => Keycode::F9,
                    10 => Keycode::F10,
                    11 => Keycode::F11,
                    12 => Keycode::F12,
                    13 => Keycode::F13,
                    14 => Keycode::F14,
                    15 => Keycode::F15,
                    16 => Keycode::F16,
                    17 => Keycode::F17,
                    18 => Keycode::F18,
                    19 => Keycode::F19,
                    _ => Keycode::F20,
                });
            }
        }
        return None;
    }

    match p.as_str() {
        "SPACE" => Some(Keycode::Space),
        "TAB" => Some(Keycode::Tab),
        "ENTER" | "RETURN" => Some(Keycode::Enter),
        "BACKSPACE" => Some(Keycode::Backspace),
        "ESC" | "ESCAPE" => Some(Keycode::Escape),
        "DELETE" | "DEL" => Some(Keycode::Delete),
        "INSERT" | "INS" => Some(Keycode::Insert),
        "HOME" => Some(Keycode::Home),
        "END" => Some(Keycode::End),
        "PAGEUP" => Some(Keycode::PageUp),
        "PAGEDOWN" => Some(Keycode::PageDown),
        "UP" | "ARROWUP" => Some(Keycode::Up),
        "DOWN" | "ARROWDOWN" => Some(Keycode::Down),
        "LEFT" | "ARROWLEFT" => Some(Keycode::Left),
        "RIGHT" | "ARROWRIGHT" => Some(Keycode::Right),
        "CAPSLOCK" => Some(Keycode::CapsLock),
        _ => {
            // Single letters A-Z
            if p.len() == 1 && p.as_bytes()[0].is_ascii_uppercase() {
                return Some(match p.as_bytes()[0] {
                    b'A' => Keycode::A,
                    b'B' => Keycode::B,
                    b'C' => Keycode::C,
                    b'D' => Keycode::D,
                    b'E' => Keycode::E,
                    b'F' => Keycode::F,
                    b'G' => Keycode::G,
                    b'H' => Keycode::H,
                    b'I' => Keycode::I,
                    b'J' => Keycode::J,
                    b'K' => Keycode::K,
                    b'L' => Keycode::L,
                    b'M' => Keycode::M,
                    b'N' => Keycode::N,
                    b'O' => Keycode::O,
                    b'P' => Keycode::P,
                    b'Q' => Keycode::Q,
                    b'R' => Keycode::R,
                    b'S' => Keycode::S,
                    b'T' => Keycode::T,
                    b'U' => Keycode::U,
                    b'V' => Keycode::V,
                    b'W' => Keycode::W,
                    b'X' => Keycode::X,
                    b'Y' => Keycode::Y,
                    b'Z' => Keycode::Z,
                    _ => unreachable!(),
                });
            }
            // Digits 0-9 (top row)
            if p.len() == 1 && p.as_bytes()[0].is_ascii_digit() {
                return Some(match p.as_bytes()[0] {
                    b'0' => Keycode::Key0,
                    b'1' => Keycode::Key1,
                    b'2' => Keycode::Key2,
                    b'3' => Keycode::Key3,
                    b'4' => Keycode::Key4,
                    b'5' => Keycode::Key5,
                    b'6' => Keycode::Key6,
                    b'7' => Keycode::Key7,
                    b'8' => Keycode::Key8,
                    b'9' => Keycode::Key9,
                    _ => unreachable!(),
                });
            }
            // Numpad 0-9
            if let Some(rest) = p.strip_prefix("NUM") {
                if let Ok(n) = rest.parse::<u8>() {
                    if n <= 9 {
                        return Some(match n {
                            0 => Keycode::Numpad0,
                            1 => Keycode::Numpad1,
                            2 => Keycode::Numpad2,
                            3 => Keycode::Numpad3,
                            4 => Keycode::Numpad4,
                            5 => Keycode::Numpad5,
                            6 => Keycode::Numpad6,
                            7 => Keycode::Numpad7,
                            8 => Keycode::Numpad8,
                            _ => Keycode::Numpad9,
                        });
                    }
                }
            }
            None
        }
    }
}

/// True if every modifier of `hk` is currently held. `device_query` reports left/right
/// variants, so check both sides.
fn modifiers_held(keys: &[Keycode], hk: &ParsedHotkey) -> bool {
    let ctrl = keys.contains(&Keycode::LControl) || keys.contains(&Keycode::RControl);
    let shift = keys.contains(&Keycode::LShift) || keys.contains(&Keycode::RShift);
    let alt = keys.contains(&Keycode::LAlt) || keys.contains(&Keycode::RAlt);
    (hk.ctrl == ctrl || (hk.ctrl && ctrl))
        && (hk.shift == shift || (hk.shift && shift))
        && (hk.alt == alt || (hk.alt && alt))
}

/// Display form of a hotkey (canonical "Ctrl+Shift+X" style).
#[cfg(test)]
pub fn hotkey_to_string(s: &str) -> String {
    s.to_string()
}

/// Manages global hotkey bindings: hotkey string -> sound id.
///
/// A background thread polls the keyboard ~4x per audio frame tick (the UI thread
/// calls [`poll_triggered`] each frame; the heavy `get_keys` call runs here). Edge
/// detection means each physical press fires the binding exactly once.
pub struct HotkeyManager {
    state: DeviceState,
    /// Parsed binding -> sound id.
    bindings: Mutex<HashMap<ParsedHotkey, String>>,
    /// Sound id -> hotkey string (for display / unbind).
    display: Mutex<HashMap<String, String>>,
    /// Bindings that fired while the key was down (edge latch).
    fired: Mutex<HashMap<ParsedHotkey, ()>>,
    /// Bindings that were down in the previous poll (for edge detection).
    prev_down: Mutex<HashMap<ParsedHotkey, bool>>,
    /// Set if the keyboard cannot be queried at all.
    available: Arc<AtomicBool>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        let state = DeviceState::new();
        Self {
            state,
            bindings: Mutex::new(HashMap::new()),
            display: Mutex::new(HashMap::new()),
            fired: Mutex::new(HashMap::new()),
            prev_down: Mutex::new(HashMap::new()),
            available: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }

    /// Bind a hotkey string to a sound id. Rebinds safely if the key changed.
    pub fn bind(&self, hotkey_str: &str, sound_id: &str) -> anyhow::Result<()> {
        let hk = parse_hotkey(hotkey_str).context("unrecognized key")?;

        let mut bindings = self.bindings.lock().unwrap();

        // If this sound had a previous hotkey, remove it.
        let prev = self.display.lock().unwrap().remove(sound_id);
        if let Some(prev_str) = prev {
            if let Some(prev_hk) = parse_hotkey(&prev_str) {
                bindings.remove(&prev_hk);
                self.fired.lock().unwrap().remove(&prev_hk);
                self.prev_down.lock().unwrap().remove(&prev_hk);
            }
        }

        // Reject duplicates (same physical binding bound to another sound).
        if bindings.get(&hk).is_some_and(|id| id != sound_id) {
            anyhow::bail!("hotkey already assigned");
        }

        bindings.insert(hk, sound_id.to_string());
        self.display
            .lock()
            .unwrap()
            .insert(sound_id.to_string(), hotkey_str.to_string());
        Ok(())
    }

    pub fn unbind(&self, sound_id: &str) {
        if let Some(prev_str) = self.display.lock().unwrap().remove(sound_id) {
            if let Some(hk) = parse_hotkey(&prev_str) {
                self.bindings.lock().unwrap().remove(&hk);
                self.fired.lock().unwrap().remove(&hk);
                self.prev_down.lock().unwrap().remove(&hk);
            }
        }
    }

    pub fn unbind_all(&self) {
        self.bindings.lock().unwrap().clear();
        self.display.lock().unwrap().clear();
        self.fired.lock().unwrap().clear();
        self.prev_down.lock().unwrap().clear();
    }

    /// Poll the keyboard and return sound ids whose binding was just pressed
    /// (edge-triggered: fires once per physical press, even if the key is held).
    ///
    /// This is *passive* — keys are observed, not consumed, so bound keys still
    /// reach other applications (games, Discord, ...).
    pub fn poll_triggered(&self) -> Vec<String> {
        let mut out = Vec::new();
        let keys = self.state.get_keys();
        let bindings = self.bindings.lock().unwrap();
        let mut prev_down = self.prev_down.lock().unwrap();

        for (hk, id) in bindings.iter() {
            // The binding's own key must be down; modifier keys are excluded as
            // the main key of a binding (they can still be held as modifiers).
            let is_mod_key = matches!(
                hk.key,
                Keycode::LControl
                    | Keycode::RControl
                    | Keycode::LShift
                    | Keycode::RShift
                    | Keycode::LAlt
                    | Keycode::RAlt
            );
            let down = !is_mod_key && keys.contains(&hk.key) && modifiers_held(&keys, hk);
            let was_down = prev_down.insert(*hk, down).unwrap_or(false);

            // Rising edge: down now, wasn't down last poll.
            if down && !was_down {
                out.push(id.clone());
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
        assert_eq!(hk.key, Keycode::F5);
        assert!(!hk.ctrl && !hk.shift && !hk.alt);
    }

    #[test]
    fn parses_modifiers() {
        let hk = parse_hotkey("Ctrl+Shift+1").unwrap();
        assert!(hk.ctrl);
        assert!(hk.shift);
        assert_eq!(hk.key, Keycode::Key1);
    }

    #[test]
    fn parses_letters() {
        let hk = parse_hotkey("Alt+K").unwrap();
        assert!(hk.alt);
        assert_eq!(hk.key, Keycode::K);
    }

    #[test]
    fn rejects_unknown() {
        assert!(parse_hotkey("NotAKey").is_none());
    }

    #[test]
    fn roundtrip_display() {
        assert_eq!(hotkey_to_string("Ctrl+F2"), "Ctrl+F2");
    }
}
