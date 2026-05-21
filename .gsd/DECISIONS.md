# DECISIONS

확정된 방향 결정 (GSD-2: memory-backed). 번복 시 새 항목 추가, 옛 항목 `SUPERSEDED` 표기.

## D001 — 일반 대시보드 포기, 백엔드 재개
- **결정**: Overview/Pools/Top Traders 같은 일반 분석은 더 만들지 않는다. 이전 "백엔드 동결" 지시는
  목적이 '제품'으로 확정되며 **무효**. 엣지는 백엔드에 있으므로 백엔드를 다시 연다.
- **이유**: 일반 분석은 Dune이 압도. 거기 투자할수록 엣지가 깎임 (PROJECT.md).

## D002 — 제품 가설: Transaction Failure Intelligence
- **결정**: 제품의 축을 "실패 트랜잭션 진단/인텔리전스"로 고정.
- **이유**: 차별 자산(`crates/decoder/src/trace.rs` + `classifier.rs`)이 이미 절반 존재. Dune이
  out-of-box로 제공 안 함. 실수요 존재.

## D003 — 스코프 동결: Ethereum + Uniswap V3
- **결정**: 체인/프로토콜 확장 금지. 깊이(실시간·진단·정합성) 우선.
- **이유**: 폭은 Dune이 이김. 좁고 깊게가 유일한 엣지 경로.

## D004 — 콜트리는 우선 "평탄화 프레임"으로 반환
- **결정**: `/v1/failed-tx/{hash}`의 call tree는 `trace_log`에 저장된 평탄 프레임을
  `call_depth` 순서로 반환. 중첩(JSON 트리) 재구성은 후속 `[sketch]`.
- **이유**: `trace_log`가 이미 평탄 저장 (`decoder::trace::flatten_call_frame`). 최소 변경으로
  최대 가치. 중첩은 소비자 요구 확인 후.

## D005 — 페이지네이션에 정확한 total 추가 (신규 엔드포인트 한정)
- **결정**: 신규 실패 목록 API는 `COUNT(*)` 기반 `total`을 응답에 포함. 기존 엔드포인트는
  계약 호환 위해 변경하지 않음.
- **이유**: 임베드형 제품엔 total이 필수. 기존 `PaginatedResponse`는 count(현재 페이지 길이)만
  제공 — 한계를 신규에서만 보완 (web/README의 "explicit total" 후보와 일치).
- **실현 (S02, REALIZED)**: `response.rs`에 `TotalPaginatedResponse<T>` + `PaginationMeta`
  추가, `GET /v1/failed-tx`가 사용. 기존 `PaginatedResponse`/`PaginationInfo` 불변 유지.
  라이브 검증 `total=3, returned=2`(limit=2)로 LIMIT 독립성 확인. → S02-SUMMARY.md

## D006 — `.gsd/`는 계획 문서로만, gsd-2 CLU/DB 미사용
- **결정**: gsd.db/STATE.md 등 런타임 산출물은 만들지 않음. PLAN/ROADMAP/SPEC markdown만 유지.
- **이유**: gsd-2 런타임은 외부 도구. 우리는 방법론·문서 구조만 차용.

## D007 — S01 검증: cargo 통합테스트 → 스크립트+의미단언으로 대체, 정식 테스트 이월
- **결정**: S01-PLAN T01이 명시한 `cargo test -p db -- --ignored` 통합테스트는 S01에서
  구축하지 않는다. 대신 `scripts/verify-failed-tx.sh`(라이브 curl + node 순서 단언)로 수용.
  정식 cargo 통합테스트 하니스(db dev-deps/runtime 구성)는 **독립 1유닛으로 이월** →
  M001-ROADMAP 백로그 `S0x-TEST-HARNESS`로 추적.
- **이유**: 테스트 하니스 구축은 그 자체로 한 유닛(Cargo dev-deps, tokio runtime, 시드 의존).
  S01에 끼워넣으면 "태스크는 컨텍스트 1개" 규칙 위반. 회귀가드는 스크립트 의미단언으로 즉시 확보.
