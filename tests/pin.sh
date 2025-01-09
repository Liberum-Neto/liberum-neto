#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/58137/quic-v1"
N2="test_n2"
N2_SEED=2
N3="test_n3"
N3_SEED=3
N4="test_n4"
N4_SEED=4
FILE1_NAME="$PWD/test-file.txt"
FILE1_CONTENT="Hello, World!"
FILE2_NAME="$PWD/test-file2.txt"
FILE2_CONTENT="Howdy Ho!"

echo "Publish and download file test:"

# run daemon
killall liberum_core &> /dev/null
$CORE_BIN --daemon  &> /dev/null &
sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
$CLI_BIN new-node $N1 --id-seed $N1_SEED 2> /dev/null
$CLI_BIN new-node $N2 --id-seed $N2_SEED 2> /dev/null
$CLI_BIN new-node $N3 --id-seed $N3_SEED 2> /dev/null
$CLI_BIN new-node $N4 --id-seed $N4_SEED 2> /dev/null
$CLI_BIN config-node $N1 add-external-addr $N1_ADDR 2> /dev/null

# start n1 and get its peer id
$CLI_BIN start-node $N1 2> /dev/null
N1_ID=$($CLI_BIN get-peer-id $N1 2> /dev/null)

# add n1 as bootstrap
$CLI_BIN start-node $N2 2> /dev/null
$CLI_BIN dial $N2 "${N1_ID}" $N1_ADDR 2> /dev/null
$CLI_BIN start-node $N3 2> /dev/null
$CLI_BIN dial $N3 "${N1_ID}" $N1_ADDR 2> /dev/null
$CLI_BIN start-node $N4 2> /dev/null
$CLI_BIN dial $N4 "${N1_ID}" $N1_ADDR 2> /dev/null

# wait for nodes to connect
sleep 0.1

# create and publish file
echo "${FILE1_CONTENT}" > "$FILE1_NAME"
FILE1_ID=$($CLI_BIN publish-file $N1 "$FILE1_NAME" 2> /dev/null)
echo "${FILE2_CONTENT}" > "$FILE2_NAME"
FILE2_ID=$($CLI_BIN publish-file $N2 "$FILE2_NAME" --pins "$FILE1_ID" 2> /dev/null)

init_asserts
# # download file
RESULT=$($CLI_BIN download-file $N3 "${FILE1_ID}" 2> /dev/null)
should_contain "$RESULT" "${FILE1_CONTENT}"
# check if file2 contains pins
RESULT=$($CLI_BIN download-file $N3 "${FILE2_ID}" 2> /dev/null)
should_contain "$RESULT" "${FILE2_CONTENT}"
should_contain "$RESULT" "${FILE1_ID}"

# check if pinned files are available on other nodes

RESULT=$($CLI_BIN get-pinned $N1 "${FILE1_ID}" 2> /dev/null)
should_contain "$RESULT" "$FILE2_CONTENT"
should_contain "$RESULT" "$FILE2_ID"

RESULT=$($CLI_BIN get-pinned $N2 "${FILE1_ID}" 2> /dev/null)
should_contain "$RESULT" "$FILE2_CONTENT"
should_contain "$RESULT" "$FILE2_ID"

RESULT=$($CLI_BIN get-pinned $N3 "${FILE1_ID}" 2> /dev/null)
should_contain "$RESULT" "$FILE2_CONTENT"
should_contain "$RESULT" "$FILE2_ID"

RESULT=$($CLI_BIN get-pinned $N4 "${FILE1_ID}" 2> /dev/null)
should_contain "$RESULT" "$FILE2_CONTENT"
should_contain "$RESULT" "$FILE2_ID"

# cleanup
$CLI_BIN stop-node $N1 2> /dev/null
$CLI_BIN stop-node $N2 2> /dev/null
$CLI_BIN stop-node $N3 2> /dev/null
$CLI_BIN stop-node $N4 2> /dev/null
killall liberum_core &> /dev/null
rm "$FILE1_NAME"
rm "$FILE2_NAME"

exit $(check_asserts)
