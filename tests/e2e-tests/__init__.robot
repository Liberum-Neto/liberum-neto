*** Settings ***
Library    Process
Library    OperatingSystem

*** Keywords ***
Suite Setup
    Set Suite Variable    ${CORE_BIN}    ${CURDIR}/../../target/debug/liberum_core
    Set Suite Variable    ${CLI_BIN}    ${CURDIR}/../../target/debug/liberum_cli
    Run Process    killall liberum_core &> /dev/null
    Run Process    $CORE_BIN --daemon  &> /dev/null
    Sleep    0.1s

Suite Teardown
    Run Process    killall liberum_core &> /dev/null
