# BACKLOG

미완료 슬라이스/항목 카탈로그. *언제* 무엇을 할지의 판단을 모은다 — ROADMAP은
마일스톤 출하 기록 유지, 본 문서는 *결정 보조*.

각 항목 형식:

- **가치** — 누가 / 왜 / 얼마나
- **리스크** — 기술 / 스코프 / 검증 부담
- **페르소나** — 어떤 사용자/잡을 직격하는가
- **사전조건** — 시작 전 필요한 결정·인프라
- **예상 크기** — 슬라이스 단위 (작 / 중 / 큼)

상태: 미완료 6 / 완료 9 (완료분은 ROADMAP 한 줄 압축).
(임계율 알림과 S15 봇 라벨/cookbook은 M005 출하로 완료, DNS-time SSRF는
HARDEN3 단독 PR로 완료 — 본 카탈로그에서 제거.)

---

## M004 깊이/운영성 잔여

### S11.1 — ABI args 디코딩 + root_cause.input 디코드

S11(name/signature)의 *깊이* 확장. selector 너머의 typed value 추출.

- **가치**: 디버깅 정밀도 ↑ — `transfer(0xabc…, 1000000)` 처럼 함수명 + 타입된
  인자값 표시. 진단 응답이 한 단계 더 똑똑해짐.
- **리스크**: med — ABI 타입 시스템(address / uint / dynamic bytes / nested
  tuple) 디코더 도입. alloy-sol-types 의존 또는 자체 minimal 디코더. D015 정신
  일관 (자기소유 ABI 시드만).
- **페르소나**: dApp 개발자 (S11 동일)
- **사전조건**: ABI 디코더 라이브러리 결정(alloy-sol-types 의존 추가 vs 자체
  minimal — 무게/유지 보수 트레이드오프).
- **예상 크기**: 중-큼. 디코더 자체 + 모델 확장(`failing_function_decoded.args`)
  + 핸들러 + 통합테스트 + 프론트.

### S12.1 — error_category enum 세분화 v2

S12(메시지·액션)의 *정밀도* 확장. 현재 6 카테고리는 *조잡* — 모든
`UNAUTHORIZED`가 동일 메시지.

- **가치**: 예: `SLIPPAGE_EXCEEDED` → `SLIPPAGE_PRICE_IMPACT` /
  `SLIPPAGE_AMOUNT_OUT` 분리해 더 정확한 진단 메시지 매핑. 진단 정밀도 ↑.
- **리스크**: 큼 — `ALTER TYPE` 마이그레이션 + classifier 룰 확장 + Rust enum
  variant + 프론트 type union + 시드 데이터 분류 + category_diagnosis 시드 행
  추가. 영향 광범위 (단일 슬라이스 부담).
- **페르소나**: dApp 개발자 (S12 동일)
- **사전조건**: 세분화 명세(어떤 카테고리를 어떻게 쪼갤지) 결정.
- **예상 크기**: 큼. 2 슬라이스로 쪼갬 권장 (스키마+enum / classifier+시드).

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

## M005 후속

### AmarilloClient admin 메서드 추가 (S15 후속)

`examples/typescript-client/client.ts` + `examples/python-client/client.py` 에
`createContractLabel` / `deleteContractLabel` 메서드 추가. cookbook 4번째
시나리오의 step 1을 TS/Python에서도 직접 호출 가능하게.

- **가치**: 작 — cookbook이 이미 curl로 시연. examples 클라가 완전체가 되는
  완결성.
- **리스크**: 작 — 패턴 명확 (기존 alert/by-label 메서드와 동일). typecheck/
  py_compile만 갱신.
- **페르소나**: 봇 운영자 (S15 동일)
- **사전조건**: 없음.
- **예상 크기**: 작 (≈ 30분).

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

### 인증 미들웨어 도입 (admin/write API 보호)

M005-SUMMARY가 끌어올린 항목. S15 admin API (`POST/DELETE /v1/contract-labels`)
+ S08 alert-subscriptions의 write 엔드포인트 모두 *인증 없음*. 운영 배포 시
필수.

- **가치**: 운영 안전 — 외부 노출 가능. 본 마일스톤들의 "데모 스코프" 솔직성
  을 마감.
- **리스크**: 큼 — 인증 모델 결정(API key / signed JWT / OAuth client_credentials),
  미들웨어 도입 (axum extractor 패턴), per-route 정책, *모든 기존 verify
  스크립트*가 새 인증 흐름과 호환되도록 갱신.
- **페르소나**: 운영자 (간접 — 모든 운영자에게 필수)
- **사전조건**: 인증 모델 결정. 운영 시나리오(누가 어떤 키를 가지는가) 정의.
- **예상 크기**: 큼 — *별 마일스톤 가치*. 단독 슬라이스로는 부담스러움 (M005.1
  또는 M006 핵심 슬라이스).

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

## 우선순위 (추천 — HARDEN3 출하 후 갱신)

| # | 항목 | 가치 | 크기 | 페르소나 |
|---|------|------|------|---------|
| 1 | AmarilloClient admin 메서드 (S15 후속) | ★★ (M005 완결성 마감) | 작 | 봇 운영자 |
| 2 | 인증 미들웨어 도입 | ★★★ (운영 필수) | 큼 (별 마일스톤) | 운영자 |
| 3 | S11.1 ABI args 디코딩 | ★★ (진단 깊이) | 중-큼 | dApp 개발자 |
| 4 | S12.1 enum 세분화 | ★★ (정밀도) | 큼 | dApp 개발자 |
| 5 | S13.1 패키지 게시 | ★ (운영성) | 작/중 | dApp 개발자 |
| 6 | OS resolver 캐시 race (hickory-dns) | ★ (잔여 SSRF 갭, 첫 요구 후) | 중 | 보안 운영자 |
| 7 | Pools/Traders FE | ☆ (D001 정신) | 작 | 데모 사용자 |

**해석**:
- #1: 가장 빠른 wins — S15에서 시작한 M005 표면을 *examples client에 마감*.
- #2: 인증 — *별 마일스톤 가치*. 단독 슬라이스로는 부담, M006 또는 M005.1
  핵심 슬라이스로.
- #3·#4: dApp 개발자 깊이 — *체감 가치 vs 부담*의 균형.
- #5·#6: 첫 사용자 명시 요청 후 (D017 / HARDEN3 정신).
- #7: 영영 안 해도 무방 (D001).

## 운영 규칙

- 마일스톤 분기(M006 등) 시 본 문서의 #1·#2·#3 그룹을 *시드*로 사용 — 우선순위
  표 + 사전조건 점검 후 마일스톤 ship_definition 작성.
- 단독 단위(#1, #2, #6, #7)는 *언제든* 단독 PR로 들어갈 수 있음. 마일스톤 진행
  중에도 병행 가능.
- 본 문서는 *결정 보조* — 변경된 우선순위/추정/사전조건이 있으면 즉시 갱신.
  ROADMAP의 백로그 줄은 본 문서 링크만 유지.
