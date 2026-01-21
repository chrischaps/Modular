//! Main application struct for the Modular Synth
//!
//! Contains the SynthApp which implements eframe::App and manages
//! the synthesizer's UI state, audio engine, and graph state.

use std::collections::HashMap;

use eframe::egui::{self, RichText, Layout, Align};
use egui_node_graph2::{GraphEditorState, NodeResponse, NodeTemplateTrait};

use crate::engine::{
    AudioEngine, AudioError, AudioProcessor, DeviceInfo, EngineChannels, EngineCommand, UiHandle,
};
use crate::graph::{
    validate_connection, AllNodeTemplates, AnyParameterId, SynthDataType, SynthGraphState,
    SynthNodeData, SynthNodeTemplate, SynthValueType,
};
use super::theme;

/// Type alias for our graph editor state
type SynthGraphEditorState = GraphEditorState<SynthNodeData, SynthDataType, SynthValueType, SynthNodeTemplate, SynthGraphState>;

/// Main application state for the Modular Synth
pub struct SynthApp {
    /// Audio engine handle
    audio_engine: Result<AudioEngine, AudioError>,

    /// UI-side handle for communicating with audio engine
    ui_handle: Option<UiHandle>,

    /// Last audio error message to display
    audio_error_message: Option<String>,

    /// Whether the transport is "playing" (audio graph processing active)
    is_playing: bool,

    /// Whether theme has been applied
    theme_applied: bool,

    /// Node graph editor state
    graph_state: SynthGraphEditorState,

    /// User state for the graph editor
    user_state: SynthGraphState,

    /// Cached list of audio devices
    audio_devices: Vec<DeviceInfo>,

    /// Index of currently selected device
    selected_device_index: usize,

    /// Cached parameter values for change detection.
    /// Key is (node_id as u64, param_index), value is the last sent value.
    cached_params: HashMap<(u64, usize), f32>,
}

