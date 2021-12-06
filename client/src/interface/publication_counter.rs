use gtk::prelude::{LabelExt, NotebookExtManual};
use gtk::{Label, Notebook};

#[doc(hidden)]
const PUBLICATIONS_TAB: u32 = 2;

/// Keeps count of unread new messages
pub struct PublicationCounter {
    new_messages_amount: u32,
    notebook: Notebook,
    feed_label: Label,
}

impl PublicationCounter {
    /// Returns a new PublicationCounter struct
    pub fn new(notebook: Notebook, feed_label: Label) -> PublicationCounter {
        PublicationCounter {
            new_messages_amount: 0,
            notebook,
            feed_label,
        }
    }

    /// Updates new messages amount by one and sets the corresponding label for the publications tab.
    ///
    /// If Notebook's current page is the publications tab the new messages amount is set to 0
    pub fn update_new_messages_amount(&mut self) {
        if self.notebook.current_page().unwrap() == PUBLICATIONS_TAB {
            self.new_messages_amount = 0;
        } else {
            self.new_messages_amount += 1;
            let new_label = format!("PUBLICACIONES ({})", self.new_messages_amount);
            self.feed_label.set_text(&new_label);
        }
    }
}
