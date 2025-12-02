use eframe::egui;
use egui::{Align2, Color32, Pos2, Rect, Stroke, Vec2};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use uuid::Uuid;
use libavif_sys;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

const COLLISION_GAP: f32 = 2.0;
const MIN_BLOCK_SIZE: f32 = 50.0;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("MaBlocks - Rust Canvas"),
        ..Default::default()
    };
    eframe::run_native(
        "MaBlocks",
        options,
        Box::new(|_cc| Ok(Box::new(CanvasApp::default()))),
    )
}

// --- Data Structures ---

#[derive(Clone, Copy, PartialEq, Debug)]
enum ResizeHandle {
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

#[derive(Clone)]
struct InteractionState {
    id: Uuid,
    handle: ResizeHandle,
    initial_mouse_pos: Pos2,
    initial_block_rect: Rect,
}

#[derive(Clone)]
enum BlockContent {
    Text {
        text: String,
    },
    Image {
        frames: Vec<egui::TextureHandle>,
        frame_delays: Vec<f64>, // Seconds
        aspect_ratio: f32,
        playing: bool,
        current_frame_idx: usize,
        last_frame_time: f64,
        counter: i32,
        path: Option<String>,
    },
}

#[derive(Clone)]
struct Block {
    id: Uuid,
    rect: Rect, // World coordinates
    content: BlockContent,
    chained: bool,
    selected: bool,
}

#[derive(Default)]
struct Viewport {
    pan: Vec2,
    zoom: f32,
}

struct CanvasApp {
    viewport: Viewport,
    blocks: Vec<Block>,
    /// State for resizing (Right mouse drag)
    resizing_state: Option<InteractionState>,
    /// UUID of the text block currently being edited
    editing_id: Option<Uuid>,
    /// Request to focus a specific text widget
    focus_request: Option<Uuid>,
    /// Track the last dragged block to resolve collisions only for it
    last_dragged_id: Option<Uuid>,
    /// Timestamp of the last interaction with a chained block
    last_chain_interaction: f64,
    /// Channel for receiving loaded image data from background threads
    image_rx: Receiver<ImageLoadData>,
    /// Sender to clone for background threads
    image_tx: Sender<ImageLoadData>,
    /// Is the counter tool active?
    counter_tool_active: bool,
    /// Show help window
    show_help: bool,
    /// Cache for markdown rendering
    common_mark_cache: CommonMarkCache,
}

#[derive(Clone)]
struct ImageLoadData {
    frames: Vec<egui::ColorImage>,
    frame_delays: Vec<f64>,
    aspect_ratio: f32,
    path: Option<String>,
    // If this load is for an existing block (session load), we pass the ID
    target_block_id: Option<Uuid>,
}

// --- Serialization Structs ---

#[derive(Serialize, Deserialize)]
struct Session {
    viewport: ViewportData,
    blocks: Vec<BlockData>,
}

#[derive(Serialize, Deserialize)]
struct ViewportData {
    pan: [f32; 2],
    zoom: f32,
}

#[derive(Serialize, Deserialize)]
struct BlockData {
    id: Uuid,
    rect: [f32; 4], // min_x, min_y, max_x, max_y
    content: BlockContentData,
    chained: bool,
}

#[derive(Serialize, Deserialize)]
enum BlockContentData {
    Text { text: String },
    Image {
        path: String,
        counter: i32,
        playing: bool,
    },
}

impl Default for CanvasApp {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            viewport: Viewport {
                pan: Vec2::ZERO,
                zoom: 1.0,
            },
            blocks: Vec::new(),
            resizing_state: None,
            editing_id: None,
            focus_request: None,
            last_dragged_id: None,
            last_chain_interaction: 0.0,
            image_rx: rx,
            image_tx: tx,
            counter_tool_active: false,
            show_help: false,
            common_mark_cache: CommonMarkCache::default(),
        }
    }
}

// --- Physics / Collision Helpers ---

