use std::io::Write;

use probe_rs_rpc_client::RpcClient;
use tempfile::NamedTempFile;

use std::sync::Arc;

use crate::rpc::functions::{ProbeAccess, RpcApp};

#[tokio::test]
async fn local_resolve_upload_tracks_content_changes() {
    let (_server, tx, rx) = RpcApp::create_server(
        16,
        ProbeAccess::All,
        Arc::new(crate::rpc::probe_broker::ProbeBroker::new()),
    );
    let client = RpcClient::new_local_from_wire(tx, rx);

    let mut file = NamedTempFile::new().unwrap();
    write!(file, "v1").unwrap();
    let path = file.path().to_path_buf();

    let first = client.resolve_upload(&path).await.unwrap();
    write!(file, "v2").unwrap();
    let second = client.resolve_upload(&path).await.unwrap();

    assert_ne!(first.content_hash, second.content_hash);
    assert_eq!(first.remote_path, second.remote_path);
}

#[tokio::test]
async fn local_server_schema_matches_the_client() {
    let (server, tx, rx) = RpcApp::create_server(
        16,
        ProbeAccess::All,
        Arc::new(crate::rpc::probe_broker::ProbeBroker::new()),
    );
    let handle = tokio::spawn(async move { server.run().await });
    let client = RpcClient::new_local_from_wire(tx, rx);

    client
        .check_compatibility()
        .await
        .expect("the in-process server must match the client schema");

    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn unknown_session_key_is_an_error_not_a_panic() {
    let (server, tx, rx) = RpcApp::create_server(
        16,
        ProbeAccess::All,
        Arc::new(crate::rpc::probe_broker::ProbeBroker::new()),
    );
    let handle = tokio::spawn(async move { server.run().await });
    let client = RpcClient::new_local_from_wire(tx, rx);

    let session =
        probe_rs_rpc_client::SessionInterface::new(client.clone(), probe_rs_rpc::Key::new());
    let error = session
        .core(0)
        .read_memory_32(0x2000_0000, 1)
        .await
        .expect_err("a key this connection never issued must be rejected");
    assert!(
        format!("{error:?}").contains("Unknown"),
        "unexpected error: {error:?}"
    );
    // Before the fix the first request panicked the connection and this one hung.
    let error = session
        .core(0)
        .halt(std::time::Duration::from_millis(10))
        .await
        .expect_err("a key this connection never issued must be rejected");
    assert!(format!("{error:?}").contains("Unknown"), "{error:?}");

    // The connection survived both requests: the second one got an answer too.

    drop(session);
    drop(client);
    let _ = handle.await;
}
