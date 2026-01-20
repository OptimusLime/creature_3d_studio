# Phase 4.5 Specification: iOS App Build

*Following HOW_WE_WORK.md and WRITING_MILESTONES.md.*

**Key principle:** Get the map editor running on iPad with minimal friction.

---

## Why This Phase Exists

Phase 4 delivered a complete asset system with database storage, semantic search, browser UI, and file watcher. The map editor is feature-rich on desktop.

But we want to:
1. **Validate mobile rendering** - Ensure our Bevy/wgpu pipeline works on iOS Metal
2. **Enable tablet-based testing** - iPad is a natural canvas for 2D map editing
3. **Prove cross-platform capability** - iOS build infrastructure enables future App Store releases

---

## Phase Outcome

**When Phase 4.5 is complete, I can:**
- Run `./scripts/build-ios.sh --device` and see the map editor on my iPad
- Touch to pan the map view
- See the same 2D terrain rendering as desktop

**Phase Foundation:**
1. `mobile/` directory with Xcode project and build scripts
2. iOS feature flags that disable desktop-only features (MCP server, file watcher)
3. `scripts/build-ios.sh` for one-command builds
4. Environment-based code signing configuration

---

## Dependency Assessment

### Low Risk (Should Just Work)

| Dependency | iOS Status | Notes |
|------------|------------|-------|
| **bevy 0.17** | ✅ Supported | Official mobile example uses 0.17.3 |
| **wgpu 26** | ✅ Supported | Metal backend for iOS |
| **mlua (vendored)** | ✅ Should work | Compiles Lua from C source |
| **rusqlite (bundled)** | ✅ Should work | Compiles SQLite from C source |
| **bevy_mod_imgui** | ✅ Should work | Uses wgpu, which supports Metal |
| **imgui** | ✅ Should work | Touch input may need mapping |

### Medium Risk (May Need Conditional Compilation)

| Dependency | iOS Status | Mitigation |
|------------|------------|------------|
| **candle-core/nn/transformers** | ⚠️ May fail | Disable embedding service on iOS |
| **tokenizers** | ⚠️ C++ deps | Disable with candle |
| **hf-hub** | ⚠️ Path issues | Disable with candle |

### Will Disable on iOS

| Dependency | Reason | Approach |
|------------|--------|----------|
| **tiny_http** | No MCP server on mobile | `#[cfg(not(target_os = "ios"))]` |
| **notify** | No file watcher on mobile | `#[cfg(not(target_os = "ios"))]` |
| **EmbeddingService** | ML deps too complex | Feature flag `embedding` |

---

## Architecture: iOS Feature Flags

```rust
// In Cargo.toml
[features]
default = ["desktop"]
desktop = ["embedding", "mcp-server", "file-watcher"]
mobile = []  # Minimal feature set for iOS
embedding = ["candle-core", "candle-nn", "candle-transformers", "tokenizers", "hf-hub"]
mcp-server = ["tiny_http"]
file-watcher = ["notify"]

// In code
#[cfg(feature = "mcp-server")]
mod mcp_server;

#[cfg(feature = "file-watcher")]
mod watcher;
```

This allows:
- Desktop: `cargo build` (default features)
- iOS: `cargo build --no-default-features --features mobile`

---

## Build Infrastructure

### Directory Structure

```
mobile/
├── .env                    # Team ID, bundle ID (gitignored)
├── .env.example            # Template for .env
├── Cargo.toml              # iOS-specific dependencies
├── Makefile                # Build targets
├── build_rust_deps.sh      # Xcode build script
├── src/
│   └── lib.rs              # iOS entry point with #[bevy_main]
├── ios-src/
│   └── Info.plist          # iOS app metadata
└── creature_studio.xcodeproj/
    └── project.pbxproj     # Xcode project (auto-generated paths)

scripts/
└── build-ios.sh            # One-command build from project root
```

### Environment Configuration

```bash
# mobile/.env (gitignored)
DEVELOPMENT_TEAM=SJ947BUCZ5
PRODUCT_BUNDLE_IDENTIFIER=studio.atelico.rust-ios
```

The Xcode project reads these via the build script.

---

## Milestone Details

### M15: iOS Build Infrastructure

**Functionality:** I can build a minimal Bevy app and run it on the iOS simulator.

**Foundation:** `mobile/` directory with complete Xcode project, build scripts, and Makefile. Reusable for all future iOS builds.

#### What Gets Created

1. **`mobile/Cargo.toml`** - Workspace member that depends on `studio_core` with mobile features
2. **`mobile/src/lib.rs`** - Minimal Bevy app with `#[bevy_main]` macro
3. **`mobile/Makefile`** - Targets for simulator and device builds
4. **`mobile/build_rust_deps.sh`** - Xcode build phase script (adapted from Bevy example)
5. **`mobile/ios-src/Info.plist`** - iOS app configuration
6. **`mobile/creature_studio.xcodeproj/`** - Xcode project with:
   - Rust build phase calling `build_rust_deps.sh`
   - Asset bundle from `assets/` directory
   - Code signing from environment variables

