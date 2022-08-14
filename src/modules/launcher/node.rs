use ksway::{Client, IpcCommand};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SwayNode {
    #[serde(rename = "type")]
    pub node_type: String,
    pub id: i32,
    pub name: Option<String>,
    pub app_id: Option<String>,
    pub focused: bool,
    pub urgent: bool,
    pub nodes: Vec<SwayNode>,
    pub floating_nodes: Vec<SwayNode>,
    pub shell: Option<String>,
    pub window_properties: Option<WindowProperties>,
}

#[derive(Debug, Deserialize)]
pub struct WindowProperties {
    pub class: String,
}

impl SwayNode {
    pub fn get_id(&self) -> &str {
        self.app_id.as_ref().map_or_else(
            || {
                &self
                    .window_properties
                    .as_ref()
                    .expect("cannot find node name")
                    .class
            },
            |app_id| app_id,
        )
    }

    pub fn is_xwayland(&self) -> bool {
        self.shell == Some(String::from("xwayland"))
    }
}

fn check_node(node: SwayNode, window_nodes: &mut Vec<SwayNode>) {
    if node.name.is_some() && (node.node_type == "con" || node.node_type == "floating_con") {
        window_nodes.push(node);
    } else {
        node.nodes.into_iter().for_each(|node| {
            check_node(node, window_nodes);
        });

        node.floating_nodes.into_iter().for_each(|node| {
            check_node(node, window_nodes);
        });
    }
}

pub fn get_open_windows(sway: &mut Client) -> Vec<SwayNode> {
    let raw = sway.ipc(IpcCommand::GetTree).unwrap();
    let root_node = serde_json::from_slice::<SwayNode>(&raw).unwrap();

    let mut window_nodes = vec![];
    check_node(root_node, &mut window_nodes);

    window_nodes
}
