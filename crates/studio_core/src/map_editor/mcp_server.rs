//! MCP (Model Context Protocol) HTTP server for external AI interaction.
//!
//! Provides a simple REST API on port 8088 for:
//! - Listing materials
//! - Creating new materials  
//! - Getting PNG output (with optional layer/surface filtering)
//! - Searching assets by name
//! - Setting generator/materials/renderer Lua source (triggers hot reload)
//! - Listing render layers
//! - Managing Lua layer registry (register/unregister layers)
//! - Querying generator state
//! - Managing render surfaces
//! - Recording generation for video export
//!
//! # Endpoints
//!
//! - `GET /health` - Health check
//! - `GET /mcp/list_materials` - Get all materials as JSON
//! - `POST /mcp/create_material` - Add new material
//! - `GET /mcp/get_output` - Get current render as PNG (composite of all surfaces)
//! - `GET /mcp/get_output?layers=base,visualizer` - Get filtered render as PNG
//! - `GET /mcp/get_output?surface=grid` - Get specific surface as PNG
//! - `GET /mcp/list_layers` - List available render layers
//! - `GET /mcp/search?q=<query>&type=<type>` - Search assets by name
//! - `POST /mcp/set_generator` - Set generator.lua source (triggers hot reload)
//! - `POST /mcp/set_materials` - Set materials.lua source (triggers hot reload)
//! - `POST /mcp/set_renderer` - Set renderer Lua source (triggers hot reload)
//! - `GET /mcp/layer_registry` - List all registered layers with details
//! - `POST /mcp/register_layer` - Register a new layer
//! - `DELETE /mcp/layer/{name}` - Unregister a layer by name
//! - `GET /mcp/generator_state` - Get current generator state (type, step, running)
//! - `GET /mcp/surfaces` - List all render surfaces and layout
//! - `POST /mcp/start_recording` - Start recording frames
//! - `POST /mcp/stop_recording` - Stop recording frames
//! - `POST /mcp/export_video` - Export recorded frames to video

use super::asset::{Asset, AssetStore};
use super::generator::StepInfoRegistry;
use super::lua_generator::GENERATOR_LUA_PATH;
use super::lua_layer_registry::{LuaLayerDef, LuaLayerRegistry, LuaLayerType};
use super::lua_materials::MATERIALS_LUA_PATH;
use super::material::{Material, MaterialPalette};
use super::playback::PlaybackState;
use super::render::{
    FrameCapture, RenderContext, RenderSurfaceManager, SurfaceInfo, RENDERER_LUA_PATH,
};
use super::voxel_buffer_2d::VoxelBuffer2D;
use bevy::prelude::*;
use image::ImageEncoder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;
use tiny_http::{Header, Method, Request, Response, Server};

/// Default port for the MCP server.
pub const MCP_SERVER_PORT: u16 = 8088;

/// Plugin that runs an HTTP server for external AI interaction.
pub struct McpServerPlugin {
    pub port: u16,
}

impl Default for McpServerPlugin {
    fn default() -> Self {
        Self {
            port: MCP_SERVER_PORT,
        }
    }
}

impl Plugin for McpServerPlugin {
    fn build(&self, app: &mut App) {
        let port = self.port;

        // Create channels for communication between HTTP thread and Bevy
        let (request_tx, request_rx) = channel::<McpRequest>();
        let (response_tx, response_rx) = channel::<McpResponse>();

        // Start HTTP server in background thread
        thread::spawn(move || {
            run_http_server(port, request_tx, response_rx);
        });

        // Use NonSend because mpsc channels are not Sync
        app.insert_non_send_resource(McpChannels {
            request_rx,
            response_tx,
        });

        app.add_systems(Update, handle_mcp_requests);

        info!("MCP server starting on port {}", port);
    }
}

/// Channels for MCP communication (non-send because Receiver is not Sync).
struct McpChannels {
    request_rx: Receiver<McpRequest>,
    response_tx: Sender<McpResponse>,
}

/// Request types from HTTP server to Bevy.
enum McpRequest {
    ListMaterials,
    CreateMaterial(MaterialCreateRequest),
    GetOutput(GetOutputRequest), // Layer/surface filter options
    ListLayers,
    Search(SearchRequest),
    SetGenerator(String),
    SetMaterials(String),
    GetGeneratorState,
    SetRenderer(String),
    // Layer registry operations
    RegisterLayer(LayerRegisterRequest),
    UnregisterLayer(String), // layer name
    GetLayerRegistry,
    // Surface operations
    GetSurfaces,
    // Recording operations
    StartRecording,
    StopRecording,
    ExportVideo(VideoExportRequest),
}

