use relay_protocol::{payloads, Message, MessageType};

pub async fn dispatch(msg: Message) -> Option<Message> {
    match msg.msg_type {
        MessageType::Request => handle_request(msg).await,
        _ => None,
    }
}

async fn handle_request(msg: Message) -> Option<Message> {
    match msg.subtype.as_str() {
        "ping" => {
            let pong = payloads::PongResponse {
                uptime_secs: 0,
                docker_version: "26.1.3".into(),
                arch: std::env::consts::ARCH.into(),
                os: std::env::consts::OS.into(),
                relay_version: Some(env!("CARGO_PKG_VERSION").into()),
            };
            Some(Message::new_response(
                msg.id,
                "pong",
                serde_json::to_value(pong).unwrap(),
            ))
        }
        _ => {
            let err = payloads::ErrorResponse {
                error_code: "RELAY.NOT_IMPLEMENTED".into(),
                message: format!("Operation '{}' not yet implemented", msg.subtype),
                details: None,
            };
            Some(Message::new_response(
                msg.id,
                "error",
                serde_json::to_value(err).unwrap(),
            ))
        }
    }
}
