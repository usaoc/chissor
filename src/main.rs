/* chissor --- GUI application for Chinese word segmentation

Copyright (C) 2024 Wing Hei Chan

This program is free software; you can redistribute it and/or modify
it under the terms of the Expat License.

You should have received a copy of the Expat License along with this
program.  If not, see <https://spdx.org/licenses/MIT.html>.  */

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![warn(clippy::pedantic)]
use eframe::egui;
use rust_i18n::t;
use std::io::Write;
use std::{error, fs, io, path, process, result};
use {jieba_rs as jieba, rust_i18n as i18n};

i18n::i18n!("locales");

const WINDOW_TITLE: &str = "Chissor";
fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        default_theme: eframe::Theme::Light,
        ..Default::default()
    };
    if let Err(err) =
        eframe::run_native(WINDOW_TITLE, options, Box::new(|cc| Box::new(App::new(cc))))
    {
        eprintln!("Initialization failed: {err}");
        process::exit(1);
    }
}

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Default)]
struct App {
    locale: Locale,
    dicts: Dicts,
    word: String,
    freq: String,
    tag: String,
    input: String,
    output: String,
    separator: String,
    use_hmm: bool,
    error_windows: ErrorWindows,
}

#[derive(Default)]
enum Locale {
    #[default]
    En,
}

// Invariants:
//  - `idx` must be between `0..dicts.len()`;
//  - `dicts` must be nonempty.
struct Dicts {
    idx: usize,
    dicts: Vec<Dict>,
}

struct Dict {
    name: String,
    jieba: jieba::Jieba,
}

#[derive(Default)]
struct ErrorWindows {
    count: u32,
    windows: Vec<ErrorWindow>,
}

struct ErrorWindow {
    id: egui::Id,
    open: bool,
    title: String,
    content: String,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        cc.egui_ctx.set_fonts(make_cjk_font_defs());
        Self::default()
    }
}

impl Default for Dicts {
    fn default() -> Self {
        Dicts {
            idx: 0,
            dicts: vec![
                make_dict_static("Default", include_bytes!("../dicts/dict.txt")),
                make_dict_static("Default (small)", include_bytes!("../dicts/dict.txt.small")),
                make_dict_static("Default (big)", include_bytes!("../dicts/dict.txt.big")),
            ],
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        i18n::set_locale(self.locale.to_locale());
        self.error_windows.show_all(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::SidePanel::left("dict panel").show_inside(ui, |ui| {
                self.show_dict_panel(ui);
            });
            egui::TopBottomPanel::top("input area")
                .exact_height(ui.available_height() / 2.0)
                .show_inside(ui, |ui| {
                    self.show_input_area(ui);
                });
            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.show_output_area(ui);
            });
        });
    }
}

