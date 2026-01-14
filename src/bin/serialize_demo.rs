use counterpoint::domain_model::*;
use uuid::Uuid;

fn main() {
    let c2s = C2SCommand::ChatMessageSend(ChatMessageSend {
        conversation_id: ConversationId(Uuid::nil()),
        message_id: MessageId(Uuid::nil()),
        content: "Hello".to_string(),
    });
    println!("{}", serde_json::to_string(&c2s).unwrap());
}
