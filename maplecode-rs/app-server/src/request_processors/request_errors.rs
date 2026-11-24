use super::*;
use maplecode_protocol::error::MapleCodeErrorDetails;

pub(super) fn environment_selection_error(err: MapleCodeErr) -> JSONRPCErrorError {
    match err.details() {
        MapleCodeErrorDetails::InvalidRequest(message) => invalid_request(message.clone()),
        _ => internal_error(format!("failed to validate environment selections: {err}")),
    }
}