- **리스크**: 스크립트는 CI에서 docker+seed 전제 필요(순수 `cargo test`보다 이식성↓). 이월 유닛에서 해소.
- **연관**: 리뷰 H1(트레이스 정렬 버그)이 이 갭으로 새어나갔음 → 스크립트에 순서 단언 추가로 보강.
- **해소 (STH, RESOLVED)**: `crates/db/tests/failed_tx.rs` 통합테스트 4건 구축
  (`#[ignore]`, `cargo test -p db -- --ignored`). H1 불변식이 실행 가능한 회귀 테스트로 박힘.
  M1 종결. 쉘 스크립트는 HTTP 레벨 상호보완으로 유지. → STH-SUMMARY.md

## D008 — API 레퍼런스는 손작성 우선, OpenAPI 프레임워크 보류
- **결정**: S04의 "임베드 가능화"는 `utoipa` 등 OpenAPI 생성 프레임워크를 도입하지 않고
  손작성 `docs/api-failed-tx.md` 통합 + curl 원커맨드로 충족. 머신리더블 `openapi.json`은
  필요 시 최소 손작성. 프레임워크 도입은 `[sketch]`로 보류.
- **이유**: 프레임워크는 의존성·매크로 침투(전 핸들러 어노테이션)로 그 자체가 한 유닛 이상.
  현재 엔드포인트 4개 규모엔 손작성이 ROI 우위. 규모 커지면 재검토.
- **트레이드오프**: 손작성은 표류 위험 → 검증 스크립트/통합테스트가 실계약을 강제하므로 완화.

## D009 — S05 실시간: poll + confirmations lag, eth_subscribe·깊은 reorg 보류
- **결정**: S05 체인 헤드 팔로워는 ① `eth_subscribe`가 아니라 **polling**(get_block_number),
  ② 헤드가 아니라 **head − N confirmations**까지만 인덱싱(얕은 reorg 노출 최소화).
  `eth_subscribe`는 S07(하드닝), **깊은 reorg 감지·정정은 S06**으로 분리.
- **이유**: polling은 RPC 호환성↑·구현 단순(연속 루프 학습에 집중). confirmations lag는
  S06 전까지의 실용적 reorg 완화. 한 슬라이스 = 한 관심사(GSD-2).
- **트레이드오프**: confirmations만큼 지연 발생(수 초~분) — REQUIREMENTS#M002 "수 초 내"는
  S06/S07 합산으로 충족. 단순/안전 우선.
- **검증 제약**: 실시간 follow는 `RPC_URL` 필요(CI/환경 부재 가능) → 순수 결정 로직
  `next_target()`를 분리해 단위테스트(RPC 불요), 라이브 follow는 수동 스모크·문서.

## D010 — reorg: 해시 저장 + 폴링마다 scan-window fork 체크 (윈도우 내 정확, 초과는 S07)
- **결정**: ① `block`에 `block_hash`/`parent_hash` 저장(첫 마이그레이션, S06-T01).
  ② follow 폴링마다 최근 `max(confirmations,64)` 블록을 체인과 대조(`find_fork_point`)
  → fork면 `rollback_from_block` 후 재인덱싱. ③ 윈도우 **전체가 불일치**하면
  `find_fork_point`는 **floor(윈도우 최저 높이)** 를 반환하고 그 높이부터 롤백한다.
- **정확성 (정정 2026-05-20, 리뷰 R1 — 내 과대서술 자가발견)**: 진짜 fork가
  윈도우 **안**이면 정확. 윈도우보다 **깊은** reorg면 윈도우 전체가 불일치 →
  floor부터만 삭제 → 윈도우 아래의 (이미 무효가 된) 더 오래된 블록이 **그대로
  남고** 그 위에 재인덱싱 → **과소삭제(under-delete) = 잠재적 조용한 오염**. 즉
  floor 롤백은 *over-delete가 아니라 under-delete*이며, 정확성은 **"reorg 깊이 ≤
  scan window"라는 미명시 가정**에 의존한다. 앞 커밋들의 "보수적=과삭제 후
  재인덱싱=안전" 서술은 **정반대로 틀렸음** — R1에서 정정(서술만, 코드 무변경).
- **이유 & 실무 리스크**: 해시 없이는 reorg 비교 불가, 폴링형이라 매 사이클 체크가
  단순. 메인넷 PoS finality ≈ 64블록 + 윈도우 ≥ 64 + confirmations lag → 윈도우
  초과 reorg는 사실상 미발생이라 **실무 리스크는 낮으나 무조건 "안전"은 아님**.
