use raphael_data::{Item, Locale, Recipe, Consumable};
use raphael_sim::{Action, Settings, SimulationState};

use crate::{
    app::{SolverConfig, MinimumStats},
    config::{CrafterConfig, QualityTarget},
};

use super::{HelpText, util};

pub struct Simulator<'a> {
    settings: &'a Settings,
    initial_quality: u16,
    solver_config: SolverConfig,
    crafter_config: &'a mut CrafterConfig,
    actions: &'a [Action],
    recipe: &'a Recipe,
    item: &'a Item,
    food: Option<Consumable>,
    potion: Option<Consumable>,
    minimum_stats: &'a MinimumStats,
    locale: Locale,
}

impl<'a> Simulator<'a> {
    pub fn new(
        settings: &'a Settings,
        initial_quality: u16,
        solver_config: SolverConfig,
        crafter_config: &'a mut CrafterConfig,
        actions: &'a [Action],
        recipe: &'a Recipe,
        item: &'a Item,
        food: Option<Consumable>,
        potion: Option<Consumable>,
        minimum_stats: &'a MinimumStats,
        locale: Locale,
    ) -> Self {
        Self {
            settings,
            initial_quality,
            solver_config,
            crafter_config,
            actions,
            recipe,
            item,
            locale,
            food,
            potion,
            minimum_stats,
        }
    }
}

