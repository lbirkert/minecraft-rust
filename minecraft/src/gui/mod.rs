use crate::renderer::{
    Renderer,
    gui::{DisplayMode, GuiRect},
};

pub mod container;
pub mod title_screen;

pub fn render_crosshair(renderer: &mut Renderer) {
    renderer.gui_renderer.add(GuiRect {
        texture: "gui/widgets".to_string(),
        display_mode: DisplayMode::Center {
            center: [0.0, 0.0],
            width: Some(0.08),
            height: None,
            uv: Some([239.0 / 256.0, 0.0, 17.0 / 256.0, 15.0 / 256.0]),
        },
        z_index: 0,
    });
}

const ITEMS_CONTAINER_HEIGHT: f32 = 0.2;
const ITEMS_CONTAINER_WIDTH: f32 = (182.0 / 256.0) / (22.0 / 256.0) * ITEMS_CONTAINER_HEIGHT;
const SELECTION_WIDTH: f32 = ITEMS_CONTAINER_WIDTH - ITEMS_CONTAINER_HEIGHT;

pub fn render_items_container(renderer: &mut Renderer) {
    renderer.gui_renderer.add(GuiRect {
        texture: "gui/widgets".to_string(),
        display_mode: DisplayMode::Center {
            center: [0.0, -0.90],
            width: None,
            height: Some(ITEMS_CONTAINER_HEIGHT),
            uv: Some([0.0, 0.0, 182.0 / 256.0, 22.0 / 256.0]),
        },
        z_index: 0,
    });
}

pub fn render_selected_item(renderer: &mut Renderer, index: i32) {
    renderer.gui_renderer.add(GuiRect {
        texture: "gui/widgets".to_string(),
        display_mode: DisplayMode::Center {
            center: [(index as f32 / 8.0 - 0.5) * SELECTION_WIDTH, -0.90],
            width: None,
            height: Some(ITEMS_CONTAINER_HEIGHT),
            uv: Some([0.0, 22.0 / 256.0, 24.0 / 256.0, 24.0 / 256.0]),
        },
        z_index: 1,
    })
}

// centered by default
pub fn render_button(renderer: &mut Renderer, text: &str, x: f32, y: f32) {
    let len = renderer.font_renderer.get_length(text, 40.0);

    renderer.gui_renderer.add(GuiRect {
        texture: "gui/widgets".to_string(),
        display_mode: DisplayMode::Center {
            center: [0.0, 0.0], // TODO: we don't want this to be center
            width: None,
            height: Some(0.5),
            uv: Some([0.0, 66.0 / 256.0, (len / 2.0) / 256.0, 20.0 / 256.0]),
        },
        z_index: 1,
    });

    renderer.gui_renderer.add(GuiRect {
        texture: "gui/widgets".to_string(),
        display_mode: DisplayMode::Center {
            center: [0.0, 0.0], // TODO: we don't want this to be center
            width: None,
            height: Some(0.5),
            uv: Some([
                (200.0 - len / 2.0) / 256.0,
                66.0 / 256.0,
                (len / 2.0) / 256.0,
                20.0 / 256.0,
            ]),
        },
        z_index: 1,
    });

    renderer
        .font_renderer
        .add_centered(text, x, y, 40.0, [1.0, 1.0, 0.0, 1.0]);
}
