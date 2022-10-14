#!/bin/bash

export RUST_LOG="info"

for i in {0..500}
do
	./target/release/rust-worker &
done
