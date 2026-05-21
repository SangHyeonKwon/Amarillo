# HARDEN3 — DNS-time SSRF guard (BACKLOG #1) · PLAN

Slice 목표: BACKLOG #1 (DNS-time SSRF) — dispatcher의 reqwest client에 custom
`dns_resolver`를 주입해 *resolved IP*가 unsafe면 connect 전에 실패시킨다.
`webhook_url_is_safe`의 IP 검증 로직을 `ip_is_safe` public 함수로 분리해 두
검증 단계(파싱 시점 + DNS resolve 시점)가 *같은 정책*을 공유.

엣지: `[edge: weak-spot]`. risk: med. 단독 PR (마일스톤 외). 신규 의존 0.

핵심 결정: **D020** (착수 시 기록) — stdlib `to_socket_addrs` + `reqwest::dns::
Resolve` trait, hickory-dns 등 async resolver lib 미도입.

검증 제약(D009~D019 일관): 단위테스트(SafeDnsResolver) + 기존 validators
단위테스트 무회귀 + verify-alerts.sh 무회귀. 라이브 DNS rebinding 시뮬은 mock
DNS server 필요라 수동 스모크로 위임.

태스크: T01 → T02.

---

## T01 — `ip_is_safe` 분리 + `SafeDnsResolver` 구현 + 단위테스트 + D020

**Must-haves**
- *Truths*
  - `crates/db/src/validators.rs`:
    - `ip_is_safe(ip: IpAddr) -> Result<(), UnsafeUrlReason>` 신규 public 함수.
      IPv4-mapped IPv6 normalize → IPv4 규칙 / 순수 IPv6 규칙 (현재
      `webhook_url_is_safe` 안의 *IP 검증 블록*과 정확히 같은 로직).
    - `webhook_url_is_safe`가 `ip_is_safe`를 호출하도록 리팩토링 — 정책 단일
      출처. 기존 단위테스트(10+건) 무회귀 보장.
  - `crates/indexer/src/alerts.rs`:
    - `SafeDnsResolver` struct + `impl reqwest::dns::Resolve` —
      `resolve(name)`에서 stdlib `to_socket_addrs`를 `tokio::task::spawn_
      blocking`으로 호출, 각 resolved IP를 `db::validators::ip_is_safe`로
      검증. unsafe IP 발견 시 `Err` 반환 → reqwest connect 단계에서 실패.
    - `dispatch_loop`의 `reqwest::Client::builder()`에 `.dns_resolver(
      Arc::new(SafeDnsResolver))` 적용.
  - 단위테스트 (`crates/indexer/src/alerts.rs` 또는 별 모듈):
    - `SafeDnsResolver`가 unsafe IP 직접 입력에 대해 Err 반환 (mock하지 않고
      `ip_is_safe` 호출 검증)
    - hostname → safe IP(8.8.8.8 같은 공개 IP) 시 Ok (실제 DNS 호출 — 환경
      의존, ignore 또는 mock)
    - 또는: `Resolve` trait를 unit 단위에서 호출하지 말고, *`ip_is_safe`
      직접 단위테스트만* — DNS resolve는 통합/수동
  - D020 결정 기록 — DECISIONS.md
  - prod `unwrap()` 0 / 신규 public `///` doc.
- *Artifacts*: `crates/db/src/validators.rs`, `crates/indexer/src/alerts.rs`,
  `.gsd/DECISIONS.md`
- *Key Links*: S08-T02 `webhook_url_is_safe` (잔여 리스크 코멘트가 해소
  대상), HARDEN-T03 bounded parallelism 패턴

## T02 — 회귀 게이트 + docs 보안 모델 갱신 + BACKLOG #1 제거 + HARDEN3-SUMMARY

**Must-haves**
- *Truths*
  - cargo fmt + clippy --workspace + cargo test -p db --lib (validators 단위
    테스트 무회귀) + cargo test -p indexer + cargo test -p db -- --ignored
    + verify 3종 ALL PASS 확인.
  - `docs/api-alerts.md`의 "Security posture (honest)" 절(또는 동등) 갱신:
    - 기존 "잔여 리스크: DNS-time IP rebinding"이 해소됨 명시
    - DNS resolve 시점 IP 검증 (SafeDnsResolver) 설명
    - 정직한 한계: OS resolver 신뢰 (DNS response 캐싱 race는 *우리 코드 밖*),
      mock DNS server 자동 시뮬 부재
  - BACKLOG.md: #1 DNS-time SSRF 제거 + 우선순위 표 갱신.
  - `.gsd/HARDEN3-SUMMARY.md` + (선택) KNOWLEDGE에 "DNS-time SSRF + 정책 단일
    출처(ip_is_safe)" Lesson 한 줄.
- *Reassess*: 출하 후 — BACKLOG 다음 우선순위(#2 AmarilloClient admin 메서드
  또는 #3 S11.1) 사용자 결정. M001~M005 출하 상태 변경 없음 (단독 보안 잔여).
- *Artifacts*: `docs/api-alerts.md`, `.gsd/{BACKLOG,HARDEN3-SUMMARY}.md`,
  (선택) `.gsd/KNOWLEDGE.md`

---

## Slice 수용 (Complete)
- [ ] T01–T02 must-haves, 모든 표면 무회귀
- [ ] DB --ignored + indexer + db lib + clippy + fmt 모두 green
- [ ] verify 3종 ALL PASS (무회귀)
- [ ] BACKLOG #1 제거 + HARDEN3-SUMMARY + docs 보안 절 갱신
- [ ] D020 기록 + (선택) KNOWLEDGE Lesson
