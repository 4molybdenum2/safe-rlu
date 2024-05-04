.PHONY: all test benchmark bench-btree bench-rluset plot deps

all:
	cargo run --bin benchmark_rlu_set --release

test:
	cargo test

benchmark:
	cargo run --bin benchmark --release > bench.csv


benchmark-btree:
	cargo run --bin benchmark_btree_set --release > bench_btree.csv


benchmark-rluset:
	cargo run --bin benchmark_rlu_set --release > bench_rluset.csv


plot:
	python bench_plot.py


deps:
	pip install -r requirements.txt