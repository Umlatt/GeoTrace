#!/usr/bin/env python3
"""
Download NaturalEarth geographic data and convert to a compact binary format
for embedding in the geotrace binary.

Binary format:
  - u32 LE: number of polylines
  For each polyline:
    - u8: detail level (0=coastline, 1=country border, 2=state/province)
    - u32 LE: number of points
    - For each point: i32 LE lat * 1000000, i32 LE lon * 1000000
"""
import json
import struct
import sys
import os
import zipfile
import io
import requests

DATA_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_FILE = os.path.join(DATA_DIR, "data", "geodata.bin")

SOURCES = [
    # (url, detail_level, is_polygon)
    (
        "https://naciscdn.org/naturalearth/110m/physical/ne_110m_coastline.zip",
        0,  # coastline
        False,  # LineString
    ),
    (
        "https://naciscdn.org/naturalearth/110m/cultural/ne_110m_admin_0_boundary_lines_land.zip",
        1,  # country borders
        False,
    ),
    (
        "https://naciscdn.org/naturalearth/10m/cultural/ne_10m_admin_1_states_provinces_lines.zip",
        2,  # state/province borders
        False,
    ),
]


def download_and_extract_geojson(url):
    """Download a zip file and extract the shapefile, convert to GeoJSON-like structure."""
    print(f"  Downloading {url}...")
    resp = requests.get(url, timeout=60)
    resp.raise_for_status()

    zf = zipfile.ZipFile(io.BytesIO(resp.content))
    # Look for .shp file
    shp_name = None
    for name in zf.namelist():
        if name.endswith('.shp'):
            shp_name = name
            break

    if shp_name is None:
        # Try to find GeoJSON
        for name in zf.namelist():
            if name.endswith('.geojson') or name.endswith('.json'):
                data = json.loads(zf.read(name))
                return data

    # We need to parse shapefile - let's try with the json/geojson if available
    # Actually, NaturalEarth zips contain shapefiles. Let's try a different approach.
    # Try downloading GeoJSON directly from an alternative source.
    return None


def download_geojson_direct(url_base, name):
    """Try to get GeoJSON from alternative sources."""
    # Try the NaturalEarth GitHub raw GeoJSON
    github_url = f"https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/{name}.geojson"
    print(f"  Trying GitHub: {github_url}")
    try:
        resp = requests.get(github_url, timeout=60)
        if resp.status_code == 200:
            return resp.json()
    except Exception as e:
        print(f"  GitHub failed: {e}")

    return None


def extract_coords_from_geojson(geojson, is_polygon):
    """Extract all coordinate sequences from a GeoJSON FeatureCollection."""
    polylines = []
    features = geojson.get("features", [])
    if not features:
        # Maybe it's a geometry collection
        if geojson.get("type") == "GeometryCollection":
            features = [{"geometry": g} for g in geojson.get("geometries", [])]

    for feature in features:
        geom = feature.get("geometry", feature)
        if geom is None:
            continue
        gtype = geom.get("type", "")
        coords = geom.get("coordinates", [])

        if gtype == "LineString":
            polylines.append(coords)
        elif gtype == "MultiLineString":
            for line in coords:
                polylines.append(line)
        elif gtype == "Polygon":
            for ring in coords:
                polylines.append(ring)
        elif gtype == "MultiPolygon":
            for polygon in coords:
                for ring in polygon:
                    polylines.append(ring)

    return polylines


def simplify_polyline(points, tolerance):
    """Douglas-Peucker simplification."""
    if len(points) <= 2:
        return points

    # Find the point with maximum distance from the line start->end
    start = points[0]
    end = points[-1]
    max_dist = 0
    max_idx = 0

    for i in range(1, len(points) - 1):
        dist = point_line_distance(points[i], start, end)
        if dist > max_dist:
            max_dist = dist
            max_idx = i

    if max_dist > tolerance:
        left = simplify_polyline(points[:max_idx + 1], tolerance)
        right = simplify_polyline(points[max_idx:], tolerance)
        return left[:-1] + right
    else:
        return [start, end]


