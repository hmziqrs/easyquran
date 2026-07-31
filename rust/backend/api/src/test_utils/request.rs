use axum::{
    body::Body,
    http::{Method, Request},
};
use serde_json::Value;

use super::csrf::csrf_header_for_session;

pub fn json_post(path: &str, body: Value, session_id: &str) -> Request<Body> {
    let (name, value) = csrf_header_for_session(session_id);
    Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", "application/json")
        .header(name, value)
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

pub fn json_get(path: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

pub fn raw_post(path: &str, content_type: &str, body: Vec<u8>, session_id: &str) -> Request<Body> {
    let (name, value) = csrf_header_for_session(session_id);
    Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", content_type)
        .header(name, value)
        .body(Body::from(body))
        .unwrap()
}
