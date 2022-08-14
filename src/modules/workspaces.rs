use crate::modules::{Module, ModuleInfo};
use gtk::prelude::*;
use gtk::{Button, Orientation};
use ksway::client::Client;
use ksway::{IpcCommand, IpcEvent};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

#[derive(Debug, Deserialize, Clone)]
pub struct WorkspacesModule {
    pub(crate) name_map: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
struct Workspace {
    name: String,
    focused: bool,
    // num: i32,
    // output: String,
}

impl Workspace {
    fn as_button(&self, name_map: &HashMap<String, String>, tx: &mpsc::Sender<String>) -> Button {
        let button = Button::builder()
            .label(name_map.get(self.name.as_str()).unwrap_or(&self.name))
            .build();

        let style_context = button.style_context();
        style_context.add_class("item");

        if self.focused {
            style_context.add_class("focused");
        }

        {
            let tx = tx.clone();
            let name = self.name.clone();
            button.connect_clicked(move |_item| tx.try_send(name.clone()).unwrap());
        }

        button
    }
}

#[derive(Deserialize, Debug)]
struct WorkspaceEvent {
    change: String,
    old: Option<Workspace>,
    current: Option<Workspace>,
}

impl Module<gtk::Box> for WorkspacesModule {
    fn into_widget(self, _info: &ModuleInfo) -> gtk::Box {
        let mut sway = Client::connect().unwrap();

        let container = gtk::Box::new(Orientation::Horizontal, 0);

        let workspaces = {
            let raw = sway.ipc(IpcCommand::GetWorkspaces).unwrap();
            serde_json::from_slice::<Vec<Workspace>>(&raw).unwrap()
        };

        let name_map = self.name_map.unwrap_or_default();

        let mut button_map: HashMap<String, Button> = HashMap::new();

        let (ui_tx, mut ui_rx) = mpsc::channel(32);

        for workspace in workspaces {
            let item = workspace.as_button(&name_map, &ui_tx);
            container.add(&item);
            button_map.insert(workspace.name, item);
        }

        let srx = sway.subscribe(vec![IpcEvent::Workspace]).unwrap();
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        spawn_blocking(move || loop {
            while let Ok((_, payload)) = srx.try_recv() {
                let payload: WorkspaceEvent = serde_json::from_slice(&payload).unwrap();
                tx.send(payload).unwrap();
            }
            sway.poll().unwrap();
        });

        {
            let menubar = container.clone();
            rx.attach(None, move |event| {
                match event.change.as_str() {
                    "focus" => {
                        let old = event.old.unwrap();
                        let old_button = button_map.get(&old.name).unwrap();
                        old_button.style_context().remove_class("focused");

                        let new = event.current.unwrap();
                        let new_button = button_map.get(&new.name).unwrap();
                        new_button.style_context().add_class("focused");
                    }
                    "init" => {
                        let workspace = event.current.unwrap();
                        let item = workspace.as_button(&name_map, &ui_tx);

                        item.show();
                        menubar.add(&item);
                        button_map.insert(workspace.name, item);
                    }
                    "empty" => {
                        let current = event.current.unwrap();
                        let item = button_map.get(&current.name).unwrap();
                        menubar.remove(item);
                    }
                    _ => {}
                }

                Continue(true)
            });
        }

        spawn(async move {
            let mut sway = Client::connect().unwrap();
            while let Some(name) = ui_rx.recv().await {
                sway.run(format!("workspace {}", name)).unwrap();
            }
        });

        container
    }
}
