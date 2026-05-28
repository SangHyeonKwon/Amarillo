<div align="center">

# defi-tx-indexer

**Ethereum Failure Intelligence API**

"이 트랜잭션이 *왜* revert 됐는가" — 개별 진단 + 실시간 + 임베드용 API.
Dune이 못 하는 trace-level 영역만 공략한다.

[![Rust](https://img.shields.io/badge/Rust-stable-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Ethereum](https://img.shields.io/badge/Ethereum-Mainnet-3C3C3D?logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Uniswap](https://img.shields.io/badge/Uniswap-V3-FF007A?logo=uniswap&logoColor=white)](https://uniswap.org/)

</div>

---

## What you get

**한 호출에 네 가지 답이 박혀 돌아온다:**

```jsonc
GET /v1/failed-tx/0xdead…0001

{
  "data": {
    "failed": {
      "error_category": "SLIPPAGE_AMOUNT_OUT",   // 10 분류된 카테고리
      "failing_function": "0x414bf389",
      "revert_reason":    "Too little received",
      ...
    },

    "root_cause": {                              // ① 어디서 revert 났나
      "trace_id":   7,
      "call_depth": 2,
      "error":      "Too little received",
      "input":      "0x414bf389...",
      ...
    },

    "failing_function_decoded": {                // ② 어떤 함수인가
      "name":      "exactInputSingle",
      "signature": "exactInputSingle((address,address,uint24,...))",
      "args": [                                  //   typed args까지
        { "type": "(address,address,uint24,address,uint256,uint256,uint256,uint160)",
          "value": [ "0xc02a...", "0xa0b8...", "3000", ... ] }
      ]
    },

    "root_cause_decoded": {                      //   서브콜이 따로 revert면 그것까지
      "name": "approve", "signature": "approve(address,uint256)", "args": [...]
    },

    "diagnosis": {                               // ③ 왜 실패 + 어떻게 고치나
      "message":            "Trade output fell below the minimum amount you specified (buy-side slippage).",
      "recommended_action": "Increase amountOutMin tolerance, or split the trade to lower price impact.",
      "source":             "builtin"
    },

    "call_tree": [ /* … pre-order DFS, trace_id ASC */ ]
  }
}
```

`null`은 항상 **명시적**(silent default 거부). 모든 필드가 *additive* — 클라이언트 무회귀.

## Why this exists

Dune은 SQL 분석의 베이스라인. amarillo는 Dune과 *경쟁하지 않고*, Dune이 *구조적으로
못 하는 trace-level 영역*만 공략한다:

| Dune이 못 하는 것 | amarillo가 박은 것 |
|------------------|------------------|
| `trace.error` per-frame attribution | `root_cause` + `call_tree` (`debug_traceTransaction` 파싱) |
| Consumer-specific ABI decoding | 자기소유 `function_signature` 시드 (17 selectors) + `alloy::dyn_abi` 런타임 디코딩 |
| Per-request webhook delivery | `/v1/alert-subscriptions` outbox 디스패처 + HMAC-SHA256 |
| Real-time failure stream | `--follow --confirmations N` + 동적 reorg scan window |
| Private-data joins | `contract_label.owner_id` — `?owner=` 필터로 분리 |

## Feature matrix

| 능력 | 표면 | 페르소나 |
|------|-----|---------|
| 단건 진단 (root_cause + decoded fn + diagnosis) | `GET /v1/failed-tx/{tx_hash}` | dApp 개발자 |
| 필터링 목록 + 정확한 `total` | `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` | dApp / 데이터팀 |
| 카테고리 × 시간 추이 | `GET /v1/analytics/failed-tx/timeseries?interval=hour\|day\|week` | dApp / SRE |
| 라벨된 컨트랙트별 실패 분포 (`owner_id` 분리) | `GET /v1/analytics/failed-tx/by-label?owner=...` | 봇 운영자 / KYC |
| 봇 라벨 admin (UPSERT / DELETE) | `POST/DELETE /v1/contract-labels` | 봇 운영자 |
| Per-event webhook 구독 (HMAC-SHA256) | `POST /v1/alert-subscriptions` (one-time `signing_secret`) | dApp / 봇 운영자 |
| Rate-threshold 알림 (count ≥ N in window, debounce) | `sub_type=rate_threshold` | 봇 운영자 |
| `signing_secret` 회전 | `POST /v1/alert-subscriptions/{id}/rotate-secret` | 운영자 |
| 실시간 인덱서 (reorg-safe) | `indexer --follow` + 동적 scan window (~`REORG_SCAN_CAP=4096`) | 운영자 |
| ABI args 디코더 (10 변형 + nested tuple) | `failing_function_decoded.args` + `root_cause_decoded` | dApp 개발자 |
| 카테고리 진단 (10 분류) | `error_category` enum + `category_diagnosis` 시드 | dApp 개발자 |
| Admin API key 인증 (write/admin 보호, GET 공개) | `Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}` | 운영자 |
| SSRF 가드 (URL 검증 + DNS-time IP 차단) | `db::validators::webhook_url_is_safe` + `SafeDnsResolver` | 운영자 (간접) |
| Drop-in 클라이언트 (의존 0) | `examples/typescript-client/`, `examples/python-client/` | dApp 개발자 |

## Quick start

**필수**: Rust stable / PostgreSQL 16+ / docker (선택). RPC 키는 *backfill 인덱싱*에만 필요 — 데모 시드만 본다면 불요.

```bash
# 1) env 설정 (AMARILLO_ADMIN_API_KEY는 필수 — 서버가 부팅 거부)
cp .env.example .env
echo "AMARILLO_ADMIN_API_KEY=$(openssl rand -hex 32)" >> .env

# 2) docker compose 한 줄
docker compose up -d
docker compose run --rm seed

# 3) 단건 진단 — 시드된 실패 tx
curl http://localhost:3000/v1/failed-tx/0xdead000000000000000000000000000000000000000000000000000000000001 | jq

# 4) 보호 엔드포인트 (write/admin)
source .env
curl -sX POST http://localhost:3000/v1/contract-labels \
  -H "Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  -H 'Content-Type: application/json' \
  -d '{"address":"0xfeed000000000000000000000000000000000bee","label":"MyArbBot","owner_id":"alice"}' | jq

# 5) 대시보드
open http://localhost:8080
```

> **메인넷 인덱싱**은 `cargo run -p indexer -- --follow --rpc-url <YOUR_RPC>`. Alchemy/Infura free tier로 작은 윈도우 backfill은 가능하지만 24/7 follow는 *paid plan 필요*.

## Live output (실측, docker compose 시드 기준)

위 Quick start를 그대로 따르면 받는 실제 응답·게이트 출력.

### `GET /v1/failed-tx/0xdead…0001`

```json
{
  "failed": {
    "tx_hash":        "0xdead000000000000000000000000000000000000000000000000000000000001",
    "error_category": "Unknown",
    "revert_reason":  null,
    "failing_function": null,
    "gas_used":       45000,
    "timestamp":      "2023-09-01T12:00:00Z"
  },
  "root_cause": {
    "trace_id":   16,
    "call_depth": 0,
    "error":      "Too little received",
    "input":      "0x414bf389"
  },
  "root_cause_decoded": {
    "selector":  "0x414bf389",
    "name":      "exactInputSingle",
    "signature": "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))",
    "source":    "uniswap-v3-router",
    "args":      null
  },
  "failing_function_decoded": null,
  "diagnosis": {
    "message":            "The exact failure mode could not be classified from the trace alone.",
    "recommended_action": "Inspect root_cause and the call_tree; raise an issue with the tx hash.",
    "source":             "builtin"
  }
}
```

> 시드된 tx는 **`root_cause_decoded`로 Uniswap V3 `exactInputSingle`이 자기시드 ABI(D015)에서 즉시 lookup**된다. `args: null`은 선택자만 있고 typed bytes가 없어 디코드 미시도(D027) — 객체 자체는 보존된다는 정신.

### `GET /v1/failed-tx?limit=3` (필터·페이지네이션 + 정확한 `total`)

```json
{
  "data": [
    { "tx_hash": "0xdead00000000...", "error_category": "Unknown", "gas_used": 52000, "timestamp": "2023-09-01T12:00:24Z" },
    { "tx_hash": "0xdead00000000...", "error_category": "Unknown", "gas_used": 38000, "timestamp": "2023-09-01T12:00:12Z" },
    { "tx_hash": "0xdead00000000...", "error_category": "Unknown", "gas_used": 45000, "timestamp": "2023-09-01T12:00:00Z" }
  ],
  "pagination": { "limit": 3, "offset": 0, "count": 3, "total": 3 }
}
```

### `GET /v1/analytics/failed-tx/timeseries?interval=day`

```json
{
  "data": [
    { "bucket": "2023-09-01T00:00:00Z", "error_category": "Unknown", "failure_count": 3 }
  ]
}
```

### `POST /v1/contract-labels` — *no Authorization header*

```
HTTP/1.1 401 Unauthorized
{"error":"unauthorized"}
```

info-leak 방지 단일 응답(D021) — 헤더 누락 / Bearer 형식 오류 / 키 불일치 / 길이 불일치 모두 *같은* 401. 키 존재 여부·길이 어느 것도 응답에 노출 X.

### `verify-failed-tx.sh` 실측

```
GOOD (0xdead...0001): HTTP 200
  PASS
  ORDER OK (pre-order: root first, trace_id strictly ascending)
  ROOT OK (trace_id=16 matches first error frame in call_tree)
  DECODED OK (null — selector absent or not in self-owned ABI seed)
  ROOT_DECODED OK (exactInputSingle :: exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)))
  DIAG OK (msg="The exact failure mode could not be clas…")
BAD  (0x0000…): HTTP 404  PASS
MALFORMED (0xnothex): HTTP 400  PASS
LIST (?category=UNKNOWN&limit=2): HTTP 200  PASS (total=3, returned=2)
LIST (?category=BOGUS): HTTP 400  PASS
LIST (?from=not-a-date): HTTP 400  PASS
TIMESERIES (?interval=day): HTTP 200  PASS (points=1)
TIMESERIES (?interval=bogus): HTTP 400  PASS
ALL PASS
```

### Test gate (단일 호흡 재실행)

```
cargo test -p decoder    31/31  (abi 9 + classifier 10 + events 7 + trace 5)
cargo test -p api        20/20  (config 6 + auth 7 + integration 7)
cargo test -p indexer    36/36  (follow + reorg + worker + dispatcher)
cargo test -p db --lib   17/17  (validators)
cargo test -p db --ign.  27/27  (alerts 3 + alert_rate 3 + category_diagnosis 3
                                 + failed_tx 10 + function_signature 4
                                 + labels 3 + rollback 1)
web test                 41/41  (contract.test 34 + App.smoke 3 + client.test 4)
cargo fmt --check        clean
clippy --all-targets     0 warnings
```

## Screenshots

대시보드 스크린샷은 `docs/screenshots/`(아래 가이드)에 둔다 — 직접 띄워서 캡처:

| 화면 | URL | 추천 캡처 |
|------|-----|----------|
| Overview | http://localhost:8080 | KPI 카드들 + 일일 볼륨 차트 |
| Failed Tx | http://localhost:8080/failed-tx | 카테고리 도넛 + 시계열 + **Tx Inspection** (root_cause + decoded args 패널) |
| Alerts | http://localhost:8080/alerts | **API key input 패널** + 구독 생성 폼 + 목록 (생성/회전/비활성 액션) + 시크릿 1회 모달 |

## Architecture

```
이더리움 노드 (RPC / WebSocket)
  → [indexer]  --follow / --from-block --to-block 워커 풀, depth-aware reorg
  → [decoder]  Uniswap V3 이벤트(Swap/Mint/Burn) + Transfer 디코딩
  → [decoder::trace]   debug_traceTransaction → revert reason + call tree
  → [decoder::classifier]  revert reason → ErrorCategory (10 변형)
  → [decoder::abi]     selector + signature + input → DecodedArg[] (typed)
  → [db]      sqlx UNNEST 배치 INSERT → PostgreSQL
  → [api]     axum REST + admin API key 게이트 (`_: AdminAuth` extractor)
  → [indexer --dispatch-alerts]  outbox → HMAC-signed webhook (SSRF + DNS guard)
```

| Crate | Type | Role |
|-------|------|------|
| `crates/indexer/` | Binary | 블록 수집·오케스트레이션·follow·디스패처 |
| `crates/api/` | Binary + Lib | axum REST 서버 (auth + 보호 라우트 5) |
| `crates/decoder/` | Library | ABI 디코딩·트레이스 파싱·error classifier |
| `crates/db/` | Library | SQLx 모델·쿼리·마이그레이션 |

## Drop-in clients

```bash
examples/typescript-client/   # fetch + node:crypto, 외부 의존 0
examples/python-client/       # urllib + hmac stdlib, 외부 의존 0
```

`AmarilloClient`로 모든 `/v1/*` 호출 + `verifyAlertSignature`(HMAC-SHA256) 포함.
**`npm install` / `pip install` 없음** — `.ts` / `.py` 파일 한 두 개 복사하면 끝
(D017 정신, 게시는 별 슬라이스 S13.1).

End-to-end 5 시나리오 (단건 진단 / 알림 + HMAC / 라벨 분포 / 봇 운영자 playbook /
`/alerts` UI 흐름): [`docs/cookbook.md`](docs/cookbook.md).

전체 API reference + Authentication 정책: [`docs/api-failed-tx.md`](docs/api-failed-tx.md).

## Scope (deliberate)

- **Chain**: Ethereum mainnet
- **Protocol**: Uniswap V3
- **Frozen** (D003): 깊이(실시간 / 진단 / 정합성) 우선, 폭(여러 체인·프로토콜) 미투자
- **Not**: 일반 on-chain 분석 대시보드 (Dune이 압도하는 영역)

## Tech stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (2021 edition, stable, `rust-version = "1.75"`) |
| Async runtime | Tokio (multi-threaded) |
| Ethereum RPC | alloy 1 (`[full]` features) |
| ABI runtime decode | `alloy::dyn_abi` (S11.1) |
| DB driver | sqlx (async, compile-time query validation) |
| Web framework | axum 0.8 + tower-http |
| Crypto / auth | hmac + sha2 + subtle (constant-time eq) |
| Outbound HTTP | reqwest (rustls, default-features off) |
| Resilience | backoff (exponential retry) |
| Database | PostgreSQL 16+ |
| Migrations | sqlx-cli |
| Dashboard | Vite + React 19 + TanStack Query + Recharts |

## Honest limits

- **RPC 비용이 가장 큰 변수**: 메인넷 `--follow` 24/7은 Alchemy free tier(300M CU/월)를 며칠 안에 소진. *작은 윈도우 backfill* 또는 *과거 데이터 시드 + 데모*로 시작 권장.
- **자기소유 ABI 시드**: 17 selectors 큐레이트. 미시드 selector는 `failing_function_decoded: null` — 운영자가 `INSERT INTO function_signature ... ON CONFLICT DO NOTHING`으로 확장(D015 자기시드 정책).
- **API key 단일 값**: env-only. 회전 = env 갱신 + 재시작. 무중단 multi-key 회전은 별 마일스톤.
- **classifier 휴리스틱**: revert reason 문자열 패턴 매칭. 외부 4byte.directory 의존 미도입(D015 정신).
- **`SLIPPAGE_EXCEEDED` / `INSUFFICIENT_BALANCE` 영원한 fallback**: PostgreSQL enum `DROP VALUE` 제약, *추가만* 방향.
- **라이브 메인넷 자동 회귀 부재**: 모든 검증은 docker compose 시드 데이터.
- **봇 운영자 대시보드 UI 미부착**: 봇 운영자는 CLI/스크립트 사용자, dApp 페르소나용 `/failed-tx` + `/alerts` UI는 있음.

## Local dev

```bash
# Prerequisites
cargo install sqlx-cli --no-default-features --features postgres

# Setup
cp .env.example .env
sqlx database create
sqlx migrate run

# Build & test
cargo build --release
cargo test                            # All unit tests
cargo test -p db -- --ignored         # Integration tests (PG required)
cargo clippy --workspace -- -D warnings
cargo fmt --check

# Run indexer (backfill)
cargo run -p indexer -- --from-block 18000000 --to-block 18001000

# Run indexer (follow)
cargo run -p indexer -- --follow --confirmations 12

# Run dispatcher (separate process)
cargo run -p indexer -- --dispatch-alerts

# Run API server (default port 3000)
cargo run -p api

# Verify the API surface
./scripts/verify-failed-tx.sh           # public GETs + diagnosis semantics
./scripts/verify-alerts.sh              # alerts CRUD + HMAC + 401 cases
./scripts/verify-failed-tx-by-label.sh  # by-label + admin endpoints
```

## Status

| Milestone | Status | Summary |
|-----------|--------|---------|
| M001 — Failure Intelligence Core | ✅ | [`.gsd/M001-SUMMARY.md`](.gsd/M001-SUMMARY.md) |
| M002 — Real-time Failure Pipeline | ✅ | [`.gsd/M002-SUMMARY.md`](.gsd/M002-SUMMARY.md) |
| M003 — Actionable Alerts + label join | ✅ | [`.gsd/M003-SUMMARY.md`](.gsd/M003-SUMMARY.md) |
| M004 — Diagnostic Depth | ✅ | [`.gsd/M004-SUMMARY.md`](.gsd/M004-SUMMARY.md) (+ S11.1 args, S12.1 enum 세분화) |
| M005 — Bot Operator Persona | ✅ | [`.gsd/M005-SUMMARY.md`](.gsd/M005-SUMMARY.md) |
| M006 — Operator Auth | ✅ | [`.gsd/M006-SUMMARY.md`](.gsd/M006-SUMMARY.md) |

세 페르소나 완결: **dApp 개발자** (진단 깊이) · **봇 운영자** (rate alerts + 봇 라벨)
· **운영자** (admin auth + 인프라). 진행 흐름·결정·교훈은 [`.gsd/`](./.gsd/) 폴더에 누적.

---

## Key contracts (Ethereum Mainnet)

| Contract | Address |
|----------|---------|
| Uniswap V3 Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` |
| Uniswap V3 Router | `0xE592427A0AEce92De3Edee1F18E0157C05861564` |
