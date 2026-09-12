//! Soundpad — Rust Desktop soundboard.
//! Entry point and the full egui application: sidebar with categories,
//! sound grid, search, settings/edit dialogs, status bar and global hotkeys.

mod audio;
mod hotkeys;
mod models;
mod store;

use audio::AudioPlayer;
use eframe::egui;
use egui::{Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Frame, Layout, Margin, RichText, Sense, Shadow, Stroke, StrokeKind, Ui, Vec2, pos2};
use hotkeys::HotkeyManager;
use models::{SoundCategory, SoundFile};
use std::path::PathBuf;
use std::sync::Arc;
use store::Store;

// ── Theme ───────────────────────────────────────────────────────────────────

const ACCENT: Color32 = Color32::from_rgb(0x00, 0xD4, 0xFF);
const ERROR_RED: Color32 = Color32::from_rgb(0xE5, 0x39, 0x35);

struct Theme {
    bg: Color32,
    surface: Color32,
    card: Color32,
    border: Color32,
    text: Color32,
    text_dim: Color32,
}

const DARK: Theme = Theme {
    bg: Color32::from_rgb(0x1A, 0x1A, 0x2E),
    surface: Color32::from_rgb(0x16, 0x21, 0x3E),
    card: Color32::from_rgb(0x0F, 0x34, 0x60),
    border: Color32::from_rgb(0x1E, 0x42, 0x78),
    text: Color32::from_rgb(0xCF, 0xBC, 0xFF),
    text_dim: Color32::from_rgb(0xCC, 0xC2, 0xDC),
};

const LIGHT: Theme = Theme {
    bg: Color32::from_rgb(0xF5, 0xF5, 0xF5),
    surface: Color32::WHITE,
    card: Color32::WHITE,
    border: Color32::from_rgb(0xE0, 0xE0, 0xE0),
    text: Color32::from_rgb(0x62, 0x5B, 0x71),
    text_dim: Color32::from_rgb(0x9E, 0x9E, 0x9E),
};

fn theme(dark: bool) -> Theme {
    if dark { DARK } else { LIGHT }
}

fn apply_visuals(ctx: &Context, dark: bool) {
    let mut vis = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    vis.panel_fill = if dark { DARK.bg } else { LIGHT.bg };
    vis.window_fill = if dark { DARK.surface } else { LIGHT.surface };
    vis.extreme_bg_color = if dark { DARK.card } else { LIGHT.card };
    vis.selection.stroke = Stroke::new(1.5, ACCENT);
    vis.selection.bg_fill = ACCENT.gamma_multiply(0.25);
    // Hand cursor over anything interactive.
    vis.interact_cursor = Some(CursorIcon::PointingHand);
    // Softer, rounder windows & popups.
    vis.window_corner_radius = CornerRadius::same(16);
    vis.menu_corner_radius = CornerRadius::same(12);
    vis.window_shadow = Shadow {
        offset: [0, 10],
        blur: 30,
        spread: 2,
        color: Color32::from_black_alpha(110),
    };
    vis.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };
    // Rounder widgets with accent hover strokes and pressed feedback.
    for w in [
        &mut vis.widgets.noninteractive,
        &mut vis.widgets.inactive,
        &mut vis.widgets.hovered,
        &mut vis.widgets.active,
        &mut vis.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(8);
    }
    vis.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.55));
    vis.widgets.active.bg_fill = ACCENT.gamma_multiply(0.30);
    vis.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    vis.slider_trailing_fill = true;
    ctx.set_visuals(vis);
}

// ── App state ───────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum Dialog {
    None,
    Settings,
    EditSound(String),
    DeleteSound(String),
    DeleteCategory(String),
    NewCategory,
    Presets,
}

struct SoundpadApp {
    store: Store,
    player: Arc<AudioPlayer>,
    hotkeys: Arc<HotkeyManager>,

    selected_category: Option<String>,
    search: String,

    dialog: Dialog,
    /// Scratch state for the edit dialog.
    edit_volume: f32,
    edit_hotkey: Option<String>,
    edit_hotkey_enabled: bool,
    /// Scratch state for the new-category dialog.
    new_cat_name: String,
    new_cat_icon: String,
    /// Scratch state for the presets dialog.
    preset_name: String,

    master_volume: f32,
    status_msg: Option<(String, std::time::Instant)>,
}

