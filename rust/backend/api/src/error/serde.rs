use crate::error::{ErrorCode, ErrorResponse, IntoErrorResponse};

impl IntoErrorResponse for serde_json::Error {
    fn into_error_response(self) -> ErrorResponse {
        if self.is_syntax() || self.is_eof() || self.column() != 0 {
            ErrorResponse::new(ErrorCode::InvalidFormat)
                .with_message("JSON deserialization error")
                .with_details(self.to_string())
        } else {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("JSON serialization error")
                .with_details(self.to_string())
        }
    }
}

impl From<serde_json::Error> for ErrorResponse {
    fn from(err: serde_json::Error) -> Self {
        err.into_error_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialization_syntax_error_maps_to_invalid_format_400() {
        let err: serde_json::Error = serde_json::from_str::<serde_json::Value>("{ bad }")
            .expect_err("malformed JSON must fail to parse");
        let resp: ErrorResponse = err.into();
        assert_eq!(resp.code, ErrorCode::InvalidFormat);
        assert_eq!(resp.status, 400u16);
    }

    #[test]
    fn eof_during_parse_maps_to_invalid_format_400() {
        let err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("{").expect_err("truncated JSON must fail");
        let resp: ErrorResponse = err.into();
        assert_eq!(resp.code, ErrorCode::InvalidFormat);
        assert_eq!(resp.status, 400u16);
    }

    #[test]
    fn io_failure_maps_to_internal_server_error_500() {
        use std::io::{self, Write};
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("write failed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let err = serde_json::to_writer(FailingWriter, &serde_json::json!({"x": 1}))
            .expect_err("failing writer must produce an error");
        let resp: ErrorResponse = err.into();
        assert_eq!(resp.code, ErrorCode::InternalServerError);
        assert_eq!(resp.status, 500u16);
    }
}
