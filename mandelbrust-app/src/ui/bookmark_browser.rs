use std::collections::HashSet;

use eframe::egui;

use crate::app::{BookmarkSnap, BookmarkTab, LabelFilterMode, MandelbRustApp};
use crate::app_state::AppScreen;
use crate::bookmarks;

const CYAN: egui::Color32 = egui::Color32::from_rgb(80, 200, 255);

impl MandelbRustApp {
    pub(crate) fn draw_bookmark_browser(&mut self, ctx: &egui::Context) {
        let leaf_labels = bookmarks::collect_leaf_labels(self.bookmark_store.bookmarks());
        let all_labels = bookmarks::collect_all_labels(self.bookmark_store.bookmarks());
        let label_tree = bookmarks::build_label_tree(&all_labels);

        let snapshot: Vec<BookmarkSnap> = self
            .bookmark_store
            .bookmarks()
            .iter()
            .enumerate()
            .map(|(i, bm)| BookmarkSnap {
                index: i,
                id: self.bookmark_store.bookmark_id(i).to_string(),
                name: bm.name.clone(),
                summary: bm.summary(),
                mode: bm.mode.clone(),
                labels: bm.labels.clone(),
                thumbnail_png: bm.thumbnail_png.clone(),
            })
            .collect();

        let query = self.bookmark_search.clone();
        let selected = self.selected_labels.clone();
        let filter_mode = self.label_filter_mode;
        let tab = self.bookmark_tab;
        let fav_only = self.favorites_only;

        let mut jump_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut rename_action: Option<(usize, String)> = None;
        let mut toggle_fav_idx: Option<usize> = None;
        let mut go_back = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(12, 12, 14))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("\u{2190} Back").clicked() {
                        go_back = true;
                    }
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new("Bookmarks").color(CYAN));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_open = self.browser_selected_bookmark.is_some();
                        if ui
                            .add_enabled(can_open, egui::Button::new("Open"))
                            .clicked()
                        {
                            jump_idx = self.browser_selected_bookmark;
                        }
                    });
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bookmark_tab, BookmarkTab::All, "All");
                    let fav_label = if self.favorites_only {
                        egui::RichText::new("\u{2605} Fav").strong()
                    } else {
                        egui::RichText::new("\u{2606} Fav")
                    };
                    if ui
                        .selectable_label(self.favorites_only, fav_label)
                        .clicked()
                    {
                        self.favorites_only = !self.favorites_only;
                    }
                    ui.selectable_value(
                        &mut self.bookmark_tab,
                        BookmarkTab::Mandelbrot,
                        "Mandelbrot",
                    );
                    ui.selectable_value(&mut self.bookmark_tab, BookmarkTab::Julia, "Julia");
                });

                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.bookmark_search)
                            .desired_width(200.0),
                    );
                    if ui.small_button("A-Z").clicked() {
                        self.bookmark_store.sort_by_name();
                    }
                    if ui.small_button("Date").clicked() {
                        self.bookmark_store.sort_by_date();
                    }
                });

                ui.separator();

                if !leaf_labels.is_empty() {
                    egui::CollapsingHeader::new("Label filter")
                        .default_open(false)
                        .show(ui, |ui| {
                            self.draw_label_filter_ui(ui, &leaf_labels, &label_tree);
                        });
                    ui.separator();
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let filtered: Vec<usize> = snapshot
                        .iter()
                        .filter(|s| {
                            passes_bookmark_filter(
                                &s.name,
                                &s.mode,
                                &s.labels,
                                tab,
                                fav_only,
                                &query,
                                filter_mode,
                                &selected,
                            )
                        })
                        .map(|s| s.index)
                        .collect();

                    if !filtered.is_empty() {
                        self.draw_bookmark_grid(
                            ui,
                            ctx,
                            &snapshot,
                            &filtered,
                            "browser_grid",
                            &mut jump_idx,
                            &mut delete_idx,
                            &mut rename_action,
                            &mut toggle_fav_idx,
                            true,
                        );
                    } else if snapshot.is_empty() {
                        ui.add_space(40.0);
                        ui.vertical_centered(|ui| {
                            ui.weak("No bookmarks yet. Save some from the fractal explorer!");
                        });
                    } else {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.weak("No bookmarks match the current filter.");
                        });
                    }
                });
            });

        if go_back {
            self.screen = AppScreen::MainMenu;
            self.browser_selected_bookmark = None;
            return;
        }

        if let Some((idx, new_name)) = rename_action {
            self.bookmark_store.rename(idx, new_name);
        }
        if let Some(idx) = toggle_fav_idx {
            self.bookmark_store.toggle_label(idx, "Favorites");
        }
        if let Some(idx) = jump_idx {
            let bm = self.bookmark_store.bookmarks()[idx].clone();
            self.last_jumped_bookmark_idx = Some(idx);
            self.jump_to_bookmark(&bm);
            self.screen = AppScreen::FractalExplorer;
            self.browser_selected_bookmark = None;
        }
        if let Some(idx) = delete_idx {
            let del_id = self.bookmark_store.bookmark_id(idx).to_string();
            self.thumbnail_cache.remove(&del_id);
            self.failed_thumbnails.remove(&del_id);
            self.bookmark_store.remove(idx);
            if self.browser_selected_bookmark == Some(idx) {
                self.browser_selected_bookmark = None;
            }
        }
    }

    /// Shared label-filter UI used by both the overlay bookmark window
    /// and the full-window bookmark browser.
    pub(crate) fn draw_label_filter_ui(
        &mut self,
        ui: &mut egui::Ui,
        leaf_labels: &[String],
        label_tree: &[bookmarks::LabelNode],
    ) {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            ui.selectable_value(&mut self.label_filter_mode, LabelFilterMode::Off, "Off");
            ui.selectable_value(
                &mut self.label_filter_mode,
                LabelFilterMode::Whitelist,
                "Whitelist",
            );
            ui.selectable_value(
                &mut self.label_filter_mode,
                LabelFilterMode::Blacklist,
                "Blacklist",
            );
        });
        if self.label_filter_mode != LabelFilterMode::Off {
            if ui.small_button("Clear selection").clicked() {
                self.selected_labels.clear();
            }
            if leaf_labels.iter().any(|l| l == "Favorites") {
                let mut fav = self.selected_labels.contains("Favorites");
                if ui
                    .checkbox(&mut fav, egui::RichText::new("* Favorites").strong())
                    .changed()
                {
                    if fav {
                        self.selected_labels.insert("Favorites".to_string());
                    } else {
                        self.selected_labels.remove("Favorites");
                    }
                }
                ui.separator();
            }
            for node in label_tree {
                if node.name == "Favorites" {
                    continue;
                }
                self.draw_label_tree_node(ui, node);
            }
        }
    }
}

