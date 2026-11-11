//! Panzer-Launcher: Desktop-App, die das Spiel herunterlädt, aktuell hält und startet.
//!
//! Schaut beim Start auf GitHub nach dem neuesten Release, lädt das Paket für dieses
//! System, prüft die SHA-256-Prüfsumme, entpackt es in den Datenordner und startet das
//! Spiel. Offline startet er die installierte Version.

// Windows: kein Konsolenfenster neben dem Launcher
#![cfg_attr(windows, windows_subsystem = "windows")]

mod update;

use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Layout, Margin, Pos2,
    Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
use update::{human, Installed, Release};

const WEBSITE: &str = "https://martinkopoky-wq.github.io/panzergame/";
const GITHUB: &str = "https://github.com/martinkopoky-wq/panzergame";

const BG: Color32 = Color32::from_rgb(247, 244, 235);
const PANEL: Color32 = Color32::from_rgb(255, 253, 247);
const LINE: Color32 = Color32::from_rgb(211, 208, 197);
const TEXT: Color32 = Color32::from_rgb(39, 43, 42);
const DIM: Color32 = Color32::from_rgb(91, 96, 90);
const ACCENT: Color32 = Color32::from_rgb(245, 194, 62);
const RED: Color32 = Color32::from_rgb(170, 49, 39);
const DECK: Color32 = Color32::from_rgb(29, 35, 36);
const DECK_TEXT: Color32 = Color32::from_rgb(242, 239, 226);
const DECK_DIM: Color32 = Color32::from_rgb(168, 181, 178);

/// Datenordner: hier landen Spiel, Downloads und Einstellungen des Launchers
fn data_dir() -> PathBuf {
    let home = || std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(home).join("Panzer")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support/Panzer")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local/share"))
            .join("panzer")
    }
}

/// Ordner oder Link mit dem Standardprogramm des Systems öffnen
fn open(target: &str) {
    let cmd = if cfg!(windows) {
        Command::new("explorer").arg(target).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(target).spawn()
    } else {
        Command::new("xdg-open").arg(target).spawn()
    };
    let _ = cmd;
}

#[derive(Clone, PartialEq)]
enum Phase {
    Checking,
    Idle,
    Downloading,
}

/// Zustand, den der Hintergrund-Thread füllt
struct Shared {
    phase: Phase,
    release: Option<Release>,
    installed: Option<Installed>,
    progress: (u64, u64),
    error: Option<String>,
    /// Release konnte nicht geladen werden (offline o. Ä.)
    offline: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Play,
    Weapons,
    News,
}

struct Launcher {
    data: PathBuf,
    shared: Arc<Mutex<Shared>>,
    hero: Option<TextureHandle>,
    icon: Option<TextureHandle>,
    close_on_play: bool,
    game: Option<Child>,
    /// Waffen-Doku statt Startbild
    page: Page,
    local_game: Option<PathBuf>,
    docs: Option<Docs>,
}

/// Eine Waffe oder ein Effekt aus website/data/waffen.json
struct DocItem {
    name: String,
    group: String,
    text: String,
    ammo: i64,
}

/// Doku: Sprite-Blatt mit einer Zeile pro Waffe (12 Bilder der Aufnahme)
struct Docs {
    /// Zeilen-Texturen, erst beim ersten Anzeigen geladen
    sheets: Vec<Option<Vec<TextureHandle>>>,
    rows: usize,
    items: Vec<DocItem>,
    filter: &'static str,
}

const DOCS_JSON: &str = include_str!("../../../website/data/waffen.json");
mod waffen_rows;

fn load_docs(ctx: &egui::Context) -> Option<Docs> {
    let v: serde_json::Value = serde_json::from_str(DOCS_JSON).ok()?;
    let items = v
        .as_array()?
        .iter()
        .map(|i| DocItem {
            name: i["name"].as_str().unwrap_or("").to_string(),
            group: i["group"].as_str().unwrap_or("").to_string(),
            text: i["text"].as_str().unwrap_or("").to_string(),
            ammo: i["ammo"].as_i64().unwrap_or(0),
        })
        .collect::<Vec<_>>();
    let rows = items.len();
    let _ = ctx;
    Some(Docs { sheets: (0..rows).map(|_| None).collect(), rows, items, filter: "Alle" })
}

