# BACKLOG

미완료 슬라이스/항목 카탈로그. *언제* 무엇을 할지의 판단을 모은다 — ROADMAP은
마일스톤 출하 기록 유지, 본 문서는 *결정 보조*.

각 항목 형식:

- **가치** — 누가 / 왜 / 얼마나
- **리스크** — 기술 / 스코프 / 검증 부담
- **페르소나** — 어떤 사용자/잡을 직격하는가
- **사전조건** — 시작 전 필요한 결정·인프라
- **예상 크기** — 슬라이스 단위 (작 / 중 / 큼)

상태: 미완료 3 / 완료 14 (완료분은 ROADMAP 한 줄 압축).
(임계율 알림과 S15 봇 라벨/cookbook은 M005 출하, DNS-time SSRF는 HARDEN3
단독 PR로 완료, AmarilloClient admin 메서드는 EXAMPLES-ADMIN 단독 PR로 완료,
인증 미들웨어는 M006 출하로 완료, S11.1 ABI args 디코딩 + **S12.1 ErrorCategory
세분화**는 각각 별 단독 PR로 완료 — 본 카탈로그에서 각 항목 제거.)

---

## M004 깊이/운영성 잔여

### S13.1 — npm / PyPI 패키지 게시

S13의 *운영성* 확장 — 카피 가능 examples → 정식 패키지.

- **가치**: 사용자가 `npm install @amarillo/client` / `pip install
  amarillo-client` 가능. 패키지 생태계 진입(SEO·신뢰도·자동 업데이트).
- **리스크**: 작(코드) / 중(운영) — 게시 토큰 / CI 게시 자동화 / semver /
  종속 그래프 관리. 첫 사용자 명시 요청 없으면 *낭비*(D017 정신).
- **페르소나**: dApp 개발자 (S13 동일)
- **사전조건**: npm / PyPI 계정 + 게시 토큰 + (선택) GitHub Actions 게시 워크플로.
- **예상 크기**: 작(코드 분리) / 중(운영 셋업).

---

## 보안 / 운영 단독 단위

### OS resolver 캐시 race 차단 (hickory-dns 직접 UDP)

HARDEN3 *잔여* — `SafeDnsResolver`(reqwest dns_resolver hook + stdlib
`to_socket_addrs`)는 OS resolver를 통하므로 *커널 stub resolver* (nscd /
systemd-resolved)가 캐시한 stale IP는 우리 코드 밖. 완전 차단은 직접 UDP
DNS resolution까지.

- **가치**: 잔여 SSRF 갭 해소. 실무 리스크는 낮음(공격자가 우리 OS의 stub
  resolver 캐시까지 poisoning해야 가능) — 첫 사용자 요구 없으면 *낭비*에
  가까움.
- **리스크**: 중-큼 — hickory-dns(또는 trust-dns) 의존 도입 + 자체 UDP/TCP
  resolver 운영 + 캐시 정책 + 환경 차이(IPv4-only vs dual-stack 등).
- **페르소나**: 보안 운영자 (간접)
- **사전조건**: hickory-dns 의존 도입 결정. SafeDnsResolver와의 통합 (trait
  같지만 backend 교체).
- **예상 크기**: 중. 첫 사용자 명시 요청 후 진행 권장.

### 별 단위 hardening — toolchain 회귀 lint 정리

S16에서 toolchain 1.92(rust-clippy 1.92) 회귀 lint 2건을 *인라인 fix*로 게이트
통과 — 의미 무변경, 의도 보존이지만 *별 슬라이스에서 깔끔하게 리팩토링* 후보.

