use maxminddb::{geoip2, Reader};
use std::{net::IpAddr, path::Path};
use tracing::debug;

use crate::GeoLocation;

pub struct GeoDatabase {
    reader: Reader<Vec<u8>>,
}

impl GeoDatabase {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let reader = Reader::open_readfile(path)?;
        Ok(Self { reader })
    }

    pub async fn lookup(&self, ip_str: &str) -> Result<GeoLocation, Box<dyn std::error::Error + Send + Sync>> {
        let ip: IpAddr = ip_str.parse()
            .map_err(|_| format!("Invalid IP address: {}", ip_str))?;

        debug!("Looking up IP: {}", ip);

        let city_record: geoip2::City = self.reader.lookup(ip)
            .map_err(|e| format!("Database lookup failed: {}", e))?
            .ok_or("IP address not found in database")?;

        debug!("Raw city record for {}: {:#?}", ip, city_record);

        let location = GeoLocation {
            ip: ip_str.to_string(),
            city: city_record.city.as_ref()
                .and_then(|city| city.names.as_ref())
                .and_then(|names| names.get("en"))
                .map(|name| name.to_string()),
            subdivision: city_record.subdivisions.as_ref()
                .and_then(|subdivisions| subdivisions.first())
                .and_then(|subdivision| subdivision.names.as_ref())
                .and_then(|names| names.get("en"))
                .map(|name| name.to_string()),
            country: city_record.country.as_ref()
                .and_then(|country| country.names.as_ref())
                .and_then(|names| names.get("en"))
                .map(|name| name.to_string()),
            country_code: city_record.country.as_ref()
                .and_then(|country| country.iso_code)
                .map(|code| code.to_string()),
            continent: city_record.continent.as_ref()
                .and_then(|continent| continent.names.as_ref())
                .and_then(|names| names.get("en"))
                .map(|name| name.to_string()),
            continent_code: city_record.continent.as_ref()
                .and_then(|continent| continent.code)
                .map(|code| code.to_string()),
            latitude: city_record.location.as_ref()
                .and_then(|loc| loc.latitude),
            longitude: city_record.location.as_ref()
                .and_then(|loc| loc.longitude),
            timezone: city_record.location.as_ref()
                .and_then(|loc| loc.time_zone)
                .map(|tz| tz.to_string()),
            accuracy_radius: city_record.location.as_ref()
                .and_then(|loc| loc.accuracy_radius),
        };

        debug!("Lookup result: city={:?}, country={:?}", location.city, location.country);
        Ok(location)
    }
}