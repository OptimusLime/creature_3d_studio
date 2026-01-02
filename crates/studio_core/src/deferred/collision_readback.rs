//! GPU collision result readback.
//!
//! This module handles reading back collision results from the GPU staging buffers
//! and making them available to the main world for physics integration.

use bevy::prelude::*;
use std::sync::{Arc, Mutex};

use crate::voxel_collision_gpu::{GpuCollisionResult, GpuContact};

/// Resource holding the latest GPU collision results.
/// Shared between render world (write) and main world (read).
#[derive(Resource, Clone, Default)]
pub struct GpuCollisionContacts {
    /// The contacts from the last frame (wrapped in Arc<Mutex> for cross-world sharing)
    pub inner: Arc<Mutex<GpuCollisionResult>>,
}

impl GpuCollisionContacts {
    /// Get a copy of the current collision result.
    pub fn get(&self) -> GpuCollisionResult {
        self.inner.lock().unwrap().clone()
    }
    
    /// Set new collision results (called from render world).
    pub fn set(&self, result: GpuCollisionResult) {
        *self.inner.lock().unwrap() = result;
    }
    
    /// Check if there are any contacts.
    pub fn has_contacts(&self) -> bool {
        !self.inner.lock().unwrap().contacts.is_empty()
    }
    
    /// Get contacts for a specific fragment index.
    pub fn contacts_for_fragment(&self, fragment_index: u32) -> Vec<GpuContact> {
        self.inner
            .lock()
            .unwrap()
            .contacts
            .iter()
            .filter(|c| c.fragment_index == fragment_index)
            .cloned()
            .collect()
    }
}

/// Plugin that sets up GPU collision readback.
/// This creates the shared resource in both main and render worlds.
pub struct GpuCollisionReadbackPlugin;

impl Plugin for GpuCollisionReadbackPlugin {
    fn build(&self, app: &mut App) {
        // Create the shared contacts resource
        let contacts = GpuCollisionContacts::default();
        
        // Add to main world
        app.insert_resource(contacts.clone());
        
        // Add to render world
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.insert_resource(contacts);
        }
    }
}