impl SoundpadApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let store = Store::init().expect("failed to initialize store");
        let settings = store.settings.clone();
        apply_visuals(&cc.egui_ctx, settings.dark_theme);

        let player = Arc::new(AudioPlayer::new());
        player.set_master_volume(settings.master_volume);
        player.set_output_device(&settings.output_device);
        let hotkeys = Arc::new(HotkeyManager::new());

        // Restore saved hotkey bindings.
        if settings.hotkeys_enabled {
            let binds: Vec<(String, String)> = store
                .sounds
                .iter()
                .filter(|s| s.hotkey.is_some() && s.hotkey_enabled)
                .map(|s| (s.hotkey.clone().unwrap(), s.id.clone()))
                .collect();
            for (hk, id) in binds {
                if let Err(e) = hotkeys.bind(&hk, &id) {
                    log::warn!("hotkey {hk}: {e}");
                }
            }
        }

        Self {
            store,
            player,
            hotkeys,
            selected_category: None,
            search: String::new(),
            dialog: Dialog::None,
            edit_volume: 1.0,
            edit_hotkey: None,
            edit_hotkey_enabled: false,
            new_cat_name: String::new(),
            new_cat_icon: "folder".to_string(),
            preset_name: String::new(),
            master_volume: settings.master_volume,
            status_msg: None,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), std::time::Instant::now()));
    }

    fn filtered_sounds(&self) -> Vec<SoundFile> {
        let q = self.search.to_lowercase();
        self.store
            .sounds
            .iter()
            .filter(|s| {
                let cat_ok = match &self.selected_category {
                    None => true,
                    Some(id) => &s.category_id == id,
                };
                let search_ok = q.is_empty()
                    || s.name.to_lowercase().contains(&q)
                    || s.file_path.to_string_lossy().to_lowercase().contains(&q);
                cat_ok && search_ok
            })
            .cloned()
            .collect()
    }

    /// Add files: copy to portable dir, probe duration, store.
    fn add_files(&mut self, paths: Vec<PathBuf>) {
        let category = self
            .selected_category
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let mut added = 0;
        for p in paths {
            if !p.exists() {
                continue;
            }
            match self.store.copy_sound_portable(&p) {
                Ok(dest) => {
                    let name = dest
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "sound".into());
                    let mut sf = SoundFile::new(name, dest, &category);
                    sf.duration = audio::probe_duration(&sf.file_path);
                    if self.store.add_sound(sf).is_ok() {
                        added += 1;
                    }
                }
                Err(e) => log::error!("add file: {e}"),
            }
        }
        if added > 0 {
            self.set_status(format!("Added {added} sound(s)"));
        }
    }

    /// (Re-)register every saved hotkey binding.
    fn rebind_all_hotkeys(&mut self) {
        self.hotkeys.unbind_all();
        if !self.store.settings.hotkeys_enabled {
            return;
        }
        let binds: Vec<(String, String)> = self
            .store
            .sounds
            .iter()
            .filter(|s| s.hotkey.is_some() && s.hotkey_enabled)
            .map(|s| (s.hotkey.clone().unwrap(), s.id.clone()))
            .collect();
        for (hk, id) in binds {
            if let Err(e) = self.hotkeys.bind(&hk, &id) {
                log::warn!("hotkey {hk}: {e}");
            }
        }
    }
}

impl eframe::App for SoundpadApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Global hotkeys → play sounds.
        if self.store.settings.hotkeys_enabled {
            if !self.hotkeys.is_available() {
                self.set_status("Global hotkeys unavailable on this system");
            }
            for id in self.hotkeys.poll_triggered() {
                if let Some(sound) = self.store.get_sound(&id).cloned() {
                    self.player.play(&sound, sound.volume);
                }
            }
        }
        // Loop restarts.
        self.player.tick();

        let th = theme(self.store.settings.dark_theme);

        // App-level drag-and-drop: the whole window is a drop target for audio files.
        // The frame is painted as a drop zone so hovering files over the title bar / empty
        // area highlights the app and shows the "drop" overlay inside the real target rect.
        let (_inner_resp, dropped): (_, Option<std::sync::Arc<egui::HoveredFile>>) = ui.dnd_drop_zone(eframe::egui::Frame::new(), |ui| {
            self.draw_top_bar(ui, &th);
            self.draw_sidebar(ui, &th);
            self.draw_status_bar(ui, &th);
            self.draw_grid(ui, &th);
            self.draw_dialogs(ui.ctx(), &th);
        });

        // When dragging files and the pointer is outside any egui widget, paint the
        // pulsing overlay across the whole viewport so the app clearly signals
        // "drop here".
        let dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if dragging && !_inner_resp.response.contains_pointer() {
            draw_drop_overlay(&ctx);
        }

        if let Some(file) = dropped {
            // `dnd_drop_zone` gives us the released file directly; if it came with a path,
            // add it. Otherwise (e.g. a file dropped from a non-path source) we ignore it.
            if let Some(path) = &file.path {
                self.add_files(vec![path.clone()]);
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.player.stop_all();
        let _ = self.store.save_sounds();
        let _ = self.store.save_categories();
        self.store.settings.master_volume = self.master_volume;
        let _ = self.store.save_settings();
    }
}

// ── Top bar ─────────────────────────────────────────────────────────────────