fn load_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, color, TextureOptions::LINEAR))
}

impl Launcher {
    fn new(cc: &eframe::CreationContext) -> Launcher {
        let ctx = &cc.egui_ctx;
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "title".into(),
            Arc::new(FontData::from_static(include_bytes!("../../../assets/fonts/RobotoCondensed-Bold.ttf"))),
        );
        fonts.font_data.insert(
            "body".into(),
            Arc::new(FontData::from_static(include_bytes!("../../../assets/fonts/Roboto-Medium.ttf"))),
        );
        fonts.families.insert(FontFamily::Name("title".into()), vec!["title".into()]);
        if let Some(p) = fonts.families.get_mut(&FontFamily::Proportional) {
            p.insert(0, "body".into());
        }
        ctx.set_fonts(fonts);
        let mut visuals = egui::Visuals::light();
        visuals.override_text_color = Some(TEXT);
        visuals.panel_fill = BG;
        visuals.window_fill = PANEL;
        visuals.faint_bg_color = BG;
        visuals.extreme_bg_color = PANEL;
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = Stroke::new(1.0, TEXT);
        for widget in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered, &mut visuals.widgets.active] {
            widget.corner_radius = CornerRadius::same(8);
            widget.fg_stroke = Stroke::new(1.0, TEXT);
            widget.bg_stroke = Stroke::new(1.0, LINE);
        }
        ctx.set_visuals_of(egui::Theme::Light, visuals);
        ctx.set_theme(egui::Theme::Light);
        ctx.style_mut_of(egui::Theme::Light, |s| {
            s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.0));
            s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
            s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(12.0, 8.0);
            s.spacing.interact_size.y = 32.0;
        });
        let data = data_dir();
        let close_on_play = std::fs::read_to_string(data.join("launcher.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("close_on_play").and_then(|b| b.as_bool()))
            .unwrap_or(true);
        let shared = Arc::new(Mutex::new(Shared {
            phase: Phase::Checking,
            release: None,
            installed: update::load_installed(&data),
            progress: (0, 0),
            error: None,
            offline: None,
        }));
        let mut l = Launcher {
            data,
            shared,
            hero: load_png(ctx, "hero", include_bytes!("../../../assets/launcher/hero.jpg")),
            icon: load_png(ctx, "icon", include_bytes!("../../../assets/icon/panzer-128.png")),
            close_on_play,
            game: None,
            // --docs startet direkt in der Waffen-Doku (zum Testen und für Links)
            page: if std::env::args().any(|a| a == "--docs") { Page::Weapons } else { Page::Play },
            local_game: if std::env::args().any(|a| a == "--local") {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "panzer.exe" } else { "panzer" })))
                    .filter(|p| p.is_file())
            } else {
                None
            },
            docs: None,
        };
        l.check(ctx.clone());
        l
    }

    fn save_settings(&self) {
        let _ = std::fs::create_dir_all(&self.data);
        let _ = std::fs::write(
            self.data.join("launcher.json"),
            serde_json::json!({ "close_on_play": self.close_on_play }).to_string(),
        );
    }

    /// Im Hintergrund nach dem neuesten Release schauen
    fn check(&mut self, ctx: egui::Context) {
        let shared = self.shared.clone();
        {
            let mut s = shared.lock().unwrap();
            s.phase = Phase::Checking;
            s.error = None;
            s.offline = None;
        }
        std::thread::spawn(move || {
            let result = update::fetch_latest(&update::feed_url());
            let mut s = shared.lock().unwrap();
            match result {
                Ok(r) => s.release = Some(r),
                Err(e) => s.offline = Some(e),
            }
            s.phase = Phase::Idle;
            ctx.request_repaint();
        });
    }

    /// Spiel installieren bzw. aktualisieren
    fn install(&mut self, ctx: egui::Context) {
        let shared = self.shared.clone();
        let data = self.data.clone();
        let Some(release) = shared.lock().unwrap().release.clone() else { return };
        {
            let mut s = shared.lock().unwrap();
            s.phase = Phase::Downloading;
            s.progress = (0, 0);
            s.error = None;
        }
        std::thread::spawn(move || {
            let mut last = std::time::Instant::now();
            let progress_shared = shared.clone();
            let progress_ctx = ctx.clone();
            let mut progress = move |done: u64, total: u64| {
                progress_shared.lock().unwrap().progress = (done, total);
                if last.elapsed() > Duration::from_millis(50) {
                    last = std::time::Instant::now();
                    progress_ctx.request_repaint();
                }
            };
            let result = update::install(&release, &data, &mut progress);
            let mut s = shared.lock().unwrap();
            match result {
                Ok(inst) => s.installed = Some(inst),
                Err(e) => s.error = Some(e),
            }
            s.phase = Phase::Idle;
            ctx.request_repaint();
        });
    }

    fn play(&mut self, ctx: &egui::Context) {
        let executable = if let Some(local) = &self.local_game {
            local.clone()
        } else {
            let Some(inst) = self.shared.lock().unwrap().installed.clone() else { return };
            inst.exe()
        };
        let mut command = Command::new(&executable);
        if let Some(dir) = executable.parent() {
            command.current_dir(dir);
        }
        match command.spawn() {
            Ok(child) => {
                if self.close_on_play {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.game = Some(child);
                }
            }
            Err(e) => self.shared.lock().unwrap().error = Some(format!("Spiel startet nicht: {e}")),
        }
    }

    /// Bild füllend zeichnen (wie CSS object-fit: cover), mit Abdunklung nach links und unten
    fn hero(&self, ui: &mut egui::Ui, rect: Rect) {
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 8, DECK);
        let image_rect = Rect::from_min_max(rect.min + Vec2::splat(8.0), rect.right_bottom() - Vec2::new(8.0, 76.0));
        if let Some(t) = &self.hero {
            let aspect = image_rect.width() / image_rect.height().max(1.0);
            let source = t.size()[0] as f32 / t.size()[1] as f32;
            let (uw, uh) = if aspect > source { (1.0, source / aspect) } else { (aspect / source, 1.0) };
            let uv = Rect::from_min_size(Pos2::new((1.0 - uw) / 2.0, (1.0 - uh) / 2.0), Vec2::new(uw, uh));
            p.add(egui::epaint::RectShape::filled(image_rect, 4, Color32::WHITE).with_texture(t.id(), uv));
        }
        let y = rect.bottom() - 45.0;
        p.text(
            Pos2::new(rect.left() + 24.0, y),
            Align2::LEFT_CENTER,
            "Kleine Panzer, große Querschläger",
            FontId::proportional(21.0),
            DECK_TEXT,
        );
        p.text(
            Pos2::new(rect.left() + 24.0, y + 26.0),
            Align2::LEFT_CENTER,
            "Bis zu 8 Panzer · Lokal oder online · Neues Labyrinth pro Runde",
            FontId::proportional(12.0),
            DECK_DIM,
        );
    }

    fn news(&self, ui: &mut egui::Ui, release: Option<&Release>, offline: Option<&str>) {
        let Some(r) = release else {
            let text = offline.unwrap_or("Lade Neuigkeiten …");
            ui.label(RichText::new(text).color(DIM));
            return;
        };
        let title = if r.name.is_empty() { r.tag.clone() } else { r.name.clone() };
        ui.label(RichText::new(title).size(22.0).color(TEXT).family(FontFamily::Name("title".into())));
        ui.label(RichText::new(format!("{} · {}", r.tag, r.date)).monospace().size(12.0).color(DIM));
        ui.add_space(12.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for line in r.body.lines() {
                let l = line.trim();
                if l.is_empty() {
                    ui.add_space(8.0);
                } else if let Some(h) = l.strip_prefix('#') {
                    ui.add_space(4.0);
                    ui.label(RichText::new(h.trim_start_matches('#').trim()).size(15.0).strong().color(TEXT));
                } else if let Some(item) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
                    ui.label(RichText::new(format!("•  {}", item.replace("**", ""))).color(TEXT));
                } else {
                    ui.label(RichText::new(l.replace("**", "")).color(TEXT));
                }
            }
        });
    }
}

