use std::{cell::RefCell, collections::HashMap};

use gtk::{
    prelude::{ButtonExt, ContainerExt, EntryExt, WidgetExt},
    Box, Button, Entry, IconSize, Label, ListBox, Orientation, Widget,
};
use packets::{qos::QoSLevel, topic::Topic};

pub struct SubscriptionList {
    list: ListBox,
    unsub_entry: Entry,
    subs: RefCell<HashMap<String, Box>>, // Como esto se ejecuta solo en el thread main de gtk, no haria falta un lock
}

impl SubscriptionList {
    /// Creates a new SubsList given a ListBox and a Entry
    pub fn new(list: ListBox, unsub_entry: Entry) -> Self {
        Self {
            list,
            unsub_entry,
            subs: RefCell::new(HashMap::new()),
        }
    }

    /// Removes the given topic from the SubsList and updates the view accordingly
    pub fn remove_sub(&self, topic: &str) {
        if let Some(box_) = self.subs.borrow_mut().remove(topic) {
            let row: Widget = box_.parent().unwrap();
            self.list.remove(&row);
            self.list.show_all();
        }
    }

    /// Removes the given topics from the SubsList and updates the view accordingly
    pub fn remove_subs(&self, topics: &[Topic]) {
        for topic in topics {
            self.remove_sub(topic.name());
        }
    }

    /// Adds the given topics to the SubsList and updates the view accordingly
    pub fn add_subs(&self, topics: &[Topic]) {
        for topic in topics {
            self.add_sub(topic.name(), topic.qos());
        }
    }

    /// Adds the given topic to the SubsList and updates the view accordingly
    pub fn add_sub(&self, topic: &str, qos: QoSLevel) {
        self.remove_sub(topic);
        let box_ = self.create_sub_box(topic, qos);
        self.list.add(&box_);
        self.list.show_all();
        self.subs.borrow_mut().insert(topic.to_string(), box_);
    }

    #[doc(hidden)]
    fn create_sub_box(&self, topic: &str, qos: QoSLevel) -> Box {
        let outer_box = Box::new(Orientation::Horizontal, 5);
        outer_box.add(&Label::new(Some(&format!("[QoS {}]", qos as u8))));
        outer_box.add(&Label::new(Some(topic)));

        // ADD UNSUB BUTTON
        let _topic = topic.to_string();
        let button = Button::from_icon_name(Some("gtk-go-up"), IconSize::Button);
        let entry = self.unsub_entry.clone();
        button.connect_clicked(move |_| {
            entry.set_text(&_topic);
        });

        outer_box.add(&button);

        outer_box
    }
}