impl SoundpadApp {
    fn draw_top_bar(&mut self, ui: &mut Ui, th: &Theme) {
        egui::Panel::top("top_bar")
            .frame(Frame::new().fill(th.surface).inner_margin(Margin::symmetric(12, 8)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("♪ Soundpad")
                            .size(20.0)
                            .strong()
                            .color(if self.store.settings.dark_theme { ACCENT } else { th.text }),
                    );
                    ui.add_space(12.0);

                    if ui
                        .add(
                            egui::Button::new(RichText::new("＋ Add").size(14.0))
                                .fill(ACCENT.gamma_multiply(0.18))
                                .stroke(Stroke::new(1.0, ACCENT)),
                        )
                        .clicked()
                    {
                        let paths = rfd::FileDialog::new()
                            .add_filter("Audio", &["mp3", "wav", "ogg", "flac", "m4a", "aac", "wma"])
                            .add_filter("All files", &["*"])
                            .pick_files();
                        if let Some(paths) = paths {
                            self.add_files(paths);
                        }
                    }

                    ui.add_space(8.0);
                    ui.add_sized(
                        [220.0, 26.0],
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("🔍 Search sounds...")
                            .font(FontId::proportional(13.0)),
                    );

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("⚙").size(16.0)).on_hover_text("Settings").clicked() {
                            self.dialog = Dialog::Settings;
                        }
                        let dark = self.store.settings.dark_theme;
                        if ui.button(RichText::new(if dark { "☀" } else { "🌙" }).size(16.0))
                            .on_hover_text("Toggle theme")
                            .clicked()
                        {
                            self.store.settings.dark_theme = !dark;
                            let _ = self.store.save_settings();
                            apply_visuals(ui.ctx(), self.store.settings.dark_theme);
                        }
                        if ui.button(RichText::new("☰").size(16.0)).on_hover_text("Presets").clicked() {
                            self.dialog = Dialog::Presets;
                        }
                    });
                });
            });
    }

    // ── Sidebar ─────────────────────────────────────────────────────────────

    fn draw_sidebar(&mut self, ui: &mut Ui, th: &Theme) {
        egui::Panel::left("sidebar")
            .resizable(false)
            .default_size(220.0)
            .frame(Frame::new().fill(th.surface).inner_margin(Margin::same(10)))
            .show(ui, |ui| {
                ui.label(RichText::new("CATEGORIES").size(11.0).strong().color(th.text_dim));
                ui.add_space(6.0);

                // "All Sounds".
                let total = self.store.sounds.len();
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
                let hover_t = ui
                    .ctx()
                    .animate_bool_with_time(ui.id().with("cat_row_all"), resp.hovered(), 0.10);
                paint_category_row(
                    ui,
                    th,
                    rect,
                    "All Sounds",
                    "📁",
                    total,
                    self.selected_category.is_none(),
                    false,
                    hover_t,
                );
                if resp.clicked() {
                    self.selected_category = None;
                }

                ui.add_space(4.0);

                let cats = self.store.categories.clone();
                for cat in cats.iter().filter(|c| c.id != "default") {
                    let count = self
                        .store
                        .sounds
                        .iter()
                        .filter(|s| s.category_id == cat.id)
                        .count();
                    let selected = self.selected_category.as_deref() == Some(cat.id.as_str());
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
                    let hover_t = ui.ctx().animate_bool_with_time(
                        ui.id().with("cat_row").with(cat.id.as_str()),
                        resp.hovered(),
                        0.10,
                    );
                    paint_category_row(
                        ui,
                        th,
                        rect,
                        &cat.name,
                        builtin_icon(&cat.icon),
                        count,
                        selected,
                        true,
                        hover_t,
                    );
                    if resp.clicked() {
                        self.selected_category = Some(cat.id.clone());
                    }
                    if resp.secondary_clicked() {
                        self.dialog = Dialog::DeleteCategory(cat.id.clone());
                    }
                }

                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("＋ Add Category").size(13.0))
                            .min_size(Vec2::new(ui.available_width(), 30.0))
                            .fill(ACCENT.gamma_multiply(0.15))
                            .stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.4))),
                    )
                    .clicked()
                {
                    self.dialog = Dialog::NewCategory;
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("♪ {} sounds", self.store.sounds.len()))
                        .size(11.0)
                        .color(th.text_dim),
                );
                ui.add_space(4.0);
                if ui.small_button(RichText::new("Presets").size(11.0)).clicked() {
                    self.dialog = Dialog::Presets;
                }
            });
    }

    // ── Sound grid ──────────────────────────────────────────────────────────

    fn draw_grid(&mut self, ui: &mut Ui, th: &Theme) {
        egui::CentralPanel::default()
            .frame(Frame::new().fill(th.bg).inner_margin(Margin::same(14)))
            .show(ui, |ui| {
                let sounds = self.filtered_sounds();
                if sounds.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        // Gently floating note.
                        let t = ui.input(|i| i.time) as f32;
                        let (r, _) = ui.allocate_exact_size(Vec2::new(80.0, 64.0), Sense::hover());
                        let y = (t * 1.4).sin() * 6.0;
                        let alpha = 0.45 + 0.20 * (t * 1.9).sin();
                        ui.painter().text(
                            r.center() + Vec2::new(0.0, y - 6.0),
                            Align2::CENTER_CENTER,
                            "♪",
                            FontId::proportional(48.0),
                            ACCENT.gamma_multiply(alpha),
                        );
                        ui.label(RichText::new("No sounds yet").size(20.0).color(th.text_dim));
                        ui.label(
                            RichText::new("Drop audio files onto the window\nor use the ＋ Add button")
                                .size(14.0)
                                .color(th.text_dim),
                        );
                    });
                    return;
                }

                let width = ui.available_width();
                let min_card = 140.0_f32;
                let gap = 12.0_f32;
                let columns = ((width / (min_card + gap)).floor() as usize).max(1);
                egui::Grid::new("sound_grid")
                    .min_col_width(min_card)
                    .spacing([gap, gap])
                    .show(ui, |ui| {
                        let mut col = 0;
                        for sound in &sounds {
                            if col == columns {
                                ui.end_row();
                                col = 0;
                            }
                            self.sound_card(ui, th, sound);
                            col += 1;
                        }
                        if col > 0 {
                            ui.end_row();
                        }
                    });
            });
    }

    fn sound_card(&mut self, ui: &mut Ui, th: &Theme, sound: &SoundFile) {
        let playing = self.player.is_playing(&sound.id);

        // Reserve next-frame space so the hover lift doesn't reflow the grid.
        // The card rect is persisted in egui memory for a stable measurement.
        let pad = 14.0;
        let card_id = ui.id().with("card_rect").with(sound.id.as_str());
        let (space, resp) =
            ui.allocate_exact_size(Vec2::new(140.0 + 2.0 * pad, 148.0 + 2.0 * pad), Sense::hover());
        let base_rect: egui::Rect = ui
            .ctx()
            .memory(|m| m.data.get_temp(card_id))
            .filter(|r: &egui::Rect| r.is_positive() && r.width() >= 100.0)
            .unwrap_or_else(|| space.shrink2(Vec2::splat(pad)));

        // Animated states.
        let hover_t = ui
            .ctx()
            .animate_bool_with_time(
                ui.id().with("card_hover").with(sound.id.as_str()),
                resp.hovered(),
                0.12,
            )
            .clamp(0.0, 1.0);
        let press_t = ui
            .ctx()
            .animate_bool_with_time(
                ui.id().with("card_press").with(sound.id.as_str()),
                resp.is_pointer_button_down_on(),
                0.08,
            )
            .clamp(0.0, 1.0);
        let play_t = ui
            .ctx()
            .animate_bool_with_time(
                ui.id().with("card_play").with(sound.id.as_str()),
                playing,
                0.25,
            )
            .clamp(0.0, 1.0);

        // Hover lift + press dip.
        let lift = 6.0 * hover_t - 3.0 * press_t;
        let card_rect = base_rect.translate(Vec2::new(0.0, -lift));

        let painter = ui.painter_at(card_rect.expand(pad + 20.0));

        // Drop shadow grows with hover.
        if hover_t > 0.01 || play_t > 0.01 {
            let glow = ACCENT.gamma_multiply(0.35 * hover_t + 0.25 * play_t);
            painter.rect_filled(
                card_rect.expand(6.0 + 8.0 * hover_t),
                CornerRadius::same(18),
                glow,
            );
        }

        // Card body.
        let card_bg = th
            .card
            .lerp_to_gamma(ACCENT.gamma_multiply(0.18), 0.8 * play_t);
        painter.rect_filled(
            card_rect,
            CornerRadius::same(14),
            card_bg,
        );
        let border = th
            .border
            .lerp_to_gamma(ACCENT, (hover_t * 0.8 + play_t).clamp(0.0, 1.0));
        painter.rect_stroke(
            card_rect,
            CornerRadius::same(14),
            Stroke::new(1.0 + 1.0 * hover_t, border),
            StrokeKind::Inside,
        );

        // Contents live in a child UI clipped to the card.
        let mut card_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(ui.id().with("card").with(sound.id.as_str()))
                .max_rect(card_rect.shrink(10.0))
                .sense(Sense::hover()),
        );
        let ui = &mut card_ui;
            ui.vertical(|ui| {
                // Top row: delete / loop badge / edit.
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("✕").size(10.0).color(ERROR_RED)).small())
                        .on_hover_text("Delete")
                        .clicked()
                    {
                        self.dialog = Dialog::DeleteSound(sound.id.clone());
                    }
                    if sound.looped {
                        ui.label(RichText::new("🔁").size(10.0));
                    }
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("✎").size(10.0).color(ACCENT)).small())
                            .on_hover_text("Edit")
                            .clicked()
                        {
                            self.dialog = Dialog::EditSound(sound.id.clone());
                            self.edit_volume = sound.volume;
                            self.edit_hotkey = sound.hotkey.clone();
                            self.edit_hotkey_enabled = sound.hotkey.is_some();
                        }
                    });
                });

            ui.vertical_centered(|ui| {
                // Play button: allocate at fixed size, then paint with
                // hover-grow / press-shrink and a pulsing halo while playing.
                let (rect, pb) = ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                if pb.clicked() {
                    if playing {
                        self.player.stop(&sound.id);
                    } else {
                        self.player.play(sound, sound.volume);
                    }
                }
                let btn_hover = ui
                    .ctx()
                    .animate_bool_with_time(
                        ui.id().with("play_hover").with(sound.id.as_str()),
                        pb.hovered(),
                        0.10,
                    )
                    .clamp(0.0, 1.0);
                let btn_press = ui
                    .ctx()
                    .animate_bool_with_time(
                        ui.id().with("play_press").with(sound.id.as_str()),
                        pb.is_pointer_button_down_on(),
                        0.08,
                    )
                    .clamp(0.0, 1.0);
                let s = 1.0 + 0.08 * btn_hover - 0.10 * btn_press;
                let center = rect.center();
                let radius = 20.0 * s;
                let t = ui.input(|i| i.time) as f32;
                if playing {
                    let pulse = 0.5 + 0.5 * (t * 2.2).sin();
                    ui.painter().circle_filled(
                        center,
                        radius + 5.0 + 3.0 * pulse,
                        ACCENT.gamma_multiply(0.10 + 0.08 * pulse),
                    );
                }
                if btn_hover > 0.01 {
                    ui.painter().circle_filled(
                        center,
                        radius + 3.0,
                        ACCENT.gamma_multiply(0.25 * btn_hover),
                    );
                }
                ui.painter()
                    .circle_filled(center, radius, th.bg);
                ui.painter().text(
                    center + Vec2::new(1.0, 0.0),
                    Align2::CENTER_CENTER,
                    if playing { "⏸" } else { "▶" },
                    FontId::proportional(18.0),
                    if playing { ACCENT } else { th.text },
                );

                ui.add_space(2.0);
                ui.label(RichText::new(&sound.name).size(12.0).color(th.text).strong());

                let total = {
                    let d = self.player.duration_secs(&sound.id);
                    if d > 0.0 { d } else { sound.duration }
                };
                if playing && total > 0.0 {
                    // Time + animated equalizer bars.
                    let elapsed = self.player.elapsed_secs(&sound.id);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{} / {}", fmt_time(elapsed), fmt_time(total)))
                                .size(10.0)
                                .color(ACCENT),
                        );
                    });
                    draw_equalizer(ui, ACCENT, 0.9);
                } else if total > 0.0 {
                    ui.label(RichText::new(fmt_time(total)).size(10.0).color(th.text_dim));
                }

                // Badges.
                ui.horizontal(|ui| {
                    ui.label(RichText::new(sound.extension()).size(9.0).color(th.text_dim));
                    if let Some(hk) = &sound.hotkey {
                        ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.label(RichText::new(hk).size(9.0).color(ACCENT));
                        });
                    }
                });
            });

            // Progress bar (animated fill, taller while hovered).
            let bar_h = 3.0 + 2.0 * hover_t;
            let (bar_rect, _) =
                ui.allocate_exact_size(Vec2::new(card_rect.width() - 20.0, bar_h), Sense::hover());
            let p = if playing {
                self.player.progress(&sound.id)
            } else {
                0.0
            };
            // Smooth the value so seeking doesn't jump.
            let p_smooth = ui
                .ctx()
                .animate_value_with_time(
                    ui.id().with("progress").with(sound.id.as_str()),
                    p.clamp(0.0, 1.0),
                    0.15,
                )
                .clamp(0.0, 1.0);
            if p_smooth > 0.001 {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(bar_rect.left_top(), Vec2::new(bar_rect.width() * p_smooth, bar_h)),
                    CornerRadius::same(2),
                    ACCENT,
                );
            }
            ui.painter().rect_filled(
                bar_rect,
                CornerRadius::same(2),
                th.border.gamma_multiply(0.6),
            );
        });

        // Keep the stable rect for next frame.
        ui.ctx().memory_mut(|m| m.data.insert_temp(card_id, base_rect));
    }

    // ── Status bar ──────────────────────────────────────────────────────────

    fn draw_status_bar(&mut self, ui: &mut Ui, th: &Theme) {
        egui::Panel::bottom("status_bar")
            .frame(Frame::new().fill(th.surface).inner_margin(Margin::symmetric(12, 6)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Now playing.
                    let playing_now = self
                        .store
                        .sounds
                        .iter()
                        .find(|s| self.player.is_playing(&s.id))
                        .map(|s| s.name.clone());
                    if let Some(name) = playing_now {
                        ui.label(RichText::new("▶").size(14.0).color(ACCENT));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("NOW PLAYING").size(8.0).color(th.text_dim));
                            ui.label(RichText::new(name).size(12.0).strong());
                        });
                    } else {
                        ui.label(
                            RichText::new("🔇 No sound playing").size(12.0).color(th.text_dim),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("⏹ Stop All").size(12.0).color(Color32::WHITE))
                                .fill(ERROR_RED)
                                .stroke(Stroke::NONE)
                                .min_size(Vec2::new(84.0, 26.0)),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.player.stop_all();
                    }

                        ui.label(
                            RichText::new(format!("🔊 {}", self.store.settings.output_device))
                                .size(11.0)
                                .color(th.text_dim),
                        );

                        ui.label(RichText::new("🔊").size(13.0));
                        let mut vol = self.master_volume;
                        if ui
                            .add(
                                egui::Slider::new(&mut vol, 0.0..=1.0)
                                    .show_value(false),
                            )
                            .changed()
                        {
                            self.master_volume = vol;
                            self.player.set_master_volume(vol);
                        }
                        ui.label(
                            RichText::new(format!("{}%", (self.master_volume * 100.0) as i32))
                                .size(11.0)
                                .color(th.text_dim),
                        );
                    });
                });

                // Transient status message.
                if let Some((msg, at)) = self.status_msg.as_ref() {
                    if at.elapsed().as_secs() < 4 {
                        ui.label(RichText::new(msg.clone()).size(10.0).color(ACCENT));
                    } else {
                        self.status_msg = None;
                    }
                }
            });
    }

    // ── Dialogs ─────────────────────────────────────────────────────────────

    fn draw_dialogs(&mut self, ctx: &Context, th: &Theme) {
        match self.dialog.clone() {
            Dialog::None => {}
            Dialog::Settings => self.settings_dialog(ctx),
            Dialog::EditSound(id) => self.edit_sound_dialog(ctx, &id),
            Dialog::DeleteSound(id) => {
                let name = self
                    .store
                    .get_sound(&id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                let mut confirmed = false;
                let mut cancel = false;
                let mut open = true;
                egui::Window::new("Delete Sound")
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("Delete \"{name}\"? This cannot be undone."));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Delete").color(ERROR_RED)).clicked() {
                                confirmed = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });
                if confirmed {
                    self.player.stop(&id);
                    self.hotkeys.unbind(&id);
                    let _ = self.store.remove_sound(&id);
                    self.dialog = Dialog::None;
                } else if cancel || !open {
                    self.dialog = Dialog::None;
                }
            }
            Dialog::DeleteCategory(id) => {
                let name = self
                    .store
                    .categories
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                let mut confirmed = false;
                let mut cancel = false;
                let mut open = true;
                egui::Window::new("Delete Category")
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("Delete \"{name}\"? Sounds will be moved to All Sounds."));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Delete").color(ERROR_RED)).clicked() {
                                confirmed = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });
                if confirmed {
                    let _ = self.store.remove_category(&id);
                    if self.selected_category.as_deref() == Some(id.as_str()) {
                        self.selected_category = None;
                    }
                    self.dialog = Dialog::None;
                } else if cancel || !open {
                    self.dialog = Dialog::None;
                }
            }
            Dialog::NewCategory => {
                let mut open = true;
                egui::Window::new("New Category")
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Category name");
                        ui.add_sized(
                            [280.0, 24.0],
                            egui::TextEdit::singleline(&mut self.new_cat_name)
                                .hint_text("e.g. Quotes"),
                        );
                        ui.add_space(6.0);
                        ui.label("Icon");
                        ui.horizontal(|ui| {
                            for (key, glyph) in [
                                ("folder", "📁"),
                                ("emoji", "😀"),
                                ("notifications", "🔔"),
                                ("music", "🎵"),
                                ("star", "⭐"),
                            ] {
                                if ui.selectable_label(self.new_cat_icon == key, glyph).clicked() {
                                    self.new_cat_icon = key.to_string();
                                }
                            }
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let name_ok = !self.new_cat_name.trim().is_empty();
                            if ui
                                .add_enabled(
                                    name_ok,
                                    egui::Button::new("Create").fill(ACCENT.gamma_multiply(0.3)),
                                )
                                .clicked()
                            {
                                let name = self.new_cat_name.trim().to_string();
                                let cat = SoundCategory::new(
                                    name.to_lowercase().replace(' ', "_"),
                                    name,
                                    &self.new_cat_icon,
                                    self.store.categories.len() as i32,
                                );
                                let _ = self.store.add_category(cat);
                                self.new_cat_name.clear();
                                self.new_cat_icon = "folder".to_string();
                                self.dialog = Dialog::None;
                            }
                            if ui.button("Cancel").clicked() {
                                self.dialog = Dialog::None;
                            }
                        });
                    });
                if !open {
                    self.dialog = Dialog::None;
                }
            }
            Dialog::Presets => {
                let mut open = true;
                let mut action: Option<u8> = None; // 1=save, 2=load, 3=delete
                let mut chosen: Option<String> = None;
                egui::Window::new("Presets")
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(
                            RichText::new("Save or load your whole sound library")
                                .size(12.0)
                                .color(th.text_dim),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            ui.add_sized(
                                [160.0, 22.0],
                                egui::TextEdit::singleline(&mut self.preset_name)
                                    .hint_text("my preset"),
                            );
                            if ui.button("Save current").clicked() {
                                action = Some(1);
                            }
                        });
                        ui.separator();
                        let presets = self.store.list_presets();
                        if presets.is_empty() {
                            ui.label("No presets saved yet.");
                        }
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            for p in presets {
                                ui.horizontal(|ui| {
                                    ui.label(&p);
                                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                        if ui.small_button("🗑").clicked() {
                                            action = Some(3);
                                            chosen = Some(p.clone());
                                        }
                                        if ui.small_button("Load").clicked() {
                                            action = Some(2);
                                            chosen = Some(p.clone());
                                        }
                                    });
                                });
                            }
                        });
                    });
                match action {
                    Some(1) => {
                        let name = self.preset_name.trim().to_string();
                        if !name.is_empty() {
                            if self.store.save_preset(&name, "").is_ok() {
                                self.set_status(format!("Preset saved: {name}"));
                            }
                            self.preset_name.clear();
                        }
                    }
                    Some(2) => {
                        if let Some(n) = &chosen {
                            if self.store.load_preset(n).is_ok() {
                                self.rebind_all_hotkeys();
                                self.set_status(format!("Preset loaded: {n}"));
                            }
                        }
                    }
                    Some(3) => {
                        if let Some(n) = &chosen {
                            let _ = self.store.delete_preset(n);
                        }
                    }
                    _ => {}
                }
                if !open {
                    self.dialog = Dialog::None;
                }
            }
        }
    }

    fn settings_dialog(&mut self, ctx: &Context) {
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        let devices = AudioPlayer::output_devices();
        let mut device = self.store.settings.output_device.clone();
        let mut dark = self.store.settings.dark_theme;
        let mut hot_enabled = self.store.settings.hotkeys_enabled;

        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("settings_grid")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Output device");
                        egui::ComboBox::from_id_salt("device_combo")
                            .selected_text(if device == "default" {
                                "System Default".to_string()
                            } else {
                                device.clone()
                            })
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for d in &devices {
                                    let label = if d == "default" {
                                        "System Default".to_string()
                                    } else {
                                        d.clone()
                                    };
                                    ui.selectable_value(&mut device, d.clone(), label);
                                }
                            });
                        ui.end_row();

                        ui.label("Dark theme");
                        ui.checkbox(&mut dark, "");
                        ui.end_row();

                        ui.label("Global hotkeys");
                        ui.checkbox(&mut hot_enabled, "");
                        ui.end_row();

                        ui.label("Minimize to tray");
                        ui.checkbox(&mut self.store.settings.minimize_to_tray, "");
                        ui.end_row();

                        ui.label("Auto start with system");
                        ui.checkbox(&mut self.store.settings.auto_start, "");
                        ui.end_row();

                        ui.label("Virtual audio cable");
                        ui.checkbox(&mut self.store.settings.virtual_cable_enabled, "");
                        ui.end_row();
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.add(egui::Button::new("Save").fill(ACCENT.gamma_multiply(0.3))).clicked() {
                        save = true;
                    }
                });
            });

        if save {
            self.store.settings.output_device = device.clone();
            self.store.settings.dark_theme = dark;
            self.store.settings.hotkeys_enabled = hot_enabled;
            apply_visuals(ctx, dark);
            self.player.set_output_device(&device);
            self.rebind_all_hotkeys();
            let _ = self.store.save_settings();
            self.dialog = Dialog::None;
        } else if cancel || !open {
            self.dialog = Dialog::None;
        }
    }

    fn edit_sound_dialog(&mut self, ctx: &Context, id: &str) {
        let Some(mut sound) = self.store.get_sound(id).cloned() else {
            self.dialog = Dialog::None;
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        let mut delete = false;
        let mut capture_hotkey = false;

        egui::Window::new(format!("Edit: {}", sound.name))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Volume");
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(
                            RichText::new(format!("{}%", (self.edit_volume * 100.0) as i32))
                                .color(ACCENT)
                                .strong(),
                        );
                    });
                });
                ui.add(egui::Slider::new(&mut self.edit_volume, 0.0..=1.0).show_value(false));
                ui.horizontal(|ui| {
                    for pct in [25, 50, 75, 100] {
                        if ui.small_button(format!("{pct}%")).clicked() {
                            self.edit_volume = pct as f32 / 100.0;
                        }
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Hotkey");
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.checkbox(&mut self.edit_hotkey_enabled, "");
                    });
                });
                if self.edit_hotkey_enabled {
                    ui.horizontal(|ui| {
                        let label = self
                            .edit_hotkey
                            .clone()
                            .unwrap_or_else(|| "No keybind set".to_string());
                        ui.label(RichText::new(label).color(ACCENT));
                        if !capture_hotkey
                            && ui
                                .small_button(if self.edit_hotkey.is_some() { "Rebind" } else { "Bind" })
                                .clicked()
                        {
                            capture_hotkey = true;
                        }
                        if self.edit_hotkey.is_some() && ui.small_button("✕").clicked() {
                            self.edit_hotkey = None;
                        }
                    });
                    if capture_hotkey {
                        ui.label(
                            RichText::new("Press any key combination...")
                                .color(ACCENT)
                                .size(11.0),
                        );
                        if let Some(c) = capture_key(ui) {
                            self.edit_hotkey = Some(c);
                            capture_hotkey = false;
                        }
                    }
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("🔁 Loop");
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.checkbox(&mut sound.looped, "");
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Category");
                    let cats = self.store.categories.clone();
                    egui::ComboBox::from_id_salt("edit_cat")
                        .selected_text(
                            cats.iter()
                                .find(|c| c.id == sound.category_id)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "—".into()),
                        )
                        .show_ui(ui, |ui| {
                            for c in &cats {
                                ui.selectable_value(&mut sound.category_id, c.id.clone(), &c.name);
                            }
                        });
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let preview_playing = self.player.is_playing(&sound.id);
                    if ui
                        .button(if preview_playing { "⏹ Stop Preview" } else { "▶ Preview Sound" })
                        .clicked()
                    {
                        if preview_playing {
                            self.player.stop(&sound.id);
                        } else {
                            self.player.play(&sound, self.edit_volume);
                        }
                    }
                    if ui.button(RichText::new("🗑").color(ERROR_RED)).clicked() {
                        delete = true;
                    }

                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.button("Cancel").clicked() {
                            self.player.stop(&sound.id);
                            cancel = true;
                        }
                        if ui.add(egui::Button::new("Save").fill(ACCENT.gamma_multiply(0.3))).clicked() {
                            self.player.stop(&sound.id);
                            save = true;
                        }
                    });
                });
            });

        if save {
            sound.volume = self.edit_volume;
            sound.hotkey = self.edit_hotkey.clone();
            sound.hotkey_enabled = self.edit_hotkey.is_some();
            let _ = self.store.update_sound(sound.clone());
            self.rebind_all_hotkeys();
            self.dialog = Dialog::None;
        } else if cancel || delete {
            if delete {
                self.player.stop(&sound.id);
                self.hotkeys.unbind(&sound.id);
                let _ = self.store.remove_sound(&sound.id);
            }
            self.dialog = Dialog::None;
        } else if !open {
            self.player.stop(&sound.id);
            self.dialog = Dialog::None;
        }
    }
}

