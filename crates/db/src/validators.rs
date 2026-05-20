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

/// webhook URL이 외부로 안전하게 POST할 수 있는 형태인지 **순수** 검증 (S08).
///
/// API의 입력 검증과 디스패처의 전송 직전 검증에서 **공유**된다(같은 규칙으로
/// 박혀야 분기 회피 없음). 정책: ① `https` 스킴만 허용. ② IP 리터럴이면
/// loopback/private(RFC1918)/link-local(169.254 메타데이터 포함)/unspecified/
/// multicast/broadcast/IPv6 ULA·link-local 거부. ③ 호스트명이면 `localhost`/
/// `*.localhost`/`*.local`(mDNS) 거부.
///
/// **잔여 리스크(정직)**: DNS 시점 IP 재바인딩(공격자가 사설 IP로 해석되는 도메인
/// 등록)은 본 가드만으론 못 막는다 — 완전 해소는 연결 시점 IP 검사(backlog).
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
        if ip.is_loopback() {
            return Err(LoopbackHost);
        }
        if ip.is_unspecified() {
            return Err(UnspecifiedIp);
        }
        if ip.is_multicast() {
            return Err(MulticastIp);
        }
        match ip {
            IpAddr::V4(v4) => {
                if v4.is_private() {
                    return Err(PrivateIp);
                }
                if v4.is_link_local() {
                    return Err(LinkLocalIp);
                }
                if v4.is_broadcast() {
                    return Err(BroadcastIp);
                }
            }
            IpAddr::V6(v6) => {
                let seg0 = v6.segments()[0];
                if (seg0 & 0xfe00) == 0xfc00 {
                    return Err(UniqueLocalIp);
                }
                if (seg0 & 0xffc0) == 0xfe80 {
                    return Err(LinkLocalIp);
                }
            }
        }
        return Ok(());
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
}
