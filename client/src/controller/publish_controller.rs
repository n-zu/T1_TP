use gtk::{prelude::ListBoxExt, Label, ListBox};
use packets::publish::Publish;

use super::super::client::PublishObserver;

pub struct PublishController {
    list: ListBox,
}

impl PublishController {
    pub fn new(list: ListBox) -> PublishController {
        PublishController { list }
    }
}

impl PublishObserver for PublishController {
    fn got_publish(&self, publish: Publish) {
        let msg = format!("{}: {}", publish.topic_name(), publish.payload().unwrap());
        self.list.insert(&Label::new(Some(&msg)), 0);
    }
}
