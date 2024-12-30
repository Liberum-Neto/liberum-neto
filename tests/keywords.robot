*** Settings ***
Library    OperatingSystem

*** Variables ***
${DAEMON_LOG}=    /tmp/liberum-core/stdout.out
${CORE_BIN}=    ${CURDIR}/../target/debug/liberum_core
${CLI_BIN}=    ${CURDIR}/../target/debug/liberum_cli

*** Keywords ***
Setup
    Run Log    cargo build
    # Start Dameon
    Run Core    --daemon &> /dev/null
    Sleep    0.1s

Teardown
    Run    killall liberum_core &> /dev/null
    ${log}=    Run    cat ${DAEMON_LOG}
    Log    ${log}

Run Log
    [Arguments]    ${cmd}
    ${out}=    Run    ${cmd}
    Log    ${out}
    RETURN    ${out}

Run Cli
    [Arguments]    ${cmd}
    ${out}=    Run   ${CLI_BIN} ${cmd}
    Log    ${out}
    RETURN    ${out}

Run Core
    [Arguments]    ${cmd}
    ${out}=    Run   ${CORE_BIN} ${cmd}
    Log    ${out}
    RETURN    ${out}
