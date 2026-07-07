import { assert } from "chai";

import {
  RpcDebugTraceOutput,
  RpcStructLog,
} from "hardhat/internal/hardhat-network/provider/output";

export function assertEqualTraces(
  actual: RpcDebugTraceOutput,
  expected: RpcDebugTraceOutput
) {
  assert.equal(actual.failed, expected.failed);
  assert.equal(actual.gas, expected.gas);
  assert.equal(actual.returnValue, expected.returnValue);
  assert.equal(actual.structLogs.length, expected.structLogs.length);

  // Eslint complains about not modifying `i`, but we need to modify `expectedLog`.
  // eslint-disable-next-line prefer-const
  for (let [i, expectedLog] of expected.structLogs.entries()) {
    const actualLog = actual.structLogs[i];

    /// Reth returns an empty array for memory, whereas Geth omits it when it's empty
    if (
      actualLog.memory !== undefined &&
      actualLog.memory.length === 0 &&
      expectedLog.memory === undefined
    ) {
      expectedLog.memory = [];
    }

    // revm-inspectors emits `refund: 0` on every step, whereas Geth only emits
    // `refund` when non-zero. Normalize the expected log to match.
    const refund = (actualLog as RpcStructLog & { refund?: number }).refund;
    if (
      refund === 0 &&
      (expectedLog as RpcStructLog & { refund?: number }).refund === undefined
    ) {
      (expectedLog as RpcStructLog & { refund?: number }).refund = 0;
    }

    assert.deepEqual(
      actualLog,
      expectedLog,
      `Different logs at ${i} (pc: ${expectedLog.pc}, opcode: ${expectedLog.op}, gas: ${expectedLog.gas})`
    );
  }
}
