use std::collections::HashSet;

use eframe::egui;

use crate::app::{BookmarkSnap, BookmarkTab, FractalMode, MandelbRustApp};
use crate::bookmarks::{self, Bookmark};
use crate::ui::bookmark_browser::passes_bookmark_filter;

impl MandelbRustApp {
    pub(crate) fn capture_bookmark(&self, name: String, labels: Vec<String>) -> Bookmark {
        let thumbnail_png = self
            .current_iterations
            .as_ref()
            .and_then(|iter_buf| {
                let buf = self.colorize_current(iter_buf, self.current_aa.as_ref());
                bookmarks::encode_thumbnail(&buf.pixels, buf.width, buf.height)
            })
            .unwrap_or_default();

        Bookmark {
            name,
            mode: self.mode.label().to_string(),
            center_re: self.viewport.center_dd.re.hi,
            center_im: self.viewport.center_dd.im.hi,
            center_re_lo: self.viewport.center_dd.re.lo,
            center_im_lo: self.viewport.center_dd.im.lo,
            scale: self.viewport.scale,
            max_iterations: self.params.max_iterations,
            escape_radius: self.params.escape_radius,
            palette_index: self.display_color.palette_index,
            smooth_coloring: self.display_color.smooth_coloring,
            display_color: Some(self.display_color.clone()),
            aa_level: self.aa_level,
            julia_c_re: self.julia_c.re,
            julia_c_im: self.julia_c.im,
            labels,
            notes: String::new(),
            created_at: bookmarks::now_timestamp(),
            thumbnail_png,
            thumbnail_file: String::new(),
        }
    }

    pub(crate) fn open_save_new_dialog(&mut self) {
        self.active_dialog = crate::app::ActiveDialog::SaveBookmark;
        self.save_bookmark_name.clear();
        self.save_bookmark_new_label.clear();
        let defaults = bookmarks::suggest_default_labels(
            self.mode.label(),
            self.viewport.scale,
            self.params.max_iterations,
        );
        self.save_bookmark_labels_selected = defaults.into_iter().collect();
    }

    pub(crate) fn update_bookmark(&mut self, idx: usize) {
        if idx >= self.bookmark_store.bookmarks().len() {
            return;
        }

        let thumbnail_png = self
            .current_iterations
            .as_ref()
            .and_then(|iter_buf| {
                let buf = self.colorize_current(iter_buf, self.current_aa.as_ref());
                bookmarks::encode_thumbnail(&buf.pixels, buf.width, buf.height)
            })
            .unwrap_or_default();

        let bm_id = self.bookmark_store.bookmark_id(idx).to_string();
        self.thumbnail_cache.remove(&bm_id);

        self.bookmark_store.update_viewport(idx, |bm| {
            bm.mode = self.mode.label().to_string();
            bm.center_re = self.viewport.center_dd.re.hi;
            bm.center_im = self.viewport.center_dd.im.hi;
            bm.center_re_lo = self.viewport.center_dd.re.lo;
            bm.center_im_lo = self.viewport.center_dd.im.lo;
            bm.scale = self.viewport.scale;
            bm.max_iterations = self.params.max_iterations;
            bm.escape_radius = self.params.escape_radius;
            bm.palette_index = self.display_color.palette_index;
            bm.smooth_coloring = self.display_color.smooth_coloring;
            bm.display_color = Some(self.display_color.clone());
            bm.aa_level = self.aa_level;
            bm.julia_c_re = self.julia_c.re;
            bm.julia_c_im = self.julia_c.im;
            bm.thumbnail_png = thumbnail_png;
        });

        tracing::info!(
            "Updated bookmark: {}",
            self.bookmark_store.bookmarks()[idx].name
        );
    }

    pub(crate) fn get_thumbnail(
        &mut self,
        bm_id: &str,
        base64_png: &str,
        ctx: &egui::Context,
    ) -> Option<&egui::TextureHandle> {
        if base64_png.is_empty() || self.failed_thumbnails.contains(bm_id) {
            return None;
        }
        let id_key = bm_id.to_string();
        if !self.thumbnail_cache.contains_key(&id_key) {
            if let Some((pixels, w, h)) = bookmarks::decode_thumbnail(base64_png) {
                while self.thumbnail_cache.len() >= crate::app::THUMBNAIL_CACHE_CAPACITY {
                    let evict_key = match self.thumbnail_cache.keys().next().cloned() {
                        Some(k) => k,
                        None => break,
                    };
                    self.thumbnail_cache.remove(&evict_key);
                }
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                let handle = ctx.load_texture(
                    format!("thumb_{bm_id}"),
                    image,
                    egui::TextureOptions::LINEAR,
                );
                self.thumbnail_cache.insert(id_key, handle);
            } else {
                self.failed_thumbnails.insert(id_key);
                return None;
            }
        }
        self.thumbnail_cache.get(bm_id)
    }