/// Request parameters for get_output.
#[derive(Debug)]
struct GetOutputRequest {
    /// Optional layer filter (applies to single surface or default behavior).
    layers: Option<Vec<String>>,
    /// Optional specific surface to render.
    surface: Option<String>,
}

/// Request parameters for video export.
#[derive(Debug, Deserialize)]
struct VideoExportRequest {
    path: String,
    #[serde(default = "default_fps")]
    fps: u32,
    #[serde(default = "default_codec")]
    codec: String,
}

fn default_fps() -> u32 {
    30
}

fn default_codec() -> String {
    "libx264".to_string()
}

/// Request parameters for search.
#[derive(Debug)]
struct SearchRequest {
    query: String,
    asset_type: Option<String>,
}

/// Response types from Bevy to HTTP server.
enum McpResponse {
    Materials(Vec<MaterialJson>),
    MaterialCreated { success: bool, id: u32 },
    Output(Vec<u8>),
    Layers(Vec<String>),
    SearchResults(Vec<SearchResultJson>),
    Success { success: bool },
    Error(String),
    LayerRegistry(Vec<LayerDefJson>),
    GeneratorState(GeneratorStateJson),
    Surfaces(SurfaceInfo),
    RecordingStarted { success: bool },
    RecordingStopped { success: bool, frame_count: usize },
    VideoExported { success: bool, path: String },
}

/// JSON representation of generator state.
#[derive(Serialize)]
struct GeneratorStateJson {
    /// Generator type: "lua", "markov", or "unknown"
    #[serde(rename = "type")]
    generator_type: String,
    /// Current step number
    step: usize,
    /// Whether generation is still running
    running: bool,
    /// Whether generation is completed
    completed: bool,
    /// Grid size [width, height]
    grid_size: [usize; 2],
    /// Generator structure (recursive tree)
    #[serde(skip_serializing_if = "Option::is_none")]
    structure: Option<super::generator::GeneratorStructure>,
    /// Step info for all paths in the scene tree
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    steps: HashMap<String, StepInfoJson>,
}

/// JSON representation of step info for a specific path.
#[derive(Serialize)]
struct StepInfoJson {
    step: usize,
    x: usize,
    y: usize,
    material_id: u32,
    completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    rule_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    affected_cells: Option<usize>,
}

/// Request body for registering a layer.
#[derive(Deserialize, Debug)]
struct LayerRegisterRequest {
    name: String,
    #[serde(rename = "type")]
    layer_type: String, // "renderer" or "visualizer"
    lua_path: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// JSON representation of a layer definition.
#[derive(Serialize)]
struct LayerDefJson {
    name: String,
    #[serde(rename = "type")]
    layer_type: String,
    lua_path: String,
    tags: Vec<String>,
}

/// JSON representation of a search result.
#[derive(Serialize)]
struct SearchResultJson {
    #[serde(rename = "type")]
    asset_type: String,
    name: String,
    id: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

/// JSON representation of a material.
#[derive(Serialize, Deserialize, Clone)]
struct MaterialJson {
    id: u32,
    name: String,
    color: [f32; 3],
}

impl From<&Material> for MaterialJson {
    fn from(m: &Material) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            color: m.color,
        }
    }
}

/// Request body for creating a material.
#[derive(Deserialize)]
struct MaterialCreateRequest {
    id: u32,
    name: String,
    color: [f32; 3],
}

/// Run the HTTP server (blocking, runs in background thread).
fn run_http_server(port: u16, request_tx: Sender<McpRequest>, response_rx: Receiver<McpResponse>) {
    let addr = format!("0.0.0.0:{}", port);
    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start MCP server on {}: {}", addr, e);
            return;
        }
    };

    println!("MCP server listening on http://{}", addr);

    for request in server.incoming_requests() {
        handle_http_request(request, &request_tx, &response_rx);
    }
}

/// Parse query string into a HashMap.
fn parse_query_string(url: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    if let Some(query_start) = url.find('?') {
        let query = &url[query_start + 1..];
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                params.insert(
                    key.to_string(),
                    urlencoding::decode(value).unwrap_or_default().into_owned(),
                );
            }
        }
    }

    params
}