impl SynthApp {
    /// Create a new SynthApp instance
    ///
    /// If `enable_test_tone` is true, audio will start with a test tone immediately.
    pub fn new(enable_test_tone: bool) -> Self {
        let mut audio_engine = AudioEngine::new();

        let audio_error_message = match &audio_engine {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        // Get initial device list and find the default device index
        let (audio_devices, selected_device_index) = match &audio_engine {
            Ok(engine) => {
                let devices = engine.enumerate_devices();
                let default_idx = devices.iter()
                    .position(|d| d.is_default)
                    .unwrap_or(0);
                (devices, default_idx)
            }
            Err(_) => (Vec::new(), 0),
        };

        // Create engine channels for communication with audio thread
        let channels = EngineChannels::with_defaults();
        let (ui_handle, engine_handle) = channels.split();

        // Create and start the audio processor if engine is available
        let ui_handle = if let Ok(ref mut engine) = audio_engine {
            let sample_rate = engine.sample_rate() as f32;
            let block_size = 256; // Standard block size
            let processor = AudioProcessor::new(sample_rate, block_size, engine_handle);

            if let Err(e) = engine.start_with_processor(processor) {
                eprintln!("Failed to start audio processor: {}", e);
            }

            Some(ui_handle)
        } else {
            // Drop the engine_handle since we can't use it
            drop(engine_handle);
            None
        };

        let app = Self {
            audio_engine,
            ui_handle,
            audio_error_message,
            is_playing: false,
            theme_applied: false,
            graph_state: GraphEditorState::new(1.0),
            user_state: SynthGraphState::default(),
            audio_devices,
            selected_device_index,
            cached_params: HashMap::new(),
        };

        // Note: enable_test_tone is ignored - test tone was removed in favor of AudioProcessor
        let _ = enable_test_tone;

        app
    }

    /// Refresh the list of available audio devices
    fn refresh_devices(&mut self) {
        if let Ok(ref engine) = self.audio_engine {
            self.audio_devices = engine.enumerate_devices();
        }
    }

    /// Select an audio output device by index
    fn select_device(&mut self, index: usize) {
        if let Ok(ref mut engine) = self.audio_engine {
            match engine.select_device(index) {
                Ok(()) => {
                    self.selected_device_index = index;
                    self.audio_error_message = None;
                }
                Err(e) => {
                    self.audio_error_message = Some(e.to_string());
                }
            }
        }
    }

    // Note: start_audio/stop_audio removed - we now use AudioProcessor which
    // starts automatically. Use the Play/Stop transport button to control audio.

    /// Toggle the test tone on/off (legacy - may conflict with AudioProcessor)
    #[allow(dead_code)]
    fn toggle_test_tone(&mut self) {
        // Test tone is disabled when using AudioProcessor
        // This function is kept for potential future debug use
    }

    /// Draw the top toolbar with transport controls and status
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) -> ToolbarActions {
        let mut actions = ToolbarActions::default();

        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Application title
            ui.label(RichText::new("MODULAR SYNTH")
                .size(18.0)
                .color(theme::text::PRIMARY)
                .strong());

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Transport controls
            ui.label(RichText::new("Transport").color(theme::text::SECONDARY));
            ui.add_space(8.0);

            // Play/Stop button - controls whether the audio graph is processing
            let play_text = if self.is_playing { "⏹ Stop" } else { "▶ Play" };
            let play_color = if self.is_playing {
                theme::accent::WARNING
            } else {
                theme::accent::SUCCESS
            };

            if ui.button(RichText::new(play_text).color(play_color)).clicked() {
                actions.toggle_playing = true;
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Audio output selector
            match &self.audio_engine {
                Ok(engine) => {
                    let is_running = engine.is_running();

                    // Device selector
                    ui.label(RichText::new("Output").color(theme::text::SECONDARY));
                    ui.add_space(8.0);

                    // Get current device name for display
                    let current_device = self.audio_devices
                        .get(self.selected_device_index)
                        .map(|d| d.name.as_str())
                        .unwrap_or("No device");

                    // Truncate long device names
                    let display_name = if current_device.len() > 30 {
                        format!("{}...", &current_device[..27])
                    } else {
                        current_device.to_string()
                    };

                    egui::ComboBox::from_id_salt("device_selector")
                        .selected_text(display_name)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for device in &self.audio_devices {
                                let label = if device.is_default {
                                    format!("{} (Default)", device.name)
                                } else {
                                    device.name.clone()
                                };

                                if ui.selectable_label(
                                    device.index == self.selected_device_index,
                                    label
                                ).clicked() {
                                    actions.select_device = Some(device.index);
                                }
                            }

                            ui.separator();
                            if ui.button("🔄 Refresh").clicked() {
                                actions.refresh_devices = true;
                            }
                        });

                    // Status indicator
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let status_color = if is_running {
                            theme::accent::SUCCESS
                        } else {
                            theme::text::DISABLED
                        };
                        let status_text = if is_running { "● Running" } else { "○ Stopped" };
                        ui.label(RichText::new(status_text).color(status_color).small());

                        ui.label(RichText::new(format!(
                            "{}Hz • {}ch",
                            engine.sample_rate(),
                            engine.channels()
                        )).color(theme::text::SECONDARY).small());
                    });
                }
                Err(e) => {
                    ui.label(RichText::new(format!("⚠ Audio unavailable: {}", e))
                        .color(theme::accent::ERROR));
                }
            }
        });

        actions
    }

    /// Send a command to the audio engine.
    fn send_command(&mut self, cmd: EngineCommand) {
        if let Some(ref mut handle) = self.ui_handle {
            // Use lossy send - if buffer is full, command is dropped
            // This is acceptable for rapid updates like parameter changes
            handle.send_command_lossy(cmd);
        }
    }

    /// Process events from the audio engine.
    /// This handles InputValue events for knob animation and OutputValue events for LED indicators.
    fn process_engine_events(&mut self) {
        if let Some(ref mut handle) = self.ui_handle {
            // Drain all available events
            while let Some(event) = handle.recv_event() {
                match event {
                    crate::engine::EngineEvent::InputValue { node_id, input_index, value } => {
                        // Store the input value for UI feedback
                        self.user_state.set_input_value(node_id, input_index, value);
                    }
                    crate::engine::EngineEvent::OutputValue { node_id, output_index, value } => {
                        // Store the output value for LED indicators
                        self.user_state.set_output_value(node_id, output_index, value);
                    }
                    // Other events are not currently handled by the app
                    // (OutputLevel, CpuLoad, Started, Stopped, Error)
                    _ => {}
                }
            }
        }
    }

    /// Draw the main content area with the node graph editor
    fn draw_main_area(&mut self, ctx: &egui::Context) {
        // Collect connections to remove (validated after drawing)
        let mut invalid_connections: Vec<(egui_node_graph2::OutputId, egui_node_graph2::InputId)> = Vec::new();
        // Collect commands to send (to avoid borrow issues)
        let mut commands_to_send: Vec<EngineCommand> = Vec::new();
        // Track if we clicked in the editor area
        let mut cursor_in_editor = false;
        // Store editor rect for coordinate conversion
        let mut editor_rect = egui::Rect::NOTHING;

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                // Store editor rect for coordinate conversion
                editor_rect = ui.available_rect_before_wrap();

                // Draw the node graph editor
                let graph_response = self.graph_state.draw_graph_editor(
                    ui,
                    AllNodeTemplates,
                    &mut self.user_state,
                    Vec::default(),
                );

                cursor_in_editor = graph_response.cursor_in_editor;

                // Disable the built-in node finder - we use our own context menu
                self.graph_state.node_finder = None;

                // Process graph responses
                for response in graph_response.node_responses {
                    match response {
                        NodeResponse::CreatedNode(node_id) => {
                            // Allocate engine node ID for the new node
                            let engine_node_id = self.user_state.allocate_engine_node_id(node_id);

                            // Get the module ID from the node's user data
                            if let Some(node) = self.graph_state.graph.nodes.get(node_id) {
                                let module_id = node.user_data.module_id;
                                commands_to_send.push(EngineCommand::AddModule {
                                    node_id: engine_node_id,
                                    module_id,
                                });

                                // Set up output monitoring for any LED indicators
                                for led_indicator in &node.user_data.led_indicators {
                                    commands_to_send.push(EngineCommand::MonitorOutput {
                                        node_id: engine_node_id,
                                        output_index: led_indicator.output_index,
                                    });
                                }
                            }
                        }
                        NodeResponse::DeleteNodeFull { node_id, .. } => {
                            // Get engine node ID before removing from mapping
                            if let Some(engine_node_id) = self.user_state.remove_node(node_id) {
                                commands_to_send.push(EngineCommand::RemoveModule {
                                    node_id: engine_node_id,
                                });
                            }
                        }
                        NodeResponse::ConnectEventEnded { output, input, .. } => {
                            // Validate the connection after it was made
                            if let Some(error_msg) = self.validate_and_check_connection(output, input) {
                                // Mark for removal
                                invalid_connections.push((output, input));
                                // Show error message
                                self.user_state.set_validation_error(error_msg);
                            } else {
                                // Connection is valid - send to engine
                                if let Some(cmd) = self.build_connect_command(output, input) {
                                    commands_to_send.push(cmd);
                                    // If this input is an exposed param, start monitoring it
                                    if let Some(monitor_cmd) = self.build_monitor_input_command(input) {
                                        commands_to_send.push(monitor_cmd);
                                    }
                                }
                            }
                        }
                        NodeResponse::DisconnectEvent { output: _, input } => {
                            // Send disconnect command to engine
                            if let Some(cmd) = self.build_disconnect_command(input) {
                                commands_to_send.push(cmd);
                                // Stop monitoring this input
                                if let Some(unmonitor_cmd) = self.build_unmonitor_input_command(input) {
                                    commands_to_send.push(unmonitor_cmd);
                                }
                            }
                        }
                        NodeResponse::User(crate::graph::SynthResponse::ParameterChanged {
                            node_id: response_node_id,
                            param_name,
                            value,
                        }) => {
                            // Handle parameter changes from bottom_ui knobs
                            // Find the input param by name and update its value
                            if let Some(node) = self.graph_state.graph.nodes.get_mut(response_node_id) {
                                if let Some((_name, input_id)) = node.inputs.iter().find(|(name, _)| *name == param_name) {
                                    let input_id = *input_id;
                                    if let Some(input) = self.graph_state.graph.inputs.get_mut(input_id) {
                                        input.value.set_actual_value(value);
                                    }
                                }
                            }
                        }
                        _ => {
                            // Other responses not yet handled
                        }
                    }
                }
            });

        // Detect right-click in editor to open context menu
        if ctx.input(|i| i.pointer.secondary_clicked()) && cursor_in_editor {
            // Only open if not already showing a menu
            if self.user_state.context_menu_pos.is_none() {
                self.user_state.context_menu_pos = ctx.input(|i| i.pointer.interact_pos());
            }
        }

        // Show custom context menu for adding nodes
        if let Some(menu_pos) = self.user_state.context_menu_pos {
            let mut close_menu = false;
            let mut template_to_create: Option<SynthNodeTemplate> = None;

            let menu_id = egui::Id::new("add_node_context_menu");
            let menu_response = egui::Area::new(menu_id)
                .fixed_pos(menu_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(120.0);

                        for (category, templates) in AllNodeTemplates::by_category() {
                            // Show category as a submenu button
                            ui.menu_button(
                                egui::RichText::new(format!("{} ", category.name()))
                                    .color(category.color()),
                                |ui| {
                                    for template in templates {
                                        let label = template.node_finder_label(&mut self.user_state);
                                        if ui.button(label.as_ref()).clicked() {
                                            template_to_create = Some(template);
                                            close_menu = true;
                                            ui.close_menu();
                                        }
                                    }
                                },
                            );
                        }
                    });
                });

            // Close menu on click outside
            let menu_rect = menu_response.response.rect;
            if ctx.input(|i| i.pointer.any_click()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    // Check if click was outside menu (with some margin for submenus)
                    let expanded_rect = menu_rect.expand(200.0); // Allow for submenu width
                    if !expanded_rect.contains(pos) && template_to_create.is_none() {
                        close_menu = true;
                    }
                }
            }

            // Close menu on Escape
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_menu = true;
            }

            // Create node if a template was selected
            if let Some(template) = template_to_create {
                // Convert screen position to graph coordinates
                let pan = self.graph_state.pan_zoom.pan;
                let zoom = self.graph_state.pan_zoom.zoom;
                let graph_pos = (menu_pos - editor_rect.min.to_vec2() - pan) / zoom;

                // Add the node to the graph
                let node_id = self.graph_state.graph.add_node(
                    template.node_graph_label(&mut self.user_state),
                    template.user_data(&mut self.user_state),
                    |graph, node_id| template.build_node(graph, &mut self.user_state, node_id),
                );

                // Set the node position and add to node_order
                self.graph_state.node_positions.insert(node_id, graph_pos);
                self.graph_state.node_order.push(node_id);

                // Allocate engine node ID and send command
                let engine_node_id = self.user_state.allocate_engine_node_id(node_id);
                commands_to_send.push(EngineCommand::AddModule {
                    node_id: engine_node_id,
                    module_id: template.module_id(),
                });

                // Set up output monitoring for any LED indicators
                if let Some(node) = self.graph_state.graph.nodes.get(node_id) {
                    for led_indicator in &node.user_data.led_indicators {
                        commands_to_send.push(EngineCommand::MonitorOutput {
                            node_id: engine_node_id,
                            output_index: led_indicator.output_index,
                        });
                    }
                }

                close_menu = true;
            }

            if close_menu {
                self.user_state.context_menu_pos = None;
            }
        }

        // Send collected commands
        for cmd in commands_to_send {
            self.send_command(cmd);
        }

        // Remove invalid connections outside the UI closure
        for (output, input) in invalid_connections {
            self.graph_state.graph.remove_connection(input, output);
        }
    }

    /// Build a Connect command from graph port IDs.
    fn build_connect_command(
        &self,
        output: egui_node_graph2::OutputId,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        let output_data = self.graph_state.graph.get_output(output);
        let input_data = self.graph_state.graph.get_input(input);

        let from_node = self.user_state.get_engine_node_id(output_data.node)?;
        let to_node = self.user_state.get_engine_node_id(input_data.node)?;

        // Get port indices
        // For output ports, we count only output ports up to this one
        let from_port = self.get_output_port_index(output_data.node, output)?;

        // For input ports, we count only input ports up to this one
        let to_port = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::Connect {
            from_node,
            from_port,
            to_node,
            to_port,
        })
    }

    /// Build a Disconnect command from a graph input port ID.
    fn build_disconnect_command(
        &self,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        // Use get() to safely check if the input exists (avoid panic on stale IDs)
        let input_data = self.graph_state.graph.inputs.get(input)?;
        let node_id = self.user_state.get_engine_node_id(input_data.node)?;
        let port = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::Disconnect {
            node_id,
            port,
            is_input: true,
        })
    }

    /// Build a MonitorInput command if this input is an exposed parameter.
    /// Exposed parameters have both an input port and a knob at the bottom.
    fn build_monitor_input_command(
        &self,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        // Use get() to safely check if the input exists (avoid panic on stale IDs)
        let input_data = self.graph_state.graph.inputs.get(input)?;
        let node = self.graph_state.graph.nodes.get(input_data.node)?;

        // Get the input name
        let input_name = node.inputs.iter()
            .find(|(_, id)| *id == input)
            .map(|(name, _)| name)?;

        // Check if this input name corresponds to an exposed knob parameter
        let is_exposed_param = node.user_data.knob_params.iter()
            .any(|kp| kp.param_name == *input_name && kp.exposed_as_input);

        if !is_exposed_param {
            return None;
        }

        // Get engine node ID and input port index
        let engine_node_id = self.user_state.get_engine_node_id(input_data.node)?;
        let input_index = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::MonitorInput {
            node_id: engine_node_id,
            input_index,
        })
    }

    /// Build an UnmonitorInput command for a given input.
    fn build_unmonitor_input_command(
        &self,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        // Use get() to safely check if the input exists (avoid panic on stale IDs)
        let input_data = self.graph_state.graph.inputs.get(input)?;
        let node = self.graph_state.graph.nodes.get(input_data.node)?;

        // Get the input name
        let input_name = node.inputs.iter()
            .find(|(_, id)| *id == input)
            .map(|(name, _)| name)?;

        // Check if this input name corresponds to an exposed knob parameter
        let is_exposed_param = node.user_data.knob_params.iter()
            .any(|kp| kp.param_name == *input_name && kp.exposed_as_input);

        if !is_exposed_param {
            return None;
        }

        // Get engine node ID and input port index
        let engine_node_id = self.user_state.get_engine_node_id(input_data.node)?;
        let input_index = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::UnmonitorInput {
            node_id: engine_node_id,
            input_index,
        })
    }

    /// Get the DspModule port index for a given egui output ID.
    ///
    /// In egui_node_graph2, outputs are numbered separately from inputs.
    /// In DspModule, all ports are in a single array with inputs first.
    /// So we need to offset the egui output index by the number of input ports.
    fn get_output_port_index(
        &self,
        node_id: egui_node_graph2::NodeId,
        output_id: egui_node_graph2::OutputId,
    ) -> Option<usize> {
        let node = self.graph_state.graph.nodes.get(node_id)?;

        // Count the number of connectable inputs (ConnectionOnly or ConnectionOrConstant)
        // These map to DspModule input ports
        let num_input_ports = node.inputs
            .iter()
            .filter(|(_, id)| {
                let input = self.graph_state.graph.get_input(*id);
                matches!(
                    input.kind,
                    egui_node_graph2::InputParamKind::ConnectionOnly
                        | egui_node_graph2::InputParamKind::ConnectionOrConstant
                )
            })
            .count();

        // Find the egui output index
        let egui_output_idx = node.outputs
            .iter()
            .position(|(_, id)| *id == output_id)?;

        // DspModule port index = num_input_ports + egui_output_index
        Some(num_input_ports + egui_output_idx)
    }

    /// Get the DspModule port index for a given egui input ID.
    ///
    /// Both ConnectionOnly and ConnectionOrConstant inputs map to DspModule input ports.
    /// ConstantOnly inputs do NOT have ports (they are parameter-only).
    fn get_input_port_index(
        &self,
        node_id: egui_node_graph2::NodeId,
        input_id: egui_node_graph2::InputId,
    ) -> Option<usize> {
        let node = self.graph_state.graph.nodes.get(node_id)?;

        // Count connectable inputs (ConnectionOnly or ConnectionOrConstant) up to target
        let mut port_index = 0;
        for (_, id) in &node.inputs {
            let input = self.graph_state.graph.get_input(*id);

            // Check if this input can accept connections
            let is_connectable = matches!(
                input.kind,
                egui_node_graph2::InputParamKind::ConnectionOnly
                    | egui_node_graph2::InputParamKind::ConnectionOrConstant
            );

            if *id == input_id {
                // Return port index if this input can accept connections
                if is_connectable {
                    return Some(port_index);
                } else {
                    // ConstantOnly inputs don't map to DspModule ports
                    return None;
                }
            }

            // Count connectable inputs as ports
            if is_connectable {
                port_index += 1;
            }
        }

        None
    }

    /// Sync parameter values from the graph UI to the audio engine.
    ///
    /// This iterates through all nodes and their parameters, compares against
    /// cached values, and sends SetParameter commands for any changes.
    fn sync_parameters(&mut self) {
        let mut commands_to_send: Vec<EngineCommand> = Vec::new();

        // Iterate through all nodes
        for (node_id, node) in self.graph_state.graph.nodes.iter() {
            // Get the engine node ID for this graph node
            let Some(engine_node_id) = self.user_state.get_engine_node_id(node_id) else {
                continue;
            };

            // Track which param index we're at (only count ConstantOnly params)
            let mut param_index = 0;

            // Iterate through inputs to find parameters
            for (_param_name, input_id) in &node.inputs {
                let input = self.graph_state.graph.get_input(*input_id);

                // Only process ConstantOnly or ConnectionOrConstant params
                use egui_node_graph2::InputParamKind;
                match input.kind {
                    InputParamKind::ConstantOnly | InputParamKind::ConnectionOrConstant => {
                        // Get the actual value (not normalized) for the audio engine
                        // This ensures frequency values are in Hz, time values in seconds, etc.
                        let actual_value = input.value.actual_value();

                        // Create cache key
                        let cache_key = (engine_node_id, param_index);

                        // Check if value has changed (use relative tolerance for large values like frequency)
                        let needs_update = match self.cached_params.get(&cache_key) {
                            Some(&cached_value) => {
                                let diff = (actual_value - cached_value).abs();
                                let threshold = if actual_value.abs() > 10.0 {
                                    actual_value.abs() * 0.0001 // Relative tolerance for large values
                                } else {
                                    0.0001 // Absolute tolerance for small values
                                };
                                diff > threshold
                            }
                            None => true, // New parameter, needs initial sync
                        };

                        if needs_update {
                            commands_to_send.push(EngineCommand::SetParameter {
                                node_id: engine_node_id,
                                param_index,
                                value: actual_value,
                            });
                            self.cached_params.insert(cache_key, actual_value);
                        }

                        param_index += 1;
                    }
                    InputParamKind::ConnectionOnly => {
                        // This is a connection-only port, not a parameter
                        // Don't increment param_index
                    }
                }
            }
        }

        // Send collected commands
        for cmd in commands_to_send {
            self.send_command(cmd);
        }
    }

    /// Validate a connection and return an error message if invalid.
    ///
    /// Returns None if the connection is valid, Some(error_msg) if invalid.
    fn validate_and_check_connection(
        &self,
        output: egui_node_graph2::OutputId,
        input: egui_node_graph2::InputId,
    ) -> Option<String> {
        // Get the signal types for both ports
        let output_type = self.graph_state.graph
            .any_param_type(AnyParameterId::Output(output))
            .ok()
            .map(|dt| dt.signal_type());

        let input_type = self.graph_state.graph
            .any_param_type(AnyParameterId::Input(input))
            .ok()
            .map(|dt| dt.signal_type());

        match (output_type, input_type) {
            (Some(from_type), Some(to_type)) => {
                let result = validate_connection(from_type, to_type);
                if !result.is_valid() {
                    result.error_message().map(|s| s.to_string())
                } else {
                    None
                }
            }
            _ => {
                // Couldn't get types - shouldn't happen
                Some("Could not determine port types".to_string())
            }
        }
    }

    /// Check if a connection between two nodes would create a self-loop.
    #[allow(dead_code)]
    fn is_self_connection(
        &self,
        output: egui_node_graph2::OutputId,
        input: egui_node_graph2::InputId,
    ) -> bool {
        let output_node = self.graph_state.graph.get_output(output).node;
        let input_node = self.graph_state.graph.get_input(input).node;
        output_node == input_node
    }

    /// Draw the bottom status bar
    fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Priority order: validation message > audio error > status
            if let Some(validation_msg) = self.user_state.validation_message() {
                // Show validation error with warning icon
                ui.label(RichText::new(format!("⚠ {}", validation_msg))
                    .color(theme::accent::WARNING)
                    .small());
            } else if let Some(ref error) = self.audio_error_message {
                // Show audio error
                ui.label(RichText::new(format!("⚠ {}", error))
                    .color(theme::accent::ERROR)
                    .small());
            } else {
                // Show node and connection count
                let node_count = self.graph_state.graph.nodes.len();
                let connection_count = self.graph_state.graph.iter_connections().count();

                let status = if node_count == 0 {
                    "Right-click to add nodes".to_string()
                } else if connection_count == 0 {
                    format!("{} node{}", node_count, if node_count == 1 { "" } else { "s" })
                } else {
                    format!(
                        "{} node{}, {} connection{}",
                        node_count,
                        if node_count == 1 { "" } else { "s" },
                        connection_count,
                        if connection_count == 1 { "" } else { "s" }
                    )
                };
                ui.label(RichText::new(status)
                    .color(theme::text::SECONDARY)
                    .small());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("Modular Synth v0.1")
                    .color(theme::text::DISABLED)
                    .small());
            });
        });
    }
}

