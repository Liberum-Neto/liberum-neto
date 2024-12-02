#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

NODE_COUNT=100

FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"
BLAKE3_HASH="7cLWjV2o1VsqwkAnyDWK3UemS2psCBHjj865Dovpu4p1"

NODE_ADDR_PREFIX="/ip6/::1/udp/"
NODE_ADDR_SUFFIX="/quic-v1"

echo "Provide and download file test:"

# run daemon
killall liberum_core &> /dev/null
cargo run -p liberum_core -- --daemon  &> /dev/null &
sleep 0.5; # the socket file is created asynchronously and may not be ready yet :))))

cargo build -p liberum_cli &> /dev/null
CLI_BIN="./target/debug/liberum_cli"

# create ndoes
N_NAMES=()
N_IDS=()
N_ADDRESSES=()

set +x
printf "${BLUE}Skipping test logs for creating $NODE_COUNT nodes...${NC}\n"
for (( i = 1; i <= $NODE_COUNT; i++ )); do
    N="test_n$i"
    N_ADDR="${NODE_ADDR_PREFIX}$(($i + 52136))${NODE_ADDR_SUFFIX}"

    $CLI_BIN -d new-node $N --id-seed $i &> /dev/null
    $CLI_BIN -d config-node $N add-external-addr $N_ADDR &> /dev/null
    if [[ $i -gt 1 ]]; then
        $CLI_BIN -d config-node $N add-bootstrap-node "${N_IDS[$(($i - 2))]}" "${N_ADDRESSES[$(($i - 2))]}" &> /dev/null
    fi
    $CLI_BIN -d start-node $N &> /dev/null

    ID=$($CLI_BIN -d get-peer-id $N 2> /dev/null)
    N_NAMES+=("$N")
    N_IDS+=("$ID")
    N_ADDRESSES+=("$N_ADDR")
done
printf "${BLUE}Nodes created${NC}\n"
set -x

sleep 0.5

# create and provide file
echo "${FILE_CONTENT}" > "$FILE_NAME"
$CLI_BIN -d publish-file ${N_NAMES[0]} "$FILE_NAME" 2> /dev/null

# download file
RESULT=$($CLI_BIN -d download-file ${N_NAMES[$((variable - 1))]} "${BLAKE3_HASH}" 2> /dev/null)

# cleanup
set +x
echo "${BLUE}Skipping test logs for stopping nodes${NC}\n"
for (( i = 1; i <= $NODE_COUNT; i++ )); do
    $CLI_BIN -d stop-node ${N_NAMES[$i]} &> /dev/null
done
echo "${BLUE}Nodes stopped${NC}\n"
set -x

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
