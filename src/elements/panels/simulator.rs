use raphael_data::Locale;
use raphael_data::{Recipe, Consumable};
use raphael_sim::{Action, ActionError, Settings, SimulationState};
use raphael_translations::{t, t_format};

use crate::{
    config::CrafterConfig,
    config::QualityTarget,
    context::AppContext,
    elements::{util, widgets::HelpText},
    solve::{SolveParameters, SolveState, MinimumStats},
};

pub struct Simulator<'a> {
    settings: Settings,
    initial_quality: u16,
    job_id: u8,
    actions: &'a [Action],
    item_always_collectable: bool,
    config_changed: bool,
    crafter_config: &'a mut CrafterConfig,
    recipe: &'a Recipe,
    consumables: [Option<Consumable>; 2],
    minimum_stats: &'a MinimumStats,
    locale: Locale,
}

fn config_changed(app_context: &AppContext, solve_state: &SolveState) -> bool {
    !solve_state.solving()
        && solve_state
            .last_solve_info()
            .is_some_and(|info| info.solve_params != SolveParameters::from(app_context))
}

impl<'a> Simulator<'a> {
    pub fn new(app_context: &'a mut AppContext, solve_state: &'a SolveState) -> Self {
        let config_changed = config_changed(app_context, solve_state);
        let settings = app_context.game_settings();
        let initial_quality = app_context.initial_quality();
        let AppContext {
            locale,
            recipe_config,
            crafter_config,
            ..
        } = app_context;
        let item_always_collectable = raphael_data::ITEMS
            .get(recipe_config.recipe().item_id)
            .map(|item| item.always_collectable)
            .unwrap_or_default();
        Self {
            settings,
            initial_quality,
            job_id: crafter_config.selected_job,
            actions: solve_state.actions(),
            item_always_collectable,
            config_changed,
            locale: *locale,
            crafter_config,
            recipe: recipe_config.recipe(),
            consumables: [app_context.selected_food, app_context.selected_potion],
            minimum_stats: &solve_state.minimum_stats,
        }
    }
}

