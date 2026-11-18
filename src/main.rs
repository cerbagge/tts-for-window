#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod edge;
mod edge_drm;

use config::Config;
use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

enum Event {
    Audio(String, Result<Vec<u8>, String>),
    Voices(Result<Vec<String>, String>),
    Error(String),
}

struct App {
    cfg: Config,
    saved: Config,
    text: String,
    status: String,
    busy: usize,
    devices: Vec<String>,
    voices: Vec<String>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    synth_tx: Sender<(Config, String)>,
    audio_tx: Sender<audio::Cmd>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        install_korean_font(&cc.egui_ctx);
        let cfg = Config::load();
        let (tx, rx) = mpsc::channel();

        let ctx = cc.egui_ctx.clone();
        let err_tx = tx.clone();
        let audio_tx = audio::spawn(move |e| {
            let _ = err_tx.send(Event::Error(e));
            ctx.request_repaint();
        });

        // Single worker keeps playback order identical to input order.
        let (synth_tx, synth_rx) = mpsc::channel::<(Config, String)>();
        let ctx = cc.egui_ctx.clone();
        let ev_tx = tx.clone();
        std::thread::spawn(move || {
            for (cfg, text) in synth_rx {
                let r = edge::synthesize(&cfg.voice, &text, cfg.rate, cfg.pitch);
                let _ = ev_tx.send(Event::Audio(text, r));
                ctx.request_repaint();
            }
        });

        let app = Self {
            saved: cfg.clone(),
            cfg,
            text: String::new(),
            status: "준비되었습니다.".into(),
            busy: 0,
            devices: audio::list_output_devices(),
            voices: vec![],
            tx,
            rx,
            synth_tx,
            audio_tx,
        };
        app.apply_devices();
        app.apply_on_top(&cc.egui_ctx);
        app
    }

    fn apply_devices(&self) {
        let mon = self.cfg.monitor_enabled.then(|| self.cfg.monitor_device.clone());
        let _ = self.audio_tx.send(audio::Cmd::SetDevices(self.cfg.output_device.clone(), mon));
    }

    fn apply_on_top(&self, ctx: &egui::Context) {
        let level = if self.cfg.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
    }

    fn speak(&mut self) {
        let text = expand_jamo(self.text.trim()).trim().to_string();
        if text.is_empty() {
            self.text.clear();
            return;
        }
        self.busy += 1;
        self.status = format!("생성 중: {text}");
        let _ = self.synth_tx.send((self.cfg.clone(), text));
        self.text.clear();
    }

    fn fetch_voices(&mut self, ctx: &egui::Context) {
        let lang = self.cfg.language.trim().to_string();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.status = "음성 목록을 불러오는 중입니다...".into();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Voices(edge::list_voices(&lang)));
            ctx.request_repaint();
        });
    }

    fn handle_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::Audio(text, Ok(bytes)) => {
                    self.busy = self.busy.saturating_sub(1);
                    self.status = format!("재생: {text}");
                    let _ = self.audio_tx.send(audio::Cmd::Play(
                        Arc::new(bytes),
                        self.cfg.volume,
                        self.cfg.monitor_volume,
                    ));
                }
                Event::Audio(_, Err(e)) => {
                    self.busy = self.busy.saturating_sub(1);
                    self.status = format!("오류: {e}");
                }
                Event::Voices(Ok(v)) => {
                    self.status = format!("음성 {}개를 불러왔습니다.", v.len());
                    self.voices = v;
                }
                Event::Voices(Err(e)) => self.status = format!("오류: {e}"),
                Event::Error(e) => self.status = e,
            }
        }
    }
}