/// Capture a key chord from egui input, e.g. "Ctrl+Shift+1".
fn capture_key(ui: &Ui) -> Option<String> {
    ui.ctx().input(|i| {
        for ev in &i.events {
            if let egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } = ev {
                let base = match key {
                    egui::Key::F1 => "F1",
                    egui::Key::F2 => "F2",
                    egui::Key::F3 => "F3",
                    egui::Key::F4 => "F4",
                    egui::Key::F5 => "F5",
                    egui::Key::F6 => "F6",
                    egui::Key::F7 => "F7",
                    egui::Key::F8 => "F8",
                    egui::Key::F9 => "F9",
                    egui::Key::F10 => "F10",
                    egui::Key::F11 => "F11",
                    egui::Key::F12 => "F12",
                    egui::Key::A => "A",
                    egui::Key::B => "B",
                    egui::Key::C => "C",
                    egui::Key::D => "D",
                    egui::Key::E => "E",
                    egui::Key::F => "F",
                    egui::Key::G => "G",
                    egui::Key::H => "H",
                    egui::Key::I => "I",
                    egui::Key::J => "J",
                    egui::Key::K => "K",
                    egui::Key::L => "L",
                    egui::Key::M => "M",
                    egui::Key::N => "N",
                    egui::Key::O => "O",
                    egui::Key::P => "P",
                    egui::Key::Q => "Q",
                    egui::Key::R => "R",
                    egui::Key::S => "S",
                    egui::Key::T => "T",
                    egui::Key::U => "U",
                    egui::Key::V => "V",
                    egui::Key::W => "W",
                    egui::Key::X => "X",
                    egui::Key::Y => "Y",
                    egui::Key::Z => "Z",
                    egui::Key::Num0 => "0",
                    egui::Key::Num1 => "1",
                    egui::Key::Num2 => "2",
                    egui::Key::Num3 => "3",
                    egui::Key::Num4 => "4",
                    egui::Key::Num5 => "5",
                    egui::Key::Num6 => "6",
                    egui::Key::Num7 => "7",
                    egui::Key::Num8 => "8",
                    egui::Key::Num9 => "9",
                    egui::Key::Space => "Space",
                    _ => "",
                };
                if base.is_empty() {
                    continue;
                }
                let mut parts = Vec::new();
                if modifiers.ctrl {
                    parts.push("Ctrl");
                }
                if modifiers.shift {
                    parts.push("Shift");
                }
                if modifiers.alt {
                    parts.push("Alt");
                }
                if modifiers.mac_cmd || modifiers.command {
                    parts.push("Meta");
                }
                parts.push(base);
                return Some(parts.join("+"));
            }
        }
        None
    })
}

