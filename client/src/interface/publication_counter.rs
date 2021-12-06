use gtk::prelude::{LabelExt, NotebookExtManual};
use gtk::{Label, Notebook};
use std::cell::RefCell;

#[doc(hidden)]
const PUBLICATIONS_TAB: u32 = 2;

/// Keeps count of unread new messages
pub struct PublicationCounter {
    new_messages_amount: RefCell<u32>,
    notebook: Notebook,
    feed_label: Label,
}

impl PublicationCounter {
    /// Returns a new PublicationCounter struct
    pub fn new(notebook: Notebook, feed_label: Label) -> PublicationCounter {
        PublicationCounter {
            new_messages_amount: RefCell::new(0),
            notebook,
            feed_label,
        }
    }

    /// Updates new messages amount by one and sets the corresponding label for the publications tab.
    ///
    /// If Notebook's current page is the publications tab the new messages amount is set to 0
    pub fn update_new_messages_amount(&self) {
        if self.notebook.current_page().unwrap() == PUBLICATIONS_TAB {
            self.reset_new_messages_amount();
        } else {
            self.increment_new_messages_amount();
        }
    }

    /// Resets the amount of new messages to 0
    pub fn reset_new_messages_amount(&self) {
        self.new_messages_amount.replace(0);
        self.feed_label.set_text("PUBLICACIONES");
    }

    #[doc(hidden)]
    /// Increments the new messages amount by 1 and sets the corresponding label for the
    /// publications tab
    fn increment_new_messages_amount(&self) {
        self.new_messages_amount
            .replace_with(|&mut old_messages_amount| old_messages_amount + 1);
        let new_label = format!("PUBLICACIONES ({})", self.new_messages_amount.borrow());
        self.feed_label.set_text(&new_label);
    }
}
