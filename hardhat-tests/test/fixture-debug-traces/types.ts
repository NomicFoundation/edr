import {
  RpcDebugTraceOutput,
  RpcStructLog,
} from "hardhat/internal/hardhat-network/provider/output";

export type GethTrace = Omit<RpcDebugTraceOutput, "structLogs"> & {
  structLogs: Array<Omit<RpcStructLog, "memSize">>;
};

export type TurboGethTrace = GethTrace;