impl App {
    fn show_dict_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(t!("new-dict.text"))
                .on_hover_text(t!("new-dict.hover"))
                .clicked()
            {
                self.new_dict();
            }
            if ui
                .button(t!("load-dict.text"))
                .on_hover_text(t!("load-dict.hover"))
                .clicked()
            {
                self.load_dict();
            }
            if ui
                .button(t!("add-word.text"))
                .on_hover_text(t!("add-word.hover"))
                .clicked()
            {
                self.add_word();
            }
            if ui
                .add_enabled(
                    self.dicts.can_remove_dict(),
                    egui::Button::new(t!("remove-dict.text")),
                )
                .on_hover_text(t!("remove-dict.hover"))
                .clicked()
            {
                self.dicts.remove_dict();
            }
        });
        ui.add(make_field(&mut self.word, t!("word.text")))
            .on_hover_text(t!("word.hover"));
        ui.horizontal(|ui| {
            let width = f32::min(ui.spacing().text_edit_width, ui.available_width());
            // Default margin is `4.0`, so subtract `4.0 * 2` == `8.0`.
            let width = (width / 2.0) - 8.0;
            ui.add(make_field(&mut self.freq, t!("word.freq.text")).desired_width(width))
                .on_hover_text(t!("word.freq.hover"));
            ui.add(make_field(&mut self.tag, t!("word.tag.text")).desired_width(width))
                .on_hover_text(t!("word.tag.hover"));
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.dicts.show_all(ui);
        });
    }

    fn show_input_area(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(t!("import.text"))
            .on_hover_text(t!("import.hover"))
            .clicked()
        {
            self.import();
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(make_editor(&mut self.input, t!("input.hint")));
        });
    }

    fn show_output_area(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(t!("export.text"))
                .on_hover_text(t!("export.hover"))
                .clicked()
            {
                self.export();
            }
            if ui
                .button(t!("segment.text"))
                .on_hover_text(t!("segment.hover"))
                .clicked()
            {
                self.segment();
            }
            if ui
                .button(t!("segment-granular.text"))
                .on_hover_text(t!("segment-granular.hover"))
                .clicked()
            {
                self.segment_granular();
            }
            if ui
                .button(t!("search.text"))
                .on_hover_text(t!("search.hover"))
                .clicked()
            {
                self.search();
            }
            if ui
                .button(t!("tag.text"))
                .on_hover_text(t!("tag.hover"))
                .clicked()
            {
                self.tag();
            }
            ui.add(make_field(&mut self.separator, t!("separator.text")))
                .on_hover_text(t!("separator.hover"));
            ui.checkbox(&mut self.use_hmm, t!("use-hmm.text"))
                .on_hover_text(t!("use-hmm.hover"));
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(make_editor(&mut &*self.output, t!("output.hint")));
        });
    }

    fn new_dict(&mut self) {
        if let Err(err) = with_pick_file(|path| {
            let name = String::from(
                path.file_name()
                    .expect("cannot be `None`; must have picked a regular file")
                    .to_string_lossy(),
            );
            let file = fs::File::open(path)?;
            self.dicts.new_dict(name, &mut io::BufReader::new(file))?;
            Ok(())
        }) {
            self.error_windows.add(&t!("new-dict.what"), err);
        }
    }

    fn load_dict(&mut self) {
        if let Err(err) = with_pick_file(|path| {
            let file = fs::File::open(path)?;
            self.dicts.load_dict(&mut io::BufReader::new(file))?;
            Ok(())
        }) {
            self.error_windows.add(&t!("load-dict.what"), err);
        }
    }

    fn add_word(&mut self) {
        if let Err(err) = self.dicts.add_word(&self.word, &self.freq, &self.tag) {
            self.error_windows.add(&t!("add-word.what"), err);
        }
    }

    fn import(&mut self) {
        if let Err(err) = with_pick_file(|path| {
            self.input = String::from(fs::read_to_string(path)?.trim());
            Ok(())
        }) {
            self.error_windows.add(&t!("import.what"), err);
        }
    }

    fn export(&mut self) {
        if let Err(err) = with_save_file(|path| {
            let mut buf = fs::File::create(path)?;
            writeln!(&mut buf, "{output}", output = self.output)?;
            Ok(())
        }) {
            self.error_windows.add(&t!("export.what"), err);
        }
    }

    fn segment(&mut self) {
        self.output = self
            .dicts
            .selected()
            .cut(&self.input, self.use_hmm)
            .join(self.get_separator());
    }

    fn segment_granular(&mut self) {
        self.output = self
            .dicts
            .selected()
            .cut_for_search(&self.input, self.use_hmm)
            .join(self.get_separator());
    }

    fn search(&mut self) {
        self.output = self
            .dicts
            .selected()
            .cut_all(&self.input)
            .join(self.get_separator());
    }

    fn tag(&mut self) {
        self.output = self
            .dicts
            .selected()
            .tag(&self.input, self.use_hmm)
            .into_iter()
            .map(|jieba::Tag { word, tag }| format!("{word} {tag}"))
            .collect::<Vec<_>>()
            .join(self.get_separator());
    }

    fn get_separator(&self) -> &str {
        let sep = &self.separator;
        if sep.is_empty() {
            "\n"
        } else {
            sep
        }
    }
}

impl Locale {
    fn to_locale(&self) -> &'static str {
        match self {
            Locale::En => "en",
        }
    }
}

impl Dicts {
    fn new_dict(&mut self, name: impl Into<String>, dict: &mut impl io::BufRead) -> Result<()> {
        let jieba = jieba::Jieba::with_dict(dict)?;
        self.dicts.push(Dict {
            name: name.into(),
            jieba,
        });
        Ok(())
    }

    fn load_dict(&mut self, dict: &mut impl io::BufRead) -> Result<()> {
        self.selected().load_dict(dict)?;
        Ok(())
    }

    fn add_word(&mut self, word: &str, freq: &str, tag: &str) -> Result<()> {
        let freq = if freq.is_empty() {
            None
        } else {
            Some(freq.parse()?)
        };
        let tag = if tag.is_empty() { None } else { Some(tag) };
        self.selected().add_word(word, freq, tag);
        Ok(())
    }

    fn can_remove_dict(&self) -> bool {
        self.dicts.len() != 1
    }

    fn remove_dict(&mut self) {
        if !self.can_remove_dict() {
            unreachable!("must not trigger this action for the only dictionary");
        }
        self.dicts.remove(self.idx);
        if self.idx == self.dicts.len() {
            self.idx -= 1;
        }
    }

    fn show_all(&mut self, ui: &mut egui::Ui) {
        for idx in 0..self.dicts.len() {
            ui.radio_value(&mut self.idx, idx, &self.dicts[idx].name);
        }
    }

    fn selected(&mut self) -> &mut jieba::Jieba {
        &mut self
            .dicts
            .get_mut(self.idx)
            .expect("cannot be `None`; must have maintained the invariants")
            .jieba
    }
}

impl ErrorWindows {
    #[allow(clippy::needless_pass_by_value)]
    fn add(&mut self, what: &str, err: Box<dyn error::Error>) {
        self.windows.push(ErrorWindow {
            id: egui::Id::new(self.count),
            open: true,
            title: String::from(t!("error-window.title", what = what)),
            content: err.to_string(),
        });
        self.count += 1;
    }

