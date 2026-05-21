//! 외부 입력 검증기 (S08~). 데이터 계층이 받아들이는 값에 대한 정책을
//! 호출자(api/indexer)와 공유하기 위해 db 크레이트에 둔다 — DB 모듈 자체와
//! 결합하지 않고 순수.

use std::net::IpAddr;
use std::str::FromStr;

/// [`webhook_url_is_safe`]의 거부 사유 (순수 — 진단/테스트용).
#[derive(Debug, PartialEq, Eq)]
pub enum UnsafeUrlReason {
    /// URL 파싱 실패
    InvalidUrl,
    /// 스킴이 `https` 가 아님
    NonHttps,
    /// 호스트 부분 부재 또는 빈 문자열
    NoHost,
    /// 127.0.0.0/8 또는 `::1`
    LoopbackHost,
    /// RFC1918 사설망(10/8, 172.16/12, 192.168/16)
    PrivateIp,
    /// 169.254/16 (메타데이터 169.254.169.254 포함) 또는 fe80::/10
    LinkLocalIp,
    /// 0.0.0.0 또는 `::`
    UnspecifiedIp,
    /// 멀티캐스트 (224.0.0.0/4 등)
    MulticastIp,
    /// 255.255.255.255 등 브로드캐스트
    BroadcastIp,
    /// IPv6 unique-local (fc00::/7)
    UniqueLocalIp,
    /// `localhost` / `*.localhost` / `*.local` (mDNS)
    MdnsHost,
}

/// 단일 IP 주소가 외부 webhook 대상으로 안전한지 **순수** 검증 (S08 + HARDEN3/D020).
///
/// `webhook_url_is_safe`(URL 파싱 시점)와 `SafeDnsResolver`(DNS resolve 시점)가
/// **공유** — 정책 단일 출처. IPv4-mapped IPv6는 IPv4 규칙으로 펴낸 뒤 검증
/// (리뷰 H1). 거부 분류: loopback / private (RFC1918) / link-local (169.254
/// 메타데이터 포함) / unspecified / multicast / broadcast / IPv6 ULA (fc00::/7)
/// / IPv6 link-local (fe80::/10).
pub fn ip_is_safe(ip: IpAddr) -> Result<(), UnsafeUrlReason> {
    use UnsafeUrlReason::*;
    let v4_to_check: Option<std::net::Ipv4Addr> = match ip {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(v6) => v6.to_ipv4_mapped(),
    };
    if let Some(v4) = v4_to_check {
        if v4.is_loopback() {
            return Err(LoopbackHost);
        }
        if v4.is_unspecified() {
            return Err(UnspecifiedIp);
        }
        if v4.is_multicast() {
            return Err(MulticastIp);
        }
        if v4.is_private() {
            return Err(PrivateIp);
        }
        if v4.is_link_local() {
            return Err(LinkLocalIp);
        }
        if v4.is_broadcast() {
            return Err(BroadcastIp);
        }
        return Ok(());
    }
    // 순수 IPv6 (mapped 아닌)
    if ip.is_loopback() {
        return Err(LoopbackHost);
    }
    if ip.is_unspecified() {
        return Err(UnspecifiedIp);
    }
    if ip.is_multicast() {
        return Err(MulticastIp);
    }
    if let IpAddr::V6(v6) = ip {
        let seg0 = v6.segments()[0];
        if (seg0 & 0xfe00) == 0xfc00 {
            return Err(UniqueLocalIp);
        }
        if (seg0 & 0xffc0) == 0xfe80 {
            return Err(LinkLocalIp);
        }
    }
    Ok(())
}

