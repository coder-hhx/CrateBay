use fantoccini::{ClientBuilder, Locator};
use serde_json::json;
use std::{env, time::Duration};

#[tokio::test]
#[ignore = "requires Linux desktop automation runtime"]
async fn desktop_shell_renders_and_navigates() {
    let app = env::var("CRATEBAY_DESKTOP_E2E_APP")
        .expect("CRATEBAY_DESKTOP_E2E_APP must point to the built Tauri binary");
    let webdriver_url =
        env::var("TAURI_DRIVER_URL").unwrap_or_else(|_| "http://127.0.0.1:4444".to_string());

    let mut caps = fantoccini::wd::Capabilities::new();
    caps.insert("browserName".into(), json!("tauri"));
    caps.insert("app".into(), json!(app));

    let mut builder = ClientBuilder::rustls().expect("rustls client builder");
    builder.capabilities(caps);
    let client = builder
        .connect(&webdriver_url)
        .await
        .expect("connect to tauri-driver");

    client
        .wait()
        .at_most(Duration::from_secs(30))
        .for_element(Locator::Css("[data-testid='nav-dashboard']"))
        .await
        .expect("dashboard navigation rendered");

    client
        .find(Locator::Css("[data-testid='nav-ai']"))
        .await
        .expect("AI nav")
        .click()
        .await
        .expect("click AI nav");

    client
        .wait()
        .at_most(Duration::from_secs(15))
        .for_element(Locator::Css("[role='tab']"))
        .await
        .expect("AI tabs rendered");

    let page = client.source().await.expect("page source");
    assert!(
        page.contains("Assistant"),
        "assistant tab should render in desktop shell"
    );

    client
        .find(Locator::Css("[data-testid='nav-settings']"))
        .await
        .expect("settings nav")
        .click()
        .await
        .expect("click settings nav");

    let settings_page = client.source().await.expect("settings source");
    assert!(
        settings_page.contains("General"),
        "settings general tab should render"
    );
    assert!(
        settings_page.contains("AI"),
        "settings AI tab should render"
    );

    client.close().await.expect("close webdriver session");
}
