#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub label: String,
    pub category: PaletteCategory,
    pub action: PaletteAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaletteCategory {
    Profile,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaletteAction {
    SwitchProfile(String),
    ToggleFence,
    RunDoctor,
    OpenSecretGenerator,
    ToggleInspector,
    ToggleGrouping,
    ImportDotEnv,
    ExportDotEnv,
    Undo,
    OpenHealthAudit,
    OpenProfileMatrix,
}

pub fn build_palette_items(profiles: &[String], active_profile: &str) -> Vec<PaletteItem> {
    let mut items = Vec::new();

    for name in profiles {
        let label = if name == active_profile {
            format!("Profile: {} (active)", name)
        } else {
            format!("Switch to profile: {}", name)
        };
        items.push(PaletteItem {
            label,
            category: PaletteCategory::Profile,
            action: PaletteAction::SwitchProfile(name.clone()),
        });
    }

    items.push(PaletteItem {
        label: "Health Audit (H)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::OpenHealthAudit,
    });
    items.push(PaletteItem {
        label: "Profile Matrix Grid (M)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::OpenProfileMatrix,
    });
    items.push(PaletteItem {
        label: "Generate Secret Password / Token (G)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::OpenSecretGenerator,
    });
    items.push(PaletteItem {
        label: "Toggle AI Fence Protection (F)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::ToggleFence,
    });
    items.push(PaletteItem {
        label: "Toggle Bottom Inspector Drawer (Tab)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::ToggleInspector,
    });
    items.push(PaletteItem {
        label: "Toggle Variable Grouping (g)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::ToggleGrouping,
    });
    items.push(PaletteItem {
        label: "Import from .env File (I)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::ImportDotEnv,
    });
    items.push(PaletteItem {
        label: "Export to .env File (E)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::ExportDotEnv,
    });
    items.push(PaletteItem {
        label: "Undo Last Action (u)".to_string(),
        category: PaletteCategory::System,
        action: PaletteAction::Undo,
    });

    items
}