impl Block {
    fn resolve_collision(&mut self, others: &[Block]) -> bool {
        let mut moved = false;
        for _ in 0..3 {
            let mut total_push = Vec2::ZERO;
            let my_rect = self.rect.expand(COLLISION_GAP);

            for other in others {
                if self.id == other.id {
                    continue;
                }

                if my_rect.intersects(other.rect) {
                    let intersection = my_rect.intersect(other.rect);
                    let dx = intersection.width();
                    let dy = intersection.height();
                    let center_diff = self.rect.center() - other.rect.center();

                    let push = if dx < dy {
                        Vec2::new(if center_diff.x > 0.0 { dx } else { -dx }, 0.0)
                    } else {
                        Vec2::new(0.0, if center_diff.y > 0.0 { dy } else { -dy })
                    };
                    total_push += push;
                }
            }

            if total_push != Vec2::ZERO {
                self.rect = self.rect.translate(total_push);
                moved = true;
            }
        }
        moved
    }
}

// --- App Implementation ---

impl eframe::App for CanvasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut help_toggled = false;
        // Poll for loaded image data
        while let Ok(data) = self.image_rx.try_recv() {
            if data.frames.is_empty() {
                continue;
            }
            
            let texture_frames: Vec<_> = data.frames.iter().enumerate().map(|(i, img)| {
                ctx.load_texture(
                    format!("img-{}-{}", Uuid::new_v4(), i),
                    img.clone(),
                    egui::TextureOptions::default(),
                )
            }).collect();

            if let Some(target_id) = data.target_block_id {
                // Update existing block (from session load)
                if let Some(block) = self.blocks.iter_mut().find(|b| b.id == target_id) {
                    if let BlockContent::Image { 
                        frames, 
                        frame_delays, 
                        aspect_ratio, 
                        .. 
                    } = &mut block.content {
                        *frames = texture_frames;
                        *frame_delays = data.frame_delays;
                        *aspect_ratio = data.aspect_ratio;
                    }
                }
            } else {
                // Create new block
                let id = Uuid::new_v4();
                let width = 300.0;
                let height = width / data.aspect_ratio;
                let size = Vec2::new(width, height);
                let center_world = -self.viewport.pan;
                let pos = self.find_free_rect(center_world, size);
                
                self.blocks.push(Block {
                    id,
                    rect: Rect::from_min_size(pos.to_pos2(), size),
                    content: BlockContent::Image {
                        frames: texture_frames,
                        frame_delays: data.frame_delays,
                        aspect_ratio: data.aspect_ratio,
                        playing: data.frames.len() > 1,
                        current_frame_idx: 0,
                        last_frame_time: 0.0,
                        counter: 0,
                        path: data.path,
                    },
                    chained: false,
                    selected: false,
                });
            }
        }

        if !self.blocks.is_empty() {
            ctx.request_repaint();
        }
        let time_now = ctx.input(|i| i.time);

        // 1. Update Animation State
        for block in &mut self.blocks {
            if let BlockContent::Image {
                frames,
                frame_delays,
                playing,
                current_frame_idx,
                last_frame_time,
                ..
            } = &mut block.content
            {
                if *playing && frames.len() > 1 {
                    let delay = frame_delays.get(*current_frame_idx).unwrap_or(&0.1);
                    if time_now - *last_frame_time > *delay {
                        *current_frame_idx = (*current_frame_idx + 1) % frames.len();
                        *last_frame_time = time_now;
                    }
                }
            }
        }

        // 2. Global Inputs
        let input = ctx.input(|i| i.clone());
        if input.raw_scroll_delta.y.abs() > 0.0 {
            let factor = 1.0 + input.raw_scroll_delta.y * 0.001;
            let old_zoom = self.viewport.zoom;
            self.viewport.zoom = (self.viewport.zoom * factor).clamp(0.1, 5.0);

            if let Some(mouse_pos) = input.pointer.hover_pos() {
                let screen_center = ctx.screen_rect().center().to_vec2();
                let mouse_offset = mouse_pos.to_vec2() - screen_center;
                let world_point_under_mouse = (mouse_offset / old_zoom) - self.viewport.pan;
                self.viewport.pan = (mouse_offset / self.viewport.zoom) - world_point_under_mouse;
            }
        }

        if input.pointer.middle_down()
            || (input.key_down(egui::Key::Space) && input.pointer.primary_down())
        {
            self.viewport.pan += input.pointer.delta() / self.viewport.zoom;
        }

        // 3. Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("💾").on_hover_text("Save Session").clicked() {
                    self.save_session();
                }
                if ui.button("📂").on_hover_text("Load Session").clicked() {
                    self.load_session();
                }
                ui.separator();
                
                if ui.button("🔤").on_hover_text("Add Text").clicked() {
                    self.spawn_text_block(ui.ctx());
                }
                if ui.button("🖼").on_hover_text("Add Image").clicked() {
                    self.spawn_image_block(ui.ctx());
                }
                
                let mut btn = egui::Button::new("🔢");
                if self.counter_tool_active {
                    btn = btn.fill(Color32::LIGHT_GREEN);
                }
                if ui.add(btn).on_hover_text("Counter Tool").clicked() {
                    self.counter_tool_active = !self.counter_tool_active;
                }

                ui.separator();
                if ui.button("❓").on_hover_text("Help").clicked() {
                    self.show_help = !self.show_help;
                    help_toggled = true;
                }
            });
        });

        // 4. Main Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, Color32::from_rgb(30, 30, 30));
            self.process_canvas(ui);
        });

        let mut help_layer_id = None;
        if self.show_help {
            let mut open = true;
            egui::Window::new("Help")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    help_layer_id = Some(ui.layer_id());
                    ui.heading("Controls");
                    ui.label("• Pan: Middle Mouse Drag OR Space + Left Mouse Drag");
                    ui.label("• Zoom: Mouse Wheel");
                    ui.label("• Move Block: Left Mouse Drag");
                    ui.label("• Resize Block: Right Mouse Drag (corners)");
                    ui.label("• Edit Text: Double Click");
                    ui.label("• Toggle GIF: Click");
                    ui.label("• Delete Block: Click 'x' handle");
                    ui.label("• Chain Block: Click 'o' handle (moves together)");
                    ui.separator();
                    ui.heading("Tools");
                    ui.label("• 💾 Save: Save current session to JSON");
                    ui.label("• 📂 Load: Load session from JSON");
                    ui.label("• 🔤 Text: Add new markdown text block");
                    ui.label("• 🖼 Image: Add image (PNG, JPG, GIF, AVIF)");
                    ui.label("• 🔢 Counter: Click image to count, Right-click to decrement");
                });
            if !open {
                self.show_help = false;
            }
        }

        if self.show_help && !help_toggled && ctx.input(|i| i.pointer.any_click()) {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                if let Some(layer_id) = ctx.layer_id_at(pos) {
                    if let Some(help_id) = help_layer_id {
                        if layer_id != help_id {
                            self.show_help = false;
                        }
                    }
                } else {
                    self.show_help = false;
                }
            }
        }
    }
}