    pub(crate) fn jump_to_bookmark(&mut self, bm: &Bookmark) {
        self.mode = match bm.mode.as_str() {
            "Julia" => FractalMode::Julia,
            _ => FractalMode::Mandelbrot,
        };
        self.julia_c = mandelbrust_core::Complex::new(bm.julia_c_re, bm.julia_c_im);
        self.params.max_iterations = bm.max_iterations;
        self.params.set_escape_radius(bm.escape_radius);
        if let Some(ref dc) = bm.display_color {
            self.display_color = dc.clone();
            if self.display_color.palette_index >= self.palettes.len() {
                self.display_color.palette_index = 0;
            }
        } else {
            if bm.palette_index < self.palettes.len() {
                self.display_color.palette_index = bm.palette_index;
            }
            self.display_color.smooth_coloring = bm.smooth_coloring;
        }
        self.aa_level = bm.aa_level;
        self.bump_minimap_revision();

        self.push_history();
        let center_dd = mandelbrust_core::ComplexDD::new(
            mandelbrust_core::DoubleDouble::new(bm.center_re, bm.center_re_lo),
            mandelbrust_core::DoubleDouble::new(bm.center_im, bm.center_im_lo),
        );
        self.viewport = mandelbrust_core::Viewport::new_dd(
            center_dd,
            bm.scale,
            self.panel_size[0],
            self.panel_size[1],
        )
        .unwrap_or_else(|_| {
            mandelbrust_core::Viewport::default_mandelbrot(self.panel_size[0], self.panel_size[1])
        });
        self.needs_render = true;
        tracing::info!("Jumped to bookmark: {}", bm.name);
    }

