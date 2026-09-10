// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, ETAG};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::Response;

pub struct EmbeddedAsset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub etag: &'static str,
    pub immutable: bool,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

pub async fn serve(uri: Uri) -> Response {
    let requested = uri.path();
    if requested.starts_with("/api/")
        || requested.starts_with("/health/")
        || requested.starts_with("/public/")
    {
        return not_found();
    }
    let path = if requested == "/" {
        "/index.html"
    } else {
        requested
    };
    let asset = find(path).or_else(|| {
        if PathKind::from_request(requested) == PathKind::EditorRoute {
            find("/index.html")
        } else {
            None
        }
    });
    let Some(asset) = asset else {
        return not_found();
    };

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, asset.content_type)
        .header(ETAG, asset.etag)
        .header(
            CACHE_CONTROL,
            if asset.immutable {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            },
        );
    if asset.path == "/index.html" {
        builder = builder.header(
            CONTENT_SECURITY_POLICY,
            "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        );
    }
    builder
        .body(Body::from(asset.bytes))
        .expect("static response headers are valid")
}

fn find(path: &str) -> Option<&'static EmbeddedAsset> {
    EMBEDDED_ASSETS.iter().find(|asset| asset.path == path)
}

fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        )
        .header(CACHE_CONTROL, HeaderValue::from_static("no-store"))
        .body(Body::from(
            r#"{"type":"about:blank","title":"Not Found","status":404}"#,
        ))
        .expect("problem response headers are valid")
}

#[derive(PartialEq, Eq)]
enum PathKind {
    EditorRoute,
    Asset,
}

impl PathKind {
    fn from_request(path: &str) -> Self {
        if path
            .rsplit('/')
            .next()
            .is_some_and(|part| part.contains('.'))
        {
            Self::Asset
        } else {
            Self::EditorRoute
        }
    }
}
