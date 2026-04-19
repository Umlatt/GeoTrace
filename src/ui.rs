use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};
use std::net::IpAddr;

use crate::geodata;

#[derive(Debug, Clone, Default)]
pub struct HopInfo {
    pub hop_num: u8,
    pub ip: Option<IpAddr>,
    pub hostname: Option<String>,
    pub last_rtt: Option<f64>,
    pub rtt_history: Vec<f64>,
    pub loss_pct: f64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub org: Option<String>,
    pub timeout: bool,
    pub sent: u32,
    pub lost: u32,
    pub location_estimated: bool,
}

#[derive(Debug)]
pub struct AppState {
    pub hops: Vec<HopInfo>,
    pub trace_complete: bool,
    pub polylines: Vec<geodata::Polyline>,
    #[allow(dead_code)]
    pub labels: Vec<geodata::TerritoryLabel>,
    pub status: Option<String>,
    /// Current viewport bounds for animated zoom
    pub view_min_lat: f64,
    pub view_max_lat: f64,
    pub view_min_lon: f64,
    pub view_max_lon: f64,
    /// Target viewport (computed from hop coords once trace is complete)
    pub target_min_lat: Option<f64>,
    pub target_max_lat: Option<f64>,
    pub target_min_lon: Option<f64>,
    pub target_max_lon: Option<f64>,
    /// Home viewport (saved from first autozoom for spacebar reset)
    pub home_min_lat: Option<f64>,
    pub home_max_lat: Option<f64>,
    pub home_min_lon: Option<f64>,
    pub home_max_lon: Option<f64>,
    /// Show help popup
    pub show_help: bool,
    /// Use metric (true) or imperial (false)
    pub use_metric: bool,
    /// Whether the user has manually zoomed/panned yet
    pub user_interacted: bool,
    /// Original target name (DNS name provided by user)
    pub target_name: Option<String>,
    /// Show info/methodology popup
    pub show_info: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            hops: Vec::new(),
            trace_complete: false,
            polylines: Vec::new(),
            labels: Vec::new(),
            status: None,
            // Start with full world view
            view_min_lat: -90.0,
            view_max_lat: 90.0,
            view_min_lon: -180.0,
            view_max_lon: 180.0,
            target_min_lat: None,
            target_max_lat: None,
            target_min_lon: None,
            target_max_lon: None,
            home_min_lat: None,
            home_max_lat: None,
            home_min_lon: None,
            home_max_lon: None,
            show_help: false,
            use_metric: true,
            user_interacted: false,
            target_name: None,
            show_info: false,
        }
    }
}

pub fn draw(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    draw_sidebar(f, state, chunks[0]);

    if let Some(ref status) = state.status {
        let map_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(chunks[1]);
        draw_map(f, state, map_chunks[0]);
        let status_line = Paragraph::new(status.as_str())
            .style(Style::default().fg(Color::Red).bg(Color::Black));
        f.render_widget(status_line, map_chunks[1]);
    } else {
        draw_map(f, state, chunks[1]);
    }

    if state.show_help {
        draw_help_popup(f);
    }
    if state.show_info {
        draw_info_popup(f);
    }
}

