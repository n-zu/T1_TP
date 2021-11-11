use gtk::{
    prelude::{BuilderExtManual, LabelExt, StackExt, WidgetExt},
    Box, Builder, Label, Stack,
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
    }

    fn show_content_menu(&self) {
        let stack: Stack = self.builder().object("content").unwrap();
        stack.set_visible_child_name("box_connected");
    }

    fn sensitive_connect_menu(&self, sensitive: bool) {
        let connect_window: Box = self.builder().object("box_connection").unwrap();
        connect_window.set_sensitive(sensitive);
    }

    fn sensitive_content_menu(&self, sensitive: bool) {
        let connect_window: Box = self.builder().object("box_connected").unwrap();
        connect_window.set_sensitive(sensitive);
    }

    fn status_message(&self, msg: &str) {
        let status_text: Label = self.builder().object("status_label").unwrap();
        status_text.set_text(msg);
    }

    fn connection_info(&self, msg: &str) {
        let info: Label = self.builder().object("connection_info").unwrap();
        info.set_text(msg);
    }
}
