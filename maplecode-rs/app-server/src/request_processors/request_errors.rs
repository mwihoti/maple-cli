use super::*;
use maplecode_protocol::error::MaplecodeErrorDetails;

pub(super) fn environment_selection_error(err: MaplecodeErr) -> JSONRPCErrorError {
    match err.details() {
        MaplecodeErrorDetails::InvalidRequest(message) => invalid_request(message.clone()),
        _ => internal_error(format!("failed to validate environment selections: {err}")),
    }
}