// ── Small helpers ───────────────────────────────────────────────────────────

fn builtin_icon(name: &str) -> &'static str {
    match name {
        "emoji" => "😀",
        "notifications" => "🔔",
        "music" | "music_note" => "🎵",
        "star" => "⭐",
        "voice" => "🎙",
        "sfx" => "🔊",
        _ => "📁",
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_category_row(
    ui: &mut Ui,
    th: &Theme,
    rect: egui::Rect,
    name: &str,
    icon: &str,
    count: usize,
    selected: bool,
    deletable: bool,
    hover_t: f32,
) {
    let painter = ui.painter();
    // Slide the row slightly right and tint the background while hovered.
    let rect = rect.translate(Vec2::new(6.0 * hover_t, 0.0));
    let bg = if selected {
        ACCENT.gamma_multiply(0.12)
    } else {
        Color32::TRANSPARENT
    };
    let hover_bg = ACCENT.gamma_multiply(0.07);
    painter.rect_filled(rect, CornerRadius::same(10), bg.lerp_to_gamma(hover_bg, hover_t));
    if hover_t > 0.01 || selected {
        painter.rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(1.0, ACCENT.gamma_multiply(0.35 * hover_t.max(if selected { 0.6 } else { 0.0 }))),
            StrokeKind::Inside,
        );
    }

    // Icon box.
    let icon_rect = egui::Rect::from_min_size(rect.left_top() + Vec2::new(6.0, 4.0), Vec2::new(26.0, 26.0));
    painter.rect_filled(
        icon_rect,
        CornerRadius::same(8),
        if selected { ACCENT.gamma_multiply(0.15) } else { th.card },
    );
    painter.text(
        icon_rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(14.0),
        if selected { ACCENT } else { th.text_dim },
    );

    // Name.
    painter.text(
        egui::pos2(rect.left() + 40.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(13.0),
        if selected { ACCENT } else { th.text },
    );

    // Count badge + delete hint.
    let right_pad = if deletable { 26.0 } else { 10.0 };
    if count > 0 {
        painter.text(
            egui::pos2(rect.right() - right_pad, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{count}"),
            FontId::proportional(11.0),
            if selected { ACCENT } else { th.text_dim },
        );
    }
    if deletable {
        // ✕ brightens on hover so it invites the click.
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            "✕",
            FontId::proportional(10.0),
            ERROR_RED
                .lerp_to_gamma(th.text_dim.gamma_multiply(0.6), 1.0 - hover_t)
                .gamma_multiply(0.5 + 0.5 * hover_t),
        );
    }
}

/// Animated 4-bar equalizer (used on playing cards).
fn draw_equalizer(ui: &mut Ui, color: Color32, height: f32) {
    let t = ui.input(|i| i.time) as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(34.0, height), Sense::hover());
    let painter = ui.painter_at(rect);
    let n = 4;
    let bw = rect.width() / (2.0 * n as f32);
    for i in 0..n {
        let phase = t * (4.0 + 1.3 * i as f32) + i as f32 * 1.7;
        let h = height * (0.35 + 0.65 * (0.5 + 0.5 * phase.sin()));
        let x = rect.left() + (2.0 * i as f32 + 0.5) * bw;
        painter.rect_filled(
            egui::Rect::from_min_size(
                pos2(x, rect.bottom() - h),
                Vec2::new(bw, h),
            ),
            CornerRadius::same(1),
            color,
        );
    }
}

