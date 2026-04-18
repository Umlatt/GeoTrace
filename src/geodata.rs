/// Geographic border data for rendering on the Braille map.
///
/// Binary format:
///   u32 LE: number of polylines
///   Per polyline:
///     u8:  detail level (0=coastline, 1=country, 2=state/province)
///     u32 LE: number of points
///     Per point: i32 LE lat*10000, i32 LE lon*10000

static GEODATA: &[u8] = include_bytes!("../data/geodata.bin");

#[derive(Debug, Clone)]
pub struct Polyline {
    pub level: u8,
    pub points: Vec<(f64, f64)>, // (lat, lon)
}

/// Parse all polylines from the embedded binary data.
pub fn load_polylines() -> Vec<Polyline> {
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
            points.push((lat_raw as f64 / 10000.0, lon_raw as f64 / 10000.0));
        }
        polylines.push(Polyline { level, points });
    }

    polylines
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

/// Determine which detail levels to show based on the current viewport span.
pub fn visible_levels(lat_span: f64, lon_span: f64) -> u8 {
    let span = lat_span.max(lon_span);
    if span > 80.0 {
        0 // world view: coastlines only
    } else if span > 20.0 {
        1 // continent view: + country borders
    } else {
        2 // country/region view: + state/province borders
    }
}
