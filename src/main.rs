mod geodata;
mod geoip;
mod network;
mod ui;

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

static MMDB_DATA: &[u8] = include_bytes!("../data/GeoLite2-City.mmdb");

#[derive(Parser, Debug)]
#[command(name = "geotrace", about = "Geographical traceroute with TUI map")]
struct Args {
    /// Target host to trace
    target: String,

    /// Maximum number of hops
    #[arg(long, default_value_t = 30)]
    max_hops: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let geo = Arc::new(geoip::GeoIpLookup::from_bytes(MMDB_DATA)?);
    let (polylines, labels) = geodata::load_geodata();
    let state = Arc::new(Mutex::new(ui::AppState {
        polylines,
        labels,
        ..Default::default()
    }));

    let (tx, mut rx) = mpsc::channel::<network::HopUpdate>(128);

    // Spawn the network probe task
    let probe_geo = geo.clone();
    let target = args.target.clone();
    let max_hops = args.max_hops;
    tokio::spawn(async move {
        network::run_traceroute(target, max_hops, tx, probe_geo).await;
    });

    // Spawn a task to receive hop updates and merge into state
    let recv_state = state.clone();
    tokio::spawn(async move {
        while let Some(update) = rx.recv().await {
            let mut st = recv_state.lock().unwrap();

            // Handle errors
            if let Some(ref err) = update.error {
                st.status = Some(err.clone());
                continue;
            }

            if update.trace_complete {
                st.trace_complete = true;
                // Compute target bounding box from all hops with coords
                let (mut tmin_lat, mut tmax_lat) = (90.0f64, -90.0f64);
                let (mut tmin_lon, mut tmax_lon) = (180.0f64, -180.0f64);
                let mut has_coords = false;
                for h in &st.hops {
                    if let (Some(lat), Some(lon)) = (h.lat, h.lon) {
                        tmin_lat = tmin_lat.min(lat);
                        tmax_lat = tmax_lat.max(lat);
                        tmin_lon = tmin_lon.min(lon);
                        tmax_lon = tmax_lon.max(lon);
                        has_coords = true;
                    }
                }
                if has_coords {
                    let lat_range = (tmax_lat - tmin_lat).max(5.0);
                    let lon_range = (tmax_lon - tmin_lon).max(10.0);
                    let lat_pad = lat_range * 0.2;
                    let lon_pad = lon_range * 0.2;
                    st.target_min_lat = Some(tmin_lat - lat_pad);
                    st.target_max_lat = Some(tmax_lat + lat_pad);
                    st.target_min_lon = Some(tmin_lon - lon_pad);
                    st.target_max_lon = Some(tmax_lon + lon_pad);
                    // Save home viewport on first compute
                    if st.home_min_lat.is_none() {
                        st.home_min_lat = Some(tmin_lat - lat_pad);
                        st.home_max_lat = Some(tmax_lat + lat_pad);
                        st.home_min_lon = Some(tmin_lon - lon_pad);
                        st.home_max_lon = Some(tmax_lon + lon_pad);
                    }
                }
                continue;
            }

            // Ensure vec is large enough
            while st.hops.len() <= update.hop as usize {
                st.hops.push(ui::HopInfo::default());
            }
            let hop = &mut st.hops[update.hop as usize];
            hop.hop_num = update.hop;
            if let Some(ip) = update.ip {
                hop.ip = Some(ip);
            }
            if let Some(hostname) = update.hostname {
                hop.hostname = Some(hostname);
            }
            if let Some(rtt) = update.rtt_ms {
                hop.last_rtt = Some(rtt);
                hop.rtt_history.push(rtt);
                if hop.rtt_history.len() > 50 {
                    hop.rtt_history.remove(0);
                }
            }
            if let Some(loss) = update.loss_pct {
                hop.loss_pct = loss;
            }
            if let Some(lat) = update.lat {
                hop.lat = Some(lat);
            }
            if let Some(lon) = update.lon {
                hop.lon = Some(lon);
            }
            if let Some(city) = update.city {
                hop.city = Some(city);
            }
            if let Some(org) = update.org {
                hop.org = Some(org);
            }
            hop.location_estimated = update.location_estimated;
            hop.timeout = update.timeout;
            hop.sent += 1;
            if update.timeout {
                hop.lost += 1;
            }
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::terminal::SetTitle(format!("geotrace — {}", args.target))
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(33); // ~30 FPS
    let mut last_tick = Instant::now();
    let zoom_speed = 0.06; // interpolation factor per frame (~1s to converge)

    loop {
        // Animate zoom if targets are set
        {
            let mut st = state.lock().unwrap();
            if let (Some(tmin_lat), Some(tmax_lat), Some(tmin_lon), Some(tmax_lon)) = (
                st.target_min_lat,
                st.target_max_lat,
                st.target_min_lon,
                st.target_max_lon,
            ) {
                st.view_min_lat += (tmin_lat - st.view_min_lat) * zoom_speed;
                st.view_max_lat += (tmax_lat - st.view_max_lat) * zoom_speed;
                st.view_min_lon += (tmin_lon - st.view_min_lon) * zoom_speed;
                st.view_max_lon += (tmax_lon - st.view_max_lon) * zoom_speed;
            }
        }

        terminal.draw(|f| {
            let st = state.lock().unwrap();
            ui::draw(f, &st);
        })?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Dismiss help popup on any key
                    {
                        let mut st = state.lock().unwrap();
                        if st.show_help {
                            st.show_help = false;
                            continue;
                        }
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('?') => {
                            let mut st = state.lock().unwrap();
                            st.show_help = true;
                        }
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            // Zoom in (shrink viewport by 30% toward center)
                            let mut st = state.lock().unwrap();
                            let lat_shrink = (st.view_max_lat - st.view_min_lat) * 0.15;
                            let lon_shrink = (st.view_max_lon - st.view_min_lon) * 0.15;
                            st.view_min_lat += lat_shrink;
                            st.view_max_lat -= lat_shrink;
                            st.view_min_lon += lon_shrink;
                            st.view_max_lon -= lon_shrink;
                            // Disable auto-zoom when user takes control
                            st.target_min_lat = None;
                            st.target_max_lat = None;
                            st.target_min_lon = None;
                            st.target_max_lon = None;
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            // Zoom out (grow viewport by 30% from center)
                            let mut st = state.lock().unwrap();
                            let lat_grow = (st.view_max_lat - st.view_min_lat) * 0.15;
                            let lon_grow = (st.view_max_lon - st.view_min_lon) * 0.15;
                            st.view_min_lat = (st.view_min_lat - lat_grow).max(-90.0);
                            st.view_max_lat = (st.view_max_lat + lat_grow).min(90.0);
                            st.view_min_lon = (st.view_min_lon - lon_grow).max(-180.0);
                            st.view_max_lon = (st.view_max_lon + lon_grow).min(180.0);
                            st.target_min_lat = None;
                            st.target_max_lat = None;
                            st.target_min_lon = None;
                            st.target_max_lon = None;
                        }
                        KeyCode::Up => {
                            let mut st = state.lock().unwrap();
                            let pan = (st.view_max_lat - st.view_min_lat) * 0.15;
                            if st.view_max_lat + pan <= 90.0 {
                                st.view_min_lat += pan;
                                st.view_max_lat += pan;
                            }
                            st.target_min_lat = None;
                            st.target_max_lat = None;
                            st.target_min_lon = None;
                            st.target_max_lon = None;
                        }
                        KeyCode::Down => {
                            let mut st = state.lock().unwrap();
                            let pan = (st.view_max_lat - st.view_min_lat) * 0.15;
                            if st.view_min_lat - pan >= -90.0 {
                                st.view_min_lat -= pan;
                                st.view_max_lat -= pan;
                            }
                            st.target_min_lat = None;
                            st.target_max_lat = None;
                            st.target_min_lon = None;
                            st.target_max_lon = None;
                        }
                        KeyCode::Right => {
                            let mut st = state.lock().unwrap();
                            let pan = (st.view_max_lon - st.view_min_lon) * 0.15;
                            if st.view_max_lon + pan <= 180.0 {
                                st.view_min_lon += pan;
                                st.view_max_lon += pan;
                            }
                            st.target_min_lat = None;
                            st.target_max_lat = None;
                            st.target_min_lon = None;
                            st.target_max_lon = None;
                        }
                        KeyCode::Left => {
                            let mut st = state.lock().unwrap();
                            let pan = (st.view_max_lon - st.view_min_lon) * 0.15;
                            if st.view_min_lon - pan >= -180.0 {
                                st.view_min_lon -= pan;
                                st.view_max_lon -= pan;
                            }
                            st.target_min_lat = None;
                            st.target_max_lat = None;
                            st.target_min_lon = None;
                            st.target_max_lon = None;
                        }
                        KeyCode::Char(' ') => {
                            // Reset to home autozoom
                            let mut st = state.lock().unwrap();
                            if let (Some(h1), Some(h2), Some(h3), Some(h4)) = (
                                st.home_min_lat, st.home_max_lat,
                                st.home_min_lon, st.home_max_lon,
                            ) {
                                st.target_min_lat = Some(h1);
                                st.target_max_lat = Some(h2);
                                st.target_min_lon = Some(h3);
                                st.target_max_lon = Some(h4);
                            }
                        }
                        KeyCode::Char('u') => {
                            let mut st = state.lock().unwrap();
                            st.use_metric = !st.use_metric;
                        }
                        _ => {}
                    }
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
