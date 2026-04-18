#!/usr/bin/env python3
"""
Download NaturalEarth geographic data and convert to a compact binary format
for embedding in the geotrace binary.

Binary format:
  - u32 LE: number of polylines
  For each polyline:
    - u8: detail level (0=coastline, 1=country border, 2=state/province)
    - u32 LE: number of points
    - For each point: i32 LE lat * 10000, i32 LE lon * 10000
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


def main():
    all_polylines = []  # list of (detail_level, [(lat, lon), ...])

    names_by_level = {
        0: "ne_110m_coastline",
        1: "ne_110m_admin_0_boundary_lines_land",
        2: "ne_10m_admin_1_states_provinces_lines",
    }

    tolerances = {
        0: 0.0,    # coastlines already simplified at 110m
        1: 0.0,    # country borders already simplified
        2: 0.05,   # light simplification for 10m province borders
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

    # Write binary format
    with open(OUT_FILE, "wb") as f:
        f.write(struct.pack("<I", len(all_polylines)))
        for level, points in all_polylines:
            f.write(struct.pack("<B", level))
            f.write(struct.pack("<I", len(points)))
            for lat, lon in points:
                f.write(struct.pack("<ii", int(lat * 10000), int(lon * 10000)))

    file_size = os.path.getsize(OUT_FILE)
    print(f"Written {OUT_FILE} ({file_size} bytes, {file_size/1024:.1f} KB)")


if __name__ == "__main__":
    main()