/// Edge cannot read chat shorthand made of bare jamo (ㅋㅋㅋ, ㄱㄱ, ...).
/// Replace each jamo run with its most common reading; drop unknown jamo.
fn expand_jamo(text: &str) -> String {
    const WORDS: &[(&str, &str)] = &[
        ("ㅇㅋ", "오케이"), ("ㄱㅅ", "감사"), ("ㄳ", "감사"), ("ㅅㄱ", "수고"),
        ("ㅈㅅ", "죄송"), ("ㅊㅋ", "축하"), ("ㅎㅇ", "하이"), ("ㄹㅇ", "레알"),
        ("ㅇㅈ", "인정"), ("ㅁㅊ", "미친"),
    ];
    // Repeated letters (ㅋㅋ, ㄱㄱ, ㅂㅂ ...) are read per letter.
    const LETTERS: &[(char, &str)] = &[
        ('ㅋ', "크"), ('ㅎ', "흐"), ('ㅠ', "흑"), ('ㅜ', "흑"), ('ㄱ', "고"),
        ('ㄷ', "덜"), ('ㅇ', "응"), ('ㄴ', "노"), ('ㅂ', "바이"), ('ㅃ', "빠이"),
    ];
    const MAX_REPEAT: usize = 3;
    let is_jamo = |c: char| ('\u{3131}'..='\u{318E}').contains(&c);

    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_jamo(chars[i]) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_jamo(chars[i]) {
            i += 1;
        }
        let run = &chars[start..i];
        let mut j = 0;
        while j < run.len() {
            // Word first (2 jamo, or 1 for ㄳ), then a run of the same letter.
            let word = [2, 1].iter().find_map(|&len| {
                let piece: String = run.get(j..j + len)?.iter().collect();
                WORDS.iter().find(|(k, _)| *k == piece).map(|(_, w)| (len, *w))
            });
            if let Some((len, w)) = word {
                out.push_str(w);
                j += len;
                continue;
            }
            let c = run[j];
            let mut n = 0;
            while j < run.len() && run[j] == c {
                n += 1;
                j += 1;
            }
            if let Some((_, r)) = LETTERS.iter().find(|(k, _)| *k == c) {
                out.push_str(&r.repeat(n.min(MAX_REPEAT)));
            }
        }
    }
    out
}

fn device_combo(ui: &mut egui::Ui, id: &str, devices: &[String], value: &mut Option<String>) -> bool {
    let mut changed = false;
    let label = value.clone().unwrap_or_else(|| "기본 장치".into());
    egui::ComboBox::from_id_salt(id)
        .width(260.0)
        .selected_text(label)
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(value, None, "기본 장치").changed();
            for d in devices {
                changed |= ui.selectable_value(value, Some(d.clone()), d).changed();
            }
        });
    changed
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();

        egui::TopBottomPanel::bottom("input").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.text)
                        .hint_text("읽을 내용을 입력하고 Enter를 누르십시오")
                        .desired_width(ui.available_width() - 110.0),
                );
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter || ui.button("읽기").clicked() {
                    self.speak();
                    resp.request_focus();
                }
                if ui.button("중지").clicked() {
                    let _ = self.audio_tx.send(audio::Cmd::Stop);
                    self.status = "재생을 중지했습니다.".into();
                }
            });
            ui.horizontal(|ui| {
                if self.busy > 0 {
                    ui.spinner();
                }
                ui.label(&self.status);
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("main").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("음성");
                ui.horizontal(|ui| {
                    let (voice, list) = (&mut self.cfg.voice, &self.voices);
                    ui.add(egui::TextEdit::singleline(voice).desired_width(200.0));
                    egui::ComboBox::from_id_salt("voices")
                        .selected_text("목록")
                        .width(60.0)
                        .show_ui(ui, |ui| {
                            if list.is_empty() {
                                ui.label("'불러오기'를 눌러 주십시오");
                            }
                            for v in list {
                                ui.selectable_value(voice, v.clone(), v);
                            }
                        });
                    if ui.button("불러오기").clicked() {
                        self.fetch_voices(ctx);
                    }
                });
                ui.end_row();

                ui.label("음성 언어 필터");
                ui.add(egui::TextEdit::singleline(&mut self.cfg.language).hint_text("ko-KR (비우면 전체)"));
                ui.end_row();

                ui.label("속도");
                ui.add(egui::Slider::new(&mut self.cfg.rate, 0.5..=2.0).step_by(0.05));
                ui.end_row();

                ui.label("피치");
                ui.add(egui::Slider::new(&mut self.cfg.pitch, -10.0..=10.0).step_by(0.5).suffix(" st"));
                ui.end_row();

                ui.label("출력 (가상 마이크)");
                let mut dev_changed = device_combo(ui, "out", &self.devices, &mut self.cfg.output_device);
                ui.end_row();

                ui.label("출력 볼륨");
                ui.add(egui::Slider::new(&mut self.cfg.volume, 0.0..=2.0));
                ui.end_row();

                ui.label("모니터 (내 스피커)");
                ui.horizontal(|ui| {
                    dev_changed |= ui.checkbox(&mut self.cfg.monitor_enabled, "").changed();
                    ui.add_enabled_ui(self.cfg.monitor_enabled, |ui| {
                        dev_changed |= device_combo(ui, "mon", &self.devices, &mut self.cfg.monitor_device);
                    });
                });
                ui.end_row();

                ui.label("모니터 볼륨");
                ui.add(egui::Slider::new(&mut self.cfg.monitor_volume, 0.0..=2.0));
                ui.end_row();

                ui.label("");
                ui.horizontal(|ui| {
                    if ui.button("장치 새로고침").clicked() {
                        self.devices = audio::list_output_devices();
                        dev_changed = true;
                    }
                    if ui.checkbox(&mut self.cfg.always_on_top, "항상 위").changed() {
                        self.apply_on_top(ctx);
                    }
                });
                ui.end_row();

                if dev_changed {
                    self.apply_devices();
                }
            });

            });
        });

        if self.cfg != self.saved {
            if let Err(e) = self.cfg.save() {
                self.status = format!("설정 저장 실패: {e}");
            }
            self.saved = self.cfg.clone();
        }
    }
}