/// Großer Knopf im Stil der Website (gefüllt bzw. umrandet)
fn big_button(ui: &mut egui::Ui, text: &str, filled: bool, enabled: bool) -> bool {
    let (fill, color) = if filled { (ACCENT, Color32::from_rgb(20, 18, 10)) } else { (Color32::TRANSPARENT, TEXT) };
    let color = if enabled { color } else { DIM };
    let button = egui::Button::new(RichText::new(text).size(18.0).color(color))
        .fill(if enabled { fill } else { LINE })
        .stroke(Stroke::new(1.0, if filled { ACCENT } else { LINE }))
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(208.0, 56.0));
    let response = ui.add_enabled(enabled, button);
    if filled && enabled {
        for j in 0..3 {
            let at = response.rect.left_center() + Vec2::new(14.0 + j as f32 * 5.0, -4.0);
            ui.painter().line_segment([at, at + Vec2::new(3.0, 9.0)], Stroke::new(1.5, TEXT));
        }
    }
    response.clicked()
}

fn small_button(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(text).size(13.0).color(TEXT))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(8)),
    )
    .clicked()
}

impl Launcher {
    /// Waffen-Doku: Zeile pro Waffe, animiert aus dem Sprite-Blatt
    fn docs_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.docs.is_none() {
            self.docs = load_docs(&ctx);
        }
        let Some(docs) = self.docs.as_mut() else {
            ui.label(RichText::new("Die Waffen-Doku konnte nicht geladen werden").color(RED));
            return;
        };
        ui.horizontal(|ui| {
            for filter in ["Alle", "Waffe", "Effekt", "Wundertüte"] {
                let count = docs.items.iter().filter(|i| filter == "Alle" || i.group == filter).count();
                if ui
                    .add(
                        egui::Button::new(RichText::new(format!("{filter}  {count}")).size(13.0).color(TEXT))
                            .fill(if docs.filter == filter { ACCENT } else { PANEL })
                            .stroke(Stroke::new(1.0, if docs.filter == filter { TEXT } else { LINE }))
                            .corner_radius(CornerRadius::same(4))
                            .min_size(Vec2::new(92.0, 36.0)),
                    )
                    .clicked()
                {
                    docs.filter = filter;
                }
            }
        });
        ui.add_space(16.0);
        let frame = ((ui.input(|i| i.time) * waffen_rows::FPS) as usize) % waffen_rows::COLS;
        ctx.request_repaint_after(Duration::from_millis(80));
        let filter = docs.filter;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (row, item) in docs.items.iter().enumerate() {
                if filter != "Alle" && item.group != filter {
                    continue;
                }
                let width = ui.available_width();
                let preview_w = ((width - 48.0) * 0.45).clamp(220.0, 400.0);
                let preview_h = preview_w * 218.0 / 320.0;
                let height = preview_h.max(220.0) + 32.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
                // Only visible rows decode their per-frame textures; scrolling does not load all 93 at once.
                if ui.is_rect_visible(rect) {
                    if docs.sheets[row].is_none() {
                        if let Some(bytes) = waffen_rows::ROWS.get(row) {
                            if let Ok(img) = image::load_from_memory(bytes) {
                                let img = img.to_rgba8();
                                let (fw, fh) = (img.width() / waffen_rows::COLS as u32, img.height());
                                docs.sheets[row] = Some(
                                    (0..waffen_rows::COLS as u32)
                                        .map(|c| {
                                            let sub = image::imageops::crop_imm(&img, c * fw, 0, fw, fh).to_image();
                                            ctx.load_texture(
                                                format!("waffe{row}_{c}"),
                                                egui::ColorImage::from_rgba_unmultiplied(
                                                    [fw as usize, fh as usize],
                                                    sub.as_raw(),
                                                ),
                                                TextureOptions::LINEAR,
                                            )
                                        })
                                        .collect(),
                                );
                            }
                        }
                    }
                    ui.painter().rect_filled(rect, 8, PANEL);
                    ui.painter().rect_stroke(rect, 8, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
                    let inner = rect.shrink(16.0);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                        ui.horizontal(|ui| {
                            if let Some(frames) = &docs.sheets[row] {
                                ui.add(
                                    egui::Image::new(&frames[frame]).fit_to_exact_size(Vec2::new(preview_w, preview_h)),
                                );
                            } else {
                                ui.allocate_space(Vec2::new(preview_w, preview_h));
                            }
                            ui.add_space(16.0);
                            ui.vertical(|ui| {
                                ui.set_width((width - preview_w - 64.0).max(120.0));
                                ui.label(
                                    RichText::new(&item.name)
                                        .size(22.0)
                                        .color(TEXT)
                                        .family(FontFamily::Name("title".into())),
                                );
                                let amount = match (item.ammo, item.group.as_str()) {
                                    (999, _) => "bis zum Rundenende".to_string(),
                                    (0, _) => String::new(),
                                    (n, "Effekt") => format!("{n} Sekunden"),
                                    (n, _) => format!("{n} Schuss"),
                                };
                                let meta = if amount.is_empty() {
                                    item.group.clone()
                                } else {
                                    format!("{}  ·  {amount}", item.group)
                                };
                                ui.label(RichText::new(meta).size(12.0).color(DIM));
                                ui.add_space(12.0);
                                ui.label(RichText::new(&item.text).size(14.0).color(TEXT));
                            });
                        });
                    });
                }
                ui.add_space(12.0);
            }
        });
    }
}

