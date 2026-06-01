//! Docker integration tests.
//!
//! These tests require either a running CrateBay built-in runtime or an explicit
//! `DOCKER_HOST`, and are marked `#[ignore]` by default. Run with
//! `cargo test --test docker_integration -- --ignored` when an engine is available.

use cratebay_core::docker;

#[tokio::test]
#[ignore = "Requires CrateBay runtime or explicit DOCKER_HOST"]
async fn docker_connect_succeeds() {
    let result = docker::connect().await;
    assert!(
        result.is_ok(),
        "docker::connect() should succeed when the CrateBay runtime or DOCKER_HOST is reachable: {:?}",
        result.err()
    );
}

#[tokio::test]
#[ignore = "Requires CrateBay runtime or explicit DOCKER_HOST"]
async fn docker_try_connect_returns_some() {
    let docker = docker::try_connect().await;
    assert!(
        docker.is_some(),
        "docker::try_connect() should return Some when the CrateBay runtime or DOCKER_HOST is reachable"
    );
}

#[tokio::test]
#[ignore = "Requires CrateBay runtime or explicit DOCKER_HOST"]
async fn docker_is_available_after_connect() {
    let docker = docker::connect()
        .await
        .expect("CrateBay runtime or DOCKER_HOST must be reachable");
    assert!(
        docker::is_available(&docker).await,
        "is_available() should be true after successful connect"
    );
}

#[tokio::test]
#[ignore = "Requires CrateBay runtime or explicit DOCKER_HOST"]
async fn docker_version_returns_info() {
    let docker = docker::connect()
        .await
        .expect("CrateBay runtime or DOCKER_HOST must be reachable");
    let version = docker::version(&docker).await;
    assert!(
        version.is_ok(),
        "version() should succeed: {:?}",
        version.err()
    );
    let v = version.unwrap();
    assert!(
        v.version.is_some(),
        "Docker version string should be present"
    );
}

#[tokio::test]
#[ignore = "Requires CrateBay runtime or explicit DOCKER_HOST"]
async fn container_list_returns_vec() {
    use cratebay_core::container;

    let docker = docker::connect()
        .await
        .expect("CrateBay runtime or DOCKER_HOST must be reachable");
    let containers = container::list(&docker, true, None).await;
    assert!(
        containers.is_ok(),
        "container::list() should succeed: {:?}",
        containers.err()
    );
    // We can't assert the count, but we can verify the return type
    let _list = containers.unwrap();
}