/// Actions collected from the toolbar for deferred execution
#[derive(Default)]
struct ToolbarActions {
    toggle_playing: bool,
    select_device: Option<usize>,
    refresh_devices: bool,
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme on first frame
        if !self.theme_applied {
            theme::apply_theme(ctx);
            self.theme_applied = true;
        }

        // Process events from the audio engine
        self.process_engine_events();

        // Request continuous repaints when playing (for LED indicators and other visualizations)
        if self.is_playing {
            ctx.request_repaint();
        }

        // Top toolbar panel
        let toolbar_actions = egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::none()
                .fill(theme::background::PANEL)
                .inner_margin(egui::Margin::symmetric(0.0, 8.0)))
            .show(ctx, |ui| {
                self.draw_toolbar(ui)
            })
            .inner;

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none()
                .fill(theme::background::PANEL)
                .inner_margin(egui::Margin::symmetric(0.0, 4.0)))
            .show(ctx, |ui| {
                self.draw_status_bar(ui);
            });

        // Main content area - the node graph editor
        self.draw_main_area(ctx);

        // Sync parameter values to the audio engine
        self.sync_parameters();

        // Handle deferred actions (to avoid borrow checker issues)
        if toolbar_actions.toggle_playing {
            self.is_playing = !self.is_playing;
            self.send_command(EngineCommand::SetPlaying(self.is_playing));
        }
        if toolbar_actions.refresh_devices {
            self.refresh_devices();
        }
        if let Some(device_index) = toolbar_actions.select_device {
            self.select_device(device_index);
        }
    }
}
