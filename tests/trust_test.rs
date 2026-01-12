use chrono::Utc;
use dcap::{
    error::NegotiationError,
    model::{TrustActivity, TrustLevel},
    trust::TrustSystem,
};
use uuid::Uuid;

#[test]
fn test_trust_system_creation() {
    let trust = TrustSystem::new();
    assert!(trust.is_ok());
}

#[test]
fn test_get_initial_reputation() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    let reputation = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { trust.get_reputation(agent_id).await });

    assert_eq!(reputation, 0);
}

#[test]
fn test_update_reputation_positive() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        trust.update_reputation(agent_id, 10).await.unwrap();
        let updated = trust.get_reputation(agent_id).await.unwrap();
        assert_eq!(updated, 10);
    });
}

#[test]
fn test_update_reputation_negative() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        trust.update_reputation(agent_id, -5).await.unwrap();
        let updated = trust.get_reputation(agent_id).await.unwrap();
        assert_eq!(updated, -5);
    });
}

#[test]
fn test_trust_level_neutral() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    let level = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { trust.get_trust_level(agent_id).await });

    assert_eq!(format!("{:?}", level), "Neutral");
}

#[test]
fn test_trust_level_trusted() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        trust.update_reputation(agent_id, 30).await.unwrap();
        let level = trust.get_trust_level(agent_id).await;
        assert_eq!(format!("{:?}", level), "Trusted");
    });
}

#[test]
fn test_trust_level_highly_trusted() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        trust.update_reputation(agent_id, 80).await.unwrap();
        let level = trust.get_trust_level(agent_id).await;
        assert_eq!(format!("{:?}", level), "HighlyTrusted");
    });
}

#[test]
fn test_trust_level_distrusted() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        trust.update_reputation(agent_id, -30).await.unwrap();
        let level = trust.get_trust_level(agent_id).await;
        assert_eq!(format!("{:?}", level), "Distrusted");
    });
}

#[test]
fn test_jwt_generation() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    let jwt = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { trust.generate_jwt(agent_id).await });

    assert!(!jwt.is_empty());
}

#[test]
fn test_jwt_validation_success() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    let jwt = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { trust.generate_jwt(agent_id).await });

    let claims = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { trust.validate_jwt(&jwt).await });

    assert!(claims.is_ok());
    assert_eq!(claims.unwrap().sub, agent_id.to_string());
}

#[test]
fn test_activity_tracking() {
    let trust = TrustSystem::new().unwrap();
    let agent_id = Uuid::new_v4();

    tokio::runtime::Runtime::new().unwrap().block_on(async {
        trust.update_reputation(agent_id, 5).await.unwrap();
        trust.update_reputation(agent_id, 10).await.unwrap();

        let history = trust.get_activity_history(agent_id, 10).await;
        assert!(history.is_ok());
        assert_eq!(history.unwrap().len(), 2);
    });
}