#### Implementation Tasks

| # | Task | Verification |
|---|------|--------------|
| 1 | Create `mobile/` directory structure | Directory exists |
| 2 | Create `mobile/Cargo.toml` with bevy dependency | `cargo check` passes |
| 3 | Create `mobile/src/lib.rs` with minimal 3D scene | Compiles |
| 4 | Copy and adapt `build_rust_deps.sh` from Bevy example | Script exists |
| 5 | Copy and adapt `Makefile` from Bevy example | Makefile exists |
| 6 | Create `ios-src/Info.plist` | File exists |
| 7 | Create Xcode project with Rust build phase | `.xcodeproj` exists |
| 8 | Configure env-based code signing | Reads from `.env` |
| 9 | Test simulator build | `make run` shows 3D scene |

#### Verification

```bash
cd mobile
make run
# iOS Simulator boots
# Window shows rotating cube/sphere (Bevy default scene)
# Touch to pan works
```

---

### M16: Feature Flags for Mobile

**Functionality:** I can compile `studio_core` for iOS without desktop-only dependencies.

**Foundation:** Feature flags in `studio_core/Cargo.toml` that separate mobile-compatible code from desktop-only features.

#### What Gets Changed

1. **`studio_core/Cargo.toml`** - Add feature flags:
   - `embedding` (optional): candle-*, tokenizers, hf-hub
   - `mcp-server` (optional): tiny_http
   - `file-watcher` (optional): notify
   - `desktop` (default): all of the above
   - `mobile`: none of the above

2. **Conditional compilation** in:
   - `mcp_server.rs` - `#[cfg(feature = "mcp-server")]`
   - `watcher.rs` - `#[cfg(feature = "file-watcher")]`
   - `embedding.rs` - `#[cfg(feature = "embedding")]`
   - `app.rs` - Skip initialization of disabled features

#### Implementation Tasks

| # | Task | Verification |
|---|------|--------------|
| 1 | Add feature flags to `studio_core/Cargo.toml` | Compiles |
| 2 | Add `#[cfg]` to mcp_server module | Compiles without feature |
| 3 | Add `#[cfg]` to watcher module | Compiles without feature |
| 4 | Add `#[cfg]` to embedding module | Compiles without feature |
| 5 | Update `app.rs` to conditionally init features | Compiles |
| 6 | Test desktop build with default features | All tests pass |
| 7 | Test mobile build without features | `cargo build --no-default-features -p studio_core` |

#### Verification

```bash
# Desktop still works
cargo test -p studio_core

# Mobile feature set compiles
cargo build -p studio_core --no-default-features --features mobile

# No errors about missing tiny_http, notify, candle, etc.
```

---

### M17: Map Editor on iOS Simulator

**Functionality:** I can see the 2D map editor rendering on the iOS simulator with touch panning.

**Foundation:** `mobile/src/lib.rs` that initializes `MapEditor2DApp` with iOS-appropriate configuration.

#### What Gets Created/Changed

1. **`mobile/src/lib.rs`** - Full app initialization:
   - `MapEditor2DApp` with disabled features
   - iOS window configuration (fullscreen, no status bar)
   - Touch-to-pan input mapping
   - `WinitSettings::mobile()` for power efficiency

2. **`mobile/Cargo.toml`** - Depend on `studio_core` with mobile features

3. **Touch input** - Map touch drag to camera pan (similar to Bevy mobile example)

#### Implementation Tasks

| # | Task | Verification |
|---|------|--------------|
| 1 | Update `mobile/Cargo.toml` to depend on `studio_core` | Compiles |
| 2 | Create iOS-specific app initialization in `lib.rs` | Compiles |
| 3 | Configure iOS window (fullscreen, hidden status bar) | Renders correctly |
| 4 | Add touch-to-pan system | Can pan view |
| 5 | Disable imgui initially (verify rendering first) | Grid renders |
| 6 | Enable imgui if Metal works | UI visible |
| 7 | Test on simulator | Map editor visible, touch works |

#### Verification

```bash
cd mobile
make run
# iOS Simulator shows:
# - 2D terrain grid rendering
# - Touch drag pans the view
# - (Optionally) imgui panels visible
```

---

### M18: iPad Device Deployment

**Functionality:** I can deploy the map editor to my physical iPad with one command.

**Foundation:** Device build target in Makefile, code signing via environment variables.

#### What Gets Changed

1. **`mobile/Makefile`** - Add `ios-device` target:
   ```makefile
   ios-device:
   	IOS_TARGETS=aarch64-apple-ios xcodebuild -scheme creature_studio ...
   ```

2. **`mobile/build_rust_deps.sh`** - Read `DEVELOPMENT_TEAM` from environment

3. **Xcode project** - Configure automatic signing with team ID

#### Implementation Tasks

