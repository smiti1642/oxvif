#![cfg(feature = "recording-download")]

use oxvif::recording::download::{DownloadError, DownloadOptions, download_until};
use std::future;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

async fn http(response: &'static [u8]) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap().0;
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
        }
        assert!(
            !String::from_utf8_lossy(&request)
                .to_ascii_lowercase()
                .contains("authorization")
        );
        stream.write_all(response).await.unwrap();
    });
    (format!("http://{addr}/export?query-secret"), task)
}

#[tokio::test]
async fn export_transfer_bounded_and_no_clobber() {
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("footage.mp4");
    let options = DownloadOptions {
        max_bytes: 8,
        ..Default::default()
    };
    let (uri, task) =
        http(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\noriginal").await;
    assert_eq!(
        download_until(&uri, &uri, &dest, &options, future::pending())
            .await
            .unwrap(),
        8
    );
    task.await.unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"original");
    assert_eq!(
        download_until(&uri, &uri, &dest, &options, future::pending()).await,
        Err(DownloadError::Exists)
    );
    let other = temp.path().join("another.mp4");
    let (uri, task) = http(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n123456789\r\n0\r\n\r\n").await;
    assert_eq!(
        download_until(&uri, &uri, &other, &options, future::pending()).await,
        Err(DownloadError::Limit)
    );
    task.await.unwrap();
    let (uri, task) = http(b"HTTP/1.1 302 Found\r\nLocation: http://invalid.example/private\r\nContent-Length: 0\r\n\r\n").await;
    assert_eq!(
        download_until(&uri, &uri, &other, &options, future::pending()).await,
        Err(DownloadError::Refused(302))
    );
    task.await.unwrap();
    assert_eq!(
        download_until(
            &uri,
            "http://other.invalid",
            &other,
            &options,
            future::pending()
        )
        .await,
        Err(DownloadError::Invalid)
    );
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn cancelling_download_does_not_publish_partial_file() {
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("cancel.mp4");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (sent, ready) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap().0;
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\npart")
            .await
            .unwrap();
        sent.send(()).unwrap();
        let mut byte = [0];
        let result = stream.read(&mut byte).await;
        assert!(result.is_err() || result.unwrap() == 0);
    });
    let url = format!("http://{addr}/export");
    let cancelled = async {
        ready.await.unwrap();
        while std::fs::read_dir(temp.path()).unwrap().next().is_none() {
            tokio::task::yield_now().await;
        }
    };
    assert_eq!(
        download_until(&url, &url, &dest, &DownloadOptions::default(), cancelled).await,
        Err(DownloadError::Cancelled)
    );
    server.await.unwrap();
    assert!(!dest.exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn competing_file_created_during_transfer_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("recording.mp4");
    let competing = destination.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut stream = listener.accept().await.unwrap().0;
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
        }
        std::fs::write(competing, b"preserve").unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew")
            .await
            .unwrap();
    });
    let url = format!("http://{addr}/export");
    assert_eq!(
        download_until(
            &url,
            &url,
            &destination,
            &DownloadOptions::default(),
            future::pending()
        )
        .await,
        Err(DownloadError::Exists)
    );
    server.await.unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), b"preserve");
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}
