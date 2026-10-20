#[cfg(test)]
mod tests {
    use crate::store::*;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("bfish-sessions-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn session_round_trip_and_list() {
        let dir = temp_dir();
        let store = SessionStore::new(&dir);
        let mut session = store.create("openai", "gpt-4.1").expect("create");
        assert_eq!(session.title, "New chat");

        session.title = "Build a lobby".to_string();
        session
            .messages
            .push(crate::models::ChatMessage::user("hello"));
        store.save(&session).expect("save");

        let loaded = store.load(&session.id).expect("load");
        assert_eq!(loaded.title, "Build a lobby");
        assert_eq!(loaded.messages.len(), 1);

        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].message_count, 1);

        store.delete(&session.id).expect("delete");
        assert!(store.list().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_path_traversal() {
        let store = SessionStore::new(temp_dir());
        assert!(store.load("../../secret").is_err());
        assert!(store.delete("bad/id").is_err());
    }

    #[test]
    fn update_changes_timestamp() {
        let dir = temp_dir();
        let store = SessionStore::new(&dir);
        let session = store.create("gemini", "gemini-2.5-pro").expect("create");
        let updated = store
            .update(&session.id, |session| session.title = "Renamed".to_string())
            .expect("update");
        assert_eq!(updated.title, "Renamed");
        assert!(updated.updated_at >= session.updated_at);
        std::fs::remove_dir_all(dir).ok();
    }
}
