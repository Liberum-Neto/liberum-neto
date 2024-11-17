#!/bin/bash

N1="test_n1"
N1_SEED=1
N1_ADDR="/ip6/::1/udp/52177/quic-v1"
N2="test_n2"
N2_SEED=2
FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"
BLAKE3_HASH="7cLWjV2o1VsqwkAnyDWK3UemS2psCBHjj865Dovpu4p1"

echo "Provide and download file test:"

# run daemon
killall liberum_core &> /dev/null
cargo run -p liberum_core -- --daemon &> /dev/null &
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
cargo run -p liberum_cli -- -d dial $N2 "${N1_ID}" $N1_ADDR 2> /dev/null


# create and provide file
echo "${FILE_CONTENT}" > "$FILE_NAME"

cargo run -p liberum_cli -- -d publish-file $N1 "$FILE_NAME" &> /dev/null

# download file
PROVIDERS_RESULT=$(cargo run -p liberum_cli -- -d get-providers $N2 "${BLAKE3_HASH}" 2> /dev/null)
RESULT1=$(cargo run -p liberum_cli -- -d download-file-rr $N1 "${BLAKE3_HASH}" 2> /dev/null)
RESULT2=$(cargo run -p liberum_cli -- -d download-file-rr $N2 "${BLAKE3_HASH}" 2> /dev/null)

# cleanup
cargo run -p liberum_cli -- -d stop-node $N1 2> /dev/null
cargo run -p liberum_cli -- -d stop-node $N2 2> /dev/null
# killall liberum_core &> /dev/null
rm "$FILE_NAME"

# check result
RESULT=1
if [[ -z "${PROVIDERS_RESULT}" ]]; then
    echo "\"${PROVIDERS_RESULT}\" is empty\""
    RESULT=0
fi
if [[ ! "${RESULT1}" =~ "${FILE_CONTENT}" ]]; then
    echo "\"${RESULT1}\" does not contain \"${FILE_CONTENT}\""
    RESULT=0
fi
if [[ ! "${RESULT2}" =~ "${FILE_CONTENT}" ]]; then
    echo "\"${RESULT2}\" does not contain \"${FILE_CONTENT}\""
    RESULT=0
fi

if [[ "${RESULT}" == "1" ]]; then
    echo "Success"
    exit 0
else
    echo "Failure"
    exit 1
fi
