use std::sync::Arc;
use std::time::Duration;
use libp2p::PeerId;
use tokio::time::sleep;

use hainet_core::networking::{
    registry::DeviceRegistry,
    service_manager::{ServiceManager, ServiceType},
    heartbeat::HeartbeatManager,
    auto_healer::{AutoHealer, AutoHealerConfig},
    peer_discovery::{PeerStatus, DeviceCapabilities, PeerInfo, DeviceRole},
};

#[tokio::test]
async fn test_auto_healing_peer_failure() {
    // 1. Setup
    let local_peer_id = PeerId::random();
    let registry = Arc::new(DeviceRegistry::new(local_peer_id));
    let service_manager = Arc::new(ServiceManager::new());
    
    // Configure HeartbeatManager with short interval
    let heartbeat_interval = Duration::from_millis(100);
    let heartbeat_manager = Arc::new(HeartbeatManager::with_interval(
        registry.clone(),
        heartbeat_interval
    ));

    // Configure AutoHealer with short interval
    let healer_config = AutoHealerConfig {
        check_interval: Duration::from_millis(100),
        enabled: true,
    };
    let auto_healer = Arc::new(AutoHealer::new(
        registry.clone(),
        service_manager.clone(),
        heartbeat_manager.clone(),
        healer_config,
    ));

    // 2. Register a remote peer
    let remote_peer_id = PeerId::random();
    let caps = DeviceCapabilities {
        cpu_cores: 4,
        ram_gb: 8,
        has_gpu: false,
        gpu_memory_mb: 0,
        disk_gb: 256,
        os: "Linux".to_string(),
        arch: "x86_64".to_string(),
        score: 0.0,
    };
    
    let peer_info = PeerInfo::new(
        remote_peer_id,
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        8080,
        caps,
        DeviceRole::Slave,
    );
    
    registry.register_device(peer_info).await.unwrap();

    // Ensure it's online
    registry.update_status(&remote_peer_id, PeerStatus::Online).await.unwrap();

    // 3. Register a service for this peer
    let service_type = ServiceType::LLM { models: vec!["gemma3:7b".to_string()] };
    let service_id = service_manager.register_service(
        service_type,
        remote_peer_id,
        "http://127.0.0.1:8080".to_string()
    );

    // Verify initial state
    assert!(service_manager.get_service(service_id).unwrap().is_healthy());
    assert_eq!(registry.get_device(&remote_peer_id).await.unwrap().peer_info.status, PeerStatus::Online);

    // 4. Start components
    heartbeat_manager.start().await.unwrap();
    auto_healer.start().await.unwrap();

    // 5. Wait for failure detection
    // Heartbeat interval 100ms. Need 3 misses -> 300ms.
    // AutoHealer checks every 100ms.
    // Wait 3 seconds to be safe.
    sleep(Duration::from_secs(3)).await;

    // 6. Verify recovery actions
    
    // Peer should be marked Offline (or Suspected at least)
    // HeartbeatManager logic: < 3 misses = Healthy. >= 3 misses = Unhealthy.
    // AutoHealer checks HeartbeatManager.check_peer_health().
    // If missed >= 5 -> Offline. If < 5 -> Suspected.
    // Wait, let's check HeartbeatManager::check_peer_health logic again.
    // if state.is_healthy() (missed < 3) -> Online
    // else if missed < 5 -> Suspected
    // else -> Offline.
    
    // So after 1 second (10 misses), it should be Offline.
    
    let device = registry.get_device(&remote_peer_id).await.unwrap();
    println!("Device status: {:?}", device.peer_info.status);
    assert_eq!(device.peer_info.status, PeerStatus::Offline);

    // Service should be marked Unhealthy
    let service = service_manager.get_service(service_id).unwrap();
    println!("Service health: {:?}", service.health_status);
    assert!(service.is_unhealthy());

    // 7. Cleanup
    auto_healer.stop().await.unwrap();
    heartbeat_manager.stop().await.unwrap();
}
