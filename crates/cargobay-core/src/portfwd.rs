use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// Key for identifying a port forward: (vm_id, host_port).
type ForwardKey = (String, u16);

/// Handle returned when a forward is started; dropping it does NOT stop the
/// forward -- call [`PortForwardManager::remove`] instead.
struct ForwardHandle {
    /// Cancel token -- when dropped the listener loop exits.
    cancel: tokio_util::sync::CancellationToken,
}

/// Manages active TCP port-forward listeners.
#[derive(Clone)]
pub struct PortForwardManager {
    inner: Arc<Mutex<HashMap<ForwardKey, ForwardHandle>>>,
}

impl Default for PortForwardManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PortForwardManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start forwarding `host_port` on the host to `guest_addr:guest_port`.
    ///
    /// `guest_addr` is typically the VM's IP address (e.g. `192.168.64.2`).
    /// For now callers may pass `"127.0.0.1"` when the VM network is not yet
    /// wired up -- the listener will still bind and attempt connections.
    pub async fn add(
        &self,
        vm_id: &str,
        host_port: u16,
        guest_addr: &str,
        guest_port: u16,
        _protocol: &str, // reserved for future UDP support
    ) -> Result<(), String> {
        let key: ForwardKey = (vm_id.to_string(), host_port);

        let mut map = self.inner.lock().await;
        if map.contains_key(&key) {
            return Err(format!(
                "Port forward already active: host {} for VM {}",
                host_port, vm_id
            ));
        }

        let bind_addr: SocketAddr = format!("0.0.0.0:{}", host_port)
            .parse()
            .map_err(|e| format!("invalid bind address: {}", e))?;

        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| format!("failed to bind port {}: {}", host_port, e))?;

        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel_clone = cancel.clone();
        let target = format!("{}:{}", guest_addr, guest_port);

        info!(
            vm_id = vm_id,
            host_port = host_port,
            target = %target,
            "port forward started"
        );

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        info!(host_port = host_port, "port forward cancelled");
                        break;
                    }
                    result = listener.accept() => {
                        match result {
                            Ok((inbound, peer)) => {
                                let target = target.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = proxy(inbound, &target).await {
                                        warn!(
                                            peer = %peer,
                                            target = %target,
                                            error = %e,
                                            "proxy connection failed"
                                        );
                                    }
                                });
                            }
                            Err(e) => {
                                error!(error = %e, "accept failed on port {}", host_port);
                                break;
                            }
                        }
                    }
                }
            }
        });

        map.insert(key, ForwardHandle { cancel });
        Ok(())
    }

    /// Stop a port forward.
    pub async fn remove(&self, vm_id: &str, host_port: u16) -> Result<(), String> {
        let key: ForwardKey = (vm_id.to_string(), host_port);
        let mut map = self.inner.lock().await;
        if let Some(handle) = map.remove(&key) {
            handle.cancel.cancel();
            info!(vm_id = vm_id, host_port = host_port, "port forward removed");
            Ok(())
        } else {
            Err(format!(
                "No active port forward on host port {} for VM {}",
                host_port, vm_id
            ))
        }
    }

    /// List host ports currently forwarded for a VM.
    pub async fn list(&self, vm_id: &str) -> Vec<u16> {
        let map = self.inner.lock().await;
        map.keys()
            .filter(|(vid, _)| vid == vm_id)
            .map(|(_, port)| *port)
            .collect()
    }

    /// Stop all forwards for a given VM.
    pub async fn remove_all(&self, vm_id: &str) {
        let mut map = self.inner.lock().await;
        let keys: Vec<ForwardKey> = map
            .keys()
            .filter(|(vid, _)| vid == vm_id)
            .cloned()
            .collect();
        for key in keys {
            if let Some(handle) = map.remove(&key) {
                handle.cancel.cancel();
            }
        }
    }
}

/// Bi-directional TCP proxy between `inbound` and `target_addr`.
async fn proxy(mut inbound: TcpStream, target_addr: &str) -> Result<(), String> {
    let mut outbound = TcpStream::connect(target_addr)
        .await
        .map_err(|e| format!("connect to {}: {}", target_addr, e))?;

    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    let client_to_server = io::copy(&mut ri, &mut wo);
    let server_to_client = io::copy(&mut ro, &mut wi);

    tokio::select! {
        r = client_to_server => { r.map_err(|e| e.to_string())?; }
        r = server_to_client => { r.map_err(|e| e.to_string())?; }
    }

    Ok(())
}
