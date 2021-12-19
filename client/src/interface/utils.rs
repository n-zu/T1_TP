use gtk::prelude::ContainerExt;
use gtk::{
    prelude::{
        BuilderExtManual, DialogExt, EntryExt, LabelExt, StackExt, SwitchExt, TextBufferExt,
        WidgetExt,
    },
    Builder, Button, ButtonsType, DialogFlags, Entry, Label, ListBox, MessageDialog, MessageType,
    Stack, Switch, TextBuffer, Window,
};

/// Icons of the status bar
pub(crate) enum Icon {
    Ok,
    Error,
    Loading,
}

impl From<Icon> for &str {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Ok => "ok",
            Icon::Error => "error",
            Icon::Loading => "loading",
        }
    }
}

pub(crate) trait InterfaceUtils {
    /// Gets the interface builder. Used to give access
    /// the different utilities of this trait access to
    /// the interface
    fn builder(&self) -> &Builder;

    /// Changes the status bar icon to the given one
    fn icon(&self, icon: Icon) {
        let status_icon: Stack = self.builder().object("status_icon").unwrap();
        status_icon.set_visible_child_name(<&str>::from(icon));
    }

    /// Switches the interface to the connect menu
    fn show_connect_menu(&self) {
        let stack: Stack = self.builder().object("content").unwrap();
        stack.set_visible_child_name("box_connection");
        self.sensitive(true);
    }

    /// Switches the interface to the connected/content menu
    fn show_content_menu(&self) {
        let stack: Stack = self.builder().object("content").unwrap();
        stack.set_visible_child_name("box_connected");
        self.sensitive(true);
    }

    /// Allows or disallows the user to touch or write anything
    fn sensitive(&self, sensitive: bool) {
        let content: Stack = self.builder().object("content").unwrap();
        let disconnect_button: Button = self.builder().object("discon_btn").unwrap();
        content.set_sensitive(sensitive);
        disconnect_button.set_sensitive(sensitive);
    }

    /// Sets the status bar message
    fn status_message(&self, msg: &str) {
        let status_text: Label = self.builder().object("status_label").unwrap();
        status_text.set_text(msg);
    }

    /// If msg is Some(_), it sets and shows the connection
    /// info bar with the given message. If it is None,
    /// it hides it.
    fn connection_info(&self, msg: Option<&str>) {
        if let Some(text) = msg {
            let label: Label = self.builder().object("connection_info").unwrap();
            label.set_text(text);
        }
    }

    /// Sets a given text to a Entry label
    fn set_text_to_entry_box(&self, box_id: &str, text: &str) {
        let entry: Entry = self.builder().object(box_id).unwrap();
        entry.set_text(text);
    }

    /// Sets a given state to a Switch label
    fn set_state_to_switch_box(&self, switch_id: &str, new_state: bool) {
        let entry: Switch = self.builder().object(switch_id).unwrap();
        entry.set_active(new_state);
    }

    fn set_buffer_to_text_buffer(&self, text_buffer_id: &str, text: &str) {
        let text_buffer: TextBuffer = self.builder().object(text_buffer_id).unwrap();
        text_buffer.set_text(text);
    }

    /// Removes all the children from a given ListBox label
    fn remove_all_children_from_listbox(&self, list_box_id: &str) {
        let subscription_list: ListBox = self.builder().object(list_box_id).unwrap();
        let widgets = subscription_list.children();
        for widget in widgets {
            subscription_list.remove(&widget);
        }
    }
}

/// Creates an error popup with the given message that
/// blocks the current thread until the user closes it
pub(crate) fn alert(message: &str) {
    let dialog = MessageDialog::new(
        None::<&Window>,
        DialogFlags::MODAL,
        MessageType::Error,
        ButtonsType::Close,
        message,
    );
    dialog.run();
    dialog.emit_close();
}
