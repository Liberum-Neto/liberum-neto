#!/bin/bash

cargo run -p liberum_cli new-node alpha
ALPHA_ADDR="/ip4/127.0.0.1/udp/12345/quic-v1"
cargo run -p liberum_cli config-node alpha add-external-addr $ALPHA_ADDR
cargo run -p liberum_cli start-node alpha
ALPHA_ID=$(cargo run -p liberum_cli -- -d get-peer-id alpha 2> /dev/null)

cargo run -p liberum_cli new-node beta
cargo run -p liberum_cli config-node beta add-bootstrap-node "${ALPHA_ID}" "${ALPHA_ADDR}"
cargo run -p liberum_cli config-node beta add-external-addr "/ip4/127.0.0.1/udp/0/quic-v1"

cargo run -p liberum_cli new-node gamma
cargo run -p liberum_cli config-node gamma add-bootstrap-node "${ALPHA_ID}" "${ALPHA_ADDR}"
cargo run -p liberum_cli config-node gamma add-external-addr "/ip4/127.0.0.1/udp/0/quic-v1"

cargo run -p liberum_cli new-node omicron
cargo run -p liberum_cli config-node omicron add-bootstrap-node "${ALPHA_ID}" "${ALPHA_ADDR}"
cargo run -p liberum_cli config-node omicron add-external-addr "/ip4/127.0.0.1/udp/0/quic-v1"

cargo run -p liberum_cli start-node beta
cargo run -p liberum_cli start-node gamma
cargo run -p liberum_cli start-node omicron
