use serde::Serialize;
use serde_json::Value;

fn clean(html: &str) -> String {
    ammonia::clean(html)
}

fn clean_field(data: &mut Value, key: &str) {
    let cleaned = match data.get(key).and_then(|v| v.as_str()) {
        Some(s) => clean(s),
        None => return,
    };
    if let Some(slot) = data.get_mut(key) {
        *slot = Value::String(cleaned);
    }
}

fn clean_items(data: &mut Value) {
    let Some(arr) = data.get_mut("items").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for item in arr.iter_mut() {
        if let Some(s) = item.as_str() {
            *item = Value::String(clean(s));
        }
    }
}

pub fn sanitize_editorjs_content(content: &mut Value) {
    let Some(blocks) = content.get_mut("blocks").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for block in blocks.iter_mut() {
        let btype = block
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let Some(data) = block.get_mut("data") else {
            continue;
        };
        match btype.as_str() {
            "paragraph" => clean_field(data, "text"),
            "list" => clean_items(data),
            "raw" => clean_field(data, "html"),
            _ => {}
        }
    }
}

pub fn serialize_sanitized_content<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut cloned = value.clone();
    sanitize_editorjs_content(&mut cloned);
    cloned.serialize(serializer)
}

pub fn serialize_sanitized_content_string<S>(
    value: &String,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match serde_json::from_str::<Value>(value) {
        Ok(mut parsed) => {
            sanitize_editorjs_content(&mut parsed);
            let cleaned = serde_json::to_string(&parsed).unwrap_or_else(|_| value.clone());
            cleaned.serialize(serializer)
        }
        Err(_) => value.serialize(serializer),
    }
}

/// Sitemap (router.rs) interpolates author-controlled slugs raw into XML; without this, a slug like `</loc></url>` injects into /sitemap.xml. No test pins it.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_tag_from_paragraph() {
        let mut c = serde_json::json!({
            "blocks": [
                { "type": "paragraph", "data": { "text": "hi <script>alert(1)</script><b>bold</b>" } }
            ]
        });
        sanitize_editorjs_content(&mut c);
        let text = c["blocks"][0]["data"]["text"].as_str().unwrap();
        assert!(
            !text.contains("<script"),
            "script tag must be stripped, got: {text}"
        );
        assert!(
            text.contains("<b>bold</b>"),
            "benign inline markup must survive, got: {text}"
        );
    }

    #[test]
    fn strips_event_handler_from_list_items() {
        let mut c = serde_json::json!({
            "blocks": [
                { "type": "list", "data": { "items": [
                    "<img src=x onerror=alert(1)>",
                    "<i>safe</i>"
                ] } }
            ]
        });
        sanitize_editorjs_content(&mut c);
        let items = c["blocks"][0]["data"]["items"].as_array().unwrap();
        let first = items[0].as_str().unwrap();
        assert!(
            !first.contains("onerror"),
            "onerror must be dropped, got: {first}"
        );
        let second = items[1].as_str().unwrap();
        assert!(
            second.contains("<i>safe</i>"),
            "benign markup survives: {second}"
        );
    }

    #[test]
    fn strips_javascript_url_from_raw() {
        let mut c = serde_json::json!({
            "blocks": [
                { "type": "raw", "data": { "html": "<a href=\"javascript:alert(1)\">click</a>" } }
            ]
        });
        sanitize_editorjs_content(&mut c);
        let html = c["blocks"][0]["data"]["html"].as_str().unwrap();
        assert!(
            !html.contains("javascript:"),
            "javascript: URL must be dropped, got: {html}"
        );
        assert!(html.contains("click"), "link text survives: {html}");
    }

    #[test]
    fn code_block_is_untouched() {
        let mut c = serde_json::json!({
            "blocks": [
                { "type": "code", "data": { "code": "let x = a < b;\n<script>alert(1)</script>" } }
            ]
        });
        let before = c["blocks"][0]["data"]["code"].as_str().unwrap().to_owned();
        sanitize_editorjs_content(&mut c);
        let after = c["blocks"][0]["data"]["code"].as_str().unwrap();
        assert_eq!(before, after, "code block must be untouched on read");
    }

    #[test]
    fn header_text_is_untouched() {
        let mut c = serde_json::json!({
            "blocks": [
                { "type": "header", "data": { "text": "if a < b then", "level": 2 } }
            ]
        });
        sanitize_editorjs_content(&mut c);
        assert_eq!(
            c["blocks"][0]["data"]["text"].as_str().unwrap(),
            "if a < b then"
        );
    }

    #[test]
    fn unknown_block_types_pass_through() {
        let mut c = serde_json::json!({
            "blocks": [
                { "type": "table", "data": { "content": [["a < b", "c > d"]] } },
                { "type": "checklist", "data": { "items": [{ "text": "x", "checked": false }] } }
            ]
        });
        let before = c.clone();
        sanitize_editorjs_content(&mut c);
        assert_eq!(c, before, "non-sink blocks must be byte-identical");
    }

    #[test]
    fn missing_or_malformed_blocks_is_noop() {
        let cases = [
            serde_json::json!({}),
            serde_json::json!({ "blocks": "not-an-array" }),
            serde_json::json!({ "blocks": [/* empty */] }),
            serde_json::json!({ "blocks": [{ "type": "paragraph" /* no data */ }] }),
        ];
        for mut c in cases {
            sanitize_editorjs_content(&mut c);
        }
    }

    #[test]
    fn serialize_adapter_cleans_without_mutating_source() {
        let value = serde_json::json!({
            "blocks": [
                { "type": "paragraph", "data": { "text": "<script>evil()</script>ok" } }
            ]
        });

        #[derive(serde::Serialize)]
        struct Wrap<'a>(#[serde(serialize_with = "serialize_sanitized_content")] &'a Value);

        let serialized = serde_json::to_string(&Wrap(&value)).unwrap();
        assert!(
            !serialized.contains("<script"),
            "serialized output is clean"
        );
        assert!(serialized.contains("ok"));
        assert!(
            value["blocks"][0]["data"]["text"]
                .as_str()
                .unwrap()
                .contains("<script>"),
            "source value must be unmutated"
        );
    }

    #[test]
    fn string_adapter_cleans_json_string_content() {
        let raw = serde_json::to_string(&serde_json::json!({
            "blocks": [
                { "type": "paragraph", "data": { "text": "<script>alert(1)</script>hi" } },
                { "type": "list", "data": { "items": ["<i>ok</i>", "<b onclick=x>y</b>"] } }
            ]
        }))
        .unwrap();

        #[derive(serde::Serialize)]
        struct Wrap<'a>(#[serde(serialize_with = "serialize_sanitized_content_string")] &'a String);

        let serialized = serde_json::to_string(&Wrap(&raw)).unwrap();
        assert!(!serialized.contains("<script"), "script must be stripped");
        assert!(
            !serialized.contains("onclick"),
            "event handler must be stripped"
        );
        assert!(serialized.contains("hi"));
        assert!(serialized.contains("<i>ok</i>"), "benign markup survives");
        assert!(raw.contains("<script>"));
    }

    #[test]
    fn string_adapter_passes_malformed_through() {
        let raw = String::from("plain text, not json <script>x</script>");
        #[derive(serde::Serialize)]
        struct Wrap<'a>(#[serde(serialize_with = "serialize_sanitized_content_string")] &'a String);
        let serialized = serde_json::to_string(&Wrap(&raw)).unwrap();
        assert!(
            serialized.contains("<script>"),
            "malformed non-JSON content is passed through, not sanitized"
        );
    }
}