impl Simulator<'_> {
    fn draw_simulation(&mut self, ui: &mut egui::Ui, state: &SimulationState) {
        let locale = self.locale;
        ui.group(|ui| {
            ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 3.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!(locale, "Simulation")).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_visible(
                            self.config_changed,
                            egui::Label::new(
                                egui::RichText::new(t!(
                                    locale,
                                    "⚠ Some parameters have changed since last solve."
                                ))
                                .small()
                                .color(ui.visuals().warn_fg_color),
                            ),
                        );
                    });
                });

                ui.separator();

                let max_text_width = util::max_text_width(
                    ui,
                    [
                        t!(locale, "Progress"),
                        t!(locale, "Quality"),
                        t!(locale, "Durability"),
                        t!(locale, "CP"),
                    ],
                    egui::TextStyle::Body,
                );
                let max_value_text_width =
                    5.0 * util::max_text_width(ui, 0..=9, egui::TextStyle::Body);

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
                            if ui.button(t_format!(locale, "Copy {stat}")).clicked() {
                                ui.ctx().copy_text(stat.to_string());
                                ui.close();
                            }
                            if stat < req_stat {
                                if ui.button(t_format!(locale, "Copy {req_stat}")).clicked() {
                                    ui.ctx().copy_text(req_stat.to_string());
                                    ui.close();
                                }
                                stat = req_stat;
                            }
                            // this will be incorrect if stat can't fulfill consumables,
                            // but that situation is rarely encountered in practice
                            let bonus = calc_bonus(stat, &self.consumables);
                            let crafter_stat = stat.saturating_sub(bonus);
                            if crafter_stat != stat {
                                if ui.button(t_format!(locale, "Copy {crafter_stat}")).clicked() {
                                    ui.ctx().copy_text(crafter_stat.to_string());
                                    ui.close();
                                }
                            }
                            if ui.add_enabled(
                                config_stat != crafter_stat,
                                egui::Button::new(t_format!(locale,
                                    "Set crafter stat to {stat}")),
                            ).clicked() {
                                set_config_stat(crafter_stat);
                                ui.close();
                            }
                        });
                    }
                };

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label(t!(locale, "Progress"));
                    });
                    let response = ui.add(
                        egui::ProgressBar::new(
                            state.progress as f32 / self.settings.max_progress as f32,
                        )
                        .text(progress_bar_text(
                            ui,
                            state.progress,
                            self.settings.max_progress,
                            max_value_text_width,
                            locale,
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
                        ui.label(t!(locale, "Quality"));
                    });
                    let quality = self.initial_quality.saturating_add(state.quality);
                    let response = ui.add(
                        egui::ProgressBar::new(quality as f32 / self.settings.max_quality as f32)
                            .text(progress_bar_text(
                                ui,
                                quality,
                                self.settings.max_quality,
                                max_value_text_width,
                                locale,
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
                        ui.label(t!(locale, "Durability"));
                    });
                    ui.add(
                        egui::ProgressBar::new(
                            state.durability as f32 / self.settings.max_durability as f32,
                        )
                        .text(progress_bar_text(
                            ui,
                            state.durability,
                            self.settings.max_durability,
                            max_value_text_width,
                            locale,
                            None,
                            0,
                            "",
                        ))
                        .corner_radius(0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(text_size, text_layout, |ui| {
                        ui.label(t!(locale, "CP"));
                    });
                    let response = ui.add(
                        egui::ProgressBar::new(state.cp as f32 / self.settings.max_cp as f32)
                            .text(progress_bar_text(
                                ui,
                                state.cp,
                                self.settings.max_cp,
                                max_value_text_width,
                                locale,
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
                            true => t!(
                                locale,
                                "Calculated assuming worst possible sequence of conditions"
                            ),
                            false => {
                                t!(locale, "Calculated assuming Normal conditon on every step")
                            }
                        }));
                        if !state.is_final(&self.settings) {
                            // do nothing
                        } else if state.progress < self.settings.max_progress {
                            ui.label(t!(locale, "Synthesis failed"));
                        } else if self.item_always_collectable {
                            let (t1, t2, t3) = (
                                QualityTarget::CollectableT1.get_target(self.settings.max_quality),
                                QualityTarget::CollectableT2.get_target(self.settings.max_quality),
                                QualityTarget::CollectableT3.get_target(self.settings.max_quality),
                            );
                            let tier = match self.initial_quality.saturating_add(state.quality) {
                                quality if quality >= t3 => 3,
                                quality if quality >= t2 => 2,
                                quality if quality >= t1 => 1,
                                _ => 0,
                            };
                            ui.label(t_format!(locale, "Tier {tier} collectable"));
                        } else {
                            let hq = raphael_data::hq_percentage(
                                self.initial_quality.saturating_add(state.quality),
                                self.settings.max_quality,
                            )
                            .unwrap_or(0);
                            ui.label(t_format!(locale, "{hq}% HQ"));
                        }
                    });
                });
            });
        });
    }

    fn draw_actions(&self, ui: &mut egui::Ui, errors: &[Result<(), ActionError>]) {
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
                        let image = util::get_action_icon(*action, self.job_id)
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
                            egui::Image::new(egui::include_image!(concat!(
                                env!("CARGO_MANIFEST_DIR"),
                                "/assets/action-icons/disabled.webp"
                            )))
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
            SimulationState::from_macro_continue_on_error(&self.settings, self.actions);
        ui.vertical(|ui| {
            self.draw_simulation(ui, &state);
            self.draw_actions(ui, &errors);
        })
        .response
    }
}

fn progress_bar_text<T: Copy + std::cmp::Ord + std::ops::Sub<Output = T> + std::fmt::Display>(
    ui: &egui::Ui,
    value: T,
    maximum: T,
    max_value_text_width: f32,
    locale: Locale,
    minimum_stat: Option<u16>,
    required_stat: u16,
    stat_name: &str,
) -> impl Into<egui::WidgetText> {
    let text = match minimum_stat {
        Some(stat) => match stat >= required_stat {
          true => format!("{value} / {maximum} ({stat_name} ≥{stat})"),
          false => t_format!(locale, "{value: >5} / {maximum} ({stat_name} ≥{stat} theoretically, recipe requires {required_stat})"),
        },
        None => format!("{value} / {maximum}"),
    };

    let value_text_width = util::text_width(ui, value, egui::TextStyle::Body);

    let style = ui.style();
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text,
        max_value_text_width - value_text_width,
        egui::TextFormat {
            font_id: egui::TextStyle::Body.resolve(style),
            color: egui::Color32::PLACEHOLDER,
            ..Default::default()
        },
    );
    job
}
