# Agent Guidelines

## Commands
- **Build**: `cargo build`
- **Run**: `cargo run`
- **Test**: `cargo test`
- **Single Test**: `cargo test <test_name>`
- **Lint**: `cargo clippy`
- **Format**: `cargo fmt`

## Code Style
- **Formatting**: Standard Rust (rustfmt). Use 4 spaces for indentation.
- **Naming**: `Snake_case` for variables/functions, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- **Imports**: Group std lib first, then external crates.
- **Error Handling**: Use `Result` and `Option` idiomatically. Avoid `unwrap()` unless necessary; prefer `expect()` or `?`.
- **Structure**: Organize code with section comments (e.g., `// --- Data Structures ---`).
- **UI**: Uses `eframe`/`egui`. Keep UI logic in `update` method.

## Rules
- Follow existing patterns in `src/main.rs`.
- Ensure `cargo clippy` passes before committing.
