use server_oxide::chat::{ChatContent, ClientToServer, SendMessage};
use server_oxide::domain::ConversationId;
use uuid::Uuid;

fn main() {
    let c2s = ClientToServer::Send(SendMessage {
        message_seq: 1,
        content: ChatContent {
            conversation_id: ConversationId(Uuid::nil()),
            content: "Hello".to_string(),
        },
    });
    println!("{}", serde_json::to_string(&c2s).unwrap());
}
