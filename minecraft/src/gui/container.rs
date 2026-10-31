use crate::renderer::{
    Renderer,
    gui::{DisplayMode, GuiRect},
};

pub enum Container {
    InventorySurvival,
    InventoryCreative,
}

impl Container {
    pub fn render(&self, renderer: &mut Renderer) {
        match self {
            Container::InventorySurvival => renderer.gui_renderer.add(GuiRect {
                texture: "gui/container/inventory".to_string(),
                display_mode: DisplayMode::Center {
                    center: [0.0, 0.0],
                    width: Some(1.5),
                    height: None,
                    uv: Some([0.0, 0.0, 1.0, 169.0 / 256.0]),
                },
                z_index: 0,
            }),
            Container::InventoryCreative => renderer.gui_renderer.add(GuiRect {
                texture: "gui/container/creative_inventory/tab_inventory".to_string(),
                display_mode: DisplayMode::Center {
                    center: [0.0, 0.0],
                    width: Some(1.5),
                    height: None,
                    uv: Some([0.0, 0.0, 1.0, 169.0 / 256.0]),
                },
                z_index: 0,
            }),
        }
    }
}
