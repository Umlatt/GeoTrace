# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.2] — 2026-04-19

### Added

- **Info popup (`i` key)** — in-app methodology tutorial explaining traceroute mechanics, GeoIP lookups, mismatch detection (anchor + 100 km/ms), distance estimation (haversine + 67 km/ms RTT fallback), and map rendering
- **Metric/Imperial toggle (`u` key)** — switch between km and miles for journey distance and map scale
- **International route labels** — when the route crosses country borders, source and destination names include the country (e.g., `Johannesburg, South Africa`)
- **DNS target name in title** — when a hostname is provided, the map title shows the DNS name instead of the resolved IP address
- **Origin-centered first zoom** — the first manual zoom centers on the source hop before applying the zoom
- **Journey distance in title** — estimated travel distance shown in the map title bar as `[est. X km]`
- **Compass scale** — map scale indicator displayed below the compass rose, adapts to zoom level and unit preference
- **Country field in GeoIP** — country name extracted from MaxMind database and propagated through the pipeline

### Changed

- **Mismatch indicator** — replaced `*mismatch*` suffix with a `*` prefix on the location name, shown in gray (e.g., `*~London`)
- **RTT-based distance for mismatch hops** — journey distance for flagged hops now uses RTT delta × 67 km/ms instead of unreliable geo-IP coordinates
- **Best-of-3 RTT averaging** — distance estimates use the average of the 3 fastest pings from each hop's history to smooth out jitter
- **Route table compacted** — renamed "Route" to "Hops", tighter column widths (#, IP, Snt, Ls%, RTT, Location)
- **Map title** — dynamic `Route from <source> to <destination> [est. distance]` with yellow names, white distance text
- **Location name fallback** — city → org → IP (or DNS name for destination)
- **Legend footer** — simplified to show only `? Help`
- **Version** — bumped to 1.0.2

## [1.0.1] — 2026-04-18

### Added

- **Help popup** — on-screen control reference shown at startup; dismiss with any key, reopen with `?`
- **Legend panel** — dedicated bordered box below the route table showing a two-column color grid of all 12 RTT/status tiers plus the mismatch indicator
- **Spacebar home reset** — press Space to animate back to the initial auto-zoom viewport
- **Sent count column** — `Snt` column in the sidebar shows how many probes have been sent per hop
- **RTT-colored route lines** — map route segments reflect the destination hop's current RTT color, updating live as latency changes

### Changed

- **11-tier RTT color scale** — expanded from 4 to 11 color grades (deep blue → cyan → teal → green → yellow-green → yellow → orange → red-orange → red → dark red); timeout is black
- **Gray mismatch indicator** — latency/distance mismatch hops now use gray instead of purple; only the location column is grayed, IP stays white
- **Province border accuracy** — upgraded from NaturalEarth 50m to 10m resolution (~1 MB geodata.bin, ~100K points)
- **Geographic line colors** — all border lines now use pure gray shades (no blue tint)
- **Default terminal background** — map area no longer forces a black background
- **Version footer** — `GeoTrace 1.0.1` shown right-aligned at the bottom of the map border

### Fixed

- Route line segments between estimated hops no longer always render white

## [0.1.0] — 2026-04-18

### Added

- **ICMP traceroute engine** — continuous mtr-style probing with per-hop loss %, RTT tracking, and 50-sample history
- **Session-isolated ICMP** — unique identifier per session (PID ⊕ timestamp) with full reply filtering on Echo Reply and Time Exceeded packets; multiple concurrent sessions no longer interfere
- **Braille-rendered world map** — equirectangular projection using Unicode Braille characters (U+2800–U+28FF) for 2×4 sub-cell resolution
- **Geographic borders** — NaturalEarth coastlines, country borders, and state/province boundaries embedded as a compact 98 KB binary; detail levels adapt to zoom level
- **Wu's anti-aliased line drawing** — smooth diagonal rendering for map borders; thick (2-dot) lines for route paths
- **GeoIP location** — MaxMind GeoLite2-City database embedded directly in the binary (~57 MB); no external files needed at runtime
- **RTT-based location validation** — haversine distance vs. round-trip time plausibility check (~100 km/ms limit); suspect locations shown with purple `@` pins and `~` city prefix
- **Animated zoom** — world map shown on startup; smooth lerp zoom (6%/frame) to route bounding box after first trace pass completes
- **Interactive controls**:
  - `+` / `=` — zoom in (20% per press)
  - `-` / `_` — zoom out (20% per press)
  - Arrow keys — pan viewport (15% per press)
  - `q` / `Esc` — quit
- **Sidebar** — hop table with columns: Hop, IP, Loss%, RTT (color-coded), Location (coordinates + city)
- **Purple mismatch indicator** — hops where GeoIP location doesn't match RTT-implied distance are highlighted in purple (IP, location, and map pin)
- **Legend** — bottom of sidebar shows `@ = latency/distance mismatch`
- **Compass** — N/W+E/S compass rose in the top-right corner of the map
- **Terminal title** — set to `geotrace — <target>` on launch
- **Status bar** — error messages (e.g., missing `cap_net_raw`) shown at the bottom of the map area
- **CLI** — `--max-hops` flag (default 30); positional target argument
- **Makefile** — `build`, `setcap`, `run`, and `clean` targets

### Technical

- Rust 2021 edition, release profile with `opt-level = 3` + LTO
- Async runtime: tokio with mpsc channels between network probe and UI tasks
- TUI: ratatui 0.28 + crossterm 0.28 at 30 FPS
- Raw sockets: pnet 0.34 with `cap_net_raw+ep` via `setcap`
- GeoIP: maxminddb 0.24 with in-memory reader from `include_bytes!`
- DNS: dns-lookup 2 for reverse lookups
- Geodata pipeline: Python script (`scripts/fetch_geodata.py`) downloads NaturalEarth GeoJSON, applies Douglas-Peucker simplification, outputs compact binary
