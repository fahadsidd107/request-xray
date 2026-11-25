use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};

use crate::LocalView;

#[cfg(xray_dashboard)]
include!(concat!(env!("OUT_DIR"), "/dashboard.rs"));

#[cfg(not(xray_dashboard))]
const INDEX: &str = include_str!("view/index.html");

pub fn routes(view: LocalView) -> Router {
    Router::new()
        .route(
            "/",
            get(move || async move {
                let html = match view {
                    LocalView::Live => INDEX.into(),
                    LocalView::Demo => INDEX.replace("data-mode=\"live\"", "data-mode=\"demo\""),
                };
                Html(html)
            }),
        )
        .route(
            "/assets/{asset}",
            get(|Path(name): Path<String>| async move { asset(&name) }),
        )
        .route("/local-view.css", get(|| async { asset("local-view.css") }))
        .route("/local-view.js", get(|| async { asset("local-view.js") }))
}

fn asset(name: &str) -> Response {
    #[cfg(xray_dashboard)]
    let assets = ASSETS;
    #[cfg(not(xray_dashboard))]
    let assets: &[(&str, &str, &[u8])] = &[
        (
            "local-view.css",
            "text/css; charset=utf-8",
            include_bytes!("view/view.css"),
        ),
        (
            "local-view.js",
            "text/javascript; charset=utf-8",
            include_bytes!("view/view.js"),
        ),
    ];

    if let Some((_, mime, bytes)) = assets.iter().find(|(asset, _, _)| *asset == name) {
        ([(header::CONTENT_TYPE, *mime)], *bytes).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
