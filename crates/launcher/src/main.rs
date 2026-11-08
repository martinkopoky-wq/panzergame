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
    self, Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Layout, Margin, Mesh,
    Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};
use update::{human, Installed, Release};

const WEBSITE: &str = "https://martinkopoky-wq.github.io/panzergame/";
const GITHUB: &str = "https://github.com/martinkopoky-wq/panzergame";

const BG: Color32 = Color32::from_rgb(13, 14, 18);
const PANEL: Color32 = Color32::from_rgb(19, 21, 27);
const LINE: Color32 = Color32::from_rgb(38, 42, 52);
const TEXT: Color32 = Color32::from_rgb(232, 236, 244);
const DIM: Color32 = Color32::from_rgb(138, 146, 164);
const ACCENT: Color32 = Color32::from_rgb(255, 196, 60);
const RED: Color32 = Color32::from_rgb(255, 110, 90);

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

struct Launcher {
    data: PathBuf,
    shared: Arc<Mutex<Shared>>,
    hero: Option<TextureHandle>,
    icon: Option<TextureHandle>,
    close_on_play: bool,
    game: Option<Child>,
    /// Waffen-Doku statt Startbild
    show_docs: bool,
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
        ctx.set_visuals(egui::Visuals::dark());
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
            show_docs: std::env::args().any(|a| a == "--docs"),
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
        let Some(inst) = self.shared.lock().unwrap().installed.clone() else { return };
        match Command::new(inst.exe()).current_dir(&inst.dir).spawn() {
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
        p.rect_filled(rect, 0.0, BG);
        if let Some(t) = &self.hero {
            let (tw, th) = (t.size()[0] as f32, t.size()[1] as f32);
            let k = (rect.width() / tw).max(rect.height() / th);
            let (vw, vh) = (rect.width() / (tw * k), rect.height() / (th * k));
            // Ausschnitt so, dass die Panzer rechts neben dem Text liegen
            let uv = Rect::from_min_size(Pos2::new((1.0 - vw) * 0.2, (1.0 - vh) / 2.0), Vec2::new(vw, vh));
            p.image(t.id(), rect, uv, Color32::from_gray(170));
        }
        let mut mesh = Mesh::default();
        let dark = Color32::from_rgba_unmultiplied(13, 14, 18, 245);
        let clear = Color32::from_rgba_unmultiplied(13, 14, 18, 40);
        // Verlauf von links (dunkel) nach rechts
        mesh.colored_vertex(rect.left_top(), dark);
        mesh.colored_vertex(rect.center_top() + Vec2::new(rect.width() * 0.2, 0.0), clear);
        mesh.colored_vertex(rect.center_bottom() + Vec2::new(rect.width() * 0.2, 0.0), clear);
        mesh.colored_vertex(rect.left_bottom(), dark);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(mesh);
        let mut mesh = Mesh::default();
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(rect.left_bottom() - Vec2::new(0.0, rect.height() * 0.45), Color32::TRANSPARENT);
        mesh.colored_vertex(rect.right_bottom() - Vec2::new(0.0, rect.height() * 0.45), Color32::TRANSPARENT);
        mesh.colored_vertex(rect.right_bottom(), BG);
        mesh.colored_vertex(rect.left_bottom(), BG);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
        p.add(mesh);

        let x = rect.left() + 40.0;
        let mut y = rect.top() + 46.0;
        p.text(Pos2::new(x, y), Align2::LEFT_TOP, "01 / LABYRINTH-PANZERSCHLACHT", FontId::monospace(11.0), DIM);
        y += 26.0;
        p.text(
            Pos2::new(x - 3.0, y),
            Align2::LEFT_TOP,
            "PANZER",
            FontId::new(92.0, FontFamily::Name("title".into())),
            TEXT,
        );
        y += 104.0;
        for line in
            ["Bis zu 8 Spieler über LAN und Internet,", "Bots, 93 Power-ups und jede Runde ein neues Labyrinth."]
        {
            p.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                line,
                FontId::proportional(16.0),
                Color32::from_rgb(200, 205, 216),
            );
            y += 22.0;
        }
        y += 14.0;
        p.text(
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "WINDOWS · MACOS · LINUX · KOSTENLOS · OPEN SOURCE",
            FontId::monospace(11.0),
            DIM,
        );
    }

    fn news(&self, ui: &mut egui::Ui, release: Option<&Release>, offline: Option<&str>) {
        ui.label(RichText::new("02 / NEUIGKEITEN").monospace().size(11.0).color(DIM));
        ui.add_space(14.0);
        let Some(r) = release else {
            let text = offline.unwrap_or("Lade Neuigkeiten …");
            ui.label(RichText::new(text).color(DIM));
            return;
        };
        let title = if r.name.is_empty() { r.tag.clone() } else { r.name.clone() };
        ui.label(RichText::new(title).size(22.0).color(TEXT).family(FontFamily::Name("title".into())));
        ui.label(RichText::new(format!("{} · {}", r.tag, r.date)).monospace().size(11.0).color(DIM));
        ui.add_space(10.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for line in r.body.lines() {
                let l = line.trim();
                if l.is_empty() {
                    ui.add_space(6.0);
                } else if let Some(h) = l.strip_prefix('#') {
                    ui.add_space(4.0);
                    ui.label(RichText::new(h.trim_start_matches('#').trim()).size(15.0).strong().color(ACCENT));
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
    let button =
        egui::Button::new(RichText::new(text).size(20.0).color(color).family(FontFamily::Name("title".into())))
            .fill(if enabled { fill } else { LINE })
            .stroke(Stroke::new(1.0, if filled { ACCENT } else { LINE }))
            .corner_radius(CornerRadius::same(2))
            .min_size(Vec2::new(220.0, 54.0));
    ui.add_enabled(enabled, button).clicked()
}

fn small_button(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(text).monospace().size(11.0).color(DIM))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(2)),
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
            ui.label(RichText::new("Die Waffen-Doku konnte nicht geladen werden.").color(RED));
            return;
        };
        ui.horizontal(|ui| {
            for f in ["Alle", "Waffe", "Effekt", "Wundertüte"] {
                if ui.selectable_label(docs.filter == f, f).clicked() {
                    docs.filter = f;
                }
            }
        });
        ui.add_space(8.0);
        let frame = ((ui.input(|i| i.time) * waffen_rows::FPS) as usize) % waffen_rows::COLS;
        ctx.request_repaint_after(Duration::from_millis(80));
        let filter = docs.filter;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (row, item) in docs.items.iter().enumerate() {
                if filter != "Alle" && item.group != filter {
                    continue;
                }
                // Zeile erst bei Bedarf dekodieren und als Textur halten
                // Eine Zeile wird beim ersten Anzeigen in ihre Einzelbilder zerlegt (je ein Texturbild)
                if docs.sheets[row].is_none() {
                    if let Some(bytes) = waffen_rows::ROWS.get(row) {
                        if let Ok(img) = image::load_from_memory(bytes) {
                            let img = img.to_rgba8();
                            let (fw, fh) = (img.width() / waffen_rows::COLS as u32, img.height());
                            let frames: Vec<TextureHandle> = (0..waffen_rows::COLS as u32)
                                .map(|c| {
                                    let sub = image::imageops::crop_imm(&img, c * fw, 0, fw, fh).to_image();
                                    let size = [fw as usize, fh as usize];
                                    let color = egui::ColorImage::from_rgba_unmultiplied(size, sub.as_raw());
                                    ctx.load_texture(format!("waffe{row}_{c}"), color, TextureOptions::LINEAR)
                                })
                                .collect();
                            docs.sheets[row] = Some(frames);
                        }
                    }
                }
                ui.horizontal(|ui| {
                    let Some(frames) = docs.sheets[row].as_ref() else {
                        ui.add_space(320.0);
                        return;
                    };
                    let tex = &frames[frame];
                    ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(320.0, 218.0)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&item.name).strong().size(18.0).color(TEXT));
                        ui.label(RichText::new(&item.group).small().color(DIM));
                        ui.add_space(4.0);
                        ui.label(RichText::new(&item.text).color(TEXT));
                        if item.ammo > 0 {
                            let ammo = match (item.ammo, item.group.as_str()) {
                                (999, _) => "bis zur Rundenende".to_string(),
                                (n, "Effekt") => format!("{n} s"),
                                (n, _) => format!("{n}× Munition"),
                            };
                            ui.label(RichText::new(ammo).small().color(DIM));
                        }
                    });
                });
                ui.add_space(14.0);
            }
        });
    }
}

