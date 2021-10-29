struct TopicHandler {

}

impl TopicHandler {
    fn subscribe(&self, packet: Subscribe, client_id: &str) {

    }

    fn publish(&self, packet: Publish) {

    }

    fn unsubscribe(&self, packet: Unsubscribe, client_id: &str) {

    }

    fn new(server: &Server) -> Self {
        Self {}
    }

    fn remove_client(&self, client_id: &str) {

    }
}