def point_line_distance(point, start, end):
    """Perpendicular distance from point to line segment."""
    dx = end[0] - start[0]
    dy = end[1] - start[1]
    if dx == 0 and dy == 0:
        return ((point[0] - start[0]) ** 2 + (point[1] - start[1]) ** 2) ** 0.5
    t = max(0, min(1, ((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / (dx * dx + dy * dy)))
    proj_x = start[0] + t * dx
    proj_y = start[1] + t * dy
    return ((point[0] - proj_x) ** 2 + (point[1] - proj_y) ** 2) ** 0.5


def compute_centroid(coords_list):
    """Compute centroid from a list of polygon/multipolygon coordinates."""
    total_lat = 0
    total_lon = 0
    count = 0
    for ring in coords_list:
        if isinstance(ring[0], (int, float)):
            # Single point [lon, lat]
            total_lon += ring[0]
            total_lat += ring[1]
            count += 1
        elif isinstance(ring[0], list) and isinstance(ring[0][0], (int, float)):
            # Ring of points [[lon, lat], ...]
            for p in ring:
                total_lon += p[0]
                total_lat += p[1]
                count += 1
        else:
            # Nested further (MultiPolygon)
            for sub in ring:
                if isinstance(sub[0], (int, float)):
                    total_lon += sub[0]
                    total_lat += sub[1]
                    count += 1
                else:
                    for p in sub:
                        if isinstance(p, list) and len(p) >= 2:
                            total_lon += p[0]
                            total_lat += p[1]
                            count += 1
    if count == 0:
        return None
    return (total_lat / count, total_lon / count)


def extract_labels(geojson, level, name_keys):
    """Extract territory labels (name + centroid) from a GeoJSON FeatureCollection."""
    labels = []
    features = geojson.get("features", [])
    for feature in features:
        props = feature.get("properties", {})

        # Try each name key in order
        name = None
        for key in name_keys:
            name = props.get(key)
            if name:
                break
        if not name:
            continue

        geom = feature.get("geometry")
        if geom is None:
            continue

        # Use explicit label point if available
        label_lat = props.get("label_y") or props.get("latitude")
        label_lon = props.get("label_x") or props.get("longitude")

        if label_lat is not None and label_lon is not None:
            labels.append((level, float(label_lat), float(label_lon), name))
            continue

        # Otherwise compute centroid from geometry
        gtype = geom.get("type", "")
        coords = geom.get("coordinates", [])

        if gtype == "Point":
            labels.append((level, coords[1], coords[0], name))
        elif gtype in ("Polygon", "MultiPolygon", "LineString", "MultiLineString"):
            centroid = compute_centroid(coords)
            if centroid:
                labels.append((level, centroid[0], centroid[1], name))

    return labels


def main():
    all_polylines = []  # list of (detail_level, [(lat, lon), ...])

    names_by_level = {
        0: "ne_50m_coastline",
        1: "ne_50m_admin_0_boundary_lines_land",
        2: "ne_10m_admin_1_states_provinces_lines",
        3: "ne_10m_admin_2_counties_lines",
    }

    tolerances = {
        0: 0.02,   # light simplification for 50m coastlines
        1: 0.02,   # light simplification for 50m country borders
        2: 0.05,   # light simplification for 10m province borders
        3: 0.08,   # moderate simplification for 10m admin-2 borders
    }

    for level, name in names_by_level.items():
        print(f"Processing level {level}: {name}")
        geojson = download_geojson_direct(None, name)
        if geojson is None:
            print(f"  FAILED to download {name}, skipping")
            continue

        polylines = extract_coords_from_geojson(geojson, False)
        print(f"  Got {len(polylines)} polylines")

        total_points = 0
        for pl in polylines:
            # coords are [lon, lat] in GeoJSON, convert to (lat, lon)
            points = [(p[1], p[0]) for p in pl]

            tol = tolerances[level]
            if tol > 0:
                points = simplify_polyline(points, tol)

            if len(points) >= 2:
                all_polylines.append((level, points))
                total_points += len(points)

        print(f"  Total points for level {level}: {total_points}")

    print(f"\nTotal polylines: {len(all_polylines)}")
    total_pts = sum(len(pts) for _, pts in all_polylines)
    print(f"Total points: {total_pts}")

    # --- Download territory labels ---
    all_labels = []  # list of (level, lat, lon, name)

    label_sources = {
        # (geojson_name, label_level, [name_keys])
        0: ("ne_110m_admin_0_countries", 0, ["NAME", "ADMIN", "name"]),
        1: ("ne_10m_admin_1_states_provinces", 1, ["name", "NAME", "name_en", "gn_name"]),
    }

    for _, (name, level, name_keys) in sorted(label_sources.items()):
        print(f"\nProcessing labels level {level}: {name}")
        geojson = download_geojson_direct(None, name)
        if geojson is None:
            print(f"  FAILED to download {name}, skipping labels")
            continue
        labels = extract_labels(geojson, level, name_keys)
        all_labels.extend(labels)
        print(f"  Got {len(labels)} labels")

    print(f"\nTotal labels: {len(all_labels)}")

    # Write binary format
    with open(OUT_FILE, "wb") as f:
        # --- Polylines section ---
        f.write(struct.pack("<I", len(all_polylines)))
        for level, points in all_polylines:
            f.write(struct.pack("<B", level))
            f.write(struct.pack("<I", len(points)))
            for lat, lon in points:
                f.write(struct.pack("<ii", round(lat * 1000000), round(lon * 1000000)))

        # --- Labels section ---
        f.write(struct.pack("<I", len(all_labels)))
        for level, lat, lon, name in all_labels:
            name_bytes = name.encode("utf-8")
            f.write(struct.pack("<B", level))
            f.write(struct.pack("<ii", round(lat * 1000000), round(lon * 1000000)))
            f.write(struct.pack("<H", len(name_bytes)))
            f.write(name_bytes)

    file_size = os.path.getsize(OUT_FILE)
    print(f"Written {OUT_FILE} ({file_size} bytes, {file_size/1024:.1f} KB)")


if __name__ == "__main__":
    main()
