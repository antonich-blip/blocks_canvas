## user reqirements

1. 'ma_blocks' should be text/image whiteboard type blocks Rust (desktop) app that can run under
Linux/PopOs/Cosmic-latest/Wayland (and MacOs in future).
1. Canvas has unlimited scrolling (any block or group  of blocks can
be moved to any direction. Ideally canvas should have some
boundaries/size limits to prevent edge cases).
2. Blocks cannot overlap (including new spawned blocks,
grouped/'chained' blocks and resizing blocks inside and outside of
groups), except while being moved/dragged.
3. blocks can be images and editable text (could add sound, drawing,
arrows in future like in mind mapping app).
4. to edit text block you have to double click inside it and to quit edit mode 
you have to click anywhere outside of the text area or press 'esc' key
4. blocks could have different shapes (future addon)
4. gif image block can play gif's animation which can be toggled on
left mouse click.
5. image blocks spawn with original ratio and when resized should keep
this ratio.
6. block resizing/moving speed is sinchronized with mouse movement
speed in real time.
7. block can be resized on right mouse hold from anywere
inside of current block (defaulted to closest corner)
7. left mouse button hold can move/drag blocks around. right mouse
button hold can resize any current block.
8. on top of the canvas should be: 'add text block', 'add image' bottons 
(more buttons could be added later).
9. each block should have two buttons in top right corner that should
become visible on mouse hower:
'close'(x) and 'chain'(some chain/unchain symbol that toggles on each click)
10. currently selected blocks with 'chain' button are automatically chained with each other.
11. (in future) dark and light color themes support (can be dark
initially)
8. (in future) app ability to save current session to file and to
convert it to .md,.pdf,.jpeg formats.
17. mappings: 
- LMB + click outside of text area: exit edit mode (same as 'esc' key)
- LMB + click inside of gif image block: start/stop animation
- LMB + hold + drag inside any block: move block aroud canvas.
- MMB + hold  + drag anywhere: move background with al objects on it.
- MMB + roll up/down: zoom background with everythig else (text size, 
blocks sizes, image sizes while keeping original ratio)
- RBM + click: no mapping
- RMB + hold + drag: resize image/text block (text size should stay 
the same but image should scale while preserving original ratio)





### Project Plan: Canvas Blocks Rust Desktop App

#### Tech Stack
*   **GUI Framework:** `egui` + `eframe` (winit backend) – Wayland-native, cross-platform (Linux/Mac).
*   **Image Handling:** `image` crate (load/resize), `gif` crate (animation).
*   **Text Editing:** `egui` built-in text editing.
*   **Collision:** Custom AABB collision resolution with iterative solver.
*   **Build:** `cargo`, desktop-only target.

#### Current Status (Completed)
*   **Framework:** `eframe` setup with infinite scrolling (pan) and zooming.
*   **Blocks:** Text and Image blocks (static & GIF) spawning.
*   **Interactions:**
    *   LMB Drag to move.
    *   RMB Drag (from anywhere in block) to resize (preserves aspect ratio for images).
    *   Double-click text to edit.
    *   Click GIF to toggle animation.
*   **Collision:** Basic non-overlapping logic.

#### Development Phases

**Phase 1: Interaction Refinement (Immediate Priority)**
*Goal: Polish existing features to strictly match User Requirements.*
1.  **Strict Collision Logic:** Tune `COLLISION_GAP` and solver iterations to prevent "bounciness" and ensure blocks snap to valid positions on release.
2.  **Per-Block Controls:**
    *   Ensure 'Close' (x) and 'Chain' (o) buttons are strictly in the top-right corner.
    *   Implement "Chain" toggle logic: clicking 'o' toggles the block's participation in a chained group.
3.  **Mouse Mapping Verification:**
    *   Verify RMB click does nothing.
    *   Verify MMB scroll zooms the entire canvas (background + blocks).
    *   Verify RMB drag works smoothly from any point inside the block.

**Phase 2: Advanced Grouping & Chaining**
*Goal: Robust implementation of Requirements 8, 9, and 10.*
1.  **Chaining Logic:**
    *   Selection of one chained block selects the whole group.
    *   Moving one chained block moves the whole group.
    *   Resizing remains individual unless specified otherwise.
2.  **Canvas Boundaries:** Implement "soft" boundaries (e.g., 10k x 10k) to prevent blocks from being lost.

**Phase 3: Visual Polish & Themes**
*Goal: Requirement 11 and UI/UX.*
1.  **Themes:** Add Light/Dark mode toggle in toolbar.
2.  **Toolbar Styling:** Improve button styling/icons. Ensure toolbar remains fixed on top.

**Phase 4: Persistence & Export (Future)**
*Goal: Requirement 8 (Save/Export).*
1.  **Save/Load:** Serialize `Vec<Block>` to JSON.
2.  **Export:**
    *   Render canvas to `.jpg` / `.png`.
    *   Export text/structure to `.md`.

#### Risks & Mitigations
*   **Wayland Input:** Continue testing on PopOS Cosmic.
*   **Performance:** Implement culling for off-screen blocks (already partially implemented).
*   **GIF Performance:** Optimize frame decoding if many GIFs are active.
