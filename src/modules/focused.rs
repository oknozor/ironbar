use crate::clients::wayland::{self, ToplevelChange};
use crate::config::{CommonConfig, TruncateMode};
use crate::image::ImageProvider;
use crate::modules::{Module, ModuleInfo, ModuleUpdateEvent, ModuleWidget, WidgetContext};
use crate::{await_sync, read_lock, send_async};
use color_eyre::Result;
use glib::Continue;
use gtk::prelude::*;
use gtk::Label;
use serde::Deserialize;
use tokio::spawn;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::error;

#[derive(Debug, Deserialize, Clone)]
pub struct FocusedModule {
    /// Whether to show icon on the bar.
    #[serde(default = "crate::config::default_true")]
    show_icon: bool,
    /// Whether to show app name on the bar.
    #[serde(default = "crate::config::default_true")]
    show_title: bool,

    /// Icon size in pixels.
    #[serde(default = "default_icon_size")]
    icon_size: i32,

    truncate: Option<TruncateMode>,

    #[serde(flatten)]
    pub common: Option<CommonConfig>,
}

const fn default_icon_size() -> i32 {
    32
}

impl Module<gtk::Box> for FocusedModule {
    type SendMessage = (String, String);
    type ReceiveMessage = ();

    fn name() -> &'static str {
        "focused"
    }

    fn spawn_controller(
        &self,
        _info: &ModuleInfo,
        tx: Sender<ModuleUpdateEvent<Self::SendMessage>>,
        _rx: Receiver<Self::ReceiveMessage>,
    ) -> Result<()> {
        let focused = await_sync(async {
            let wl = wayland::get_client().await;
            let toplevels = read_lock!(wl.toplevels);

            toplevels
                .iter()
                .find(|(_, (top, _))| top.active)
                .map(|(_, (top, _))| top.clone())
        });

        if let Some(top) = focused {
            tx.try_send(ModuleUpdateEvent::Update((top.title.clone(), top.app_id)))?;
        }

        spawn(async move {
            let mut wlrx = {
                let wl = wayland::get_client().await;
                wl.subscribe_toplevels()
            };

            while let Ok(event) = wlrx.recv().await {
                let update = match event.change {
                    ToplevelChange::Focus(focus) => focus,
                    ToplevelChange::Title(_) => event.toplevel.active,
                    _ => false,
                };

                if update {
                    send_async!(
                        tx,
                        ModuleUpdateEvent::Update((event.toplevel.title, event.toplevel.app_id))
                    );
                }
            }
        });

        Ok(())
    }

    fn into_widget(
        self,
        context: WidgetContext<Self::SendMessage, Self::ReceiveMessage>,
        info: &ModuleInfo,
    ) -> Result<ModuleWidget<gtk::Box>> {
        let icon_theme = info.icon_theme;

        let container = gtk::Box::new(info.bar_position.get_orientation(), 5);

        let icon = gtk::Image::builder().name("icon").build();
        let label = Label::builder().name("label").build();

        if let Some(truncate) = self.truncate {
            truncate.truncate_label(&label);
        }

        container.add(&icon);
        container.add(&label);

        {
            let icon_theme = icon_theme.clone();
            context.widget_rx.attach(None, move |(name, id)| {
                if self.show_icon {
                    if let Err(err) = ImageProvider::parse(&id, &icon_theme, self.icon_size)
                        .and_then(|image| image.load_into_image(icon.clone()))
                    {
                        error!("{err:?}");
                    }
                }

                if self.show_title {
                    label.set_label(&name);
                }

                Continue(true)
            });
        }

        Ok(ModuleWidget {
            widget: container,
            popup: None,
        })
    }
}