/// Get the path portion of a URL (before the query string).
fn url_path(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Handle a single HTTP request.
fn handle_http_request(
    mut request: Request,
    request_tx: &Sender<McpRequest>,
    response_rx: &Receiver<McpResponse>,
) {
    let url = request.url().to_string();
    let method = request.method().clone();
    let path = url_path(&url);

    match (method, path) {
        (Method::Get, "/health") => {
            let response =
                Response::from_string(r#"{"status":"ok"}"#).with_header(content_type_json());
            let _ = request.respond(response);
        }

        (Method::Get, "/mcp/list_materials") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::ListMaterials).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response (with timeout)
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Materials(materials)) => {
                    let json = serde_json::to_string(&materials).unwrap_or_default();
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/create_material") => {
            // Read request body
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let _ = request.respond(error_response(&format!("Failed to read body: {}", e)));
                return;
            }

            // Parse JSON
            let create_req: MaterialCreateRequest = match serde_json::from_str(&body) {
                Ok(r) => r,
                Err(e) => {
                    let _ = request.respond(error_response(&format!("Invalid JSON: {}", e)));
                    return;
                }
            };

            // Send request to Bevy
            if request_tx
                .send(McpRequest::CreateMaterial(create_req))
                .is_err()
            {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::MaterialCreated { success, id }) => {
                    let json = format!(r#"{{"success":{},"id":{}}}"#, success, id);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Get, "/mcp/get_output") => {
            // Parse optional layers and surface parameters
            let params = parse_query_string(&url);
            let layers = params.get("layers").map(|s| {
                s.split(',')
                    .map(|l| l.trim().to_string())
                    .collect::<Vec<_>>()
            });
            let surface = params.get("surface").cloned();

            // Send request to Bevy
            if request_tx
                .send(McpRequest::GetOutput(GetOutputRequest { layers, surface }))
                .is_err()
            {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response (longer timeout for screenshot)
            match response_rx.recv_timeout(std::time::Duration::from_secs(10)) {
                Ok(McpResponse::Output(png_data)) => {
                    let response = Response::from_data(png_data)
                        .with_header(Header::from_bytes("Content-Type", "image/png").unwrap());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Get, "/mcp/list_layers") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::ListLayers).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Layers(layers)) => {
                    let json = serde_json::to_string(&layers).unwrap_or_default();
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Get, "/mcp/search") => {
            let params = parse_query_string(&url);

            let query = match params.get("q") {
                Some(q) => q.clone(),
                None => {
                    let _ = request.respond(error_response("Missing 'q' parameter"));
                    return;
                }
            };

            let asset_type = params.get("type").cloned();

            // Send request to Bevy
            if request_tx
                .send(McpRequest::Search(SearchRequest { query, asset_type }))
                .is_err()
            {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::SearchResults(results)) => {
                    let json = serde_json::to_string(&results).unwrap_or_default();
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/set_generator") => {
            // Read Lua source from request body
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let _ = request.respond(error_response(&format!("Failed to read body: {}", e)));
                return;
            }

            // Send request to Bevy
            if request_tx.send(McpRequest::SetGenerator(body)).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Success { success }) => {
                    let json = format!(r#"{{"success":{}}}"#, success);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/set_materials") => {
            // Read Lua source from request body
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let _ = request.respond(error_response(&format!("Failed to read body: {}", e)));
                return;
            }

            // Send request to Bevy
            if request_tx.send(McpRequest::SetMaterials(body)).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Success { success }) => {
                    let json = format!(r#"{{"success":{}}}"#, success);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/set_renderer") => {
            // Read Lua source from request body
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let _ = request.respond(error_response(&format!("Failed to read body: {}", e)));
                return;
            }

            // Send request to Bevy
            if request_tx.send(McpRequest::SetRenderer(body)).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Success { success }) => {
                    let json = format!(r#"{{"success":{}}}"#, success);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Get, "/mcp/layer_registry") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::GetLayerRegistry).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::LayerRegistry(layers)) => {
                    let json = serde_json::to_string(&layers).unwrap_or_default();
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/register_layer") => {
            // Read JSON from request body
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let _ = request.respond(error_response(&format!("Failed to read body: {}", e)));
                return;
            }

            // Parse JSON
            let layer_req: LayerRegisterRequest = match serde_json::from_str(&body) {
                Ok(req) => req,
                Err(e) => {
                    let _ = request.respond(error_response(&format!("Invalid JSON: {}", e)));
                    return;
                }
            };

            // Send request to Bevy
            if request_tx
                .send(McpRequest::RegisterLayer(layer_req))
                .is_err()
            {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Success { success }) => {
                    let json = format!(r#"{{"success":{}}}"#, success);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Get, "/mcp/generator_state") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::GetGeneratorState).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::GeneratorState(state)) => {
                    let json = serde_json::to_string(&state).unwrap_or_default();
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Delete, path) if path.starts_with("/mcp/layer/") => {
            // Extract layer name from path: /mcp/layer/{name}
            let name = path.strip_prefix("/mcp/layer/").unwrap_or("").to_string();
            if name.is_empty() {
                let _ = request.respond(error_response("Missing layer name"));
                return;
            }

            // Send request to Bevy
            if request_tx.send(McpRequest::UnregisterLayer(name)).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Success { success }) => {
                    let json = format!(r#"{{"success":{}}}"#, success);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Get, "/mcp/surfaces") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::GetSurfaces).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::Surfaces(info)) => {
                    let json = serde_json::to_string(&info).unwrap_or_default();
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/start_recording") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::StartRecording).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::RecordingStarted { success }) => {
                    let json = format!(r#"{{"success":{}}}"#, success);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/stop_recording") => {
            // Send request to Bevy
            if request_tx.send(McpRequest::StopRecording).is_err() {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response
            match response_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(McpResponse::RecordingStopped {
                    success,
                    frame_count,
                }) => {
                    let json =
                        format!(r#"{{"success":{},"frame_count":{}}}"#, success, frame_count);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        (Method::Post, "/mcp/export_video") => {
            // Read JSON from request body
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let _ = request.respond(error_response(&format!("Failed to read body: {}", e)));
                return;
            }

            // Parse JSON
            let export_req: VideoExportRequest = match serde_json::from_str(&body) {
                Ok(req) => req,
                Err(e) => {
                    let _ = request.respond(error_response(&format!("Invalid JSON: {}", e)));
                    return;
                }
            };

            // Send request to Bevy
            if request_tx
                .send(McpRequest::ExportVideo(export_req))
                .is_err()
            {
                let _ = request.respond(error_response("Server shutting down"));
                return;
            }

            // Wait for response (longer timeout for video export)
            match response_rx.recv_timeout(std::time::Duration::from_secs(60)) {
                Ok(McpResponse::VideoExported { success, path }) => {
                    let json = format!(r#"{{"success":{},"path":"{}"}}"#, success, path);
                    let response = Response::from_string(json).with_header(content_type_json());
                    let _ = request.respond(response);
                }
                Ok(McpResponse::Error(e)) => {
                    let _ = request.respond(error_response(&e));
                }
                _ => {
                    let _ = request.respond(error_response("Unexpected response"));
                }
            }
        }

        _ => {
            let response = Response::from_string(r#"{"error":"Not found"}"#)
                .with_status_code(404)
                .with_header(content_type_json());
            let _ = request.respond(response);
        }
    }
}

fn content_type_json() -> Header {
    Header::from_bytes("Content-Type", "application/json").unwrap()
}

fn error_response(msg: &str) -> Response<Cursor<Vec<u8>>> {
    let json = format!(r#"{{"error":"{}"}}"#, msg);
    Response::from_string(json)
        .with_status_code(500)
        .with_header(content_type_json())
}

/// System to handle MCP requests in Bevy.
fn handle_mcp_requests(
    channels: NonSend<McpChannels>,
    mut palette: ResMut<MaterialPalette>,
    buffer: Res<VoxelBuffer2D>,
    surface_manager: Option<Res<RenderSurfaceManager>>,
    mut frame_capture: Option<ResMut<FrameCapture>>,
    mut layer_registry: Option<ResMut<LuaLayerRegistry>>,
    playback: Option<Res<PlaybackState>>,
    step_registry: Option<Res<StepInfoRegistry>>,
    active_generator: Option<NonSend<super::generator::ActiveGenerator>>,
    mut reload_flag: Option<ResMut<super::lua_generator::GeneratorReloadFlag>>,
) {
    // Process all pending requests
    loop {
        match channels.request_rx.try_recv() {
            Ok(McpRequest::ListMaterials) => {
                let materials: Vec<MaterialJson> =
                    palette.available.iter().map(MaterialJson::from).collect();
                let _ = channels.response_tx.send(McpResponse::Materials(materials));
            }

            Ok(McpRequest::CreateMaterial(req)) => {
                if palette.has_material(req.id) {
                    // Update existing material
                    if let Some(mat) = palette.get_by_id_mut(req.id) {
                        mat.name = req.name.clone();
                        mat.color = req.color;
                    }
                    palette.changed = true;
                    info!("MCP: Updated material {} (id={})", req.name, req.id);
                } else {
                    // Add new material
                    palette.add_material(Material::new(req.id, req.name.clone(), req.color));
                    // Also add to active palette so it's immediately usable
                    palette.add_to_active(req.id);
                    info!("MCP: Created material {} (id={})", req.name, req.id);
                }

                let _ = channels.response_tx.send(McpResponse::MaterialCreated {
                    success: true,
                    id: req.id,
                });
            }

            Ok(McpRequest::GetOutput(req)) => {
                let ctx = RenderContext::new(&buffer, &palette);

                let png_data = if let Some(ref manager) = surface_manager {
                    // If specific surface requested, render just that surface
                    if let Some(ref surface_name) = req.surface {
                        if let Some(pixels) = if req.layers.is_some() {
                            let names: Vec<&str> = req
                                .layers
                                .as_ref()
                                .unwrap()
                                .iter()
                                .map(|s| s.as_str())
                                .collect();
                            manager.render_surface_filtered(surface_name, &ctx, &names)
                        } else {
                            manager.render_surface(surface_name, &ctx)
                        } {
                            encode_png(&pixels)
                        } else {
                            let _ = channels.response_tx.send(McpResponse::Error(format!(
                                "Surface '{}' not found",
                                surface_name
                            )));
                            continue;
                        }
                    } else {
                        // Render composite of all surfaces
                        let pixels = manager.render_composite(&ctx);
                        encode_png(&pixels)
                    }
                } else {
                    // Legacy fallback when no surface manager
                    generate_png_from_buffer(&buffer, &palette)
                };
                let _ = channels.response_tx.send(McpResponse::Output(png_data));
            }

            Ok(McpRequest::ListLayers) => {
                let layers = if let Some(ref manager) = surface_manager {
                    // List layers from the "grid" surface
                    manager
                        .get_surface("grid")
                        .map(|s| s.list_layers().into_iter().map(|l| l.to_string()).collect())
                        .unwrap_or_else(Vec::new)
                } else {
                    vec!["base".to_string()] // Default when no stack
                };
                let _ = channels.response_tx.send(McpResponse::Layers(layers));
            }

            Ok(McpRequest::Search(req)) => {
                // Search materials using MaterialPalette::search
                // In future, this can search across all asset types
                let results: Vec<SearchResultJson> = if let Some(ref t) = req.asset_type {
                    // Filter by type - only search if type matches
                    if t == Material::asset_type() {
                        palette.search(&req.query)
                    } else {
                        Vec::new()
                    }
                } else {
                    palette.search(&req.query)
                }
                .into_iter()
                .map(|mat| SearchResultJson {
                    asset_type: Material::asset_type().to_string(),
                    name: mat.name.clone(),
                    id: mat.id,
                    tags: mat.tags.clone(),
                })
                .collect();

                let _ = channels
                    .response_tx
                    .send(McpResponse::SearchResults(results));
            }

            Ok(McpRequest::SetGenerator(lua_source)) => {
                // Write Lua source to generator.lua file
                // File watcher will trigger hot reload automatically
                match fs::write(GENERATOR_LUA_PATH, &lua_source) {
                    Ok(_) => {
                        info!("MCP: Wrote generator.lua ({} bytes)", lua_source.len());
                        let _ = channels
                            .response_tx
                            .send(McpResponse::Success { success: true });
                    }
                    Err(e) => {
                        error!("MCP: Failed to write generator.lua: {}", e);
                        let _ = channels.response_tx.send(McpResponse::Error(e.to_string()));
                    }
                }
            }

            Ok(McpRequest::SetMaterials(lua_source)) => {
                // Write Lua source to materials.lua file
                // File watcher will trigger hot reload automatically
                match fs::write(MATERIALS_LUA_PATH, &lua_source) {
                    Ok(_) => {
                        info!("MCP: Wrote materials.lua ({} bytes)", lua_source.len());
                        let _ = channels
                            .response_tx
                            .send(McpResponse::Success { success: true });
                    }
                    Err(e) => {
                        error!("MCP: Failed to write materials.lua: {}", e);
                        let _ = channels.response_tx.send(McpResponse::Error(e.to_string()));
                    }
                }
            }

            Ok(McpRequest::SetRenderer(lua_source)) => {
                // Write Lua source to renderer file
                // Note: Hot reload for renderer would need to be implemented separately
                match fs::write(RENDERER_LUA_PATH, &lua_source) {
                    Ok(_) => {
                        info!("MCP: Wrote renderer ({} bytes)", lua_source.len());
                        let _ = channels
                            .response_tx
                            .send(McpResponse::Success { success: true });
                    }
                    Err(e) => {
                        error!("MCP: Failed to write renderer: {}", e);
                        let _ = channels.response_tx.send(McpResponse::Error(e.to_string()));
                    }
                }
            }

            Ok(McpRequest::GetLayerRegistry) => {
                let layers = if let Some(ref registry) = layer_registry {
                    registry
                        .list()
                        .iter()
                        .map(|def| LayerDefJson {
                            name: def.name.clone(),
                            layer_type: match def.layer_type {
                                LuaLayerType::Renderer => "renderer".to_string(),
                                LuaLayerType::Visualizer => "visualizer".to_string(),
                            },
                            lua_path: def.lua_path.clone(),
                            tags: def.tags.clone(),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let _ = channels
                    .response_tx
                    .send(McpResponse::LayerRegistry(layers));
            }

            Ok(McpRequest::RegisterLayer(req)) => {
                if let Some(ref mut registry) = layer_registry {
                    let layer_type = match req.layer_type.as_str() {
                        "renderer" => LuaLayerType::Renderer,
                        "visualizer" => LuaLayerType::Visualizer,
                        _ => {
                            let _ = channels.response_tx.send(McpResponse::Error(format!(
                                "Invalid layer type: {}",
                                req.layer_type
                            )));
                            continue;
                        }
                    };

                    let def = LuaLayerDef {
                        name: req.name.clone(),
                        layer_type,
                        lua_path: req.lua_path.clone(),
                        tags: req.tags,
                    };

                    registry.register(def);
                    registry.mark_for_reload(&req.name);
                    info!("MCP: Registered layer '{}' ({})", req.name, req.layer_type);
                    let _ = channels
                        .response_tx
                        .send(McpResponse::Success { success: true });
                } else {
                    let _ = channels.response_tx.send(McpResponse::Error(
                        "Layer registry not available".to_string(),
                    ));
                }
            }

            Ok(McpRequest::UnregisterLayer(name)) => {
                if let Some(ref mut registry) = layer_registry {
                    if registry.unregister(&name).is_some() {
                        info!("MCP: Unregistered layer '{}'", name);
                        let _ = channels
                            .response_tx
                            .send(McpResponse::Success { success: true });
                    } else {
                        let _ = channels
                            .response_tx
                            .send(McpResponse::Error(format!("Layer '{}' not found", name)));
                    }
                } else {
                    let _ = channels.response_tx.send(McpResponse::Error(
                        "Layer registry not available".to_string(),
                    ));
                }
            }

            Ok(McpRequest::GetGeneratorState) => {
                // Build step info map from registry
                let steps: HashMap<String, StepInfoJson> = step_registry
                    .as_ref()
                    .map(|reg| {
                        reg.all()
                            .iter()
                            .map(|(path, info)| {
                                (
                                    path.clone(),
                                    StepInfoJson {
                                        step: info.step_number,
                                        x: info.x,
                                        y: info.y,
                                        material_id: info.material_id,
                                        completed: info.completed,
                                        rule_name: info.rule_name.clone(),
                                        affected_cells: info.affected_cells,
                                    },
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Get structure from active generator (if available)
                let structure = active_generator.as_ref().and_then(|ag| ag.structure());

                // Determine generator type from structure or default to "lua"
                let generator_type = structure
                    .as_ref()
                    .map(|s| s.type_name.clone())
                    .unwrap_or_else(|| "lua".to_string());

                // Get generator state from playback
                let state = if let Some(ref pb) = playback {
                    GeneratorStateJson {
                        generator_type,
                        step: pb.step_index,
                        running: pb.playing && !pb.completed,
                        completed: pb.completed,
                        grid_size: [buffer.width, buffer.height],
                        structure,
                        steps,
                    }
                } else {
                    GeneratorStateJson {
                        generator_type,
                        step: 0,
                        running: false,
                        completed: false,
                        grid_size: [buffer.width, buffer.height],
                        structure,
                        steps,
                    }
                };
                let _ = channels
                    .response_tx
                    .send(McpResponse::GeneratorState(state));
            }

            Ok(McpRequest::GetSurfaces) => {
                let info = if let Some(ref manager) = surface_manager {
                    manager.info()
                } else {
                    // Default to single grid surface based on buffer dimensions
                    SurfaceInfo {
                        surfaces: vec!["grid".to_string()],
                        layout: super::render::SurfaceLayout::Single("grid".to_string()),
                        total_size: [buffer.width, buffer.height],
                    }
                };
                let _ = channels.response_tx.send(McpResponse::Surfaces(info));
            }

            Ok(McpRequest::StartRecording) => {
                if let Some(ref mut capture) = frame_capture {
                    capture.clear();
                    capture.start();
                    // Trigger generator reset so it runs again and captures frames
                    if let Some(ref mut flag) = reload_flag {
                        flag.needs_reload = true;
                    }
                    info!("MCP: Started recording (triggering generator reset)");
                    let _ = channels
                        .response_tx
                        .send(McpResponse::RecordingStarted { success: true });
                } else {
                    let _ = channels.response_tx.send(McpResponse::Error(
                        "Frame capture not available".to_string(),
                    ));
                }
            }

            Ok(McpRequest::StopRecording) => {
                if let Some(ref mut capture) = frame_capture {
                    capture.stop();
                    let frame_count = capture.frame_count();
                    info!("MCP: Stopped recording ({} frames)", frame_count);
                    let _ = channels.response_tx.send(McpResponse::RecordingStopped {
                        success: true,
                        frame_count,
                    });
                } else {
                    let _ = channels.response_tx.send(McpResponse::Error(
                        "Frame capture not available".to_string(),
                    ));
                }
            }

            Ok(McpRequest::ExportVideo(req)) => {
                if let Some(ref mut capture) = frame_capture {
                    if capture.frame_count() == 0 {
                        let _ = channels
                            .response_tx
                            .send(McpResponse::Error("No frames recorded".to_string()));
                        continue;
                    }

                    // Set frame rate from request
                    capture.set_frame_rate(req.fps);
                    let path = std::path::Path::new(&req.path);
                    match capture.export_video(path, &req.codec) {
                        Ok(_) => {
                            info!("MCP: Exported video to {}", req.path);
                            let _ = channels.response_tx.send(McpResponse::VideoExported {
                                success: true,
                                path: req.path,
                            });
                        }
                        Err(e) => {
                            error!("MCP: Failed to export video: {}", e);
                            let _ = channels.response_tx.send(McpResponse::Error(e.to_string()));
                        }
                    }
                } else {
                    let _ = channels.response_tx.send(McpResponse::Error(
                        "Frame capture not available".to_string(),
                    ));
                }
            }

            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Generate a PNG image from the voxel buffer (legacy fallback).
fn generate_png_from_buffer(buffer: &VoxelBuffer2D, palette: &MaterialPalette) -> Vec<u8> {
    let width = buffer.width as u32;
    let height = buffer.height as u32;

    // Create RGBA image data
    let mut img_data = vec![0u8; (width * height * 4) as usize];

    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mat_id = buffer.get(x, y);
            let color = if mat_id == 0 {
                [30u8, 30, 30, 255] // Empty = dark gray
            } else if let Some(mat) = palette.get_by_id(mat_id) {
                [
                    (mat.color[0] * 255.0) as u8,
                    (mat.color[1] * 255.0) as u8,
                    (mat.color[2] * 255.0) as u8,
                    255,
                ]
            } else {
                [255u8, 0, 255, 255] // Unknown = magenta
            };

            let idx = (y * buffer.width + x) * 4;
            img_data[idx..idx + 4].copy_from_slice(&color);
        }
    }

    // Encode as PNG
    let mut png_bytes = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(&img_data, width, height, image::ExtendedColorType::Rgba8)
            .expect("PNG encoding failed");
    }

    png_bytes
}

/// Encode a PixelBuffer as PNG.
fn encode_png(pixels: &super::render::PixelBuffer) -> Vec<u8> {
    let mut png_bytes = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(
                pixels.as_bytes(),
                pixels.width as u32,
                pixels.height as u32,
                image::ExtendedColorType::Rgba8,
            )
            .expect("PNG encoding failed");
    }
    png_bytes
}
