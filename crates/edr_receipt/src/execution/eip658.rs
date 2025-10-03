use edr_primitives::Bloom;

use super::Eip658;
use crate::{ExecutionReceipt, MapReceiptLogs, RootOrStatus};

impl<LogT, NewLogT> MapReceiptLogs<LogT, NewLogT, Eip658<NewLogT>> for Eip658<LogT> {
    fn map_logs(self, map_fn: impl FnMut(LogT) -> NewLogT) -> Eip658<NewLogT> {
        Eip658 {
            status: self.status,
            cumulative_gas_used: self.cumulative_gas_used,
            logs_bloom: self.logs_bloom,
            logs: self.logs.into_iter().map(map_fn).collect(),
        }
    }
}

impl<LogT> ExecutionReceipt for Eip658<LogT> {
    type Log = LogT;

    fn cumulative_gas_used(&self) -> u64 {
        self.cumulative_gas_used
    }

    fn logs_bloom(&self) -> &Bloom {
        &self.logs_bloom
    }

    fn transaction_logs(&self) -> &[Self::Log] {
        &self.logs
    }

    fn root_or_status(&self) -> RootOrStatus<'_> {
        RootOrStatus::Status(self.status)
    }
}