impl eframe::App for Launcher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(child) = self.game.as_mut() {
            if !matches!(child.try_wait(), Ok(None)) {
                self.game = None;
            }
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        let (phase, release, installed, progress, error, offline) = {
            let s = self.shared.lock().unwrap();
            (s.phase.clone(), s.release.clone(), s.installed.clone(), s.progress, s.error.clone(), s.offline.clone())
        };
        let local = self.local_game.is_some();
        let update_ready = !local
            && match (&release, &installed) {
                (Some(r), Some(i)) => r.tag != i.version,
                (Some(_), None) => true,
                _ => false,
            };
        egui::Panel::left("navigation")
            .exact_size(176.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(DECK).inner_margin(Margin::symmetric(16, 24)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(icon) = &self.icon {
                        ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(28.0)));
                    }
                    ui.label(
                        RichText::new("PANZER").size(23.0).family(FontFamily::Name("title".into())).color(DECK_TEXT),
                    );
                });
                ui.add_space(32.0);
                for (page, label) in [(Page::Play, "Spielen"), (Page::Weapons, "Waffen"), (Page::News, "Neuigkeiten")] {
                    let selected = self.page == page;
                    let response = ui.add_sized(
                        Vec2::new(ui.available_width(), 44.0),
                        egui::Button::new(RichText::new(label).size(15.0).color(if selected {
                            TEXT
                        } else {
                            DECK_TEXT
                        }))
                        .fill(if selected { ACCENT } else { DECK })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(4)),
                    );
                    if selected {
                        for j in 0..3 {
                            let at = response.rect.left_center() + Vec2::new(9.0 + j as f32 * 4.0, -4.0);
                            ui.painter().line_segment([at, at + Vec2::new(3.0, 8.0)], Stroke::new(1.5, TEXT));
                        }
                    }
                    if response.clicked() {
                        self.page = page;
                    }
                    ui.add_space(4.0);
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.label(
                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).monospace().size(11.0).color(DECK_DIM),
                    );
                    ui.add_space(16.0);
                    for (label, url) in [("Quellcode", GITHUB), ("Website", WEBSITE)] {
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).size(12.0).color(DECK_DIM))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE),
                            )
                            .clicked()
                        {
                            open(url);
                        }
                    }
                });
            });
        egui::Panel::bottom("launch_bay")
            .exact_size(if self.page == Page::Play { 148.0 } else { 112.0 })
            .frame(
                egui::Frame::new().fill(PANEL).inner_margin(Margin::symmetric(24, 16)).stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let left = (ui.available_width() - 240.0).max(240.0);
                    ui.vertical(|ui| {
                        ui.set_width(left);
                        let (status, color) = if let Some(error) = &error {
                            (error.clone(), RED)
                        } else if local {
                            (format!("Lokaler Build · v{}", env!("CARGO_PKG_VERSION")), TEXT)
                        } else {
                            match (&phase, &error) {
                                (_, Some(e)) => (e.clone(), RED),
                                (Phase::Checking, _) => ("Suche nach Updates …".to_string(), DIM),
                                (Phase::Downloading, _) => {
                                    (format!("Download: {} / {}", human(progress.0), human(progress.1)), TEXT)
                                }
                                (Phase::Idle, None) => match (&installed, &release, &offline) {
                                    (Some(i), Some(r), _) if r.tag == i.version => {
                                        (format!("Bereit · {} ist aktuell", i.version), TEXT)
                                    }
                                    (Some(i), Some(r), _) => (format!("Update: {} auf {}", i.version, r.tag), TEXT),
                                    (None, Some(r), _) => (format!("Noch nicht installiert · {}", r.tag), TEXT),
                                    (Some(i), None, _) => (format!("Offline · installiert ist {}", i.version), DIM),
                                    (None, None, Some(e)) => (e.clone(), RED),
                                    _ => ("Noch keine Installation".to_string(), DIM),
                                },
                            }
                        };
                        ui.label(
                            RichText::new(if self.game.is_some() { "Das Spiel läuft" } else { &status })
                                .size(15.0)
                                .color(color),
                        );
                        ui.add_space(8.0);
                        if phase == Phase::Downloading && !local {
                            let fraction = if progress.1 > 0 { progress.0 as f32 / progress.1 as f32 } else { 0.0 };
                            ui.add(egui::ProgressBar::new(fraction).desired_height(8.0).fill(ACCENT));
                        } else {
                            ui.horizontal_wrapped(|ui| {
                                if (installed.is_some() || local) && small_button(ui, "Spielordner") {
                                    if let Some(path) = &self.local_game {
                                        if let Some(dir) = path.parent() {
                                            open(&dir.to_string_lossy());
                                        }
                                    } else if let Some(i) = &installed {
                                        open(&i.dir.to_string_lossy());
                                    }
                                }
                                if !local
                                    && phase == Phase::Idle
                                    && installed.is_some()
                                    && release.is_some()
                                    && small_button(ui, "Reparieren")
                                {
                                    self.install(ctx.clone());
                                }
                                if !local
                                    && phase == Phase::Idle
                                    && (offline.is_some() || error.is_some())
                                    && small_button(ui, "Erneut versuchen")
                                {
                                    self.check(ctx.clone());
                                }
                            });
                            if self.page == Page::Play {
                                ui.add_space(8.0);
                                if ui
                                    .checkbox(
                                        &mut self.close_on_play,
                                        RichText::new("Launcher beim Spielstart schließen").size(12.0).color(DIM),
                                    )
                                    .changed()
                                {
                                    self.save_settings();
                                }
                            }
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let idle = (local || phase == Phase::Idle) && self.game.is_none();
                        let label = if update_ready {
                            if installed.is_some() {
                                "Aktualisieren"
                            } else {
                                "Installieren"
                            }
                        } else {
                            "Spielen"
                        };
                        if big_button(ui, label, true, idle && (update_ready || local || installed.is_some())) {
                            if update_ready {
                                self.install(ctx.clone());
                            } else {
                                self.play(&ctx);
                            }
                        }
                    });
                });
                if update_ready
                    && installed.is_some()
                    && phase == Phase::Idle
                    && ui.link(RichText::new("Ohne Update spielen").size(12.0).color(DIM)).clicked()
                {
                    self.play(&ctx);
                }
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(Margin::same(24))).show(
            ui,
            |ui| {
                let (title, subtitle) = match self.page {
                    Page::Play => ("Jeder Winkel zählt", "Die nächste Runde beginnt im Labyrinth"),
                    Page::Weapons => ("Waffenlager", "Was du aufsammelst und wie du es einsetzt"),
                    Page::News => ("Neuigkeiten", "Änderungen an Panzer"),
                };
                ui.label(RichText::new(title).size(30.0).color(TEXT).family(FontFamily::Name("title".into())));
                ui.label(RichText::new(subtitle).size(13.0).color(DIM));
                ui.add_space(20.0);
                match self.page {
                    Page::Play => {
                        let rect = ui.available_rect_before_wrap();
                        self.hero(ui, rect);
                    }
                    Page::Weapons => self.docs_ui(ui),
                    Page::News => self.news(ui, release.as_ref(), offline.as_deref()),
                }
            },
        );
    }
}

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!("../../../assets/icon/panzer-256.png")).ok().map(|i| {
        let i = i.to_rgba8();
        Arc::new(egui::IconData { width: i.width(), height: i.height(), rgba: i.into_raw() })
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Panzer")
        .with_app_id("panzer-launcher")
        .with_inner_size([1040.0, 640.0])
        .with_min_inner_size([860.0, 540.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Panzer", options, Box::new(|cc| Ok(Box::new(Launcher::new(cc)))))
}
