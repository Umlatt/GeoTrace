use crate::geoip::GeoIpLookup;
use dns_lookup::lookup_addr;
use pnet::packet::icmp::echo_reply::EchoReplyPacket;
use pnet::packet::icmp::echo_request::MutableEchoRequestPacket;
use pnet::packet::icmp::time_exceeded::TimeExceededPacket;
use pnet::packet::icmp::{IcmpCode, IcmpPacket, IcmpTypes};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::Packet;
use pnet::transport::{
    icmp_packet_iter, transport_channel, TransportChannelType, TransportProtocol,
    TransportReceiver, TransportSender,
};
use pnet::util::checksum;
use std::collections::HashMap;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct HopUpdate {
    pub hop: u8,
    pub ip: Option<IpAddr>,
    pub hostname: Option<String>,
    pub rtt_ms: Option<f64>,
    pub loss_pct: Option<f64>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub org: Option<String>,
    pub timeout: bool,
    pub trace_complete: bool,
    pub error: Option<String>,
    pub location_estimated: bool,
}

pub async fn run_traceroute(
    target: String,
    max_hops: u8,
    tx: mpsc::Sender<HopUpdate>,
    geo: Arc<GeoIpLookup>,
) {
    let dest_ip = match resolve_host(&target) {
        Some(ip) => ip,
        None => {
            let _ = tx
                .send(HopUpdate {
                    hop: 0, ip: None, hostname: None, rtt_ms: None,
                    loss_pct: None, lat: None, lon: None, city: None,
                    country: None, org: None, timeout: false, trace_complete: false,
                    error: Some(format!("Failed to resolve host: {}", target)),
                    location_estimated: false,
                })
                .await;
            return;
        }
    };

    // Verify raw socket access and open a single shared socket for the session
    let protocol = TransportChannelType::Layer4(TransportProtocol::Ipv4(
        IpNextHeaderProtocols::Icmp,
    ));
    let (sender, receiver) = match transport_channel(4096, protocol) {
        Ok((s, r)) => (s, r),
        Err(e) => {
            let msg = format!(
                "Cannot open raw socket: {}. Run with: sudo <binary>",
                e
            );
            let _ = tx
                .send(HopUpdate {
                    hop: 0, ip: None, hostname: None, rtt_ms: None,
                    loss_pct: None, lat: None, lon: None, city: None,
                    country: None, org: None, timeout: false, trace_complete: false,
                    error: Some(msg),
                    location_estimated: false,
                })
                .await;
            return;
        }
    };

    // Unique session identifier derived from PID + timestamp to avoid collisions
    let session_id = {
        let pid = std::process::id() as u16;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u16;
        pid ^ ts
    };

    let sender = Arc::new(std::sync::Mutex::new(sender));
    let receiver = Arc::new(std::sync::Mutex::new(receiver));

    // Track per-hop stats
    let mut sent_counts: HashMap<u8, u32> = HashMap::new();
    let mut lost_counts: HashMap<u8, u32> = HashMap::new();
    let mut first_pass = true;

    // RTT-based location validation state
    // anchor = first hop with a GeoIP location (our ISP, closest to us)
    let mut anchor: Option<(f64, f64, f64)> = None; // (lat, lon, rtt_ms)

    // Max plausible distance per ms of round-trip time.
    // Speed of light in fiber ≈ 200,000 km/s → ~100 km/ms one-way → ~100 km/ms RTT.
    // Real-world routing adds ~40-50% overhead, so we use 67 km/ms as the limit.
    // Multiply by 1.5 safety factor = ~100 km/ms to avoid false positives.
    const MAX_KM_PER_MS_RTT: f64 = 100.0;

    // Continuous probing loop like mtr
    loop {
        for ttl in 1..=max_hops {
            let s = sender.clone();
            let r = receiver.clone();
            let sid = session_id;
            let result = probe_hop(dest_ip, ttl, s, r, sid).await;

            let sent = sent_counts.entry(ttl).or_insert(0);
            *sent += 1;
            let total_sent = *sent;

            let mut update = HopUpdate {
                hop: ttl,
                ip: None,
                hostname: None,
                rtt_ms: None,
                loss_pct: None,
                lat: None,
                lon: None,
                city: None,
                country: None,
                org: None,
                timeout: false,
                trace_complete: false,
                error: None,
                location_estimated: false,
            };

            match result {
                ProbeResult::Reply { addr, rtt } => {
                    update.ip = Some(addr);
                    update.rtt_ms = Some(rtt);

                    // Async DNS lookup
                    let hostname = tokio::task::spawn_blocking(move || {
                        lookup_addr(&addr).ok()
                    })
                    .await
                    .ok()
                    .flatten();
                    update.hostname = hostname;

                    // GeoIP lookup + RTT-based plausibility check
                    if let Some(info) = geo.lookup(addr) {
                        let mut estimated = false;

                        if let Some((a_lat, a_lon, a_rtt)) = anchor {
                            // Max distance from anchor given RTT difference
                            let rtt_delta = (rtt - a_rtt).max(0.0);
                            let max_dist = rtt_delta * MAX_KM_PER_MS_RTT;
                            let geoip_dist = crate::geoip::haversine_km(
                                a_lat, a_lon, info.lat, info.lon,
                            );

                            if geoip_dist > max_dist && max_dist < 15000.0 {
                                estimated = true;
                            }
                        }

                        // Set/update anchor from first valid GeoIP hop
                        if anchor.is_none() {
                            anchor = Some((info.lat, info.lon, rtt));
                        }

                        // Always use the GeoIP location; the purple marker
                        // on the map indicates the latency doesn't match.
                        update.lat = Some(info.lat);
                        update.lon = Some(info.lon);
                        update.location_estimated = estimated;
                        if !estimated {
                            update.city = info.city;
                        } else {
                            update.city = info.city.map(|c| format!("~{}", c));
                        }
                        update.org = info.org;
                        update.country = info.country;
                    }

                    let lost = *lost_counts.entry(ttl).or_insert(0);
                    update.loss_pct = Some((lost as f64 / total_sent as f64) * 100.0);
                }
                ProbeResult::Timeout => {
                    update.timeout = true;
                    let lost = lost_counts.entry(ttl).or_insert(0);
                    *lost += 1;
                    update.loss_pct =
                        Some((*lost as f64 / total_sent as f64) * 100.0);
                }
            }

            let reached_dest = update.ip == Some(dest_ip);
            if tx.send(update).await.is_err() {
                return; // UI closed
            }
            if reached_dest {
                break;
            }
        }

        // Signal trace complete after first pass
        if first_pass {
            first_pass = false;
            let _ = tx
                .send(HopUpdate {
                    hop: 0,
                    ip: None,
                    hostname: None,
                    rtt_ms: None,
                    loss_pct: None,
                    lat: None,
                    lon: None,
                    city: None,
                    country: None,
                    org: None,
                    timeout: false,
                    trace_complete: true,
                    error: None,
                    location_estimated: false,
                })
                .await;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

enum ProbeResult {
    Reply { addr: IpAddr, rtt: f64 },
    Timeout,
}

async fn probe_hop(
    dest: IpAddr,
    ttl: u8,
    sender: Arc<std::sync::Mutex<TransportSender>>,
    receiver: Arc<std::sync::Mutex<TransportReceiver>>,
    session_id: u16,
) -> ProbeResult {
    tokio::task::spawn_blocking(move || {
        probe_hop_sync(dest, ttl, &sender, &receiver, session_id)
    })
    .await
    .unwrap_or(ProbeResult::Timeout)
}

fn probe_hop_sync(
    dest: IpAddr,
    ttl: u8,
    sender: &std::sync::Mutex<TransportSender>,
    receiver: &std::sync::Mutex<TransportReceiver>,
    session_id: u16,
) -> ProbeResult {
    // Build ICMP echo request with session-unique identifier
    let mut buf = vec![0u8; 64];
    let mut packet = match MutableEchoRequestPacket::new(&mut buf) {
        Some(p) => p,
        None => return ProbeResult::Timeout,
    };

    // Use session_id as the ICMP identifier and ttl as the sequence number
    // This uniquely identifies our probes across concurrent sessions
    packet.set_icmp_type(IcmpTypes::EchoRequest);
    packet.set_icmp_code(IcmpCode::new(0));
    packet.set_identifier(session_id);
    packet.set_sequence_number(ttl as u16);
    packet.set_checksum(0);
    let cksum = checksum(packet.packet(), 1);
    packet.set_checksum(cksum);

    let start = Instant::now();

    {
        let mut tx = sender.lock().unwrap();
        tx.set_ttl(ttl).ok();
        if tx.send_to(packet, dest).is_err() {
            return ProbeResult::Timeout;
        }
    }

    let deadline = Duration::from_secs(2);

    let mut rx = receiver.lock().unwrap();
    let mut iter = icmp_packet_iter(&mut *rx);

    loop {
        let elapsed = start.elapsed();
        if elapsed >= deadline {
            return ProbeResult::Timeout;
        }
        let remaining = deadline - elapsed;

        match iter.next_with_timeout(remaining) {
            Ok(Some((icmp_pkt, addr))) => {
                // Check if this reply belongs to our session
                if is_our_reply(&icmp_pkt, session_id, ttl) {
                    let rtt = start.elapsed().as_secs_f64() * 1000.0;
                    return ProbeResult::Reply { addr, rtt };
                }
                // Not our packet — keep waiting
                continue;
            }
            Ok(None) => return ProbeResult::Timeout,
            Err(_) => return ProbeResult::Timeout,
        }
    }
}

/// Check if an ICMP reply matches our session_id.
/// Handles both:
///   - Echo Reply (type 0): identifier is directly in the reply
///   - Time Exceeded (type 11): the original echo request is embedded in the payload
fn is_our_reply(icmp_pkt: &IcmpPacket, session_id: u16, seq: u8) -> bool {
    match icmp_pkt.get_icmp_type() {
        IcmpTypes::EchoReply => {
            if let Some(reply) = EchoReplyPacket::new(icmp_pkt.packet()) {
                return reply.get_identifier() == session_id
                    && reply.get_sequence_number() == seq as u16;
            }
            false
        }
        IcmpTypes::TimeExceeded => {
            if let Some(te) = TimeExceededPacket::new(icmp_pkt.packet()) {
                // The payload of TimeExceeded contains the original IP header + 8 bytes
                // Parse the embedded IPv4 packet to get at the original ICMP header
                let payload = te.payload();
                if let Some(inner_ip) = Ipv4Packet::new(payload) {
                    let ihl = (inner_ip.get_header_length() as usize) * 4;
                    if payload.len() >= ihl + 8 {
                        // Bytes at ihl+4..ihl+6 are the ICMP identifier
                        // Bytes at ihl+6..ihl+8 are the ICMP sequence number
                        let id = u16::from_be_bytes([payload[ihl + 4], payload[ihl + 5]]);
                        let sn = u16::from_be_bytes([payload[ihl + 6], payload[ihl + 7]]);
                        return id == session_id && sn == seq as u16;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn resolve_host(host: &str) -> Option<IpAddr> {
    // Try parsing as IP first
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    // DNS resolution
    let addrs = format!("{}:0", host).to_socket_addrs().ok()?;
    for addr in addrs {
        if addr.ip().is_ipv4() {
            return Some(addr.ip());
        }
    }
    None
}