/// Full-window "drop files here" overlay while dragging files over the app.
fn draw_drop_overlay(ctx: &Context) {
    let dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
    if !dragging {
        return;
    }        // Paint over the whole viewport so the drop highlight is visible even when
        // hovering over the title bar and other non-egui chrome.
        let screen: egui::Rect = ctx.input(|i| i.viewport_rect());
    {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, "drop_overlay".into()));
        let t = ctx.input(|i| i.time) as f32;
        let pulse = 0.5 + 0.5 * (t * 3.0).sin();
        painter.rect_filled(
            screen,
            CornerRadius::same(0),
            ACCENT.gamma_multiply(0.08 + 0.04 * pulse),
        );
        let inner = screen.shrink(24.0);
        painter.rect_stroke(
            inner,
            CornerRadius::same(18),
            Stroke::new(2.5, ACCENT.gamma_multiply(0.6 + 0.3 * pulse)),
            StrokeKind::Outside,
        );
        painter.text(
            screen.center(),
            Align2::CENTER_CENTER,
            "⬇ Drop to add sounds",
            FontId::proportional(26.0),
            Color32::WHITE,
        );
    }
}

fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

// ── main ────────────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Soundpad")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Soundpad",
        options,
        Box::new(|cc| Ok(Box::new(SoundpadApp::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlaybackState;

    #[test]
    fn formats_time() {
        assert_eq!(fmt_time(0.0), "0:00");
        assert_eq!(fmt_time(65.0), "1:05");
        assert_eq!(fmt_time(600.5), "10:00");
    }

    #[test]
    fn icon_names_map() {
        assert_eq!(builtin_icon("music"), "🎵");
        assert_eq!(builtin_icon("unknown"), "📁");
    }

    #[test]
    fn playback_state_default_is_stopped() {
        let s = PlaybackState::Stopped;
        assert_ne!(s, PlaybackState::Playing);
    }
}