    pub(crate) fn show_update_or_save_choice(&mut self, ctx: &egui::Context) {
        if self.active_dialog != crate::app::ActiveDialog::UpdateOrSave {
            return;
        }

        let bm_name = self
            .last_jumped_bookmark_idx
            .and_then(|idx| {
                self.bookmark_store
                    .bookmarks()
                    .get(idx)
                    .map(|bm| bm.name.clone())
            })
            .unwrap_or_else(|| "bookmark".to_string());

        let mut open = true;
        let mut do_update = false;
        let mut do_save_new = false;

        egui::Window::new("Save Bookmark")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "You navigated from bookmark \"{bm_name}\".\nWhat would you like to do?"
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Update existing").clicked() {
                        do_update = true;
                    }
                    if ui.button("Save as new").clicked() {
                        do_save_new = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.active_dialog = crate::app::ActiveDialog::None;
                    }
                });
            });

        if do_update {
            if let Some(idx) = self.last_jumped_bookmark_idx {
                self.update_bookmark(idx);
            }
            self.active_dialog = crate::app::ActiveDialog::None;
        } else if do_save_new {
            self.active_dialog = crate::app::ActiveDialog::None;
            self.open_save_new_dialog();
        }

        if !open {
            self.active_dialog = crate::app::ActiveDialog::None;
        }
    }

    pub(crate) fn show_save_bookmark_dialog(&mut self, ctx: &egui::Context) {
        if self.active_dialog != crate::app::ActiveDialog::SaveBookmark {
            return;
        }

        let all_known: Vec<String> = {
            let mut set: HashSet<String> = HashSet::new();
            for bm in self.bookmark_store.bookmarks() {
                for l in &bm.labels {
                    set.insert(l.clone());
                }
            }
            for l in &self.save_bookmark_labels_selected {
                set.insert(l.clone());
            }
            let mut v: Vec<String> = set.into_iter().collect();
            v.sort_by(|a, b| {
                let fa = a == "Favorites";
                let fb = b == "Favorites";
                fb.cmp(&fa)
                    .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
            });
            v
        };

        let auto_name = self.bookmark_store.next_auto_name(self.mode.label());

        let mut open = true;
        let mut do_save = false;

        egui::Window::new("Save Bookmark")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.save_bookmark_name);
                });
                if self.save_bookmark_name.trim().is_empty() {
                    ui.weak(format!("Leave empty to auto-name: {auto_name}"));
                }

                ui.add_space(4.0);
                ui.label("Labels:");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for label in &all_known {
                        let selected = self.save_bookmark_labels_selected.contains(label);
                        let text = if selected {
                            egui::RichText::new(format!("[x] {label}"))
                                .color(egui::Color32::WHITE)
                                .background_color(egui::Color32::from_rgb(50, 100, 170))
                        } else {
                            egui::RichText::new(label)
                                .color(egui::Color32::from_gray(180))
                                .background_color(egui::Color32::from_gray(50))
                        };
                        if ui.add(egui::Button::new(text).small()).clicked() {
                            if selected {
                                self.save_bookmark_labels_selected.remove(label);
                            } else {
                                self.save_bookmark_labels_selected.insert(label.clone());
                            }
                        }
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("New label:");
                    let resp = ui.text_edit_singleline(&mut self.save_bookmark_new_label);
                    if (resp.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter)))
                        || ui.small_button("+").clicked()
                    {
                        let new = self.save_bookmark_new_label.trim().to_string();
                        if !new.is_empty() {
                            self.save_bookmark_labels_selected.insert(new);
                            self.save_bookmark_new_label.clear();
                        }
                    }
                });
                ui.weak("Use / for nesting (e.g. Spirals/Double)");

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        do_save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.active_dialog = crate::app::ActiveDialog::None;
                    }
                });
            });

        if do_save {
            let name = {
                let trimmed = self.save_bookmark_name.trim().to_string();
                if trimmed.is_empty() {
                    auto_name
                } else {
                    trimmed
                }
            };
            let labels: Vec<String> = self.save_bookmark_labels_selected.iter().cloned().collect();
            let bm = self.capture_bookmark(name, labels);
            self.bookmark_store.add(bm);
            self.active_dialog = crate::app::ActiveDialog::None;
        }

        if !open {
            self.active_dialog = crate::app::ActiveDialog::None;
        }
    }

    pub(crate) fn show_bookmark_window(&mut self, ctx: &egui::Context) {
        if !self.show_bookmarks || !self.show_hud {
            return;
        }

        let all_labels = bookmarks::collect_all_labels(self.bookmark_store.bookmarks());
        let label_tree = bookmarks::build_label_tree(&all_labels);
        let leaf_labels = bookmarks::collect_leaf_labels(self.bookmark_store.bookmarks());
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

        let mut open = true;
        let mut jump_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut rename_action: Option<(usize, String)> = None;
        let mut toggle_fav_idx: Option<usize> = None;

        egui::Window::new("Bookmarks")
            .open(&mut open)
            .resizable(true)
            .default_width(520.0)
            .default_height(480.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 10, 210)),
            )
            .show(ctx, |ui| {
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
                            .desired_width(160.0),
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
                            "main_grid",
                            &mut jump_idx,
                            &mut delete_idx,
                            &mut rename_action,
                            &mut toggle_fav_idx,
                            false,
                        );
                    } else if snapshot.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.weak("No bookmarks yet. Press S to save one.");
                        });
                    } else {
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| {
                            ui.weak("No bookmarks match the current filter.");
                        });
                    }
                });
            });

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
        }
        if let Some(idx) = delete_idx {
            let del_id = self.bookmark_store.bookmark_id(idx).to_string();
            self.thumbnail_cache.remove(&del_id);
            self.failed_thumbnails.remove(&del_id);
            self.bookmark_store.remove(idx);
            if self.last_jumped_bookmark_idx == Some(idx) {
                self.last_jumped_bookmark_idx = None;
            } else if self.last_jumped_bookmark_idx.is_some_and(|last| last > idx) {
                self.last_jumped_bookmark_idx =
                    self.last_jumped_bookmark_idx.map(|last| last - 1);
            }
        }

        if !open {
            self.show_bookmarks = false;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_bookmark_grid(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        snapshot: &[BookmarkSnap],
        indices: &[usize],
        grid_id: &str,
        jump_idx: &mut Option<usize>,
        delete_idx: &mut Option<usize>,
        rename_action: &mut Option<(usize, String)>,
        toggle_fav_idx: &mut Option<usize>,
        select_mode: bool,
    ) {
        let card_width = 150.0_f32;
        let thumb_height = 84.0_f32;
        let spacing = 8.0_f32;
        let available_width = ui.available_width();
        let cols = ((available_width + spacing) / (card_width + spacing))
            .floor()
            .max(1.0) as usize;

        egui::Grid::new(ui.id().with(grid_id))
            .num_columns(cols)
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (ci, &idx) in indices.iter().enumerate() {
                    let Some(snap) = snapshot.iter().find(|s| s.index == idx) else {
                        continue;
                    };
                    let i = snap.index;
                    let name = &snap.name;
                    let labels = &snap.labels;
                    let thumb_png = &snap.thumbnail_png;
                    let bm_id = &snap.id;

                    let card_resp = ui.vertical(|ui| {
                        ui.set_width(card_width);
                        let thumb_tex = self.get_thumbnail(bm_id, thumb_png, ctx);
                        let thumb_rect = ui.allocate_exact_size(
                            egui::vec2(card_width, thumb_height),
                            egui::Sense::click(),
                        );

                        if let Some(tex) = thumb_tex {
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            );
                            ui.painter().image(
                                tex.id(),
                                thumb_rect.0,
                                uv,
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().rect_filled(
                                thumb_rect.0,
                                4.0,
                                egui::Color32::from_gray(40),
                            );
                            ui.painter().text(
                                thumb_rect.0.center(),
                                egui::Align2::CENTER_CENTER,
                                "No preview",
                                egui::FontId::proportional(10.0),
                                egui::Color32::GRAY,
                            );
                        }

                        if select_mode {
                            if thumb_rect.1.double_clicked() {
                                *jump_idx = Some(i);
                            } else if thumb_rect.1.clicked() {
                                self.browser_selected_bookmark = Some(i);
                            }
                        } else if thumb_rect.1.clicked() {
                            *jump_idx = Some(i);
                        }

                        if self.editing_bookmark == Some(i) {
                            let resp = ui.text_edit_singleline(&mut self.editing_name);
                            if resp.lost_focus()
                                || ui.input(|inp| inp.key_pressed(egui::Key::Enter))
                            {
                                let new_name = self.editing_name.trim().to_string();
                                if !new_name.is_empty() {
                                    *rename_action = Some((i, new_name));
                                }
                                self.editing_bookmark = None;
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.set_width(card_width);
                                let name_resp = ui
                                    .add(
                                        egui::Label::new(
                                            egui::RichText::new(name).strong().size(11.0),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(name);
                                if select_mode {
                                    if name_resp.double_clicked() {
                                        *jump_idx = Some(i);
                                    } else if name_resp.clicked() {
                                        self.browser_selected_bookmark = Some(i);
                                    }
                                } else if name_resp.clicked() {
                                    *jump_idx = Some(i);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 1.0;
                                        if ui.small_button("\u{1f5d1}").clicked() {
                                            *delete_idx = Some(i);
                                        }
                                        if ui.small_button("\u{270f}").clicked() {
                                            self.editing_bookmark = Some(i);
                                            self.editing_name = name.clone();
                                        }
                                        let is_fav =
                                            labels.iter().any(|l| l == "Favorites");
                                        let star =
                                            if is_fav { "\u{2605}" } else { "\u{2606}" };
                                        if ui
                                            .small_button(star)
                                            .on_hover_text(if is_fav {
                                                "Remove from Favorites"
                                            } else {
                                                "Add to Favorites"
                                            })
                                            .clicked()
                                        {
                                            *toggle_fav_idx = Some(i);
                                        }
                                    },
                                );
                            });
                        }

                        if !labels.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(3.0, 2.0);
                                for label in labels {
                                    let short =
                                        label.split('/').next_back().unwrap_or(label);
                                    ui.label(
                                        egui::RichText::new(short)
                                            .size(9.0)
                                            .color(egui::Color32::from_rgb(140, 180, 220))
                                            .background_color(
                                                egui::Color32::from_black_alpha(80),
                                            ),
                                    );
                                }
                            });
                        }
                    });

                    if select_mode && self.browser_selected_bookmark == Some(i) {
                        let card_rect = card_resp.response.rect;
                        ui.painter().rect_stroke(
                            card_rect,
                            4.0,
                            egui::Stroke::new(
                                2.0,
                                egui::Color32::from_rgb(80, 200, 255),
                            ),
                            egui::StrokeKind::Outside,
                        );
                    }

                    if (ci + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    pub(crate) fn draw_label_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        node: &bookmarks::LabelNode,
    ) {
        let is_selected = self.selected_labels.contains(&node.full_path);

        if node.children.is_empty() {
            let mut checked = is_selected;
            if ui.checkbox(&mut checked, &node.name).changed() {
                if checked {
                    self.selected_labels.insert(node.full_path.clone());
                } else {
                    self.selected_labels.remove(&node.full_path);
                }
            }
        } else {
            let mut checked = is_selected;
            ui.horizontal(|ui| {
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        self.selected_labels.insert(node.full_path.clone());
                    } else {
                        self.selected_labels.remove(&node.full_path);
                    }
                }
            });
            ui.indent(egui::Id::new(&node.full_path), |ui| {
                egui::CollapsingHeader::new(&node.name)
                    .default_open(true)
                    .show(ui, |ui| {
                        for child in &node.children {
                            self.draw_label_tree_node(ui, child);
                        }
                    });
            });
        }
    }
}