    fn cleanup(&mut self) {
        self.windows.retain(|ErrorWindow { open, .. }| *open);
    }

    fn show_all(&mut self, ctx: &egui::Context) {
        self.cleanup();
        for win in &mut self.windows {
            win.show(ctx);
        }
    }
}

impl ErrorWindow {
    fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new(&self.title)
            .id(egui::Id::new(self.id))
            .resizable(false)
            .collapsible(false)
            .open(&mut self.open)
            .show(ctx, |ui| {
                ui.label(&self.content);
            });
    }
}

const FONT_NAME: &str = "noto-sans-cjk";
fn make_cjk_font_defs() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        String::from(FONT_NAME),
        egui::FontData::from_static(include_bytes!("../fonts/NotoSansCJKsc-Regular.otf")),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .expect("cannot be `None`; must have inserted the font")
        .insert(0, String::from(FONT_NAME));
    fonts
}

fn make_dict_static(name: &'static str, bytes: &'static [u8]) -> Dict {
    Dict {
        name: String::from(name),
        jieba: jieba::Jieba::with_dict(&mut io::BufReader::new(bytes))
            .expect("cannot be `Err(_)`; must have provided a valid static dict"),
    }
}

fn make_field(
    text: &mut impl egui::widgets::TextBuffer,
    hint: impl Into<egui::WidgetText>,
) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text).hint_text(hint)
}

fn make_editor(
    text: &mut impl egui::widgets::TextBuffer,
    hint: impl Into<egui::WidgetText>,
) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text)
        .hint_text(hint)
        .desired_rows(10)
        .desired_width(f32::INFINITY)
}

fn with_pick_file(func: impl FnOnce(path::PathBuf) -> Result<()>) -> Result<()> {
    match rfd::FileDialog::new().pick_file() {
        Some(path) => func(path),
        None => Ok(()),
    }
}

fn with_save_file(func: impl FnOnce(path::PathBuf) -> Result<()>) -> Result<()> {
    match rfd::FileDialog::new().save_file() {
        Some(path) => func(path),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app() {
        let mut app = App {
            input: String::from("分词测试案例"),
            ..Default::default()
        };

        let segment_result = vec!["分词", "测试", "案例"];
        let segment_granular_result = vec!["分词", "测试", "案例"];
        let search_result = vec!["分", "分词", "词", "测", "测试", "试", "案", "案例", "例"];
        let tag_result = vec!["分词 n", "测试 vn", "案例 n"];

        assert_eq!(app.get_separator(), "\n");
        app.segment();
        assert_eq!(app.output, segment_result.join("\n"));
        app.segment_granular();
        assert_eq!(app.output, segment_granular_result.join("\n"));
        app.search();
        assert_eq!(app.output, search_result.join("\n"));
        app.tag();
        assert_eq!(app.output, tag_result.join("\n"));

        let separator = " / ";
        app.separator = String::from(separator);
        assert_eq!(app.get_separator(), separator);
        app.segment();
        assert_eq!(app.output, segment_result.join(separator));
        app.segment_granular();
        assert_eq!(app.output, segment_granular_result.join(separator));
        app.search();
        assert_eq!(app.output, search_result.join(separator));
        app.tag();
        assert_eq!(app.output, tag_result.join(separator));
    }

    #[test]
    fn dicts() {
        fn check_invariant(dicts: &Dicts) {
            assert!((0..dicts.dicts.len()).contains(&dicts.idx));
            assert!(!dicts.dicts.is_empty());
        }

        fn with_dict<T>(strs: Vec<&str>, func: impl FnOnce(&mut io::BufReader<&[u8]>) -> T) -> T {
            let mut str = strs.join("\n");
            str.push('\n');
            func(&mut io::BufReader::new(str.as_bytes()))
        }

        let mut dicts = Dicts::default();
        check_invariant(&dicts);

        assert!(with_dict(vec!["甲", "乙 20", "丙 40 m"], |buf| {
            dicts.new_dict("example", buf).is_ok()
        }));
        check_invariant(&dicts);

        assert!(with_dict(vec!["天", "地 20", "人 40 m"], |buf| {
            dicts.load_dict(buf).is_ok()
        }));

        assert!(dicts.add_word("一", "", "").is_ok());
        assert!(dicts.add_word("二", "20", "").is_ok());
        assert!(dicts.add_word("三", "40", "m").is_ok());
        assert!(dicts.add_word("错误", "not a frequency", "").is_err());

        let end = dicts.dicts.len() - 1;
        dicts.idx = end;
        for _ in 0..end {
            assert!(dicts.can_remove_dict());
            dicts.remove_dict();
            check_invariant(&dicts);
        }
        assert!(!dicts.can_remove_dict());
    }

    #[test]
    fn error_windows() {
        let mut error_windows = ErrorWindows::default();
        error_windows.add("example", Box::from("one"));
        error_windows.add("example", Box::from("two"));
        error_windows.add("example", Box::from("three"));
        (&mut error_windows.windows).into_iter().for_each(|win| {
            win.open = false;
        });
        error_windows.cleanup();
        assert!(error_windows.windows.is_empty());
    }
}
