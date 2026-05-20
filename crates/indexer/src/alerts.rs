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
use serde::Serialize;
use sha2::Sha256;
use sqlx::PgPool;

use db::models::AlertMatch;
use db::validators::webhook_url_is_safe;

const USER_AGENT: &str = concat!("amarillo-alerts/", env!("CARGO_PKG_VERSION"));
/// HMAC 본문 서명을 담을 헤더. 값 형식 `sha256=<hex>`.
const SIGNATURE_HEADER: &str = "X-Amarillo-Signature";
const REQUEST_TIMEOUT_SECS: u64 = 10;
const DISPATCH_BATCH: i64 = 100;
/// claim 토큰이 stale로 간주되는 임계 (HARDEN-T02). 잡고 죽은 워커의 'claimed'
/// 행은 이 시간 후 자동 재claim 가능. `REQUEST_TIMEOUT_SECS` (10s)의 6배 마진.
/// `find_pending_alert_matches` 의 anti-join도 같은 값을 사용 — 두 함수가 한
/// 기준을 공유해야 동일 row가 한 워커에게만 보임.
const CLAIM_STALE_AFTER_SECS: i64 = 60;
/// 한 사이클 안에 동시에 진행할 수 있는 webhook POST 수의 상한 (HARDEN-T03/M2).
/// 직렬일 때 워스트케이스 `DISPATCH_BATCH × REQUEST_TIMEOUT_SECS` (100×10s=17min)을
/// `~MAX_CONCURRENT_POSTS`배 단축한다. 수신자 부하·로컬 file descriptor·DB 풀
/// 사이즈의 균형점. 너무 키우면 한 receiver에 thundering herd가 가능 — 보수적 기본값.
const MAX_CONCURRENT_POSTS: usize = 10;

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

/// 외부 에러 메시지에서 `http(s)://…` URL을 `<redacted-url>`로 치환한다 (순수,
/// HARDEN2-T01).
///
/// `alert_delivery.last_error`에 영구 저장되는 메시지가 webhook URL/내부 IP/
/// DNS 진단을 누설하지 않도록 보호. URL 종료는 **첫 공백 문자**로 단순 식별 —
/// 종결자 휴리스틱의 false negative보다 약간의 over-redact를 우선(누설 방지가
/// 1순위). 스킴이 없는 호스트는 false positive 방지를 위해 손대지 않는다.
pub(crate) fn redact_urls(input: &str) -> String {
    const REDACT: &str = "<redacted-url>";
    let schemes = ["https://", "http://"];
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    loop {
        let mut next: Option<usize> = None;
        for s in &schemes {
            if let Some(p) = rest.find(s) {
                next = Some(match next {
                    None => p,
                    Some(q) => q.min(p),
                });
            }
        }
        let Some(start) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let end = after
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(after.len());
        out.push_str(REDACT);
        rest = &after[end..];
    }
    out
}

/// 디스패처 누적 지표(프로세스 메모리).
#[derive(Debug, Default, Clone)]
pub struct DispatchMetrics {
    /// 루프 사이클 수
    pub cycles: u64,
    /// 시도한 매칭 건수(누적) — 매칭 후보 모두
    pub attempted: u64,
    /// 성공 전송 누적
    pub delivered: u64,
    /// 실패(재시도 대상) 누적
    pub failed: u64,
    /// SSRF 가드로 스킵된 매칭(실패로 기록)
    pub unsafe_url_skipped: u64,
    /// claim 실패로 스킵된 매칭 — 다른 워커가 들고 있거나 이미 delivered
    /// (HARDEN-T02). 정상 분산 처리의 신호이지 결함이 아님.
    pub claim_skipped: u64,
}

/// webhook 본문 페이로드 — `serde` derive로 키 순서(struct 정의 순) 고정해 서명
/// 결정성 보장. 수기 `format!` 보간 대신 derive를 써 future-field 추가 시 escape
/// 버그를 방지한다(리뷰 L2).
#[derive(Serialize)]
struct AlertPayload<'a> {
    subscription_id: i64,
    tx_hash: &'a str,
}