- **트레이드오프 & 후속**: 매 폴링 N RPC(S07 R2에서 tip→lazy 조회로 개선). 윈도우
  초과 reorg의 **완전 해소 = S07-T03**(윈도우를 cap까지 동적 확대해 진짜 최소
  공통조상까지 하강). **안전 규칙(불변)**: 체인 해시 불확실 시 절대 롤백 안 함.
- **REALIZED (2026-05-20, S07-T03)**: lazy + 동적 확대 구현. `detect_fork`가
  tip부터 on-demand 조회(정상=1 RPC → R2 해소)하고 윈도우 전부 불일치면
  `next_scan_depth`로 ×4 확대해 `REORG_SCAN_CAP`(4096) 내 **진짜 최소 공통조상**
  까지 롤백 → **under-delete 갭 제거(R1 해소)**. 잔여: 4096블록 초과 reorg만
  best-effort floor(≈finality 64배 — 사실상 불가, 명시된 경계). 순수
  `classify_fork`/`next_scan_depth` 단위테스트(8), 라이브 수동.

## D011 — 실시간 트리거: 폴링 기본, eth_subscribe 옵트인 (S07-T02)
- **결정**: follow 사이클 트리거는 **폴링이 기본**(`sleep(poll)`). `--subscribe` +
  `WS_URL` **동시** 지정 시에만 newHeads `eth_subscribe`로 트리거를 대체한다. WS
  미지정/연결 실패/스트림 종료 → **폴링으로 폴백**(무회귀). reorg 체크·`next_target`
  ·`index_range`는 트리거와 무관하게 그대로 재사용(트리거만 교체).
- **이유**: WS 엔드포인트 가용성은 환경별 편차가 큼(공개 RPC는 WS 미제공/제한
  흔함). 폴링은 호환성·단순성의 안전 기본값. 구독은 지연 단축(폴 간격에 안 묶임)
  이지만 **옵트인**이어야 운영 리스크가 낮고, 폴백 일원화로 "구독 실패 시 멈춤"
  회귀를 차단한다. (`Config.ws_url`의 기존 `#[allow(dead_code)]`를 여기서 해소.)
- **검증 제약(D009/D010과 동일)**: 라이브 WS는 `WS_URL` 필요(환경 미보장) → 순수
  `resolve_trigger_mode(subscribe, ws_url)→모드`만 단위테스트, 실제 구독·폴백은
  컴파일+clippy+수동 스모크·문서.

## D012 — M003 알림: outbox 디스패처 + SSRF/서명, indexer 서브모드 (S08)
- **결정**: ① 알림 전송은 **outbox/디스패처** 패턴 — follow 루프에 인라인 금지.
  실패 격리: 느리거나 깨진 webhook이 인덱싱·reorg 정정을 막아선 안 됨. 신규 실패는
  이미 DB에 적재(M002) → 디스패처가 매칭·미전송분을 스캔해 전송. ② 디스패처는
  신규 크레이트 아닌 **`indexer` 바이너리 서브모드**(`--dispatch-alerts`) — follow의
  순수+드라이버/graceful 패턴 재사용("재작성 금지·추가로 빌드", PROJECT 스코프).
  ③ `webhook_url`은 신뢰불가 입력 → **SSRF 가드**(https-only, loopback/RFC1918/
  link-local/메타데이터 거부, 리다이렉트 비추적) + per-sub **HMAC-SHA256 서명**
  (시크릿 DB 저장·**로그 미출력**). ④ 신규 의존성 `reqwest`(rustls, 최소 피처)
  1개 수용 — 아웃바운드 HTTP는 본질적 신규 능력(S07 T01/T02 무의존과 별개,
  정직 표기). ⑤ MVP = **건별 정확매칭**(category/to_addr) 전송; 임계·율(급증)
  집계는 백로그.
- **이유**: 실패 격리·멱등(`alert_delivery` anti-join)·재시작 안전·테스트성
  (순수 매칭/가드/서명 + 얇은 비동기 드라이버 = D009 철학 일관). SSRF/서명은
  외부로 나가는 신뢰불가 URL에 대한 필수 안전장치.