/// webhook URL이 외부로 안전하게 POST할 수 있는 형태인지 **순수** 검증 (S08).
///
/// API의 입력 검증과 디스패처의 전송 직전 검증에서 **공유**된다(같은 규칙으로
/// 박혀야 분기 회피 없음). 정책: ① `https` 스킴만 허용. ② IP 리터럴이면
/// [`ip_is_safe`] 규칙 적용. ③ 호스트명이면 `localhost`/`*.localhost`/`*.local`
/// (mDNS) 거부.
///
/// **DNS 시점 IP 재바인딩**(공격자가 처음엔 공개 IP를 응답 후 connect 직전
/// 사설 IP로 rebind)은 본 함수가 못 막지만, dispatcher의 `SafeDnsResolver`
/// (HARDEN3/D020)가 resolve 시점에 `ip_is_safe`를 다시 적용해 차단한다.
/// 디스패처는 `reqwest::Policy::none()`로 리다이렉트 비추적해 한 단계 우회 차단.
pub fn webhook_url_is_safe(input: &str) -> Result<(), UnsafeUrlReason> {
    use UnsafeUrlReason::*;
    let parsed = url::Url::parse(input).map_err(|_| InvalidUrl)?;
    if parsed.scheme() != "https" {
        return Err(NonHttps);
    }
    let host_raw = parsed.host_str().ok_or(NoHost)?;
    if host_raw.is_empty() {
        return Err(NoHost);
    }
    // IPv6 리터럴은 `host_str`이 대괄호 포함("[::1]")으로 반환될 수 있음 →
    // IpAddr 파싱 가능한 형태로 정규화한다.
    let host = host_raw
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host_raw);
    if let Ok(ip) = IpAddr::from_str(host) {
        return ip_is_safe(ip);
    }
    let host_lower = host.to_ascii_lowercase();
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
    {
        return Err(MdnsHost);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_only() {
        assert!(webhook_url_is_safe("https://example.com/hook").is_ok());
        assert_eq!(
            webhook_url_is_safe("http://example.com/hook"),
            Err(UnsafeUrlReason::NonHttps)
        );
        assert_eq!(
            webhook_url_is_safe("ftp://example.com"),
            Err(UnsafeUrlReason::NonHttps)
        );
        assert_eq!(
            webhook_url_is_safe("javascript:alert(1)"),
            Err(UnsafeUrlReason::NonHttps)
        );
    }

    #[test]
    fn rejects_localhost_variants() {
        assert_eq!(
            webhook_url_is_safe("https://localhost/x"),
            Err(UnsafeUrlReason::MdnsHost)
        );
        assert_eq!(
            webhook_url_is_safe("https://foo.localhost/x"),
            Err(UnsafeUrlReason::MdnsHost)
        );
        assert_eq!(
            webhook_url_is_safe("https://printer.local/x"),
            Err(UnsafeUrlReason::MdnsHost)
        );
    }

    #[test]
    fn rejects_loopback_ip() {
        assert_eq!(
            webhook_url_is_safe("https://127.0.0.1/x"),
            Err(UnsafeUrlReason::LoopbackHost)
        );
        assert_eq!(
            webhook_url_is_safe("https://[::1]/x"),
            Err(UnsafeUrlReason::LoopbackHost)
        );
    }

    #[test]
    fn rejects_private_ip() {
        assert_eq!(
            webhook_url_is_safe("https://10.0.0.1/x"),
            Err(UnsafeUrlReason::PrivateIp)
        );
        assert_eq!(
            webhook_url_is_safe("https://192.168.1.1/x"),
            Err(UnsafeUrlReason::PrivateIp)
        );
        assert_eq!(
            webhook_url_is_safe("https://172.16.0.1/x"),
            Err(UnsafeUrlReason::PrivateIp)
        );
    }

    #[test]
    fn rejects_link_local_and_cloud_metadata() {
        // AWS/GCP/Azure 메타데이터 IP — 169.254/16 (link-local) 으로 차단
        assert_eq!(
            webhook_url_is_safe("https://169.254.169.254/latest/meta-data/"),
            Err(UnsafeUrlReason::LinkLocalIp)
        );
        assert_eq!(
            webhook_url_is_safe("https://169.254.0.1/x"),
            Err(UnsafeUrlReason::LinkLocalIp)
        );
    }

    #[test]
    fn rejects_unspecified_and_multicast() {
        assert_eq!(
            webhook_url_is_safe("https://0.0.0.0/x"),
            Err(UnsafeUrlReason::UnspecifiedIp)
        );
        assert_eq!(
            webhook_url_is_safe("https://224.0.0.1/x"),
            Err(UnsafeUrlReason::MulticastIp)
        );
    }

    #[test]
    fn rejects_ipv6_ula_and_link_local() {
        assert_eq!(
            webhook_url_is_safe("https://[fd00::1]/x"),
            Err(UnsafeUrlReason::UniqueLocalIp)
        );
        assert_eq!(
            webhook_url_is_safe("https://[fe80::1]/x"),
            Err(UnsafeUrlReason::LinkLocalIp)
        );
    }

    // 리뷰 H1 회귀: IPv4-mapped IPv6 (`::ffff:a.b.c.d`) 우회 차단 — 매핑된
    // IPv4를 펴낸 뒤 IPv4 규칙을 적용해 loopback/private/link-local·메타데이터
    // 모두 잡는다.

    #[test]
    fn rejects_ipv4_mapped_loopback() {
        assert_eq!(
            webhook_url_is_safe("https://[::ffff:127.0.0.1]/x"),
            Err(UnsafeUrlReason::LoopbackHost)
        );
    }

    #[test]
    fn rejects_ipv4_mapped_private() {
        assert_eq!(
            webhook_url_is_safe("https://[::ffff:10.0.0.1]/x"),
            Err(UnsafeUrlReason::PrivateIp)
        );
        assert_eq!(
            webhook_url_is_safe("https://[::ffff:192.168.1.1]/x"),
            Err(UnsafeUrlReason::PrivateIp)
        );
    }

    #[test]
    fn rejects_ipv4_mapped_link_local_and_metadata() {
        assert_eq!(
            webhook_url_is_safe("https://[::ffff:169.254.169.254]/x"),
            Err(UnsafeUrlReason::LinkLocalIp)
        );
    }

    #[test]
    fn rejects_ipv4_mapped_unspecified() {
        assert_eq!(
            webhook_url_is_safe("https://[::ffff:0.0.0.0]/x"),
            Err(UnsafeUrlReason::UnspecifiedIp)
        );
    }

    #[test]
    fn allows_ipv4_mapped_public() {
        // 매핑된 공개 IP는 그대로 허용(IPv4 정책과 일관)
        assert!(webhook_url_is_safe("https://[::ffff:8.8.8.8]/x").is_ok());
    }

    #[test]
    fn rejects_invalid_url() {
        assert_eq!(
            webhook_url_is_safe("not a url"),
            Err(UnsafeUrlReason::InvalidUrl)
        );
        assert_eq!(webhook_url_is_safe(""), Err(UnsafeUrlReason::InvalidUrl));
    }

    #[test]
    fn allows_public_hosts_and_ips() {
        assert!(webhook_url_is_safe("https://example.com/hook").is_ok());
        assert!(webhook_url_is_safe("https://api.example.test/v1/alerts").is_ok());
        assert!(webhook_url_is_safe("https://8.8.8.8/x").is_ok());
        assert!(webhook_url_is_safe("https://[2001:db8::1]/x").is_ok());
    }

    // HARDEN3 / D020 — `ip_is_safe`는 `webhook_url_is_safe`와 dispatcher의
    // `SafeDnsResolver` 양쪽이 공유하는 정책 단일 출처. 직접 호출 테스트로
    // 두 진입점이 *같은 룰*을 보고 있음을 명시한다.

    #[test]
    fn ip_is_safe_accepts_public_ipv4_and_ipv6() {
        assert!(ip_is_safe(IpAddr::from_str("8.8.8.8").unwrap()).is_ok());
        assert!(ip_is_safe(IpAddr::from_str("1.1.1.1").unwrap()).is_ok());
        assert!(ip_is_safe(IpAddr::from_str("2001:db8::1").unwrap()).is_ok());
    }

    #[test]
    fn ip_is_safe_rejects_loopback_private_link_local() {
        assert_eq!(
            ip_is_safe(IpAddr::from_str("127.0.0.1").unwrap()),
            Err(UnsafeUrlReason::LoopbackHost)
        );
        assert_eq!(
            ip_is_safe(IpAddr::from_str("10.0.0.1").unwrap()),
            Err(UnsafeUrlReason::PrivateIp)
        );
        assert_eq!(
            ip_is_safe(IpAddr::from_str("169.254.169.254").unwrap()),
            Err(UnsafeUrlReason::LinkLocalIp)
        );
        assert_eq!(
            ip_is_safe(IpAddr::from_str("::1").unwrap()),
            Err(UnsafeUrlReason::LoopbackHost)
        );
        assert_eq!(
            ip_is_safe(IpAddr::from_str("fd00::1").unwrap()),
            Err(UnsafeUrlReason::UniqueLocalIp)
        );
    }

    #[test]
    fn ip_is_safe_unwraps_ipv4_mapped_ipv6() {
        // ::ffff:127.0.0.1 → IPv4 loopback (리뷰 H1 회귀, dispatcher의
        // SafeDnsResolver도 같은 정책)
        assert_eq!(
            ip_is_safe(IpAddr::from_str("::ffff:127.0.0.1").unwrap()),
            Err(UnsafeUrlReason::LoopbackHost)
        );
        assert_eq!(
            ip_is_safe(IpAddr::from_str("::ffff:169.254.169.254").unwrap()),
            Err(UnsafeUrlReason::LinkLocalIp)
        );
        // 매핑된 공개 IP는 그대로 허용
        assert!(ip_is_safe(IpAddr::from_str("::ffff:8.8.8.8").unwrap()).is_ok());
    }
}
