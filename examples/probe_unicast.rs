//! Diagnostic: probe a list of IPs via unicast WS-Discovery.
//!
//! Useful for confirming whether a known device is reachable from this host
//! when multicast WS-Discovery cannot find it. If the device responds here
//! but not to `probe()`, the gap is a multicast routing problem; if it
//! doesn't respond to either, the device is on an unreachable network or
//! its discovery stack is silent.
//!
//! Usage:
//!     cargo run --example probe_unicast -- 192.168.0.123 192.168.4.142
//!
//! Per-IP timeout is 2 s. Output is one line per IP:
//!     192.168.0.123  ←  REACHED  endpoint=uuid:... xaddrs=[http://...]
//!     192.168.4.142  ←  NO REPLY

use std::time::Duration;

#[tokio::main]
async fn main() {
    let ips: Vec<String> = std::env::args().skip(1).collect();
    if ips.is_empty() {
        eprintln!("usage: cargo run --example probe_unicast -- <ip> [<ip>...]");
        std::process::exit(2);
    }

    println!(
        "probing {} target{} via unicast WS-Discovery (timeout 2s each)\n",
        ips.len(),
        if ips.len() == 1 { "" } else { "s" }
    );

    for ip_str in &ips {
        let Ok(ip) = ip_str.parse::<std::net::IpAddr>() else {
            println!("{ip_str:<16}  ←  INVALID IP");
            continue;
        };
        let devices = oxvif::discovery::probe_unicast(ip, Duration::from_secs(2)).await;
        if devices.is_empty() {
            println!("{ip_str:<16}  ←  NO REPLY");
        } else {
            for d in &devices {
                println!(
                    "{ip_str:<16}  ←  REACHED  endpoint={}  xaddrs={:?}",
                    d.endpoint, d.xaddrs
                );
            }
        }
    }
}