- **검증 제약(D009~D011과 동일)**: 라이브 전송은 수신 엔드포인트 필요(CI 미보장)
  → 순수 SSRF 가드·HMAC 서명·매칭 술어 단위테스트 + 매칭 쿼리 통합테스트(PG)가
  1차 증빙, 실제 POST·재시도는 컴파일+clippy+수동 스모크·문서.
- **REALIZED & DEVIATION (2026-05-20, S08-T02/T03, 정직 표기)**: ④에서 "신규
  의존성 `reqwest` 1개 수용"이라 했으나 S08 전체 실제 추가는 **5개**:
  - `reqwest`(0.12, rustls, default-features=off) — 아웃바운드 HTTP (T02)
  - `hmac`(0.12) + `sha2`(0.10) — HMAC-SHA256 본문 서명 (T02, RustCrypto)
  - `url`(2) — SSRF 가드의 URL 파서, db 크레이트에 둠(api/indexer 공유, T03)
  - `getrandom`(0.2) — signing_secret CSPRNG (T03, api 전용)

  모두 작고 `no_std`-friendly, 시스템 의존 없음(rustls-tls). HMAC/SHA/CSPRNG/HTTP
  를 손수 구현하는 건 crypto·security 영역에서 anti-pattern이라 도입 1개로는 실현
  불가했음. 본 줄로 계획 문구를 사실로 정정한다(D012 결정 자체는 변경 없음).

## D013 — S09 유스케이스: 컨트랙트 라벨 (M003 출하 게이트)
- **결정**: "온체인 × 비공개 데이터 조인 예시 1건"(REQUIREMENTS#M003)을
  **컨트랙트 라벨** 1건으로 시연한다. 신규 테이블 `contract_label(address, label,
  owner_id?, created_at)` + 시드(Uniswap V3 router/factory + 기존 `pool` 테이블
  자동 매핑) + 신규 분석 엔드포인트 `GET /v1/analytics/failed-tx/by-label`이
  실패 인텔리전스(`failed_transaction` × `transaction`) × `contract_label`을
  JOIN해 "라벨된 컨트랙트별 실패 분포"를 노출한다.
- **이유**: 세 후보(컨트랙트 라벨 / 봇 운영자의 자기 봇 주소 / 거래소-CS의 KYC
  tx 매핑) 중 ① 가장 *공개 시연 가능*(KYC는 시연용 시드조차 부담), ② 기존
  `pool`/`token` 시드 라벨과 자연 결합(시연 데이터 부담 최소), ③ "개발자가 사용
  하도록"이라는 직전 사용자 가이드와 정합(자기 dApp 컨트랙트별 실패 분포가
  자연스러운 사용). 봇/KYC는 같은 패턴이라 추후 라벨 종류 확장으로 가능 — 본
  슬라이스 스코프 밖.