/// 안정 직렬화된 webhook 본문. `subscription_id`/`tx_hash`만 담는 고정 스키마라
/// 직렬화 실패는 발생하지 않음(infallible — `expect`로 명시).
fn build_payload(m: &AlertMatch) -> String {
    serde_json::to_string(&AlertPayload {
        subscription_id: m.subscription_id,
        tx_hash: &m.tx_hash,
    })
    .expect("AlertPayload {i64, &str} cannot fail to serialize")
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

/// 단일 매칭 항목 처리 결과 — 병렬 orchestrator가 [`DispatchMetrics`]에 집계.
#[derive(Debug, PartialEq, Eq)]
enum DispatchOutcome {
    Delivered,
    Failed,
    UnsafeUrl,
    ClaimSkipped,
}

/// 한 매칭 항목의 전 과정: claim → SSRF 가드 → 서명 → POST → 멱등 기록.
/// `pool`/`client`는 소유 인자로 받아 `'static` 미래로 spawn 가능(HARDEN-T03/M2).
async fn dispatch_item(
    pool: PgPool,
    client: reqwest::Client,
    item: AlertMatch,
) -> Result<DispatchOutcome> {
    let claimed = db::queries::try_claim_alert_match(
        &pool,
        item.subscription_id,
        &item.tx_hash,
        CLAIM_STALE_AFTER_SECS,
    )
    .await?;
    if !claimed {
        tracing::debug!(
            subscription_id = item.subscription_id,
            tx_hash = %item.tx_hash,
            "claim not acquired (held by another worker or already delivered) — skipping"
        );
        return Ok(DispatchOutcome::ClaimSkipped);
    }
    if let Err(reason) = webhook_url_is_safe(&item.webhook_url) {
        tracing::warn!(
            subscription_id = item.subscription_id,
            tx_hash = %item.tx_hash,
            ?reason,
            "webhook URL unsafe — skipping (recorded as failed)"
        );
        db::queries::record_alert_delivery(
            &pool,
            item.subscription_id,
            &item.tx_hash,
            false,
            Some(&format!("unsafe url: {reason:?}")),
        )
        .await?;
        return Ok(DispatchOutcome::UnsafeUrl);
    }
    // HMAC 키 = 저장된 hex 문자열을 32바이트로 디코드(리뷰 H2). API가 항상
    // 64-hex를 박지만 외부 변조에 대비.
    let key = match hex::decode(&item.signing_secret) {
        Ok(k) => k,
        Err(_) => {
            tracing::warn!(
                subscription_id = item.subscription_id,
                tx_hash = %item.tx_hash,
                "stored signing_secret is not valid hex — recording as failed"
            );
            db::queries::record_alert_delivery(
                &pool,
                item.subscription_id,
                &item.tx_hash,
                false,
                Some("corrupt signing_secret (not hex)"),
            )
            .await?;
            return Ok(DispatchOutcome::Failed);
        }
    };
    let body = build_payload(&item);
    let signature = sign_payload(&key, body.as_bytes());
    match post_signed(&client, &item.webhook_url, &signature, body).await {
        Ok(()) => {
            db::queries::record_alert_delivery(
                &pool,
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
            Ok(DispatchOutcome::Delivered)
        }
        Err(e) => {
            // 리뷰 L1: 외부 에러 메시지(reqwest)는 URL/IP/내부 진단을 포함해 영구
            // 저장된다. 순서가 중요: ① redact_urls로 URL 마스킹(HARDEN2-T01),
            // ② 500자 캡(원본 사이즈 폭주 방어). redact 먼저여야 잘림 직전에
            // 노출되던 URL 꼬리도 안전.
            let err_msg: String = redact_urls(&e.to_string()).chars().take(500).collect();
            tracing::warn!(
                subscription_id = item.subscription_id,
                tx_hash = %item.tx_hash,
                error = %err_msg,
                "alert delivery failed (will retry)"
            );
            db::queries::record_alert_delivery(
                &pool,
                item.subscription_id,
                &item.tx_hash,
                false,
                Some(&err_msg),
            )
            .await?;
            Ok(DispatchOutcome::Failed)
        }
    }
}

/// 디스패처 1사이클: 배치를 가져와 각 매칭을 `MAX_CONCURRENT_POSTS` 만큼
/// **bounded 동시**(HARDEN-T03/M2) 처리한다.
///
/// `client`는 `Policy::none()` + 타임아웃 + UA가 설정된 상태로 주입한다(SSRF의
/// 리다이렉트 우회 방지). 한 사이클만 수행 — 루프는 [`dispatch_loop`]. 한 task
/// 가 panic/내부 에러로 실패해도 다른 task는 계속 처리(알림은 best-effort);
/// 실패는 `metrics.failed`로 가시화.
pub async fn dispatch_once(
    pool: &PgPool,
    client: &reqwest::Client,
    batch: i64,
    metrics: &mut DispatchMetrics,
) -> Result<()> {
    let pending =
        db::queries::find_pending_alert_matches(pool, batch, CLAIM_STALE_AFTER_SECS).await?;
    let mut iter = pending.into_iter();
    let mut tasks: tokio::task::JoinSet<Result<DispatchOutcome>> = tokio::task::JoinSet::new();

    // Prime: spawn up to MAX_CONCURRENT_POSTS tasks
    while tasks.len() < MAX_CONCURRENT_POSTS {
        let Some(item) = iter.next() else { break };
        metrics.attempted += 1;
        let pool = pool.clone();
        let client = client.clone();
        tasks.spawn(async move { dispatch_item(pool, client, item).await });
    }

    // Drain + refill: 완료마다 다음 1개를 spawn해 항상 ≤MAX 유지
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(Ok(outcome)) => match outcome {
                DispatchOutcome::Delivered => metrics.delivered += 1,
                DispatchOutcome::Failed => metrics.failed += 1,
                DispatchOutcome::UnsafeUrl => metrics.unsafe_url_skipped += 1,
                DispatchOutcome::ClaimSkipped => metrics.claim_skipped += 1,
            },
            Ok(Err(e)) => {
                // 내부 에러(DB 등) — task 자체는 살아남았으나 결과가 에러.
                // 알림은 best-effort라 사이클 전체를 죽이지 않고 격리.
                tracing::error!(error = %e, "alert dispatch task returned error");
                metrics.failed += 1;
            }
            Err(e) => {
                // task panic 또는 cancellation
                tracing::error!(error = %e, "alert dispatch task panicked or was cancelled");
                metrics.failed += 1;
            }
        }
        if let Some(item) = iter.next() {
            metrics.attempted += 1;
            let pool = pool.clone();
            let client = client.clone();
            tasks.spawn(async move { dispatch_item(pool, client, item).await });
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
            claim_skipped = metrics.claim_skipped,
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

    #[test]
    fn dispatcher_key_uses_hex_decoded_secret() {
        // 리뷰 H2 회귀: 저장된 hex 문자열을 디코드한 32바이트를 HMAC 키로 쓴다
        // — `as_bytes()`로 hex 문자(64B ASCII)를 그대로 키로 쓰는 경로와는 결과가
        // 달라야 한다(수신자 인터옵 기대치 일치).
        let hex_secret = "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
        let body = b"Hi There";
        let decoded = hex::decode(hex_secret).expect("test fixture is valid hex");
        let with_decode = sign_payload(&decoded, body);
        let without_decode = sign_payload(hex_secret.as_bytes(), body);
        assert_ne!(
            with_decode, without_decode,
            "decoded(32B) vs as-bytes(64B hex chars) must produce different HMACs"
        );
        assert_eq!(with_decode.len(), 64);
    }

    // ── 본문 빌더 ──────────────────────────────────────────────

    // ── redact_urls (HARDEN2-T01) ─────────────────────────────

    #[test]
    fn redact_urls_passthrough_when_no_url() {
        assert_eq!(redact_urls("connection refused"), "connection refused");
        assert_eq!(redact_urls(""), "");
    }

    #[test]
    fn redact_urls_single_https() {
        assert_eq!(
            redact_urls("failed: https://example.com/hook"),
            "failed: <redacted-url>"
        );
    }

    #[test]
    fn redact_urls_handles_http_and_https() {
        assert_eq!(
            redact_urls("http://internal/x timed out"),
            "<redacted-url> timed out"
        );
    }

    #[test]
    fn redact_urls_multiple() {
        assert_eq!(
            redact_urls("two: https://a.com and https://b.com here"),
            "two: <redacted-url> and <redacted-url> here"
        );
    }

    #[test]
    fn redact_urls_url_at_end_with_port_and_path() {
        assert_eq!(
            redact_urls("Connection refused on https://10.0.0.1:8080/x"),
            "Connection refused on <redacted-url>"
        );
    }

    #[test]
    fn redact_urls_no_false_positives() {
        // 스킴 없는 호스트·식별자는 건드리지 않음
        assert_eq!(
            redact_urls("see example.com for details"),
            "see example.com for details"
        );
        assert_eq!(
            redact_urls("tx_hash=0xabc failed at https://x"),
            "tx_hash=0xabc failed at <redacted-url>"
        );
    }

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

    // ── HARDEN-T03 / L3: mock receiver e2e (wire 서명 계약) ──────
    //
    // SSRF 가드가 loopback을 막아 production `dispatch_once` 전체를 localhost로
    // 돌릴 수 없다(가드 우회는 보안 회귀라 거부). 대신 *wire 서명 계약*만 격리해
    // 검증: 디스패처가 보내는 `sign_payload + post_signed`의 결과가 수신자
    // 입장에서 (a) HMAC 재계산과 일치하고 (b) 본문이 그대로 도착하며 (c) 헤더
    // 형식이 계약대로인지 확인. SSRF rejection 통합은 14 단위 + `verify-alerts.sh`가
    // 별도 가드. 클레임/기록은 claim 통합테스트가 별도 가드.

    #[tokio::test]
    async fn wire_signed_post_roundtrips_to_receiver() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local TCP");
        let port = listener.local_addr().expect("addr").port();

        // 수신자 비밀 — 32바이트 키, hex 인코딩된 형태는 DB에 저장될 형태
        let secret_bytes: [u8; 32] = [0xaa; 32];
        let secret_hex = hex::encode(secret_bytes);
        let secret_for_verify = secret_bytes.to_vec();

        // 수신자 태스크: 1 connection 수락 → 요청 파싱 → HMAC 검증 → 200 응답
        let receiver = tokio::spawn(async move {
            let (stream, _peer) = listener.accept().await.expect("accept");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = tokio::io::BufReader::new(read_half);

            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .await
                .expect("read request line");

            let mut content_length = 0usize;
            let mut signature_header: Option<String> = None;
            let mut content_type: Option<String> = None;
            loop {
                let mut line = String::new();
                let n = reader.read_line(&mut line).await.expect("read header");
                if n == 0 || line == "\r\n" {
                    break;
                }
                let trimmed = line.trim_end_matches("\r\n");
                let lower = trimmed.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    content_length = v.trim().parse().expect("content-length parse");
                } else if let Some(v) = lower.strip_prefix("x-amarillo-signature:") {
                    signature_header = Some(v.trim().to_string());
                } else if let Some(v) = lower.strip_prefix("content-type:") {
                    content_type = Some(v.trim().to_string());
                }
            }

            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).await.expect("read body");

            let header_val = signature_header.expect("signature header present");
            let signature_hex = header_val
                .strip_prefix("sha256=")
                .expect("sha256= prefix")
                .to_string();
            let expected = sign_payload(&secret_for_verify, &body);
            let sig_ok = signature_hex == expected;

            let _ = write_half
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await;
            let _ = write_half.flush().await;

            (sig_ok, body, content_type, request_line)
        });

        let item = AlertMatch {
            subscription_id: 42,
            tx_hash: "0xabc".to_string(),
            webhook_url: format!("http://127.0.0.1:{port}/hook"),
            signing_secret: secret_hex,
        };

        // production 디스패처와 동일한 client 설정(redirect off + timeout)
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .expect("client");

        // wire 경로: production이 따르는 정확한 시퀀스
        let body = build_payload(&item);
        let key = hex::decode(&item.signing_secret).expect("hex decode");
        let signature = sign_payload(&key, body.as_bytes());
        post_signed(&client, &item.webhook_url, &signature, body.clone())
            .await
            .expect("post_signed");

        let (sig_ok, body_received, content_type, request_line) =
            receiver.await.expect("receiver join");
        assert!(sig_ok, "수신자의 HMAC 재계산이 디스패처 서명과 일치해야");
        assert_eq!(
            String::from_utf8(body_received).expect("utf8 body"),
            body,
            "본문이 wire로 그대로 round-trip"
        );
        assert_eq!(
            content_type.as_deref(),
            Some("application/json"),
            "Content-Type 계약"
        );
        assert!(
            request_line.starts_with("POST "),
            "method = POST: got {request_line:?}"
        );
    }
}
