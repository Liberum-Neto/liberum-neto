*** Settings ***
Resource    ../keywords.robot

Suite Setup    Run Keywords   Setup
...    AND    Run    echo -n ${FILE_CONTENT} > ${FILE_NAME}

Suite Teardown    Run Keywords   Teardown
...    AND    Run    rm ${FILE_NAME}

Library    String
Library    Collections

*** Variables ***

${NODE_ADDR_PREFIX}=    /ip6/::1/udp/
${NODE_ADDR_SUFFIX}=    /quic-v1
${NODE_BASE_PORT}=    52137
${NODE_COUNT}=    10
${FILE_NAME}=    ${CURDIR}/test_file.txt
${FILE_CONTENT}=    Hello, World!
${FILE_ID}=    3jFpcrWSnKvfEaYPCiXAujR1JiBaKVs4gEMRW4sTVn63

*** Test Cases ***
Publish Download Rope
    # Create nodes
    ${NODE_NAMES}=    Create List    ${NODE_COUNT}
    ${NODE_IDS}=    Create List    ${NODE_COUNT}
    ${NODE_SEEDS}=    Create List    ${NODE_COUNT}
    ${NODE_ADDRS}=    Create List    ${NODE_COUNT}
    FOR    ${i}    IN RANGE    ${NODE_COUNT}
        ${NODE_NAME}=    test_node_${i}
        ${NODE_ADDR}=    ${NODE_ADDR_PREFIX}${${NODE_BASE_PORT}+${i}}${NODE_ADDR_SUFFIX}
        Set List Value    ${NODE_NAMES}    ${i}    ${NODE_NAME}
        Set List Value    ${NODE_ADDRS}    ${i}    ${NODE_ADDR}
        Run Cli     -d new-node ${NODE_NAMES}[${i}] --id-seed ${i} 2> /dev/null
        IF    ${i} > 1
            Run Cli    -d config-node ${NODE_NAMES}[${i}] add-bootstrap-node ${NODE_IDS}[${i}-1] ${NODE_ADDRS[${i}-1]} 2> /dev/null
        END
        ${N_ID}=    Run Cli    -d get-peer-id ${NODE_NAMES}[${i}] 2> /dev/null
        Set List Value    ${NODE_IDS}    ${i}    ${N_ID}
    END
    Sleep    0.1s    # Wait for nodes to connect

    Run Cli     -d publish-file ${NODE_NAMES}[0] ${FILE_NAME}

    ${FILE}=    Run Cli    -d download-file ${NODE_NAMES}[${NODE_COUNT}-1] ${FILE_ID} 2> /dev/null
    Should Match Regexp    ${FILE}    ${FILE_CONTENT}