impl eframe::App for Launcher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Läuft das Spiel noch?
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
        let update_ready = match (&release, &installed) {
            (Some(r), Some(i)) => r.tag != i.version,
            (Some(_), None) => true,
            _ => false,
        };

        egui::Panel::top("bar")
            .exact_size(46.0)
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin::symmetric(16, 0)).stroke(Stroke::new(1.0, LINE)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(icon) = &self.icon {
                        ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(24.0)));
                    }
                    ui.label(RichText::new("PANZER").size(18.0).color(TEXT).family(FontFamily::Name("title".into())));
                    ui.label(RichText::new("LAUNCHER").monospace().size(11.0).color(DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).monospace().size(11.0).color(DIM),
                        );
                        if small_button(ui, if self.show_docs { "ZURÜCK" } else { "WAFFEN" }) {
                            self.show_docs = !self.show_docs;
                        }
                        if small_button(ui, "GITHUB") {
                            open(GITHUB);
                        }
                        if small_button(ui, "WEBSITE") {
                            open(WEBSITE);
                        }
                    });
                });
            });

        egui::Panel::bottom("actions")
            .exact_size(104.0)
            .frame(
                egui::Frame::new().fill(PANEL).inner_margin(Margin::symmetric(28, 16)).stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 260.0).max(200.0));
                        let (status, color) = match (&phase, &error) {
                            (_, Some(e)) => (e.clone(), RED),
                            (Phase::Checking, _) => ("Suche nach Updates …".to_string(), DIM),
                            (Phase::Downloading, _) => {
                                let (d, t) = progress;
                                (
                                    format!(
                                        "Lade {} herunter … {} / {}",
                                        release.as_ref().map_or("", |r| r.tag.as_str()),
                                        human(d),
                                        human(t)
                                    ),
                                    TEXT,
                                )
                            }
                            (Phase::Idle, None) => match (&installed, &release, &offline) {
                                (Some(i), Some(r), _) if r.tag == i.version => {
                                    (format!("Bereit · {} ist aktuell", i.version), TEXT)
                                }
                                (Some(i), Some(r), _) => {
                                    (format!("Update verfügbar: {} auf {}", i.version, r.tag), ACCENT)
                                }
                                (None, Some(r), _) => {
                                    let size = r
                                        .asset(&update::platform_asset())
                                        .map_or(String::new(), |a| format!(" ({})", human(a.size)));
                                    (format!("Noch nicht installiert · {}{size}", r.tag), TEXT)
                                }
                                (Some(i), None, _) => (format!("Offline · installiert ist {}", i.version), DIM),
                                (None, None, Some(e)) => (e.clone(), RED),
                                (None, None, None) => (String::new(), DIM),
                            },
                        };
                        if self.game.is_some() {
                            ui.label(RichText::new("Das Spiel läuft …").size(15.0).color(ACCENT));
                        } else {
                            ui.label(RichText::new(status).size(15.0).color(color));
                        }
                        ui.add_space(8.0);
                        if phase == Phase::Downloading {
                            let (d, t) = progress;
                            let f = if t > 0 { d as f32 / t as f32 } else { 0.0 };
                            ui.add(
                                egui::ProgressBar::new(f)
                                    .desired_height(8.0)
                                    .fill(ACCENT)
                                    .corner_radius(CornerRadius::same(1)),
                            );
                        } else {
                            ui.horizontal(|ui| {
                                let busy = phase != Phase::Idle;
                                if installed.is_some() && small_button(ui, "SPIELORDNER") {
                                    if let Some(i) = &installed {
                                        open(&i.dir.to_string_lossy());
                                    }
                                }
                                if !busy && installed.is_some() && release.is_some() && small_button(ui, "REPARIEREN") {
                                    self.install(ctx.clone());
                                }
                                if !busy
                                    && (offline.is_some() || error.is_some())
                                    && small_button(ui, "NOCHMAL VERSUCHEN")
                                {
                                    self.check(ctx.clone());
                                }
                                ui.add_space(6.0);
                                if ui
                                    .checkbox(
                                        &mut self.close_on_play,
                                        RichText::new("Launcher beim Spielstart schließen").size(12.0).color(DIM),
                                    )
                                    .changed()
                                {
                                    self.save_settings();
                                }
                            });
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let idle = phase == Phase::Idle && self.game.is_none();
                        if update_ready {
                            let label = if installed.is_some() { "AKTUALISIEREN" } else { "INSTALLIEREN" };
                            if big_button(ui, label, true, idle) {
                                self.install(ctx.clone());
                            }
                        } else if big_button(ui, "SPIELEN", true, idle && installed.is_some()) {
                            self.play(&ctx);
                        }
                    });
                });
                if update_ready && installed.is_some() && phase == Phase::Idle {
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if ui
                            .add(
                                egui::Label::new(
                                    RichText::new("ohne Update spielen").size(11.0).underline().color(DIM),
                                )
                                .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            self.play(&ctx);
                        }
                    });
                }
            });

        egui::Panel::right("news")
            .exact_size(330.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin::same(24)).stroke(Stroke::new(1.0, LINE)))
            .show(ui, |ui| self.news(ui, release.as_ref(), offline.as_deref()));

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ui, |ui| {
            if self.show_docs {
                self.docs_ui(ui);
            } else {
                let rect = ui.max_rect();
                self.hero(ui, rect);
            }
        });
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
