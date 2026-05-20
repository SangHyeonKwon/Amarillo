//! S08 — actionable alerts dispatcher.
//!
//! Outbox 패턴(D012): follow 루프와 분리된 디스패처가 (활성 구독 × 매칭 실패 tx)
//! 를 스캔해 HMAC 서명된 webhook을 정확히 1회 POST한다. 실패 격리: 느린/깨진
//! webhook이 인덱싱·reorg 정정을 막지 않는다. 순수(SSRF 가드/HMAC 서명) + 얇은
//! 비동기 드라이버 — S05/S06의 "순수 결정 + 얇은 IO" 패턴 일관.
//!
//! 라이브 POST는 수신 엔드포인트가 필요하므로 자동 테스트 대상이 아님(D009~D012
//! 일관). 순수 SSRF·HMAC 단위 + 매칭 anti-join 통합테스트(`-p db --ignored`)가
//! 1차 증빙, 실제 전송·폴백은 컴파일+clippy+수동 스모크·문서.

use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;

use db::models::AlertMatch;

const USER_AGENT: &str = concat!("amarillo-alerts/", env!("CARGO_PKG_VERSION"));
/// HMAC 본문 서명을 담을 헤더. 값 형식 `sha256=<hex>`.
const SIGNATURE_HEADER: &str = "X-Amarillo-Signature";
const REQUEST_TIMEOUT_SECS: u64 = 10;
const DISPATCH_BATCH: i64 = 100;

type HmacSha256 = Hmac<Sha256>;

/// SSRF 가드의 거부 사유 (순수 — 진단/테스트용 정수형 enum).
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