impl CanvasApp {
    fn process_canvas(&mut self, ui: &mut egui::Ui) {
        let screen_rect = ui.max_rect();
        let screen_center = screen_rect.center().to_vec2();
        let zoom = self.viewport.zoom;
        let pan = self.viewport.pan;

        let mouse_pos = ui.input(|i| i.pointer.hover_pos());
        let secondary_down = ui.input(|i| i.pointer.secondary_down());
        let secondary_pressed =
            ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
        let secondary_released =
            ui.input(|i| i.pointer.button_released(egui::PointerButton::Secondary));

        // --- Resize Logic ---
        if secondary_pressed && !self.counter_tool_active {
            if let Some(m_pos) = mouse_pos {
                let world_mouse = (m_pos.to_vec2() - screen_center) / zoom - pan;
                if let Some(block) = self.blocks.iter().rev().find(|b| b.rect.contains(world_mouse.to_pos2())) {
                    let center = block.rect.center();
                    let handle = match (world_mouse.x < center.x, world_mouse.y < center.y) {
                        (true, true) => ResizeHandle::TopLeft,
                        (false, true) => ResizeHandle::TopRight,
                        (true, false) => ResizeHandle::BottomLeft,
                        (false, false) => ResizeHandle::BottomRight,
                    };
                    self.resizing_state = Some(InteractionState {
                        id: block.id,
                        handle,
                        initial_mouse_pos: m_pos,
                        initial_block_rect: block.rect,
                    });
                    self.last_dragged_id = Some(block.id);
                }
            }
        }

        if secondary_released {
            self.resizing_state = None;
        }

        if let Some(state) = &self.resizing_state {
            if let Some(curr_mouse_pos) = mouse_pos {
                if let Some(idx) = self.blocks.iter().position(|b| b.id == state.id) {
                    let delta_screen = curr_mouse_pos - state.initial_mouse_pos;
                    let delta_world = delta_screen / zoom;
                    let mut new_rect = state.initial_block_rect;
                    let min_size = MIN_BLOCK_SIZE;

                    match state.handle {
                        ResizeHandle::BottomRight => {
                            new_rect.max.x += delta_world.x;
                            new_rect.max.y += delta_world.y;
                        }
                        ResizeHandle::BottomLeft => {
                            new_rect.min.x += delta_world.x;
                            new_rect.max.y += delta_world.y;
                        }
                        ResizeHandle::TopRight => {
                            new_rect.max.x += delta_world.x;
                            new_rect.min.y += delta_world.y;
                        }
                        ResizeHandle::TopLeft => {
                            new_rect.min.x += delta_world.x;
                            new_rect.min.y += delta_world.y;
                        }
                    }

                    if new_rect.width() < min_size {
                        if state.handle == ResizeHandle::TopLeft || state.handle == ResizeHandle::BottomLeft {
                            new_rect.min.x = new_rect.max.x - min_size;
                        } else {
                            new_rect.max.x = new_rect.min.x + min_size;
                        }
                    }
                    if new_rect.height() < min_size {
                        if state.handle == ResizeHandle::TopLeft || state.handle == ResizeHandle::TopRight {
                            new_rect.min.y = new_rect.max.y - min_size;
                        } else {
                            new_rect.max.y = new_rect.min.y + min_size;
                        }
                    }

                    if let BlockContent::Image { aspect_ratio, .. } = self.blocks[idx].content {
                        let w = new_rect.width();
                        new_rect.set_height(w / aspect_ratio);
                    }
                    self.blocks[idx].rect = new_rect;
                }
            }
        }

        // --- Render & Interaction ---
        let mut ids_to_delete = HashSet::new();
        let mut interact_captured = false;
        let mut pending_move = None;

        for i in 0..self.blocks.len() {
            let b_id = self.blocks[i].id;
            let b_rect = self.blocks[i].rect;
            let b_selected = self.blocks[i].selected;
            let b_chained = self.blocks[i].chained;
            let is_editing = self.editing_id == Some(b_id);

            let screen_pos_min = screen_center + (b_rect.min.to_vec2() + pan) * zoom;
            let screen_size = b_rect.size() * zoom;
            let screen_rect = Rect::from_min_size(screen_pos_min.to_pos2(), screen_size);

            if !screen_rect.intersects(screen_rect) { continue; }

            let border_color = if b_selected { Color32::YELLOW } else if b_chained { Color32::GREEN } else { Color32::BLACK };
            let bg_color = Color32::from_rgb(240, 240, 240);

            ui.painter().rect_filled(screen_rect, 5.0, bg_color);
            ui.painter().rect_stroke(screen_rect, 5.0, Stroke::new(2.0, border_color));

            let interact_id = ui.make_persistent_id(b_id);
            let sense = if is_editing { egui::Sense::hover() } else if self.counter_tool_active { egui::Sense::click() } else { egui::Sense::click_and_drag() };
            let response = ui.interact(screen_rect, interact_id, sense);

            if response.hovered() || response.dragged() { interact_captured = true; }

            let btn_size = 16.0 * zoom;
            let padding = 4.0 * zoom;
            let top_right = screen_rect.right_top();
            let close_rect = Rect::from_center_size(top_right + Vec2::new(-btn_size / 2.0 - padding, btn_size / 2.0 + padding), Vec2::splat(btn_size));
            let chain_rect = Rect::from_center_size(close_rect.center() - Vec2::new(btn_size + padding, 0.0), Vec2::splat(btn_size));

            let close_hovered = mouse_pos.map_or(false, |p| close_rect.contains(p));
            let chain_hovered = mouse_pos.map_or(false, |p| chain_rect.contains(p));

            if response.dragged() && !secondary_down && !ui.input(|i| i.pointer.middle_down()) {
                let delta = response.drag_delta() / zoom;
                pending_move = Some((i, delta));
                self.last_dragged_id = Some(b_id);
            }

            if is_editing {
                let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(screen_rect.shrink(4.0)).layout(egui::Layout::left_to_right(egui::Align::Min)));
                if let Some(text_mut) = self.blocks[i].content.as_text_mut() {
                    let output = egui::TextEdit::multiline(text_mut)
                        .font(egui::FontId::proportional(16.0 * zoom))
                        .frame(false)
                        .desired_width(f32::INFINITY)
                        .show(&mut child_ui);
                    if self.focus_request == Some(b_id) {
                        output.response.request_focus();
                        self.focus_request = None;
                    }
                    if ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                        self.editing_id = None;
                    }
                }
            } else {
                match &mut self.blocks[i].content {
                    BlockContent::Text { text } => {
                        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(screen_rect.shrink(5.0 * zoom)).layout(egui::Layout::left_to_right(egui::Align::Min)));
                        for (_text_style, font_id) in child_ui.style_mut().text_styles.iter_mut() {
                            font_id.size *= zoom;
                        }
                        CommonMarkViewer::new().show(&mut child_ui, &mut self.common_mark_cache, text);
                        if response.double_clicked() && !close_hovered && !chain_hovered {
                            self.editing_id = Some(b_id);
                            self.focus_request = Some(b_id);
                        }
                    }
                    BlockContent::Image { frames, current_frame_idx, playing, counter, .. } => {
                        if let Some(tex) = frames.get(*current_frame_idx) {
                            ui.painter().image(tex.id(), screen_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                        if *counter > 0 {
                            let circle_radius = 15.0 * zoom;
                            let circle_center = screen_rect.min + Vec2::new(circle_radius + 5.0, circle_radius + 5.0);
                            ui.painter().circle_filled(circle_center, circle_radius, Color32::GREEN);
                            ui.painter().text(circle_center, Align2::CENTER_CENTER, counter.to_string(), egui::FontId::proportional(20.0 * zoom), Color32::BLACK);
                        }
                        if self.counter_tool_active {
                            if response.clicked() { *counter += 1; }
                            else if response.secondary_clicked() { *counter = (*counter - 1).max(0); }
                        } else if response.clicked() && !close_hovered && !chain_hovered {
                            *playing = !*playing;
                        }
                    }
                }
            }

            if response.hovered() || response.dragged() || b_chained {
                ui.painter().circle_filled(close_rect.center(), btn_size / 2.0, if close_hovered { Color32::from_rgb(255, 100, 100) } else { Color32::RED });
                ui.painter().text(close_rect.center(), Align2::CENTER_CENTER, "x", egui::FontId::monospace(12.0 * zoom), Color32::WHITE);
                ui.painter().circle_filled(chain_rect.center(), btn_size / 2.0, if b_chained { Color32::GREEN } else if chain_hovered { Color32::LIGHT_GRAY } else { Color32::GRAY });
                ui.painter().text(chain_rect.center(), Align2::CENTER_CENTER, "o", egui::FontId::monospace(12.0 * zoom), Color32::WHITE);

                if response.clicked() {
                    if close_hovered { ids_to_delete.insert(b_id); }
                    else if chain_hovered {
                        self.blocks[i].chained = !self.blocks[i].chained;
                        self.last_chain_interaction = ui.input(|i| i.time);
                    }
                }
            }
        }