impl Simulator<'_> {
    fn config_changed(&self, ctx: &egui::Context) -> bool {
        ctx.data(|data| {
            match data.get_temp::<(Settings, u16, SolverConfig)>(egui::Id::new("LAST_SOLVE_PARAMS"))
            {
                Some((settings, initial_quality, solver_config)) => {
                    settings != *self.settings
                        || initial_quality != self.initial_quality
                        || solver_config != self.solver_config
                }
                None => false,
            }
        })
    }

    fn draw_simulation(&mut self, ui: &mut egui::Ui, state: &SimulationState) {
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Simulation").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_visible(
                            !self.actions.is_empty() && self.config_changed(ui.ctx()),
                            egui::Label::new(
                                egui::RichText::new(
                                    "⚠ Some parameters have changed since last solve.",
                                )
                                .small()
                                .color(ui.visuals().warn_fg_color),
                            ),
                        );
                    });
                });

                ui.separator();

                let progress_text_width = text_width(ui, "Progress");
                let quality_text_width = text_width(ui, "Quality");
                let durability_text_width = text_width(ui, "Durability");
                let cp_text_width = text_width(ui, "CP");

                let max_text_width = progress_text_width
                    .max(quality_text_width)
                    .max(durability_text_width)
                    .max(cp_text_width);

                let text_size = egui::vec2(max_text_width, ui.spacing().interact_size.y);
                let text_layout = egui::Layout::right_to_left(egui::Align::Center);

                let add_context_menu = |
                    response: &egui::Response,
                    minimum_stat: Option<u16>,
                    req_stat: u16,
                    calc_bonus: fn(u16, &[Option<Consumable>]) -> u16,
                    config_stat: u16,
                    set_config_stat: &mut dyn FnMut(u16),
                | {
                    if let Some(mut stat) = minimum_stat {
                        response.interact(egui::Sense::click()).context_menu(|ui| {
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                ui.close();
                            }
                            if ui.button(format!("Copy {stat}")).clicked() {
                                ui.ctx().copy_text(stat.to_string());
                                ui.close();
                            }
                            if stat < req_stat {
                                if ui.button(format!("Copy {req_stat}")).clicked() {
                                    ui.ctx().copy_text(req_stat.to_string());
                                    ui.close();
                                }
                                stat = req_stat;
                            }
                            // this will be incorrect if stat can't fulfill consumables,
                            // but that situation is rarely encountered in practice
                            let bonus = calc_bonus(stat, &[self.food, self.potion]);
                            let crafter_stat = stat.saturating_sub(bonus);
                            if crafter_stat != stat {
                                if ui.button(format!("Copy {crafter_stat}")).clicked() {
                                    ui.ctx().copy_text(crafter_stat.to_string());
                                    ui.close();
                                }
                            }
                            if ui.add_enabled(
                                config_stat != crafter_stat,
                                egui::Button::new(format!("Set crafter stat to {crafter_stat}")),
                            ).clicked() {
                                set_config_stat(crafter_stat);
                                ui.close();
                            }
                        });
                    }
                };

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("Progress");
                    });
                    let response = ui.add(
                        egui::ProgressBar::new(
                            state.progress as f32 / self.settings.max_progress as f32,
                        )
                        .text(progress_bar_text(
                            state.progress,
                            u32::from(self.settings.max_progress),
                            self.minimum_stats.craftsmanship,
                            self.recipe.req_craftsmanship,
                            "Craftsmanship",
                        ))
                        .corner_radius(0),
                    );
                    add_context_menu(
                        &response,
                        self.minimum_stats.craftsmanship,
                        self.recipe.req_craftsmanship,
                        raphael_data::craftsmanship_bonus,
                        self.crafter_config.active_stats().craftsmanship,
                        &mut |stat| {
                            self.crafter_config.detach_from_job();
                            self.crafter_config.active_stats_mut().craftsmanship = stat;
                        }
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("Quality");
                    });
                    let quality = u32::from(self.initial_quality) + state.quality;
                    let response = ui.add(
                        egui::ProgressBar::new(quality as f32 / self.settings.max_quality as f32)
                            .text(progress_bar_text(
                                quality,
                                u32::from(self.settings.max_quality),
                                self.minimum_stats.control,
                                self.recipe.req_control,
                                "Control",
                            ))
                            .corner_radius(0),
                    );
                    add_context_menu(
                        &response,
                        self.minimum_stats.control,
                        self.recipe.req_control,
                        raphael_data::control_bonus,
                        self.crafter_config.active_stats().control,
                        &mut |stat| {
                            self.crafter_config.detach_from_job();
                            self.crafter_config.active_stats_mut().control = stat;
                        }
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("Durability");
                    });
                    ui.add(
                        egui::ProgressBar::new(
                            state.durability as f32 / self.settings.max_durability as f32,
                        )
                        .text(progress_bar_text(
                            state.durability,
                            self.settings.max_durability,
                            None,
                            0,
                            "",
                        ))
                        .corner_radius(0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label("CP");
                    });
                    let response = ui.add(
                        egui::ProgressBar::new(state.cp as f32 / self.settings.max_cp as f32)
                            .text(progress_bar_text(
                                state.cp,
                                self.settings.max_cp,
                                None,
                                0,
                                "",
                            ))
                            .corner_radius(0),
                    );
                    add_context_menu(
                        &response,
                        self.minimum_stats.cp,
                        0,
                        raphael_data::cp_bonus,
                        self.crafter_config.active_stats().cp,
                        &mut |stat| {
                            self.crafter_config.detach_from_job();
                            self.crafter_config.active_stats_mut().cp = stat;
                        }
                    );
                });

                ui.horizontal(|ui| {
                    ui.with_layout(text_layout, |ui| {
                        ui.set_height(ui.style().spacing.interact_size.y);
                        ui.add(HelpText::new(match self.settings.adversarial {
                            true => "Calculated assuming worst possible sequence of conditions",
                            false => "Calculated assuming Normal conditon on every step",
                        }));
                        if !state.is_final(self.settings) {
                            // do nothing
                        } else if state.progress < u32::from(self.settings.max_progress) {
                            ui.label("Synthesis failed");
                        } else if self.item.always_collectable {
                            let (t1, t2, t3) = (
                                QualityTarget::CollectableT1.get_target(self.settings.max_quality),
                                QualityTarget::CollectableT2.get_target(self.settings.max_quality),
                                QualityTarget::CollectableT3.get_target(self.settings.max_quality),
                            );
                            let tier = match u32::from(self.initial_quality) + state.quality {
                                quality if quality >= u32::from(t3) => 3,
                                quality if quality >= u32::from(t2) => 2,
                                quality if quality >= u32::from(t1) => 1,
                                _ => 0,
                            };
                            ui.label(format!("Tier {} collectable", tier));
                        } else {
                            let hq = raphael_data::hq_percentage(
                                u32::from(self.initial_quality) + state.quality,
                                self.settings.max_quality,
                            )
                            .unwrap_or(0);
                            ui.label(format!("{}% HQ", hq));
                        }
                    });
                });
            });
        });
    }

    fn draw_actions(&self, ui: &mut egui::Ui, errors: &[Result<(), &str>]) {
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.set_height(30.0);
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing = egui::vec2(3.0, 8.0);
                    for (step_index, (action, error)) in
                        self.actions.iter().zip(errors.iter()).enumerate()
                    {
                        let image =
                            util::get_action_icon(*action, self.crafter_config.selected_job)
                                .fit_to_exact_size(egui::Vec2::new(30.0, 30.0))
                                .corner_radius(4.0)
                                .tint(match error {
                                    Ok(_) => egui::Color32::WHITE,
                                    Err(_) => egui::Color32::DARK_GRAY,
                                });
                        let response = ui
                            .add(image)
                            .on_hover_text(raphael_data::action_name(*action, self.locale));
                        if error.is_err() {
                            egui::Image::new(egui::include_image!(
                                "../../assets/action-icons/disabled.webp"
                            ))
                            .tint(egui::Color32::GRAY)
                            .paint_at(ui, response.rect);
                        }
                        let mut step_count_ui = ui.new_child(egui::UiBuilder::default());
                        let step_count_text = egui::RichText::new((step_index + 1).to_string())
                            .color(egui::Color32::BLACK)
                            .size(12.0);
                        let text_offset_adjust = step_count_text.text().len() as f32 * 2.5;
                        let text_offset = egui::Vec2::new(-12.5 + text_offset_adjust, 11.0);
                        for shadow_offset in [
                            egui::Vec2::new(-0.5, -0.5),
                            egui::Vec2::new(-0.5, 0.0),
                            egui::Vec2::new(-0.5, 0.5),
                            egui::Vec2::new(0.5, -0.5),
                            egui::Vec2::new(0.5, 0.0),
                            egui::Vec2::new(0.5, 0.5),
                            egui::Vec2::new(0.0, -0.5),
                            egui::Vec2::new(0.0, 0.5),
                        ] {
                            step_count_ui.put(
                                response.rect.translate(text_offset + shadow_offset),
                                egui::Label::new(step_count_text.clone()).selectable(false),
                            );
                        }
                        step_count_ui.put(
                            response.rect.translate(text_offset),
                            egui::Label::new(step_count_text.color(egui::Color32::WHITE))
                                .selectable(false),
                        );
                    }
                });
            });
        });
    }
}

impl egui::Widget for Simulator<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        let (state, errors) =
            SimulationState::from_macro_continue_on_error(self.settings, self.actions);
        ui.vertical(|ui| {
            self.draw_simulation(ui, &state);
            self.draw_actions(ui, &errors);
        })
        .response
    }
}

fn text_width(ui: &mut egui::Ui, text: impl Into<String>) -> f32 {
    ui.fonts(|fonts| {
        let galley = fonts.layout_no_wrap(
            text.into(),
            egui::FontId::default(),
            egui::Color32::default(),
        );
        galley.rect.width()
    })
}

fn progress_bar_text<T: Copy + std::fmt::Display>(
    value: T,
    maximum: T,
    minimum_stat: Option<u16>,
    required_stat: u16,
    stat_name: &str,
) -> String {
    match minimum_stat {
        Some(stat) => match stat >= required_stat {
          true => format!("{value: >5} / {maximum} ({stat_name} ≥{stat})"),
          false => format!("{value: >5} / {maximum} ({stat_name} ≥{stat} theoretically, recipe requires {required_stat})"),
        },
        None => format!("{value: >5} / {maximum}"),
    }
}
