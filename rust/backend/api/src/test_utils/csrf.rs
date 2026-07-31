use crate::middlewares::static_csrf::compute_csrf_token;

pub fn csrf_token_for_session(session_id: &str) -> String {
    compute_csrf_token(session_id)
}

pub fn csrf_header_for_session(session_id: &str) -> (&'static str, String) {
    ("csrf-token", csrf_token_for_session(session_id))
}
