-- S12.1 (M004): ErrorCategory enum 세분화 v2 — SLIPPAGE 3 신규 + INSUFFICIENT_ALLOWANCE.
-- D028: fallback 유지 (SLIPPAGE_EXCEEDED / INSUFFICIENT_BALANCE는 그대로) + 4 신규
--       세부 카테고리 추가. 기존 데이터 reclassify 안 함 (backward compat).
-- D029: ALTER TYPE ... ADD VALUE IF NOT EXISTS는 PostgreSQL 9.6+ 트랜잭션 안전 +
--       16+에서 같은 트랜잭션 내 즉시 사용 가능. category_diagnosis.error_category가
--       TEXT라 enum 값 사용 없이 INSERT 안전.
-- 멱등: 4 ADD VALUE는 IF NOT EXISTS, 4 INSERT는 ON CONFLICT DO NOTHING — 재실행 안전.

BEGIN;

-- 1) ErrorCategory enum에 4 신규 변형 추가.
ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'SLIPPAGE_AMOUNT_OUT';
ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'SLIPPAGE_AMOUNT_IN';
ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'SLIPPAGE_PRICE_IMPACT';
ALTER TYPE error_category ADD VALUE IF NOT EXISTS 'INSUFFICIENT_ALLOWANCE';

-- 2) category_diagnosis 시드 4 행 — dApp 개발자가 받는 진단 메시지 + 추천 액션.
-- 세부 카테고리는 generic (SLIPPAGE_EXCEEDED / INSUFFICIENT_BALANCE)의 *더 정확한*
-- 메시지 — 동일 사고의 reasonable 분기 후 클라이언트가 *맞춤형 액션*을 받게.
INSERT INTO category_diagnosis (error_category, message, recommended_action, source) VALUES
  ('SLIPPAGE_AMOUNT_OUT',
   'Trade output fell below the minimum amount you specified (buy-side slippage).',
   'Increase amountOutMin tolerance, or split the trade to lower price impact.',
   'builtin'),
  ('SLIPPAGE_AMOUNT_IN',
   'Trade required more input than the maximum you specified (sell-side slippage).',
   'Increase amountInMax tolerance, or split the trade to lower price impact.',
   'builtin'),
  ('SLIPPAGE_PRICE_IMPACT',
   'Pool price moved past the allowed limit during execution.',
   'Widen sqrtPriceLimitX96 (or remove the limit) and consider splitting the trade.',
   'builtin'),
  ('INSUFFICIENT_ALLOWANCE',
   'The spender lacks ERC-20 allowance for this token transfer.',
   'Call approve(spender, amount) on the token before the trade or rerun.',
   'builtin')
ON CONFLICT (error_category) DO NOTHING;

COMMIT;
