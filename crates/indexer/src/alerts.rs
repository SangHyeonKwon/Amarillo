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

use std::time::Duration;

use anyhow::Result;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgPool;

use db::models::AlertMatch;
use db::validators::webhook_url_is_safe;

const USER_AGENT: &str = concat!("amarillo-alerts/", env!("CARGO_PKG_VERSION"));
/// HMAC 본문 서명을 담을 헤더. 값 형식 `sha256=<hex>`.
const SIGNATURE_HEADER: &str = "X-Amarillo-Signature";
const REQUEST_TIMEOUT_SECS: u64 = 10;
const DISPATCH_BATCH: i64 = 100;

type HmacSha256 = Hmac<Sha256>;

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

    // SSRF 가드 단위테스트는 `db::validators::tests`로 이주됨(공유 검증기).
    // 본 모듈은 HMAC 서명·payload 빌더 등 디스패처 고유 순수 로직만 검증.

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
