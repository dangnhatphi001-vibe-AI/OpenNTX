// ui.rs — Production-ready TUI renderer for OpenNTX.

use crate::app::{AppMode, AppState, Feedback};
use openntx_core::registry::RegisteredApp;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, List, ListItem, Paragraph, Row, Table, Wrap},
    Frame,
};

/// Format files/metadata size in the app's registered directory.
fn get_app_size(app: &RegisteredApp) -> String {
    if let Some(app_dir) = app.manifest_path.parent() {
        if let Ok(meta) = std::fs::metadata(&app.manifest_path) {
            let mut total_bytes = 0;
            if let Ok(entries) = std::fs::read_dir(app_dir) {
                for entry in entries.flatten() {
                    if let Ok(m) = entry.metadata() {
                        if m.is_file() {
                            total_bytes += m.len();
                        }
                    }
                }
            }
            if total_bytes == 0 {
                total_bytes = meta.len();
            }
            if total_bytes < 1024 {
                return format!("{} B", total_bytes);
            } else if total_bytes < 1024 * 1024 {
                return format!("{:.1} KB", total_bytes as f64 / 1024.0);
            } else {
                return format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0));
            }
        }
    }
    "N/A".to_string()
}

/// Main drawing entry point. Called from the terminal rendering loop.
pub fn draw(f: &mut Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header (Top - 3 lines)
            Constraint::Min(5),    // Body (Middle)
            Constraint::Length(1), // Footer (Bottom - 1 line)
        ])
        .split(f.area());

    // ── 1. Header (Top - 3 lines) ──
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let header_inner = header_block.inner(chunks[0]);
    f.render_widget(header_block, chunks[0]);

    let header_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(header_inner);

    // Left header content: Logo & Version
    let logo_spans = vec![
        Span::styled(" ❖ OpenNTX ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("v1.1.0", Style::default().fg(Color::White)),
        Span::styled("  [System Admin Control Center]", Style::default().fg(Color::DarkGray)),
    ];
    f.render_widget(Paragraph::new(Line::from(logo_spans)), header_split[0]);

    // Right header content: System status & Spinner
    let mut status_spans = vec![
        Span::styled("System Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled("ACTIVE", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ];
    if app.is_loading {
        status_spans.push(Span::styled(
            format!(" [{}] LOADING...", app.spinner_char()),
            Style::default().fg(Color::Yellow),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(status_spans)).alignment(Alignment::Right),
        header_split[1],
    );

    // ── 2. Body (Middle) ──
    let body_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Sidebar
            Constraint::Percentage(80), // Main Content
        ])
        .split(chunks[1]);

    // Sidebar: Menu List
    let active_sidebar_idx = match app.mode {
        AppMode::Library | AppMode::AppDetails => Some(0),
        AppMode::Analyze | AppMode::InstallPlan => Some(1),
        AppMode::Capture | AppMode::CaptureAction => Some(2),
        AppMode::Package => Some(3),
        AppMode::Logs => Some(4),
        _ => None,
    };

    let menu_items = vec!["Library", "Analyzer", "Capture", "Builder", "Logs"];
    let sidebar_items: Vec<ListItem> = menu_items
        .iter()
        .enumerate()
        .map(|(i, name)| {
            if Some(i) == active_sidebar_idx {
                ListItem::new(Line::from(vec![
                    Span::styled(" ❯ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(*name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]))
                .style(Style::default().bg(Color::Rgb(15, 30, 45)))
            } else {
                ListItem::new(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(*name, Style::default().fg(Color::Gray)),
                ]))
            }
        })
        .collect();

    let sidebar_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Menu ");
    f.render_widget(List::new(sidebar_items).block(sidebar_block), body_split[0]);

    // Main Content (Dynamic panel)
    match app.mode {
        AppMode::Library => {
            draw_library(f, body_split[1], app);
        }
        AppMode::Capture | AppMode::Package => {
            draw_library_table(f, body_split[1], app);
        }
        AppMode::Analyze => {
            draw_analyzer_zone(f, body_split[1], app);
        }
        AppMode::Home => {
            draw_home_dashboard(f, body_split[1], app);
        }
        AppMode::AppDetails => {
            draw_app_details(f, body_split[1], app);
        }
        AppMode::CaptureAction => {
            draw_capture_actions(f, body_split[1], app);
        }
        AppMode::Logs => {
            draw_logs_list(f, body_split[1], app);
        }
        AppMode::Doctor => {
            draw_doctor_menu(f, body_split[1], app);
        }
        AppMode::DoctorResult | AppMode::InstallPlan => {
            draw_json_viewer(f, body_split[1], app);
        }
        AppMode::Settings => {
            draw_settings_page(f, body_split[1], app);
        }
    }

    // ── 3. Footer (Bottom - 1 line) ──
    let footer_block = Block::default().style(Style::default().bg(Color::Rgb(15, 15, 20)));
    let footer_spans = match &app.feedback {
        Feedback::None => vec![
            Span::styled("[↑↓] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("Điều hướng", Style::default().fg(Color::White)),
            Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Enter] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("Chọn", Style::default().fg(Color::White)),
            Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc/Backspace] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("Quay lại", Style::default().fg(Color::White)),
            Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[q] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Thoát", Style::default().fg(Color::White)),
        ],
        Feedback::Info(msg) => vec![
            Span::styled("ℹ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(msg, Style::default().fg(Color::Cyan)),
        ],
        Feedback::Success(msg) => vec![
            Span::styled("✔ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(msg, Style::default().fg(Color::Green)),
        ],
        Feedback::Error(msg) => vec![
            Span::styled("✘ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(msg, Style::default().fg(Color::Red)),
        ],
    };

    f.render_widget(
        Paragraph::new(Line::from(footer_spans)).block(footer_block),
        chunks[2],
    );
}

// ── Render Helpers ───────────────────────────────────────────────────────────

pub fn draw_library(f: &mut Frame, area: ratatui::layout::Rect, app: &AppState) {
    let header_cells = ["App Name", "App ID", "Version", "Architecture"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app.profiles.iter().enumerate().map(|(i, p)| {
        let arch_color = match p.metadata.arch {
            openntx_core::profile::Arch::X86_64 => Color::Green,
            _ => Color::Yellow, // X86 hoặc Unknown
        };
        
        let row = Row::new(vec![
            Cell::from(p.metadata.name.clone()),
            Cell::from(p.app_id.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(p.metadata.version.clone()),
            Cell::from(format!("{:?}", p.metadata.arch)).style(Style::default().fg(arch_color)),
        ]);

        if i == app.selected_profile_index {
            row.style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
        } else {
            row
        }
    }).collect();

    let table = Table::new(rows, [
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Local Profile Database "));

    f.render_widget(table, area);
}

fn draw_library_table(f: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" App Registry Library ");

    if app.library_apps.is_empty() {
        let p = Paragraph::new("\n\nNo registered apps found.\nUse the Analyzer or register an app first.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let mut rows = Vec::new();
    for (i, app_item) in app.library_apps.iter().enumerate() {
        let is_selected = i == app.selected_index;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let size_str = get_app_size(app_item);
        let prefix = if is_selected { "▶ " } else { "  " };

        let name_cell = Cell::from(format!("{}{}", prefix, app_item.name));
        let id_cell = Cell::from(app_item.app_id.clone());
        let size_cell = Cell::from(size_str);
        
        let status_style = match app_item.status.as_str() {
            s if s.contains("error") => Style::default().fg(Color::Red),
            s if s.contains("registered") => Style::default().fg(Color::Green),
            _ => Style::default().fg(Color::Yellow),
        };
        let status_cell = Cell::from(Span::styled(app_item.status.clone(), status_style));

        rows.push(Row::new(vec![name_cell, id_cell, size_cell, status_cell]).style(style));
    }

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths)
        .block(block)
        .header(
            Row::new(vec![
                Cell::from("Name").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("App ID").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("Size").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("Status").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ])
            .bottom_margin(1),
        );

    f.render_widget(table, area);
}

fn draw_analyzer_zone(f: &mut Frame, area: Rect, _app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" PE / EXE Analyzer ");

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let vertical_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(inner_area.height.saturating_sub(3) / 2),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner_area);

    let banner = Paragraph::new("📥 Drop file or Enter path here")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    f.render_widget(banner, vertical_split[1]);
}

fn draw_home_dashboard(f: &mut Frame, area: Rect, _app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Control Center Overview ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let info_text = vec![
        Line::from(vec![
            Span::styled("Welcome to OpenNTX TUI Dashboard", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("OpenNTX", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" manages Windows applications safely on Linux systems using Wine sandbox containers."),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Quickstart Shortcuts:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  • Press [1] or select 'Library' to view registered apps."),
        Line::from("  • Press [2] or select 'Analyzer' to inspect new PE/EXE installers."),
        Line::from("  • Press [4] or select 'Capture' to snapshot changes before/after installs."),
        Line::from("  • Press [5] or select 'Builder' to bundle registry/sandboxes to .deb packages."),
        Line::from("  • Press [6] or select 'Logs' to inspect standard output or auditing traces."),
    ];

    let p = Paragraph::new(info_text).wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

fn draw_app_details(f: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" App Manifest Detail ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    if let Some(m) = &app.manifest {
        lines.push(Line::from(vec![
            Span::styled("App ID:          ", Style::default().fg(Color::DarkGray)),
            Span::styled(&m.app_id, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Name:            ", Style::default().fg(Color::DarkGray)),
            Span::styled(&m.name, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Architecture:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(&m.architecture, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Install Mode:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(&m.install_mode, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Sandbox Profile: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&m.sandbox.profile, Style::default().fg(Color::Green)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Desktop Launcher:", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if app.desktop_exists { "✓ Present" } else { "✗ Missing" },
                if app.desktop_exists { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Yellow) },
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Actions Available on Selected App:",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  [R] Run plan          [C] Create launcher   [X] Remove launcher"));
        lines.push(Line::from("  [1] Snapshot before   [2] Snapshot after    [3] Diff"));
        lines.push(Line::from("  [4] Report            [5] Status            [P] Build Package"));
        lines.push(Line::from("  [L] Show Logs         [G] Run Doctor"));
    } else {
        lines.push(Line::from("No manifest metadata loaded."));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

fn draw_capture_actions(f: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Capture Subsystem ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let actions = [
        "Snapshot Before (Scan clean state)",
        "Snapshot After (Scan dirty post-install state)",
        "Registry & Filesystem Diff",
        "Generate Sandbox Policy Rules",
        "View Capture Status",
        "Clean Temporary Artifacts",
    ];

    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let is_selected = i == app.selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "▶ " } else { "  " };
            ListItem::new(Line::from(Span::styled(format!("{}{}", prefix, name), style)))
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

fn draw_logs_list(f: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Audit & Event Logs ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.logs_list.is_empty() {
        let p = Paragraph::new("No log files recorded yet.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(p, inner);
        return;
    }

    let items: Vec<ListItem> = app
        .logs_list
        .iter()
        .take(20)
        .enumerate()
        .map(|(i, log)| {
            let is_selected = i == app.selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "▶ " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{} [{}] {} | {} (Status: {})", prefix, log.timestamp, log.app_id, log.app_name, log.status),
                style,
            )))
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

fn draw_doctor_menu(f: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" System & Application Doctor ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items = [
        "Run Global Sandbox/Environment Doctor Check",
        "Run App-Specific Dependency/Prefix Doctor Check",
    ];

    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let is_selected = i == app.selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "▶ " } else { "  " };
            Line::from(Span::styled(format!("{}{}", prefix, name), style))
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_json_viewer(f: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Output Payload (JSON Mode) ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let json = app
        .last_doctor_json
        .as_ref()
        .or(app.last_capture_json.as_ref())
        .or(app.last_package_json.as_ref());

    match json {
        Some(v) => {
            let text = serde_json::to_string_pretty(v).unwrap_or_else(|_| "Serialization error".to_string());
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        }
        None => {
            f.render_widget(
                Paragraph::new("No active result report or manifest JSON to show.")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow)),
                inner,
            );
        }
    }
}

fn draw_settings_page(f: &mut Frame, area: Rect, _app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Options & Settings ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = format!(
        "OpenNTX Configuration\n\
         ──────────────────────────────────────────────────\n\
         Sandbox Default Isolation Profile:  standard\n\
         Container Engine:                    Wine-proton-flatpak\n\
         Automatic Manifest Gen:             enabled\n\
         Active TUI Palette Theme:            System Admin Dark\n\
         Target Linux Distro:                 Ubuntu / Debian compat\n\
         \n\
         Control center state: ACTIVE\n\
         Core Version: {}\n\
         \n\
         Sandbox constraints and run plans are ready to deploy.",
         crate::app::VERSION
    );

    f.render_widget(Paragraph::new(text), inner);
}
