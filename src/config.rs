use std::num::NonZeroUsize;

use raphael_data::{CrafterStats, CustomRecipeOverrides, Recipe};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum QualitySource {
    HqMaterialList([u8; 6]),
    Value(u16),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AppConfig {
    pub zoom_percentage: u16,
    pub num_threads: Option<NonZeroUsize>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            zoom_percentage: 100,
            num_threads: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CustomRecipeOverridesConfiguration {
    pub use_custom_recipe: bool,
    pub custom_recipe_overrides: CustomRecipeOverrides,
    pub use_base_increase_overrides: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RecipeConfiguration {
    pub recipe: Recipe,
    pub quality_source: QualitySource,
}

impl Default for RecipeConfiguration {
    fn default() -> Self {
        Self {
            recipe: *raphael_data::RECIPES.values().next().unwrap(),
            quality_source: QualitySource::HqMaterialList([0; 6]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct CrafterConfig {
    pub selected_job: u8,
    pub crafter_stats: [CrafterStats; 8],
    #[serde(default)]
    working_stats: CrafterStats,
    #[serde(default = "CrafterConfig::intermediate_attached_job")]
    attached_job: Option<u8>,
}

impl CrafterConfig {
    pub fn active_stats(&self) -> &CrafterStats {
        if self.attached_job != None {
            &self.crafter_stats[self.selected_job as usize]
        } else {
            &self.working_stats
        }
    }

    pub fn active_stats_mut(&mut self) -> &mut CrafterStats {
        // egui::Checkbox doesn't support value setter, so we have to make a copy preemptively
        if let Some(job) = self.attached_job {
            if job != self.selected_job {
                self.attached_job = Some(self.selected_job);
                self.working_stats = self.crafter_stats[self.selected_job as usize];
            }
        }
        &mut self.working_stats
    }

    fn intermediate_attached_job() -> Option<u8> {
        Some(255)
    }

    pub fn reset_to_job(&mut self) {
        self.attached_job = CrafterConfig::intermediate_attached_job();
    }

    pub fn detach_from_job(&mut self) {
        self.attached_job = None;
    }

    pub fn is_detached(&self) -> bool {
        self.attached_job.is_none()
    }
}

impl Default for CrafterConfig {
    fn default() -> Self {
        Self {
            selected_job: 1,
            crafter_stats: Default::default(),
            working_stats: Default::default(),
            attached_job: CrafterConfig::intermediate_attached_job(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityTarget {
    Zero,
    CollectableT1,
    CollectableT2,
    CollectableT3,
    Full,
    Custom(u16),
}

impl QualityTarget {
    pub fn get_target(self, max_quality: u16) -> u16 {
        match self {
            Self::Zero => 0,
            Self::CollectableT1 => (max_quality as u32 * 55 / 100) as u16,
            Self::CollectableT2 => (max_quality as u32 * 75 / 100) as u16,
            Self::CollectableT3 => (max_quality as u32 * 95 / 100) as u16,
            Self::Full => max_quality,
            Self::Custom(quality) => quality,
        }
    }
}

impl Default for QualityTarget {
    fn default() -> Self {
        Self::Full
    }
}

impl std::fmt::Display for QualityTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Zero => "0% quality",
                Self::CollectableT1 => "55% quality",
                Self::CollectableT2 => "75% quality",
                Self::CollectableT3 => "95% quality",
                Self::Full => "100% quality",
                Self::Custom(_) => "Custom",
            }
        )
    }
}