pub(crate) fn passes_bookmark_filter(
    name: &str,
    mode: &str,
    labels: &[String],
    tab: BookmarkTab,
    fav_only: bool,
    query: &str,
    filter_mode: LabelFilterMode,
    selected: &HashSet<String>,
) -> bool {
    let tab_ok = match tab {
        BookmarkTab::All => true,
        BookmarkTab::Mandelbrot => mode == "Mandelbrot",
        BookmarkTab::Julia => mode == "Julia",
    };
    let fav_ok = !fav_only || labels.iter().any(|l| l == "Favorites");
    let q_ok = query.is_empty()
        || name.to_lowercase().contains(&query.to_lowercase())
        || labels
            .iter()
            .any(|l| l.to_lowercase().contains(&query.to_lowercase()));
    let l_ok = match filter_mode {
        LabelFilterMode::Off => true,
        _ => {
            if selected.is_empty() {
                true
            } else {
                let has_match = labels.iter().any(|l| {
                    selected
                        .iter()
                        .any(|s| l == s || l.starts_with(&format!("{s}/")))
                });
                match filter_mode {
                    LabelFilterMode::Whitelist => has_match,
                    LabelFilterMode::Blacklist => !has_match,
                    _ => true,
                }
            }
        }
    };
    tab_ok && fav_ok && q_ok && l_ok
}
