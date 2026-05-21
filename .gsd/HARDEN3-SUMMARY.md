---
slice: HARDEN3
title: DNS-time SSRF guard — SafeDnsResolver + ip_is_safe 단일 출처
status: done
edge: weak-spot
milestone: standalone (BACKLOG #1)
tasks: [T01, T02]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 17/17 (ip_is_safe 3 신규) · -p db --ignored 27/27 무회귀 · verify 3종 ALL PASS 무회귀
decision: D020
artifacts:
  - crates/db/src/validators.rs           # ip_is_safe public 분리 + webhook_url_is_safe가 호출 (정책 단일 출처) + 단위테스트 3 신규
  - crates/indexer/src/alerts.rs          # SafeDnsResolver(reqwest::dns::Resolve 구현) + dispatch_loop의 client builder에 dns_resolver 주입
  - docs/api-alerts.md                    # Security posture 절: DNS-time SSRF 잔여 → HARDEN3로 해소 명시 + OS resolver 캐시 race를 새 잔여로 정직 기록
  - .gsd/DECISIONS.md                     # D020 (reqwest hook + stdlib to_socket_addrs, hickory-dns 미도입)
  - .gsd/BACKLOG.md                       # #1 DNS-time SSRF 제거 + 우선순위 표 갱신 + "OS resolver 캐시 race (hickory-dns 직접 UDP)" 잔여 신규 등재
verification_constraint: "ip_is_safe 단위테스트 + 기존 webhook_url_is_safe 단위테스트 무회귀(같은 정책 단일 출처 보장)가 핵심. SafeDnsResolver의 *라이브 DNS rebinding 시뮬*은 mock DNS server 필요 — 환경 부담 큼, 수동 스모크 또는 향후 통합테스트로 위임. OS stub resolver 캐시 race는 우리 코드 밖 — BACKLOG로 자연 추적."
---

# HARDEN3 — 무엇이 실제로 일어났나

BACKLOG #1 출하. dispatcher의 reqwest client에 custom `dns_resolver`를 주입해
DNS rebinding 공격을 *resolve 시점*에 차단. 파싱 시점 + DNS 시점이 **같은
정책**(`ip_is_safe`)을 공유하도록 리팩토링 — 정책 단일 출처.

- **T01 (ip_is_safe 분리 + SafeDnsResolver + D020)**:
  - `crates/db/src/validators.rs`에 `ip_is_safe(ip: IpAddr) -> Result<(),
    UnsafeUrlReason>` public 함수 분리. IPv4-mapped IPv6 normalize → IPv4 규칙
    + 순수 IPv6 규칙 (loopback / private / link-local / unspecified /
    multicast / broadcast / IPv6 ULA / IPv6 link-local). 기존
    `webhook_url_is_safe`가 본 함수를 호출하도록 리팩토링 — 정책 단일 출처.
    단위테스트 3 신규(public IP 통과 / loopback·private·link-local 거부 /
    IPv4-mapped IPv6 unwrap). 기존 webhook_url_is_safe 단위테스트 10+건 모두
    무회귀(같은 함수 호출).
  - `crates/indexer/src/alerts.rs`에 `SafeDnsResolver` struct +
    `impl reqwest::dns::Resolve`. resolve(name)은 stdlib `to_socket_addrs`를
    `tokio::task::spawn_blocking`으로 호출(OS resolver 활용 + blocking 격리),
    각 resolved IP를 `ip_is_safe`로 검증 → unsafe면 Err 반환(reqwest connect
    단계에서 실패).
  - `dispatch_loop`의 `reqwest::Client::builder()`에
    `.dns_resolver(Arc::new(SafeDnsResolver))` 추가. 기존 redirect/timeout/UA
    설정과 함께 일관 적용. 신규 의존 0(stdlib만 사용, hickory-dns 미도입 —
    D020 명시 결정).
  - D020 기록.

- **T02 (회귀 + docs + BACKLOG + SUMMARY)**:
  - 게이트 무회귀 확인: fmt clean / clippy 0 / db lib 17/17 / db --ignored
    27/27 / indexer 36/36 / verify-alerts.sh ALL PASS.
  - `docs/api-alerts.md`의 "Security posture (honest)" 절 갱신:
    - 기존 "Residual: DNS-time IP rebinding" 항목을 HARDEN3로 해소 명시
    - 새 항목: SafeDnsResolver 동작 + 정책 단일 출처(`ip_is_safe`) 설명
    - 새 Residual: OS stub resolver 응답 캐시 race(nscd / systemd-resolved)는
      우리 코드 밖 — hickory-dns 직접 UDP로 완전 해소 가능하지만 BACKLOG에 등재.
  - `BACKLOG.md` #1 DNS-time SSRF 제거 + 우선순위 표 갱신 + "OS resolver 캐시
    race (hickory-dns 직접 UDP)" 항목 신규 등재(잔여 솔직성).

**보안 정책 단일 출처 패턴 (D020)**
- `ip_is_safe`가 모든 IP 검증의 단일 출처 — URL 파싱 시점 / DNS resolve 시점
  / 향후 추가될 진입점(예: 인증 미들웨어에서 client_addr 검증?) 모두 동일.
- 신규 잔여(OS resolver 캐시) 발견 시 *같은 함수*에 룰 추가 → 두 진입점 자동
  반영. 정책 drift 방지.

**정직한 한계 (HARDEN3 잔여)**
- **OS stub resolver 캐시 race** — 커널 stub resolver가 캐시한 stale IP를
  공격자가 poisoning한 시점부터 우리가 보는 데 lag — 우리 코드 밖. 완전 차단은
  hickory-dns의 직접 UDP DNS resolution(BACKLOG에 등재, 첫 사용자 요구 후).
- **라이브 DNS rebinding 시뮬 자동 테스트 부재** — mock DNS server 필요(환경
  부담 큼). `ip_is_safe` 단위테스트가 정책 단일 출처를 보장하므로 dispatcher
  쪽도 같은 룰을 보고 있음은 *코드로* 명시(SafeDnsResolver가 호출).

**Reassess**: BACKLOG #1 제거 + 우선순위 표 갱신(M005 SUMMARY가 끌어올린 인증
미들웨어 / S15 후속 admin client 메서드 등이 다음 호흡 후보). M001~M005 출하
상태 변경 없음 — 본 슬라이스는 보안 잔여 닫기 단독 PR. 다음 호흡은 사용자
지시 시 BACKLOG.md 우선순위 표 + (선택) M006 분기.
