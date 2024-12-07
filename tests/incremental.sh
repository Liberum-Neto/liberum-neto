#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

INIT_COUNT=5
NODE_COUNT=30

FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"

NODE_ADDR_PREFIX="/ip6/::1/udp/"
NODE_ADDR_SUFFIX="/quic-v1"

echo "Provide and download file test:"

# run daemon
# killall liberum_core &> /dev/null
# $CORE_BIN --daemon  &> /dev/null &
# sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create nodes
N_NAMES=()
N_IDS=()
N_ADDRESSES=()

set +x
printf "${BLUE}Skipping test logs for creating $NODE_COUNT nodes...${NC}\n"
for (( i = 1; i <= $INIT_COUNT; i++ )); do
    {
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
    }
done

printf "${BLUE}Nodes created${NC}\n"
set -x

# wait for nodes to connect
sleep 0.5

# create and provide file
echo "${FILE_CONTENT}" > "$FILE_NAME"
FILE_ID=$($CLI_BIN publish-file ${N_NAMES[0]} "$FILE_NAME" 2> /dev/null)

init_asserts
COUNT_PASS=0
COUNT_FAIL=0
set +x
for (( i = 1; i <= $NODE_COUNT; i++ )); do
    {
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

    sleep 0.5
    RESULT=$($CLI_BIN -d download-file ${N} "${FILE_ID}" 2> /dev/null)
    if [[ "$RESULT" == "$FILE_CONTENT" ]]; then
        COUNT_PASS=$((COUNT_PASS+1))
    else
        COUNT_FAIL=$((COUNT_FAIL+1))
    fi
    }
done

echo Pass: $COUNT_PASS
echo Fail: $COUNT_FAIL


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

exit $(check_asserts)
