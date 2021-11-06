use gtk::{Label, ListBox, prelude::ListBoxExt};

use super::super::client::PublishObserver;
use packets::publish::Publish as PublishRecv;

pub struct PublishController {
    list : ListBox
}

impl PublishController {
    pub fn new(list : ListBox) -> PublishController {
        PublishController {
            list
        }
    }
}

impl PublishObserver for PublishController {
    fn got_publish(&self, publish: PublishRecv) {
        let msg = format!("{}: {}", publish.topic_name(), publish.payload().unwrap());
        self.list.insert(&Label::new(Some(&msg)), 0);
    }
}