Liberum Neto application


https://drive.google.com/file/d/1TK2H1KUWIjeOfqyyrp3xbxVN7YuNpVQ-/view?usp=sharing

![](./message-diagram.svg)


* A Client, UI is a binary that using liberum_core library connects to the
`liberum_core` daemon via a socket. The client sends messages of type `liberum_core::mesages::CoreRequest`
to the `liberum_core` daemon and receives `liberum_core::messages::CoreResult`,
which is a type of `Result`, as response.
`liberum_cli` and `liberum_gui` are implementations of clients.

* `liberum_core` defines a library and a binary. The library provides all the
necessary knowledge to communicate with the daemon while the binary runs the daemon.
use `cargo run -p liberum_core` to run the core in the terminal or pass `--daemon` to
start it as a daemon. Daemon's files like the socket, pid and standard streams are stored in `/tmp/liberum-core`.

* The daemon parses the requests from the UI and decides on the communication with
other modules.

* `NodeManager`'s role is to manage creating, starting, stopping and saving nodes.
The `core daemon` communicates with it using `liberum_core::node::manager::` messages.
Nodes are identified using String names.

* `NodeStore` provides the abstraction of serializing and deserializing nodes to the
hard drive for the `NodeManager`. The nodes are saved in `$HOME/.liberum-neto`. The
node configuration files can be modified manually or using the client if it provides
the functionality.

* `Node` represents a virtual node in the network. The `core_daemon` receives references
to node actors using the `NodeManager` and sends mesages to `Node` actors from `liberum_core::node::`
module. `Node` is used as a wrapper over the swarm which holds the data necessary
to stop and hides away the libp2p implementation.

* `swarm_runner` module defines the creation, management and the functionality of
a swarm.

Example usage:
```
cargo run -p liberum_core -- --daemon
cargo run -p liberum_cli new-node node1
cargo run -p liberum_cli new-node node2
cargo run -p liberum_cli start-node node1
cargo run -p liberum_cli start-node node2
cargo run -p liberum_cli publish-file node1 ./LICENSE
cargo run -p liberum_cli download-file node2 HpyAhoZv9FkFNUmQX6qEQdvPEV2bKBCVJNSx2eQ1fQMJ
```
