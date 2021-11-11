use gtk::{
    prelude::{BuilderExtManual, DialogExt, LabelExt, StackExt, WidgetExt},
    Box, Builder, Button, ButtonsType, DialogFlags, Label, MessageDialog, MessageType, Stack,
    Window,
};

pub enum Icon {
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

pub trait InterfaceUtils {
    fn builder(&self) -> &Builder;

    fn icon(&self, icon: Icon) {
        let status_icon: Stack = self.builder().object("status_icon").unwrap();
        status_icon.set_visible_child_name(<&str>::from(icon));
    }

    fn show_connect_menu(&self) {
        let stack: Stack = self.builder().object("content").unwrap();
        stack.set_visible_child_name("box_connection");
        self.sensitive(true);
    }

    fn show_content_menu(&self) {
        let stack: Stack = self.builder().object("content").unwrap();
        stack.set_visible_child_name("box_connected");
        self.sensitive(true);
    }

    fn sensitive(&self, sensitive: bool) {
        let content: Stack = self.builder().object("content").unwrap();
        let disconnect_button: Button = self.builder().object("discon_btn").unwrap();
        content.set_sensitive(sensitive);
        disconnect_button.set_sensitive(sensitive);
    }

    fn status_message(&self, msg: &str) {
        let status_text: Label = self.builder().object("status_label").unwrap();
        status_text.set_text(msg);
    }

    fn connection_info(&self, msg: Option<&str>) {
        let info_box: Box = self.builder().object("info_box").unwrap();
        if let Some(text) = msg {
            let label: Label = self.builder().object("connection_info").unwrap();
            info_box.set_visible(true);
            label.set_text(text);
        } else {
            info_box.set_visible(false);
        }
    }
}

pub fn alert(message: &str) {
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
