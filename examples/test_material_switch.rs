//! Test that switching materials actually changes the checkerboard
//!
//! Takes two screenshots:
//! 1. Frame 30: Initial state with stone + dirt  -> screenshots/test_bevy_1.png
//! 2. Frame 60: After switching to coal_ore      -> screenshots/test_bevy_2.png
//!
//! Run: cargo run --example test_material_switch

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy_mod_imgui::prelude::*;
use studio_core::map_editor::{
    checkerboard::{fill_checkerboard, CheckerboardState},
    imgui_screenshot::{ImguiScreenshotConfig, ImguiScreenshotPlugin},
    lua_materials::LuaMaterialsPlugin,
    material::MaterialPalette,
    playback::PlaybackState,
    voxel_buffer_2d::VoxelBuffer2D,
};

const GRID_WIDTH: usize = 32;
const GRID_HEIGHT: usize = 32;

fn main() {
    println!("=== Material Switch Test ===");
    println!("Frame 30: Screenshot 1 with stone(1) + dirt(2)");
    println!("Frame 45: Switch to coal_ore(3)");
    println!("Frame 60: Screenshot 2 with stone(1) + coal_ore(3)");
    println!();

    std::fs::create_dir_all("screenshots").ok();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (800, 600).into(),
                title: "Material Switch Test".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(bevy_mod_imgui::ImguiPlugin::default())
        .add_plugins(ImguiScreenshotPlugin)
        .add_plugins(LuaMaterialsPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.15, 0.15, 0.15)))
        .insert_resource(VoxelBuffer2D::new(GRID_WIDTH, GRID_HEIGHT))
        .insert_resource(MaterialPalette::default())
        .insert_resource(CheckerboardState::default())
        .insert_resource(PlaybackState::default())
        .insert_resource(TestState::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                load_materials_on_first_frame,
                test_controller,
                update_canvas_texture,
                render_ui,
            )
                .chain(),
        )
        .run();
}

#[derive(Resource, Default)]
struct TestState {
    frame: u32,
    initialized: bool,
    switched: bool,
}

#[derive(Resource)]
struct CanvasTexture {
    handle: Handle<Image>,
    imgui_id: Option<TextureId>,
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

    let size = Extent3d {
        width: GRID_WIDTH as u32,
        height: GRID_HEIGHT as u32,
        depth_or_array_layers: 1,
    };

    // Create image with proper texture usages for dynamic updates
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("canvas_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        },
        asset_usage: RenderAssetUsages::all(),
        ..default()
    };
    image.resize(size);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());

    let handle = images.add(image);

    // Also spawn a Sprite to display the texture (Bevy Sprite WILL update correctly)
    commands.spawn((
        Sprite {
            image: handle.clone(),
            custom_size: Some(Vec2::new(
                GRID_WIDTH as f32 * 10.0,
                GRID_HEIGHT as f32 * 10.0,
            )),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    commands.insert_resource(CanvasTexture {
        handle,
        imgui_id: None,
    });
}

fn load_materials_on_first_frame(
    mut state: ResMut<TestState>,
    mut checker: ResMut<CheckerboardState>,
    mut buffer: ResMut<VoxelBuffer2D>,
    mut playback: ResMut<PlaybackState>,
    palette: Res<MaterialPalette>,
) {
    if state.initialized {
        return;
    }

    // Wait for materials to load
    if palette.materials.is_empty() {
        return;
    }

    state.initialized = true;

    // Set initial materials: stone (1) + dirt (2)
    if palette.materials.len() >= 2 {
        checker.material_a = palette.materials[0].id; // stone = 1
        checker.material_b = palette.materials[1].id; // dirt = 2
    }

    // Generate checkerboard
    buffer.clear();
    fill_checkerboard(&mut buffer, &checker);
    playback.step_index = buffer.cell_count();
    playback.complete();

    println!(
        "Initialized with material_a={}, material_b={}",
        checker.material_a, checker.material_b
    );
}

fn test_controller(
    mut state: ResMut<TestState>,
    mut checker: ResMut<CheckerboardState>,
    mut buffer: ResMut<VoxelBuffer2D>,
    mut playback: ResMut<PlaybackState>,
    palette: Res<MaterialPalette>,
    mut commands: Commands,
    mut exit: EventWriter<bevy::app::AppExit>,
    screenshot_config: Option<Res<ImguiScreenshotConfig>>,
) {
    state.frame += 1;

    // Frame 30: Take screenshot 1 using Bevy's screenshot (captures full scene including Sprite)
    if state.frame == 30 {
        println!("Frame 30: Taking Bevy screenshot 1...");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::Path::new(
                "screenshots/test_bevy_1.png",
            )));
    }

    // Frame 30: Screenshot 1 should be taken
    if state.frame == 30 {
        println!(
            "Frame 30: Screenshot 1 should capture material_a={}, material_b={}",
            checker.material_a, checker.material_b
        );
        // Debug buffer contents
        let sample0 = buffer.get(0, 0);
        let sample1 = buffer.get(1, 0);
        println!("  Buffer[0,0]={}, Buffer[1,0]={}", sample0, sample1);
    }

    // Frame 45: Switch to coal_ore
    if state.frame == 45 && !state.switched {
        state.switched = true;

        // Find coal_ore (id=3)
        if let Some(coal) = palette.materials.iter().find(|m| m.id == 3) {
            println!(
                "Frame 45: Switching material_b from {} to coal_ore (id={})",
                checker.material_b, coal.id
            );
            checker.material_b = coal.id;

            // Regenerate checkerboard
            buffer.clear();
            fill_checkerboard(&mut buffer, &checker);
            playback.step_index = buffer.cell_count();

            println!(
                "  Regenerated with material_a={}, material_b={}",
                checker.material_a, checker.material_b
            );

            // Debug buffer contents after regeneration
            let sample0 = buffer.get(0, 0);
            let sample1 = buffer.get(1, 0);
            println!("  Buffer[0,0]={}, Buffer[1,0]={}", sample0, sample1);
        } else {
            println!("ERROR: coal_ore not found in palette!");
            for mat in &palette.materials {
                println!("  - {} (id={})", mat.name, mat.id);
            }
        }
    }

    // Frame 60: Take screenshot 2 using Bevy's screenshot
    if state.frame == 60 {
        println!("Frame 60: Taking Bevy screenshot 2...");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(std::path::Path::new(
                "screenshots/test_bevy_2.png",
            )));
    }

    // Frame 60: Screenshot 2 should be taken
    if state.frame == 60 {
        println!(
            "Frame 60: Screenshot 2 should capture material_a={}, material_b={}",
            checker.material_a, checker.material_b
        );
        let sample0 = buffer.get(0, 0);
        let sample1 = buffer.get(1, 0);
        println!("  Buffer[0,0]={}, Buffer[1,0]={}", sample0, sample1);
    }

    // Frame 75: Exit
    if state.frame >= 75 {
        println!();
        println!("=== Test Complete ===");
        println!("Compare screenshots:");
        println!("  screenshots/test_switch_1.png - should show stone + DIRT (brown)");
        println!("  screenshots/test_switch_2.png - should show stone + COAL_ORE (dark)");
        println!();
        println!("If they look THE SAME, the texture is not updating!");
        exit.write(AppExit::Success);
    }
}

