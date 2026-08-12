# Repository Guidelines

## Project Structure & Module Organization

The React frontend lives in `src/`. Reusable UI is in `src/components/`, page-level work is grouped under `src/features/`, Zustand state belongs in `src/stores/`, and shared Tauri bindings, formatters, and TypeScript contracts live in `src/lib/` and `src/types/`.

The Rust backend lives in `src-tauri/src/`. Keep Tauri entry points narrow in `commands/`, engine-specific logic in `engines/`, queue and child-process ownership in `queue/`, persistence in `database/`, and shared DTOs in `models.rs`. SQLite migrations are append-only files under `src-tauri/src/database/migrations/`. Application icons are in `src-tauri/icons/`. Read `docs/ARCHITECTURE.md` before changing subsystem boundaries; planned work is tracked in `docs/ROADMAP.md`.

## Build, Test, and Development Commands

- `npm install`: install pinned frontend and Tauri CLI dependencies.
- `npm run tauri:dev`: launch the complete desktop application with hot reload.
- `npm run typecheck`: run strict TypeScript checks without emitting files.
- `npm run build`: type-check and build the Vite frontend.
- `cd src-tauri && cargo test --all-targets`: run Rust unit and persistence tests.
- `cd src-tauri && cargo clippy --all-targets -- -D warnings`: enforce warning-free Rust.
- `npm run tauri:build`: produce the native desktop binary.

Before submitting changes, also run `cargo fmt --all -- --check` from `src-tauri/`.

## Coding Style & Naming Conventions

Use two-space indentation in TypeScript/TSX and standard `rustfmt` formatting in Rust. React components and TypeScript interfaces use `PascalCase`; functions, hooks, and Zustand actions use `camelCase`. Rust modules and functions use `snake_case`, while serializable DTO fields use `#[serde(rename_all = "camelCase")]` to match TypeScript.

Keep engine arguments as `Command` argument arrays. Never introduce shell-string concatenation or let React execute processes directly.

## Testing Guidelines

Rust tests are colocated in `#[cfg(test)]` modules. Name tests after behavior, such as `parses_structured_progress`. Add coverage for validation, metadata normalization, argument construction, migrations, and queue state changes. A frontend test runner is not configured yet; always run type checking and the production Vite build for UI changes.

## Commit & Pull Request Guidelines

Follow the existing Conventional Commit style, for example `feat: add playlist analysis` or `fix: reap cancelled child processes`. Keep commits focused. Pull requests should explain the user impact and architecture changes, list validation commands, link relevant issues, and include screenshots for visible UI changes. Call out security-sensitive changes to URL, path, process, or Tauri permission handling.
