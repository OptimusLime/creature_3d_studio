//! MCP (Model Context Protocol) HTTP server for external AI interaction.
//!
//! Provides a simple REST API on port 8088 for:
//! - Listing materials
//! - Creating new materials  
//! - Getting PNG output
//! - Searching assets by name
//!
//! # Endpoints
//!
//! - `GET /health` - Health check
//! - `GET /mcp/list_materials` - Get all materials as JSON
//! - `POST /mcp/create_material` - Add new material
//! - `GET /mcp/get_output` - Get current render as PNG
//! - `GET /mcp/search?q=<query>&type=<type>` - Search assets by name

use super::asset::{Asset, AssetStore};
use super::material::{Material, MaterialPalette};
use super::voxel_buffer_2d::VoxelBuffer2D;
use bevy::prelude::*;
use image::ImageEncoder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    GetOutput,
    Search(SearchRequest),
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
    SearchResults(Vec<SearchResultJson>),
    Error(String),
}

/// JSON representation of a search result.
#[derive(Serialize)]
struct SearchResultJson {
    #[serde(rename = "type")]
    asset_type: String,
    name: String,
    id: u32,
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
            // Send request to Bevy
            if request_tx.send(McpRequest::GetOutput).is_err() {
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

            Ok(McpRequest::GetOutput) => {
                // Generate PNG from current buffer state
                let png_data = generate_png_from_buffer(&buffer, &palette);
                let _ = channels.response_tx.send(McpResponse::Output(png_data));
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
                })
                .collect();

                let _ = channels
                    .response_tx
                    .send(McpResponse::SearchResults(results));
            }

            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Generate a PNG image from the voxel buffer.
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
