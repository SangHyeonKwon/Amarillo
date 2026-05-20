-- S11 (M004): function_signature 자기소유 ABI 시드.
-- 4-byte function selector → 사람이 읽는 함수명/시그니처 매핑.
-- /v1/failed-tx/{tx_hash} 응답의 failing_function 4-byte를 즉시 식별 가능하도록.
-- 외부 의존(4byte.directory 등) 미도입 — 모든 selector는 자기 시드(D015).
-- args 디코딩은 별 슬라이스(S11.1, sketch).

BEGIN;

CREATE TABLE IF NOT EXISTS function_signature (
    selector   TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    signature  TEXT NOT NULL,
    source     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE function_signature IS
  '4-byte function selector → 함수명/시그니처 매핑 (S11 / M004). 자기소유 ABI 시드만.';
COMMENT ON COLUMN function_signature.selector IS
  '4-byte selector (lowercased hex, 10 chars including 0x prefix)';
COMMENT ON COLUMN function_signature.name IS
  '함수명 (예: transfer, exactInputSingle)';
COMMENT ON COLUMN function_signature.signature IS
  'ABI signature: name(type1,type2,...). 튜플 인자는 ((type,...)) 형태.';
COMMENT ON COLUMN function_signature.source IS
  'Seed origin: erc20 | uniswap-v3-router | uniswap-v3-factory | uniswap-v3-pool | weth9';

-- 자기소유 시드. selector 값은 EIP-20 / Uniswap V3 공식 ABI 기반.
-- ON CONFLICT DO NOTHING으로 멱등 (재실행 안전).
INSERT INTO function_signature (selector, name, signature, source) VALUES
  -- ERC-20 (EIP-20)
  ('0xa9059cbb', 'transfer',     'transfer(address,uint256)',                                                                  'erc20'),
  ('0x095ea7b3', 'approve',      'approve(address,uint256)',                                                                   'erc20'),
  ('0x23b872dd', 'transferFrom', 'transferFrom(address,address,uint256)',                                                      'erc20'),
  ('0x70a08231', 'balanceOf',    'balanceOf(address)',                                                                         'erc20'),
  ('0xdd62ed3e', 'allowance',    'allowance(address,address)',                                                                 'erc20'),
  -- Uniswap V3 SwapRouter (ExactInputSingleParams / ExactOutputSingleParams 튜플 인자)
  ('0x414bf389', 'exactInputSingle',  'exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))',    'uniswap-v3-router'),
  ('0xdb3e2198', 'exactOutputSingle', 'exactOutputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))',   'uniswap-v3-router'),
  ('0xc04b8d59', 'exactInput',        'exactInput((bytes,address,uint256,uint256,uint256))',                                   'uniswap-v3-router'),
  ('0xf28c0498', 'exactOutput',       'exactOutput((bytes,address,uint256,uint256,uint256))',                                  'uniswap-v3-router'),
  ('0xac9650d8', 'multicall',         'multicall(bytes[])',                                                                    'uniswap-v3-router'),
  ('0x5ae401dc', 'multicall',         'multicall(uint256,bytes[])',                                                            'uniswap-v3-router'),
  -- Uniswap V3 Factory
  ('0xa1671295', 'createPool',        'createPool(address,address,uint24)',                                                    'uniswap-v3-factory'),
  -- Uniswap V3 Pool
  ('0x3c8a7d8d', 'mint',              'mint(address,int24,int24,uint128,bytes)',                                               'uniswap-v3-pool'),
  ('0xa34123a7', 'burn',              'burn(int24,int24,uint128)',                                                             'uniswap-v3-pool'),
  ('0x4f1eb3d8', 'collect',           'collect(address,int24,int24,uint128,uint128)',                                          'uniswap-v3-pool'),
  -- WETH9
  ('0xd0e30db0', 'deposit',           'deposit()',                                                                             'weth9'),
  ('0x2e1a7d4d', 'withdraw',          'withdraw(uint256)',                                                                     'weth9')
ON CONFLICT (selector) DO NOTHING;

COMMIT;
