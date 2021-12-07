use gtk::prelude::{LabelExt, NotebookExtManual};
use gtk::{Label, Notebook};
use std::cell::RefCell;

#[doc(hidden)]
const PUBLICATIONS_TAB: u32 = 2;
#[doc(hidden)]
const PUBLICATIONS_LABEL: &str = "PUBLICACIONES";

/// Keeps count of unread new messages
pub struct PublicationCounter {
    new_messages_amount: RefCell<u32>,
    notebook: Notebook,
    feed_label: Label,
}

impl PublicationCounter {
    /// Returns a new PublicationCounter struct
    pub fn new(notebook: Notebook, feed_label: Label) -> PublicationCounter {
        let publication_counter = PublicationCounter {
            new_messages_amount: RefCell::new(0),
            notebook,
            feed_label,
        };
        publication_counter.reset_new_messages_amount();
        publication_counter
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
        self.feed_label.set_text(PUBLICATIONS_LABEL);
    }

    #[doc(hidden)]
    /// Increments the new messages amount by 1 and sets the corresponding label for the
    /// publications tab
    fn increment_new_messages_amount(&self) {
        *self.new_messages_amount.borrow_mut() += 1;
        let new_label = format!(
            "{} ({})",
            PUBLICATIONS_LABEL,
            self.new_messages_amount.borrow()
        );
        self.feed_label.set_text(&new_label);
    }
}