fn draw_help_popup(f: &mut Frame) {
    let help_lines = vec![
        Line::from(Span::styled("  Controls", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  +/=       ", Style::default().fg(Color::White)),
            Span::styled("Zoom in", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  -/_       ", Style::default().fg(Color::White)),
            Span::styled("Zoom out", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Arrows    ", Style::default().fg(Color::White)),
            Span::styled("Pan map", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Space     ", Style::default().fg(Color::White)),
            Span::styled("Reset to auto-zoom", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  u         ", Style::default().fg(Color::White)),
            Span::styled("Toggle km / miles", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled("              (distances & scale)", Style::default().fg(Color::Rgb(80, 80, 80)))),
        Line::from(vec![
            Span::styled("  i         ", Style::default().fg(Color::White)),
            Span::styled("How it works", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  ?         ", Style::default().fg(Color::White)),
            Span::styled("Show this help", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc   ", Style::default().fg(Color::White)),
            Span::styled("Quit", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Mismatch", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("  A location prefixed with *", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  means the reported geo-IP", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  location does not align with", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  the network latency. This can", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  indicate a VPN, CDN, anycast,", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  or stale geo-IP record.", Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(Span::styled("  Press any key to dismiss", Style::default().fg(Color::DarkGray))),
    ];

    let popup_height = help_lines.len() as u16 + 2; // +2 for border
    let popup_width = 38;
    let area = f.area();
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    f.render_widget(Clear, popup_area);
    let popup = Paragraph::new(help_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .style(Style::default().fg(Color::White)),
        );
    f.render_widget(popup, popup_area);
}

fn draw_info_popup(f: &mut Frame) {
    let s_head = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let s_text = Style::default().fg(Color::DarkGray);
    let s_hi   = Style::default().fg(Color::White);

    let info_lines = vec![
        Line::from(Span::styled("  How GeoTrace Works", s_head)),
        Line::from(""),
        Line::from(Span::styled("  Traceroute", s_head)),
        Line::from(Span::styled("  ICMP Echo packets are sent with", s_text)),
        Line::from(Span::styled("  increasing TTL (Time To Live).", s_text)),
        Line::from(Span::styled("  Each router decrements the TTL;", s_text)),
        Line::from(Span::styled("  when it hits zero the router sends", s_text)),
        Line::from(Span::styled("  back a Time Exceeded reply. This", s_text)),
        Line::from(Span::styled("  reveals each hop along the path.", s_text)),
        Line::from(Span::styled("  Probes repeat continuously (like", s_text)),
        Line::from(Span::styled("  mtr) to track live RTT and loss.", s_text)),
        Line::from(""),
        Line::from(Span::styled("  GeoIP Location", s_head)),
        Line::from(Span::styled("  Each hop IP is looked up in an", s_text)),
        Line::from(Span::styled("  embedded MaxMind GeoLite2 database", s_text)),
        Line::from(Span::styled("  to resolve lat/lon coordinates,", s_text)),
        Line::from(Span::styled("  city name, and country.", s_text)),
        Line::from(""),
        Line::from(Span::styled("  Mismatch Detection", s_head)),
        Line::from(Span::styled("  The first hop with GeoIP becomes", s_text)),
        Line::from(vec![
            Span::styled("  the ", s_text),
            Span::styled("anchor", s_hi),
            Span::styled(". For each later hop:", s_text),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("    max_dist = ", s_text),
            Span::styled("RTT_delta × 100 km/ms", s_hi),
        ]),
        Line::from(""),
        Line::from(Span::styled("  100 km/ms is the theoretical max", s_text)),
        Line::from(Span::styled("  for light in fiber (with routing", s_text)),
        Line::from(Span::styled("  overhead). If the GeoIP distance", s_text)),
        Line::from(Span::styled("  from the anchor exceeds max_dist,", s_text)),
        Line::from(Span::styled("  the location is flagged with a", s_text)),
        Line::from(vec![
            Span::styled("  leading ", s_text),
            Span::styled("*", s_hi),
            Span::styled(" and shown in gray.", s_text),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Distance Estimation", s_head)),
        Line::from(Span::styled("  Total journey distance sums each", s_text)),
        Line::from(Span::styled("  hop-to-hop segment:", s_text)),
        Line::from(""),
        Line::from(vec![
            Span::styled("    Normal: ", s_text),
            Span::styled("Haversine (geo-IP coords)", s_hi),
        ]),
        Line::from(vec![
            Span::styled("    Mismatch: ", s_text),
            Span::styled("RTT_delta × 67 km/ms", s_hi),
        ]),
        Line::from(""),
        Line::from(Span::styled("  67 km/ms is a realistic average", s_text)),
        Line::from(Span::styled("  for fiber paths (vs the 100 km/ms", s_text)),
        Line::from(Span::styled("  theoretical max). RTT uses the", s_text)),
        Line::from(vec![
            Span::styled("  average of the ", s_text),
            Span::styled("3 fastest pings", s_hi),
            Span::styled(" to", s_text),
        ]),
        Line::from(Span::styled("  smooth out jitter.", s_text)),
        Line::from(""),
        Line::from(Span::styled("  Map Rendering", s_head)),
        Line::from(Span::styled("  Borders use NaturalEarth data at", s_text)),
        Line::from(Span::styled("  10m resolution with 4 detail tiers", s_text)),
        Line::from(Span::styled("  that adapt to zoom level. Lines", s_text)),
        Line::from(Span::styled("  are drawn with Wu's anti-aliasing", s_text)),
        Line::from(Span::styled("  algorithm. Route segments use the", s_text)),
        Line::from(Span::styled("  destination hop's RTT color from", s_text)),
        Line::from(Span::styled("  an 11-tier thermal scale.", s_text)),
        Line::from(""),
        Line::from(Span::styled("  Press any key to dismiss", s_text)),
    ];

    let popup_height = info_lines.len() as u16 + 2;
    let popup_width = 44;
    let area = f.area();
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    f.render_widget(Clear, popup_area);
    let popup = Paragraph::new(info_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" How It Works ")
                .style(Style::default().fg(Color::White)),
        );
    f.render_widget(popup, popup_area);
}

fn draw_sidebar(f: &mut Frame, state: &AppState, area: Rect) {
    // Split sidebar into route table + legend box
    // Legend: 6 color rows + 2 border = 8 lines
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(8)])
        .split(area);

    draw_route_table(f, state, sidebar_chunks[0]);
    draw_legend(f, state, sidebar_chunks[1]);
}

fn draw_route_table(f: &mut Frame, state: &AppState, area: Rect) {
    let header = Row::new(vec![
        Cell::from("#"),
        Cell::from("IP"),
        Cell::from("Snt"),
        Cell::from("Ls%"),
        Cell::from("RTT"),
        Cell::from("Location"),
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = state
        .hops
        .iter()
        .filter(|h| h.sent > 0)
        .map(|hop| {
            let rtt_color = rtt_to_color(hop.last_rtt);
            let ip_str = hop
                .ip
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "*".to_string());
            let rtt_str = hop
                .last_rtt
                .map(|r| format!("{:.1}ms", r))
                .unwrap_or_else(|| "*".to_string());
            let coords_str = match (hop.lat, hop.lon) {
                (Some(lat), Some(lon)) => format!("[{:.2},{:.2}] ", lat, lon),
                _ => String::new(),
            };
            let loc_base = match (&hop.city, &hop.org) {
                (Some(c), Some(o)) => format!("{}{}/{}", coords_str, c, o),
                (Some(c), None) => format!("{}{}", coords_str, c),
                (None, Some(o)) => format!("{}{}", coords_str, o),
                _ => coords_str,
            };
            let loc = if hop.location_estimated && !loc_base.is_empty() {
                format!("*{}", loc_base)
            } else {
                loc_base
            };
            let ip_color = Color::White;
            let loc_color = if hop.location_estimated {
                Color::Rgb(140, 140, 140)
            } else {
                Color::White
            };

            Row::new(vec![
                Cell::from(format!("{:>2}", hop.hop_num)),
                Cell::from(ip_str).style(Style::default().fg(ip_color)),
                Cell::from(format!("{}", hop.sent)),
                Cell::from(format!("{:.0}%", hop.loss_pct)),
                Cell::from(rtt_str).style(Style::default().fg(rtt_color)),
                Cell::from(loc).style(Style::default().fg(loc_color)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(15),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(8),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Hops ")
        );

    f.render_widget(table, area);
}

fn draw_legend(f: &mut Frame, _state: &AppState, area: Rect) {
    let mismatch_gray = Color::Rgb(140, 140, 140);

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(" ■", Style::default().fg(Color::Rgb(0, 100, 255))),
            Span::styled("  < 1ms    ", Style::default().fg(Color::DarkGray)),
            Span::styled("■", Style::default().fg(Color::Rgb(255, 140, 0))),
            Span::styled("  < 150ms", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ■", Style::default().fg(Color::Rgb(0, 180, 255))),
            Span::styled("  < 5ms    ", Style::default().fg(Color::DarkGray)),
            Span::styled("■", Style::default().fg(Color::Rgb(255, 80, 0))),
            Span::styled("  < 200ms", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ■", Style::default().fg(Color::Rgb(0, 210, 180))),
            Span::styled("  < 15ms   ", Style::default().fg(Color::DarkGray)),
            Span::styled("■", Style::default().fg(Color::Rgb(255, 30, 0))),
            Span::styled("  < 300ms", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ■", Style::default().fg(Color::Rgb(0, 200, 80))),
            Span::styled("  < 30ms   ", Style::default().fg(Color::DarkGray)),
            Span::styled("■", Style::default().fg(Color::Rgb(200, 0, 0))),
            Span::styled("  > 300ms", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ■", Style::default().fg(Color::Rgb(180, 220, 0))),
            Span::styled("  < 60ms   ", Style::default().fg(Color::DarkGray)),
            Span::styled("■", Style::default().fg(Color::Black)),
            Span::styled("  timeout", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ■", Style::default().fg(Color::Rgb(255, 200, 0))),
            Span::styled("  < 100ms  ", Style::default().fg(Color::DarkGray)),
            Span::styled("■", Style::default().fg(mismatch_gray)),
            Span::styled("  *mismatch", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    // Pad to fill the box
    while lines.len() < (area.height.saturating_sub(2)) as usize {
        lines.push(Line::from(""));
    }

    let legend = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Legend ")
                .title_bottom(
                    Line::from(vec![
                        Span::styled(" ? ", Style::default().fg(Color::Yellow)),
                        Span::styled("Help ", Style::default().fg(Color::DarkGray)),
                    ]).alignment(Alignment::Left),
                )
        );
    f.render_widget(legend, area);
}

fn format_distance(km: f64, use_metric: bool) -> String {
    if use_metric {
        if km >= 1000.0 {
            format!("{:.0}km", km)
        } else if km >= 1.0 {
            format!("{:.1}km", km)
        } else {
            format!("{:.0}m", km * 1000.0)
        }
    } else {
        let miles = km * 0.621371;
        if miles >= 1000.0 {
            format!("{:.0}mi", miles)
        } else if miles >= 1.0 {
            format!("{:.1}mi", miles)
        } else {
            format!("{:.0}ft", miles * 5280.0)
        }
    }
}

fn hop_location_name(hop: &HopInfo) -> Option<String> {
    // Try city first, then org, then IP-based fallback
    if let Some(ref city) = hop.city {
        return Some(city.clone());
    }
    if let Some(ref org) = hop.org {
        return Some(org.clone());
    }
    if let Some(ip) = hop.ip {
        return Some(ip.to_string());
    }
    None
}

fn draw_map(f: &mut Frame, state: &AppState, area: Rect) {
    let center_lat = (state.view_min_lat + state.view_max_lat) / 2.0;

    // Build source name with fallback: city -> org -> IP
    let source_hop = state.hops.iter().find(|h| h.lat.is_some());
    let source_mismatch = source_hop.map_or(false, |h| h.location_estimated);

    // Build destination name with fallback: city -> org -> IP
    // For mismatch, show the reported city/country name
    let dest_hop = state.hops.iter().rev().find(|h| h.lat.is_some());
    let dest_mismatch = dest_hop.map_or(false, |h| h.location_estimated);

    // Determine if the route crosses international borders
    let source_country = source_hop.and_then(|h| h.country.clone());
    let dest_country = dest_hop.and_then(|h| h.country.clone());
    let international = match (&source_country, &dest_country) {
        (Some(sc), Some(dc)) => sc != dc,
        _ => false,
    };

    let source_name = source_hop
        .and_then(|h| {
            let base = hop_location_name(h)?;
            if international {
                if let Some(ref country) = h.country {
                    return Some(format!("{}, {}", base, country));
                }
            }
            Some(base)
        })
        .unwrap_or_else(|| "Source".to_string());

    let dest_name = dest_hop
        .and_then(|h| {
            // Always show the reported city name even for mismatch
            let base = if let Some(ref city) = h.city {
                city.clone()
            } else if let Some(ref org) = h.org {
                org.clone()
            } else if let Some(ref name) = state.target_name {
                // Prefer DNS target name over raw IP
                name.clone()
            } else if let Some(ip) = h.ip {
                ip.to_string()
            } else {
                return None;
            };
            if international {
                if let Some(ref country) = h.country {
                    return Some(format!("{}, {}", base, country));
                }
            }
            Some(base)
        })
        .unwrap_or_else(|| state.target_name.clone().unwrap_or_else(|| "Destination".to_string()));

    let journey_km = compute_journey_distance(&state.hops);
    let journey_str = if journey_km >= 1.0 {
        format!(" [est. {}]", format_distance(journey_km, state.use_metric))
    } else {
        String::new()
    };

    let mut title_spans = vec![
        Span::styled(" Route from ", Style::default().fg(Color::White)),
    ];
    if source_mismatch {
        title_spans.push(Span::styled(format!("*{}", source_name), Style::default().fg(Color::DarkGray)));
    } else {
        title_spans.push(Span::styled(&source_name, Style::default().fg(Color::Yellow)));
    }
    title_spans.push(Span::styled(" to ", Style::default().fg(Color::White)));
    if dest_mismatch {
        title_spans.push(Span::styled(format!("*{}", dest_name), Style::default().fg(Color::DarkGray)));
    } else {
        title_spans.push(Span::styled(&dest_name, Style::default().fg(Color::Yellow)));
    }
    title_spans.push(Span::styled(journey_str, Style::default().fg(Color::White)));
    title_spans.push(Span::styled(" ", Style::default()));
    let title_spans = Line::from(title_spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title_spans)
        .title_bottom(
            Line::from(vec![
                Span::styled(" GeoTrace 1.0.2 ", Style::default().fg(Color::White)),
            ]).alignment(Alignment::Right),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    // Collect hop coordinates + estimated flag
    let coords: Vec<(f64, f64, Option<f64>, bool)> = state
        .hops
        .iter()
        .filter(|h| h.lat.is_some() && h.lon.is_some())
        .map(|h| (h.lat.unwrap(), h.lon.unwrap(), h.last_rtt, h.location_estimated))
        .collect();

    // Braille canvas: each cell is 2 wide x 4 tall in dot space
    let canvas_w = inner.width as usize * 2;
    let canvas_h = inner.height as usize * 4;

    // Use the current animated viewport
    let min_lat = state.view_min_lat;
    let max_lat = state.view_max_lat;
    let min_lon = state.view_min_lon;
    let max_lon = state.view_max_lon;

    let lat_span = max_lat - min_lat;
    let lon_span = max_lon - min_lon;

    if lat_span <= 0.0 || lon_span <= 0.0 {
        return;
    }

    // Equirectangular projection: lon -> x, lat -> y (inverted for terminal)
    let map_to_canvas = |lat: f64, lon: f64| -> (i32, i32) {
        let nx = (lon - min_lon) / lon_span;
        let ny = 1.0 - (lat - min_lat) / lat_span;
        let x = (nx * (canvas_w as f64 - 1.0)).round() as i32;
        let y = (ny * (canvas_h as f64 - 1.0)).round() as i32;
        (x, y)
    };

    let clamp_canvas = |x: i32, y: i32| -> (usize, usize) {
        (
            x.clamp(0, (canvas_w - 1) as i32) as usize,
            y.clamp(0, (canvas_h - 1) as i32) as usize,
        )
    };

    // Braille dot buffer + color buffer (per cell)
    let cell_cols = inner.width as usize;
    let cell_rows = inner.height as usize;
    let mut dots = vec![vec![false; canvas_w]; canvas_h];
    // Color layers: base is DarkGray for borders, overwritten by hop colors
    let mut cell_colors = vec![vec![Color::DarkGray; cell_cols]; cell_rows];

    // Determine visible detail level based on zoom
    let max_level = geodata::visible_levels(lat_span, lon_span);

    // Draw geographic borders
    let border_color_for_level = |level: u8| -> Color {
        match level {
            0 => Color::DarkGray,           // coastlines
            1 => Color::Rgb(100, 100, 100), // country borders
            2 => Color::Rgb(70, 70, 70),    // state/province
            _ => Color::Rgb(55, 55, 55),    // admin-2 / county
        }
    };

    for polyline in &state.polylines {
        if polyline.level > max_level {
            continue;
        }

        let color = border_color_for_level(polyline.level);

        for i in 0..polyline.points.len().saturating_sub(1) {
            let (lat0, lon0) = polyline.points[i];
            let (lat1, lon1) = polyline.points[i + 1];

            // Skip segments completely outside viewport (with margin)
            let seg_min_lat = lat0.min(lat1);
            let seg_max_lat = lat0.max(lat1);
            let seg_min_lon = lon0.min(lon1);
            let seg_max_lon = lon0.max(lon1);

            if seg_max_lat < min_lat || seg_min_lat > max_lat
                || seg_max_lon < min_lon || seg_min_lon > max_lon
            {
                continue;
            }

            // Skip very long segments that wrap around (> 90° longitude jump)
            if (lon1 - lon0).abs() > 90.0 {
                continue;
            }

            let (x0, y0) = map_to_canvas(lat0, lon0);
            let (x1, y1) = map_to_canvas(lat1, lon1);

            // Clip to canvas bounds
            let (cx0, cy0) = clamp_canvas(x0, y0);
            let (cx1, cy1) = clamp_canvas(x1, y1);

            draw_line_aa(&mut dots, cx0, cy0, cx1, cy1, canvas_w, canvas_h);

            // Set border color for affected cells
            set_line_color(&mut cell_colors, cx0, cy0, cx1, cy1, color, cell_cols, cell_rows);
        }
    }

    // Draw route lines between consecutive hops (color reflects RTT)
    let hop_coords: Vec<(i32, i32, bool, Option<f64>)> = coords
        .iter()
        .map(|&(lat, lon, rtt, estimated)| {
            let (x, y) = map_to_canvas(lat, lon);
            (x, y, estimated, rtt)
        })
        .collect();
    for i in 0..hop_coords.len().saturating_sub(1) {
        let (x0, y0) = clamp_canvas(hop_coords[i].0, hop_coords[i].1);
        let (x1, y1) = clamp_canvas(hop_coords[i + 1].0, hop_coords[i + 1].1);
        // Always use RTT color for segments (even estimated hops)
        let seg_color = rtt_to_color(hop_coords[i + 1].3);
        draw_line_thick(&mut dots, x0, y0, x1, y1, canvas_w, canvas_h);
        set_line_color(&mut cell_colors, x0, y0, x1, y1, seg_color, cell_cols, cell_rows);
    }

    // Track cell-level pin overlay: (cell_col, cell_row, color) for '@' markers
    let mut pin_cells: Vec<(usize, usize, Color)> = Vec::new();

    // Draw hop pins and set colors
    for &(lat, lon, rtt, estimated) in coords.iter() {
        let (px, py) = map_to_canvas(lat, lon);
        let color = if estimated {
            Color::Rgb(140, 140, 140) // lighter gray for estimated/suspect locations
        } else {
            rtt_to_color(rtt)
        };

        // Mark the cell containing this pin for '@' overlay
        let cx = (px.clamp(0, (canvas_w - 1) as i32) as usize) / 2;
        let cy = (py.clamp(0, (canvas_h - 1) as i32) as usize) / 4;
        if cx < cell_cols && cy < cell_rows {
            pin_cells.push((cx, cy, color));
        }
    }

    // Render braille characters
    let mut lines: Vec<Line> = Vec::with_capacity(cell_rows);
    for row in 0..cell_rows {
        let mut spans: Vec<Span> = Vec::with_capacity(cell_cols);
        for col in 0..cell_cols {
            let base_x = col * 2;
            let base_y = row * 4;

            let mut code: u32 = 0x2800;
            let offsets = [
                (0, 0, 0x01),
                (0, 1, 0x02),
                (0, 2, 0x04),
                (1, 0, 0x08),
                (1, 1, 0x10),
                (1, 2, 0x20),
                (0, 3, 0x40),
                (1, 3, 0x80),
            ];
            for &(dx, dy, bit) in &offsets {
                let x = base_x + dx;
                let y = base_y + dy;
                if x < canvas_w && y < canvas_h && dots[y][x] {
                    code |= bit;
                }
            }
            let ch = char::from_u32(code).unwrap_or(' ');
            let color = cell_colors[row][col];

            // Check if this cell has a pin overlay
            let is_pin = pin_cells.iter().any(|&(pc, pr, _)| pc == col && pr == row);
            if is_pin {
                let pin_color = pin_cells.iter()
                    .rev()
                    .find(|&&(pc, pr, _)| pc == col && pr == row)
                    .map(|&(_, _, c)| c)
                    .unwrap_or(color);
                spans.push(Span::styled(
                    "@".to_string(),
                    Style::default().fg(pin_color),
                ));
            } else {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(color),
                ));
            }
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);

    // Compass legend + scale in top-right corner of the map
    if inner.width >= 10 && inner.height >= 6 {
        let width_km = lon_span * center_lat.to_radians().cos().abs() * 111.32;
        let scale_str = format!("~{}", format_distance(width_km, state.use_metric));

        let compass_text = vec![
            Line::from(Span::styled("  N  ", Style::default().fg(Color::Yellow))).centered(),
            Line::from(Span::styled("W + E", Style::default().fg(Color::Yellow))).centered(),
            Line::from(Span::styled("  S  ", Style::default().fg(Color::Yellow))).centered(),
            Line::from(Span::styled(&scale_str, Style::default().fg(Color::Yellow))).centered(),
        ];
        let cw = 9_u16.max(scale_str.len() as u16 + 2);
        let compass_area = Rect::new(
            inner.x + inner.width.saturating_sub(cw),
            inner.y,
            cw.min(inner.width),
            4,
        );
        let compass = Paragraph::new(compass_text);
        f.render_widget(compass, compass_area);
    }
}

/// Xiaolin Wu-style anti-aliased line for binary Braille dots.
/// Plots neighbor dots on diagonals for smoother curves.
fn draw_line_aa(dots: &mut [Vec<bool>], x0: usize, y0: usize, x1: usize, y1: usize, w: usize, h: usize) {
    let (x0f, y0f, x1f, y1f) = (x0 as f64, y0 as f64, x1 as f64, y1 as f64);

    let dx = (x1f - x0f).abs();
    let dy = (y1f - y0f).abs();
    let steep = dy > dx;

    // If steep, transpose so we always iterate along the longer axis
    let (x0f, y0f, x1f, y1f) = if steep {
        (y0f, x0f, y1f, x1f)
    } else {
        (x0f, y0f, x1f, y1f)
    };

    // Ensure left-to-right
    let (x0f, y0f, x1f, y1f) = if x0f > x1f {
        (x1f, y1f, x0f, y0f)
    } else {
        (x0f, y0f, x1f, y1f)
    };

    let dxf = x1f - x0f;
    let dyf = y1f - y0f;
    let gradient = if dxf < 0.5 { dyf.signum() } else { dyf / dxf };

    let plot = |dots: &mut [Vec<bool>], px: i32, py: i32| {
        let (rx, ry) = if steep { (py, px) } else { (px, py) };
        if rx >= 0 && (rx as usize) < w && ry >= 0 && (ry as usize) < h {
            dots[ry as usize][rx as usize] = true;
        }
    };

    let ix0 = x0f.round() as i32;
    let ix1 = x1f.round() as i32;
    let mut intery = y0f + gradient * (ix0 as f64 - x0f);

    for x in ix0..=ix1 {
        let iy = intery.floor() as i32;
        let frac = intery - intery.floor();

        plot(dots, x, iy);
        // Plot neighbor pixel when fractional offset is significant
        if frac >= 0.3 {
            plot(dots, x, iy + 1);
        }

        intery += gradient;
    }
}

/// Draw a thick (2-dot) line for route paths: main line + 1px perpendicular offset
fn draw_line_thick(dots: &mut [Vec<bool>], x0: usize, y0: usize, x1: usize, y1: usize, w: usize, h: usize) {
    draw_line_aa(dots, x0, y0, x1, y1, w, h);

    let dx = (x1 as f64 - x0 as f64).abs();
    let dy = (y1 as f64 - y0 as f64).abs();

    // Offset perpendicular to the dominant direction
    if dy > dx {
        // Mostly vertical: offset in X
        let offset = |v: usize| -> usize { (v + 1).min(w.saturating_sub(1)) };
        draw_line_aa(dots, offset(x0), y0, offset(x1), y1, w, h);
    } else {
        // Mostly horizontal: offset in Y
        let offset = |v: usize| -> usize { (v + 1).min(h.saturating_sub(1)) };
        draw_line_aa(dots, x0, offset(y0), x1, offset(y1), w, h);
    }
}

/// Set cell colors along a Bresenham line path
fn set_line_color(
    cell_colors: &mut [Vec<Color>],
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    color: Color,
    cell_cols: usize,
    cell_rows: usize,
) {
    let (mut x, mut y) = (x0 as i32, y0 as i32);
    let (ex, ey) = (x1 as i32, y1 as i32);
    let dx = (ex - x).abs();
    let dy = -(ey - y).abs();
    let sx = if x < ex { 1 } else { -1 };
    let sy = if y < ey { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        let cx = (x as usize) / 2;
        let cy = (y as usize) / 4;
        if cx < cell_cols && cy < cell_rows {
            cell_colors[cy][cx] = color;
        }
        if x == ex && y == ey {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Average of the 3 fastest RTTs from a hop's history, or last_rtt as fallback.
fn best_rtt(hop: &HopInfo) -> Option<f64> {
    if hop.rtt_history.len() >= 3 {
        let mut sorted = hop.rtt_history.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let sum: f64 = sorted[..3].iter().sum();
        Some(sum / 3.0)
    } else if !hop.rtt_history.is_empty() {
        let mut sorted = hop.rtt_history.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let sum: f64 = sorted.iter().sum();
        Some(sum / sorted.len() as f64)
    } else {
        hop.last_rtt
    }
}

/// Compute total journey distance in km.
/// For mismatch hops, estimate distance from RTT delta instead of geo-IP coordinates.
/// RTT-based estimate: ~67 km per ms of RTT (speed of light in fiber with routing overhead).
const RTT_KM_PER_MS: f64 = 67.0;

fn compute_journey_distance(hops: &[HopInfo]) -> f64 {
    let valid_hops: Vec<&HopInfo> = hops
        .iter()
        .filter(|h| h.lat.is_some() && h.lon.is_some())
        .collect();

    let mut total = 0.0;
    for i in 0..valid_hops.len().saturating_sub(1) {
        let h0 = valid_hops[i];
        let h1 = valid_hops[i + 1];

        if h1.location_estimated {
            // Use RTT-based estimate for mismatch hops (avg of 3 fastest pings)
            if let (Some(rtt0), Some(rtt1)) = (best_rtt(h0), best_rtt(h1)) {
                let rtt_delta = (rtt1 - rtt0).max(0.0);
                total += rtt_delta * RTT_KM_PER_MS;
            }
            // If no RTT data, skip this segment
        } else {
            // Normal geo-IP distance
            let (lat0, lon0) = (h0.lat.unwrap(), h0.lon.unwrap());
            let (lat1, lon1) = (h1.lat.unwrap(), h1.lon.unwrap());
            total += haversine_km(lat0, lon0, lat1, lon1);
        }
    }
    total
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // Earth radius in km
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r * c
}

fn rtt_to_color(rtt: Option<f64>) -> Color {
    match rtt {
        None => Color::Black,                    // timeout — black
        Some(r) if r < 1.0   => Color::Rgb(0, 100, 255),    // <1ms — deep blue (coldest)
        Some(r) if r < 5.0   => Color::Rgb(0, 180, 255),    // <5ms — cyan-blue
        Some(r) if r < 15.0  => Color::Rgb(0, 210, 180),    // <15ms — teal
        Some(r) if r < 30.0  => Color::Rgb(0, 200, 80),     // <30ms — green
        Some(r) if r < 60.0  => Color::Rgb(180, 220, 0),    // <60ms — yellow-green
        Some(r) if r < 100.0 => Color::Rgb(255, 200, 0),    // <100ms — yellow-orange
        Some(r) if r < 150.0 => Color::Rgb(255, 140, 0),    // <150ms — orange
        Some(r) if r < 200.0 => Color::Rgb(255, 80, 0),     // <200ms — red-orange
        Some(r) if r < 300.0 => Color::Rgb(255, 30, 0),     // <300ms — red
        Some(_) => Color::Rgb(200, 0, 0),                    // 300ms+ — dark red (hottest)
    }
}