- **트레이드오프**: 라벨은 컨트랙트 주소 → 사람이 읽는 이름의 단순 매핑. 더
  강한 해자(봇 자기-봇 식별·거래소 KYC)는 *같은 인프라*에 라벨 종류만 다르게
  추가하면 됨 — 본 슬라이스가 그 패턴을 한 번 박아 둠. 멀티-테넌시(`owner_id`)는
  스키마에만 박고 인증 미연결(인증 도입은 별 단위 — D008 정신 일관 "프레임워크
  보류").
- **검증 제약(D009~D012 일관)**: T01 통합테스트(PG 시드+쿼리), T02 verify 스크립트,
  T03 `web` typecheck+vitest+build. 라이브 시각 회귀·실제 컨트랙트 라벨 시드 다양화는
  수동·운영 측 책임.

## D014 — M004 방향: 진단 깊이, dApp 개발자 페르소나
- **결정**: M004 = 단건 진단 응답이 *누적적으로* 똑똑해지는 묶음. 새 분석
  엔드포인트 대신 기존 `/v1/failed-tx/{tx_hash}`에 가산. 첫 슬라이스 **S10 =
  콜트리 루트코즈 어트리뷰션**(`trace_log.error` 활용).
- **이유**: M001~M003이 인프라(데이터·실시간·알림·라벨조인)를 박았으니 다음 호흡은
  *진단 자체의 품질*. dApp 개발자의 1차 잡은 "내 tx가 왜 실패했나" 단건. 차별
  자산(`decoder::trace` + `classifier`)이 이미 절반 존재(D002)하는 영역에서 깊이를
  쌓는 것이 가장 자연스러운 ROI. 사용자 의도: "C 끝내고 개발자 실사용 프로덕트로
  발전 + DeFi 백엔드 인프라 영역이 좁다" → 깊이로 차별화.
- **탈락 후보 메모**:
  - A 임계율 집계 알림(D012 MVP 제외분) — 봇 운영자 페르소나로 선회. M005 후보로 보존.
  - B 라벨 종류 확장(봇/KYC) — S09의 *양적* 확장이라 새 깊이 없음. 백로그.
  - D 운영 성숙(DNS-rebind SSRF / 인증 도입) — 보안·운영 잔여. 단독 PR 단위 백로그.
- **스코프**: D003 동결 유지(Ethereum + Uniswap V3). 함수 디코딩(S11)은 자기소유
  ABI 시드만, 4byte.directory 같은 외부 데이터 의존 미도입(D008 정신).
- **검증 제약(D009~D013 일관)**: 통합 PG(`-p db --ignored`) + verify HTTP +
  clippy/fmt + web typecheck/test/build. 라이브 메인넷 tx 자동 회귀는 불가능 —
  자기 시드로 의미 단언, 메인넷은 수동·운영 측.

## D015 — S11 스코프: selector → name + signature(까지), args 디코딩은 별 슬라이스
- **결정**: S11(`failing_function` selector → 함수명 디코딩)의 1차 스코프는
  **selector → name + signature 매핑까지**. ABI args 디코딩(typed value 추출)은
  같은 슬라이스에 포함하지 않고 별 슬라이스(`S11.1` sketch) 또는 S12와 묶음으로
  보류. `function_signature(selector PK, name, signature, source?)` 자기소유
  시드 + `FailedTxDetail.failing_function_decoded: Option<DecodedFunction>`
  1필드 가산.
- **이유**: args 디코딩은 그 자체로 한 유닛 이상 — ABI 타입 시스템(address/
  uint256/bytes/tuple/dynamic 등) + 입력 bytes → typed value의 ABI decoder
  호출 + 중첩 처리. *이름*만 알아도 dApp 개발자에겐 "내 트랜잭션의 `transfer`
  가 실패했다"가 즉시 보이는 큰 가치. 슬라이스는 컨텍스트 1개(GSD-2).
  selector↔name이 1단계, args는 2단계.
- **스코프**: D003 동결 유지. 시드는 ERC20 5종(transfer/approve/transferFrom/
  balanceOf/allowance) + Uniswap V3 SwapRouter 핵심 4(exactInputSingle/
  exactOutputSingle/exactInput/exactOutput) + Factory createPool + WETH9
  deposit/withdraw + Pool mint/burn/collect 등 10여 selector. 외부 의존
  미도입(D008/D014 정신; 4byte.directory 등 보류).
- **트레이드오프**: `root_cause.input`의 selector도 같은 디코드 대상이지만
  본 슬라이스에선 `failing_function_decoded` 1필드만 — root_cause input
  디코드 가산은 후속 슬라이스(S11.1) 호흡. 응답에 또 하나 필드 추가 = 별
  슬라이스 단위(GSD-2 일관).
- **검증 제약(D009~D014 일관)**: 통합테스트 + verify HTTP + clippy/fmt + web.
  실측: `failing_function`이 이미 인덱서 루트 frame input 첫 4바이트로 채워짐
  (`crates/indexer/src/worker.rs:597`). 그 selector를 lookup해 가산만 하면 됨.
  라이브 메인넷 자동 회귀 부재 — 자기 시드 selector로 의미 단언.

## D016 — S12 스코프: 진단 메시지/추천 액션(까지), 카테고리 세분화는 별 슬라이스
- **결정**: S12(카테고리 진단 깊이)의 1차 스코프는 **카테고리별 진단 메시지 +
  추천 액션 시드 + 응답 가산**까지. 기존 `error_category` enum 값(`UNKNOWN` /
  `SLIPPAGE_EXCEEDED` 등 6개)은 그대로 두고 *진단 텍스트와 추천 액션*만 추가.
  enum 자체의 세분화(예: `SLIPPAGE_EXCEEDED` → `SLIPPAGE_PRICE_IMPACT` /
  `SLIPPAGE_AMOUNT_OUT`)는 별 슬라이스(`S12.1` sketch).
- **이유**: enum 세분화는 마이그레이션(`ALTER TYPE`) + classifier 룰 확장 +
  Rust enum 변형(non-exhaustive 이슈) + 프론트 type union + 시드 데이터 분류
  영향으로 *그 자체로 한 유닛 이상*. 진단 메시지/추천 액션은 기존 카테고리
  6개에 대해 시드 1행씩 + 응답 1필드 가산 — *즉시 dApp 개발자에게 액션 가능*.
  D014 일관: 단건 응답이 누적적으로 똑똑해짐 (S10 어디서 → S11 어떤 함수 →
  S12 왜+어떻게).
- **스코프**: D003 동결 유지. 시드는 자기소유(`category_diagnosis` 멱등 + 6
  카테고리 1행씩). 외부 의존 미도입(D008/D015 정신). `category_diagnosis
  (error_category PK TEXT, message, recommended_action?, source?, created_at)`
  TEXT PK로 단순화 — Postgres enum 컬럼 회피해 마이그레이션 영향 최소화.
- **트레이드오프**: 카테고리가 6개로 "조잡"하다는 한계가 남음(예: 모든
  `UNAUTHORIZED` 케이스가 같은 메시지). enum 세분화로 정밀도 ↑는 후속
  슬라이스(`S12.1`) 호흡 — 본 슬라이스는 *추천 액션의 기본선*을 박는다.
- **검증 제약(D009~D015 일관)**: 통합테스트 (시드 6행 lookup) + verify HTTP
  (`diagnosis` 필드 존재 + null|object + 시드된 카테고리에 대한 의미 단언) +
  clippy/fmt + web typecheck/test/build. 라이브 메인넷 자동 회귀 부재 — 자기
  시드 카테고리로 의미 단언.

## D017 — S13 스코프: 예시 클라이언트(TS+Python) + cookbook까지, 패키지 게시는 별 단위
- **결정**: S13(개발자 SDK/문서)의 1차 스코프는 **TypeScript + Python 미니멈
  *예시 클라이언트* 코드 + cookbook 문서**까지. 두 언어 모두 외부 런타임 의존 0
  (TS는 `fetch`, Python은 `urllib.request`). `examples/typescript-client/` +
  `examples/python-client/` 자기-완결 1~2 파일. npm / PyPI 게시는 본 슬라이스
  스코프 밖 — 별 단위(`S13.1` sketch). `package.json` / `pyproject.toml`도 도입
  안 함(빌드/배포 인프라 그 자체로 한 유닛 이상; D008 정신).
- **이유**: dApp 개발자 페르소나(D014)에게 *즉시 카피해 쓰는* 작동 예시가 가장
  큰 가치 — npm install 절차 없이 1 파일을 자기 프로젝트에 붙이면 동작. 우리
  API의 응답 계약(S10/S11/S12 가산 누적)이 *복사 가능한 코드*로 시연되면 그
  자체가 가장 좋은 문서. 게시 단계의 무게(semver / 게시 토큰 / CI / 종속 그래프
  관리)는 첫 사용자가 실제로 npm 게시를 요청하기 전에는 *낭비*.
- **스코프**: D003 동결 유지. cookbook 문서 `docs/cookbook.md` 하나에 3 시나리오:
  (1) 단건 진단 (`/v1/failed-tx/{tx_hash}` 응답에서 root_cause / decoded /
  diagnosis 활용), (2) 알림 구독 (`POST /v1/alert-subscriptions` + HMAC 검증),
  (3) 라벨된 컨트랙트 실패 분포 (`/v1/analytics/failed-tx/by-label`). 각 시나리오
  마다 curl + TS + Python 3중 예시.
- **트레이드오프**: 예시 코드는 사용자가 직접 카피해야 — "그냥 `npm install ...`
  되면 좋겠다"는 욕구는 S13.1로 미룬다. 단, 코드가 외부 의존 0이라서 *카피
  자체로 작동*한다(예시 코드 = SDK = 동일).
- **검증 제약(D009~D016 일관)**: clippy/fmt 무회귀 + web typecheck/test/build
  무회귀 (예시 코드는 web 빌드 안 들어감 — 별 디렉토리). 예시 코드는 *컴파일
  검증*까지(TS는 `tsc --noEmit`로 typecheck, Python은 `python -m py_compile`로
  syntax check). 라이브 호출은 docker compose + verify 스크립트와 동일 환경
  요구라 본 슬라이스에서는 syntax/type 검증까지 + 수동 스모크 절차 문서화.

## D018 — M005 방향: 봇 운영자 페르소나 첫 진입, 임계율 집계 알림
- **결정**: M005 = **봇 운영자 페르소나 첫 진입**. 핵심 가치는 *시간 윈도우
  임계율 알림* (BACKLOG #1, D012 MVP 제외분). 기존 `alert_subscription`에
  `sub_type` 컬럼 + `threshold_count` / `threshold_window_secs` / `debounce_secs`
  컬럼 가산. 첫 슬라이스 **S14**가 핵심 메커니즘. S15(봇 라벨)·S16(cookbook
  봇 시나리오)는 후속(`[sketch]`).
- **이유**: M001~M004가 dApp 개발자 페르소나(진단/SDK)에 집중했으므로, 다음
  호흡은 *새 페르소나 확장*. 봇 운영자는 "내 봇 망가졌어?"가 1차 잡 — 건별
  알림은 *노이즈* (정상 봇도 간헐적 실패함), 임계율 알림은 *시그널* (급증=망가
  진 신호). D012(M003)의 MVP 제외분이 본 마일스톤에서 실현.
- **스코프**: D003 동결 유지(Ethereum + Uniswap V3). 임계 표면은 **기존 테이블
  가산**으로 단순화 — 새 테이블 분리는 작업량 큼(별도 검증·인덱싱·매칭 룰
  복잡). 컬럼 가산 + `sub_type` 명시(silent default 금지 정신 일관, 기본값
  `'per_event'`로 backwards compat). 디바운스는 *시간 기반*만 — 카운트 기반은
  별 단위.
- **트레이드오프**:
  - rate 모드는 *시간 윈도우 내 count* 만 비교. 비율(예: tx_count 대비
    실패율)이나 추세(예: 이전 시간 대비 증가율) 계산은 별 슬라이스(S14.1
    sketch). 본 마일스톤은 *절대 임계* 만으로도 봇 운영자 1차 잡 충족함.
  - rate sub의 디바운스는 *마지막 발송 시각 + debounce_secs*가 윈도우. 같은
    sub의 *서로 다른* 카테고리/주소는 별 매칭이지만 디바운스는 sub_id 단위.
  - 기존 per-event sub은 *완전 호환* — sub_type='per_event' default + dispatcher
    분기로 동일 동작 보장.
- **검증 제약(D009~D017 일관)**: 통합 PG(매칭/디바운스 쿼리) + verify HTTP(rate
  sub 생성 + 발송 시뮬레이션) + clippy/fmt + web typecheck/test/build. 라이브
  임계율 시뮬은 시드 데이터의 실패 tx 시간 분포 기반 — 자동 검증 가능.

## D019 — S15 스코프: 봇 라벨 admin API + cookbook 봇 시나리오 (M005 마감)
- **결정**: S15 = M005 마감 슬라이스. **봇 라벨 admin API**(`POST /v1/contract-
  labels` + `DELETE /v1/contract-labels/{address}`) + **cookbook 봇 운영자 시나리오**
  (S16 흡수). 별도 `bot_label` 테이블 신설 *없이* 기존 `contract_label` 인프라
  재사용(S09) — owner_id 필터로 자기 봇 분리. 인증 미부착(D008 정신 일관 — 인증
  도입은 별 단위 이상).
- **이유**: M005 첫 슬라이스 S14가 rate 알림 메커니즘을 박았지만, 봇 운영자가
  *자기 봇 라벨을 동적으로 등록*하는 표면이 없으면 라벨이 *시드 데이터로만*
  존재. 진짜 *프로덕트* 흐름(라벨 등록 → rate sub 생성 → 알림 → by-label로
  자기 봇 실패 분포)이 코드로 박혀야 봇 운영자 페르소나가 완결. cookbook
  시나리오 4는 S13 패턴 일관(S08+S09+M004 통합 시나리오 → 본 슬라이스에서
  S14+S15+M005 봇 시나리오 추가).
- **스코프**:
  - 신규 테이블 X — `contract_label`(S09) 재사용. `insert_contract_label`/
    `delete_contract_label` 쿼리도 이미 존재(S09-T01). API 핸들러만 추가.
  - 인증 X — 데모 스코프 명시(D008/D013 정신). 운영 배포 시 별 단위.
  - 봇 라벨 *시드* 추가 X — 운영자가 admin API로 동적 등록 (또는 시드 SQL).
  - 프론트 폼 추가 X — 봇 운영자는 CLI/스크립트 사용자, dApp 개발자용 UI(FailedTx
    페이지의 by-label 카드)는 이미 있음.
- **트레이드오프**:
  - 인증 미부착 = 데모만, 운영 위험. 명시 + S15.1 (선택) 인증 도입은 별 슬라이스.
  - bot_label 별 테이블 분리하지 않음 — *모든 라벨이 같은 테이블*. 봇/컨트랙트
    구분은 owner_id로(공개 라벨 = NULL, 봇 운영자 = 자기 ID).
- **검증 제약(D009~D018 일관)**: 통합 PG(insert/delete 라운드트립) + verify HTTP
  (POST 201 / DELETE 204·404 / 잘못된 주소 400) + cookbook 4 시나리오 자동
  검증 어려움 → 라이브 호출은 docker compose + 수동 스모크(README/cookbook 명시).

## D020 — DNS-time SSRF: reqwest dns_resolver hook + ip_is_safe 공유
- **결정**: BACKLOG #1 — DNS rebinding 공격 차단. dispatcher의 reqwest client에
  custom `dns_resolver`를 주입해 *resolved IP*가 unsafe면 connect 전에 실패시킴.
  기존 `webhook_url_is_safe`의 IP 검증 로직을 `ip_is_safe(ip: IpAddr) -> Result<(),
  UnsafeUrlReason>` public 함수로 분리해 *두 검증 단계가 같은 정책*을 공유.
- **이유**: `webhook_url_is_safe`는 *URL 파싱 시점* IP 검증만 — host가
  hostname이면 통과. 공격자가 `attacker.com`을 처음엔 공개 IP로 응답 후
  dispatcher가 connect 직전 resolve 다시 하면 사설 IP(`127.0.0.1`)로 rebind →
  내부 서비스 SSRF. *resolved IP* 검증으로 차단. S08-T02 코멘트의 "잔여 리스크
  (정직)" 항목을 닫는다.
- **스코프**: 신규 의존 0 — stdlib `to_socket_addrs`(blocking)를
  `tokio::task::spawn_blocking`으로 호출 + `reqwest::dns::Resolve` trait 구현.
  hickory-dns 같은 async resolver lib 미도입 (D008/D013/D015 정신 일관).
  의도적 정책 단순화 — OS resolver 신뢰 + 결과 IP만 검증.
- **트레이드오프**:
  - blocking resolver는 thread-pool 부담 — dispatcher의 `MAX_CONCURRENT_POSTS=10`
    범위에선 무문제.
  - hickory-dns(async)는 더 빠르지만 의존 1개 + 학습 곡선. 첫 사용자 부하 요구
    없으면 *낭비*.
  - DNS *response 캐싱* race는 *우리 코드 밖* (커널 stub resolver / nscd) —
    OS resolver 신뢰 한도. 완전 차단은 hickory-dns의 직접 UDP resolution까지
    가야 가능 (별 단위, BACKLOG로 잠재 이월).
- **검증 제약(D009~D019 일관)**: 단위테스트(`SafeDnsResolver`가 unsafe IP를
  `Err`로 반환) + 기존 `webhook_url_is_safe` 단위테스트 무회귀 + verify-alerts.sh
  무회귀. 라이브 DNS rebinding 시뮬은 mock DNS server 필요 — 환경 부담 큼,
  수동 스모크(공격 시뮬 도구 별도)로 위임.
