#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52137/quic-v1"
N2="test_n2"
N2_SEED=2
FILE_NAME="test-file.txt"
FILE_CONTENT="Hello, World!"
BLAKE3_HASH="7cLWjV2o1VsqwkAnyDWK3UemS2psCBHjj865Dovpu4p1"
NONEXISTING_HASH="000000001VsqwkAnyDWK3UemS2psCBHjj865Dovpu4p1"
DOWNLOAD_FAILED_MSG="Failed to download file"
echo "Download Deleted file test:"

# run daemon
killall liberum_core &> /dev/null
cargo run -p liberum_core -- --daemon  &> /dev/null &
sleep 0.5; # the socket file is created asynchronously and may not be ready yet :))))

# create ndoes
cargo run -p liberum_cli -- -d new-node $N1 --id-seed $N1_SEED 2> /dev/null
cargo run -p liberum_cli -- -d new-node $N2 --id-seed $N2_SEED 2> /dev/null
cargo run -p liberum_cli -- -d config-node $N1 add-external-addr $N1_ADDR 2> /dev/null

# start n1 and get its peer id
cargo run -p liberum_cli -- -d start-node $N1 2> /dev/null
N1_ID=$(cargo run -p liberum_cli -- -d get-peer-id $N1 2> /dev/null)

# add n1 as bootstrap
cargo run -p liberum_cli -- -d config-node $N2 add-bootstrap-node "${N1_ID}" $N1_ADDR 2> /dev/null
cargo run -p liberum_cli -- -d start-node $N2 2> /dev/null

# create and provide file
echo "${FILE_CONTENT}" > "$FILE_NAME"

cargo run -p liberum_cli -- -d provide-file $N1 "$FILE_NAME" &> /dev/null


init_asserts
# download existing file
RESULT1=$(cargo run -p liberum_cli -- -d download-file $N2 "${BLAKE3_HASH}" 2> /dev/null)
should_contain "$RESULT1" "${FILE_CONTENT}"
rm "$FILE_NAME"

# download deleted file
RESULT2=$(cargo run -p liberum_cli -- -d download-file $N2 "${BLAKE3_HASH}" 2> /dev/null)
should_contain "$RESULT2" "${DOWNLOAD_FAILED_MSG}"

# download nonexisting file
RESULT3=$(cargo run -p liberum_cli -- -d download-file $N2 "nonexisting_hash" 2> /dev/null)
should_contain "$RESULT3" "${DOWNLOAD_FAILED_MSG}"

ALIVE=$(cargo run -p liberum_cli -- -d list-nodes 2> /dev/null | grep -c "true")
should_be_equal "$ALIVE" "2"

# cleanup
cargo run -p liberum_cli -- -d stop-node $N1 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N2 2> /dev/null
killall liberum_core &> /dev/null
rm "$FILE_NAME"

exit $(check_asserts)
