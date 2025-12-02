use eframe::egui;
use egui::{Align2, Color32, Pos2, Rect, Stroke, Vec2};
use rfd::FileDialog;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use uuid::Uuid;

const COLLISION_GAP: f32 = 2.0; // Small gap between blocks
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
}

#[derive(Clone)]
struct ImageLoadData {
    frames: Vec<egui::ColorImage>,
    frame_delays: Vec<f64>,
    aspect_ratio: f32,
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
        }
    }
}

// --- Physics / Collision Helpers ---

impl Block {
    /// Resolves collisions by pushing `self` out of `others`.
    /// Returns true if a position change occurred.
    fn resolve_collision(&mut self, others: &[Block]) -> bool {
        let mut moved = false;

        // We run a few iterations to stabilize chain reactions
        for _ in 0..3 {
            let mut total_push = Vec2::ZERO;
            let my_rect = self.rect.expand(COLLISION_GAP); // Expand slightly to maintain gap

            for other in others {
                if self.id == other.id {
                    continue;
                }

                if my_rect.intersects(other.rect) {
                    // Calculate Minimal Translation Vector (MTV)
                    let intersection = my_rect.intersect(other.rect);

                    let dx = intersection.width();
                    let dy = intersection.height();

                    let center_diff = self.rect.center() - other.rect.center();

                    // Create a push vector based on the shallowest axis
                    let push = if dx < dy {
                        Vec2::new(if center_diff.x > 0.0 { dx } else { -dx }, 0.0)
                    } else {
                        Vec2::new(0.0, if center_diff.y > 0.0 { dy } else { -dy })
                    };

                    total_push += push;
                }
            }

            // Only apply if there is a collision
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
        // Poll for loaded image data and create textures
        while let Ok(data) = self.image_rx.try_recv() {
            if data.frames.is_empty() {
                continue;
            }
            let id = Uuid::new_v4();
            let texture_frames: Vec<_> = data.frames.iter().enumerate().map(|(i, img)| {
                ctx.load_texture(
                    format!("img-{id}-{i}"),
                    img.clone(),
                    egui::TextureOptions::default(),
                )
            }).collect();
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
                },
                chained: false,
                selected: false,
            });
        }

        // Repaint constantly to keep physics/animations smooth
        if !self.blocks.is_empty() {
            ctx.request_repaint();
        }
        let time_now = ctx.input(|i| i.time);

        // 1. Update Animation State (GIFs)
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

        // 2. Handle Global Inputs (Pan/Zoom)
        let input = ctx.input(|i| i.clone());

        // Zoom (Ctrl + Scroll) or (MMB + Scroll)
        if input.raw_scroll_delta.y.abs() > 0.0 {
            let factor = 1.0 + input.raw_scroll_delta.y * 0.001;
            let old_zoom = self.viewport.zoom;
            self.viewport.zoom = (self.viewport.zoom * factor).clamp(0.1, 5.0);

            // Zoom towards mouse cursor
            if let Some(mouse_pos) = input.pointer.hover_pos() {
                let screen_center = ctx.screen_rect().center().to_vec2();
                let mouse_offset = mouse_pos.to_vec2() - screen_center;
                // Adjust pan to keep mouse over same world point
                let world_point_under_mouse = (mouse_offset / old_zoom) - self.viewport.pan;
                self.viewport.pan = (mouse_offset / self.viewport.zoom) - world_point_under_mouse;
            }
        }

        // Pan (Middle Mouse or Space+Drag)
        if input.pointer.middle_down()
            || (input.key_down(egui::Key::Space) && input.pointer.primary_down())
        {
            self.viewport.pan += input.pointer.delta() / self.viewport.zoom;
        }

        // 3. Top Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("🔤").on_hover_text("Add Text").clicked() {
                    self.spawn_text_block(ui.ctx());
                }
                if ui.button("🖼").on_hover_text("Add Image").clicked() {
                    self.spawn_image_block(ui.ctx());
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("LMB: Move/Toggle GIF | RMB: Resize | Scroll: Zoom | MMB: Pan");
                    ui.separator();
                });
            });
        });

        // 4. Main Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            // Draw specific background color
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, Color32::from_rgb(30, 30, 30));

            self.process_canvas(ui);
        });
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

        // --- 1. Resize Logic (Right Mouse Hold) ---

        if secondary_pressed {
            // Check if we clicked inside a block to start resizing
            if let Some(m_pos) = mouse_pos {
                // Convert mouse to world
                let world_mouse = (m_pos.to_vec2() - screen_center) / zoom - pan;

                // Find clicked block (iterate reverse to find top-most)
                if let Some(block) = self
                    .blocks
                    .iter()
                    .rev()
                    .find(|b| b.rect.contains(world_mouse.to_pos2()))
                {
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

                    // Track resized block as "dragged" for collision resolution
                    self.last_dragged_id = Some(block.id);
                }
            }
        }

        if secondary_released {
            self.resizing_state = None;
        }

        // Apply Resize
        if let Some(state) = &self.resizing_state {
            if let Some(curr_mouse_pos) = mouse_pos {
                if let Some(idx) = self.blocks.iter().position(|b| b.id == state.id) {
                    // Calculate world delta based strictly on mouse movement vs start
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

                    // Enforce Min Size
                    if new_rect.width() < min_size {
                        if state.handle == ResizeHandle::TopLeft
                            || state.handle == ResizeHandle::BottomLeft
                        {
                            new_rect.min.x = new_rect.max.x - min_size;
                        } else {
                            new_rect.max.x = new_rect.min.x + min_size;
                        }
                    }
                    if new_rect.height() < min_size {
                        if state.handle == ResizeHandle::TopLeft
                            || state.handle == ResizeHandle::TopRight
                        {
                            new_rect.min.y = new_rect.max.y - min_size;
                        } else {
                            new_rect.max.y = new_rect.min.y + min_size;
                        }
                    }

                    // Correct Aspect Ratio for Images
                    if let BlockContent::Image { aspect_ratio, .. } = self.blocks[idx].content {
                        let w = new_rect.width();
                        new_rect.set_height(w / aspect_ratio);
                    }

                    // Apply
                    self.blocks[idx].rect = new_rect;
                }
            }
        }

        // --- 2. Render & Interaction Loop ---

        let mut ids_to_delete = HashSet::new();
        let mut interact_captured = false;

        // Queue of movements: (block_index, world_delta)
        let mut pending_move = None;

        for i in 0..self.blocks.len() {
            // Extract metadata so we don't hold a borrow of self.blocks
            let b_id = self.blocks[i].id;
            let b_rect = self.blocks[i].rect;
            let b_selected = self.blocks[i].selected;
            let b_chained = self.blocks[i].chained;

            // Check editing status locally
            let is_editing = self.editing_id == Some(b_id);

            // Transform World -> Screen
            let screen_pos_min = screen_center + (b_rect.min.to_vec2() + pan) * zoom;
            let screen_size = b_rect.size() * zoom;
            let screen_rect = Rect::from_min_size(screen_pos_min.to_pos2(), screen_size);

            // Culling
            if !screen_rect.intersects(screen_rect) {
                continue;
            }

            // Draw Shadow/Background
            let border_color = if b_selected {
                Color32::YELLOW
            } else if b_chained {
                Color32::GREEN
            } else {
                Color32::BLACK
            };
            let bg_color = Color32::from_rgb(240, 240, 240);

            ui.painter().rect_filled(screen_rect, 5.0, bg_color);
            ui.painter()
                .rect_stroke(screen_rect, 5.0, Stroke::new(2.0, border_color));

            // Unique ID for interaction
            let interact_id = ui.make_persistent_id(b_id);

            // Interaction Logic
            let sense = if is_editing {
                egui::Sense::hover()
            } else {
                egui::Sense::click_and_drag()
            };

            // We place an invisible button over the block area to catch inputs
            let response = ui.interact(screen_rect, interact_id, sense);

            if response.hovered() || response.dragged() {
                interact_captured = true;
            }

            // --- Calculate Button Rects Early ---
            let btn_size = 16.0 * zoom;
            let padding = 4.0 * zoom;
            let top_right = screen_rect.right_top();

            let close_rect_center =
                top_right + Vec2::new(-btn_size / 2.0 - padding, btn_size / 2.0 + padding);
            let close_rect = Rect::from_center_size(close_rect_center, Vec2::splat(btn_size));

            let chain_rect_center = close_rect_center - Vec2::new(btn_size + padding, 0.0);
            let chain_rect = Rect::from_center_size(chain_rect_center, Vec2::splat(btn_size));

            let close_hovered = if let Some(ptr) = mouse_pos {
                close_rect.contains(ptr)
            } else {
                false
            };
            let chain_hovered = if let Some(ptr) = mouse_pos {
                chain_rect.contains(ptr)
            } else {
                false
            };

            // Drag Move Logic captured here, applied after loop
            if response.dragged() && !secondary_down && !ui.input(|i| i.pointer.middle_down()) {
                let delta = response.drag_delta() / zoom;
                pending_move = Some((i, delta));
                self.last_dragged_id = Some(b_id);
            }

            // Render Content
            if is_editing {
                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(screen_rect.shrink(4.0))
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );

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
                match &self.blocks[i].content {
                    BlockContent::Text { text } => {
                        ui.painter().text(
                            screen_rect.min + Vec2::splat(5.0),
                            Align2::LEFT_TOP,
                            text,
                            egui::FontId::proportional(16.0 * zoom),
                            Color32::BLACK,
                        );

                        if response.double_clicked() && !close_hovered && !chain_hovered {
                            self.editing_id = Some(b_id);
                            self.focus_request = Some(b_id);
                        }
                    }
                    BlockContent::Image {
                        frames,
                        current_frame_idx,
                        playing: _playing,
                        ..
                    } => {
                        if let Some(tex) = frames.get(*current_frame_idx) {
                            ui.painter().image(
                                tex.id(),
                                screen_rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }

                        if response.clicked() && !close_hovered && !chain_hovered {
                            if let BlockContent::Image { playing, .. } = &mut self.blocks[i].content
                            {
                                *playing = !*playing;
                            }
                        }
                    }
                }
            }

            // Hover Overlays (Close / chain)
            if response.hovered() || response.dragged() || b_chained {
                let close_col = if close_hovered {
                    Color32::from_rgb(255, 100, 100)
                } else {
                    Color32::RED
                };
                ui.painter()
                    .circle_filled(close_rect.center(), btn_size / 2.0, close_col);
                ui.painter().text(
                    close_rect.center(),
                    Align2::CENTER_CENTER,
                    "x",
                    egui::FontId::monospace(12.0 * zoom),
                    Color32::WHITE,
                );

                let link_col = if b_chained {
                    Color32::GREEN
                } else if chain_hovered {
                    Color32::LIGHT_GRAY
                } else {
                    Color32::GRAY
                };

                ui.painter()
                    .circle_filled(chain_rect.center(), btn_size / 2.0, link_col);
                ui.painter().text(
                    chain_rect.center(),
                    Align2::CENTER_CENTER,
                    "o",
                    egui::FontId::monospace(12.0 * zoom),
                    Color32::WHITE,
                );

                // Handle Button Clicks via response.clicked()
                if response.clicked() {
                    if close_hovered {
                        ids_to_delete.insert(b_id);
                    } else if chain_hovered {
                        self.blocks[i].chained = !self.blocks[i].chained;
                        self.last_chain_interaction = ui.input(|i| i.time);
                    }
                }
            }
        }

        // --- 3. Post-Loop Updates ---

        if let Some((idx, delta)) = pending_move {
            let mut moved_indices = vec![idx];
            if self.blocks[idx].chained {
                self.last_chain_interaction = ui.input(|i| i.time);
                for (i, b) in self.blocks.iter().enumerate() {
                    if i != idx && b.chained {
                        moved_indices.push(i);
                    }
                }
            }

            for &i in &moved_indices {
                self.blocks[i].rect = self.blocks[i].rect.translate(delta);
            }
        }

        if ui.input(|i| {
            i.pointer.button_released(egui::PointerButton::Primary)
                || i.pointer.button_released(egui::PointerButton::Secondary)
        }) {
            if let Some(dragged_id) = self.last_dragged_id {
                if let Some(idx) = self.blocks.iter().position(|b| b.id == dragged_id) {
                    let others = self.blocks.clone();
                    self.blocks[idx].resolve_collision(&others);

                    if self.blocks[idx].chained {
                        for i in 0..self.blocks.len() {
                            if self.blocks[i].chained && i != idx {
                                self.blocks[i].resolve_collision(&others);
                            }
                        }
                    }
                }
                self.last_dragged_id = None;
            }
        }

        self.blocks.retain(|b| !ids_to_delete.contains(&b.id));

        if ui.input(|i| i.pointer.any_click()) && !interact_captured && !secondary_down {
            self.editing_id = None;
            for b in &mut self.blocks {
                b.selected = false;
            }
        }

        let time_now = ui.input(|i| i.time);
        let any_chained = self.blocks.iter().any(|b| b.chained);
        if any_chained {
            if time_now - self.last_chain_interaction > 10.0 {
                for b in &mut self.blocks {
                    b.chained = false;
                }
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(
                        (10.0 - (time_now - self.last_chain_interaction)).max(0.0),
                    ));
            }
        }
    }

    // --- helpers ---

    fn spawn_text_block(&mut self, ctx: &egui::Context) {
        // Find center of current view
        let _rect = ctx.screen_rect();
        let center_world = -self.viewport.pan;

        let size = Vec2::new(200.0, 100.0);
        let pos = self.find_free_rect(center_world, size);

        self.blocks.push(Block {
            id: Uuid::new_v4(),
            rect: Rect::from_min_size(pos.to_pos2(), size),
            content: BlockContent::Text {
                text: "Double click to edit...".to_string(),
            },
            chained: false,
            selected: false,
        });
    }

    fn spawn_image_block(&mut self, ctx: &egui::Context) {
        if let Some(path) = FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "gif", "avif"])
            .pick_file()
        {
            let _ctx = ctx.clone();
            let tx = self.image_tx.clone();

            thread::spawn(move || {
                let is_gif = path.extension().is_some_and(|e| e.to_string_lossy().to_lowercase() == "gif");
                let mut frames_data = vec![];
                let mut delays = vec![];
                let mut aspect = 1.0;

                if is_gif {
                    // Load animated GIF
                    match File::open(&path) {
                        Ok(file) => {
                            let mut decoder = gif::DecodeOptions::new();
                            decoder.set_color_output(gif::ColorOutput::RGBA);
                            match decoder.read_info(BufReader::new(file)) {
                                Ok(mut decoder) => {
                                    while let Some(frame) = decoder.read_next_frame().unwrap_or(None) {
                                        let size = [frame.width as usize, frame.height as usize];
                                        if frames_data.is_empty() {
                                            aspect = size[0] as f32 / size[1] as f32;
                                        }
                                        frames_data.push(egui::ColorImage::from_rgba_unmultiplied(size, &frame.buffer));
                                        delays.push(frame.delay as f64 / 100.0); // GIF delay is in 1/100s
                                    }
                                }
                                Err(e) => eprintln!("GIF decoder error {:?}: {}", path.display(), e),
                            }
                        }
                        Err(e) => eprintln!("GIF open error {:?}: {}", path.display(), e),
                    }
                } else {
                    // Load static image (PNG, JPG, AVIF, etc.)
                    match image::open(&path) {
                        Ok(img) => {
                            let buffer = img.to_rgba8();
                            let size = [buffer.width() as usize, buffer.height() as usize];
                            aspect = size[0] as f32 / size[1] as f32;
                            frames_data.push(egui::ColorImage::from_rgba_unmultiplied(size, buffer.as_raw()));
                            delays.push(0.0);
                            eprintln!("Decoded static image: {:?}", path.display());
                        }
                        Err(e) => eprintln!("Static image decode error {:?}: {}", path.display(), e),
                    }
                }

                if !frames_data.is_empty() {
                    let frame_count = frames_data.len();
                    let _ = tx.send(ImageLoadData {
                        frames: frames_data,
                        frame_delays: delays,
                        aspect_ratio: aspect,
                    });
                    eprintln!("Sent {} frames for {:?}", frame_count, path.display());
                } else {
                    eprintln!("No frames decoded for {:?}", path.display());
                }
            });
        }
    }

    /// Naively finds a spot that doesn't overlap drastically
    fn find_free_rect(&self, start_pos: Vec2, size: Vec2) -> Vec2 {
        let mut pos = start_pos;
        let mut offset = 0.0;

        // Spiral search outward
        for _ in 0..10 {
            let candidate = Rect::from_min_size(pos.to_pos2(), size);
            let collision = self.blocks.iter().any(|b| b.rect.intersects(candidate));
            if !collision {
                return pos;
            }

            offset += 50.0;
            pos += Vec2::splat(offset);
        }
        pos // Give up and return overlapped
    }
}

// Helper trait to mutate enum content
impl BlockContent {
    fn as_text_mut(&mut self) -> Option<&mut String> {
        if let BlockContent::Text { text } = self {
            Some(text)
        } else {
            None
        }
    }
}
