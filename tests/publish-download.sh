#!/bin/bash

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52137/quic-v1"
N2="test_n2"
N2_SEED=2
FILE_NAME="test-file.txt"
FILE_CONTENT="Hello, World!"
BLAKE3_HASH="7cLWjV2o1VsqwkAnyDWK3UemS2psCBHjj865Dovpu4p1"

echo "Publish and download file test:"

# run daemon
killall liberum_core &> /dev/null
nohup cargo run -p liberum_core  &> /dev/null &
sleep 0.01; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
cargo run -p liberum_cli new-node $N1 --id-seed $N1_SEED &> /dev/null
cargo run -p liberum_cli new-node $N2 --id-seed $N2_SEED &> /dev/null
cargo run -p liberum_cli config-node $N1 add-external-addr $N1_ADDR &> /dev/null

# start n1 and get its peer id
cargo run -p liberum_cli -- -d start-node $N1 &> /dev/null
cargo run -p liberum_cli -- -d get-peer-id $N1 &> /dev/null
N1_ID=$(cargo run -p liberum_cli get-peer-id $N1 2> /dev/null)

# add n1 as bootstrap
cargo run -p liberum_cli config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR &> /dev/null
cargo run -p liberum_cli start-node $N2 &> /dev/null

# create and publish file
echo "${FILE_CONTENT}" > "$FILE_NAME"
cargo run -p liberum_cli publish-file $N1 "$FILE_NAME" &> /dev/null

# download file
RESULT=$(cargo run -p liberum_cli -- -d download-file $N2 "${BLAKE3_HASH}" 2> /dev/null)

# cleanup
cargo run -p liberum_cli stop-node $N1 &> /dev/null
cargo run -p liberum_cli stop-node $N2 &> /dev/null
killall liberum_core &> /dev/null
rm "$FILE_NAME"

# check result
if [[ "${RESULT}" =~ "${FILE_CONTENT}" ]]; then
    echo "Success"
    exit 0
else
    echo "Failure"
    echo "\"${RESULT}\" does not contain \"${FILE_CONTENT}\""
    exit 1
fi
