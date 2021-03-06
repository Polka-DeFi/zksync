#!/bin/bash

set -e
trap cat_logs EXIT

TIMEOUT_PID=""
PROVER_PID=""
SERVER_PID=""

function timeout() {
    sleep 1200
    echo Timeout is reached
    kill -s TERM "$1"
}

timeout "$$" &
TIMEOUT_PID=$!

function cat_logs() {
    exitcode=$?
    echo Termination started

    # Wait for server to finish any ongoing jobs
    sleep 30

    set +e
    pkill -P $SERVER_PID
    pkill -P $PROVER_PID
    pkill -P $TIMEOUT_PID
    echo Server logs:
    cat rust-sdk-server.log 
    echo ===========
    echo Prover logs:
    cat rust-sdk-prover.log

    # Wait for server to be surely killed
    sleep 10

    exit $exitcode
}

zksync dummy-prover status | grep -q 'disabled' && zksync dummy-prover enable

zksync server &> rust-sdk-server.log &
SERVER_PID=$!
zksync dummy-prover &> rust-sdk-prover.log &
PROVER_PID=$!

sleep 10
echo Performing rust SDK tests...
cargo test -p zksync --release -- --ignored --test-threads=1