        if let Some((idx, delta)) = pending_move {
            let mut moved_indices = vec![idx];
            if self.blocks[idx].chained {
                self.last_chain_interaction = ui.input(|i| i.time);
                for (i, b) in self.blocks.iter().enumerate() {
                    if i != idx && b.chained { moved_indices.push(i); }
                }
            }
            for &i in &moved_indices {
                self.blocks[i].rect = self.blocks[i].rect.translate(delta);
            }
        }

        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary) || i.pointer.button_released(egui::PointerButton::Secondary)) {
            if let Some(dragged_id) = self.last_dragged_id {
                if let Some(idx) = self.blocks.iter().position(|b| b.id == dragged_id) {
                    let others = self.blocks.clone();
                    self.blocks[idx].resolve_collision(&others);
                    if self.blocks[idx].chained {
                        for i in 0..self.blocks.len() {
                            if self.blocks[i].chained && i != idx { self.blocks[i].resolve_collision(&others); }
                        }
                    }
                }
                self.last_dragged_id = None;
            }
        }

        self.blocks.retain(|b| !ids_to_delete.contains(&b.id));

        if ui.input(|i| i.pointer.any_click()) && !interact_captured && !secondary_down {
            self.editing_id = None;
            for b in &mut self.blocks { b.selected = false; }
        }

        let time_now = ui.input(|i| i.time);
        if self.blocks.iter().any(|b| b.chained) {
            if time_now - self.last_chain_interaction > 10.0 {
                for b in &mut self.blocks { b.chained = false; }
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((10.0 - (time_now - self.last_chain_interaction)).max(0.0)));
            }
        }
    }

    fn spawn_text_block(&mut self, _ctx: &egui::Context) {
        let center_world = -self.viewport.pan;
        let size = Vec2::new(200.0, 100.0);
        let pos = self.find_free_rect(center_world, size);
        self.blocks.push(Block {
            id: Uuid::new_v4(),
            rect: Rect::from_min_size(pos.to_pos2(), size),
            content: BlockContent::Text { text: "Double click to edit...".to_string() },
            chained: false,
            selected: false,
        });
    }

    fn spawn_image_block(&mut self, ctx: &egui::Context) {
        if let Some(path) = FileDialog::new().add_filter("Image", &["png", "jpg", "jpeg", "gif", "avif"]).pick_file() {
            self.load_image_file(path, ctx.clone(), None);
        }
    }

    fn load_image_file(&self, path: PathBuf, _ctx: egui::Context, target_block_id: Option<Uuid>) {
        let tx = self.image_tx.clone();
        let path_str = path.to_string_lossy().to_string();

        thread::spawn(move || {
            let is_gif = path.extension().is_some_and(|e| e.to_string_lossy().to_lowercase() == "gif");
            let is_avif = path.extension().is_some_and(|e| e.to_string_lossy().to_lowercase() == "avif");
            let mut frames_data = vec![];
            let mut delays = vec![];
            let mut aspect = 1.0;

            if is_gif {
                match File::open(&path) {
                    Ok(file) => {
                        let mut decoder = gif::DecodeOptions::new();
                        decoder.set_color_output(gif::ColorOutput::RGBA);
                        if let Ok(mut decoder) = decoder.read_info(BufReader::new(file)) {
                            while let Some(frame) = decoder.read_next_frame().ok().flatten() {
                                let size = [frame.width as usize, frame.height as usize];
                                if frames_data.is_empty() { aspect = size[0] as f32 / size[1] as f32; }
                                frames_data.push(egui::ColorImage::from_rgba_unmultiplied(size, &frame.buffer[..]));
                                delays.push(frame.delay as f64 / 100.0);
                            }
                        }
                    }
                    Err(e) => eprintln!("GIF open error: {}", e),
                }
            } else if is_avif {
                match File::open(&path) {
                    Ok(mut file) => {
                        let mut buffer = Vec::new();
                        if file.read_to_end(&mut buffer).is_ok() {
                            unsafe {
                                let decoder = libavif_sys::avifDecoderCreate();
                                if !decoder.is_null() {
                                    if libavif_sys::avifDecoderSetIOMemory(decoder, buffer.as_ptr(), buffer.len()) == libavif_sys::AVIF_RESULT_OK {
                                        if libavif_sys::avifDecoderParse(decoder) == libavif_sys::AVIF_RESULT_OK {
                                            while libavif_sys::avifDecoderNextImage(decoder) == libavif_sys::AVIF_RESULT_OK {
                                                let image = (*decoder).image;
                                                let width = (*image).width;
                                                let height = (*image).height;
                                                let mut rgb: libavif_sys::avifRGBImage = std::mem::zeroed();
                                                libavif_sys::avifRGBImageSetDefaults(&mut rgb, image);
                                                rgb.format = libavif_sys::AVIF_RGB_FORMAT_RGBA;
                                                rgb.depth = 8;
                                                libavif_sys::avifRGBImageAllocatePixels(&mut rgb);
                                                libavif_sys::avifImageYUVToRGB(image, &mut rgb);
                                                
                                                let size = [width as usize, height as usize];
                                                if frames_data.is_empty() { aspect = size[0] as f32 / size[1] as f32; }
                                                
                                                let mut packed_pixels = Vec::with_capacity(size[0] * size[1] * 4);
                                                let pixel_slice = std::slice::from_raw_parts(rgb.pixels, (rgb.rowBytes * height) as usize);
                                                for y in 0..height {
                                                    let src_offset = (y * rgb.rowBytes) as usize;
                                                    let src_row = &pixel_slice[src_offset..src_offset + (width * 4) as usize];
                                                    packed_pixels.extend_from_slice(src_row);
                                                }
                                                frames_data.push(egui::ColorImage::from_rgba_unmultiplied(size, &packed_pixels));
                                                delays.push((*decoder).imageTiming.duration);
                                                libavif_sys::avifRGBImageFreePixels(&mut rgb);
                                            }
                                        }
                                    }
                                    libavif_sys::avifDecoderDestroy(decoder);
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("AVIF open error: {}", e),
                }
            } else {
                if let Ok(img) = image::open(&path) {
                    let buffer = img.to_rgba8();
                    let size = [buffer.width() as usize, buffer.height() as usize];
                    if size[0] > 0 && size[1] > 0 {
                        aspect = size[0] as f32 / size[1] as f32;
                        frames_data.push(egui::ColorImage::from_rgba_unmultiplied(size, buffer.as_raw()));
                        delays.push(0.0);
                    }
                }
            }

            if !frames_data.is_empty() {
                let _ = tx.send(ImageLoadData {
                    frames: frames_data,
                    frame_delays: delays,
                    aspect_ratio: aspect,
                    path: Some(path_str),
                    target_block_id,
                });
            }
        });
    }

    fn find_free_rect(&self, start_pos: Vec2, size: Vec2) -> Vec2 {
        let mut pos = start_pos;
        let mut offset = 0.0;
        for _ in 0..10 {
            let candidate = Rect::from_min_size(pos.to_pos2(), size);
            if !self.blocks.iter().any(|b| b.rect.intersects(candidate)) { return pos; }
            offset += 50.0;
            pos += Vec2::splat(offset);
        }
        pos
    }

    fn save_session(&self) {
        if let Some(mut path) = FileDialog::new().add_filter("JSON", &["json"]).save_file() {
            if path.extension().is_none() {
                path.set_extension("json");
            }

            let session = Session {
                viewport: ViewportData {
                    pan: [self.viewport.pan.x, self.viewport.pan.y],
                    zoom: self.viewport.zoom,
                },
                blocks: self.blocks.iter().map(|b| BlockData {
                    id: b.id,
                    rect: [b.rect.min.x, b.rect.min.y, b.rect.max.x, b.rect.max.y],
                    chained: b.chained,
                    content: match &b.content {
                        BlockContent::Text { text } => BlockContentData::Text { text: text.clone() },
                        BlockContent::Image { path, counter, playing, .. } => BlockContentData::Image {
                            path: path.clone().unwrap_or_default(),
                            counter: *counter,
                            playing: *playing,
                        },
                    },
                }).collect(),
            };

            if let Ok(file) = File::create(path) {
                let _ = serde_json::to_writer_pretty(file, &session);
            }
        }
    }

    fn load_session(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
            if let Ok(file) = File::open(path) {
                if let Ok(session) = serde_json::from_reader::<_, Session>(BufReader::new(file)) {
                    self.viewport.pan = Vec2::new(session.viewport.pan[0], session.viewport.pan[1]);
                    self.viewport.zoom = session.viewport.zoom;
                    self.blocks.clear();

                    for b_data in session.blocks {
                        let rect = Rect::from_min_max(
                            Pos2::new(b_data.rect[0], b_data.rect[1]),
                            Pos2::new(b_data.rect[2], b_data.rect[3]),
                        );

                        let content = match b_data.content {
                            BlockContentData::Text { text } => BlockContent::Text { text },
                            BlockContentData::Image { path, counter, playing } => {
                                // Trigger async load
                                if !path.is_empty() {
                                    self.load_image_file(PathBuf::from(&path), egui::Context::default(), Some(b_data.id));
                                }
                                // Create placeholder
                                BlockContent::Image {
                                    frames: vec![],
                                    frame_delays: vec![],
                                    aspect_ratio: 1.0,
                                    playing,
                                    current_frame_idx: 0,
                                    last_frame_time: 0.0,
                                    counter,
                                    path: Some(path),
                                }
                            }
                        };

                        self.blocks.push(Block {
                            id: b_data.id,
                            rect,
                            content,
                            chained: b_data.chained,
                            selected: false,
                        });
                    }
                }
            }
        }
    }
}

impl BlockContent {
    fn as_text_mut(&mut self) -> Option<&mut String> {
        if let BlockContent::Text { text } = self { Some(text) } else { None }
    }
}
