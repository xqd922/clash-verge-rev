# Project Structure

## Root Directory Organization

```
├── src/                    # Frontend React application
├── src-tauri/             # Rust backend (Tauri application)
├── scripts/               # Build and release automation scripts
├── scripts-workflow/      # CI/CD workflow scripts
├── docs/                  # Documentation and preview images
├── .kiro/                 # Kiro IDE configuration and steering
├── .github/               # GitHub workflows and templates
├── .husky/                # Git hooks (pre-commit, pre-push)
└── .devcontainer/         # Development container configuration
```

## Frontend Structure (`src/`)

### Core Application Files

- `main.tsx` - Application entry point and Tauri setup
- `App.tsx` - Root component with routing and providers
- `index.html` - HTML template

### Feature-Based Organization

- `pages/` - Top-level route components (home, settings, profiles, etc.)
- `components/` - Reusable UI components organized by feature
  - `base/` - Generic base components
  - `common/` - Shared components across features
  - `layout/` - Layout and navigation components
  - `[feature]/` - Feature-specific components (proxy, profile, setting, etc.)

### Supporting Directories

- `hooks/` - Custom React hooks for state and side effects
- `services/` - API clients, IPC communication, and external services
- `providers/` - React context providers for global state
- `utils/` - Pure utility functions and helpers
- `assets/` - Static assets (fonts, images, styles)
- `locales/` - Internationalization JSON files
- `polyfills/` - Browser compatibility polyfills

## Backend Structure (`src-tauri/src/`)

### Core Application

- `main.rs` - Application entry point
- `lib.rs` - Library exports and module declarations

### Feature Modules

- `cmd/` - Tauri command handlers (IPC endpoints)
- `core/` - Core application logic (service management, system integration)
- `config/` - Configuration management (profiles, settings, runtime)
- `feat/` - Feature implementations (proxy, backup, window management)
- `enhance/` - Configuration enhancement (merge, script, field processing)

### Supporting Modules

- `utils/` - Utility functions and helpers
- `state/` - Application state management
- `ipc/` - Inter-process communication handlers
- `process/` - Process management and async handlers
- `module/` - Modular components (lightweight mode, system info)

## Configuration Files

### Frontend Configuration

- `vite.config.mts` - Vite build configuration with optimized chunking
- `tsconfig.json` - TypeScript compiler configuration
- `eslint.config.ts` - ESLint linting rules
- `.prettierrc` - Code formatting rules

### Backend Configuration

- `Cargo.toml` - Rust dependencies and build configuration
- `tauri.conf.json` - Tauri application configuration
- `build.rs` - Custom build script
- Platform-specific configs: `tauri.{windows,linux,macos}.conf.json`

### Development Tools

- `.editorconfig` - Editor configuration for consistent formatting
- `.tool-versions` - Development tool versions (asdf/mise)
- `renovate.json` - Dependency update automation
- `crowdin.yml` - Translation management

## Naming Conventions

### Files and Directories

- **React Components**: PascalCase (`ProfileCard.tsx`)
- **Hooks**: camelCase with `use` prefix (`useProfiles.ts`)
- **Utilities**: kebab-case (`parse-traffic.ts`)
- **Pages**: lowercase (`settings.tsx`)
- **Rust Files**: snake_case (`proxy_manager.rs`)

### Code Conventions

- **React Components**: PascalCase function components
- **TypeScript Interfaces**: PascalCase with descriptive names
- **Constants**: SCREAMING_SNAKE_CASE
- **Rust Functions**: snake_case
- **Rust Structs/Enums**: PascalCase

## Import/Export Patterns

### Frontend

- Use path aliases: `@/` for `src/`, `@root/` for project root
- Barrel exports in feature directories via `index.ts`
- Absolute imports preferred over relative for cross-feature dependencies

### Backend

- Module organization with `mod.rs` files
- Public API exposure through `lib.rs`
- Feature-based module grouping with clear boundaries

## Asset Organization

### Static Assets (`src/assets/`)

- `fonts/` - Custom font files
- `image/` - Application images and icons
- `styles/` - Global SCSS stylesheets

### Tauri Assets (`src-tauri/`)

- `icons/` - Application icons for different platforms
- `assets/` - Bundled resources
- `images/` - Backend-specific images