fn install_korean_font(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    for name in ["malgun.ttf", "gulim.ttc"] {
        if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\{name}")) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("kr".into(), Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().insert(0, "kr".into());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TTS for Window")
            .with_inner_size([560.0, 520.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
                    .expect("icon.png"),
            ),
        ..Default::default()
    };
    eframe::run_native("TTS for Window", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

#[cfg(test)]
mod tests {
    use super::expand_jamo;

    #[test]
    fn expands_chat_shorthand() {
        assert_eq!(expand_jamo("ㄱㄱ"), "고고");
        assert_eq!(expand_jamo("ㅋㅋㅋ"), "크크크");
        assert_eq!(expand_jamo("ㅋㅋㅋㅋㅋㅋㅋㅋ"), "크크크");
        assert_eq!(expand_jamo("ㅎㅎ 좋아요"), "흐흐 좋아요");
        assert_eq!(expand_jamo("ㅠㅠ"), "흑흑");
        assert_eq!(expand_jamo("ㅇㅋ 알겠어"), "오케이 알겠어");
        assert_eq!(expand_jamo("ㄹㅇㅋㅋ"), "레알크크");
        assert_eq!(expand_jamo("ㅋㅋㅇㅋ"), "크크오케이");
        assert_eq!(expand_jamo("ㄱㄱㄱㄱㄱ"), "고고고");
        assert_eq!(expand_jamo("ㅂㅂ"), "바이바이");
        assert_eq!(expand_jamo("ㄳ"), "감사");
        assert_eq!(expand_jamo("가자ㄱㄱ"), "가자고고");
        assert_eq!(expand_jamo("ㅁ"), "");
        assert_eq!(expand_jamo("hello 안녕"), "hello 안녕");
    }

    /// Network test: `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn expanded_shorthand_synthesizes() {
        let text = expand_jamo("ㅋㅋㅋ ㄱㄱ ㅇㅋ");
        let mp3 = crate::edge::synthesize("ko-KR-SunHiNeural", &text, 1.0, 0.0).expect("synth");
        assert!(mp3.len() > 1000, "{} bytes", mp3.len());
    }
}