fn update_canvas_texture(
    buffer: Res<VoxelBuffer2D>,
    palette: Res<MaterialPalette>,
    canvas: Res<CanvasTexture>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(image) = images.get_mut(&canvas.handle) else {
        return;
    };

    // Build new pixel data
    let mut new_data = vec![0u8; buffer.width * buffer.height * 4];

    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mat_id = buffer.get(x, y);
            let color = if mat_id == 0 {
                [30u8, 30, 30, 255]
            } else if let Some(mat) = palette.get_by_id(mat_id) {
                [
                    (mat.color[0] * 255.0) as u8,
                    (mat.color[1] * 255.0) as u8,
                    (mat.color[2] * 255.0) as u8,
                    255,
                ]
            } else {
                [255u8, 0, 255, 255] // Magenta for unknown
            };

            let idx = (y * buffer.width + x) * 4;
            new_data[idx..idx + 4].copy_from_slice(&color);
        }
    }

    // Update image data - Bevy should sync this to GPU
    image.data = Some(new_data);
}

fn render_ui(
    mut context: NonSendMut<ImguiContext>,
    mut canvas: ResMut<CanvasTexture>,
    checker: Res<CheckerboardState>,
    state: Res<TestState>,
    palette: Res<MaterialPalette>,
) {
    // Register texture once (for ImGui display - may not update)
    if canvas.imgui_id.is_none() {
        let id = context.register_bevy_texture(canvas.handle.clone());
        canvas.imgui_id = Some(id);
    }

    let ui = context.ui();

    ui.window("Test Status")
        .size([300.0, 200.0], Condition::FirstUseEver)
        .position([20.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Frame: {}", state.frame));
            ui.text(format!(
                "Checker: {} + {}",
                checker.material_a, checker.material_b
            ));
            ui.separator();

            ui.text("Materials:");
            for mat in &palette.materials {
                let color = [mat.color[0], mat.color[1], mat.color[2], 1.0];
                let _token = ui.push_style_color(imgui::StyleColor::Text, color);
                ui.text(format!("  {} (id={})", mat.name, mat.id));
            }
        });

    ui.window("Canvas")
        .size(
            [
                GRID_WIDTH as f32 * 12.0 + 20.0,
                GRID_HEIGHT as f32 * 12.0 + 40.0,
            ],
            Condition::FirstUseEver,
        )
        .position([340.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            if let Some(tex_id) = canvas.imgui_id {
                imgui::Image::new(
                    tex_id,
                    [GRID_WIDTH as f32 * 12.0, GRID_HEIGHT as f32 * 12.0],
                )
                .build(ui);
            }
        });
}