- **가치**: 작음 — 코드 품질·일관성. lint allow 제거 + 더 명확한 표현.
- **리스크**: 작음 — test 코드만 영향, 의미 보존 단순 리팩토링.
- **페르소나**: 개발자 (자기 코드베이스 품질)
- **사전조건**: 없음.
- **예상 크기**: 작 — 두 위치 fix + clippy 무회귀.
- **위치**:
  - `crates/decoder/src/events.rs` `test_decode_swap_event` —
    `#[allow(clippy::cmp_owned)]` + `BigDecimal::from(0)` 비교. *named binding*
    (`let zero = BigDecimal::from(0); assert!(s.amount_in > zero);`) 패턴으로
    리팩토링하면 lint allow 제거 가능.
  - `crates/indexer/src/worker.rs` (이미 인라인 fix 완료 — `&chain` → `chain`).
    *별 단위에 합쳐 한 PR로 깔끔하게* 정리 가능.

본 항목은 *작은 단위*이라 별 마일스톤 가치 X, *언제든 단독 PR*. 첫 사용자
요구 없어도 자기 코드 품질 정신으로 가능.

---

## 단독 단위 (낮은 우선순위)

### Pools/Traders 페이지 신규 API 매핑

FE-WIRE 후속 — 기존 `pool` / `trader` 페이지를 실제 신규 API(`/v1/pools` /
`/v1/traders`)에 결선. 현재는 데모 데이터 또는 미결선.

- **가치**: 낮음 — D001 일관(일반 대시보드는 Dune 강점, 미투자). 운영자 데모
  완성도 정도.
- **리스크**: 작 — 기존 page 갱신만. 새 API 엔드포인트 이미 존재.
- **페르소나**: 데모 사용자 / 운영자
- **사전조건**: 없음.
- **예상 크기**: 작 (FE만).

---

## 우선순위 (추천 — S12.1 출하 후 갱신)

| # | 항목 | 가치 | 크기 | 페르소나 |
|---|------|------|------|---------|
| 1 | S13.1 패키지 게시 | ★ (운영성) | 작/중 | dApp 개발자 |
| 2 | OS resolver 캐시 race (hickory-dns) | ★ (잔여 SSRF 갭, 첫 요구 후) | 중 | 보안 운영자 |
| 3 | 별 단위 hardening (toolchain 회귀 lint) | ☆ (코드 품질) | 작 | 개발자 |
| 4 | Pools/Traders FE | ☆ (D001 정신) | 작 | 데모 사용자 |

**해석**:
- S12.1 출하로 **M004 깊이 시리즈 자연 마감** (S10 root_cause → S11 selector
  → S11.1 args → S12 diagnosis → S12.1 enum 세분화). dApp 개발자 페르소나
  완결 — 다음 호흡은 *운영성*(S13.1) / *보안*(hickory-dns) / *코드 품질*
  (hardening) / *데모*(FE) / *새 페르소나*(M007 분기) 중 사용자 결정.
- #1: 첫 사용자 명시 요청 후 권장 (D017 정신) — 게시는 *코드 분리 작 + 운영
  중간*. 게시 토큰 / CI 자동화 / semver 관리는 *별 운영 단위*.
- #2: 첫 요구 후 (HARDEN3 정신).
- #3: *언제든 단독 PR* — 작은 hardening, 코드 품질 정신.
- #4: 영영 안 해도 무방 (D001).
- **M007 분기 후보**(M006 출하로 분기 가능 — GSD-2 원칙 일관): multi-key
  runtime 회전 / 거래소 KYC 매핑 / 자동화된 incident response / RPC 성능
  대시보드. 사용자 결정 시 BACKLOG와 같이 시드.

## 운영 규칙

- 마일스톤 분기(M006 등) 시 본 문서의 #1·#2·#3 그룹을 *시드*로 사용 — 우선순위
  표 + 사전조건 점검 후 마일스톤 ship_definition 작성.
- 단독 단위(#1, #2, #6, #7)는 *언제든* 단독 PR로 들어갈 수 있음. 마일스톤 진행
  중에도 병행 가능.
- 본 문서는 *결정 보조* — 변경된 우선순위/추정/사전조건이 있으면 즉시 갱신.
  ROADMAP의 백로그 줄은 본 문서 링크만 유지.