| # | Task | Verification |
|---|------|--------------|
| 1 | Add `ios-device` target to Makefile | Target exists |
| 2 | Update build script to use env vars | Reads DEVELOPMENT_TEAM |
| 3 | Configure Xcode automatic signing | No signing errors |
| 4 | Test device build | App installs on iPad |
| 5 | Test touch input on real hardware | Panning works smoothly |
| 6 | Document Developer Mode requirement | README updated |

#### Verification

```bash
# Ensure iPad connected and Developer Mode enabled
cd mobile
source .env
make ios-device
# iPad shows map editor
# Touch panning is responsive
```

---

### M19: One-Command Build Script

**Functionality:** I can build and deploy to iPad with `./scripts/build-ios.sh --device` from project root.

**Foundation:** `scripts/build-ios.sh` that handles the full pipeline with options for simulator vs device, debug vs release.

#### What Gets Created

**`scripts/build-ios.sh`:**
```bash
#!/bin/bash
# Usage: ./scripts/build-ios.sh [--simulator|--device] [--release]

# Check iOS targets installed
# Source .env from mobile/
# Run appropriate make target
# Report success/failure
```

#### Implementation Tasks

| # | Task | Verification |
|---|------|--------------|
| 1 | Create `scripts/build-ios.sh` | File exists, executable |
| 2 | Add `--simulator` flag | Builds for simulator |
| 3 | Add `--device` flag | Builds for device |
| 4 | Add `--release` flag | Release build works |
| 5 | Add target check (rustup) | Warns if targets missing |
| 6 | Add help text | `--help` shows usage |
| 7 | Test from project root | Builds successfully |

#### Verification

```bash
# From project root
./scripts/build-ios.sh --device
# Builds and deploys to connected iPad

./scripts/build-ios.sh --simulator
# Builds and runs on simulator

./scripts/build-ios.sh --help
# Shows usage information
```

---

## Files Changed Summary

### New Files

| File | Purpose |
|------|---------|
| `mobile/Cargo.toml` | iOS crate configuration |
| `mobile/src/lib.rs` | iOS entry point |
| `mobile/Makefile` | Build targets |
| `mobile/build_rust_deps.sh` | Xcode build script |
| `mobile/ios-src/Info.plist` | iOS app metadata |
| `mobile/creature_studio.xcodeproj/` | Xcode project |
| `mobile/.env` | Code signing config (gitignored) |
| `mobile/.env.example` | Template for .env |
| `scripts/build-ios.sh` | One-command build |

### Modified Files

| File | Change |
|------|--------|
| `crates/studio_core/Cargo.toml` | Add feature flags |
| `crates/studio_core/src/map_editor/mcp_server.rs` | Add `#[cfg]` |
| `crates/studio_core/src/map_editor/asset/watcher.rs` | Add `#[cfg]` |
| `crates/studio_core/src/map_editor/asset/embedding.rs` | Add `#[cfg]` |
| `crates/studio_core/src/map_editor/app.rs` | Conditional feature init |
| `.gitignore` | Add `mobile/.env` |
| `Cargo.toml` (workspace) | Add `mobile` member |

---

## Verification Summary

| Milestone | Verification Command | Expected Result |
|-----------|---------------------|-----------------|
| M15 | `cd mobile && make run` | Simulator shows Bevy 3D scene |
| M16 | `cargo build -p studio_core --no-default-features` | Compiles without desktop deps |
| M17 | `cd mobile && make run` | Simulator shows map editor grid |
| M18 | `cd mobile && make ios-device` | iPad shows map editor |
| M19 | `./scripts/build-ios.sh --device` | iPad shows map editor |

---

## Risk Mitigation

### If candle/ML fails to compile for iOS

**Mitigation:** Already planned - `embedding` is an optional feature disabled on mobile.

### If bevy_mod_imgui fails on Metal

**Mitigation:** Start M17 with imgui disabled. If it fails:
1. Skip imgui on iOS (render grid only)
2. Add `#[cfg(not(target_os = "ios"))]` to imgui systems
3. File issue upstream

### If touch input doesn't work

**Mitigation:** Bevy's touch input is well-tested on iOS. Use the same approach as the official mobile example.

### If rusqlite fails to compile

**Mitigation:** The `bundled` feature compiles SQLite from source. If it fails, try system SQLite or skip database on iOS (in-memory only).

---

## Out of Scope

These are explicitly NOT part of Phase 4.5:

1. **App Store submission** - No icons, launch screens, or store metadata
2. **Android build** - iOS only for now
3. **MCP server on iOS** - Desktop-only feature
4. **File watcher on iOS** - Desktop-only feature
5. **Semantic search on iOS** - ML dependencies too complex
6. **Asset browser on iOS** - Depends on imgui working; defer if problematic

---

## Summary

Phase 4.5 gets the map editor running on iPad with:
- Minimal changes to existing code (feature flags, not rewrites)
- One-command deployment via `./scripts/build-ios.sh --device`
- Foundation for future iOS releases

**Critical path:** M15 → M16 → M17 → M18 → M19

Each milestone is independently verifiable. If M17 (map editor) fails, M15 (hello bevy) still proves the pipeline works.
