/// Geographic border data for rendering on the Braille map.
///
/// Binary format:
///   u32 LE: number of polylines
///   Per polyline:
///     u8:  detail level (0=coastline, 1=country, 2=state/province, 3=admin-2)
///     u32 LE: number of points
///     Per point: i32 LE lat*1000000, i32 LE lon*1000000
///
///   Then (if data remains):
///   u32 LE: number of labels
///   Per label:
///     u8:  level (0=country, 1=state/province)
///     i32 LE: lat*1000000
///     i32 LE: lon*1000000
///     u16 LE: name length in bytes
///     [u8]:  name (UTF-8)

static GEODATA: &[u8] = include_bytes!("../data/geodata.bin");

#[derive(Debug, Clone)]
pub struct Polyline {
    pub level: u8,
    pub points: Vec<(f64, f64)>, // (lat, lon)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TerritoryLabel {
    pub level: u8,          // 0=country, 1=state/province
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

/// Parse all polylines and territory labels from the embedded binary data.
pub fn load_geodata() -> (Vec<Polyline>, Vec<TerritoryLabel>) {
    let data = GEODATA;
    let mut offset = 0;

    let count = read_u32(data, &mut offset) as usize;
    let mut polylines = Vec::with_capacity(count);

    for _ in 0..count {
        let level = data[offset];
        offset += 1;

        let n_points = read_u32(data, &mut offset) as usize;
        let mut points = Vec::with_capacity(n_points);
        for _ in 0..n_points {
            let lat_raw = read_i32(data, &mut offset);
            let lon_raw = read_i32(data, &mut offset);
            points.push((lat_raw as f64 / 1_000_000.0, lon_raw as f64 / 1_000_000.0));
        }
        polylines.push(Polyline { level, points });
    }

    // Parse labels if data remains
    let mut labels = Vec::new();
    if offset + 4 <= data.len() {
        let label_count = read_u32(data, &mut offset) as usize;
        labels.reserve(label_count);
        for _ in 0..label_count {
            if offset >= data.len() {
                break;
            }
            let level = data[offset];
            offset += 1;
            let lat_raw = read_i32(data, &mut offset);
            let lon_raw = read_i32(data, &mut offset);
            let name_len = read_u16(data, &mut offset) as usize;
            let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
            offset += name_len;
            labels.push(TerritoryLabel {
                level,
                lat: lat_raw as f64 / 1_000_000.0,
                lon: lon_raw as f64 / 1_000_000.0,
                name,
            });
        }
    }

    (polylines, labels)
}

fn read_u32(data: &[u8], offset: &mut usize) -> u32 {
    let val = u32::from_le_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
    ]);
    *offset += 4;
    val
}

fn read_i32(data: &[u8], offset: &mut usize) -> i32 {
    let val = i32::from_le_bytes([
        data[*offset],
        data[*offset + 1],
        data[*offset + 2],
        data[*offset + 3],
    ]);
    *offset += 4;
    val
}

fn read_u16(data: &[u8], offset: &mut usize) -> u16 {
    let val = u16::from_le_bytes([
        data[*offset],
        data[*offset + 1],
    ]);
    *offset += 2;
    val
}

/// Find the most appropriate territory name for the viewport center.
/// At wide zoom, returns the nearest country name.
/// At close zoom, returns the nearest province/state name if available.
#[allow(dead_code)]
pub fn find_territory_name(
    labels: &[TerritoryLabel],
    center_lat: f64,
    center_lon: f64,
    lat_span: f64,
    lon_span: f64,
) -> Option<String> {
    if labels.is_empty() {
        return None;
    }

    let span = lat_span.max(lon_span);
    // Decide which level to show based on zoom
    let target_level: u8 = if span < 15.0 { 1 } else { 0 };

    // First try the target level, fall back to country level
    for level in [target_level, 0] {
        let mut best: Option<(&TerritoryLabel, f64)> = None;
        for label in labels {
            if label.level != level {
                continue;
            }
            // Only consider labels whose centroid is within the viewport
            if label.lat < center_lat - lat_span / 2.0
                || label.lat > center_lat + lat_span / 2.0
                || label.lon < center_lon - lon_span / 2.0
                || label.lon > center_lon + lon_span / 2.0
            {
                continue;
            }
            let dlat = label.lat - center_lat;
            let dlon = label.lon - center_lon;
            let dist = dlat * dlat + dlon * dlon;
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((label, dist));
            }
        }
        if let Some((label, _)) = best {
            return Some(label.name.clone());
        }
    }
    None
}

/// Determine which detail levels to show based on the current viewport span.
pub fn visible_levels(lat_span: f64, lon_span: f64) -> u8 {
    let span = lat_span.max(lon_span);
    if span > 80.0 {
        0 // world view: coastlines only
    } else if span > 30.0 {
        1 // continent view: + country borders
    } else if span > 8.0 {
        2 // country/region view: + state/province borders
    } else {
        3 // close view: + admin-2 / county borders
    }
}
