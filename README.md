## User Requirements

1.  **Platform:** 'ma_blocks' is a text/image whiteboard-type Rust (desktop) app running on Linux/PopOs/Cosmic-latest/Wayland (and MacOS in future).
2.  **Canvas:** Unlimited scrolling (infinite canvas). Blocks can be moved in any direction.
3.  **Collision:** Blocks cannot overlap (including new spawned blocks, grouped/'chained' blocks, and resized blocks), except while being moved/dragged.
4.  **Block Types:**
    *   **Text:** Editable. Double-click on text to edit, click outside or press 'Esc' to exit edit mode.
    *   **Image:** Static images and GIFs.
    *   **GIFs:** Click to toggle animation.
    *   **Future:** Shapes, sound, drawing, arrows.
5.  **Resizing:**
    *   Image blocks spawn with original aspect ratio and maintain it during resize.
    *   Right Mouse Button (RMB) hold anywhere inside a block resizes it (defaulting to the closest corner).
    *   Resizing/moving is synchronized with mouse movement in real-time.
6.  **Controls & Mappings:**
    *   **LMB + Drag:** Move blocks.
    *   **RMB + Drag:** Resize blocks.
    *   **MMB + Drag:** Pan the canvas.
    *   **Scroll:** Zoom in/out (zooms background and all objects, text size).
    *   **LMB + Click (GIF):** Toggle animation.
    *   **Double Click (Text):** Edit text.
    *   **Counter Tool Active:**
        *   **LMB (Image):** Add/Increment counter.
        *   **RMB (Image):** Decrement counter.
7.  **Toolbar:** Located at the top. Contains:
    *   **Save/Load:** Session persistence.
    *   **Add Text/Image:** Spawn blocks.
    *   **Counter:** Toggle counting mode.
    *   **Reset:** Reset all image counters to zero.
    *   **Help:** Show controls.
8.  **Block UI:**
    *   Top-right corner buttons (visible on hover/interaction):
        *   'x': Close/Delete block.
        *   'o': Chain/Unchain block.
9.  **Chaining (Grouping):**
    *   Blocks with the 'chain' ('o') toggled on (green border) move together as a group.
    *   **Auto-Unchain:** If a chained group is inactive for 10 seconds, it automatically unchains (reverts to individual blocks).
10. **Tools:**
    *   **Counter:** Visual counting on images (Green circle with number).
    *   **Reset:** Reset all image counters to zero.
11. **Persistence:**
    *   **Save/Load:** Save canvas state to JSON (preserves text, image paths, counters).
12. **Future Features:**
    *   Dark/Light themes.
    *   Export to .md, .pdf, .jpeg.

---

### Build Instructions

#### Prerequisites

To build the application, you need the following system dependencies.

**Linux (Debian/Ubuntu/Pop!_OS):**
Essential build tools, graphics/windowing libraries, and AVIF support:
```bash
sudo apt install build-essential libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libavif-dev nasm
```
*Note: `libgtk-3-dev` is required for the file dialogs (`rfd`), and `libxkbcommon-dev` is needed for windowing (`eframe`).*

**macOS:**
1.  Install Xcode Command Line Tools (if not already installed):
    ```bash
    xcode-select --install
    ```
2.  Install AVIF dependencies via Homebrew:
    ```bash
    brew install libavif nasm
    ```

#### Running the App
```bash
cargo run
```

### Project Plan: Canvas Blocks Rust Desktop App

#### Tech Stack
*   **GUI Framework:** `egui` + `eframe` (winit backend) – Wayland-native, cross-platform.
*   **Image Handling:** `image` crate (load/resize), `gif` crate (animation).
*   **Text Editing:** `egui` built-in text editing.
*   **Collision:** Custom AABB collision resolution with iterative solver.
*   **Build:** `cargo`, desktop-only target.

#### Current Status (Completed - Phase 2)
*   **Framework:** `eframe` setup with infinite scrolling (pan) and zooming.
*   **Blocks:** Text and Image blocks (static & GIF) spawning.
*   **Interactions:**
    *   LMB Drag to move.
    *   RMB Drag (from anywhere in block) to resize (preserves aspect ratio for images).
    *   Double-click text to edit.
    *   Click GIF to toggle animation.
    *   **Zoom:** Mouse wheel scrolls to zoom.
    *   **Pan:** MMB drag or Space + LMB drag.
*   **Collision:** Robust non-overlapping logic. Blocks push each other out (already positioned blocks 'keep' their positions) of the way when released.
*   **Chaining:**
    *   Toggle 'o' button to chain blocks.
    *   Moving one chained block moves all others.
    *   **Auto-Unchain:** 10-second inactivity timer automatically unchains blocks.
*   **Persistence:**
    *   Save session to JSON.
    *   Load session from JSON.
*   **Tools:**
    *   **Counter:** Visual counting on images.
    *   **Reset:** Reset all image counters to zero.
    *   **Help:** In-app help window.
*   **UI:**
    *   Hover buttons ('x', 'o') working correctly.
    *   Toolbar with icons for all actions.

#### Development Phases

**Phase 1: Interaction Refinement (Completed)**
*   *Goal: Polish existing features to strictly match User Requirements.*
*   Implemented strict collision logic.
*   Implemented per-block controls ('x', 'o').
*   Refined mouse mappings (Zoom on scroll, RMB resize).
*   Implemented basic chaining with auto-unchain timeout.

**Phase 2: Persistence & Tools (Completed)**
*   *Goal: Robust grouping, Save/Load, and Utility Tools.*
*   **Persistence:**
    *   Save current session to JSON.
    *   Load session from file.
*   **Tools:**
    *   Counter tool for images.
    *   Reset tool to clear all counters.
    *   Help window.

**Phase 3: Visual Polish & Themes (Future)**
*   *Goal: Requirement 11 and UI/UX.*
*   **Themes:** Add Light/Dark mode toggle.
*   **Export:** Render canvas to image/PDF.

#### Risks & Mitigations
*   **Wayland Input:** Tested on PopOS Cosmic; seems stable.
*   **Performance:** Culling implemented for off-screen blocks.

LMB: Move/Toggle_GIF | RMB: Resize | Scroll: Zoom | MMB: Pan
