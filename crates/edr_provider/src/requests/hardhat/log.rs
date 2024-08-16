use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub fn handle_set_logging_enabled_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<TimerT>,
    is_enabled: bool,
) -> Result<bool, ProviderError> {
    data.logger_mut().set_is_enabled(is_enabled);
    Ok(true)
}
