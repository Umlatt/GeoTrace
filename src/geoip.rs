use maxminddb::Reader;
use std::net::IpAddr;
use std::sync::Mutex;
use std::collections::HashMap;

pub struct GeoIpInfo {
    pub lat: f64,
    pub lon: f64,
    pub city: Option<String>,
    pub country: Option<String>,
    pub org: Option<String>,
}

/// Haversine great-circle distance between two lat/lon points, in km.
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // Earth radius in km
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

pub struct GeoIpLookup {
    reader: Reader<&'static [u8]>,
    cache: Mutex<HashMap<IpAddr, Option<GeoIpInfo>>>,
}

impl GeoIpLookup {
    pub fn from_bytes(data: &'static [u8]) -> Result<Self, maxminddb::MaxMindDBError> {
        let reader = Reader::from_source(data)?;
        Ok(Self {
            reader,
            cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<GeoIpInfo> {
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&ip) {
                return cached.as_ref().map(|c| GeoIpInfo {
                    lat: c.lat,
                    lon: c.lon,
                    city: c.city.clone(),
                    country: c.country.clone(),
                    org: c.org.clone(),
                });
            }
        }

        let result: Option<GeoIpInfo> = self.do_lookup(ip);

        let mut cache = self.cache.lock().unwrap();
        cache.insert(ip, result.as_ref().map(|r| GeoIpInfo {
            lat: r.lat,
            lon: r.lon,
            city: r.city.clone(),
            country: r.country.clone(),
            org: r.org.clone(),
        }));

        result
    }

    fn do_lookup(&self, ip: IpAddr) -> Option<GeoIpInfo> {
        let city: maxminddb::geoip2::City = self.reader.lookup(ip).ok()?;

        let location = city.location.as_ref()?;
        let lat = location.latitude?;
        let lon = location.longitude?;

        let city_name = city
            .city
            .as_ref()
            .and_then(|c| c.names.as_ref())
            .and_then(|n| n.get("en"))
            .map(|s| s.to_string());

        let country_name = city
            .country
            .as_ref()
            .and_then(|c| c.names.as_ref())
            .and_then(|n| n.get("en"))
            .map(|s| s.to_string());

        let org = None;

        Some(GeoIpInfo {
            lat,
            lon,
            city: city_name,
            country: country_name,
            org,
        })
    }
}
