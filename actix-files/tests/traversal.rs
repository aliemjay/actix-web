use actix_files::Files;
use actix_web::{
    http::StatusCode,
    test::{self, TestRequest},
    App,
};
use std::fs::File;
use std::io::Read;

#[actix_rt::test]
async fn test_path_traversal() {
    let mut urls = String::new();
    File::open("tests/Traversal.txt")
        .unwrap()
        .read_to_string(&mut urls)
        .unwrap();
    let urls = urls.lines().map(|s| format!("/{}", s));

    let srv = test::init_service(App::new().service(Files::new("", "."))).await;
    let mut failed = false;
    for url in urls {
        let req = TestRequest::get().uri(&url).to_request();
        let res = test::call_service(&srv, req).await;
        if res.status() != StatusCode::BAD_REQUEST {
            eprintln!("path traversal: {} {}", res.status(), &url);
            failed = true;
        }
    }

    assert!(!failed);
}
