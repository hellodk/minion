use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

pub const MAX_REDIRECTS: usize = 3;

pub fn validate_url(raw: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(raw).map_err(|e| format!("Invalid URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        s => return Err(format!("Disallowed scheme '{s}': only http/https are permitted")),
    }
    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?;
    for socket_addr in addrs {
        let ip = socket_addr.ip();
        if is_private_or_loopback(ip) {
            return Err(format!(
                "Host '{host}' resolves to private/loopback address {ip} — blocked by SSRF guard"
            ));
        }
    }
    Ok(parsed)
}

fn is_private_or_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => is_private_v6(v6),
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || (octets[0] == 100 && (octets[1] & 0xC0) == 64)
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || (ip.segments()[0] & 0xFE00) == 0xFC00
        || (ip.segments()[0] & 0xFFC0) == 0xFE80
        || ip.is_unspecified()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn loopback_v4_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    }
    #[test]
    fn private_10_block_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))));
    }
    #[test]
    fn private_172_16_block_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
        assert!(!is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1))));
    }
    #[test]
    fn private_192_168_block_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))));
    }
    #[test]
    fn link_local_169_254_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
    }
    #[test]
    fn public_ip_is_not_private() {
        assert!(!is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }
    #[test]
    fn ipv6_loopback_is_private() {
        assert!(is_private_or_loopback(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }
    #[test]
    fn ipv6_unique_local_fc_is_private() {
        let ip: Ipv6Addr = "fc00::1".parse().unwrap();
        assert!(is_private_or_loopback(IpAddr::V6(ip)));
    }
    #[test]
    fn ipv6_link_local_fe80_is_private() {
        let ip: Ipv6Addr = "fe80::1".parse().unwrap();
        assert!(is_private_or_loopback(IpAddr::V6(ip)));
    }
}
