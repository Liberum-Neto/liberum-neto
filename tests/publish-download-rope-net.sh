#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR"/lib/asserts.sh

CORE_BIN=$1
CLI_BIN=$2

NODE_COUNT=100

FILE_NAME="$PWD/test-file.txt"
FILE_CONTENT="Hello, World!"

NODE_ADDR_PREFIX="/ip6/::1/udp/"
NODE_ADDR_SUFFIX="/quic-v1"

echo "Provide and download file test:"

# run daemon
killall liberum_core &> /dev/null
$CORE_BIN --daemon  &> /dev/null &
sleep 0.1; # the socket file is created asynchronously and may not be ready yet :))))

# create nodes
N_NAMES=()
N_IDS=()
N_ADDRESSES=()

# set +x
printf "${BLUE}Skipping test logs for creating $NODE_COUNT nodes...${NC}\n"
for (( i = 1; i <= $NODE_COUNT; i++ )); do
    {
    N="test_n$i"
    N_ADDR="${NODE_ADDR_PREFIX}$(($i + 53136))${NODE_ADDR_SUFFIX}"

    $CLI_BIN new-node $N --id-seed $i &> /dev/null
    $CLI_BIN config-node $N add-external-addr $N_ADDR &> /dev/null
    if [[ $i -gt 1 ]]; then
        MAX_N=$(( $i - 2 ))
        BNODE=$(shuf -i 0-$MAX_N -n 1)
        echo "Connecting $i with $BNODE"
        $CLI_BIN config-node $N add-bootstrap-node "${N_IDS[$(( $BNODE ))]}" "${N_ADDRESSES[$(( $BNODE ))]}" &> /dev/null
    fi
    $CLI_BIN start-node $N &> /dev/null

    ID=$($CLI_BIN get-peer-id $N 2> /dev/null)
    N_NAMES+=("$N")
    N_IDS+=("$ID")
    N_ADDRESSES+=("$N_ADDR")
    }
done

printf "${BLUE}Nodes created${NC}\n"
# set -x

# wait for nodes to connect
#sleep 0.5

# create and provide file
echo "${FILE_CONTENT}" > "$FILE_NAME"
FILE_ID=$($CLI_BIN provide-file ${N_NAMES[0]} "$FILE_NAME" 2> /dev/null)

init_asserts

RESULT=$($CLI_BIN download-file ${N_NAMES[$(($NODE_COUNT - 1))]} "${FILE_ID}" 2> /dev/null)
should_be_equal "$RESULT" "$FILE_CONTENT"


# cleanup
set +x
echo "${BLUE}Skipping test logs for stopping nodes${NC}\n"
for (( i = 1; i <= $NODE_COUNT; i++ )); do
    $CLI_BIN stop-node ${N_NAMES[$i]} &> /dev/null
done
echo "${BLUE}Nodes stopped${NC}\n"
set -x

killall liberum_core &> /dev/null
rm "$FILE_NAME"

exit $(check_asserts)