/// webhook URL이 외부로 안전하게 POST할 수 있는 형태인지 **순수** 검증.
///
/// 정책: ① `https` 스킴만 허용. ② IP 리터럴이면 loopback/private(RFC1918)/
/// link-local(169.254 메타데이터 포함)/unspecified/multicast/broadcast/IPv6
/// ULA·link-local 거부. ③ 호스트명이면 `localhost`/`*.localhost`/`*.local`(mDNS)
/// 거부. **잔여 리스크(정직)**: DNS 시점 IP 재바인딩(공격자가 사설 IP로 해석되는
/// 도메인 등록)은 본 가드만으로는 못 막는다 — 완전 해소는 연결 시점 IP 검사
/// (backlog). 리다이렉트는 호출 측 `reqwest::Policy::none()`로 비추적해 한 단계
/// 우회는 차단.
pub fn webhook_url_is_safe(url: &str) -> Result<(), UnsafeUrlReason> {
    use UnsafeUrlReason::*;
    let parsed = reqwest::Url::parse(url).map_err(|_| InvalidUrl)?;
    if parsed.scheme() != "https" {
        return Err(NonHttps);
    }
    let host_raw = parsed.host_str().ok_or(NoHost)?;
    if host_raw.is_empty() {
        return Err(NoHost);
    }
    // IPv6 리터럴은 `host_str`이 대괄호 포함("[::1]")으로 반환 → IpAddr 파싱
    // 가능한 형태로 정규화한다(대괄호 벗기기).
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
                    // 169.254.169.254 (AWS/GCP/Azure 메타데이터)도 여기서 차단
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

/// `HMAC-SHA256(secret, body)` → 소문자 hex 64자. 순수.
///
/// 본문은 호출자가 안정 직렬화(키 순서·공백 고정)해 전달해야 서명이 결정적이다
/// — [`build_payload`] 참조.
pub fn sign_payload(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// 디스패처 누적 지표(프로세스 메모리).
#[derive(Debug, Default, Clone)]
pub struct DispatchMetrics {
    /// 루프 사이클 수
    pub cycles: u64,
    /// 시도한 매칭 건수(누적)
    pub attempted: u64,
    /// 성공 전송 누적
    pub delivered: u64,
    /// 실패(재시도 대상) 누적
    pub failed: u64,
    /// SSRF 가드로 스킵된 매칭(실패로 기록)
    pub unsafe_url_skipped: u64,
}

/// 안정 직렬화된 webhook 본문 — 키 순서·공백 고정으로 서명 결정성 보장.
fn build_payload(m: &AlertMatch) -> String {
    format!(
        r#"{{"subscription_id":{},"tx_hash":"{}"}}"#,
        m.subscription_id, m.tx_hash
    )
}

async fn post_signed(
    client: &reqwest::Client,
    url: &str,
    signature_hex: &str,
    body: String,
) -> Result<()> {
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .header(SIGNATURE_HEADER, format!("sha256={signature_hex}"))
        .body(body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("http: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("http status {}", resp.status()))
    }
}

/// 디스패처 1사이클: 배치를 가져와 각 매칭에 SSRF 검증 → 서명 → POST → 멱등 기록.
///
/// `client`는 `Policy::none()` + 타임아웃 + UA가 설정된 상태로 주입한다(SSRF의
/// 리다이렉트 우회 방지). 한 사이클만 수행 — 루프는 [`dispatch_loop`].
pub async fn dispatch_once(
    pool: &PgPool,
    client: &reqwest::Client,
    batch: i64,
    metrics: &mut DispatchMetrics,
) -> Result<()> {
    let pending = db::queries::find_pending_alert_matches(pool, batch).await?;
    for item in pending {
        metrics.attempted += 1;
        if let Err(reason) = webhook_url_is_safe(&item.webhook_url) {
            tracing::warn!(
                subscription_id = item.subscription_id,
                tx_hash = %item.tx_hash,
                ?reason,
                "webhook URL unsafe — skipping (recorded as failed)"
            );
            metrics.unsafe_url_skipped += 1;
            db::queries::record_alert_delivery(
                pool,
                item.subscription_id,
                &item.tx_hash,
                false,
                Some(&format!("unsafe url: {reason:?}")),
            )
            .await?;
            continue;
        }
        let body = build_payload(&item);
        let signature = sign_payload(item.signing_secret.as_bytes(), body.as_bytes());
        match post_signed(client, &item.webhook_url, &signature, body).await {
            Ok(()) => {
                metrics.delivered += 1;
                db::queries::record_alert_delivery(
                    pool,
                    item.subscription_id,
                    &item.tx_hash,
                    true,
                    None,
                )
                .await?;
                tracing::info!(
                    subscription_id = item.subscription_id,
                    tx_hash = %item.tx_hash,
                    "alert delivered"
                );
            }
            Err(e) => {
                metrics.failed += 1;
                let err_msg = e.to_string();
                tracing::warn!(
                    subscription_id = item.subscription_id,
                    tx_hash = %item.tx_hash,
                    error = %err_msg,
                    "alert delivery failed (will retry)"
                );
                db::queries::record_alert_delivery(
                    pool,
                    item.subscription_id,
                    &item.tx_hash,
                    false,
                    Some(&err_msg),
                )
                .await?;
            }
        }
    }
    Ok(())
}

/// 디스패처 루프 — [`dispatch_once`] → sleep → 반복. ctrl_c graceful 종료.
pub async fn dispatch_loop(pool: PgPool, poll: Duration) -> Result<()> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()?;
    tracing::info!(
        poll_secs = poll.as_secs(),
        batch = DISPATCH_BATCH,
        "alert dispatcher started (Ctrl-C to stop)"
    );
    let mut metrics = DispatchMetrics::default();
    loop {
        metrics.cycles += 1;
        dispatch_once(&pool, &client, DISPATCH_BATCH, &mut metrics).await?;
        tracing::info!(
            cycle = metrics.cycles,
            attempted = metrics.attempted,
            delivered = metrics.delivered,
            failed = metrics.failed,
            unsafe_url_skipped = metrics.unsafe_url_skipped,
            "dispatch cycle summary"
        );
        tokio::select! {
            _ = tokio::time::sleep(poll) => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Ctrl-C received — stopping dispatcher");
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SSRF 가드 ─────────────────────────────────────────────

    #[test]
    fn ssrf_https_only() {
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
    fn ssrf_rejects_localhost_variants() {
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
    fn ssrf_rejects_loopback_ip() {
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
    fn ssrf_rejects_private_ip() {
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
    fn ssrf_rejects_link_local_and_cloud_metadata() {
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
    fn ssrf_rejects_unspecified_and_multicast() {
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
    fn ssrf_rejects_ipv6_ula_and_link_local() {
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
    fn ssrf_rejects_invalid_url() {
        assert_eq!(
            webhook_url_is_safe("not a url"),
            Err(UnsafeUrlReason::InvalidUrl)
        );
        assert_eq!(webhook_url_is_safe(""), Err(UnsafeUrlReason::InvalidUrl));
    }

    #[test]
    fn ssrf_allows_public_hosts_and_ips() {
        assert!(webhook_url_is_safe("https://example.com/hook").is_ok());
        assert!(webhook_url_is_safe("https://api.example.test/v1/alerts").is_ok());
        assert!(webhook_url_is_safe("https://8.8.8.8/x").is_ok());
        assert!(webhook_url_is_safe("https://[2001:db8::1]/x").is_ok());
    }

    // ── HMAC 서명 ──────────────────────────────────────────────

    #[test]
    fn sign_payload_is_deterministic_lowercase_hex_64() {
        let sig = sign_payload(b"k", b"hello");
        assert_eq!(sig.len(), 64);
        assert!(sig
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        // 같은 입력 → 같은 결과
        assert_eq!(sig, sign_payload(b"k", b"hello"));
        // 다른 본문/키 → 다른 결과
        assert_ne!(sig, sign_payload(b"k", b"hellp"));
        assert_ne!(sig, sign_payload(b"k2", b"hello"));
    }

    #[test]
    fn sign_payload_known_vector() {
        // RFC 4231 Test Case 1: key = 0x0b*20, data = "Hi There"
        // 기대값: HMAC-SHA256 = b0344c61d8db38535ca8afceaf0bf12b
        //                      881dc200c9833da726e9376c2e32cff7
        let key = [0x0bu8; 20];
        let sig = sign_payload(&key, b"Hi There");
        assert_eq!(
            sig,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn sign_payload_accepts_empty_key_and_body() {
        let sig = sign_payload(b"", b"");
        assert_eq!(sig.len(), 64);
    }

    // ── 본문 빌더 ──────────────────────────────────────────────

    #[test]
    fn build_payload_is_stable() {
        let m = AlertMatch {
            subscription_id: 42,
            tx_hash: "0xabc".to_string(),
            webhook_url: "https://x".to_string(),
            signing_secret: "s".to_string(),
        };
        let p = build_payload(&m);
        assert_eq!(p, r#"{"subscription_id":42,"tx_hash":"0xabc"}"#);
        // 같은 입력 → 같은 본문 (서명 결정성)
        assert_eq!(p, build_payload(&m));
    }
}
