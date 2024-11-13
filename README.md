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

If our bootstrap node is running you can test it:
```
# run daemon
cargo run -p liberum_core -- --daemon

# create and start your node
cargo run -p liberum_cli new-node node1
cargo run -p liberum_cli start-node node1

# dial the bootstrap node
cargo run -p liberum_cli dial node1 12D3KooWJE1MwwkHSB8JCErBFe6sfU9o6Ye3kKzQnjYchsq1iTnG /ip4/192.166.217.23/udp/52137/quic-v1

#   the bootstrap node can be added to the config to connect to it automatically when starting your node
# cargo run -p liberum_cli config-node add-bootstrap-node 12D3KooWJE1MwwkHSB8JCErBFe6sfU9o6Ye3kKzQnjYchsq1iTnG /ip4/192.166.217.23/udp/52137/quic-v1

# Some files served by the network:
cargo run -p liberum_cli download-file node1 48czMELuzA6y23AYLKwNwazuwTUK6WCV2zDWcrSo9zCi
cargo run -p liberum_cli download-file node1 HpyAhoZv9FkFNUmQX6qEQdvPEV2bKBCVJNSx2eQ1fQMJ
cargo run -p liberum_cli download-file node1 6uerPJKd8jkEfwvSgwr7zsC7eXtYhrmRh4chYM1kkqsb

# You can publish a file of your own
# The ID of the file will be printed
cargo run -p liberum_cli publish-file node1 <file-path>

# You and all the other nodes will be able to retrieve it
cargo run -p liberum_cli download-file node1 <file-id>

# kill the daemon
killall liberum_core
```
